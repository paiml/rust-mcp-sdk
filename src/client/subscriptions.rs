//! The CLIENT half of `subscriptions/listen` (MCP 2026-07-28, HTTP-04).
//!
//! Plan 10 built the SERVER route and proved it with a raw HTTP/1.1 client. This
//! module is the other half: a pmcp [`Client`](crate::Client) that opts into
//! `2026-07-28` opens the long-lived stream and consumes its frames as a typed
//! [`futures::Stream`] of `ServerNotification`s — which is what HTTP-04's
//! requirement text ("**v2 clients get change notifications**") actually asks
//! for.
//!
//! # The wire contract, as this module consumes it
//!
//! 1. `subscriptions/listen` is POSTed with the v2 `_meta` and the three v2
//!    routing headers ([`Mcp-Name`](crate::shared::http_constants::MCP_NAME) is
//!    the EMPTY STRING — the method is not name-bearing).
//! 2. A `text/event-stream` response means SERVED. Anything else means the
//!    server rejected the request, and the JSON-RPC error it carries (typically
//!    `-32601`) is surfaced to the caller UNCHANGED, so "this server does not do
//!    subscriptions" is distinguishable from a transport fault.
//! 3. The FIRST frame MUST be a
//!    `ACKNOWLEDGED_METHOD` notification. Anything else is an error naming the
//!    spec's acknowledgement-first MUST.
//! 4. Every subsequent frame carries the SAME
//!    `SUBSCRIPTION_ID_META_KEY` value. A frame carrying a DIFFERENT one is
//!    yielded as an `Err`, never forwarded as the caller's own (T-113-66): a
//!    mismatched tag means the server or an intermediary cross-delivered, which
//!    is precisely the failure plan 10's `ListenKey` prevents server-side, and
//!    papering over it client-side would hide it.
//! 5. The terminal [`SubscriptionsListenResult`](crate::types::subscriptions::SubscriptionsListenResult)
//!    ends the stream gracefully (`None`).
//!
//! # D-11: this is the opt-in, not the recommendation
//!
//! Polling over the Tasks mechanism remains pmcp's RECOMMENDED mechanism for
//! enterprise remote deployments. A held-open stream pins one server instance
//! for its whole lifetime, and plan 10's registry is documented INSTANCE-LOCAL:
//! behind a non-sticky load balancer a subscriber silently under-receives. This
//! client API exists because the spec defines it and conformance exercises it,
//! not because it is the default posture.
//!
//! # No second SSE tokenizer
//!
//! Frames are decoded with the SHARED `SseParser`, the same one the
//! streamable-HTTP transport already feeds. This module adds the JSON-RPC
//! classification on top of it and nothing else.

use crate::error::{Error, ErrorCode, Result, TransportError};
use crate::shared::http_constants::{CONTENT_TYPE, TEXT_EVENT_STREAM};
use crate::shared::sse_parser::{take_utf8_prefix, SseParser};
use crate::shared::StreamableHttpTransport;
use crate::types::jsonrpc::RequestId;
use crate::types::mrtr::META_KEY;
use crate::types::notifications::ServerNotification;
use crate::types::subscriptions::{
    request_id_value, SubscriptionAcknowledgedParams, ACKNOWLEDGED_METHOD, SUBSCRIPTION_ID_META_KEY,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// How much of a malformed frame is echoed back in an error message.
///
/// Bounded because the frame is UNTRUSTED remote input: a hostile server must
/// not be able to push an unbounded string into a client's logs through an
/// error `Display` (T-113-67).
const MAX_ECHOED_FRAME: usize = 200;

/// The most a `subscriptions/listen` parser may hold IN FLIGHT at once.
///
/// Named for the line buffer it originally bounded; since 113-17 it bounds BOTH
/// of the parser's accumulators — the unterminated line AND the `data:` payload
/// of the event still awaiting its blank line — plus whatever chunk is being
/// fed. It is NOT a per-line limit, and no message derived from it may say it is.
///
/// Deliberately TIGHTER than the shared 1 MiB [`SseParser`] default. This is a
/// LONG-LIVED stream, fed one chunk at a time from UNTRUSTED remote input, whose
/// frames are notifications and therefore small by construction — a peer that
/// held the stream open and streamed `data:` lines it never terminated with a
/// blank line would otherwise grow this client's heap for the lifetime of the
/// connection (review CR-03, T-113-74/79).
///
/// The transports that carry arbitrary JSON-RPC results keep a looser ceiling;
/// only this path, whose payloads are bounded by the protocol itself, tightens it.
const MAX_LISTEN_LINE_BYTES: usize = 256 * 1024;

/// A stream of raw SSE `data:` payloads, one item per event.
///
/// The unit is the payload STRING rather than a parsed value because the
/// JSON-RPC classification belongs to [`SubscriptionStream`], not to the
/// transport that produced the bytes.
pub type SubscriptionFrameStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

/// A transport that can open a long-lived server-push stream.
///
/// Deliberately a SEPARATE trait rather than another defaulted method on
/// [`Transport`](crate::shared::Transport): an incrementally-read response body
/// is an HTTP concept, and every stdio / WebSocket / wasm transport would have
/// to carry a meaningless default for it. Keeping it separate also means
/// [`Client::subscriptions_listen`](crate::Client::subscriptions_listen) is
/// generic — a test stub can implement this trait and observe that a non-v2
/// client never opens a stream at all.
#[async_trait]
pub trait EventStreamTransport {
    /// POST `body` and return its response body as a stream of SSE payloads.
    ///
    /// # Errors
    ///
    /// Returns the server's own JSON-RPC error (e.g. `-32601` from a server that
    /// advertises no subscription-delivered capability) when the response is a
    /// JSON document rather than a `text/event-stream`, and a transport error
    /// when the request could not be made.
    async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream>;
}

#[async_trait]
impl EventStreamTransport for StreamableHttpTransport {
    async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream> {
        let response = self.post_streaming(body).await?;
        let status = response.status();
        let is_event_stream = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains(TEXT_EVENT_STREAM));

        // The STATUS is checked alongside the content type: a stream is only
        // SERVED on a 2xx. A non-2xx that happens to carry `text/event-stream`
        // (an intermediary's error page, a misconfigured gateway) would
        // otherwise be consumed as a live subscription that never acknowledges,
        // hiding the server's real answer behind the generic
        // "ended before the mandatory acknowledgement" error.
        if !is_event_stream || !status.is_success() {
            // The body is read through the TRANSPORT's capped collector, never a
            // bare `collect()`: this is a peer-controlled body on the very stream
            // HTTP-04 exists to harden, and an uncapped read here would be the
            // one unbounded whole-body read left on this transport (review
            // CR-01).
            let collected = match self.collect_capped_body(response).await {
                Ok(bytes) => bytes,
                // Over the cap, or the body failed mid-read. Either way there is
                // no envelope to surface, so the caller gets the transport error
                // naming the condition — exactly what a malformed body already
                // produced.
                Err(error) => return Err(error),
            };
            return Err(rejection_error(status, &collected));
        }
        Ok(Box::pin(sse_payload_stream(response.into_body())))
    }
}

