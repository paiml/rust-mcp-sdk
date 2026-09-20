//! The `StreamableHTTP` server-to-client channel (Phase 118.1 plan 10, CONF-07 / G-3).
//!
//! # The property this module exists to preserve
//!
//! `Server::run`'s wiring carries this rationale verbatim
//! (`src/server/mod.rs`), and it is quoted rather than paraphrased because a
//! reader who does not see it will re-introduce the bug:
//!
//! > Transport Actor design (Phase 108, D-01/D-02): the transport is OWNED
//! > by exactly one actor task and is NEVER wrapped in a shared
//! > `Arc<RwLock<T>>`. ALL outbound frames (responses, server-requests,
//! > notifications) funnel through a single UNBOUNDED `send_tx`; inbound
//! > requests go to a SINGLE sequential worker via an UNBOUNDED
//! > `request_tx`. The receive/drain path therefore never blocks on request
//! > execution, request-queue capacity, or a transport write-lock, so an
//! > in-tool `peer.sample()` / `.list_roots()` round-trip cannot deadlock
//! > the loop. Request handling stays serialized (one worker) — zero
//! > behavior change for existing single-request servers.
//!
//! This module reproduces that property over HTTP, where the shape of the hazard
//! is different but the hazard is the same. `dispatch_public_request` holds
//! `state.server.lock().await` for the ENTIRE duration of a tool handler, so a
//! handler awaiting `peer.sample()` holds the server mutex while it waits for
//! the client's answer — and that answer arrives as a POST whose gate
//! (`run_v2_header_gate`, twice) and auth (`extract_and_validate_auth`) stages
//! all take the same mutex. Left alone that is a guaranteed deadlock, and it is
//! also a denial-of-service control: ONE tool call would otherwise take the
//! whole transport offline.
//!
//! Two structural answers, both here:
//!
//! 1. The [`ServerRequestDispatcher`] lives on `ServerState`, OUTSIDE
//!    `Mutex<Server>`. A dispatcher behind that mutex is the same deadlock in a
//!    different costume.
//! 2. An inbound JSON-RPC RESPONSE is classified and routed BEFORE any stage
//!    that takes the mutex (plan 10 Task 3, on both POST entrypoints).
//!
//! # Correlation ownership
//!
//! `Server::run` serves exactly one client, so a server-to-client request has
//! only one place it can go. This transport multiplexes many sessions, so it
//! dispatches through [`ServerRequestDispatcher::dispatch_owned`], which records
//! WHICH session minted each correlation id. The outbound drain resolves that
//! owner to pick the stream, and the inbound path requires it to equal the
//! session presented on the response POST — otherwise a client could resolve
//! another client's pending `sampling/createMessage` (T-118.1-10-02).
//!
//! # Module shape
//!
//! The same three rules `v1_session.rs` states for the v1 pair, for the same
//! reasons: every entry point returns an OWNED answer or performs a whole
//! operation and never hands out a lock guard or an `&Arc<RwLock<..>>`; state
//! lives in a dedicated struct with private fields; any lint allow carries a
//! `// Why:` comment. It is NOT part of the v1/v2 pair — inbound response
//! correlation is era-agnostic — but the OUTBOUND half necessarily routes
//! through `v1::route_to_session_stream`, whose zero-sized twin always answers
//! "no stream", so on a `full-v2` build every outbound dispatch is refused
//! immediately and correctly rather than being silently dropped.
//!
//! # Why this is a submodule and not more of `streamable_http_server.rs`
//!
//! That file is already ~6,700 lines. G-3 does not add to it.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution already used by `v1_session.rs`, `src/server/task_dispatch.rs` and
// `src/shared/http_body_cap.rs`.
#![allow(clippy::redundant_pub_crate)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tracing::debug;

use super::{v1, HttpIngress, ServerState};
use crate::error::{Error, ErrorCode, Result};
use crate::server::roots::ListRootsResult;
use crate::server::server_request_dispatcher::ServerRequestDispatcher;
use crate::shared::peer::PeerHandle;
use crate::shared::TransportMessage;
use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use crate::types::protocol::context::TransportBackchannel;
use crate::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools,
};
use crate::types::{JSONRPCResponse, Notification, ProgressToken, ServerRequest};
use serde::Deserialize;