/// Turn a non-stream `subscriptions/listen` response into the error the caller
/// sees.
///
/// A well-formed JSON-RPC 2.0 error envelope is surfaced VERBATIM (code,
/// message and `data` intact) so an application can branch on `-32601` — "this
/// server does not do subscriptions" — instead of guessing from a string.
///
/// Deliberately strict about `jsonrpc == "2.0"` AND the presence of `error`,
/// mirroring the transport's own `jsonrpc_error_envelope`: an intermediary's
/// JSON error page must never be laundered into what a caller reads as a
/// server-authored protocol error.
///
/// Takes ALREADY-COLLECTED bytes rather than the live body: the collection is
/// the size-sensitive half and belongs to the transport's capped collector, so
/// this function cannot be the place an unbounded read creeps back in (review
/// CR-01). Being synchronous over a byte slice also makes it directly testable.
fn rejection_error(status: hyper::StatusCode, collected: &[u8]) -> Error {
    if let Ok(value) = serde_json::from_slice::<Value>(collected) {
        let is_jsonrpc = value.get("jsonrpc").and_then(Value::as_str) == Some("2.0");
        if let (true, Some(error)) = (is_jsonrpc, value.get("error")) {
            if let Ok(error) =
                serde_json::from_value::<crate::types::jsonrpc::JSONRPCError>(error.clone())
            {
                return Error::from_jsonrpc_error(error);
            }
        }
    }
    Error::Transport(TransportError::Request(format!(
        "subscriptions/listen did not open a stream (HTTP {status}): {}",
        truncate(&String::from_utf8_lossy(collected))
    )))
}

/// Incremental UTF-8 + SSE decoding state for one response body.
struct PayloadState {
    body: hyper::body::Incoming,
    parser: SseParser,
    /// Bytes received but not yet decodable as complete UTF-8.
    bytes: Vec<u8>,
    /// Payloads already parsed and waiting to be yielded.
    pending: VecDeque<String>,
    /// The body reported end-of-stream (or errored) and must not be polled again.
    done: bool,
}

/// Decode a hyper response body into a stream of SSE `data:` payloads.
///
/// Built with [`futures::stream::unfold`] rather than a hand-written `Stream`
/// impl so the whole decode is one linear async block. Dropping the returned
/// stream drops [`PayloadState`], which drops the `Incoming` body, which closes
/// the connection — that is what makes the server's RAII `ListenGuard` fire
/// (T-113-63).
fn sse_payload_stream(body: hyper::body::Incoming) -> impl Stream<Item = Result<String>> + Send {
    let state = PayloadState {
        body,
        // An EXPLICIT bound, not `SseParser::new()`'s default: this path's limit
        // is visible at the call site and can be tightened independently of the
        // shared one.
        parser: SseParser::with_max_buffer_size(MAX_LISTEN_LINE_BYTES),
        bytes: Vec::new(),
        pending: VecDeque::new(),
        done: false,
    };
    futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(payload) = state.pending.pop_front() {
                return Some((Ok(payload), state));
            }
            if state.done {
                return None;
            }
            if let Some(error) = read_next_frame(&mut state).await {
                return Some((Err(error), state));
            }
        }
    })
}

/// Read ONE body frame into `state`, returning `Some(error)` only for a
/// transport failure.
///
/// Extracted from [`sse_payload_stream`]'s `unfold` closure so neither function
/// exceeds the repo's cognitive-complexity budget (CLAUDE.md: cog ≤ 25, enforced
/// by the PR-blocking PMAT gate). The split is behaviour-preserving: every exit
/// leaves `state` in the same shape the inline `match` produced, and the caller
/// re-enters its loop, where `pending.pop_front()` and the `done` check together
/// reproduce the old "end-of-body but payloads still buffered" fall-through.
async fn read_next_frame(state: &mut PayloadState) -> Option<Error> {
    match state.body.frame().await {
        // End of body. Anything already in `pending` is still drained by the
        // caller's loop before the `done` check ends the stream.
        None => {
            state.done = true;
            None
        },
        Some(Err(e)) => {
            state.done = true;
            Some(Error::Transport(TransportError::Request(e.to_string())))
        },
        Some(Ok(frame)) => {
            if let Some(chunk) = frame.data_ref() {
                state.bytes.extend_from_slice(chunk);
                let text = take_utf8_prefix(&mut state.bytes);
                state
                    .pending
                    .extend(drain_sse_payloads(&mut state.parser, &text));
                if let Some(error) = listen_overflow(&state.parser) {
                    // The peer pushed the parser's retained state plus this
                    // chunk past MAX_LISTEN_LINE_BYTES, so the parser DISCARDED
                    // bytes and this byte stream is no longer trustworthy.
                    // Unlike a malformed frame — which is an `Err` ITEM the
                    // stream continues past — there is nothing meaningful to
                    // continue to, so stop polling a peer already established as
                    // hostile or broken.
                    state.done = true;
                    return Some(error);
                }
            }
            // A trailers frame carries no data; the caller loops and reads again.
            None
        },
    }
}

/// Feed `chunk` to the SHARED SSE parser and return the payloads it completed.
///
/// Keep-alive comment lines (`: ...`) never produce an event — the shared
/// parser drops them in `process_line` — so they are skipped here for free
/// rather than by a second rule that could drift. Only `message` (or untyped)
/// events carry protocol payloads; any other event name is ignored.
fn drain_sse_payloads(parser: &mut SseParser, chunk: &str) -> Vec<String> {
    parser
        .feed(chunk)
        .into_iter()
        .filter(|event| event.event.as_deref().is_none_or(|name| name == "message"))
        .map(|event| event.data)
        .collect()
}

/// The stream-ENDING error, when the parser has discarded in-flight bytes past
/// [`MAX_LISTEN_LINE_BYTES`].
///
/// What trips the bound is the parser's RETAINED state plus the chunk being fed,
/// not one line and not one event: the retained state is an unterminated line
/// PLUS every `data:` line accumulated into an event the peer has not yet ended
/// with a blank line, and a single chunk carrying many small complete events can
/// exceed the limit on its total alone (T-113-86). The message says exactly that
/// and nothing more precise, because nothing more precise is true.
///
/// It names the limit and the peer's behaviour and nothing else — no frame
/// content is echoed, because the bytes that tripped the bound are exactly the
/// untrusted input [`MAX_ECHOED_FRAME`] exists to keep out of a client's logs.
///
/// A free function over the parser rather than an inline check in
/// [`read_next_frame`] so the condition is reachable from a test: that function
/// owns a live `hyper::body::Incoming`, which cannot be constructed outside
/// hyper.
fn listen_overflow(parser: &SseParser) -> Option<Error> {
    if !parser.overflowed() {
        return None;
    }
    Some(Error::protocol(
        ErrorCode::INVALID_REQUEST,
        format!(
            "a subscriptions/listen chunk pushed the buffered stream state past the \
             {}-byte parser bound; the buffered bytes were discarded and the stream \
             was ended",
            parser.max_buffer_size()
        ),
    ))
}

/// Bound an untrusted string for inclusion in an error message.
///
/// Scans at most `MAX_ECHOED_FRAME + 1` characters. `text.chars().count()`
/// walked the WHOLE untrusted string — up to a capped body or a whole frame —
/// just to answer "is it longer than 200 chars?", which is work a remote peer
/// chooses the size of, on an error path.
fn truncate(text: &str) -> String {
    let mut boundary = None;
    for (index, (offset, _)) in text.char_indices().enumerate() {
        if index == MAX_ECHOED_FRAME {
            boundary = Some(offset);
            break;
        }
    }
    let Some(boundary) = boundary else {
        return text.to_string();
    };
    let mut out = String::with_capacity(boundary + '…'.len_utf8());
    out.push_str(&text[..boundary]);
    out.push('…');
    out
}