/// Capacity of the outbound server-to-client request channel.
///
/// Mirrors `Server::run`'s `mpsc::channel(100)` so the two paths queue alike.
const OUTBOUND_CAPACITY: usize = 100;

/// How long a server-to-client RPC may stay pending on THIS transport.
///
/// EXPLICIT, and deliberately shorter than the 60-second in-process
/// [`DEFAULT_DISPATCH_TIMEOUT`](crate::server::server_request_dispatcher::DEFAULT_DISPATCH_TIMEOUT),
/// because the cost of a pending entry is higher here. Without a bound, a client
/// that opens an SSE stream, triggers N tools that each park on a peer call, and
/// then never answers grows the pending map without limit (T-118.1-10-05).
///
/// And the pending map is not the only thing it pins. `dispatch_public_request`
/// holds `state.server.lock().await` across the whole handler, so every parked
/// peer call also holds the server mutex. A timed-out peer call RELEASES it: the
/// dispatch returns `REQUEST_TIMEOUT`, the handler returns, its stack frame
/// unwinds and `dispatch_public_request`'s guard drops. That is why this value is
/// the transport's real upper bound on how long one absent client can serialize
/// the server — and why it is stated here rather than inherited silently.
///
/// 30 seconds is the compromise: long enough for a real host to run an LLM
/// completion or put an elicitation form in front of a person, short enough that
/// an abandoned round trip is not an outage.
const HTTP_DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);

/// The outbound channel's receiving half, parked until the drain claims it.
///
/// A named alias because the nested generic trips `clippy::type_complexity`
/// inline — and because the shape is load-bearing enough to deserve a name: it is
/// the `(correlation_id, ServerRequest)` pair `Server::run` also queues, held in
/// a `parking_lot::Mutex<Option<..>>` so [`ensure_outbound_drain`] can take it
/// exactly once.
type ParkedOutboundReceiver = Arc<Mutex<Option<mpsc::Receiver<(String, ServerRequest)>>>>;

/// A one-way notification sink for ONE client session.
///
/// Named because the shape is load-bearing rather than incidental: it is EXACTLY
/// [`ServerProgressReporter::new`](crate::server::progress::ServerProgressReporter::new)'s
/// second parameter, so the transport's sink is handed to the request-scoped
/// progress reporter with no adapter at all. The type is what the two eras share
/// (plan 12 supplies v2's own closure of the same type); the closure is not.
type NotificationSink = Arc<dyn Fn(Notification) + Send + Sync>;

/// Logged when an outbound dispatch reaches the drain with no recorded owner.
const NO_OWNER: &str = "outbound dispatch carried no recorded session owner";

/// Logged when the owning session has no live SSE stream to deliver on.
const NO_LIVE_STREAM: &str = "the owning session has no live SSE stream";

// ---------------------------------------------------------------------------
// State.
// ---------------------------------------------------------------------------

/// The transport's server-to-client channel state.
///
/// Lives on `ServerState`, NOT inside `Mutex<Server>` — see the module doc.
/// Both fields are private: every read goes through an operation below, so no
/// caller can take the dispatcher's locks in an order this module did not choose.
#[derive(Clone)]
pub(crate) struct PeerChannel {
    /// THE correlation authority for this transport. One per server, shared by
    /// every session's [`SessionPeerHandle`].
    dispatcher: Arc<ServerRequestDispatcher>,
    /// The receiving half of the outbound channel, until the drain takes it.
    ///
    /// Held here rather than spawned at construction because `ServerState` is
    /// built by a SYNCHRONOUS function (`make_server_state`, also reached from
    /// `pmcp::axum::router()`), and `tokio::spawn` outside a runtime panics.
    /// [`ensure_outbound_drain`] takes it exactly once, from whichever call site
    /// first runs inside a runtime.
    outbound_rx: ParkedOutboundReceiver,
}