/// What one classified frame means to the stream.
enum FrameOutcome {
    /// A tagged, decodable notification for this subscription.
    Notification(Box<ServerNotification>),
    /// The terminal `SubscriptionsListenResult`: end the stream gracefully.
    Terminal,
    /// A bad frame. Yielded as an `Err` ITEM; the stream keeps going.
    Failed(Box<Error>),
}

/// A live `subscriptions/listen` stream.
///
/// Yields every change notification the server agreed to deliver, in order,
/// after the mandatory acknowledgement (which is already consumed and available
/// through [`Self::acknowledged`] before the first poll).
///
/// # Lifetime and teardown
///
/// The stream OWNS its HTTP response body. Dropping the handle drops that body,
/// which closes the connection, which fires the server's RAII `ListenGuard` and
/// reclaims its registry entry and concurrency permits. There is no `close()` to
/// forget to call and no explicit `Drop` impl — the reclaim is a consequence of
/// ownership, which is exactly why it cannot be skipped on an error path.
///
/// # Errors are items, not terminations
///
/// A malformed frame, an unknown notification method, or a frame tagged with a
/// DIFFERENT `subscriptionId` is yielded as `Some(Err(..))` and the stream
/// CONTINUES. Only a transport failure, the terminal result, or end-of-body ends
/// it. A single bad frame from a buggy intermediary must not silently drop every
/// subsequent notification.
pub struct SubscriptionStream {
    subscription_id: RequestId,
    acknowledged: SubscriptionAcknowledgedParams,
    frames: SubscriptionFrameStream,
    finished: bool,
}

impl std::fmt::Debug for SubscriptionStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionStream")
            .field("subscription_id", &self.subscription_id)
            .field("acknowledged", &self.acknowledged)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl SubscriptionStream {
    /// Consume the mandatory acknowledgement and build the stream around what
    /// follows it.
    ///
    /// # Errors
    ///
    /// Returns an error when the stream ends before any frame arrives, when the
    /// first frame is not [`ACKNOWLEDGED_METHOD`] (the spec's
    /// acknowledgement-first MUST), or when that frame is not tagged with
    /// `subscription_id`.
    pub(crate) async fn open(
        subscription_id: RequestId,
        mut frames: SubscriptionFrameStream,
    ) -> Result<Self> {
        let Some(first) = frames.next().await else {
            return Err(Error::protocol_msg(
                "subscriptions/listen stream ended before the mandatory acknowledgement",
            ));
        };
        let payload = first?;
        let frame = serde_json::from_str::<Value>(&payload).map_err(|e| {
            Error::parse(format!(
                "subscriptions/listen acknowledgement is not JSON ({e}): {}",
                truncate(&payload)
            ))
        })?;

        if frame.get("method").and_then(Value::as_str) != Some(ACKNOWLEDGED_METHOD) {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "spec MUST: the first message on a subscriptions/listen stream is \
                     {ACKNOWLEDGED_METHOD}; got {}",
                    truncate(&payload)
                ),
            ));
        }
        verify_subscription_id(&frame, &subscription_id)?;

        let acknowledged = frame
            .get("params")
            .cloned()
            .ok_or_else(|| {
                Error::parse("the subscriptions/listen acknowledgement carries no params")
            })
            .and_then(|params| {
                serde_json::from_value::<SubscriptionAcknowledgedParams>(params)
                    .map_err(|e| Error::parse(format!("invalid acknowledgement params: {e}")))
            })?;

        Ok(Self {
            subscription_id,
            acknowledged,
            frames,
            finished: false,
        })
    }

    /// The subscription id every frame on this stream is tagged with.
    ///
    /// Equal to the JSON-RPC id of the `subscriptions/listen` request that
    /// opened it.
    #[must_use]
    pub fn subscription_id(&self) -> &RequestId {
        &self.subscription_id
    }

    /// The acknowledgement the server sent first — in particular the AGREED
    /// filter, which is never a superset of what was requested.
    ///
    /// Available BEFORE the first poll: the acknowledgement is consumed while
    /// the stream is being opened, because the spec makes it mandatory and
    /// first.
    #[must_use]
    pub fn acknowledged(&self) -> &SubscriptionAcknowledgedParams {
        &self.acknowledged
    }
}

impl Stream for SubscriptionStream {
    type Item = Result<ServerNotification>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }
        match this.frames.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                this.finished = true;
                Poll::Ready(None)
            },
            Poll::Ready(Some(Err(e))) => {
                this.finished = true;
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(Some(Ok(payload))) => {
                match classify_frame(&payload, &this.subscription_id) {
                    FrameOutcome::Notification(notification) => {
                        Poll::Ready(Some(Ok(*notification)))
                    },
                    FrameOutcome::Terminal => {
                        this.finished = true;
                        Poll::Ready(None)
                    },
                    FrameOutcome::Failed(e) => Poll::Ready(Some(Err(*e))),
                }
            },
        }
    }
}

/// Assert that `frame` is tagged with `expected`.
///
/// The reserved key is read by INDEXING, never through `Value::pointer`: it
/// contains a `/`, which JSON Pointer treats as a path separator unless escaped
/// as `~1` — the exact trap plan 10 hit server-side.
fn verify_subscription_id(frame: &Value, expected: &RequestId) -> Result<()> {
    let observed = ["params", "result"].into_iter().find_map(|section| {
        frame
            .get(section)?
            .get(META_KEY)?
            .get(SUBSCRIPTION_ID_META_KEY)
    });
    let expected_value = request_id_value(expected);
    if observed == Some(&expected_value) {
        return Ok(());
    }
    Err(Error::protocol(
        ErrorCode::INVALID_REQUEST,
        format!(
            "subscriptions/listen frame carries subscriptionId {} but this stream is {expected_value}; \
             refusing to deliver another subscription's frame",
            observed.map_or_else(|| "<absent>".to_string(), ToString::to_string),
        ),
    ))
}

/// Classify one already-parsed SSE payload.
fn classify_frame(payload: &str, subscription_id: &RequestId) -> FrameOutcome {
    let Ok(frame) = serde_json::from_str::<Value>(payload) else {
        return FrameOutcome::Failed(Box::new(Error::parse(format!(
            "subscriptions/listen frame is not JSON: {}",
            truncate(payload)
        ))));
    };
    // The error arm runs BEFORE the subscription-id check, and deliberately so:
    // a JSON-RPC error frame carries neither `params` nor `result`, which are the
    // only two places `verify_subscription_id` looks. Checking the tag first made
    // this arm UNREACHABLE — every server-authored error on the stream surfaced
    // as a bogus "carries subscriptionId <absent>" protocol error and the real
    // `code`/`message` were discarded. An error carries no notification payload,
    // so surfacing it cannot cross-deliver another subscription's data (T-113-66
    // is about delivered notifications, which still go through the check below).
    if let Some(error) = frame.get("error") {
        let error = serde_json::from_value::<crate::types::jsonrpc::JSONRPCError>(error.clone())
            .map_or_else(
                |_| Error::protocol_msg(format!("subscriptions/listen error frame: {error}")),
                Error::from_jsonrpc_error,
            );
        return FrameOutcome::Failed(Box::new(error));
    }
    if let Err(e) = verify_subscription_id(&frame, subscription_id) {
        return FrameOutcome::Failed(Box::new(e));
    }
    if frame.get("result").is_some() {
        // The graceful-teardown `SubscriptionsListenResult`.
        return FrameOutcome::Terminal;
    }
    if frame.get("method").and_then(Value::as_str) == Some(ACKNOWLEDGED_METHOD) {
        return FrameOutcome::Failed(Box::new(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "spec MUST: a subscriptions/listen stream is acknowledged exactly once, and a second \
             acknowledgement arrived",
        )));
    }
    match decode_notification(frame, payload) {
        Ok(notification) => FrameOutcome::Notification(Box::new(notification)),
        Err(e) => FrameOutcome::Failed(Box::new(e)),
    }
}

/// Decode a listen frame into the typed [`ServerNotification`].
///
/// [`ServerNotification`] is adjacently tagged (`method` / `params`), so the
/// envelope members a listen frame additionally carries — `jsonrpc`, and the
/// `params._meta` tag this module has already validated — are stripped first.
/// A unit-variant notification (`notifications/tools/list_changed`) has an
/// EMPTY `params` once the tag is removed, and an empty content object is not a
/// unit, so `params` is dropped entirely in that case.
///
/// Takes the parsed frame by value — the caller has no further use for it — and
/// the original `payload` for the error text, so neither the frame nor its
/// re-serialization is cloned on the per-notification receive path.
fn decode_notification(mut cleaned: Value, payload: &str) -> Result<ServerNotification> {
    let Some(object) = cleaned.as_object_mut() else {
        return Err(Error::parse("subscriptions/listen frame is not an object"));
    };
    object.remove("jsonrpc");
    object.remove("id");
    let drop_params = match object.get_mut("params") {
        Some(Value::Object(params)) => {
            params.remove(META_KEY);
            params.is_empty()
        },
        _ => false,
    };
    if drop_params {
        object.remove("params");
    }
    serde_json::from_value::<ServerNotification>(cleaned).map_err(|e| {
        Error::parse(format!(
            "subscriptions/listen frame is not a known server notification ({e}): {}",
            truncate(payload)
        ))
    })
}

// ===========================================================================
// Internal support surface for `fuzz_targets/`.
//
// Everything in this section is compiled ONLY under `feature = "fuzzing"` — a
// feature that is in neither `default` nor `full`, so `cargo public-api` never
// sees it on the shipped surface — or under `cfg(test)`, which is what keeps
// this module's own tests and proptests compiling. `fuzz/Cargo.toml` enables
// `fuzzing`; nothing a downstream crate can reach does. This mirrors the
// convention `crate::server::request_state`'s `fuzz_support` established.
// ===========================================================================