/// Hand-written: `ServerRequestDispatcher`'s own `Debug` prints cardinality only,
/// and this must not widen that. It also takes NO lock, for the same reason
/// `V1State`'s does not — a `Debug` impl that blocked inside a panic formatter
/// would turn a diagnostic into a hang.
impl std::fmt::Debug for PeerChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerChannel").finish_non_exhaustive()
    }
}

impl Default for PeerChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerChannel {
    /// Build the channel and its dispatcher ONCE, at `ServerState` construction.
    ///
    /// Mirrors `Server::run`'s wiring — an `mpsc::channel::<(String,
    /// ServerRequest)>` plus a `ServerRequestDispatcher::new_with_channel(tx)` —
    /// with the transport's own explicit [`HTTP_DISPATCH_TIMEOUT`].
    pub(crate) fn new() -> Self {
        let (outbound_tx, outbound_rx) =
            mpsc::channel::<(String, ServerRequest)>(OUTBOUND_CAPACITY);
        Self {
            dispatcher: Arc::new(
                ServerRequestDispatcher::new_with_channel(outbound_tx)
                    .with_timeout(HTTP_DISPATCH_TIMEOUT),
            ),
            outbound_rx: Arc::new(Mutex::new(Some(outbound_rx))),
        }
    }
}

/// The transport's correlation authority, as an owned handle.
///
/// An OPERATION, not a borrow of the field: the caller gets an `Arc` it owns and
/// this module keeps the only reference to the struct itself.
pub(crate) fn dispatcher(state: &ServerState) -> Arc<ServerRequestDispatcher> {
    Arc::clone(&state.peer_channel.dispatcher)
}

/// Start the outbound drain, if it is not running already.
///
/// Idempotent: the receiver can only be taken once, so a second call is a no-op.
/// Called from `make_server_state` (covering `pmcp::axum::router()` users) and
/// again from `StreamableHttpServer::start()`, because the first of those is
/// synchronous and may run outside a Tokio runtime — in which case it declines
/// and leaves the receiver for the second.
pub(crate) fn ensure_outbound_drain(state: &ServerState) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        // No runtime yet. `start()` will call this again from inside one.
        return;
    };
    let Some(outbound_rx) = state.peer_channel.outbound_rx.lock().take() else {
        return; // Already draining.
    };
    // WEAK, and this is load-bearing rather than tidy. The dispatcher OWNS
    // `outbound_tx`, the sending half of the very channel this task drains. A
    // STRONG clone (or a clone of the whole `ServerState`, which reaches the
    // dispatcher transitively) would keep that sender alive for as long as the
    // task lives and the task alive for as long as the sender lives, so
    // `recv()` could never answer `None`, the task would never exit, and it
    // would pin the whole `ServerState` — `Arc<Mutex<Server>>`, the session map
    // and the event store included — for the life of the PROCESS. Every
    // `pmcp::axum::router()` / `StreamableHttpServer::start()` would leak one.
    //
    // `V1State` is cloned by value because it holds no sender for this channel:
    // it is the SSE-stream map, the session table and the event store, i.e.
    // exactly what `route_to_session_stream` needs and nothing more.
    let v1_state = state.v1.clone();
    let dispatcher = Arc::downgrade(&state.peer_channel.dispatcher);
    handle.spawn(async move { drain_outbound(v1_state, dispatcher, outbound_rx).await });
}

/// Forward each outbound server-to-client request onto its ORIGINATING session's
/// live SSE stream.
///
/// Exits cleanly when the channel closes — which happens when the last
/// `ServerRequestDispatcher` clone (and with it `outbound_tx`) is dropped, i.e.
/// when the server state goes away. The `Weak` upgrade is the second exit: a
/// message still in flight when the dispatcher dies has nothing left to
/// correlate against.
async fn drain_outbound(
    v1_state: v1::V1State,
    dispatcher: std::sync::Weak<ServerRequestDispatcher>,
    mut outbound_rx: mpsc::Receiver<(String, ServerRequest)>,
) {
    while let Some((correlation_id, server_request)) = outbound_rx.recv().await {
        let Some(dispatcher) = dispatcher.upgrade() else {
            break;
        };
        route_outbound(&v1_state, &dispatcher, correlation_id, server_request).await;
    }
    debug!("StreamableHTTP server-request drain exited");
}