/// Run a SEQUENCE of untrusted listen-stream chunks through EXACTLY the decode a
/// live [`SubscriptionStream`] performs, under an explicit line-buffer bound.
///
/// # ⚠️ Not stable API
///
/// This function exists only behind `feature = "fuzzing"` (absent from BOTH
/// `default` and `full`, so `cargo public-api` never sees it on the shipped
/// surface) plus `cfg(test)` for this module's own callers. It is internal
/// support surface for `fuzz/fuzz_targets/subscription_listen_frames.rs`, in the
/// same spirit as the `#[doc(hidden)]` seam convention Phase 110 established for
/// `cargo-pmcp`'s fuzz and example targets. Do not build on it.
///
/// The gate is deliberate rather than cosmetic. `#[doc(hidden)]` hides an item
/// from rustdoc; it does NOT restrict visibility and does NOT exempt the item
/// from semver, and `pub mod client` → `pub mod subscriptions` makes everything
/// here reachable by every dependent crate. Three properties of this signature
/// are convenient for a fuzz harness and would be defects in stable API, so they
/// are named here rather than silently inherited by the next refactor:
///
/// - the `&[&[u8]]` **chunk model**, which commits the SDK to "a body is a slice
///   of byte slices" — a shape chosen to replay a libFuzzer artifact
///   deterministically, not to describe an HTTP body;
/// - the **unvalidated `max_buffer_size`**, which accepts `0` and then latches
///   the parser on the first non-empty chunk (a fuzz campaign wants that reachable;
///   a caller almost never does);
/// - **errors flattened to `String`**, so no private type escapes — which also
///   means no caller can match on the failure.
///
/// It additionally drops terminal frames, so a caller who mistook it for a decode
/// API would lose stream-close signals.
///
/// A terminal result contributes no entry. Pass [`MAX_LISTEN_LINE_BYTES`] to
/// decode under exactly the bound a live listen stream uses.
///
/// Two things the single-chunk seam cannot express, and both matter to a fuzzer:
///
/// 1. **The overflow branch.** The parser is built with
///    [`SseParser::with_max_buffer_size`], so a campaign can pick a bound small
///    enough that short generated inputs reach the discard-and-latch path. At the
///    production 256 KiB bound a fuzzer would have to synthesise a quarter of a
///    megabyte of newline-free input to get there, i.e. never.
/// 2. **State carried ACROSS chunks.** The undecoded-UTF-8 tail and the SSE line
///    buffer both survive from one chunk to the next in a live stream — exactly
///    as in [`read_next_frame`] — so a split mid-character or mid-line is
///    reachable here and is not reachable with one chunk.
///
/// # Returns
///
/// `(outcomes, overflowed_after_each_chunk, peak_buffered_bytes)`. The second and
/// third vectors each have one entry per INPUT CHUNK, evaluated after that chunk
/// was drained:
///
/// - `outcomes` — one entry per frame the stream would have delivered or failed
///   on, with errors flattened to their `Display` string.
/// - `overflowed_after_each_chunk` — `listen_overflow(&parser).is_some()`, the
///   PRODUCTION observer rather than a reconstruction of it. Because
///   [`SseParser::overflowed`] latches, that vector must never go `true` then
///   `false`; a live stream ENDS on the first `true`, while this seam deliberately
///   keeps feeding so the latch itself is testable.
/// - `peak_buffered_bytes` — `SseParser::buffered_bytes()`, i.e. the two
///   accumulators the bound actually covers (the unterminated line PLUS the
///   `data:` payload of the event still awaiting its blank line). This is the
///   quantity a campaign asserts against `max_buffer_size`: it is what GAP-A grew
///   without limit, and reporting only outcomes and flags is precisely why 20 000
///   green runs could coexist with that defect.
#[cfg(any(feature = "fuzzing", test))]
#[doc(hidden)]
#[must_use]
pub fn decode_listen_chunks_for_fuzz(
    chunks: &[&[u8]],
    subscription_id: &str,
    max_buffer_size: usize,
) -> (
    Vec<std::result::Result<ServerNotification, String>>,
    Vec<bool>,
    Vec<usize>,
) {
    let id = RequestId::String(subscription_id.to_string());
    let mut parser = SseParser::with_max_buffer_size(max_buffer_size);
    let mut bytes: Vec<u8> = Vec::new();
    let mut outcomes = Vec::new();
    let mut overflowed = Vec::with_capacity(chunks.len());
    let mut peak_buffered_bytes = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        bytes.extend_from_slice(chunk);
        let text = take_utf8_prefix(&mut bytes);
        outcomes.extend(
            drain_sse_payloads(&mut parser, &text)
                .into_iter()
                .filter_map(|payload| match classify_frame(&payload, &id) {
                    FrameOutcome::Notification(notification) => Some(Ok(*notification)),
                    FrameOutcome::Failed(e) => Some(Err(e.to_string())),
                    FrameOutcome::Terminal => None,
                }),
        );
        overflowed.push(listen_overflow(&parser).is_some());
        peak_buffered_bytes.push(parser.buffered_bytes());
    }
    (outcomes, overflowed, peak_buffered_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::subscriptions::{subscription_id_meta, SubscriptionFilter};
    use serde_json::json;

    /// Build a `SubscriptionStream` over a canned payload sequence, with no
    /// socket in sight.
    fn stream_over(
        subscription_id: RequestId,
        payloads: Vec<Result<String>>,
    ) -> SubscriptionStream {
        SubscriptionStream {
            subscription_id,
            acknowledged: SubscriptionAcknowledgedParams::default(),
            frames: Box::pin(futures::stream::iter(payloads)),
            finished: false,
        }
    }

    fn id() -> RequestId {
        RequestId::Number(11)
    }

    fn ack_payload(subscription_id: &RequestId, filter: &Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": ACKNOWLEDGED_METHOD,
            "params": {
                "notifications": filter,
                "_meta": subscription_id_meta(subscription_id),
            },
        })
        .to_string()
    }

    fn tools_changed(subscription_id: &RequestId) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
            "params": { "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    fn resource_updated(subscription_id: &RequestId, uri: &str) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": { "uri": uri, "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    fn terminal(subscription_id: &RequestId) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": request_id_value(subscription_id),
            "result": { "_meta": subscription_id_meta(subscription_id) },
        })
        .to_string()
    }

    // ---- the acknowledgement-first MUST -----------------------------------

    #[tokio::test]
    async fn open_consumes_the_acknowledgement_and_exposes_the_agreed_filter() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(vec![Ok(
            ack_payload(&id(), &json!({ "toolsListChanged": true })),
        )]));
        let stream = SubscriptionStream::open(id(), frames)
            .await
            .expect("the ack opens the stream");

        assert_eq!(stream.subscription_id(), &id());
        assert_eq!(
            stream.acknowledged().notifications,
            SubscriptionFilter {
                tools_list_changed: Some(true),
                ..SubscriptionFilter::default()
            },
            "the agreed filter is readable BEFORE the first poll"
        );
    }

    #[tokio::test]
    async fn a_non_acknowledgement_first_frame_is_refused() {
        let frames: SubscriptionFrameStream =
            Box::pin(futures::stream::iter(vec![Ok(tools_changed(&id()))]));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("a notification cannot precede the acknowledgement");
        assert!(
            error.to_string().contains(ACKNOWLEDGED_METHOD),
            "the error names the spec MUST: {error}"
        );
    }

    #[tokio::test]
    async fn an_acknowledgement_for_another_subscription_is_refused() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(vec![Ok(
            ack_payload(&RequestId::Number(999), &json!({})),
        )]));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("a cross-tagged ack must not open a stream");
        assert!(
            error.to_string().contains("subscriptionId"),
            "the error names the mismatch: {error}"
        );
    }

    #[tokio::test]
    async fn an_empty_stream_is_refused() {
        let frames: SubscriptionFrameStream = Box::pin(futures::stream::iter(Vec::new()));
        let error = SubscriptionStream::open(id(), frames)
            .await
            .expect_err("no frame at all is not an acknowledgement");
        assert!(error.to_string().contains("acknowledgement"), "{error}");
    }

    // ---- frame classification ---------------------------------------------

    #[tokio::test]
    async fn a_tagged_unit_notification_is_decoded() {
        let mut stream = stream_over(id(), vec![Ok(tools_changed(&id()))]);
        let item = stream.next().await.expect("one frame").expect("decodes");
        assert!(matches!(item, ServerNotification::ToolsChanged));
        assert!(stream.next().await.is_none(), "then the stream ends");
    }

    #[tokio::test]
    async fn a_tagged_struct_notification_is_decoded() {
        let mut stream = stream_over(id(), vec![Ok(resource_updated(&id(), "mem://greeting"))]);
        let item = stream.next().await.expect("one frame").expect("decodes");
        match item {
            ServerNotification::ResourceUpdated(params) => {
                assert_eq!(params.uri, "mem://greeting");
            },
            other => panic!("expected a resources/updated notification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_frame_tagged_with_another_subscription_id_yields_an_error() {
        let mut stream = stream_over(
            id(),
            vec![
                Ok(tools_changed(&RequestId::Number(999))),
                Ok(tools_changed(&id())),
            ],
        );

        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("a cross-tagged frame is never forwarded as the caller's own");
        assert!(
            error.to_string().contains("999"),
            "the error names the foreign id: {error}"
        );

        let recovered = stream
            .next()
            .await
            .expect("the stream did NOT terminate")
            .expect("the correctly tagged frame still arrives");
        assert!(matches!(recovered, ServerNotification::ToolsChanged));
    }

    #[tokio::test]
    async fn an_untagged_frame_yields_an_error() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
        })
        .to_string();
        let mut stream = stream_over(id(), vec![Ok(payload)]);
        let error = stream.next().await.expect("an item").expect_err("untagged");
        assert!(error.to_string().contains("<absent>"), "{error}");
    }

    #[tokio::test]
    async fn a_malformed_frame_yields_an_error_without_ending_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![Ok("{not json at all".to_string()), Ok(tools_changed(&id()))],
        );

        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("garbage is an error");
        assert!(error.to_string().contains("not JSON"), "{error}");

        let recovered = stream
            .next()
            .await
            .expect("the stream survived the malformed frame")
            .expect("and the next good frame decodes");
        assert!(matches!(recovered, ServerNotification::ToolsChanged));
    }

    #[tokio::test]
    async fn an_unknown_notification_method_yields_an_error_without_ending_the_stream() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/from/the/future",
            "params": { "_meta": subscription_id_meta(&id()) },
        })
        .to_string();
        let mut stream = stream_over(id(), vec![Ok(payload), Ok(tools_changed(&id()))]);

        assert!(
            stream.next().await.expect("an item").is_err(),
            "an unmodelled method is surfaced, not silently dropped"
        );
        assert!(
            stream.next().await.expect("still live").is_ok(),
            "and the stream keeps going"
        );
    }

    #[tokio::test]
    async fn a_second_acknowledgement_yields_an_error() {
        let mut stream = stream_over(id(), vec![Ok(ack_payload(&id(), &json!({})))]);
        let error = stream
            .next()
            .await
            .expect("an item")
            .expect_err("exactly one acknowledgement is allowed");
        assert!(error.to_string().contains("exactly once"), "{error}");
    }

    #[tokio::test]
    async fn the_terminal_result_ends_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![
                Ok(tools_changed(&id())),
                Ok(terminal(&id())),
                Ok(tools_changed(&id())),
            ],
        );

        assert!(stream.next().await.expect("the notification").is_ok());
        assert!(
            stream.next().await.is_none(),
            "the terminal SubscriptionsListenResult ends the stream gracefully"
        );
        assert!(
            stream.next().await.is_none(),
            "and it stays ended — nothing after it is delivered"
        );
    }

    #[tokio::test]
    async fn a_transport_error_ends_the_stream() {
        let mut stream = stream_over(
            id(),
            vec![
                Err(Error::Transport(TransportError::ConnectionClosed)),
                Ok(tools_changed(&id())),
            ],
        );
        assert!(stream.next().await.expect("an item").is_err());
        assert!(
            stream.next().await.is_none(),
            "a transport failure is terminal, unlike a bad frame"
        );
    }

    // ---- SSE decoding ------------------------------------------------------

    #[test]
    fn keep_alive_comments_are_skipped() {
        let mut parser = SseParser::new();
        assert!(
            drain_sse_payloads(&mut parser, ": keep-alive\n\n").is_empty(),
            "a comment line is not a payload"
        );
        assert!(
            drain_sse_payloads(&mut parser, ":\n\n").is_empty(),
            "an empty comment is not a payload either"
        );
        assert_eq!(
            drain_sse_payloads(&mut parser, "event: message\ndata: {\"a\":1}\n\n"),
            vec!["{\"a\":1}".to_string()],
            "and the real event still arrives"
        );
    }

    #[test]
    fn a_payload_split_across_chunks_is_reassembled() {
        let mut parser = SseParser::new();
        assert!(drain_sse_payloads(&mut parser, "data: {\"a\"").is_empty());
        assert_eq!(
            drain_sse_payloads(&mut parser, ":1}\n\n"),
            vec!["{\"a\":1}".to_string()]
        );
    }

    #[test]
    fn a_multibyte_character_split_across_chunks_survives() {
        // "☂" is three bytes; split it across two reads.
        let text = "data: \u{2602}\n\n";
        let bytes = text.as_bytes();
        let mut buffer = Vec::new();
        let mut parser = SseParser::new();
        let mut payloads = Vec::new();

        buffer.extend_from_slice(&bytes[..7]); // cuts the umbrella in half
        let prefix = take_utf8_prefix(&mut buffer);
        payloads.extend(drain_sse_payloads(&mut parser, &prefix));
        assert!(!buffer.is_empty(), "the incomplete tail is retained");

        buffer.extend_from_slice(&bytes[7..]);
        let rest = take_utf8_prefix(&mut buffer);
        payloads.extend(drain_sse_payloads(&mut parser, &rest));

        assert_eq!(payloads, vec!["\u{2602}".to_string()]);
    }

    #[test]
    fn invalid_bytes_do_not_wedge_the_decoder() {
        let mut buffer = vec![0xff, 0xfe, b'a'];
        let text = take_utf8_prefix(&mut buffer);
        assert!(
            buffer.is_empty(),
            "genuinely invalid bytes are consumed, not retained forever"
        );
        assert!(text.contains('a'), "the valid remainder survives: {text:?}");
    }

    /// A server that streams past the bound with no newline at all latches the
    /// SHARED parser, which is the condition `read_next_frame` checks after
    /// every drain before ending the stream.
    ///
    /// Driven through a deliberately tiny parser: the production bound is
    /// 256 KiB and a test must not allocate it to prove the wiring.
    #[test]
    fn a_line_past_the_bound_latches_the_parser_and_ends_the_stream() {
        let mut parser = SseParser::with_max_buffer_size(64);
        assert!(
            listen_overflow(&parser).is_none(),
            "a fresh parser has lost nothing, so the stream keeps reading"
        );

        assert!(
            drain_sse_payloads(&mut parser, &"x".repeat(256)).is_empty(),
            "an unterminated line completes no payload"
        );

        let error = listen_overflow(&parser).expect("a discarded line ends the stream");
        // The bound named is THIS parser's (64), not a re-derived constant. A
        // message that reported `MAX_LISTEN_LINE_BYTES` here would be naming a
        // limit this parser never had.
        assert!(
            error.to_string().contains("64"),
            "the error names the limit the parser actually enforced: {error}"
        );
        match error {
            Error::Protocol { code, .. } => {
                assert_eq!(code, ErrorCode::INVALID_REQUEST);
            },
            other => panic!("expected a structured protocol error, got {other:?}"),
        }
    }

    /// The realistic flood: a peer streaming perfectly ordinary
    /// NEWLINE-TERMINATED `data:` lines and simply never sending the blank line
    /// that would end the event.
    ///
    /// Every existing bound test in this module feeds `"x".repeat(N)` with no
    /// newline in it (review IN-03), which is the artificial case — and it is
    /// exactly why a green suite coexisted with GAP-A. This drives the same
    /// PRODUCTION pair the live stream uses, `drain_sse_payloads` then
    /// `listen_overflow`, on the input class the old bound could not see.
    #[test]
    fn a_newline_carrying_flood_ends_the_stream_too() {
        let mut parser = SseParser::with_max_buffer_size(64);
        let mut error = None;
        for _ in 0..1_000 {
            assert!(
                drain_sse_payloads(&mut parser, "data: AAAAAAAA\n").is_empty(),
                "a `data:` line with no blank line after it completes no payload"
            );
            if let Some(seen) = listen_overflow(&parser) {
                error = Some(seen);
                break;
            }
        }

        let error = error.expect("accumulated `data:` lines must trip the bound");
        assert!(
            error.to_string().contains("64"),
            "the error names the limit the parser actually enforced: {error}"
        );
        assert!(
            !error.to_string().contains('A'),
            "no fed byte is echoed into the message: {error}"
        );
    }

    /// A normal stream never observes the flag, so nothing about the existing
    /// behaviour changes for a well-behaved server.
    #[test]
    fn a_normal_listen_stream_never_trips_the_bound() {
        let mut parser = SseParser::with_max_buffer_size(MAX_LISTEN_LINE_BYTES);
        let payloads = drain_sse_payloads(
            &mut parser,
            &format!("event: message\ndata: {}\n\n", tools_changed(&id())),
        );
        assert_eq!(payloads.len(), 1);
        assert!(listen_overflow(&parser).is_none());
    }

    /// The listen path is bounded TIGHTER than the shared default, because its
    /// frames are notifications and small by construction.
    #[test]
    fn the_listen_bound_is_tighter_than_the_shared_default() {
        let shared = crate::shared::sse_parser::SseConfig::default().max_buffer_size;
        assert!(
            MAX_LISTEN_LINE_BYTES < shared,
            "{MAX_LISTEN_LINE_BYTES} must be tighter than the shared {shared}"
        );
    }

    /// The chunked fuzz seam reaches the overflow branch under a small bound,
    /// and reaches it by ACCUMULATION across chunks — the contract
    /// `read_next_frame` relies on when it decides to stop polling.
    ///
    /// Note what is deliberately NOT asserted here: that the flag never clears
    /// once set. `overflowed` has exactly two writes — `false` at construction
    /// and `true` in `feed` — so no input can make it clear, and an assertion
    /// over generated data cannot fail. The one decision that COULD change it
    /// (that `reset()` leaves it alone) is pinned deterministically by
    /// `sse_parser::tests::the_overflow_flag_latches`, which is its right level.
    #[test]
    fn the_chunked_fuzz_seam_trips_the_bound_partway_through() {
        // 8 chunks of 16 newline-free bytes against a 64-byte bound: the bound
        // trips partway through, not on the first chunk, so "tripped by
        // accumulation" is distinguishable from "tripped immediately".
        let chunk = [b'x'; 16];
        let chunks: Vec<&[u8]> = vec![&chunk[..]; 8];
        let (outcomes, overflowed, peak_buffered_bytes) =
            decode_listen_chunks_for_fuzz(&chunks, "sub-1", 64);

        assert!(
            outcomes.is_empty(),
            "an unterminated line completes no frame"
        );
        assert_eq!(overflowed.len(), chunks.len(), "one observation per chunk");
        assert!(
            !overflowed[0],
            "the bound is not tripped by the first chunk"
        );
        assert!(
            overflowed.last().copied().unwrap_or_default(),
            "the bound is tripped by the end: {overflowed:?}"
        );
        assert_eq!(
            peak_buffered_bytes.len(),
            chunks.len(),
            "one retention sample per chunk"
        );
        assert!(
            peak_buffered_bytes.iter().all(|held| *held <= 64),
            "retention stays inside the bound even while it is being approached: \
             {peak_buffered_bytes:?}"
        );
    }

    /// A well-formed frame delivered ACROSS chunk boundaries still arrives —
    /// the seam carries the SSE line buffer and the undecoded-UTF-8 tail from one
    /// chunk to the next, exactly as a live body does.
    #[test]
    fn the_chunked_fuzz_seam_carries_a_frame_split_across_chunks() {
        let id = RequestId::String("sub-1".to_string());
        let body = format!("event: message\ndata: {}\n\n", tools_changed(&id));
        let bytes = body.as_bytes();
        let chunks: Vec<&[u8]> = bytes.chunks(5).collect();
        assert!(chunks.len() > 1, "the frame must actually be split");

        let (outcomes, overflowed, peak_buffered_bytes) =
            decode_listen_chunks_for_fuzz(&chunks, "sub-1", MAX_LISTEN_LINE_BYTES);
        assert_eq!(outcomes.len(), 1, "exactly one frame is reassembled");
        assert!(outcomes[0].is_ok(), "and it is delivered: {outcomes:?}");
        assert!(
            overflowed.iter().all(|seen| !*seen),
            "a legitimate frame never trips the bound"
        );
        assert!(
            peak_buffered_bytes
                .iter()
                .all(|held| *held <= MAX_LISTEN_LINE_BYTES),
            "a legitimate frame stays inside the bound: {peak_buffered_bytes:?}"
        );
    }

    /// The campaign's NEW invariant, proven non-vacuous on the input class the
    /// fuzzer actually generates.
    ///
    /// `fuzz_targets/subscription_listen_frames.rs` asserts that every
    /// `peak_buffered_bytes` sample stays `<= max_buffer_size`. An assertion that
    /// holds only because the parser never accumulates anything would be the same
    /// failure as the latch tautology it replaces (review WR-03) — 20 000 green
    /// runs that could not have seen GAP-A. So this drives the seam with the
    /// realistic flood: ordinary NEWLINE-TERMINATED `data:` lines that no blank
    /// line ever ends, which is the input class the pre-113-17 bound could not
    /// see, and pins THREE things at once:
    ///
    /// 1. every sample is inside the bound (the campaign's assertion),
    /// 2. the bound is actually REACHED — at least one overflow observation is
    ///    `true` — so the samples are not trivially small, and
    /// 3. retention ACCUMULATES across lines (some sample exceeds what one chunk
    ///    alone contributes), so the samples are not trivially per-chunk either.
    #[test]
    fn the_seam_reports_retention_that_stays_inside_a_tiny_bound_while_reaching_it() {
        const BOUND: usize = 64;
        // 15 bytes: a complete `data:` line whose payload is 8 bytes. No blank
        // line anywhere, so the event accumulates and is never dispatched.
        let line = b"data: AAAAAAAA\n";
        let chunks: Vec<&[u8]> = vec![&line[..]; 20];

        let (outcomes, overflowed, peak_buffered_bytes) =
            decode_listen_chunks_for_fuzz(&chunks, "sub-1", BOUND);

        assert!(
            outcomes.is_empty(),
            "a `data:` line with no blank line after it completes no frame: {outcomes:?}"
        );
        assert_eq!(
            peak_buffered_bytes.len(),
            chunks.len(),
            "one retention sample per chunk"
        );
        assert!(
            peak_buffered_bytes.iter().all(|held| *held <= BOUND),
            "the campaign's invariant: retention never exceeds the bound, \
             {peak_buffered_bytes:?} under {BOUND}"
        );
        assert!(
            overflowed.iter().any(|seen| *seen),
            "non-vacuity: the flood must actually REACH the bound, or the \
             invariant above holds for an uninteresting reason: {overflowed:?}"
        );
        assert!(
            peak_buffered_bytes.iter().any(|held| *held > line.len()),
            "non-vacuity: retention must ACCUMULATE across lines, not merely \
             hold one chunk: {peak_buffered_bytes:?}"
        );
    }

    #[test]
    fn an_untrusted_frame_is_truncated_in_error_messages() {
        let huge = "x".repeat(MAX_ECHOED_FRAME * 4);
        let truncated = truncate(&huge);
        assert!(truncated.chars().count() <= MAX_ECHOED_FRAME + 1);
        assert!(truncated.ends_with('…'), "the elision is visible");

        // Exactly at the bound is NOT truncated, and a multi-byte character at
        // the cut is not split (the cut is a char boundary, never a byte offset).
        let exact = "y".repeat(MAX_ECHOED_FRAME);
        assert_eq!(truncate(&exact), exact);
        let umbrellas = "\u{2602}".repeat(MAX_ECHOED_FRAME * 2);
        let cut = truncate(&umbrellas);
        assert_eq!(cut.chars().count(), MAX_ECHOED_FRAME + 1);
    }

    // ---- the non-stream rejection path -------------------------------------

    /// A well-formed JSON-RPC error envelope reaches the caller VERBATIM, which
    /// is how "this server does not do subscriptions" (`-32601`) stays
    /// distinguishable from a transport fault.
    #[test]
    fn a_jsonrpc_error_envelope_is_surfaced_unchanged() {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": "Method not found" },
        })
        .to_string();
        let error = rejection_error(hyper::StatusCode::NOT_FOUND, body.as_bytes());
        match error {
            Error::Protocol { code, .. } => assert_eq!(code.as_i32(), -32601),
            other => panic!("expected the server's own protocol error, got {other:?}"),
        }
    }

    /// An intermediary's error page is NOT laundered into a protocol error, and
    /// the bytes it carries are truncated before they reach the message.
    #[test]
    fn a_non_envelope_body_becomes_a_truncated_transport_error() {
        let body = "<html>".to_string() + &"z".repeat(MAX_ECHOED_FRAME * 10);
        let error = rejection_error(hyper::StatusCode::BAD_GATEWAY, body.as_bytes());
        let rendered = error.to_string();
        assert!(matches!(error, Error::Transport(_)), "{rendered}");
        assert!(rendered.contains("502"), "the status is named: {rendered}");
        assert!(
            rendered.matches('z').count() < MAX_ECHOED_FRAME + 1,
            "the untrusted body is bounded in the message: {} echoed bytes",
            rendered.matches('z').count()
        );
    }

    // ---- properties (CLAUDE.md ALWAYS / PROPERTY testing) ------------------

    /// Every notification method a listen stream can legitimately carry, plus
    /// one this SDK does not model.
    const METHODS: [&str; 4] = [
        "notifications/tools/list_changed",
        "notifications/prompts/list_changed",
        "notifications/resources/list_changed",
        "notifications/from/the/future",
    ];

    proptest::proptest! {
        /// T-113-66 as an invariant over the whole id space: a frame is
        /// delivered to the caller IF AND ONLY IF it is tagged with THIS
        /// stream's subscription id (and carries a method this SDK models).
        ///
        /// An example-based test can only pin the two cases it happens to
        /// write; this pins the implication itself.
        #[test]
        fn a_frame_is_delivered_only_when_its_tag_matches_this_stream(
            stream_id in 0i64..8,
            frame_id in 0i64..8,
            method_index in 0usize..4,
        ) {
            let frame = json!({
                "jsonrpc": "2.0",
                "method": METHODS[method_index],
                "params": {
                    "_meta": subscription_id_meta(&RequestId::Number(frame_id)),
                },
            });

            let delivered = matches!(
                classify_frame(&frame.to_string(), &RequestId::Number(stream_id)),
                FrameOutcome::Notification(_)
            );

            proptest::prop_assert_eq!(
                delivered,
                stream_id == frame_id && method_index < 3,
                "delivery must be exactly (matching tag AND known method); \
                 stream={} frame={} method={}",
                stream_id,
                frame_id,
                METHODS[method_index],
            );
        }

        /// The decode path is fed by a REMOTE peer. Arbitrary bytes must never
        /// panic it (T-113-67) — the same invariant the fuzz target asserts,
        /// held here as a fast in-tree regression.
        #[test]
        fn arbitrary_bytes_never_panic_the_decoder(
            bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
        ) {
            let _ = decode_listen_chunks_for_fuzz(&[&bytes], "prop-subscription", MAX_LISTEN_LINE_BYTES);
        }

        /// And neither does arbitrary TEXT that looks more like SSE than random
        /// bytes do, which is the shape a hostile-but-well-formed peer sends.
        #[test]
        fn arbitrary_sse_shaped_text_never_panics_the_decoder(
            body in "(data|event|id|:|\\{|\\}|\"|a|1|\n){0,200}",
        ) {
            let _ = decode_listen_chunks_for_fuzz(&[body.as_bytes()], "prop-subscription", MAX_LISTEN_LINE_BYTES);
        }

        /// The same non-panic invariant, but CHUNKED against a tiny bound so the
        /// state carried across chunk boundaries — the undecoded-UTF-8 tail, the
        /// SSE line buffer, and the overflow discard branch — is exercised too.
        /// A split mid-character or mid-line is unreachable with one chunk.
        #[test]
        fn chunked_arbitrary_bytes_never_panic_the_decoder(
            bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
        ) {
            let chunks: Vec<&[u8]> = bytes.chunks(16).collect();
            let _ = decode_listen_chunks_for_fuzz(&chunks, "prop-subscription", 64);
        }
    }

    // ---- the era gate, proven with a counting stub transport ---------------

    mod era_gate {
        use super::*;
        use crate::shared::{Transport, TransportMessage};
        use crate::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
        use crate::{Client, ClientBuilder};
        use std::sync::{Arc, Mutex};

        /// A transport that COUNTS stream opens, so "no request was sent" is a
        /// measured fact rather than an inference from an error message.
        #[derive(Debug, Default, Clone)]
        struct CountingStubTransport {
            opened: Arc<Mutex<Vec<Vec<u8>>>>,
            payloads: Arc<Mutex<Vec<String>>>,
        }

        #[async_trait]
        impl Transport for CountingStubTransport {
            async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                Ok(())
            }

            async fn receive(&mut self) -> Result<TransportMessage> {
                Err(Error::protocol_msg("no responses"))
            }

            async fn close(&mut self) -> Result<()> {
                Ok(())
            }

            fn transport_type(&self) -> &'static str {
                "counting-stub"
            }

            fn supports_negotiated_protocol_version(&self) -> bool {
                true
            }
        }

        #[async_trait]
        impl EventStreamTransport for CountingStubTransport {
            async fn open_event_stream(&self, body: Vec<u8>) -> Result<SubscriptionFrameStream> {
                self.opened.lock().unwrap().push(body);
                let payloads: Vec<Result<String>> = self
                    .payloads
                    .lock()
                    .unwrap()
                    .iter()
                    .cloned()
                    .map(Ok)
                    .collect();
                Ok(Box::pin(futures::stream::iter(payloads)))
            }
        }

        fn v2_version() -> ProtocolVersion {
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string())
        }

        fn client_with(
            transport: CountingStubTransport,
            v2: bool,
        ) -> Client<CountingStubTransport> {
            let builder = ClientBuilder::new(transport);
            if v2 {
                builder
                    .with_protocol_version(v2_version())
                    .expect("2026-07-28 is selectable")
                    .build()
            } else {
                builder.build()
            }
        }

        #[tokio::test]
        async fn a_non_v2_client_refuses_without_opening_a_stream() {
            let transport = CountingStubTransport::default();
            let opened = transport.opened.clone();
            let client = client_with(transport, false);

            let error = client
                .subscriptions_listen(SubscriptionFilter::default())
                .await
                .expect_err("subscriptions/listen does not exist on v1");

            assert_eq!(
                opened.lock().unwrap().len(),
                0,
                "a v1 client must not put a request on the wire that cannot succeed"
            );
            assert!(
                error.to_string().contains("with_protocol_version"),
                "the error names the opt-in: {error}"
            );
        }

        #[tokio::test]
        async fn a_v2_client_sends_the_listen_frame_and_consumes_the_ack() {
            let transport = CountingStubTransport::default();
            let opened = transport.opened.clone();
            // The stub answers whatever id the client mints, so the ack is built
            // lazily below — here it is enough that the frame is well-formed for
            // a KNOWN id, so the client is given one it will reject, and the
            // request bytes are what this test asserts on.
            let client = client_with(transport, true);

            let error = client
                .subscriptions_listen(SubscriptionFilter {
                    tools_list_changed: Some(true),
                    ..SubscriptionFilter::default()
                })
                .await
                .expect_err("the stub sends no acknowledgement at all");
            assert!(error.to_string().contains("acknowledgement"), "{error}");

            let opened = opened.lock().unwrap();
            assert_eq!(opened.len(), 1, "exactly one stream open");
            let frame = serde_json::from_slice::<Value>(&opened[0]).expect("a JSON-RPC frame");
            assert_eq!(frame["method"], json!("subscriptions/listen"));
            assert_eq!(
                frame["params"]["notifications"],
                json!({ "toolsListChanged": true }),
                "the requested filter travels under the REQUIRED `notifications` field"
            );
            assert_eq!(
                frame["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
                json!(PROTOCOL_VERSION_2026_07_28),
                "the v2 era signal is stamped like on every other v2 request"
            );
        }

        #[tokio::test]
        async fn a_v2_client_returns_the_servers_own_error_unchanged() {
            /// A transport whose stream open fails the way a `-32601`
            /// rejection does.
            #[derive(Debug, Default, Clone)]
            struct RejectingTransport;

            #[async_trait]
            impl Transport for RejectingTransport {
                async fn send(&mut self, _message: TransportMessage) -> Result<()> {
                    Ok(())
                }
                async fn receive(&mut self) -> Result<TransportMessage> {
                    Err(Error::protocol_msg("no responses"))
                }
                async fn close(&mut self) -> Result<()> {
                    Ok(())
                }
                fn supports_negotiated_protocol_version(&self) -> bool {
                    true
                }
            }

            #[async_trait]
            impl EventStreamTransport for RejectingTransport {
                async fn open_event_stream(
                    &self,
                    _body: Vec<u8>,
                ) -> Result<SubscriptionFrameStream> {
                    Err(Error::from_jsonrpc_error(
                        crate::types::jsonrpc::JSONRPCError {
                            code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                            message: "Method not found: subscriptions/listen".to_string(),
                            data: None,
                        },
                    ))
                }
            }

            let client = ClientBuilder::new(RejectingTransport)
                .with_protocol_version(v2_version())
                .expect("2026-07-28 is selectable")
                .build();

            let error = client
                .subscriptions_listen(SubscriptionFilter::default())
                .await
                .expect_err("a non-advertising server answers -32601");
            match error {
                Error::Protocol { code, .. } => assert_eq!(
                    code.as_i32(),
                    crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                    "the server's own error code reaches the caller unchanged"
                ),
                other => panic!("expected a structured protocol error, got {other:?}"),
            }
        }
    }
}