/// Route ONE outbound server-to-client request, or fail its correlation.
///
/// The owner lookup is the step that makes the `(correlation_id, ServerRequest)`
/// channel type sufficient: the session is not carried in the tuple, it is
/// RECORDED against the correlation id at dispatch time and resolved here.
async fn route_outbound(
    v1_state: &v1::V1State,
    dispatcher: &ServerRequestDispatcher,
    correlation_id: String,
    server_request: ServerRequest,
) {
    // An HTTP-owned dispatcher must never see an ownerless dispatch: everything
    // that reaches this channel came through `dispatch_owned`.
    let Some(session_id) = dispatcher.owner_of(&correlation_id).await else {
        dispatcher.fail_pending(&correlation_id, NO_OWNER).await;
        return;
    };

    // Framed exactly as `spawn_server_request_drain` frames it on the in-process
    // path, so a client sees the same wire shape on both transports.
    let request = crate::types::Request::Server(Box::new(server_request));
    let id = crate::types::RequestId::from(correlation_id.clone());
    let message = TransportMessage::Request { id, request };

    // Ownership-in / ownership-back: `Some(message)` means nothing took it, i.e.
    // there is no live stream for the owning session. Dropping it there would
    // strand the caller until HTTP_DISPATCH_TIMEOUT while holding the server
    // mutex, so the correlation is failed AT ONCE instead.
    if v1::route_to_session_stream(v1_state, &session_id, message).is_some() {
        dispatcher
            .fail_pending(&correlation_id, NO_LIVE_STREAM)
            .await;
    }
}

// ---------------------------------------------------------------------------
// The session-bound peer handle.
// ---------------------------------------------------------------------------

/// A [`PeerHandle`] bound to ONE v1 session.
///
/// The multiplexing twin of
/// [`DispatchPeerHandle`](crate::server::peer_impl::DispatchPeerHandle): same
/// shared correlation authority, same request construction, same error mapping —
/// the ONLY difference is that every dispatch goes through `dispatch_owned` with
/// this handle's session id, never through `dispatch`. That is what lets the
/// drain send the request to the client that triggered it and lets the inbound
/// path refuse an answer from anyone else.
///
/// Three `Arc` clones per request and no allocation beyond that, matching the
/// cost bar `DispatchPeerHandle`'s rustdoc sets.
#[derive(Clone)]
pub(crate) struct SessionPeerHandle {
    dispatcher: Arc<ServerRequestDispatcher>,
    session_id: Arc<str>,
    /// The SAME one-way sink the backchannel carries, so
    /// [`PeerHandle::progress_notify`] and `extra.report_progress(..)` cannot
    /// disagree about where a v1 progress frame goes.
    notification_sink: NotificationSink,
}

/// Hand-written so a `{:?}` of this handle publishes no capability handle and no
/// session token — the same redaction discipline `TransportBackchannel` and
/// `ServerRequestDispatcher` already apply (T-118.1-10-09).
impl std::fmt::Debug for SessionPeerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionPeerHandle").finish_non_exhaustive()
    }
}

impl SessionPeerHandle {
    /// Bind the transport's dispatcher and this session's notification sink to
    /// one session.
    pub(crate) fn new(
        dispatcher: Arc<ServerRequestDispatcher>,
        session_id: &str,
        notification_sink: NotificationSink,
    ) -> Self {
        Self {
            dispatcher,
            session_id: Arc::from(session_id),
            notification_sink,
        }
    }
}

#[async_trait]
impl PeerHandle for SessionPeerHandle {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        let value = self
            .dispatcher
            .dispatch_owned(
                &self.session_id,
                ServerRequest::CreateMessage(Box::new(params)),
            )
            .await?;
        serde_json::from_value::<CreateMessageResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid sample response: {e}"),
            )
        })
    }

    async fn sample_with_tools(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        // The legacy-decode fallback is the twin's, verbatim: an older host may
        // answer the SAME `sampling/createMessage` with a single-content
        // `CreateMessageResult`, and that must not crash the tool call.
        let value = self
            .dispatcher
            .dispatch_owned(
                &self.session_id,
                ServerRequest::CreateMessage(Box::new(params)),
            )
            .await?;
        // Borrowing deserializer: the strict shape is TRIED first, so a clone
        // here would copy the whole completion payload on every call just to
        // fall back on the legacy shape. `&Value` leaves `value` owned for the
        // fallback below.
        if let Ok(with_tools) = CreateMessageResultWithTools::deserialize(&value) {
            return Ok(with_tools);
        }
        let legacy = serde_json::from_value::<CreateMessageResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid sample_with_tools response: {e}"),
            )
        })?;
        Ok(CreateMessageResultWithTools::from_single(legacy))
    }

    async fn list_roots(&self) -> Result<ListRootsResult> {
        let value = self
            .dispatcher
            .dispatch_owned(&self.session_id, ServerRequest::ListRoots)
            .await?;
        serde_json::from_value::<ListRootsResult>(value).map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid list_roots response: {e}"),
            )
        })
    }

    async fn elicit(&self, params: ElicitRequestParams) -> Result<ElicitResult> {
        let value = self
            .dispatcher
            .dispatch_owned(
                &self.session_id,
                ServerRequest::ElicitationCreate(Box::new(params)),
            )
            .await?;
        serde_json::from_value::<ElicitResult>(value).map_err(|e| {
            // Peer-supplied JSON deciding whether a human approved something must
            // FAIL rather than degrade to a default (plan 09, T-118.1-09-01).
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Invalid elicit response: {e}"),
            )
        })
    }

    /// Emit a `notifications/progress` frame on THIS session's live SSE stream.
    ///
    /// Goes through the same [`NotificationSink`] the request-scoped
    /// `ServerProgressReporter` uses, so a handler that reaches for the peer API
    /// and one that calls `extra.report_progress(..)` land on the identical
    /// vehicle and the identical session. Unlike
    /// [`DispatchPeerHandle::progress_notify`](crate::server::peer_impl::DispatchPeerHandle),
    /// this is NOT a no-op: the sink exists here because the transport built it
    /// at the one site that knows which session the request arrived on.
    ///
    /// # Still infallible, deliberately
    ///
    /// A one-way notification has no correlation to fail. A session whose SSE
    /// stream has gone away drops the frame and still answers `Ok(())`, matching
    /// both `route_to_session_stream`'s best-effort contract and
    /// `RequestHandlerExtra::report_progress`'s `None`-reporter guard. Returning
    /// a transport error here would break every caller that treats progress as
    /// infallible.
    ///
    /// No rate limiting is applied at this level — `ServerProgressReporter`
    /// owns that policy (10/s per token, plus a monotonic-progress rule) and it
    /// is the channel `extra.report_progress(..)` goes through.
    async fn progress_notify(
        &self,
        token: ProgressToken,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()> {
        let mut notification = crate::types::ProgressNotification::new(token, progress, message);
        if let Some(total) = total {
            notification = notification.with_total(total);
        }
        (self.notification_sink)(Notification::Progress(notification));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transport-side construction.
// ---------------------------------------------------------------------------

/// Build the V1 one-way notification sink bound to one session's live SSE
/// stream.
///
/// # This closure is v1-ONLY. Only the ATTACHMENT POINT and the TYPE are shared
///
/// [`route_to_session_stream`](v1::route_to_session_stream) is keyed by
/// `session_id`, and v2 has no sessions at all — a v2 GET answers `405`
/// (`sessions_active_truth_table`, `v2_verb_rejection`). Reusing this closure
/// there would look up a session id that cannot exist, `route_to_session_stream`
/// would hand the message straight back on every call, and EVERY v2 progress
/// notification would be silently dropped: a green build with a permanently red
/// `tools-call-with-progress` (T-118.1-11-08).
///
/// So [`attach_session_backchannel`] constructs this only when v1 sessions are
/// live for the request. v2's sink is plan 12's — a bounded per-request queue
/// whose receiver becomes the multi-frame SSE POST response body, which is a
/// different vehicle entirely. What the two eras SHARE is the attachment site
/// and the [`NotificationSink`] type, and that is the whole extent of it.
///
/// # Capture
///
/// Captures `V1State` (which is `Clone` and holds only the SSE-stream map, the
/// session table and the event store) plus the session id — NEVER `ServerState`,
/// which holds `Arc<Mutex<Server>>`. A sink outlives the request that built it,
/// so capturing the server mutex would put a lock handle on the notification
/// path and widen what a leaked closure can reach (T-118.1-11-07).
///
/// Best-effort by construction: a notification is one-way, so a session with no
/// live stream drops it rather than failing a correlation (there is none to
/// fail).
fn session_notification_sink(state: &ServerState, session_id: &str) -> NotificationSink {
    let v1_state = state.v1.clone();
    let session_id = session_id.to_string();
    Arc::new(move |notification| {
        let _ = v1::route_to_session_stream(
            &v1_state,
            &session_id,
            TransportMessage::Notification(notification),
        );
    })
}

/// Everything [`attach_session_backchannel`] needs about the request it is
/// deciding for.
///
/// A struct rather than four positional arguments because three of the four are
/// booleans-or-options that read identically at a call site.
pub(crate) struct BackchannelSite<'a> {
    /// The session this request arrived on, if it has one.
    pub(crate) session_id: Option<&'a str>,
    /// Whether sessions are live for this request's era (v2 has none, HTTP-01).
    pub(crate) sessions_on: bool,
    /// Whether this is the `initialize` handshake itself.
    pub(crate) is_init_request: bool,
}

/// The v1 context this transport would have resolved, had the server opted into
/// v2 era detection.
///
/// Built through the SAME shared unit `fold_v1_handshake_capabilities` uses for
/// its own `None` arm — `first_v1_version` over the server's accept-list — so a
/// context synthesised here is indistinguishable from the one dispatch would
/// have synthesised a moment later. That equality is the whole point: it is what
/// makes attaching a back-channel on a NON-opted-in server a no-op for every
/// other reader of the context.
///
/// Takes the server lock briefly. That is not a new blocking property: the
/// caller sits immediately after `extract_and_validate_auth`, which takes and
/// releases the same lock, and it runs BEFORE dispatch takes it for the handler.
async fn synthesised_v1_context(
    state: &ServerState,
) -> Option<crate::types::protocol::ProtocolContext> {
    let version = {
        let server = state.server.lock().await;
        crate::types::protocol::context::first_v1_version(server.supported_protocol_versions())
    }?;
    Some(crate::types::protocol::ProtocolContext::new(
        crate::types::protocol::Era::V1,
        version,
    ))
}

/// Attach this request's server-to-client capability handles to its
/// [`ProtocolContext`](crate::types::protocol::ProtocolContext).
///
/// Returns `context` UNCHANGED when there is no back-channel to offer:
///
/// * sessions suppressed for this era — v2 is session-free (HTTP-01), so there
///   is no stream a server-to-client request could be delivered on;
/// * no session on this request — same reason;
/// * the `initialize` handshake itself — the session is being minted by THIS
///   request and no SSE stream can exist yet, so a handle would be inert. Not
///   attaching also keeps `initialize`'s dispatch context exactly as it was.
///
/// # Why an ABSENT context is synthesised rather than skipped
///
/// A stock `Server::builder()` is not opted into v2, so `run_v2_header_gate`
/// short-circuits and this request's `protocol_context` is `None` (D-04's
/// zero-era-code rule) — on precisely the v1 stateful sessions the
/// server-to-client channel exists for. Attaching only to an already-resolved
/// context would therefore give the back-channel to v2-opted-in servers and to
/// nobody else, i.e. to no default deployment at all.
///
/// So a v1 context is synthesised HERE, from the same shared unit that
/// `fold_v1_handshake_capabilities` uses for its own `None` arm. Dispatch already
/// synthesises exactly this value one layer down whenever a v1 handshake has
/// happened, so what reaches the handler is unchanged in every field but the new
/// one — and `era` / `sessions_on` / the legacy-version guard have all already
/// been decided from the ORIGINAL `None` by the time this runs.
///
/// The handles are TRANSPORT-owned and bound to the ORIGINATING session here, at
/// the one site that knows which session this request arrived on. `attach_peer`
/// (plan 11) prefers them over the server's single global `peer_handle`, which is
/// what stops one client's `sampling/createMessage` reaching another's stream
/// (T-118.1-10-04, the T-113-07 class).
///
/// # The v2 branch is INERT here, and that is plan 12's seam
///
/// The `!site.sessions_on` early return means a v2 request gets NO backchannel:
/// no peer (there is no session stream to deliver a server-to-client request on)
/// and no `notification_sink`. A v2 handler therefore finds no reporter and
/// `extra.report_progress(..)` returns `Ok(())` silently — exactly as it did
/// before this phase, so nothing regresses.
///
/// Plan 12 fills that branch at THIS SAME point with a DIFFERENT closure: a
/// bounded per-request queue created before dispatch, whose receiver becomes the
/// multi-frame SSE POST response body. It must not reuse the v1 closure — see
/// [`session_notification_sink`] for why a session-keyed sink silently drops
/// every frame on an era that has no sessions (T-118.1-11-08).
pub(crate) async fn attach_session_backchannel(
    state: &ServerState,
    context: Option<crate::types::protocol::ProtocolContext>,
    site: BackchannelSite<'_>,
) -> Option<crate::types::protocol::ProtocolContext> {
    if !site.sessions_on || site.is_init_request {
        return context;
    }
    let Some(session_id) = site.session_id else {
        return context;
    };
    let carrier = match context {
        Some(ctx) => ctx,
        None => synthesised_v1_context(state).await?,
    };
    // ONE sink, shared by the peer handle and the backchannel, so
    // `peer.progress_notify(..)` and `extra.report_progress(..)` are provably
    // the same vehicle rather than two closures that happen to agree today.
    let sink = session_notification_sink(state, session_id);
    let peer: Arc<dyn PeerHandle> = Arc::new(SessionPeerHandle::new(
        dispatcher(state),
        session_id,
        Arc::clone(&sink),
    ));
    let backchannel = TransportBackchannel::new()
        .with_peer(peer)
        .with_notification_sink(sink);
    Some(carrier.with_transport_backchannel(backchannel))
}

// ---------------------------------------------------------------------------
// The inbound half: correlation BEFORE the server mutex.
//
// This is the deadlock fix (T-118.1-10-01), not an optimization. See
// `try_route_inbound_response` for the three lock sites it bypasses and for why
// bypassing them is sound for a response and for nothing else.
// ---------------------------------------------------------------------------

/// Try to answer this POST as an inbound JSON-RPC RESPONSE, before the pipeline
/// takes the server mutex.
///
/// Returns `Some(202 Accepted)` for EVERY response envelope — resolved, unknown
/// or wrongly-presented alike — and `None` for every other ingress, which then
/// falls through to the untouched gate / session / auth / dispatch pipeline.
///
/// # Why this must run before `resolve_v2_gate` and `extract_and_validate_auth`
///
/// `dispatch_public_request` holds `state.server.lock().await` for the ENTIRE
/// duration of a tool handler. A handler parked on `peer.sample()` therefore
/// holds the server mutex while it waits for the client's answer — and that
/// answer arrives as a POST whose first stages all take the same mutex:
///
/// * `run_v2_header_gate`, the accept-list read;
/// * `run_v2_header_gate` again, the negotiation read;
/// * `extract_and_validate_auth`, the auth-provider read.
///
/// Left in that order the answer can never reach the dispatcher that would
/// release the handler: a guaranteed deadlock, and a denial-of-service control,
/// because ONE tool call would take the whole transport offline.
///
/// # Why ONLY responses may bypass, and why that is not an auth bypass
///
/// An inbound response carries NO AUTHORITY. It invokes no method, reads nothing
/// and changes no server state: it can only resolve a correlation the SERVER
/// itself minted and is already waiting on. Requests and notifications keep
/// going through the full gate and auth pipeline, unchanged. Do NOT widen this —
/// a request that skipped `extract_and_validate_auth` would be a genuine
/// elevation of privilege (T-118.1-10-06).
///
/// # No new body read
///
/// `HttpIngress::Public(TransportMessage::Response(_))` is the ALREADY-PARSED
/// message, produced by the classifier from the buffer it already owns. The
/// existing `MAX_*` body bounds therefore still apply, unchanged and unmodified
/// (T-118.1-10-07), and this path allocates nothing per unknown id.
/// `pub(super)`, not `pub(crate)`: [`HttpIngress`] is private to the transport
/// module, and a `pub(crate)` signature naming it would be more visible than the
/// type it takes. Both call sites live in that module, so this is exactly wide
/// enough.
pub(super) async fn try_route_inbound_response(
    state: &ServerState,
    ingress: &HttpIngress,
    session_id: Option<&str>,
) -> Option<Response> {
    let HttpIngress::Public(TransportMessage::Response(response)) = ingress else {
        return None;
    };
    Some(route_inbound_response(state, response, session_id).await)
}

/// Correlate ONE inbound response envelope and answer `202 Accepted`.
///
/// Split out from [`try_route_inbound_response`] so the residual
/// `TransportMessage::Response` arms of the two dispatchers can route through the
/// identical path instead of silently discarding, which is what they used to do.
/// Those arms are unreachable by construction — the classification above answered
/// every response before either dispatcher ran — but a match must stay
/// exhaustive, and an exhaustive arm that DISCARDS is exactly the hole this plan
/// closed.
///
/// # The rejection shape is ONE shape, deliberately
///
/// Three negative cases — an unknown correlation id, an id owned by a DIFFERENT
/// session, and a response presented with no session at all — all answer the
/// same `202 Accepted`, with no body and no distinguishing header, and none of
/// them calls `handle_response`. Differentiating them (say `404` for unknown and
/// `403` for wrong-session) would turn this endpoint into an enumeration oracle
/// for live correlation ids (T-118.1-10-03). The correlation id is logged at
/// `debug`; the payload never is.
pub(crate) async fn route_inbound_response(
    state: &ServerState,
    response: &JSONRPCResponse,
    session_id: Option<&str>,
) -> Response {
    let correlation_id = response.id.to_string();
    let dispatcher = dispatcher(state);

    // OWNERSHIP FIRST. A client that POSTs a response carrying an id it never
    // received must not be able to resolve another session's pending
    // `sampling/createMessage` (T-118.1-10-02). The check is exactly
    // `owner_of(id) == Some(presented_session_id)`, and the RECORDED owner must
    // EXIST: a bare `owner_of(..) != session_id` comparison is satisfied by the
    // `None == None` cell, so a session-less POST (v2, or a stateless v1 server)
    // carrying any correlation id at all would pass ownership and be handed to
    // `handle_response`. Today that only ever answers `Err` for an unknown id,
    // but the moment anything on this transport dispatches WITHOUT recording an
    // owner, that cell becomes an unauthenticated resolution path.
    let owner = dispatcher.owner_of(&correlation_id).await;
    if owner.is_none() || owner.as_deref() != session_id {
        debug!(
            "Discarded inbound response for correlation {}: not owned by the presenting session",
            correlation_id
        );
        return StatusCode::ACCEPTED.into_response();
    }

    // Mapped exactly as `Server::route_response` maps it on the in-process path:
    // a result verbatim, an error through `serde_json::to_value`, so the awaiting
    // caller can tell the two apart.
    let payload = match &response.payload {
        crate::types::jsonrpc::ResponsePayload::Result(value) => value.clone(),
        crate::types::jsonrpc::ResponsePayload::Error(err) => {
            serde_json::to_value(err).unwrap_or(serde_json::Value::Null)
        },
    };

    if let Err(e) = dispatcher.handle_response(&correlation_id, payload).await {
        debug!("Failed to route response {}: {}", correlation_id, e);
    }
    StatusCode::ACCEPTED.into_response()
}
