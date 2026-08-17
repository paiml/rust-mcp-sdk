use crate::error::{Error, Result, TransportError};
use crate::shared::http_constants::{
    ACCEPT, ACCEPT_STREAMABLE, APPLICATION_JSON, CONTENT_TYPE, MCP_METHOD, MCP_NAME,
    MCP_PROTOCOL_VERSION, MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
// The resumption cursor header, imported behind the SAME gate as the constant
// itself (`http_constants::LAST_EVENT_ID`). This transport holds the LAST
// remaining reader of it in the crate — the server's moved into the paired
// module in plan 117-12 — so on a `full-v2` build nothing in `pmcp` names
// `Last-Event-ID` at all.
#[cfg(feature = "v1-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
use crate::shared::http_constants::LAST_EVENT_ID;
use crate::shared::sse_parser::SseParser;
use crate::shared::{Transport, TransportMessage};
use crate::types::mrtr::encode_header_value;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::{Method, Request, Response as HyperResponse, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use parking_lot::RwLock;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use url::Url;

/// Options for sending messages over streamable HTTP transport.
///
/// # Examples
///
/// Constructed with functional-update syntax so the example compiles on BOTH
/// feature sets: `resumption_token` exists only behind `v1-compat`, and
/// `..SendOptions::default()` names no gated field.
///
/// ```rust
/// use pmcp::shared::streamable_http::SendOptions;
///
/// // Default options for a simple message
/// let opts = SendOptions::default();
/// assert!(opts.related_request_id.is_none());
///
/// // Options with request correlation
/// let opts = SendOptions {
///     related_request_id: Some("req-123".to_string()),
///     ..SendOptions::default()
/// };
/// assert_eq!(opts.related_request_id.as_deref(), Some("req-123"));
/// ```
#[cfg_attr(
    feature = "v1-compat",
    doc = r#"
Resuming an interrupted stream is v1-only (`v1-compat`), so this example is
compiled only when that feature is on:

```rust
use pmcp::shared::streamable_http::SendOptions;

let opts = SendOptions {
    related_request_id: None,
    resumption_token: Some("event-456".to_string()),
};
assert_eq!(opts.resumption_token.as_deref(), Some("event-456"));
```
"#
)]
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Related request ID for associating responses
    pub related_request_id: Option<String>,
    /// Resumption token for continuing interrupted streams.
    ///
    /// v1-ONLY (`v1-compat`): MCP `2026-07-28` removed SSE resumability, so a
    /// `full-v2` build has no cursor to resume from and this field does not
    /// exist. The private `SendOptions::resumption_cursor` accessor answers the
    /// question for the send path on both feature sets.
    #[cfg(feature = "v1-compat")]
    pub resumption_token: Option<String>,
}

impl SendOptions {
    /// The resumption cursor this send should restart an SSE stream from.
    ///
    /// The `v1-compat` half: whatever the caller put in
    /// [`Self::resumption_token`].
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    fn resumption_cursor(&self) -> Option<String> {
        self.resumption_token.clone()
    }

    /// The null twin: a `full-v2` build carries no resumption cursor, so the
    /// answer is the constant `None`.
    ///
    /// Do NOT "improve" this by inspecting `self` — there is no field to
    /// inspect, and the point of the constant is that `send_with_options`
    /// needs no `#[cfg]` at its call site.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self)]
    const fn resumption_cursor(&self) -> Option<String> {
        None
    }
}

/// Configuration for the `StreamableHttpTransport`.
///
/// # Examples
///
/// Built through [`StreamableHttpTransportConfigBuilder`] rather than through a
/// struct literal, because two of this struct's fields (`session_id`,
/// `on_resumption_token`) exist only behind `v1-compat`: a literal naming them
/// does not compile on a `full-v2` build, while the builder compiles on both.
///
/// ```rust
/// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
/// use url::Url;
///
/// // Minimal configuration for stateless operation
/// let config = StreamableHttpTransportConfigBuilder::new(
///     Url::parse("http://localhost:8080").unwrap(),
/// )
/// .build();
/// assert!(!config.enable_json_response);
///
/// // Configuration for simple request/response (JSON instead of SSE)
/// let config = StreamableHttpTransportConfigBuilder::new(
///     Url::parse("http://localhost:8080").unwrap(),
/// )
/// .with_header("X-API-Key", "secret")
/// .enable_json_response()
/// .build();
/// assert!(config.enable_json_response);
/// assert_eq!(config.extra_headers.len(), 1);
/// ```
#[cfg_attr(
    feature = "v1-compat",
    doc = r#"
A session-bearing configuration is v1-only (MCP `2025-11-25`), so this example
compiles only when `v1-compat` is on:

```rust
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use url::Url;

let config = StreamableHttpTransportConfigBuilder::new(
    Url::parse("http://localhost:8080").unwrap(),
)
.with_session_id("session-123")
.build();
assert_eq!(config.session_id.as_deref(), Some("session-123"));
```
"#
)]
#[derive(Clone)]
pub struct StreamableHttpTransportConfig {
    /// The HTTP endpoint URL
    pub url: Url,
    /// Additional headers to include in requests
    pub extra_headers: Vec<(String, String)>,
    /// Optional authentication provider
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    /// Optional session ID (for stateful operation).
    ///
    /// v1-ONLY (`v1-compat`). MCP `2026-07-28` has no session at all, so a
    /// `full-v2` build stores none and this field does not exist — which is
    /// what makes "the severed client holds no session identifier to echo"
    /// (T-117-52) a property of the TYPE rather than of a runtime branch.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub session_id: Option<String>,
    /// Enable JSON responses instead of SSE (for simple request/response)
    pub enable_json_response: bool,
    /// Callback when resumption token is received.
    ///
    /// v1-ONLY (`v1-compat`). MCP `2026-07-28` removed SSE resumability, so
    /// there is no cursor to report back and a `full-v2` build has no such
    /// callback. `StreamableHttpTransport::resumption_callback` answers the
    /// question for both feature sets.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// HTTP middleware chain for request/response transformation
    pub http_middleware_chain: Option<Arc<crate::client::http_middleware::HttpMiddlewareChain>>,
}

impl StreamableHttpTransportConfig {
    /// Render the `v1-compat`-only fields into a `Debug` struct builder.
    ///
    /// Split out of [`Debug::fmt`] so the gate sits on an ITEM rather than as a
    /// `#[cfg]` wedged into the middle of a method-call chain.
    #[cfg(feature = "v1-compat")]
    fn debug_v1_fields(&self, out: &mut std::fmt::DebugStruct<'_, '_>) {
        out.field("session_id", &self.session_id)
            .field("on_resumption_token", &self.on_resumption_token.is_some());
    }

    /// The null twin: a `full-v2` config carries no v1 fields to render.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self)]
    const fn debug_v1_fields(&self, _out: &mut std::fmt::DebugStruct<'_, '_>) {}
}

impl Debug for StreamableHttpTransportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("StreamableHttpTransportConfig");
        out.field("url", &self.url)
            .field("extra_headers", &self.extra_headers)
            .field("auth_provider", &self.auth_provider.is_some())
            .field("enable_json_response", &self.enable_json_response)
            .field(
                "http_middleware_chain",
                &self.http_middleware_chain.is_some(),
            );
        self.debug_v1_fields(&mut out);
        out.finish()
    }
}

/// Builder for `StreamableHttpTransportConfig`.
///
/// Provides a fluent API for configuring HTTP transport with middleware support.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
/// use pmcp::client::http_middleware::HttpMiddlewareChain;
/// use url::Url;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), pmcp::Error> {
/// let mut http_chain = HttpMiddlewareChain::new();
/// // Add middleware to chain...
///
/// let config = StreamableHttpTransportConfigBuilder::new(
///         Url::parse("http://localhost:8080").unwrap()
///     )
///     .with_http_middleware(Arc::new(http_chain))
///     .with_header("X-API-Key", "secret")
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct StreamableHttpTransportConfigBuilder {
    url: Url,
    extra_headers: Vec<(String, String)>,
    auth_provider: Option<Arc<dyn AuthProvider>>,
    /// v1-ONLY (`v1-compat`) — mirrors `StreamableHttpTransportConfig::session_id`.
    #[cfg(feature = "v1-compat")]
    session_id: Option<String>,
    enable_json_response: bool,
    /// v1-ONLY (`v1-compat`) — mirrors
    /// `StreamableHttpTransportConfig::on_resumption_token`.
    #[cfg(feature = "v1-compat")]
    on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
    http_middleware_chain: Option<Arc<crate::client::http_middleware::HttpMiddlewareChain>>,
}

impl StreamableHttpTransportConfigBuilder {
    /// Render the `v1-compat`-only builder fields into a `Debug` struct
    /// builder — the builder's counterpart of
    /// `StreamableHttpTransportConfig::debug_v1_fields`.
    #[cfg(feature = "v1-compat")]
    fn debug_v1_fields(&self, out: &mut std::fmt::DebugStruct<'_, '_>) {
        out.field("session_id", &self.session_id)
            .field("on_resumption_token", &self.on_resumption_token.is_some());
    }

    /// The null twin: a `full-v2` builder carries no v1 fields to render.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self)]
    const fn debug_v1_fields(&self, _out: &mut std::fmt::DebugStruct<'_, '_>) {}
}

impl Debug for StreamableHttpTransportConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("StreamableHttpTransportConfigBuilder");
        out.field("url", &self.url)
            .field("extra_headers", &self.extra_headers)
            .field("auth_provider", &self.auth_provider.is_some())
            .field("enable_json_response", &self.enable_json_response)
            .field(
                "http_middleware_chain",
                &self.http_middleware_chain.is_some(),
            );
        self.debug_v1_fields(&mut out);
        out.finish()
    }
}

impl StreamableHttpTransportConfigBuilder {
    /// Create a new config builder with the specified URL.
    pub fn new(url: Url) -> Self {
        Self {
            url,
            extra_headers: Vec::new(),
            auth_provider: None,
            #[cfg(feature = "v1-compat")]
            session_id: None,
            enable_json_response: false,
            // `#[cfg]` only. A `doc(cfg(..))` badge on a struct-EXPRESSION field
            // documents nothing — there is no item here for rustdoc to badge —
            // and rustc rejects it as a misplaced `#[doc]` under `--cfg docsrs`,
            // which is exactly the configuration docs.rs builds with. The badge
            // belongs on the FIELD DECLARATION, where it already is.
            #[cfg(feature = "v1-compat")]
            on_resumption_token: None,
            http_middleware_chain: None,
        }
    }

    /// Add an HTTP header to include in all requests.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    /// Set the authentication provider.
    pub fn with_auth_provider(mut self, provider: Arc<dyn AuthProvider>) -> Self {
        self.auth_provider = Some(provider);
        self
    }

    /// Set the session ID for stateful operation.
    ///
    /// v1-ONLY (`v1-compat`): MCP `2026-07-28` has no session to set, so this
    /// method does not exist on a `full-v2` build.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Enable JSON responses instead of SSE streams.
    pub fn enable_json_response(mut self) -> Self {
        self.enable_json_response = true;
        self
    }

    /// Set callback for resumption token updates.
    ///
    /// v1-ONLY (`v1-compat`): MCP `2026-07-28` removed SSE resumability, so
    /// there is no cursor to report and this method does not exist on a
    /// `full-v2` build.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub fn on_resumption_token(mut self, callback: Arc<dyn Fn(String) + Send + Sync>) -> Self {
        self.on_resumption_token = Some(callback);
        self
    }

    /// Set the HTTP middleware chain for request/response transformation.
    ///
    /// HTTP middleware operates at the transport layer, before protocol processing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
    /// use pmcp::client::http_middleware::HttpMiddlewareChain;
    /// use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
    /// use url::Url;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let mut http_chain = HttpMiddlewareChain::new();
    ///
    /// // Add OAuth middleware
    /// let token = BearerToken::with_expiry("my-token".to_string(), Duration::from_hours(1));
    /// http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));
    ///
    /// let config = StreamableHttpTransportConfigBuilder::new(
    ///         Url::parse("http://localhost:8080").unwrap()
    ///     )
    ///     .with_http_middleware(Arc::new(http_chain))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_http_middleware(
        mut self,
        chain: Arc<crate::client::http_middleware::HttpMiddlewareChain>,
    ) -> Self {
        self.http_middleware_chain = Some(chain);
        self
    }

    /// Build the configuration.
    pub fn build(self) -> StreamableHttpTransportConfig {
        StreamableHttpTransportConfig {
            url: self.url,
            extra_headers: self.extra_headers,
            auth_provider: self.auth_provider,
            #[cfg(feature = "v1-compat")]
            session_id: self.session_id,
            enable_json_response: self.enable_json_response,
            #[cfg(feature = "v1-compat")]
            on_resumption_token: self.on_resumption_token,
            http_middleware_chain: self.http_middleware_chain,
        }
    }
}

/// Default cap on ONE fully-collected HTTP response body, in bytes (16 MiB).
///
/// # What this bounds
///
/// Exactly one thing: the size of a single response body that
/// [`StreamableHttpTransport`] reads into memory in one piece. Every one of this
/// transport's response reads is a whole-body read — the POST response, the GET
/// SSE stream and the v2 structured-error envelope — and the peer chooses how
/// many bytes it sends.
///
/// # Why it exists
///
/// The SSE parser's complete-body entry point deliberately performs NO bound
/// check of its own: the body handed to it was already read into memory in one
/// piece, so the parser's incremental in-flight bound is meaningless there. That
/// makes this cap the ONLY thing standing between a hostile or merely broken peer
/// and an unbounded allocation in this process (T-113-84). Before Phase 113-20
/// the precondition was stated but not met — every call site used a bare,
/// uncapped `collect()`.
///
/// Enforcement is a STREAMING bound, not a post-hoc check: a peer-declared
/// `Content-Length` over the cap is refused before a single body byte is read,
/// and the bytes actually delivered are read through `Limited`, which stops at
/// the cap. A peer that understates or omits `Content-Length` therefore gains
/// nothing (T-113-93). Collecting the whole body and only then measuring it would
/// perform exactly the allocation this cap exists to prevent.
///
/// # Deliberately NOT the same limit as the SSE in-flight ceiling
///
/// [`crate::shared::http::DEFAULT_HTTP_SSE_BUFFERED_BYTES`] bounds INCREMENTAL
/// in-flight retention inside a long-lived `HttpTransport` reader — a running
/// total across many chunks, on a different transport. This constant bounds a
/// ONE-SHOT collected body on `StreamableHttpTransport`. They are two different
/// quantities on two different types and share no configuration surface; do not
/// "unify" them.
///
/// # What breaks at this boundary
///
/// A response larger than the configured cap now fails with
/// [`TransportError::Request`] instead of being delivered. That is a real
/// behaviour change. MCP `image`/`audio` content is unconstrained base64 and
/// base64 expands by ~4/3, so a 12 MiB binary is ALREADY 16 MiB once encoded,
/// before the JSON envelope — such a payload does NOT fit under this default.
/// [`StreamableHttpTransport::with_max_collected_body_bytes`] is the escape
/// hatch.
pub const DEFAULT_MAX_COLLECTED_BODY_BYTES: usize = 16 * 1024 * 1024;

/// How many server-to-client messages may sit between a reader task and
/// [`Transport::receive`] before the reader must wait (Phase 118.2, D-04).
///
/// # What it bounds
///
/// The IN-FLIGHT messages on this transport's receive queue. Worst-case in-flight
/// memory is therefore `CLIENT_RECEIVE_QUEUE_CAPACITY` x
/// `StreamableHttpTransport::max_collected_body_bytes`. Before this constant the
/// queue was `mpsc::unbounded_channel()`, which is not a bound at all: a peer
/// that pushes frames faster than the application consumes them grows this
/// process's heap for the lifetime of the connection.
///
/// # The policy is AWAIT CAPACITY, never drop
///
/// A spawned reader task `.await`s capacity, so backpressure lands on the reader,
/// the TCP window closes, and the peer stops writing. That is deliberately the
/// OPPOSITE of the server-side `V2_PROGRESS_QUEUE_CAPACITY` precedent
/// (`streamable_http_server.rs`), which is the same size and `try_send`s with a
/// drop-newest policy. Said here rather than left for a reader to assume drift:
/// this queue carries `sampling` / `roots` / `elicit` **requests**, and a dropped
/// request strands a correlation until it times out, whereas a dropped progress
/// frame is superseded by the next one. The two directions are allowed to differ
/// precisely because their payloads differ.
///
/// The CALLER-task send sites (inside [`StreamableHttpTransport::post_body`])
/// are the exception, and they `try_send` instead: `Client::dispatch_request`
/// calls `send()` and only then loops on `receive()`, so a caller-task site that
/// blocked on a full queue could never be drained by the consumer that is inside
/// it. Those sites fail loudly with a named error rather than blocking.
///
/// # Why the element is a `Result`
///
/// So a reader task can report WHY a stream ended. See [`Transport::receive`] for
/// the five end reasons and how they differ.
const CLIENT_RECEIVE_QUEUE_CAPACITY: usize = 64;

/// How long the session-stream reader waits before its FIRST reconnect
/// (Phase 118.2, D-03).
///
/// The REFERENCE client's `initialReconnectionDelay`
/// (`@modelcontextprotocol/sdk/dist/esm/client/streamableHttp.js:9`), and taken
/// from there deliberately: a pmcp client that blinks back on a different
/// schedule than every JavaScript client a server operator has already tuned for
/// is a pmcp-specific operational surprise, not an improvement.
///
/// Spelled in SECONDS rather than as the reference's literal `1000` ms because
/// `clippy::duration_suboptimal_units` requires it; the value is identical.
const INITIAL_SSE_RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// The ceiling on any reconnect wait, including one a SERVER asked for.
///
/// The reference client's `maxReconnectionDelay` (`streamableHttp.js:10`). It
/// caps the exponential curve, and — unlike the reference — it also caps a
/// server-provided SSE `retry:` value. That is deliberate: `retry:` is
/// peer-controlled, and an uncapped one lets a peer park a client's reader task
/// for an arbitrary duration.
///
/// This is only HALF the story, and was once mistaken for the whole of it
/// (CR-01). A ceiling bounds the direction that makes the client too SLOW; the
/// direction that makes it too FAST is bounded by
/// [`MIN_SSE_RECONNECT_DELAY`], and a peer that can reach either end can
/// choose which denial of service to inflict.
///
/// Spelled in SECONDS rather than as the reference's literal `30000` ms for the
/// same lint reason as [`INITIAL_SSE_RECONNECT_DELAY`]; the value is identical.
const MAX_SSE_RECONNECT_DELAY: Duration = Duration::from_secs(30);

/// The FLOOR under any reconnect wait, including one a SERVER asked for
/// (Phase 118.2 plan 14, CR-01).
///
/// `retry:` is remote input in BOTH directions, and the reference client bounds
/// neither. An uncapped value parks a client's reader task for a duration the
/// peer chose ([`MAX_SSE_RECONNECT_DELAY`] closes that end). An UNFLOORED one is
/// worse: `SseEvent::retry` is `Option<u64>` milliseconds parsed straight off the
/// wire, so a `retry: 0` from a hostile server — or from a broken proxy replaying
/// a cached page — turns the reconnect loop into a tight spin that burns a core
/// locally, floods the peer's ingress, and re-mints an access token through the
/// configured `AuthProvider` on every single iteration
/// (T-118.2-14-01, T-118.2-14-02).
///
/// 500 ms because it is short enough to be invisible against a genuine proxy
/// blink — the case D-03 exists for, where the reference curve's first wait is
/// already a full second — and long enough that a peer asking for zero cannot
/// convert one SSE frame into an unbounded request rate.
///
/// PRIVATE, with no `with_*` override, for the same reason
/// [`MAX_SSE_RECONNECT_ATTEMPTS`] is: every knob on this transport is private
/// precisely so that none of them is a semver event.
const MIN_SSE_RECONNECT_DELAY: Duration = Duration::from_millis(500);

/// How long a re-opened stream must STAY UP to earn a fresh reconnect budget
/// (Phase 118.2 plan 14, CR-01).
///
/// The budget used to be refunded on any single delivered event. That reads as
/// generous and is in fact the other half of the unbounded-loop defect: a peer
/// that writes ONE frame per body and then closes refunds the whole budget on
/// every iteration, so the loop never terminates no matter how small
/// [`MAX_SSE_RECONNECT_ATTEMPTS`] is.
///
/// A working stream is one that stayed up, not one that bounced with a frame
/// attached. 30 s is comfortably longer than any single-frame bounce and
/// comfortably shorter than the ALB-class idle timeouts (~60 s) D-03 exists to
/// survive, so a genuinely healthy stream that blinks after an hour still earns
/// its fresh budget.
const RECONNECT_BUDGET_RESET_UPTIME: Duration = Duration::from_secs(30);

/// Whether a body that just ended earns a FRESH reconnect budget.
///
/// A free function rather than an inline conjunction inside
/// [`StreamableHttpTransport::run_session_stream`], for two reasons. That loop is
/// already split four ways specifically to hold the repo's PMAT cog-25 budget
/// (CLAUDE.md), and adding an `&&` with an `Instant::elapsed()` call pushes it
/// back over. And a pure predicate is unit-testable at both sides of its
/// threshold with no clock manipulation at all, which is what
/// `mod reconnect_delay_bounds` does.
fn budget_reset_earned(delivered: bool, uptime: Duration) -> bool {
    delivered && uptime >= RECONNECT_BUDGET_RESET_UPTIME
}

/// How fast the reconnect wait grows per attempt.
///
/// The reference client's `reconnectionDelayGrowFactor`
/// (`streamableHttp.js:11`), applied as `initial * growth^attempt` exactly as
/// its `_getNextReconnectionDelay` does.
const SSE_RECONNECT_GROWTH: f64 = 1.5;

/// How many times a dropped session stream is re-opened before the reader gives
/// up LOUDLY.
///
/// The reference client's `maxRetries` (`streamableHttp.js:12`), and
/// deliberately small. D-03's stated purpose is surviving a proxy BLINK — an
/// ALB or gateway idle timeout closing an otherwise healthy stream — not
/// indefinite reconnection to a peer that is gone. An unbounded loop against a
/// server that keeps closing is a self-inflicted denial of service aimed at a
/// server that is already unwell (T-118.2-04-01).
///
/// PRIVATE, and with no `with_*` override, because no caller in this repo needs
/// one. Were one ever needed it would go on [`StreamableHttpTransport`] as an
/// inherent method — never as a field on
/// [`StreamableHttpTransportConfig`], which is externally constructible with
/// all-`pub` fields and would therefore make a new field a MAJOR semver event.
const MAX_SSE_RECONNECT_ATTEMPTS: u32 = 2;

/// Build the over-cap refusal shared by every collected-body read.
///
/// Names the LIMIT and the observed size, and deliberately echoes no body
/// content: the refusal must not become a channel for the very bytes it refused.
/// `declared` is `Some` only when the peer's `Content-Length` was itself over the
/// cap; when the peer understated or omitted it the read is stopped mid-flight
/// and no total is knowable, so the message says so rather than inventing one.
///
/// Lives in the same [`TransportError::Request`] family the collect sites already
/// used, so a caller matching on the error family sees no new shape.
fn collected_body_over_cap(max_bytes: usize, declared: Option<usize>) -> Error {
    let observed = match declared {
        Some(bytes) => format!("declares Content-Length {bytes}"),
        None => "delivered more than the cap (Content-Length absent or understated)".to_string(),
    };
    Error::Transport(TransportError::Request(format!(
        "response body {observed}, over this transport's {max_bytes}-byte collected-body cap \
         (DEFAULT_MAX_COLLECTED_BODY_BYTES); raise it with \
         StreamableHttpTransport::with_max_collected_body_bytes"
    )))
}

/// What an outbound POST carries, as far as the `202 Accepted` handler needs to
/// know (Phase 118.2, Defect A).
///
/// Deliberately NOT the broad `is_notification: bool` this replaced. The
/// reference implementation guards its stream-open on
/// `isInitializedNotification(message)`
/// (`@modelcontextprotocol/sdk/dist/esm/client/streamableHttp.js:370-377`), and
/// the difference is not cosmetic: [`StreamableHttpTransport::start_sse`] aborts
/// the live reader as its FIRST act, so a broad predicate tears down and re-opens
/// the session stream on every `notifications/cancelled` and
/// `notifications/progress` the client sends.
///
/// Derived from the TYPED outbound message, never re-parsed from the serialized
/// body: the typed path already has the identity in hand, and re-parsing would
/// let a hostile body influence which client branch runs (T-118.2-01-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutboundFrame {
    /// The `notifications/initialized` notification, specifically.
    InitializedNotification,
    /// Every other frame: a request, a response, or any other notification.
    Other,
}

impl OutboundFrame {
    /// Classify a typed transport message.
    fn of(message: &TransportMessage) -> Self {
        match message {
            TransportMessage::Notification(crate::types::Notification::Client(
                crate::types::ClientNotification::Initialized,
            )) => Self::InitializedNotification,
            _ => Self::Other,
        }
    }
}

/// The shared state EVERY outgoing request on this transport is built from.
///
/// # Why this exists
///
/// [`StreamableHttpTransport::build_request_with_middleware`] and
/// [`StreamableHttpTransport::process_response_headers`] were `&self` methods,
/// which made them unreachable from the `'static` reconnect task (Phase 118.2,
/// D-03): a spawned task cannot borrow the transport. Cloning the whole
/// transport into the task is not a way around it either — the task would then
/// own an `Arc` holding its OWN join handle, so dropping the last real transport
/// clone could never release it (see [`SseReaderContext`]).
///
/// A BORROWED view rather than an owned struct, so neither caller clones an
/// `Arc` merely to build a request. [`StreamableHttpTransport`] and
/// [`SseReaderContext`] each hand one out, and the request builder underneath is
/// therefore ONE builder with TWO callers.
///
/// Do NOT fork a second builder for the reconnect path. A divergent request
/// builder is precisely how a reconnect quietly stops sending the
/// `Authorization` header or stops running the request-middleware chain
/// (T-118.2-04-07) — the failure would be invisible until an auth-enforcing
/// deployment saw its stream 401 after every idle timeout.
struct RequestParts<'a> {
    config: &'a Arc<RwLock<StreamableHttpTransportConfig>>,
    protocol_version: &'a Arc<RwLock<Option<String>>>,
    v2_mode: &'a Arc<AtomicBool>,
}

impl RequestParts<'_> {
    /// Whether this connection speaks the v2 (`2026-07-28`) wire contract.
    ///
    /// The same read [`StreamableHttpTransport::is_v2`] performs, on the same
    /// `Arc`; see [`StreamableHttpTransport::v2_mode`] for why it is not derived
    /// from the negotiated protocol version.
    fn is_v2(&self) -> bool {
        self.v2_mode.load(Ordering::Relaxed)
    }
}

/// A streamable HTTP transport for MCP.
///
/// This transport supports both stateless and stateful operation modes:
/// - Stateless: No session tracking, each request is independent (suitable for Lambda)
/// - Stateful: Optional session ID tracking for persistent sessions
///
/// The transport can handle both JSON responses and SSE streams based on server response.
///
/// HTTPS is supported via rustls with the ring crypto provider, which is compatible
/// with AWS Lambda and other serverless environments.
#[derive(Clone)]
pub struct StreamableHttpTransport {
    config: Arc<RwLock<StreamableHttpTransportConfig>>,
    client: Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        Full<Bytes>,
    >,
    /// Channel for receiving messages from SSE streams or responses.
    ///
    /// BOUNDED at [`CLIENT_RECEIVE_QUEUE_CAPACITY`], and its element is a
    /// `Result` so a reader task can deliver a terminal reason rather than
    /// vanishing. See [`Transport::receive`].
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Result<TransportMessage>>>>,
    /// Sender for messages. See [`Self::receiver`].
    sender: mpsc::Sender<Result<TransportMessage>>,
    /// Protocol version negotiated with server
    protocol_version: Arc<RwLock<Option<String>>>,
    /// Whether the CLIENT explicitly selected the v2 (`2026-07-28`) era
    /// (Phase 113, CLNT-01).
    ///
    /// Written ONLY by [`Transport::set_negotiated_protocol_version`], i.e. once
    /// at `ClientBuilder::build` time. Deliberately separate from
    /// [`Self::protocol_version`], which [`Self::process_response_headers`]
    /// overwrites from whatever the SERVER echoed: a rogue or confused server
    /// replying `MCP-Protocol-Version: 2026-07-28` must not be able to flip a v1
    /// client into v2 emission mode (which would suppress its session id and
    /// break the connection).
    v2_mode: Arc<AtomicBool>,
    /// Abort controller for SSE streams
    abort_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Last event ID for resumability
    last_event_id: Arc<RwLock<Option<String>>>,
    /// Cap on ONE fully-collected response body, defaulted from
    /// [`DEFAULT_MAX_COLLECTED_BODY_BYTES`] and overridable through
    /// [`Self::with_max_collected_body_bytes`].
    ///
    /// A PRIVATE field on the transport rather than a `pub` field on
    /// [`StreamableHttpTransportConfig`]: that config struct is externally
    /// constructible with all-`pub` fields and no `Default` derive, and its own
    /// rustdoc carries three struct-literal examples enumerating every field, so
    /// adding a field to it fails `cargo semver-checks`'s
    /// `constructible_struct_adds_field` and would force pmcp to a MAJOR
    /// version. Measured, not assumed — see plan 113-17's
    /// `<config_surface_decision>`. Every field of THIS struct is already
    /// private, so adding one here is invisible to semver (T-113-95).
    max_collected_body_bytes: usize,
}

impl Debug for StreamableHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransport")
            .field("config", &self.config)
            .field("protocol_version", &self.protocol_version)
            .field("last_event_id", &self.last_event_id)
            .field("max_collected_body_bytes", &self.max_collected_body_bytes)
            .finish()
    }
}

impl StreamableHttpTransport {
    /// Creates a new `StreamableHttpTransport`.
    ///
    /// This automatically sets up HTTPS support using rustls with the ring crypto provider.
    /// Both HTTP and HTTPS URLs are supported.
    ///
    /// Note: Currently uses HTTP/1.1 only for maximum compatibility with various
    /// MCP servers and API gateways. HTTP/2 can be enabled via `new_with_http2()`.
    pub fn new(config: StreamableHttpTransportConfig) -> Self {
        Self::new_internal(config, false)
    }

    /// Creates a new `StreamableHttpTransport` with HTTP/2 support enabled.
    ///
    /// This enables both HTTP/1.1 and HTTP/2, with HTTP/2 being preferred via ALPN
    /// negotiation when the server supports it.
    ///
    /// Note: Some servers or API gateways may have issues with HTTP/2. If you
    /// experience empty responses or connection issues, try using `new()` instead
    /// which uses HTTP/1.1 only.
    pub fn new_with_http2(config: StreamableHttpTransportConfig) -> Self {
        Self::new_internal(config, true)
    }

    /// Internal constructor with HTTP version control.
    fn new_internal(config: StreamableHttpTransportConfig, enable_http2: bool) -> Self {
        // Install ring crypto provider explicitly to avoid conflicts with aws-lc-rs
        // in Lambda environments. This is idempotent - safe to call multiple times.
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Create HTTPS connector that supports both HTTP and HTTPS
        let https = if enable_http2 {
            tracing::debug!("Creating HTTPS connector with HTTP/1.1 and HTTP/2 support");
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .expect("Failed to load native root certificates")
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build()
        } else {
            tracing::debug!("Creating HTTPS connector with HTTP/1.1 only");
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .expect("Failed to load native root certificates")
                .https_or_http()
                .enable_http1()
                .build()
        };

        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build(https);

        let (sender, receiver) = mpsc::channel(CLIENT_RECEIVE_QUEUE_CAPACITY);
        Self {
            config: Arc::new(RwLock::new(config)),
            client,
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            sender,
            protocol_version: Arc::new(RwLock::new(None)),
            v2_mode: Arc::new(AtomicBool::new(false)),
            abort_handle: Arc::new(RwLock::new(None)),
            last_event_id: Arc::new(RwLock::new(None)),
            max_collected_body_bytes: DEFAULT_MAX_COLLECTED_BODY_BYTES,
        }
    }

    /// Override the cap on ONE fully-collected response body.
    ///
    /// Defaults to [`DEFAULT_MAX_COLLECTED_BODY_BYTES`] (16 MiB). Raise it for a
    /// deployment whose responses are legitimately larger — base64 `image` /
    /// `audio` content expands by ~4/3, so a 12 MiB binary does NOT fit under the
    /// default once encoded.
    ///
    /// Additive by construction: an inherent method on a struct whose fields are
    /// all private, rather than a field on the externally-constructible
    /// [`StreamableHttpTransportConfig`] (see `Self::max_collected_body_bytes`).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::shared::streamable_http::{
    ///     StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
    /// };
    /// use url::Url;
    ///
    /// let config =
    ///     StreamableHttpTransportConfigBuilder::new(Url::parse("http://localhost:8080").unwrap())
    ///         .build();
    /// let transport =
    ///     StreamableHttpTransport::new(config).with_max_collected_body_bytes(64 * 1024 * 1024);
    /// ```
    #[must_use]
    pub fn with_max_collected_body_bytes(mut self, max_collected_body_bytes: usize) -> Self {
        self.max_collected_body_bytes = max_collected_body_bytes;
        self
    }

    /// Collect a response body, refusing anything over `max_bytes`.
    ///
    /// The ONE place this transport turns a peer-controlled response into an
    /// in-memory buffer. Two independently-sufficient refusals:
    ///
    /// 1. A declared `Content-Length` over the cap is refused before a single
    ///    body byte is read. The header is a peer-controlled OPTIMISATION, never
    ///    the authority.
    /// 2. The bytes actually delivered are read through `Limited`, which stops at
    ///    the cap. A peer that understates or omits `Content-Length` therefore
    ///    gains nothing (T-113-93), and the allocation is bounded DURING the read
    ///    rather than measured after it.
    ///
    /// A body of exactly `max_bytes` is admitted; one byte over is refused.
    async fn collect_body_within_cap(
        response: HyperResponse<hyper::body::Incoming>,
        max_bytes: usize,
    ) -> Result<Bytes> {
        let declared = response
            .headers()
            .get(hyper::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok());
        if let Some(declared) = declared {
            if declared > max_bytes {
                return Err(collected_body_over_cap(max_bytes, Some(declared)));
            }
        }

        match Limited::new(response.into_body(), max_bytes)
            .collect()
            .await
        {
            Ok(collected) => Ok(collected.to_bytes()),
            Err(error) if error.is::<LengthLimitError>() => {
                Err(collected_body_over_cap(max_bytes, None))
            },
            Err(error) => Err(Error::Transport(TransportError::Request(error.to_string()))),
        }
    }

    /// [`Self::collect_body_within_cap`] at THIS transport's configured cap.
    ///
    /// The seam the `subscriptions/listen` client uses for the ONE collected-body
    /// read that lives outside this module: `open_event_stream`'s non-stream
    /// REJECTION path, which reads a peer-controlled error envelope off the same
    /// response `post_streaming` returned. Routing it here rather than through a
    /// bare `body.collect()` is what keeps "every one of this transport's
    /// whole-body reads is capped" true (review CR-01, T-113-84) — and doing it
    /// through the transport's own configured value means
    /// [`Self::with_max_collected_body_bytes`] raises this cap too, rather than
    /// leaving one read pinned to a constant a deployment cannot move.
    pub(crate) async fn collect_capped_body(
        &self,
        response: HyperResponse<hyper::body::Incoming>,
    ) -> Result<Bytes> {
        Self::collect_body_within_cap(response, self.max_collected_body_bytes).await
    }

    /// Put a message on the receive queue from a CALLER task (Phase 118.2, D-04).
    ///
    /// `try_send`, deliberately, where a spawned reader task would `.await`
    /// capacity: [`crate::Client`]'s `dispatch_request` calls `send()` and only
    /// THEN loops on `receive()`, so a caller-task site that blocked on a full
    /// queue could never be drained by the consumer that is inside it — a
    /// self-deadlock. Failing loudly with an error naming
    /// [`CLIENT_RECEIVE_QUEUE_CAPACITY`] satisfies D-04's never-silently-drop
    /// rule while keeping the send path non-blocking.
    ///
    /// A closed channel maps to the same [`TransportError::Send`] family the
    /// unbounded sender's failure mapped to, so a caller matching on the error
    /// family sees no new shape.
    fn queue_from_caller(&self, message: TransportMessage) -> Result<()> {
        self.sender.try_send(Ok(message)).map_err(|error| {
            Error::Transport(TransportError::Send(match error {
                mpsc::error::TrySendError::Full(_) => format!(
                    "the client receive queue is full at its {CLIENT_RECEIVE_QUEUE_CAPACITY}-message \
                     capacity (CLIENT_RECEIVE_QUEUE_CAPACITY); the application is not draining \
                     Transport::receive() fast enough"
                ),
                mpsc::error::TrySendError::Closed(_) => {
                    "the client receive queue is closed".to_string()
                },
            }))
        })
    }

    /// Whether this connection speaks the v2 (`2026-07-28`) wire contract.
    ///
    /// See [`Self::v2_mode`] for why this is not derived from
    /// [`Self::protocol_version`].
    fn is_v2(&self) -> bool {
        self.v2_mode.load(Ordering::Relaxed)
    }

    /// The callback to notify when an SSE event carries a resumption cursor.
    ///
    /// The `v1-compat` half: whatever
    /// [`StreamableHttpTransportConfig::on_resumption_token`] holds.
    #[cfg(feature = "v1-compat")]
    fn resumption_callback(&self) -> Option<Arc<dyn Fn(String) + Send + Sync>> {
        self.config.read().on_resumption_token.clone()
    }

    /// The null twin: a `full-v2` build has no resumption cursor to report, so
    /// the answer is the constant `None`.
    ///
    /// Do NOT "improve" this by reading the config — the field does not exist
    /// on this build. Answering here is what keeps the two SSE-parse loops free
    /// of a `#[cfg]` at their call sites.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self)]
    const fn resumption_callback(&self) -> Option<Arc<dyn Fn(String) + Send + Sync>> {
        None
    }

    /// Write the SSE resumption cursor onto an outgoing GET request.
    ///
    /// v1-ONLY, with NO null twin — deliberately. This is the LAST remaining
    /// reader of [`crate::shared::http_constants::LAST_EVENT_ID`] in the crate
    /// (the server's moved into the paired module in plan 117-12), so the
    /// constant and this function carry the same `#[cfg]`, applied in one edit;
    /// gating either alone is a compile break.
    ///
    /// A twin returning `Ok(())` would be indistinguishable from absence and
    /// would only tempt a later author to "improve" it by logging the ignored
    /// cursor. On a `full-v2` build nothing in `pmcp` names `Last-Event-ID` at
    /// all — which is what makes T-117-53 a property of the compiled crate.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    fn apply_resumption_header(
        request: &mut Request<Full<Bytes>>,
        resumption_token: Option<&str>,
    ) -> Result<()> {
        if let Some(token) = resumption_token {
            request.headers_mut().insert(
                LAST_EVENT_ID,
                token.parse().map_err(|e| {
                    Error::Transport(TransportError::InvalidMessage(format!(
                        "Invalid header: {}",
                        e
                    )))
                })?,
            );
        }
        Ok(())
    }

    /// Get the current session ID.
    ///
    /// v1-ONLY (`v1-compat`): a `full-v2` build stores no session id, so there
    /// is nothing to get and this accessor does not exist.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub fn session_id(&self) -> Option<String> {
        self.config.read().session_id.clone()
    }

    /// Set the session ID (useful for resuming sessions).
    ///
    /// v1-ONLY (`v1-compat`), for the same reason as [`Self::session_id`].
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub fn set_session_id(&self, session_id: Option<String>) {
        self.config.write().session_id = session_id;
    }

    /// The `Mcp-Session-Id` value an outgoing request should carry, if any.
    ///
    /// The `v1-compat` half: the stored session id.
    ///
    /// # Why this takes `&StreamableHttpTransportConfig`, not `&self`
    ///
    /// So the caller can read it INSIDE the lock scope it already holds. The
    /// first version of this pair took `&self` and acquired its own
    /// `self.config.read()` after the caller's guard had dropped, which silently
    /// turned one atomic snapshot of `extra_headers` / `auth_provider` /
    /// `http_middleware_chain` / `session_id` into two — a concurrent
    /// [`Self::set_session_id`] landing between the acquisitions would have built
    /// a request from a MIX of two config states. Benign while only `session_id`
    /// moved, and a real tear on a struct this crate explicitly documents as
    /// runtime-mutable. Taking the guard's `&Config` makes a second acquisition
    /// unrepresentable.
    #[cfg(feature = "v1-compat")]
    fn outbound_session_from(config: &StreamableHttpTransportConfig) -> Option<String> {
        config.session_id.clone()
    }

    /// The null twin: a `full-v2` build never stores a session id, so the
    /// answer is the constant `None`.
    ///
    /// Do NOT "improve" this by reading the config — the field does not exist
    /// on this build. Answering here is what keeps the request-building
    /// pipeline free of a `#[cfg]` at its call site.
    #[cfg(not(feature = "v1-compat"))]
    const fn outbound_session_from(_config: &StreamableHttpTransportConfig) -> Option<String> {
        None
    }

    /// Store the `Mcp-Session-Id` a response carried, if this build has
    /// sessions at all.
    ///
    /// The `v1-compat` half. NEVER on v2 (T-113-06): there is no session in
    /// `2026-07-28`, so a response that carries `Mcp-Session-Id` anyway (a
    /// misconfigured intermediary, a dual-stack server echoing v1 state) must
    /// not be able to plant one that the outbound path would then have to
    /// suppress. Refusing to STORE it is the belt to the outbound braces.
    #[cfg(feature = "v1-compat")]
    fn capture_session_header(parts: &RequestParts<'_>, headers: &hyper::HeaderMap) {
        if parts.is_v2() {
            return;
        }
        if let Some(value) = headers.get(MCP_SESSION_ID) {
            if let Ok(text) = value.to_str() {
                // Compare under the READ lock first. After the first response of
                // a session the value is identical every time, and this is the
                // same lock `build_request_from_parts` read-holds for every
                // outgoing request — so an unconditional write makes a writer
                // contend with the request path once per response, to store a
                // value that is already there.
                if parts.config.read().session_id.as_deref() == Some(text) {
                    return;
                }
                parts.config.write().session_id = Some(text.to_string());
            }
        }
    }

    /// The null twin: a `full-v2` build has nowhere to store a session id, so
    /// it does not look for one.
    ///
    /// The v1 half's `is_v2()` guard is a RUNTIME belt over a v1-shaped store;
    /// here the store itself is gone, so the suppression is structural. Do NOT
    /// "improve" this by inspecting `headers` to log an ignored session id.
    #[cfg(not(feature = "v1-compat"))]
    const fn capture_session_header(_parts: &RequestParts<'_>, _headers: &hyper::HeaderMap) {}

    /// Terminate the HTTP session this transport established, if any.
    ///
    /// The `v1-compat` half: when a session id is stored, DELETE the endpoint
    /// and clear it.
    #[cfg(feature = "v1-compat")]
    async fn terminate_session(&self) -> Result<()> {
        // ONE read guard yields both facts. Two separate acquisitions (a
        // presence check, then a `url` read) let a concurrent `set_session_id`
        // land between them and build the DELETE from two config states — the
        // torn-snapshot shape WR-15 declared unrepresentable for
        // `build_request_with_middleware`. Testing `is_some()` in place also
        // drops the `String` clone `session_id()` would allocate and discard.
        let Some(url) = ({
            let config = self.config.read();
            config.session_id.is_some().then(|| config.url.clone())
        }) else {
            return Ok(());
        };
        let request = self
            .build_request_with_middleware(Method::DELETE, url.as_str(), vec![])
            .await?;

        // Send DELETE request (ignore 405 as per spec)
        let response = self.client.request(request).await;
        if let Ok(resp) = response {
            if !resp.status().is_success() && resp.status() != StatusCode::METHOD_NOT_ALLOWED {
                // Log error but don't fail close operation
                tracing::warn!("Failed to terminate session: {}", resp.status());
            }
        }

        // Clear session ID
        self.config.write().session_id = None;
        Ok(())
    }

    /// The null twin: a `full-v2` build never established a session, so there
    /// is nothing to terminate.
    ///
    /// This is the load-bearing half of T-117-55. The severed build has NO
    /// DELETE construction site at all — not a runtime `if` that is always
    /// false: nothing here names [`Method::DELETE`], builds a request, or
    /// touches `self.client`. A teardown for a session that never existed is
    /// not something this build can emit.
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self, clippy::unused_async)]
    async fn terminate_session(&self) -> Result<()> {
        Ok(())
    }

    /// Get the protocol version
    pub fn protocol_version(&self) -> Option<String> {
        self.protocol_version.read().clone()
    }

    /// Set the protocol version (called after initialization)
    pub fn set_protocol_version(&self, version: Option<String>) {
        *self.protocol_version.write() = version;
    }

    /// Get the last event ID (for resumability)
    pub fn last_event_id(&self) -> Option<String> {
        self.last_event_id.read().clone()
    }

    /// Open the GET session stream and read it INCREMENTALLY, frame at a time.
    ///
    /// Returns as soon as the reader task is spawned. Delivered events reach
    /// [`Transport::receive`] while the stream is still open; a stream that ends
    /// — cleanly or otherwise — is reported through the taxonomy documented on
    /// that method.
    ///
    /// # Response-BODY middleware is bypassed on this stream
    ///
    /// The HTTP response-middleware chain does NOT run over the session stream's
    /// body, for the same reason [`Self::post_streaming`] gives for the
    /// `subscriptions/listen` body: the chain processes a complete `Vec<u8>`
    /// body, and a stream has none by construction. Request middleware and
    /// response-HEADER processing are unaffected. A deployment with a
    /// body-rewriting middleware will see it applied to POST responses and not to
    /// this stream.
    ///
    /// # Bound
    ///
    /// The parser is built at this transport's `max_collected_body_bytes` (D-02):
    /// the collected-body cap BECOMES the in-flight parser bound rather than a
    /// second knob, so [`Self::with_max_collected_body_bytes`] moves both. There
    /// is deliberately no new field on [`StreamableHttpTransportConfig`] — that
    /// struct is externally constructible with all-`pub` fields, so a new field
    /// is a MAJOR semver event.
    ///
    /// # The cursor argument is v1-only
    ///
    /// The parameter keeps the same POSITION and TYPE on both feature sets, and
    /// since Phase 118.2 it also keeps the same NAME: it is passed straight
    /// through to [`Self::build_sse_get_request`], which is where the one gated
    /// read of it lives. On a `full-v2` build that function discards it — MCP
    /// `2026-07-28` removed SSE resumability, so there is nothing to resume from
    /// and the GET carries no `Last-Event-ID`; the constant does not even exist
    /// on that build.
    ///
    /// # Reconnect (D-03)
    ///
    /// The spawned reader does not merely read one body. When a stream is
    /// DROPPED — an end-of-body, or a connection failure mid-body, with no
    /// corruption observed — it re-opens the GET under a bounded retry budget,
    /// carrying the resumption cursor. See
    /// [`SseReaderContext::run_session_stream`] for the loop and for what is
    /// deliberately NOT retried.
    pub async fn start_sse(&self, cursor: Option<String>) -> Result<()> {
        // Abort any existing SSE stream
        let handle = self.abort_handle.write().take();
        if let Some(handle) = handle {
            handle.abort();
        }

        // The OWNED context the reader task keeps. Built before the first open so
        // the initial GET and every reconnect go through the same code.
        let context = self.sse_reader_context();

        // `None` means the server answered 405: it does not offer a GET stream at
        // all, which the spec makes an ordinary answer rather than an error.
        let Some(body) = context.open_sse_once(cursor).await? else {
            return Ok(());
        };

        // The HTTP response-BODY middleware chain is deliberately NOT run over
        // this stream, exactly as `Self::post_streaming` states for the
        // `subscriptions/listen` body: "the chain processes a complete `Vec<u8>`
        // body, and a stream has none by construction". `process_headers_from`
        // inside `open_sse_once` still runs, so HEADER-level middleware behaviour
        // is unchanged.
        //
        // Rejected alternatives, recorded so they are not re-derived: prohibiting
        // body-transforming middleware (unenforceable — the chain is a public
        // extension point) and a frame-aware middleware API (new public surface,
        // and a semver event this phase does not plan for).
        //
        // The session stream is the FIRST of this transport's two SSE read sites;
        // the second is the POST response in `Self::post_body`. Both read through
        // `read_sse_body`, so there is one reader shape, one parser bound, and one
        // corrupt-frame story rather than two that can drift. Only THIS one
        // reconnects: a POST response stream that ends is the call's answer
        // arriving, not a stream to restore.
        let handle = tokio::spawn(async move { context.run_session_stream(body).await });

        // ONLY the session stream is tracked here: `close()` and the next
        // `start_sse` abort whatever this slot holds, and a POST reader parked in
        // it would be torn down by an unrelated re-open.
        *self.abort_handle.write() = Some(handle);
        Ok(())
    }

    /// Build the GET that opens a `text/event-stream` session stream.
    ///
    /// The ONE place this transport constructs that request. [`Self::start_sse`]
    /// reaches it through [`SseReaderContext::open_sse_once`], and so does every
    /// reconnect, so the initial open and each re-open carry the same auth
    /// header, the same middleware chain and the same `Accept`.
    ///
    /// # The cursor argument is v1-only
    ///
    /// The parameter keeps the same POSITION and TYPE on both feature sets so
    /// every caller compiles unchanged, but only the `v1-compat` build names it
    /// `resumption_token` and reads it. On a `full-v2` build it is
    /// `_ignored_cursor`, and the GET this builds carries no `Last-Event-ID` —
    /// the constant does not even exist on that build.
    async fn build_sse_get_request(
        parts: &RequestParts<'_>,
        #[cfg(feature = "v1-compat")] resumption_token: Option<String>,
        #[cfg(not(feature = "v1-compat"))] _ignored_cursor: Option<String>,
    ) -> Result<Request<Full<Bytes>>> {
        let url = parts.config.read().url.clone();

        // Build GET request with middleware integration
        let mut request = Self::build_request_from_parts(
            parts,
            Method::GET,
            url.as_str(),
            vec![], // Empty body for GET
        )
        .await?;

        // Add SSE-specific headers
        request.headers_mut().insert(
            ACCEPT,
            TEXT_EVENT_STREAM.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {e}"
                )))
            })?,
        );

        // Add the SSE resumption cursor, on the builds that have one.
        //
        // Why: this is the ONLY `#[cfg]` at a CALL SITE in this file. It is
        // unavoidable because the argument it reads is itself gated (see this
        // function's doc): on `full-v2` the parameter is `_ignored_cursor` and
        // `apply_resumption_header` does not exist. Every other v1 read on this
        // transport goes through a paired accessor with a constant `full-v2`
        // answer instead — including the RECONNECT cursor
        // ([`SseReaderContext::reconnect_cursor`]), which was added in Phase
        // 118.2 specifically so that a second gate did NOT accumulate here. Do
        // not let one accumulate now; `tests/v1_severability_tripwire.rs` counts
        // them.
        #[cfg(feature = "v1-compat")]
        Self::apply_resumption_header(&mut request, resumption_token.as_deref())?;

        Ok(request)
    }

    /// The OWNED state a spawned session-stream reader carries.
    ///
    /// Every field is a cheap clone of an already-shared value; the hyper
    /// `Client` clone reuses the connection POOL rather than opening a second
    /// one. `abort_handle` is deliberately absent — see [`SseReaderContext`].
    fn sse_reader_context(&self) -> SseReaderContext {
        SseReaderContext {
            client: self.client.clone(),
            config: Arc::clone(&self.config),
            protocol_version: Arc::clone(&self.protocol_version),
            v2_mode: Arc::clone(&self.v2_mode),
            sender: self.sender.clone(),
            last_event_id: Arc::clone(&self.last_event_id),
            // Read ONCE, here, through the paired accessor, so the spawned task
            // itself carries no `#[cfg]` at all.
            on_resumption: self.resumption_callback(),
            max_collected_body_bytes: self.max_collected_body_bytes,
        }
    }

    /// Spawn the incremental SSE reader over ONE live POST response body, and
    /// hand back its join handle.
    ///
    /// The POST half of this transport's two `text/event-stream` sites. Both
    /// halves read through [`read_sse_body`], so the parser bound (D-02, the
    /// transport's `max_collected_body_bytes`), the resumption-cursor write, the
    /// message-event filter, the corruption taxonomy (D-02/D-05) and the
    /// await-capacity delivery policy (D-04) are shared rather than duplicated.
    ///
    /// # This site does NOT reconnect, and that is the point
    ///
    /// A POST response stream that ends is the call's ANSWER arriving; there is
    /// nothing to restore, and re-issuing the GET would not restore it anyway.
    /// Reconnect belongs to the session stream alone
    /// ([`SseReaderContext::run_session_stream`]). What this site does instead is
    /// exactly what plan 01's taxonomy prescribes: an ordinary end-of-body is
    /// silent, and a transport failure mid-body is delivered once as
    /// [`TransportError::Request`].
    ///
    /// The reader RETURNS when a send fails, which is what happens the moment the
    /// last transport clone drops the receive queue's `Receiver`. That, not a
    /// `Drop` impl, is what stops a reader outliving its transport: the transport
    /// is `Clone` and shares its abort handle, so one clone's drop would kill the
    /// original's stream. See [`Transport::receive`] rule 3.
    fn spawn_sse_reader(&self, body: hyper::body::Incoming) -> tokio::task::JoinHandle<()> {
        let sender = self.sender.clone();
        let on_resumption = self.resumption_callback();
        let last_event_id = Arc::clone(&self.last_event_id);
        let max_buffer_size = self.max_collected_body_bytes;

        tokio::spawn(async move {
            let end = read_sse_body(
                &sender,
                &last_event_id,
                on_resumption.as_ref(),
                body,
                max_buffer_size,
            )
            .await;
            // Terminal, and latched by returning: at most ONE error per reader,
            // so one failed stream cannot become an error storm.
            if let SseBodyEnd::Dropped {
                cause: Some(error), ..
            } = end
            {
                let _ = sender.send(Err(error)).await;
            }
        })
    }

    /// Emit the two v2-only routing headers onto an outbound request builder
    /// (Phase 113, CLNT-01 / VERS-05).
    ///
    /// Mirrors the SERVER-side emitter
    /// (`streamable_http_server.rs::apply_v2_outbound_headers`): every insert
    /// goes through `HeaderValue::from_str` and a pathological value SKIPS its
    /// header rather than panicking (T-113-20).
    ///
    /// `MCP-Protocol-Version` is emitted by the existing per-request block, not
    /// here, so a v1 request keeps exactly the headers it has today.
    fn apply_v2_outbound_headers(
        mut builder: hyper::http::request::Builder,
        method: &str,
        name: &str,
    ) -> hyper::http::request::Builder {
        if let Ok(value) = hyper::header::HeaderValue::from_str(method) {
            builder = builder.header(MCP_METHOD, value);
        }
        // `Mcp-Name` is emitted on EVERY v2 request, with the EMPTY STRING for a
        // method that carries no routing name. The value is resolved through
        // `name_bearing_key`, so it is the task id for `tasks/get` /
        // `tasks/update` / `tasks/cancel`, `params.name` for `tools/call` /
        // `prompts/get` and `params.uri` for `resources/read`.
        //
        // Emitting it unconditionally is a SUPERSET of what the spec requires.
        // Since Phase 118 D-13 the server requires the header only on
        // name-bearing methods and DISCARDS a value sent on any other, so the
        // empty string a name-less method carries is accepted and ignored — this
        // client keeps working against both the pre- and post-D-13 rule. Phase
        // 118 D-18 pointed the server's predicate at this same
        // `name_bearing_key` table, so the value emitted here for a `tasks/*`
        // method is now cross-checked against the body rather than ignored.
        if let Ok(value) = hyper::header::HeaderValue::from_str(name) {
            builder = builder.header(MCP_NAME, value);
        }
        builder
    }

    /// A borrowed view of everything an outgoing request is built from.
    ///
    /// See [`RequestParts`] for why the builder underneath takes this rather
    /// than `&self`.
    fn request_parts(&self) -> RequestParts<'_> {
        RequestParts {
            config: &self.config,
            protocol_version: &self.protocol_version,
            v2_mode: &self.v2_mode,
        }
    }

    /// Build a `hyper::Request` with middleware integration.
    ///
    /// This method:
    /// 1. Builds initial request with config headers, auth, session, protocol version
    /// 2. Runs HTTP middleware on the request
    /// 3. Returns the modified `hyper::Request` ready to send
    ///
    /// A thin `&self` wrapper over [`Self::build_request_from_parts`], kept so
    /// every existing caller is unchanged; the reconnect task reaches the same
    /// body through [`SseReaderContext`]'s own [`RequestParts`].
    async fn build_request_with_middleware(
        &self,
        method: Method,
        url: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        Self::build_request_from_parts(&self.request_parts(), method, url, body).await
    }

    /// The ONE request builder on this transport (Phase 118.2, T-118.2-04-07).
    ///
    /// Both the caller-task path ([`Self::build_request_with_middleware`]) and
    /// the spawned reconnect path ([`SseReaderContext::open_sse_once`]) come
    /// here, so the auth header, the session header, the protocol-version
    /// header, the v2 routing headers and the request-middleware chain cannot
    /// diverge between an initial open and a re-open.
    async fn build_request_from_parts(
        parts: &RequestParts<'_>,
        method: Method,
        url: &str,
        body: Vec<u8>,
    ) -> Result<Request<Full<Bytes>>> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpRequest};

        // Extract config data — ONE snapshot, under ONE lock acquisition.
        //
        // `outbound_session` is read through the paired accessor rather than off
        // the config directly (on a `full-v2` build there is no field to read and
        // the twin answers the constant `None`), but INSIDE this scope: a second
        // acquisition after the guard dropped would let a concurrent
        // `set_session_id` produce a request built from two different config
        // states. See `outbound_session_from`.
        let (extra_headers, auth_provider, middleware_chain, outbound_session) = {
            let config = parts.config.read();
            (
                config.extra_headers.clone(),
                config.auth_provider.clone(),
                config.http_middleware_chain.clone(),
                Self::outbound_session_from(&config),
            )
        };

        // Start building request with hyper
        let mut request_builder = Request::builder().method(method.clone()).uri(url);

        // Add extra headers from config
        for (key, value) in &extra_headers {
            request_builder = request_builder.header(key.as_str(), value.as_str());
        }

        // Add auth header if provider is present (highest priority)
        let has_auth = if let Some(auth_provider) = auth_provider {
            let token = auth_provider.get_access_token().await?;
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
            true
        } else {
            false
        };

        let is_v2 = parts.is_v2();

        // Add session ID header if we have one.
        //
        // NEVER on v2: `2026-07-28` has no session at all, and a session id
        // surviving into the v2 path is exactly the identity-collapse failure
        // class HTTP-01/HTTP-05 exist to close (T-113-06). The suppression is
        // unconditional — a value left over from a v1 exchange on the same
        // transport must not leak either.
        if let Some(session) = &outbound_session {
            if !is_v2 {
                request_builder = request_builder.header(MCP_SESSION_ID, session.as_str());
            }
        }

        // Add protocol version header if we have one
        if let Some(protocol_version) = parts.protocol_version.read().as_ref() {
            request_builder =
                request_builder.header(MCP_PROTOCOL_VERSION, protocol_version.as_str());
        }

        // v2 routing headers, DERIVED from the body this request is about to
        // carry — the same `logical_name_key` table the server's
        // `extract_body_method_and_name` reads. Deriving them here (rather than
        // threading a name down from `Client`) is what makes the header and the
        // body incapable of desyncing (T-113-08). A body with no `method` (a
        // JSON-RPC response, or the empty GET/SSE body) yields `None` and emits
        // neither header.
        if is_v2 {
            if let Some((method, name)) = v2_routing_headers(&body) {
                request_builder = Self::apply_v2_outbound_headers(request_builder, &method, &name);
            }
        }

        // Build temporary request to extract headers for middleware
        let temp_req = request_builder
            .body(Full::new(Bytes::from(body.clone())))
            .map_err(|e| Error::Transport(TransportError::InvalidMessage(e.to_string())))?;

        // Extract headers from temp request
        let headers = temp_req.headers();

        // Run HTTP middleware if configured
        if let Some(chain) = middleware_chain {
            // Create HttpRequest from hyper components
            let mut http_req = HttpRequest::new(method.as_str().to_string(), url.to_string(), body);

            // Copy headers
            for (key, value) in headers {
                if let Ok(value_str) = value.to_str() {
                    http_req.add_header(key.as_str(), value_str);
                }
            }

            // Create context
            let context = HttpMiddlewareContext::new(url.to_string(), method.as_str().to_string());

            // Set metadata if auth was already set by transport
            if has_auth {
                context.set_metadata("auth_already_set".to_string(), "true".to_string());
            }

            // Run middleware chain
            if let Err(e) = chain.process_request(&mut http_req, &context).await {
                // Call error handlers
                chain.handle_transport_error(&e, &context).await;
                return Err(e);
            }

            // Rebuild request with modified headers and body
            let mut final_builder = Request::builder().method(method).uri(url);

            for (key, value) in &http_req.headers {
                final_builder = final_builder.header(key, value);
            }

            final_builder
                .body(Full::new(Bytes::from(http_req.body)))
                .map_err(|e| Error::Transport(TransportError::InvalidMessage(e.to_string())))
        } else {
            // No middleware - return original request
            Ok(temp_req)
        }
    }

    /// Apply HTTP middleware to a response after receiving.
    #[allow(clippy::future_not_send)]
    async fn apply_response_middleware(
        &self,
        method: &str,
        url: &str,
        response: &HyperResponse<impl hyper::body::Body>,
        body: Vec<u8>,
    ) -> Result<Vec<u8>> {
        use crate::client::http_middleware::{HttpMiddlewareContext, HttpResponse};

        let middleware_chain = self.config.read().http_middleware_chain.clone();
        if let Some(chain) = middleware_chain {
            // Create HttpResponse from hyper components
            let header_map = response.headers().clone();

            let mut http_resp =
                HttpResponse::with_headers(response.status().as_u16(), header_map, body);

            // Create context
            let context = HttpMiddlewareContext::new(url.to_string(), method.to_string());

            // Run middleware chain
            if let Err(e) = chain.process_response(&mut http_resp, &context).await {
                // Call error handlers
                chain.handle_transport_error(&e, &context).await;
                return Err(e);
            }

            // Return modified body
            Ok(http_resp.body)
        } else {
            // No middleware - return original body
            Ok(body)
        }
    }

    /// Process response headers and extract session/protocol information
    ///
    /// A thin `&self` wrapper over [`Self::process_headers_from`], so every
    /// existing caller is unchanged while the spawned reconnect task reaches the
    /// same body through [`SseReaderContext`]'s [`RequestParts`]. A reconnect
    /// that skipped this would stop tracking a protocol version the server
    /// re-states on the re-opened stream.
    fn process_response_headers(&self, response: &HyperResponse<impl hyper::body::Body>) {
        Self::process_headers_from(&self.request_parts(), response.headers());
    }

    /// The ONE response-header processor on this transport.
    fn process_headers_from(parts: &RequestParts<'_>, headers: &hyper::HeaderMap) {
        // Update session ID from response header, on the builds that have a
        // session to update. See `Self::capture_session_header` for why the v1
        // half still carries a runtime `is_v2()` guard and the `full-v2` twin
        // needs none.
        Self::capture_session_header(parts, headers);

        // Update protocol version from response header
        if let Some(protocol_version) = headers.get(MCP_PROTOCOL_VERSION) {
            if let Ok(protocol_version_str) = protocol_version.to_str() {
                *parts.protocol_version.write() = Some(protocol_version_str.to_string());
            }
        }
    }

    /// Send a message with options (hyper-based with middleware)
    pub async fn send_with_options(
        &mut self,
        message: TransportMessage,
        options: SendOptions,
    ) -> Result<()> {
        // If we have a resumption cursor, restart the SSE stream.
        //
        // Read through `SendOptions::resumption_cursor` rather than the field:
        // on a `full-v2` build the field does not exist and the accessor is the
        // constant `None`, so this branch is dead code the optimiser removes
        // rather than a `#[cfg]` wedged into the send path.
        if let Some(token) = options.resumption_cursor() {
            self.start_sse(Some(token)).await?;
            return Ok(());
        }

        // Classify BEFORE serializing: the 202 handler needs the notification's
        // identity, and the typed value is where it lives. See `OutboundFrame`.
        let outbound = OutboundFrame::of(&message);

        // Use JSON-RPC compatibility layer for serialization
        let body_bytes = crate::shared::StdioTransport::serialize_message(&message)?;
        self.post_body(body_bytes, outbound).await
    }

    /// Read the JSON-RPC ERROR envelope out of a non-2xx response body (D-113-E).
    ///
    /// Returns `Some(TransportMessage::Response)` only when the body is a
    /// well-formed JSON-RPC 2.0 frame carrying an `error` member — i.e. the
    /// server deliberately answered with a structured protocol error that plan
    /// 04 mapped onto a 4xx status. A proxy's HTML error page, a bare
    /// `{"message":"..."}`, or a `result`-carrying frame all return `None`, so
    /// the caller falls back to the status-only transport error.
    ///
    /// Deliberately strict about `jsonrpc == "2.0"` **and** the presence of
    /// `error`: an intermediary's JSON error document must never be laundered
    /// into what a caller reads as a server-authored protocol error.
    async fn jsonrpc_error_envelope(
        response: HyperResponse<hyper::body::Incoming>,
        max_collected_body_bytes: usize,
    ) -> Option<TransportMessage> {
        // The THIRD whole-body read on this transport, capped for the same reason
        // as the other two (T-113-84). An error envelope is still a
        // peer-controlled body, and an over-cap one is simply not an envelope:
        // `None` here falls back to the status-only transport error, which is
        // exactly what a malformed body already did.
        let body = Self::collect_body_within_cap(response, max_collected_body_bytes)
            .await
            .ok()?;
        let value = serde_json::from_slice::<serde_json::Value>(&body).ok()?;
        if value.get("jsonrpc").and_then(serde_json::Value::as_str) != Some("2.0")
            || value.get("error").is_none()
        {
            return None;
        }
        match crate::shared::StdioTransport::parse_message(&body) {
            Ok(message @ TransportMessage::Response(_)) => Some(message),
            _ => None,
        }
    }

    /// Insert the two per-request POST headers every JSON-RPC frame carries.
    ///
    /// Factored out of [`Self::post_once`] so the first attempt and the 401
    /// retry cannot drift: the retry MUST preserve the original method, body and
    /// headers, and the only way to guarantee that is for both to build them
    /// from one place.
    fn apply_post_headers(request: &mut Request<Full<Bytes>>) -> Result<()> {
        request.headers_mut().insert(
            CONTENT_TYPE,
            APPLICATION_JSON.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {}",
                    e
                )))
            })?,
        );
        request.headers_mut().insert(
            ACCEPT,
            ACCEPT_STREAMABLE.parse().map_err(|e| {
                Error::Transport(TransportError::InvalidMessage(format!(
                    "Invalid header: {}",
                    e
                )))
            })?,
        );
        Ok(())
    }

    /// Build, send, and (on a `401` with a configured auth provider) retry ONCE
    /// an already-serialized POST body, returning the raw HTTP response with its
    /// body UNREAD.
    ///
    /// The shared head of [`Self::post_body`] and [`Self::post_streaming`].
    /// Extracted because a long-lived `subscriptions/listen` stream (HTTP-04)
    /// must go through the SAME header emission and the SAME single-shot 401
    /// refresh as every other request — a second, hand-rolled POST path would
    /// silently miss the auth retry.
    ///
    /// The retry is structurally at-most-once: it returns directly from this
    /// function, so there is no loop to go around twice. A second `401` on the
    /// retry is returned to the caller unchanged.
    async fn post_once(&self, body_bytes: Vec<u8>) -> Result<HyperResponse<hyper::body::Incoming>> {
        // Clone body_bytes so we can retry with the identical payload on 401.
        let body_bytes_snapshot = body_bytes.clone();

        let url = self.config.read().url.clone();

        // Build POST request with middleware integration
        let mut request = self
            .build_request_with_middleware(Method::POST, url.as_str(), body_bytes)
            .await?;
        Self::apply_post_headers(&mut request)?;

        // Send first attempt.
        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }

        // No auth provider — cannot retry; return the 401 as-is.
        let auth_provider = self.config.read().auth_provider.clone();
        let Some(provider) = auth_provider else {
            return Ok(response);
        };

        // Step 1: purge cached token (on_unauthorized BEFORE get_access_token — Test 5).
        provider.on_unauthorized().await?;

        // Step 2: rebuild the request using the byte-identical body snapshot.
        let mut retry_request = self
            .build_request_with_middleware(Method::POST, url.as_str(), body_bytes_snapshot)
            .await?;
        Self::apply_post_headers(&mut retry_request)?;

        // Step 3: send retry — do NOT retry again on a second 401.
        self.client
            .request(retry_request)
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))
    }

    /// POST an already-serialized frame and hand back the response with its body
    /// STILL UNREAD, so a long-lived `text/event-stream` can be consumed
    /// incrementally (HTTP-04, `subscriptions/listen`).
    ///
    /// [`Self::post_body`] collects the body to completion, which would hang
    /// forever on a stream that never ends. This is the streaming sibling: same
    /// request construction, same 401 retry, same response-header processing —
    /// and then the caller owns the body.
    ///
    /// The HTTP response-middleware chain is deliberately NOT run here, for the
    /// same reason the server side does not run its own over a listen stream:
    /// the chain processes a complete `Vec<u8>` body, and a stream has none by
    /// construction.
    pub(crate) async fn post_streaming(
        &self,
        body_bytes: Vec<u8>,
    ) -> Result<HyperResponse<hyper::body::Incoming>> {
        let response = self.post_once(body_bytes).await?;
        self.process_response_headers(&response);
        Ok(response)
    }

    /// POST an ALREADY-SERIALIZED JSON-RPC frame.
    ///
    /// The shared tail of [`Self::send_with_options`] and
    /// [`Transport::send_raw`]: the v2 client path assembles its own frame (so
    /// it can stamp `params._meta` on methods whose typed struct has no `_meta`
    /// field), and both paths must go through the SAME header emission, 401
    /// retry, and response handling.
    ///
    /// `outbound` only selects the 202-Accepted behavior; it is not re-derived
    /// from the bytes so the typed path keeps its exact semantics.
    ///
    /// # A `text/event-stream` answer is a STREAM, not a body
    ///
    /// When the response's `Content-Type` is `text/event-stream` this method
    /// hands the LIVE body to [`Self::spawn_sse_reader`] and returns as soon as
    /// the stream is open, rather than collecting the body first. Such a response
    /// stays open for the whole call and can carry notifications **and
    /// server-to-client requests** before its result frame; collecting it whole
    /// would deliver every one of them only after the call ended, and an in-tool
    /// elicitation over a POST stream would deadlock outright — the client cannot
    /// answer a request it has not parsed yet (Phase 118.2, D-01).
    ///
    /// On that path the HTTP response-**BODY** middleware chain is deliberately
    /// NOT run, exactly as [`Self::post_streaming`] states for the
    /// `subscriptions/listen` body: the chain processes a complete `Vec<u8>`
    /// body, and a stream has none by construction.
    /// [`Self::process_response_headers`] still runs, so HEADER-level middleware
    /// behaviour is unchanged. A deployment with a body-rewriting middleware sees
    /// it applied to JSON POST responses and not to a streaming one.
    async fn post_body(&self, body_bytes: Vec<u8>, outbound: OutboundFrame) -> Result<()> {
        let response = self.post_once(body_bytes).await?;

        // Process headers for session and protocol info
        self.process_response_headers(&response);

        // 202 Accepted — the notification acknowledgement, handled BEFORE the
        // non-2xx guard (Phase 118.2, Defect A).
        //
        // Why the branch moved: `202` satisfies `is_success()` — `http`'s own
        // `StatusCode::is_success` is `(200..300).contains(..)` — so the 202
        // sub-branch that used to live inside `if !response.status().is_success()`
        // was dead code from the day it was written. MEASURED in Phase 118.2
        // research: a real `ClientBuilder` handshake against a recording listener
        // produced two POSTs and ZERO GETs, i.e. the session stream was never
        // opened at all. Fenced by `tests/client_sse_stream.rs`.
        if response.status() == StatusCode::ACCEPTED {
            // The guard is the `notifications/initialized` notification
            // SPECIFICALLY, carried as a typed identity rather than as the
            // method string — see `OutboundFrame` for why the broad
            // "is a notification" predicate is an active regression.
            if outbound == OutboundFrame::InitializedNotification {
                // Tolerate failure: a server answering `405 Method Not Allowed`
                // to the GET is a perfectly valid StreamableHTTP server that
                // simply offers no session stream. That tolerance was in the
                // dead branch and is preserved here.
                let _ = self.start_sse(None).await;
            }
            return Ok(());
        }

        // Handle non-success responses
        if !response.status().is_success() {
            // D-113-E: on v2 a STRUCTURED JSON-RPC error rides a 4xx.
            //
            // Phase-113 plan 04 maps the v2 error codes onto HTTP statuses
            // (`-32601` at 404; `-32020`/`-32021`/`-32022`/`-32602` at 400), so
            // erroring on the status alone discards the very `error.code` the
            // caller has to dispatch on — an MRTR retry loop cannot tell an
            // expired-token `-32602` from a transport fault, and plan 09's
            // `-32021 MissingRequiredClientCapability` becomes unactionable.
            // When the body IS a JSON-RPC error envelope, feed it through the
            // normal response channel so it surfaces as `Error::Protocol`.
            //
            // v2 ONLY: v1's behavior is byte-identical to every prior release.
            if self.is_v2() {
                let status = response.status();
                match Self::jsonrpc_error_envelope(response, self.max_collected_body_bytes).await {
                    Some(message) => {
                        tracing::debug!(
                            %status,
                            "v2 non-2xx carried a JSON-RPC error envelope — surfacing it structurally"
                        );
                        // A CALLER-task send: non-blocking, loud on a full queue.
                        // See `Self::queue_from_caller`.
                        self.queue_from_caller(message)?;
                        return Ok(());
                    },
                    None => {
                        return Err(Error::Transport(TransportError::Request(format!(
                            "Request failed with status: {}",
                            status
                        ))));
                    },
                }
            }

            return Err(Error::Transport(TransportError::Request(format!(
                "Request failed with status: {}",
                response.status()
            ))));
        }

        // Get response metadata before consuming the response
        let status_code = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        // The SECOND SSE read site, decided BEFORE the collect below because
        // there is nothing here to collect: a `text/event-stream` POST response
        // stays open for the whole call and its body ends only when the server
        // has finished answering. Awaiting that end is what made a
        // server-to-client request unanswerable (Phase 118.2, D-01) — see this
        // method's rustdoc, including why the response-BODY middleware chain does
        // not run on this path and `Self::post_streaming` is the precedent.
        //
        // The reader is DETACHED rather than stored in `self.abort_handle`: that
        // slot belongs to the GET session stream, and parking a POST reader in it
        // would let the next `start_sse` tear down a call's own stream. Its
        // lifetime is bounded by the send-failure rule instead — see
        // `Self::spawn_sse_reader`.
        if content_type.contains(TEXT_EVENT_STREAM) {
            drop(self.spawn_sse_reader(response.into_body()));
            return Ok(());
        }

        // Collect response body under this transport's collected-body cap
        // (T-113-84).
        //
        // Every remaining branch parses a COMPLETE body, so this is the only
        // thing bounding the allocation on those paths. See
        // `DEFAULT_MAX_COLLECTED_BODY_BYTES`.
        let body_bytes =
            Self::collect_body_within_cap(response, self.max_collected_body_bytes).await?;

        // Debug logging for response diagnostics
        tracing::debug!(
            status = %status_code,
            content_type = %content_type,
            content_length = ?content_length,
            body_len = body_bytes.len(),
            "HTTP response received"
        );

        // Fast path: Check if middleware exists before creating temp response.
        // The URL is read from the SAME acquisition rather than eagerly at entry:
        // `post_once` already owns the request build, so the no-middleware path
        // (the common one) pays no config lock and no `Url` clone here.
        let middleware_url = {
            let config = self.config.read();
            config
                .http_middleware_chain
                .is_some()
                .then(|| config.url.clone())
        };
        let modified_body = if let Some(url) = middleware_url {
            // Run response middleware (create a minimal response for middleware processing)
            let temp_response = HyperResponse::builder()
                .status(status_code)
                .body(Full::new(Bytes::new()))
                .unwrap();
            self.apply_response_middleware(
                "POST",
                url.as_str(),
                &temp_response,
                body_bytes.to_vec(),
            )
            .await?
        } else {
            // No middleware - use body directly (fast path)
            body_bytes.to_vec()
        };

        // If it's a 200 response with Content-Length: 0 or no Content-Type
        if status_code == StatusCode::OK && (content_length == Some(0) || content_type.is_empty()) {
            if modified_body.is_empty() {
                // Empty 200 response (e.g., for notifications) - just return Ok
                return Ok(());
            }

            // If there was a body but no content-type, that's an error
            if content_type.is_empty() {
                return Err(Error::Transport(TransportError::Request(
                    "Response has body but no Content-Type header".to_string(),
                )));
            }

            // We have a body with content, parse it as JSON
            // Try to parse as array first (batch response - JSON-RPC 2.0)
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&modified_body) {
                for json_msg in batch {
                    let json_str = serde_json::to_string(&json_msg).map_err(|e| {
                        Error::Transport(TransportError::Deserialization(e.to_string()))
                    })?;
                    // Use JSON-RPC compatibility layer
                    let msg = crate::shared::StdioTransport::parse_message(json_str.as_bytes())?;
                    // A CALLER-task send. See `Self::queue_from_caller`.
                    self.queue_from_caller(msg)?;
                }
            } else {
                // Single message - use JSON-RPC compatibility layer
                let msg_parsed = crate::shared::StdioTransport::parse_message(&modified_body)?;
                // A CALLER-task send. See `Self::queue_from_caller`.
                self.queue_from_caller(msg_parsed)?;
            }
            return Ok(());
        }

        if content_type.contains(APPLICATION_JSON) {
            // Check for empty body with JSON content type
            // Note: 202 Accepted with empty body is valid for notification acknowledgments
            if modified_body.is_empty() {
                if status_code == StatusCode::ACCEPTED {
                    // 202 Accepted with empty body is valid (notification acknowledged)
                    tracing::debug!(
                        status = %status_code,
                        "Notification acknowledged with 202 Accepted"
                    );
                    return Ok(());
                }

                // For other 2xx statuses, empty body with application/json is an error
                tracing::warn!(
                    status = %status_code,
                    content_type = %content_type,
                    "Server returned empty body with application/json content type"
                );
                return Err(Error::Transport(TransportError::Request(
                    "Server returned empty response body with Content-Type: application/json. \
                     This may indicate a server error or network issue."
                        .to_string(),
                )));
            }

            // JSON response (single or batch)
            // Try to parse as array first (batch response - JSON-RPC 2.0)
            if let Ok(batch) = serde_json::from_slice::<Vec<serde_json::Value>>(&modified_body) {
                for json_msg in batch {
                    let json_str = serde_json::to_string(&json_msg).map_err(|e| {
                        Error::Transport(TransportError::Deserialization(e.to_string()))
                    })?;
                    // Use JSON-RPC compatibility layer
                    let msg = crate::shared::StdioTransport::parse_message(json_str.as_bytes())?;
                    // A CALLER-task send. See `Self::queue_from_caller`.
                    self.queue_from_caller(msg)?;
                }
            } else {
                // Single message - use JSON-RPC compatibility layer
                let msg_parsed = crate::shared::StdioTransport::parse_message(&modified_body)?;
                // A CALLER-task send. See `Self::queue_from_caller`.
                self.queue_from_caller(msg_parsed)?;
            }
        } else if status_code == StatusCode::ACCEPTED {
            // 202 Accepted with no body is valid
            return Ok(());
        } else {
            return Err(Error::Transport(TransportError::Request(format!(
                "Unsupported content type: {}",
                content_type
            ))));
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.send_with_options(message, SendOptions::default())
            .await
    }

    /// Take the next server-to-client message, or the reason the stream ended.
    ///
    /// # The five end reasons, and how they differ (Phase 118.2, D-02/D-03/D-05)
    ///
    /// | End reason | What the reader puts on the queue | What this returns |
    /// |---|---|---|
    /// | A message arrived | `Ok(msg)` | `Ok(msg)` |
    /// | Parser overflow (D-02) | `Err(TransportError::InvalidMessage)` naming the parser bound and echoing NO body content; NOT retried | that error, exactly once, after every already-queued message |
    /// | Unparseable frame (D-05) | `Err(TransportError::InvalidMessage)` naming the parse failure, with any echoed frame text truncated to a 200-character bound; NOT retried | as above |
    /// | Reconnect budget exhausted (D-03) | `Err(TransportError::Request)` naming the exhausted budget | as above |
    /// | Ordinary EOF, or a deliberate [`Transport::close`] / last-transport-drop | NOTHING — the reader exits silently | this keeps awaiting, exactly as before |
    ///
    /// Three rules make that taxonomy hold:
    ///
    /// 1. **FIFO is the ordering guarantee.** A terminal `Err` is queued BEHIND
    ///    every message already delivered, so a consumer sees all successfully
    ///    parsed frames before the failure. That is why the reason rides the
    ///    queue rather than a side slot — a slot would have to be polled, and
    ///    the transport holds a `Sender` clone for its whole life, so `recv()`
    ///    never returns `None` on its own.
    /// 2. **At most ONE terminal error per reader.** The reader returns
    ///    immediately after a successful terminal send, so one corrupt frame
    ///    cannot become an error storm.
    /// 3. **A failed send means the receiver is gone**, so the reader RETURNS
    ///    rather than retrying. That is what makes "dropping the last transport
    ///    clone terminates the reader" true without a `Drop` impl — impossible
    ///    here anyway, since the transport is `Clone` and shares its abort
    ///    handle, so one clone's drop would kill the original's stream.
    ///
    /// The public signature is unchanged; only the private queue's element type
    /// moved, which `cargo semver-checks` cannot see.
    async fn receive(&mut self) -> Result<TransportMessage> {
        // Receive from channel - this will block until a message is available
        let mut receiver = self.receiver.lock().await;
        receiver
            .recv()
            .await
            .ok_or_else(|| Error::Transport(TransportError::ConnectionClosed))?
    }

    async fn close(&mut self) -> Result<()> {
        // Abort any running SSE stream
        let handle = self.abort_handle.write().take();
        if let Some(handle) = handle {
            handle.abort();
        }

        // Optionally send a DELETE request to terminate the session.
        //
        // Routed through the paired helper, so the `full-v2` build has no DELETE
        // construction site at all rather than a runtime branch that is always
        // false (T-117-55).
        self.terminate_session().await
    }

    fn is_connected(&self) -> bool {
        // In streamable HTTP, we're always "connected" in the sense that
        // we can make requests. There's no persistent connection.
        true
    }

    fn transport_type(&self) -> &'static str {
        "streamable-http"
    }

    /// Receive the client's per-connection era selection (Phase 113, CLNT-01).
    ///
    /// Delegates to the existing inherent
    /// [`Self::set_protocol_version`], which writes the field the
    /// `MCP-Protocol-Version` request header is emitted from, AND latches the
    /// separate `Self::v2_mode` flag that gates every v2-only behavior. The
    /// two are distinct on purpose — see that field's docs.
    fn set_negotiated_protocol_version(&mut self, version: Option<String>) {
        // Classify through `protocol_era`, the single source of truth, NOT by
        // string equality against the v2 constant. `Client::era()` already goes
        // through it, and the two MUST agree: if a second v2-generation version
        // string is ever added to the classifier, an equality check here would
        // leave the client stamping `_meta` and calling `send_raw` while the
        // transport silently stayed in v1 emission mode — no `Mcp-Method` /
        // `Mcp-Name`, session id leaked back on — and every request would 400
        // with `HEADER_MISMATCH`. Compile-time silent, runtime total.
        let is_v2 = version.as_deref().map(crate::types::protocol::protocol_era)
            == Some(crate::types::protocol::Era::V2);
        self.set_protocol_version(version);
        self.v2_mode.store(is_v2, Ordering::Relaxed);
    }

    /// This transport DOES have a wire representation for the negotiated version
    /// (the `MCP-Protocol-Version` header plus the v2 routing headers).
    fn supports_negotiated_protocol_version(&self) -> bool {
        true
    }

    async fn send_raw(&mut self, body: Vec<u8>) -> Result<()> {
        // The v2 raw path never carries `notifications/initialized` — v2 has no
        // handshake at all — so its 202s never open a session stream.
        self.post_body(body, OutboundFrame::Other).await
    }
}

// ---------------------------------------------------------------------------
// The GET session-stream reader (Phase 118.2, D-01/D-02/D-05).
//
// Lifted from `src/client/subscriptions.rs`'s `subscriptions/listen` reader —
// the same shape, on the same parser, with the same three invariants. There is
// deliberately NOT a second SSE tokenizer, and the four-way split below exists
// specifically to keep every function under the repo's PMAT cog-25 budget
// (CLAUDE.md, enforced by the PR-blocking gate). Do not collapse it.
// ---------------------------------------------------------------------------

/// How much of a malformed frame is echoed back in an error message.
///
/// Bounded because the frame is UNTRUSTED remote input: a hostile server must
/// not be able to push an unbounded string into a client's logs through an error
/// `Display` (ASVS V7, T-118.2-01-05).
const MAX_ECHOED_SSE_FRAME: usize = 200;

/// Incremental UTF-8 + SSE decoding state for one live session stream.
struct SseReadState {
    body: hyper::body::Incoming,
    parser: SseParser,
    /// Bytes received but not yet decodable as complete UTF-8.
    bytes: Vec<u8>,
    /// Events already parsed and waiting to be delivered.
    pending: std::collections::VecDeque<crate::shared::sse_parser::SseEvent>,
    /// The body reported end-of-stream (or errored) and must not be polled again.
    done: bool,
}

impl SseReadState {
    /// Build the state for one response body at an EXPLICIT parser bound.
    ///
    /// The bound is the transport's `max_collected_body_bytes` (D-02), not
    /// `SseParser::new()`'s default, so the limit is visible at the call site and
    /// moves with [`StreamableHttpTransport::with_max_collected_body_bytes`].
    fn new(body: hyper::body::Incoming, max_buffer_size: usize) -> Self {
        Self {
            body,
            parser: SseParser::with_max_buffer_size(max_buffer_size),
            bytes: Vec::new(),
            pending: std::collections::VecDeque::new(),
            done: false,
        }
    }
}

/// Why [`read_next_sse_frame`] stopped, when it stopped (Phase 118.2, D-03).
///
/// The two variants are NOT interchangeable, and the distinction is the whole of
/// T-118.2-04-02: a DROP is a peer or an intermediary going away, which is
/// exactly what a reconnect exists to survive, while CORRUPTION means this byte
/// stream has already lost data. Retrying a stream the peer is actively
/// corrupting is a reconnect storm aimed at a server that is already unwell, and
/// it would replay the same corruption.
enum SseFrameStop {
    /// The body ended in a transport FAILURE mid-flight. Retryable on the
    /// session stream; delivered as-is on the POST stream.
    Dropped(Error),
    /// The parser discarded in-flight bytes past its bound (D-02). NEVER
    /// retried.
    Corrupt(Error),
}

/// How ONE live SSE body ended (Phase 118.2, D-03).
enum SseBodyEnd {
    /// The stream is over with NO corruption observed: either an ordinary
    /// end-of-body (`cause: None`) or a transport failure mid-body
    /// (`cause: Some`).
    ///
    /// `delivered` records whether this body produced at least one event, which
    /// is what lets a long-lived stream that blinks again earn a FRESH reconnect
    /// budget rather than inheriting a spent one. `retry` is the last
    /// server-provided SSE `retry:` value seen on it.
    Dropped {
        cause: Option<Error>,
        delivered: bool,
        retry: Option<Duration>,
    },
    /// The stream ended for a reason that is not a drop: a D-02 overflow, a D-05
    /// parse failure, or a receive queue whose `Receiver` is gone. Any terminal
    /// error has ALREADY been sent. Never retried, and nothing further is sent.
    Ended,
}

/// Read ONE live body to its end, delivering every event it yields.
///
/// The SINGLE reader body for both of this transport's `text/event-stream`
/// sites — the GET session stream and the POST response — so the parser bound,
/// the message-event filter, the corruption taxonomy and the await-capacity
/// delivery policy cannot drift between them. What the two sites do with the
/// RETURN VALUE is where they legitimately differ: only the session stream
/// reconnects on a [`SseBodyEnd::Dropped`].
///
/// A free function over already-decoded values rather than a method, for the
/// same reason the four helpers under it are: a `hyper::body::Incoming` cannot
/// be constructed outside hyper, so every part of this path that CAN be reached
/// from a test is.
async fn read_sse_body(
    sender: &mpsc::Sender<Result<TransportMessage>>,
    last_event_id: &Arc<RwLock<Option<String>>>,
    on_resumption: Option<&Arc<dyn Fn(String) + Send + Sync>>,
    body: hyper::body::Incoming,
    max_buffer_size: usize,
) -> SseBodyEnd {
    let mut state = SseReadState::new(body, max_buffer_size);
    let mut progress = SseProgress::default();
    loop {
        if !drain_pending_events(
            sender,
            last_event_id,
            on_resumption,
            &mut state,
            &mut progress,
        )
        .await
        {
            // Either a frame was unparseable (D-05, terminal error already
            // sent) or the receiver is gone. Neither is a drop to retry.
            return SseBodyEnd::Ended;
        }
        // End-of-body drains `pending` FIRST (above), so trailing complete
        // events are never lost to the `done` check.
        if state.done {
            return progress.dropped(None);
        }
        match read_next_sse_frame(&mut state).await {
            None => {},
            Some(SseFrameStop::Dropped(error)) => return progress.dropped(Some(error)),
            Some(SseFrameStop::Corrupt(error)) => {
                // Terminal, and latched by returning: at most ONE error per
                // reader, so one corrupt frame cannot become an error storm.
                let _ = sender.send(Err(error)).await;
                return SseBodyEnd::Ended;
            },
        }
    }
}

/// What one body has produced so far, for the two facts the reconnect loop
/// needs from a stream that then dropped.
#[derive(Default)]
struct SseProgress {
    /// At least one event was delivered, so a re-opened stream that WORKED
    /// earns a fresh reconnect budget.
    delivered: bool,
    /// The last server-provided SSE `retry:` value, which
    /// [`next_reconnect_delay`] lets win over the computed backoff.
    retry: Option<Duration>,
}

impl SseProgress {
    /// This body DROPPED, with `cause` if the drop was a transport failure
    /// rather than an ordinary end-of-body.
    fn dropped(&self, cause: Option<Error>) -> SseBodyEnd {
        SseBodyEnd::Dropped {
            cause,
            delivered: self.delivered,
            retry: self.retry,
        }
    }
}

/// Deliver every event already parsed, returning `false` when the reader must
/// STOP without retrying.
///
/// Split out of [`read_sse_body`] purely for the repo's cognitive-complexity
/// budget (CLAUDE.md, cog 25) — inline, the loop measured 29. The split is
/// along the natural seam: this owns the per-EVENT rules, the caller owns the
/// per-BODY ones.
async fn drain_pending_events(
    sender: &mpsc::Sender<Result<TransportMessage>>,
    last_event_id: &Arc<RwLock<Option<String>>>,
    on_resumption: Option<&Arc<dyn Fn(String) + Send + Sync>>,
    state: &mut SseReadState,
    progress: &mut SseProgress,
) -> bool {
    while let Some(event) = state.pending.pop_front() {
        // Recorded BEFORE delivery, and from any event: a peer that sends
        // `retry:` alongside a frame is telling the client how long to wait if
        // this stream drops, and the answer must survive the frame.
        if let Some(millis) = event.retry {
            progress.retry = Some(Duration::from_millis(millis));
        }
        if !deliver_sse_event(sender, last_event_id, on_resumption, event).await {
            return false;
        }
        progress.delivered = true;
    }
    true
}

/// Read ONE body frame into `state`, returning `Some(stop)` only when the
/// stream must END.
///
/// Extracted from the reader task's loop so neither exceeds the repo's
/// cognitive-complexity budget. Three properties make this shape correct, each of
/// which a fresh implementation gets wrong:
///
/// 1. `take_utf8_prefix` runs in the SAME iteration as the append, so the
///    residual in `bytes` is at most a three-byte incomplete-character tail.
///    Never decode per chunk with a lossy converter — that corrupts a multi-byte
///    character split across a TCP segment into `U+FFFD` (T-118.2-01-06).
/// 2. [`sse_stream_overflow`] is polled ONCE PER CHUNK, and the parser's flag
///    LATCHES, so a caller cannot miss it.
/// 3. End-of-body sets `done` without draining: the CALLER drains `pending`
///    before it checks `done`, so trailing complete events are not lost.
async fn read_next_sse_frame(state: &mut SseReadState) -> Option<SseFrameStop> {
    match state.body.frame().await {
        // End of body. Anything already in `pending` is still drained by the
        // caller's loop before the `done` check ends the stream.
        None => {
            state.done = true;
            None
        },
        Some(Err(e)) => {
            state.done = true;
            Some(SseFrameStop::Dropped(Error::Transport(
                TransportError::Request(e.to_string()),
            )))
        },
        Some(Ok(frame)) => {
            if let Some(chunk) = frame.data_ref() {
                state.bytes.extend_from_slice(chunk);
                let text = crate::shared::sse_parser::take_utf8_prefix(&mut state.bytes);
                state
                    .pending
                    .extend(drain_sse_events(&mut state.parser, &text));
                if let Some(error) = sse_stream_overflow(&state.parser) {
                    // The peer pushed the parser's retained state plus this chunk
                    // past the bound, so the parser DISCARDED bytes and this byte
                    // stream is no longer trustworthy. There is nothing
                    // meaningful to continue to, and nothing to RECONNECT to
                    // either — a re-open would replay the same corruption.
                    state.done = true;
                    return Some(SseFrameStop::Corrupt(error));
                }
            }
            // A trailers frame carries no data; the caller loops and reads again.
            None
        },
    }
}

/// Feed `chunk` to the SHARED SSE parser and return the events it completed.
///
/// Keeps the existing message-event filter semantics: an event whose `event`
/// field is absent or equals `"message"`. Keep-alive comment lines never produce
/// an event at all — the shared parser drops them — so they are skipped for free
/// rather than by a second rule that could drift.
fn drain_sse_events(
    parser: &mut SseParser,
    chunk: &str,
) -> Vec<crate::shared::sse_parser::SseEvent> {
    parser
        .feed(chunk)
        .into_iter()
        .filter(|event| event.event.as_deref().is_none_or(|name| name == "message"))
        .collect()
}

/// The stream-ENDING error, when the parser has discarded in-flight bytes past
/// its bound (D-02).
///
/// It NAMES the limit and the peer's behaviour and nothing else — no frame
/// content is echoed, because the bytes that tripped the bound are exactly the
/// untrusted input [`MAX_ECHOED_SSE_FRAME`] exists to keep out of a client's
/// logs.
///
/// A free function over the parser rather than an inline check in
/// [`read_next_sse_frame`] so the condition is reachable from a test: that
/// function owns a live `hyper::body::Incoming`, which cannot be constructed
/// outside hyper.
fn sse_stream_overflow(parser: &SseParser) -> Option<Error> {
    if !parser.overflowed() {
        return None;
    }
    Some(Error::Transport(TransportError::InvalidMessage(format!(
        "a session-stream chunk pushed the buffered stream state past the {}-byte parser bound; \
         the buffered bytes were discarded and the stream was ended",
        parser.max_buffer_size()
    ))))
}

/// The stream-ENDING error, when a `message` frame does not parse as JSON-RPC
/// (D-05).
///
/// Deliberately terminal rather than an `if let Ok(..)` silent drop: an
/// unparseable frame may be a server-to-client REQUEST, and swallowing it hangs
/// both ends with no signal at either. Corruption gets ONE story, not two.
///
/// A free function over the already-decoded payload for the same testability
/// reason as [`sse_stream_overflow`], and the echo is bounded by
/// [`truncate_sse_frame`].
fn unparseable_sse_frame(cause: &Error, data: &str) -> Error {
    Error::Transport(TransportError::InvalidMessage(format!(
        "a session-stream frame did not parse as a JSON-RPC message ({cause}); the stream was \
         ended. Frame: {}",
        truncate_sse_frame(data)
    )))
}

/// Bound an untrusted string for inclusion in an error message.
///
/// Scans at most `MAX_ECHOED_SSE_FRAME + 1` characters: `chars().count()` would
/// walk the WHOLE untrusted string, whose length a remote peer chooses, just to
/// answer "is it longer than 200 characters?".
fn truncate_sse_frame(text: &str) -> String {
    let mut boundary = None;
    for (index, (offset, _)) in text.char_indices().enumerate() {
        if index == MAX_ECHOED_SSE_FRAME {
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

/// Deliver ONE parsed event, returning `false` when the reader must stop.
///
/// Split out of the reader task's loop for the cognitive-complexity budget, and
/// because it owns the two rules the loop must not get wrong: a resumption
/// cursor is recorded BEFORE the payload is classified, and a failed
/// `sender.send` means the receiver was dropped — the last transport clone is
/// gone — so the reader RETURNS rather than retrying.
async fn deliver_sse_event(
    sender: &mpsc::Sender<Result<TransportMessage>>,
    last_event_id: &Arc<RwLock<Option<String>>>,
    on_resumption: Option<&Arc<dyn Fn(String) + Send + Sync>>,
    event: crate::shared::sse_parser::SseEvent,
) -> bool {
    if let Some(id) = &event.id {
        *last_event_id.write() = Some(id.clone());
        if let Some(callback) = on_resumption {
            callback(id.clone());
        }
    }
    match crate::shared::StdioTransport::parse_message(event.data.as_bytes()) {
        Ok(message) => sender.send(Ok(message)).await.is_ok(),
        Err(cause) => {
            let _ = sender
                .send(Err(unparseable_sse_frame(&cause, &event.data)))
                .await;
            false
        },
    }
}

/// Decode a SEQUENCE of body chunks exactly as a live SSE stream would, for
/// fuzzing and property testing (Phase 118.2, ALWAYS/FUZZ).
///
/// # Not a decode API
///
/// `#[doc(hidden)]` and gated behind the non-default `fuzzing` feature, so it is
/// absent from `default` and `full` builds. It differs from the shipping reader
/// in ways a caller would not want:
///
/// - the **unvalidated `max_buffer_size`**, which accepts `0` and then latches
///   the parser on the first non-empty chunk (a fuzz campaign wants that
///   reachable; a caller almost never does);
/// - **errors flattened to `String`**, so no private type escapes — which also
///   means no caller can match on the failure;
/// - it keeps FEEDING after an overflow, where a live stream ends on the first
///   `true`, so that the latch itself is testable.
///
/// The exact twin of
/// [`crate::client::subscriptions::decode_listen_chunks_for_fuzz`], whose
/// precedent this follows deliberately rather than inventing a second shape.
///
/// # Why a chunk SEQUENCE, and not one chunk
///
/// 1. **The overflow branch.** The parser is built with
///    [`SseParser::with_max_buffer_size`], so a campaign can pick a bound small
///    enough that short generated inputs reach the discard-and-latch path. At
///    the production 16 MiB bound ([`DEFAULT_MAX_COLLECTED_BODY_BYTES`]) a
///    fuzzer would have to synthesise 16 MiB of newline-free input to get there,
///    i.e. never.
/// 2. **State carried ACROSS chunks.** The undecoded-UTF-8 tail and the SSE line
///    buffer both survive from one chunk to the next in a live stream — exactly
///    as in [`read_next_sse_frame`] — so a split mid-character or mid-line is
///    reachable here and is not reachable with one chunk.
///
/// # Returns
///
/// `(outcomes, overflowed_after_each_chunk, peak_buffered_bytes,
/// undecoded_tail_bytes)`. The last three each have one entry per INPUT CHUNK,
/// evaluated after that chunk was drained:
///
/// - `outcomes` — one entry per `message` frame the stream would have delivered
///   or failed on, decoded through the SAME `parse_message` call
///   [`deliver_sse_event`] uses, with errors flattened to their `Display` string.
/// - `overflowed_after_each_chunk` — `sse_stream_overflow(&parser).is_some()`,
///   the PRODUCTION observer rather than a reconstruction of it.
/// - `peak_buffered_bytes` — `SseParser::buffered_bytes()`, i.e. the two
///   accumulators the bound actually covers (the unterminated line PLUS the
///   `data:` payload of the event still awaiting its blank line). This is the
///   quantity a campaign asserts against `max_buffer_size`. Reporting only
///   outcomes and flags is precisely why 20 000 green runs of the sibling target
///   could coexist with an unbounded-growth defect.
/// - `undecoded_tail_bytes` — what is left in the byte buffer after
///   `take_utf8_prefix`, which must never exceed 3: the longest incomplete UTF-8
///   character. **One vector more than the sibling seam returns**, and
///   deliberately: this reader's UTF-8 tail is a second unbounded-growth
///   candidate, and a target cannot assert a bound it cannot observe.
#[cfg(any(feature = "fuzzing", test))]
#[doc(hidden)]
#[must_use]
// Why: `clippy::type_complexity` fires on the four-vector return, which the
// sibling seam avoids only by returning three. Factoring it into a `pub type`
// alias would add a SECOND public item to this crate's API surface — and this
// plan's semver claim is that it adds exactly ONE (the function). The tuple is
// positional at both call sites and its four members are enumerated in the
// `# Returns` section above, so an alias would buy documentation that is
// already there at the cost of a surface this crate does not want.
#[allow(clippy::type_complexity)]
pub fn decode_sse_chunks_for_fuzz(
    chunks: &[&[u8]],
    max_buffer_size: usize,
) -> (
    Vec<std::result::Result<TransportMessage, String>>,
    Vec<bool>,
    Vec<usize>,
    Vec<usize>,
) {
    let mut parser = SseParser::with_max_buffer_size(max_buffer_size);
    let mut bytes: Vec<u8> = Vec::new();
    let mut outcomes = Vec::new();
    let mut overflowed = Vec::with_capacity(chunks.len());
    let mut peak_buffered_bytes = Vec::with_capacity(chunks.len());
    let mut undecoded_tail_bytes = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        bytes.extend_from_slice(chunk);
        let text = crate::shared::sse_parser::take_utf8_prefix(&mut bytes);
        outcomes.extend(
            drain_sse_events(&mut parser, &text)
                .into_iter()
                .map(|event| {
                    crate::shared::StdioTransport::parse_message(event.data.as_bytes())
                        .map_err(|error| error.to_string())
                }),
        );
        overflowed.push(sse_stream_overflow(&parser).is_some());
        peak_buffered_bytes.push(parser.buffered_bytes());
        undecoded_tail_bytes.push(bytes.len());
    }
    (
        outcomes,
        overflowed,
        peak_buffered_bytes,
        undecoded_tail_bytes,
    )
}

// ---------------------------------------------------------------------------
// The session stream's OWNED reader context and its bounded reconnect loop
// (Phase 118.2, D-03).
// ---------------------------------------------------------------------------

/// Everything a spawned session-stream reader OWNS, so it can reissue an
/// authenticated, middleware-aware GET without borrowing the transport.
///
/// # Why an owned context, and not a `StreamableHttpTransport` clone
///
/// The reader task is `'static`: it cannot borrow `&self`, and a `&self` helper
/// is therefore not callable from it. Cloning the whole transport in would
/// compile — but it would also hand the task `abort_handle`, and that is a
/// genuine leak rather than a convenience:
/// `abort_handle: Arc<RwLock<Option<JoinHandle<()>>>>` holds THIS TASK's own
/// join handle, so a task owning that `Arc` owns its own handle. Dropping the
/// last real transport clone would not release it, and the task would never be
/// reachable for abort at all (T-118.2-04-06).
///
/// **This struct therefore has NO `abort_handle` field, deliberately.** Its two
/// termination signals are instead:
///
/// 1. `sender.send(..)` returning `Err` — the receive queue's `Receiver` is
///    gone, so there is nobody to deliver to; and
/// 2. `sender.is_closed()`, checked BEFORE and AFTER every backoff sleep, which
///    is what makes a dropped transport stop a loop that is asleep rather than
///    reading.
///
/// Both go true the moment the last transport clone drops the `Receiver`.
///
/// Every field is a cheap clone of an already-shared value. The hyper `Client`
/// is `Clone` and pools connections, so the clone REUSES the pool rather than
/// opening a second one.
struct SseReaderContext {
    /// The transport's own hyper client, so a reconnect reuses its pool.
    client: Client<
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
        Full<Bytes>,
    >,
    /// The URL, the auth provider and the request-middleware chain all live
    /// here, which is what makes a reissued GET authenticated and
    /// middleware-aware rather than a bare request.
    config: Arc<RwLock<StreamableHttpTransportConfig>>,
    /// Read by the shared request builder for the `MCP-Protocol-Version` header,
    /// and WRITTEN by the shared response-header processor on each re-open.
    protocol_version: Arc<RwLock<Option<String>>>,
    /// Read by the shared request builder to decide session-header suppression
    /// and v2 routing headers.
    v2_mode: Arc<AtomicBool>,
    /// Plan 01's `Result`-carrying receive queue, and this task's liveness
    /// signal.
    sender: mpsc::Sender<Result<TransportMessage>>,
    /// Where a delivered event's id is recorded, and what
    /// [`Self::reconnect_cursor`] reads back out on the `v1-compat` build.
    last_event_id: Arc<RwLock<Option<String>>>,
    /// Read ONCE from the paired `resumption_callback` accessor at construction,
    /// so this task carries no `#[cfg]` of its own.
    on_resumption: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// The D-02 parser bound, i.e. the transport's `max_collected_body_bytes`.
    max_collected_body_bytes: usize,
}

impl SseReaderContext {
    /// A borrowed view of everything a reissued request is built from.
    fn request_parts(&self) -> RequestParts<'_> {
        RequestParts {
            config: &self.config,
            protocol_version: &self.protocol_version,
            v2_mode: &self.v2_mode,
        }
    }

    /// The cursor a reconnect resumes from.
    ///
    /// The `v1-compat` half: whatever the last delivered event's id was.
    #[cfg(feature = "v1-compat")]
    fn reconnect_cursor(&self) -> Option<String> {
        self.last_event_id.read().clone()
    }

    /// The null twin: a `full-v2` build has no resumption cursor, so the answer
    /// is the constant `None`.
    ///
    /// MCP `2026-07-28` removed SSE resumability outright. Do NOT "improve" this
    /// by reading `last_event_id` — answering here is exactly what keeps the
    /// reconnect call site free of a `#[cfg]`, and
    /// `tests/v1_severability_tripwire.rs` asserts both halves of that. A
    /// `full-v2` build cannot construct a cursor at all, which is what makes
    /// "no attacker-influenced cursor reaches the wire" a property of the
    /// compiled crate rather than of a runtime branch (T-118.2-04-03).
    #[cfg(not(feature = "v1-compat"))]
    #[allow(clippy::unused_self)]
    const fn reconnect_cursor(&self) -> Option<String> {
        None
    }

    /// Issue ONE GET and hand back its live body.
    ///
    /// `Ok(None)` means the server answered `405 Method Not Allowed`: it does
    /// not offer a GET session stream at all, which the spec makes an ordinary
    /// answer rather than an error. The initial open treats that as "no stream,
    /// no problem"; a RECONNECT treats it as a named end, because a server that
    /// offered the stream a moment ago and now refuses it is not a stream that
    /// can be restored.
    async fn open_sse_once(&self, cursor: Option<String>) -> Result<Option<hyper::body::Incoming>> {
        let request =
            StreamableHttpTransport::build_sse_get_request(&self.request_parts(), cursor).await?;

        let response = self
            .client
            .request(request)
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        // Handle 405 (SSE not supported) gracefully
        if response.status() == StatusCode::METHOD_NOT_ALLOWED {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(Error::Transport(TransportError::Request(format!(
                "SSE request failed with status: {}",
                response.status()
            ))));
        }

        // Process response headers
        StreamableHttpTransport::process_headers_from(&self.request_parts(), response.headers());
        Ok(Some(response.into_body()))
    }

    /// Read the session stream, RE-OPENING it under a bounded budget whenever it
    /// is dropped (Phase 118.2, D-03).
    ///
    /// # The retry lives INSIDE this one task, deliberately
    ///
    /// [`StreamableHttpTransport::start_sse`] aborts `abort_handle` as its very
    /// FIRST act, so a recursive `start_sse` call from in here would abort the
    /// task making the call. The loop therefore owns the attempt counter and the
    /// sleep, and re-opens through [`Self::open_sse_once`].
    ///
    /// # What is retried, and what is not
    ///
    /// Only a DROP — an end-of-body, or a connection failure mid-body, with no
    /// corruption seen. A D-02 overflow or a D-05 parse failure is a CORRUPTION
    /// end: the terminal error is already on the queue and re-opening would
    /// replay the same corruption against a peer that is already misbehaving
    /// (T-118.2-04-02).
    ///
    /// # No re-handshake, ever
    ///
    /// A reconnect reissues the GET and NOTHING else. A `404` or an expired
    /// session ends the loop with a named error rather than silently re-running
    /// `initialize`: a silent re-handshake would mint a new session id and
    /// orphan every in-flight correlation, which would let a peer induce
    /// correlation loss simply by expiring a session (T-118.2-04-04, RESEARCH
    /// Open Question 2).
    ///
    /// # Cancellation
    ///
    /// `sender.is_closed()` is checked before AND after each sleep. Without the
    /// post-sleep check the loop wakes up and reconnects to a peer nobody is
    /// listening to. `close()` additionally aborts this task outright, but a
    /// DROPPED transport cannot — there is no transport left to call `close()`
    /// on — which is precisely why the loop consults its sender rather than
    /// relying on the abort handle.
    async fn run_session_stream(self, mut body: hyper::body::Incoming) {
        // Reconnects already made. Reset to 0 after a re-opened stream that
        // STAYED UP for at least RECONNECT_BUDGET_RESET_UPTIME, so a stream that
        // survives for hours and then blinks again earns a FRESH budget rather
        // than inheriting a spent one: the budget bounds a burst of failures,
        // not the lifetime of a healthy connection.
        //
        // The condition used to be a bare `delivered`, and that was CR-01: a peer
        // writing ONE frame per body refunded the whole budget on every
        // iteration, so no value of MAX_SSE_RECONNECT_ATTEMPTS bounded the loop.
        // Uptime is what distinguishes a working stream from a one-frame bounce.
        let mut attempt: u32 = 0;
        loop {
            // std::time::Instant, not tokio::time::Instant: the latter moves
            // under `tokio::time::pause()`, which would make this arm's
            // behaviour depend on whether some caller paused the clock.
            let opened_at = std::time::Instant::now();
            let end = read_sse_body(
                &self.sender,
                &self.last_event_id,
                self.on_resumption.as_ref(),
                body,
                self.max_collected_body_bytes,
            )
            .await;

            let SseBodyEnd::Dropped {
                cause,
                delivered,
                retry,
            } = end
            else {
                return;
            };
            if budget_reset_earned(delivered, opened_at.elapsed()) {
                attempt = 0;
            }

            if attempt >= MAX_SSE_RECONNECT_ATTEMPTS {
                // Do NOT go quiet: a reader that simply stops is
                // indistinguishable from a healthy idle stream.
                let _ = self
                    .sender
                    .send(Err(reconnect_budget_exhausted(attempt, cause.as_ref())))
                    .await;
                return;
            }

            if self.sender.is_closed() {
                return;
            }
            tokio::time::sleep(next_reconnect_delay(attempt, retry)).await;
            if self.sender.is_closed() {
                return;
            }
            attempt += 1;

            // The call site carries NO `#[cfg]`: `reconnect_cursor` is a paired
            // accessor whose `full-v2` twin answers the constant `None`.
            match self.open_sse_once(self.reconnect_cursor()).await {
                Ok(Some(reopened)) => body = reopened,
                Ok(None) => {
                    let _ = self.sender.send(Err(reconnect_stream_gone(attempt))).await;
                    return;
                },
                Err(error) => {
                    let _ = self
                        .sender
                        .send(Err(reconnect_open_failed(attempt, &error)))
                        .await;
                    return;
                },
            }
        }
    }
}

/// How long to wait before reconnect attempt `attempt` (0-based).
///
/// `min(INITIAL_SSE_RECONNECT_DELAY * SSE_RECONNECT_GROWTH^attempt,
/// MAX_SSE_RECONNECT_DELAY)`, the reference client's `_getNextReconnectionDelay`
/// curve — unless the peer sent an SSE `retry:` field, which WINS, exactly as the
/// reference client lets it.
///
/// Unlike the reference, a peer-provided value is BOUNDED ON BOTH SIDES, to
/// [`MIN_SSE_RECONNECT_DELAY`] and [`MAX_SSE_RECONNECT_DELAY`]: `retry:` is
/// remote input, an uncapped one parks a client's reader task for a duration the
/// peer chose, and an unfloored one (`retry: 0`) turns the reconnect loop into a
/// request flood (CR-01, T-118.2-14-03).
///
/// Non-panicking on any `attempt`: an exponent that overflows `i32`, or a
/// product that is infinite, falls back to the maximum rather than
/// `unwrap`-ing a `Duration` conversion.
fn next_reconnect_delay(attempt: u32, server_retry: Option<Duration>) -> Duration {
    if let Some(retry) = server_retry {
        // `.max().min()` rather than `Duration`'s `clamp`: clamp PANICS when
        // `min > max`, and both operands here are constants a later edit could
        // invert. The non-panicking spelling degrades an inverted pair to a
        // value instead of to a panic inside a client's reader task.
        return retry
            .max(MIN_SSE_RECONNECT_DELAY)
            .min(MAX_SSE_RECONNECT_DELAY);
    }
    let exponent = i32::try_from(attempt).unwrap_or(i32::MAX);
    let seconds = INITIAL_SSE_RECONNECT_DELAY.as_secs_f64() * SSE_RECONNECT_GROWTH.powi(exponent);
    Duration::try_from_secs_f64(seconds)
        .unwrap_or(MAX_SSE_RECONNECT_DELAY)
        .min(MAX_SSE_RECONNECT_DELAY)
}

/// The stream-ENDING error, when the reconnect budget is spent (D-03).
///
/// Deliberately in the [`TransportError::Request`] family and deliberately
/// worded so it cannot be confused with the D-02 overflow or the D-05 parse
/// failure, both of which are [`TransportError::InvalidMessage`]: a consumer has
/// to be able to tell a LIFECYCLE end (the peer went away; the correlations in
/// flight are lost but nothing was corrupted) from CORRUPTION (this byte stream
/// lost data). `tests/client_sse_stream.rs` asserts the three-way distinctness
/// in both directions.
fn reconnect_budget_exhausted(attempts: u32, cause: Option<&Error>) -> Error {
    let because = cause.map_or_else(
        || "the peer ended the body".to_string(),
        |error| format!("the last attempt ended with: {error}"),
    );
    Error::Transport(TransportError::Request(format!(
        "the session stream was dropped and its {MAX_SSE_RECONNECT_ATTEMPTS}-attempt reconnect \
         budget (MAX_SSE_RECONNECT_ATTEMPTS) is exhausted after {attempts} attempt(s); {because}"
    )))
}

/// The stream-ENDING error, when a reconnect is answered `405 Method Not
/// Allowed`.
fn reconnect_stream_gone(attempts: u32) -> Error {
    Error::Transport(TransportError::Request(format!(
        "the session stream was dropped and reconnect attempt {attempts} was answered 405 Method \
         Not Allowed: the server no longer offers a GET session stream, so there is nothing left \
         to resume"
    )))
}

/// The stream-ENDING error, when a reconnect cannot re-open the stream.
///
/// Names the refusal to re-handshake explicitly, because "why did it not just
/// start a new session?" is the first question this message will be asked.
fn reconnect_open_failed(attempts: u32, cause: &Error) -> Error {
    Error::Transport(TransportError::Request(format!(
        "the session stream was dropped and reconnect attempt {attempts} could not re-open it \
         ({cause}); the stream was ended. The client deliberately does NOT re-handshake: a silent \
         re-`initialize` would mint a new session and orphan every in-flight correlation"
    )))
}

/// Derive the `(Mcp-Method, Mcp-Name)` pair a v2 request must carry, from the
/// JSON-RPC frame that request is about to send (Phase 113, CLNT-01 / VERS-05).
///
/// Returns `None` when the body is not a JSON-RPC frame with a `method` — a
/// response, or the empty body of a GET/DELETE — in which case no v2 routing
/// header is emitted.
///
/// The logical name is resolved METHOD-AWARELY through
/// [`crate::types::mrtr::name_bearing_key`], the SAME combined lookup the server's
/// `extract_body_method_and_name` reads: `tools/call` and `prompts/get` carry it
/// in `params.name`, `resources/read` in `params.uri`, `tasks/get` /
/// `tasks/update` / `tasks/cancel` in `params.taskId` (Phase 114, DQ4 — the spec
/// makes this a client **MUST** so an intermediary can route to the instance
/// holding the task state), and every other method has none — for which the
/// returned name is the EMPTY STRING, never an omission.
///
/// The tasks rows come from a SEPARATE table
/// ([`crate::types::mrtr::TASK_NAME_BEARING_METHODS`]) precisely so that naming
/// them does not make them MRTR-eligible; see that table's rustdoc.
///
/// The value is run through [`crate::types::mrtr::encode_header_value`], so a
/// non-header-safe name (non-ASCII, or an RFC 9110 field-value delimiter) travels
/// in the `=?base64?…?=` sentinel form the server's decoder understands
/// (T-113-47). The empty string round-trips unchanged.
///
/// Pure and non-panicking on arbitrary bytes.
fn v2_routing_headers(body: &[u8]) -> Option<(String, String)> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    // Derived through the ONE shared routing-pair reader, so the emitting half
    // and the server's cross-checking half cannot drift apart.
    let (method, name) = crate::types::mrtr::frame_routing_pair(&value)?;
    Some((
        method.to_string(),
        encode_header_value(name.as_deref().unwrap_or_default()),
    ))
}

/// A trait for providing authentication tokens.
#[async_trait]
pub trait AuthProvider: Send + Sync + Debug {
    /// Returns an access token.
    async fn get_access_token(&self) -> Result<String>;

    /// Called by the SDK transport immediately after receiving an HTTP 401 response,
    /// **before** the retry's `get_access_token()` call.
    ///
    /// Implementors should evict any cached access token here so that the subsequent
    /// `get_access_token()` call (which the SDK makes automatically as part of the
    /// single retry) returns a freshly-vended token rather than the now-invalid cached
    /// one.
    ///
    /// The default implementation is a no-op, preserving backward compatibility for
    /// all existing `AuthProvider` implementations.
    ///
    /// # Retry guarantee
    ///
    /// The SDK invokes `on_unauthorized()` at most once per request.  If the retry
    /// itself also receives a 401, that response is returned to the caller without
    /// further retrying or calling `on_unauthorized()` again.
    async fn on_unauthorized(&self) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(all(test, not(target_arch = "wasm32"), feature = "streamable-http"))]
mod tests {
    use super::*;
    use crate::shared::TransportMessage;
    use mockito::Server as MockServer;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use url::Url;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// A minimal `AuthProvider` that counts calls and returns a configurable token.
    #[derive(Debug)]
    struct CountingProvider {
        token: String,
        get_count: AtomicUsize,
        unauthorized_count: AtomicUsize,
        /// When `Some`, records the order of method calls as strings.
        call_order: Option<StdMutex<Vec<&'static str>>>,
    }

    impl CountingProvider {
        fn new(token: impl Into<String>) -> Self {
            Self {
                token: token.into(),
                get_count: AtomicUsize::new(0),
                unauthorized_count: AtomicUsize::new(0),
                call_order: None,
            }
        }

        fn with_order_tracking(token: impl Into<String>) -> Self {
            Self {
                token: token.into(),
                get_count: AtomicUsize::new(0),
                unauthorized_count: AtomicUsize::new(0),
                call_order: Some(StdMutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl AuthProvider for CountingProvider {
        async fn get_access_token(&self) -> Result<String> {
            self.get_count.fetch_add(1, Ordering::SeqCst);
            if let Some(order) = &self.call_order {
                order.lock().unwrap().push("get_access_token");
            }
            Ok(self.token.clone())
        }

        async fn on_unauthorized(&self) -> Result<()> {
            self.unauthorized_count.fetch_add(1, Ordering::SeqCst);
            if let Some(order) = &self.call_order {
                order.lock().unwrap().push("on_unauthorized");
            }
            Ok(())
        }
    }

    /// Build a transport pointed at a mock server URL.
    fn make_transport(
        url: Url,
        provider: Option<Arc<dyn AuthProvider>>,
    ) -> StreamableHttpTransport {
        let mut builder = StreamableHttpTransportConfigBuilder::new(url);
        if let Some(p) = provider {
            builder = builder.with_auth_provider(p);
        }
        let config = builder.build();
        StreamableHttpTransport::new(config)
    }

    /// Helper: build a simple initialized notification for sending.
    fn ping_message() -> TransportMessage {
        use crate::types::{ClientNotification, Notification};
        TransportMessage::Notification(Notification::Client(ClientNotification::Initialized))
    }

    /// Helper: build a simple list-tools request for body-identity testing.
    fn list_tools_message() -> TransportMessage {
        use crate::types::{ClientRequest, ListToolsRequest, Request, RequestId};
        TransportMessage::Request {
            id: RequestId::from(42i64),
            request: Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
                cursor: None,
            }))),
        }
    }

    // ------------------------------------------------------------------
    // Test 1: default on_unauthorized is a no-op (compile + call succeeds)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_on_unauthorized_default_noop_compiles_and_succeeds() {
        /// A minimal impl that only provides `get_access_token`.
        #[derive(Debug)]
        struct MinimalProvider;

        #[async_trait]
        impl AuthProvider for MinimalProvider {
            async fn get_access_token(&self) -> Result<String> {
                Ok("token".to_string())
            }
            // on_unauthorized NOT overridden — uses the default no-op.
        }

        let p = MinimalProvider;
        // The default no-op should compile and return Ok(()).
        let result = p.on_unauthorized().await;
        assert!(
            result.is_ok(),
            "default on_unauthorized should return Ok(())"
        );
    }

    // ------------------------------------------------------------------
    // Test 2: on_unauthorized invoked exactly once; request retried once on 401.
    //          A second 401 on the retry is returned as-is (no infinite loop).
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_max_one_retry_on_401() {
        let mut server = MockServer::new_async().await;

        // Both requests return 401 — verifies exactly one retry (no infinite loop).
        let _m = server
            .mock("POST", "/")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"unauthorized"}"#)
            .expect(2) // first attempt + exactly one retry
            .create_async()
            .await;

        let url = Url::parse(&server.url()).unwrap();
        let provider = Arc::new(CountingProvider::new("initial-token"));
        let mut transport = make_transport(url, Some(provider.clone() as Arc<dyn AuthProvider>));

        // Sending will hit 401 twice (original + retry) and then return an error.
        let _ = transport
            .send_with_options(ping_message(), SendOptions::default())
            .await;

        // on_unauthorized called exactly once (not twice).
        assert_eq!(
            provider.unauthorized_count.load(Ordering::SeqCst),
            1,
            "on_unauthorized should be called exactly once"
        );
        // get_access_token called twice: once per attempt.
        assert_eq!(
            provider.get_count.load(Ordering::SeqCst),
            2,
            "get_access_token should be called twice (once per attempt)"
        );
    }

    // ------------------------------------------------------------------
    // Test 3: on_unauthorized NOT called for non-401 responses.
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_on_unauthorized_not_called_for_non_401() {
        let mut server = MockServer::new_async().await;

        // 200 with valid JSON-RPC response.
        let _m200 = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
            .create_async()
            .await;

        let url = Url::parse(&server.url()).unwrap();
        let provider = Arc::new(CountingProvider::new("token"));
        let mut transport =
            make_transport(url.clone(), Some(provider.clone() as Arc<dyn AuthProvider>));

        let _ = transport
            .send_with_options(ping_message(), SendOptions::default())
            .await;
        assert_eq!(
            provider.unauthorized_count.load(Ordering::SeqCst),
            0,
            "on_unauthorized must NOT be called on 200"
        );

        // Also check 500 — no call to on_unauthorized.
        let mut server2 = MockServer::new_async().await;
        let _m500 = server2
            .mock("POST", "/")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"server error"}"#)
            .create_async()
            .await;

        let url2 = Url::parse(&server2.url()).unwrap();
        let provider2 = Arc::new(CountingProvider::new("token"));
        let mut transport2 = make_transport(url2, Some(provider2.clone() as Arc<dyn AuthProvider>));

        let _ = transport2
            .send_with_options(ping_message(), SendOptions::default())
            .await;
        assert_eq!(
            provider2.unauthorized_count.load(Ordering::SeqCst),
            0,
            "on_unauthorized must NOT be called on 500"
        );
    }

    // ------------------------------------------------------------------
    // Test 4 (Codex HIGH #17): Retried request body, method, and non-Authorization
    //         headers are byte-identical to the original request.
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_retry_body_and_headers_are_byte_identical() {
        use hyper::service::service_fn;
        use hyper_util::rt::TokioExecutor;
        use hyper_util::server::conn::auto::Builder as ServerBuilder;
        use std::sync::Mutex as StdMutex;
        use tokio::net::TcpListener;

        /// Captures `(method, body, auth_header)` for each received request.
        #[derive(Debug, Default)]
        struct Captured {
            requests: Vec<(String, Vec<u8>, String)>,
        }

        /// Provider returns different tokens per call to prove the
        /// `Authorization` header changes between attempts.
        #[derive(Debug)]
        struct DualTokenProvider {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AuthProvider for DualTokenProvider {
            async fn get_access_token(&self) -> Result<String> {
                let n = self.call_count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Ok("token-attempt-1".to_string())
                } else {
                    Ok("token-attempt-2".to_string())
                }
            }
        }

        let captured = Arc::new(StdMutex::new(Captured::default()));
        let captured_clone = captured.clone();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let cap = captured_clone.clone();
        tokio::spawn(async move {
            let mut attempt = 0u8;
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let cap = cap.clone();
                let io = hyper_util::rt::TokioIo::new(stream);
                tokio::spawn(async move {
                    let _ = ServerBuilder::new(TokioExecutor::new())
                        .serve_connection(
                            io,
                            service_fn(move |req: Request<hyper::body::Incoming>| {
                                let cap = cap.clone();
                                async move {
                                    let method = req.method().to_string();
                                    let auth = req
                                        .headers()
                                        .get("authorization")
                                        .and_then(|v| v.to_str().ok())
                                        .unwrap_or("")
                                        .to_string();
                                    let body_bytes = req
                                        .collect()
                                        .await
                                        .map(|b| b.to_bytes().to_vec())
                                        .unwrap_or_default();
                                    cap.lock()
                                        .unwrap()
                                        .requests
                                        .push((method, body_bytes, auth));
                                    // First request: 401. Second: 200.
                                    let status = {
                                        let len = cap.lock().unwrap().requests.len();
                                        if len == 1 {
                                            401u16
                                        } else {
                                            200u16
                                        }
                                    };
                                    Ok::<_, hyper::Error>(
                                        HyperResponse::builder()
                                            .status(status)
                                            .header("content-type", "application/json")
                                            .body(Full::new(Bytes::from(if status == 200 {
                                                r#"{"jsonrpc":"2.0","id":1,"result":{}}"#
                                            } else {
                                                r#"{"error":"unauthorized"}"#
                                            })))
                                            .unwrap(),
                                    )
                                }
                            }),
                        )
                        .await;
                });
                attempt += 1;
                if attempt >= 2 {
                    break;
                }
            }
        });

        let provider = Arc::new(DualTokenProvider {
            call_count: AtomicUsize::new(0),
        });
        let url = Url::parse(&format!("http://127.0.0.1:{}", addr.port())).unwrap();
        let mut transport = make_transport(url, Some(provider as Arc<dyn AuthProvider>));

        let _ = transport
            .send_with_options(list_tools_message(), SendOptions::default())
            .await;

        let cap = captured.lock().unwrap();
        assert_eq!(
            cap.requests.len(),
            2,
            "expected exactly 2 requests (original + retry)"
        );

        let (method1, body1, auth1) = &cap.requests[0];
        let (method2, body2, auth2) = &cap.requests[1];

        // Method must be identical.
        assert_eq!(
            method1, method2,
            "method must be byte-identical across retry"
        );

        // Body must be byte-identical.
        assert_eq!(body1, body2, "body must be byte-identical across retry");

        // Authorization header must DIFFER (new token on retry).
        assert_ne!(
            auth1, auth2,
            "Authorization header should differ (new token)"
        );
        assert!(auth1.contains("token-attempt-1"), "first auth: {}", auth1);
        assert!(auth2.contains("token-attempt-2"), "retry auth: {}", auth2);
    }

    // ------------------------------------------------------------------
    // Test 5: on_unauthorized() is invoked BEFORE get_access_token() on retry.
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_on_unauthorized_called_before_get_access_token_on_retry() {
        let mut server = MockServer::new_async().await;

        let _m = server
            .mock("POST", "/")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"unauthorized"}"#)
            .expect(2)
            .create_async()
            .await;

        let url = Url::parse(&server.url()).unwrap();
        let provider = Arc::new(CountingProvider::with_order_tracking("token"));
        let mut transport = make_transport(url, Some(provider.clone() as Arc<dyn AuthProvider>));

        let _ = transport
            .send_with_options(ping_message(), SendOptions::default())
            .await;

        let order = provider
            .call_order
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .clone();

        // Expected order: get_access_token (attempt 1), on_unauthorized, get_access_token (retry).
        assert!(
            order.len() >= 3,
            "expected at least 3 calls, got {:?}",
            order
        );
        // Locate on_unauthorized and the get_access_token that follows it.
        let unauth_pos = order
            .iter()
            .position(|&s| s == "on_unauthorized")
            .expect("on_unauthorized must appear in call order");
        let retry_get_pos = order
            .iter()
            .skip(unauth_pos + 1)
            .position(|&s| s == "get_access_token");
        assert!(
            retry_get_pos.is_some(),
            "get_access_token must be called AFTER on_unauthorized; order = {:?}",
            order
        );
    }

    // ==================================================================
    // Phase 113 / CLNT-01 — v2 (`2026-07-28`) outbound headers.
    // ==================================================================

    mod v2_outbound {
        use super::*;
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;
        use serde_json::json;

        fn body(method: &str, params: &serde_json::Value) -> Vec<u8> {
            json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
                .to_string()
                .into_bytes()
        }

        /// Plant a v1 session id on a fresh config, where a field exists to hold
        /// one.
        ///
        /// `StreamableHttpTransportConfig::session_id` is gated behind
        /// `v1-compat`, so a bare `config.session_id = …` here made
        /// `cargo test -p pmcp --no-default-features --features full-v2` a hard
        /// BUILD failure rather than a green run — the whole lib-test target,
        /// including the ~40 era-neutral tests around this one. This pair keeps
        /// the call sites identical on both feature sets, exactly as the
        /// production `v1` module pair does, so no `#[cfg]` reaches the fixtures
        /// below.
        ///
        /// The `full-v2` half is not a weakened assertion: on that build there is
        /// no field to plant a session id IN, which is the severance claim stated
        /// structurally. The tests that assert what a STORED session id does are
        /// gated individually below.
        #[cfg(feature = "v1-compat")]
        fn plant_session_id(config: &mut StreamableHttpTransportConfig, session_id: Option<&str>) {
            config.session_id = session_id.map(str::to_string);
        }

        /// The `full-v2` half of [`plant_session_id`]: there is nowhere to plant it.
        #[cfg(not(feature = "v1-compat"))]
        #[allow(clippy::missing_const_for_fn)]
        fn plant_session_id(
            _config: &mut StreamableHttpTransportConfig,
            _session_id: Option<&str>,
        ) {
        }

        /// A transport with the v2 era selected through the PRODUCTION seam.
        fn v2_transport(session_id: Option<&str>) -> StreamableHttpTransport {
            let mut config = StreamableHttpTransportConfigBuilder::new(
                Url::parse("http://127.0.0.1:1/").unwrap(),
            )
            .build();
            plant_session_id(&mut config, session_id);
            let mut transport = StreamableHttpTransport::new(config);
            transport
                .set_negotiated_protocol_version(Some(PROTOCOL_VERSION_2026_07_28.to_string()));
            transport
        }

        fn v1_transport(session_id: Option<&str>) -> StreamableHttpTransport {
            let mut config = StreamableHttpTransportConfigBuilder::new(
                Url::parse("http://127.0.0.1:1/").unwrap(),
            )
            .build();
            plant_session_id(&mut config, session_id);
            StreamableHttpTransport::new(config)
        }

        async fn headers_for(
            transport: &StreamableHttpTransport,
            body: Vec<u8>,
        ) -> hyper::HeaderMap {
            transport
                .build_request_with_middleware(Method::POST, "http://127.0.0.1:1/", body)
                .await
                .expect("request builds")
                .headers()
                .clone()
        }

        fn header(map: &hyper::HeaderMap, name: &str) -> Option<String> {
            map.get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        }

        // ---- the pure derivation --------------------------------------

        #[test]
        fn routing_headers_read_name_for_tools_call() {
            let derived = v2_routing_headers(&body("tools/call", &json!({ "name": "search" })));
            assert_eq!(
                derived,
                Some(("tools/call".to_string(), "search".to_string()))
            );
        }

        #[test]
        fn routing_headers_read_name_for_prompts_get() {
            let derived = v2_routing_headers(&body("prompts/get", &json!({ "name": "greeting" })));
            assert_eq!(
                derived,
                Some(("prompts/get".to_string(), "greeting".to_string()))
            );
        }

        /// `resources/read` carries its logical name in `params.uri` — a
        /// `ReadResourceRequest` has NO `name` field, so reading `params.name`
        /// would emit an empty value and fail the server's body cross-check.
        #[test]
        fn routing_headers_read_uri_for_resources_read() {
            let derived =
                v2_routing_headers(&body("resources/read", &json!({ "uri": "mem://greeting" })));
            assert_eq!(
                derived,
                Some(("resources/read".to_string(), "mem://greeting".to_string()))
            );
        }

        /// The spec MUST: `Mcp-Name` carries `params.taskId` (Phase 114, DQ4).
        ///
        /// pmcp emitted `Mcp-Name: ""` here before this change, which Phase
        /// 118's conformance run grades.
        #[test]
        fn routing_headers_read_task_id_for_the_three_tasks_methods() {
            for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
                let derived = v2_routing_headers(&body(method, &json!({ "taskId": "abc" })));
                assert_eq!(
                    derived,
                    Some((method.to_string(), "abc".to_string())),
                    "{method} must route on its taskId"
                );
            }
        }

        /// `tasks/list` is NOT in the tasks routing table, so it keeps emitting
        /// the empty name every non-name-bearing method emits.
        ///
        /// The negative half of the assertion above: without it, an
        /// implementation that made every `tasks/*` method name-bearing would
        /// pass.
        #[test]
        fn routing_headers_are_empty_for_tasks_list_and_tasks_result() {
            for method in ["tasks/list", "tasks/result"] {
                let derived = v2_routing_headers(&body(method, &json!({ "taskId": "abc" })));
                assert_eq!(
                    derived,
                    Some((method.to_string(), String::new())),
                    "{method} is not name-bearing"
                );
            }
        }

        #[test]
        fn routing_headers_are_none_for_a_body_without_a_method() {
            assert_eq!(
                v2_routing_headers(br#"{"jsonrpc":"2.0","id":1,"result":{}}"#),
                None
            );
            assert_eq!(v2_routing_headers(b"not json"), None);
            assert_eq!(v2_routing_headers(b""), None);
        }

        #[test]
        fn routing_headers_sentinel_encode_a_non_ascii_name() {
            let (_, name) = v2_routing_headers(&body("tools/call", &json!({ "name": "поиск" })))
                .expect("derived");
            assert!(
                name.starts_with(crate::types::mrtr::HEADER_SENTINEL_PREFIX),
                "a non-header-safe name must travel as a sentinel, got {name}"
            );
            assert_eq!(
                crate::types::mrtr::decode_header_value(&name).as_deref(),
                Some("поиск"),
                "the shared codec must round-trip"
            );
        }

        // ---- emission on the actual request builder ---------------------

        #[tokio::test]
        async fn v2_tools_call_emits_all_three_headers() {
            let transport = v2_transport(None);
            let map = headers_for(
                &transport,
                body("tools/call", &json!({ "name": "search", "arguments": {} })),
            )
            .await;

            assert_eq!(header(&map, MCP_METHOD).as_deref(), Some("tools/call"));
            assert_eq!(header(&map, MCP_NAME).as_deref(), Some("search"));
            assert_eq!(
                header(&map, MCP_PROTOCOL_VERSION).as_deref(),
                Some(PROTOCOL_VERSION_2026_07_28)
            );
        }

        /// The client's emission rule: PRESENT, EMPTY — never omitted.
        ///
        /// Since Phase 118 D-13 the SERVER no longer requires the header on a
        /// name-less method, so this is now a superset of what is demanded rather
        /// than a necessity. It is pinned anyway: the empty value is still
        /// accepted (and discarded) by the gate, and dropping the emission would
        /// be a silent wire-shape change with no upside.
        #[tokio::test]
        async fn v2_nameless_method_emits_an_empty_mcp_name() {
            let transport = v2_transport(None);
            let map = headers_for(&transport, body("tools/list", &json!({}))).await;

            assert_eq!(header(&map, MCP_METHOD).as_deref(), Some("tools/list"));
            assert!(
                map.contains_key(MCP_NAME),
                "this client emits Mcp-Name on every v2 request"
            );
            assert_eq!(header(&map, MCP_NAME).as_deref(), Some(""));
        }

        #[tokio::test]
        async fn v2_resources_read_puts_the_uri_in_mcp_name() {
            let transport = v2_transport(None);
            let map = headers_for(
                &transport,
                body("resources/read", &json!({ "uri": "mem://greeting" })),
            )
            .await;
            assert_eq!(header(&map, MCP_NAME).as_deref(), Some("mem://greeting"));
        }

        #[tokio::test]
        async fn v2_lists_both_accept_content_types() {
            // `Accept` is set on the POST itself (both content types, because a
            // v2 POST may be answered with JSON or an SSE stream).
            assert_eq!(ACCEPT_STREAMABLE, "application/json, text/event-stream");
        }

        // ---- session-id suppression (T-113-06) ---------------------------

        #[tokio::test]
        async fn v2_never_emits_a_stored_session_id() {
            let transport = v2_transport(Some("left-over-from-v1"));
            let map = headers_for(&transport, body("tools/list", &json!({}))).await;
            assert!(
                !map.contains_key(MCP_SESSION_ID),
                "a session id must never reach the v2 wire, even when one is stored"
            );
        }

        /// Gated with its v1 sibling below: `StreamableHttpTransport::session_id`
        /// is itself severed on `full-v2`, so the property this test measures —
        /// "the response header was not STORED" — has no observer there because
        /// there is no store. The structural form of the same claim is proven by
        /// `tests/v2_client_carries_no_session_on_severed_build.rs`, which RUNS
        /// on the severed build.
        #[cfg(feature = "v1-compat")]
        #[test]
        fn v2_does_not_store_a_session_id_from_a_response() {
            let transport = v2_transport(None);
            let response = HyperResponse::builder()
                .status(StatusCode::OK)
                .header(MCP_SESSION_ID, "planted")
                .body(Full::new(Bytes::new()))
                .unwrap();
            transport.process_response_headers(&response);
            assert_eq!(
                transport.session_id(),
                None,
                "a v2 response's Mcp-Session-Id must not be stored"
            );
        }

        #[cfg(feature = "v1-compat")]
        #[test]
        fn v1_still_stores_a_session_id_from_a_response() {
            let transport = v1_transport(None);
            let response = HyperResponse::builder()
                .status(StatusCode::OK)
                .header(MCP_SESSION_ID, "kept")
                .body(Full::new(Bytes::new()))
                .unwrap();
            transport.process_response_headers(&response);
            assert_eq!(transport.session_id().as_deref(), Some("kept"));
        }

        /// A rogue server echoing the v2 version header must NOT be able to flip
        /// a v1 client into v2 emission mode (which would suppress its session).
        #[test]
        fn a_server_echo_cannot_flip_a_v1_client_into_v2() {
            let transport = v1_transport(Some("s1"));
            let response = HyperResponse::builder()
                .status(StatusCode::OK)
                .header(MCP_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28)
                .body(Full::new(Bytes::new()))
                .unwrap();
            transport.process_response_headers(&response);
            assert!(!transport.is_v2(), "only the client selects the era");
        }

        // ---- v1 is unchanged ---------------------------------------------

        /// `true` when the compiled client transport is the one that HAS a v1
        /// session.
        ///
        /// `StreamableHttpTransportConfig::session_id` is severed on `full-v2`,
        /// so the session half of the assertion below has nothing to be true
        /// about there. Expressed as a const rather than a `#[cfg]` on the whole
        /// test so the v2-routing-header half — which is era-neutral and is the
        /// part CLNT-01 is actually about — keeps RUNNING on the severed build.
        const V1_SESSION_EXISTS: bool = cfg!(feature = "v1-compat");

        #[tokio::test]
        async fn v1_emits_no_v2_routing_headers_and_keeps_its_session() {
            let transport = v1_transport(Some("session-123"));
            let map = headers_for(
                &transport,
                body("tools/call", &json!({ "name": "search", "arguments": {} })),
            )
            .await;

            assert!(!map.contains_key(MCP_METHOD));
            assert!(!map.contains_key(MCP_NAME));
            assert_eq!(
                header(&map, MCP_SESSION_ID).as_deref(),
                V1_SESSION_EXISTS.then_some("session-123"),
                "a v1 client emits its stored session id; a severed client has none to emit"
            );
        }

        // ---- non-panicking emission (T-113-20) ---------------------------

        proptest::proptest! {
            #[test]
            fn header_emission_never_panics_for_any_method_or_name(
                method in ".{0,64}",
                name in ".{0,64}",
            ) {
                let frame = body(&method, &json!({ "name": name, "uri": name }));
                // Derivation is total and non-panicking...
                let derived = v2_routing_headers(&frame);
                // ...and so is emission, for whatever it produced.
                if let Some((m, n)) = derived {
                    let builder = Request::builder().method(Method::POST).uri("http://127.0.0.1:1/");
                    let _ = StreamableHttpTransport::apply_v2_outbound_headers(builder, &m, &n);
                }
            }
        }
    }

    // ==================================================================
    // Phase 113 / D-113-E — a structured JSON-RPC error on a v2 non-2xx.
    // ==================================================================

    mod v2_error_envelope {
        use super::*;
        use crate::types::jsonrpc::ResponsePayload;
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;

        /// The exact shape plan 04 puts on the wire for an expired/tampered
        /// `requestState`: `-32602` at HTTP 400.
        const INVALID_PARAMS_BODY: &str = r#"{"jsonrpc":"2.0","id":"abc","error":{"code":-32602,"message":"requestState could not be accepted"}}"#;

        fn transport_for(url: &str, v2: bool) -> StreamableHttpTransport {
            let config =
                StreamableHttpTransportConfigBuilder::new(Url::parse(url).unwrap()).build();
            let mut transport = StreamableHttpTransport::new(config);
            if v2 {
                transport
                    .set_negotiated_protocol_version(Some(PROTOCOL_VERSION_2026_07_28.to_string()));
            }
            transport
        }

        /// A `-32602` at HTTP 400 reaches the CLIENT as a JSON-RPC error, not as
        /// an opaque `TransportError::Request("… status: 400 …")`. Without this
        /// the MRTR retry loop cannot dispatch on `error.code` at all.
        #[tokio::test]
        async fn v2_surfaces_a_jsonrpc_error_carried_on_a_400() {
            let mut server = MockServer::new_async().await;
            let mock = server
                .mock("POST", "/")
                .with_status(400)
                .with_header("content-type", "application/json")
                .with_body(INVALID_PARAMS_BODY)
                .create_async()
                .await;

            let mut transport = transport_for(&server.url(), true);
            transport
                .send_raw(br#"{"jsonrpc":"2.0","id":"abc","method":"tools/call","params":{"name":"x","arguments":{}}}"#.to_vec())
                .await
                .expect("a structured error must NOT be a transport failure");

            let message = transport
                .receive()
                .await
                .expect("the envelope is delivered");
            let TransportMessage::Response(response) = message else {
                panic!("expected a response, got {message:?}");
            };
            let ResponsePayload::Error(error) = response.payload else {
                panic!("expected the error payload");
            };
            assert_eq!(error.code, -32602);
            mock.assert_async().await;
        }

        /// A non-JSON-RPC 4xx body (a proxy's error page) still fails loudly on
        /// the status — nothing is laundered into a "server-authored" error.
        #[tokio::test]
        async fn v2_falls_back_to_the_status_error_for_a_non_envelope_body() {
            let mut server = MockServer::new_async().await;
            let _mock = server
                .mock("POST", "/")
                .with_status(502)
                .with_header("content-type", "text/html")
                .with_body("<html>bad gateway</html>")
                .create_async()
                .await;

            let mut transport = transport_for(&server.url(), true);
            let error = transport
                .send_raw(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#.to_vec())
                .await
                .expect_err("a proxy error page is still a transport failure");
            assert!(
                error.to_string().contains("502"),
                "the status must survive: {error}"
            );
        }

        /// v1 is UNCHANGED: a 400 is still an opaque transport error there.
        #[tokio::test]
        async fn v1_still_errors_on_the_status_alone() {
            let mut server = MockServer::new_async().await;
            let _mock = server
                .mock("POST", "/")
                .with_status(400)
                .with_header("content-type", "application/json")
                .with_body(INVALID_PARAMS_BODY)
                .create_async()
                .await;

            let mut transport = transport_for(&server.url(), false);
            let error = transport
                .send(list_tools_message())
                .await
                .expect_err("v1 behavior must be byte-identical to prior releases");
            assert!(
                error.to_string().contains("400"),
                "v1 must still report the status: {error}"
            );
        }
    }

    // ==================================================================
    // Phase 113-20 / T-113-84 — the collected-body cap.
    //
    // Every one of this transport's response reads is a WHOLE-BODY read, and
    // the SSE parser's complete-body entry point performs no bound check of its
    // own. These tests are what make that entry point's precondition an
    // established fact rather than a hope.
    //
    // The two parser-feeding sites are SEPARATE `collect()` call sites, so each
    // gets its OWN over-cap test and its OWN negative control. A single shared
    // test would pass with one of them uncapped.
    // ==================================================================

    mod collected_body_cap {
        use super::*;
        use std::time::Duration;

        /// Small enough that these tests cost bytes, not megabytes.
        const CAP: usize = 512;

        /// A parseable JSON-RPC response, so the under-cap tests can prove the
        /// body reached the parser rather than merely failing to error.
        const RESPONSE_JSON: &str = r#"{"jsonrpc":"2.0","id":42,"result":{"tools":[]}}"#;

        /// How long to wait before concluding nothing was dispatched. Only ever
        /// used to prove ABSENCE; the positive assertions await a real message.
        const QUIET_WINDOW: Duration = Duration::from_millis(250);

        fn capped_transport(url: &str, cap: usize) -> StreamableHttpTransport {
            let config =
                StreamableHttpTransportConfigBuilder::new(Url::parse(url).unwrap()).build();
            StreamableHttpTransport::new(config).with_max_collected_body_bytes(cap)
        }

        /// An SSE body of EXACTLY `len` bytes carrying one parseable frame.
        ///
        /// Padding rides an SSE COMMENT line (`:` … `\n`), which the parser
        /// ignores, so `len` changes the byte count and nothing else — the same
        /// body is expected to parse identically at any size.
        fn sse_body_of(len: usize) -> String {
            let frame = format!("event: message\ndata: {RESPONSE_JSON}\n\n");
            let padding = len
                .checked_sub(frame.len())
                .expect("requested length must fit one frame");
            let comment = match padding {
                0 => String::new(),
                1 => panic!("a comment line costs at least two bytes"),
                n => format!(":{}\n", "p".repeat(n - 2)),
            };
            let body = format!("{comment}{frame}");
            assert_eq!(body.len(), len, "the body must be exactly {len} bytes");
            body
        }

        /// A parseable JSON-RPC response of EXACTLY `len` bytes.
        ///
        /// The JSON twin of [`sse_body_of`], for the branches that still COLLECT
        /// a complete body. Padding rides an ignored `result` member, so `len`
        /// changes the byte count and nothing else.
        fn json_body_of(len: usize) -> String {
            let empty = r#"{"jsonrpc":"2.0","id":42,"result":{"tools":[],"pad":""}}"#;
            let padding = len
                .checked_sub(empty.len())
                .expect("requested length must fit one frame");
            let body = format!(
                r#"{{"jsonrpc":"2.0","id":42,"result":{{"tools":[],"pad":"{}"}}}}"#,
                "p".repeat(padding)
            );
            assert_eq!(body.len(), len, "the body must be exactly {len} bytes");
            body
        }

        /// Assert the refusal names the limit and leaks no body content.
        fn assert_over_cap_refusal(error: &Error, cap: usize) {
            let text = error.to_string();
            assert!(
                text.contains(&cap.to_string()),
                "the refusal must NAME the limit: {text}"
            );
            assert!(
                !text.contains("jsonrpc") && !text.contains("pppppppp"),
                "the refusal must not echo body content: {text}"
            );
        }

        // --------------------------------------------------------------
        // Site 1 of 2: the POST response (`post_body`).
        // --------------------------------------------------------------

        /// The POST-response path's bound is the PARSER's in-flight bound
        /// (Phase 118.2, D-01/D-02), and its refusal arrives on `receive()`.
        ///
        /// The exact twin of
        /// [`start_sse_over_the_parser_bound_ends_the_stream_with_a_named_error`]
        /// below, and it changed for exactly the same reason. This test USED to
        /// assert that `send` itself returned `Err` for a body one byte over the
        /// cap. `post_body` no longer collects a `text/event-stream` response —
        /// such a response stays open for the whole call, so there is no
        /// end-of-body to collect to and no body SIZE to refuse: `send` returns
        /// as soon as the reader task is spawned, and the refusal arrives where a
        /// live stream's failures have to arrive, on the receive queue behind
        /// every frame already delivered.
        ///
        /// The peer streams an unterminated `data:` line — the shape that
        /// ACCUMULATES, and so trips the bound regardless of how the transport
        /// happens to frame its reads.
        #[tokio::test]
        async fn post_response_over_the_parser_bound_ends_the_stream_with_a_named_error() {
            let mut server = MockServer::new_async().await;
            // No terminating newline and no blank line: pure accumulation.
            let body = format!("data: {}", "p".repeat(CAP * 2));
            let _mock = server
                .mock("POST", "/")
                .with_status(200)
                .with_header("content-type", TEXT_EVENT_STREAM)
                // Chunked: no `Content-Length` at all, so the refusal can only
                // come from the authoritative streaming bound (T-113-93).
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            transport
                .send(list_tools_message())
                .await
                .expect("send returns as soon as the reader task is spawned");

            let error = tokio::time::timeout(QUIET_WINDOW, transport.receive())
                .await
                .expect("the terminal error must be dispatched")
                .expect_err("an over-bound chunk must END the stream, not be silently dropped");
            assert_over_cap_refusal(&error, CAP);
        }

        /// Exactly the cap is ADMITTED and parses normally — pinning that the
        /// comparison is `>`, not `>=`.
        #[tokio::test]
        async fn post_response_at_the_cap_parses_normally() {
            let mut server = MockServer::new_async().await;
            let body = sse_body_of(CAP);
            let _mock = server
                .mock("POST", "/")
                .with_status(200)
                .with_header("content-type", TEXT_EVENT_STREAM)
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            transport
                .send(list_tools_message())
                .await
                .expect("a body at the cap must be accepted");

            let message = tokio::time::timeout(QUIET_WINDOW, transport.receive())
                .await
                .expect("the parsed event must be dispatched")
                .expect("the parsed event must be a message");
            assert!(
                matches!(message, TransportMessage::Response(_)),
                "expected the parsed response, got {message:?}"
            );
        }

        // --------------------------------------------------------------
        // Site 2 of 2: the GET SSE stream (`start_sse`).
        // --------------------------------------------------------------

        /// The GET path's bound is the PARSER's in-flight bound (Phase 118.2,
        /// D-01/D-02), and its refusal arrives on `receive()`.
        ///
        /// This test USED to assert that `start_sse` itself returned `Err` for a
        /// body one byte over the cap. `start_sse` no longer collects the body —
        /// a session stream has no end-of-body to collect to — so it has no body
        /// SIZE to refuse: it returns as soon as its reader task is spawned, and
        /// the refusal arrives where a live stream's failures have to arrive, on
        /// the receive queue behind every frame already delivered.
        ///
        /// What is bounded is therefore the parser's RETAINED state plus the
        /// chunk being fed, not a whole-body total. The peer here streams an
        /// unterminated `data:` line, which is the shape that ACCUMULATES and so
        /// trips the bound regardless of how the transport happens to frame its
        /// reads.
        #[tokio::test]
        async fn start_sse_over_the_parser_bound_ends_the_stream_with_a_named_error() {
            let mut server = MockServer::new_async().await;
            // No terminating newline and no blank line: pure accumulation.
            let body = format!("data: {}", "p".repeat(CAP * 2));
            let _mock = server
                .mock("GET", "/")
                .with_status(200)
                .with_header("content-type", TEXT_EVENT_STREAM)
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            transport
                .start_sse(None)
                .await
                .expect("start_sse returns as soon as its reader task is spawned");

            let error = tokio::time::timeout(QUIET_WINDOW, transport.receive())
                .await
                .expect("the terminal error must be dispatched")
                .expect_err("an over-bound chunk must END the stream, not be silently dropped");
            assert_over_cap_refusal(&error, CAP);
        }

        /// Exactly the bound is admitted on the GET path too.
        #[tokio::test]
        async fn start_sse_at_the_cap_parses_normally() {
            let mut server = MockServer::new_async().await;
            let body = sse_body_of(CAP);
            let _mock = server
                .mock("GET", "/")
                .with_status(200)
                .with_header("content-type", TEXT_EVENT_STREAM)
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            transport
                .start_sse(None)
                .await
                .expect("a body at the cap must be accepted");

            let message = tokio::time::timeout(QUIET_WINDOW, transport.receive())
                .await
                .expect("the parsed event must be dispatched")
                .expect("the parsed event must be a message");
            assert!(
                matches!(message, TransportMessage::Response(_)),
                "expected the parsed response, got {message:?}"
            );
        }

        // --------------------------------------------------------------
        // The peer-declared hint, the escape hatch, and the default wiring.
        // --------------------------------------------------------------

        /// A `Content-Length` over the cap is refused BEFORE the body is read.
        /// The header is an optimisation; the refusal names the declared size.
        ///
        /// Measured on a JSON response, which is where the whole-body collect
        /// now lives: since Phase 118.2 plan 03 a `text/event-stream` POST answer
        /// is read INCREMENTALLY and never collected, so it has no declared size
        /// to pre-check. The collected-body cap and this early refusal are
        /// unchanged for every branch that still parses a complete body.
        #[tokio::test]
        async fn a_declared_content_length_over_the_cap_is_refused_early() {
            let mut server = MockServer::new_async().await;
            let _mock = server
                .mock("POST", "/")
                .with_status(200)
                .with_header("content-type", APPLICATION_JSON)
                // `with_body` sets `Content-Length`.
                .with_body(json_body_of(CAP + 1))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            let error = transport
                .send(list_tools_message())
                .await
                .expect_err("an over-cap Content-Length must be refused");
            assert_over_cap_refusal(&error, CAP);
            assert!(
                error.to_string().contains(&(CAP + 1).to_string()),
                "the early refusal must name the DECLARED size: {error}"
            );
        }

        /// The seam is wired, not decorative: the body refused above is accepted
        /// once the cap is raised through the additive inherent builder.
        #[tokio::test]
        async fn raising_the_cap_admits_a_body_the_lower_one_refuses() {
            let mut server = MockServer::new_async().await;
            let body = sse_body_of(CAP + 1);
            let _mock = server
                .mock("POST", "/")
                .with_status(200)
                .with_header("content-type", TEXT_EVENT_STREAM)
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .expect_at_least(1)
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP * 4);
            transport
                .send(list_tools_message())
                .await
                .expect("the raised cap must admit the body the lower one refused");

            let message = tokio::time::timeout(QUIET_WINDOW, transport.receive())
                .await
                .expect("the parsed event must be dispatched")
                .expect("the parsed event must be a message");
            assert!(
                matches!(message, TransportMessage::Response(_)),
                "expected the parsed response, got {message:?}"
            );
        }

        /// Every construction path defaults the cap from the NAMED constant, and
        /// the builder overrides it. Scaled-down siblings above prove the
        /// behaviour; this proves the default they scale down FROM, without
        /// allocating 16 MiB in a unit test.
        #[test]
        fn every_constructor_defaults_the_cap_to_the_named_constant() {
            let url = Url::parse("http://127.0.0.1:1/").unwrap();
            let config = StreamableHttpTransportConfigBuilder::new(url).build();

            assert_eq!(
                StreamableHttpTransport::new(config.clone()).max_collected_body_bytes,
                DEFAULT_MAX_COLLECTED_BODY_BYTES,
                "`new` must default from the named constant"
            );
            assert_eq!(
                StreamableHttpTransport::new_with_http2(config.clone()).max_collected_body_bytes,
                DEFAULT_MAX_COLLECTED_BODY_BYTES,
                "`new_with_http2` must default from the named constant"
            );
            assert_eq!(
                StreamableHttpTransport::new(config)
                    .with_max_collected_body_bytes(CAP)
                    .max_collected_body_bytes,
                CAP,
                "the builder must override the default"
            );
        }

        /// The THIRD whole-body read — the v2 structured-error envelope — is
        /// capped too. An over-cap envelope is not an envelope: the caller falls
        /// back to the status-only transport error rather than allocating it.
        #[tokio::test]
        async fn an_over_cap_v2_error_envelope_falls_back_to_the_status_error() {
            let padding = "z".repeat(CAP);
            let body = format!(
                r#"{{"jsonrpc":"2.0","id":"abc","error":{{"code":-32602,"message":"{padding}"}}}}"#
            );
            assert!(body.len() > CAP);

            let mut server = MockServer::new_async().await;
            let _mock = server
                .mock("POST", "/")
                .with_status(400)
                .with_header("content-type", APPLICATION_JSON)
                .with_chunked_body(move |w| w.write_all(body.as_bytes()))
                .create_async()
                .await;

            let mut transport = capped_transport(&server.url(), CAP);
            transport.set_negotiated_protocol_version(Some(
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            ));
            let error = transport
                .send_raw(
                    br#"{"jsonrpc":"2.0","id":"abc","method":"tools/list","params":{}}"#.to_vec(),
                )
                .await
                .expect_err("an over-cap envelope cannot be surfaced structurally");
            assert!(
                error.to_string().contains("400"),
                "the status must survive: {error}"
            );
        }
    }

    // ==================================================================
    // Phase 118.2 plan 14 / ALWAYS-UNIT — every boundary of the two-sided
    // reconnect bound and both arms of the budget-refund predicate (CR-01).
    //
    // Each test is named for the PROPERTY it pins rather than for the
    // function it calls, so a failure names the contract that broke rather
    // than merely the symbol that moved.
    // ==================================================================

    mod reconnect_delay_bounds {
        use super::*;

        /// One millisecond, the step used either side of each threshold.
        const STEP: Duration = Duration::from_millis(1);

        #[test]
        fn a_peer_asking_for_zero_still_waits_the_floor() {
            assert_eq!(
                next_reconnect_delay(0, Some(Duration::ZERO)),
                MIN_SSE_RECONNECT_DELAY,
                "`retry: 0` is the CR-01 input: honoured verbatim it turns the reconnect loop \
                 into a request flood that also re-mints an access token per iteration"
            );
        }

        #[test]
        fn a_peer_value_below_the_floor_is_raised_to_it() {
            assert_eq!(
                next_reconnect_delay(0, Some(MIN_SSE_RECONNECT_DELAY - STEP)),
                MIN_SSE_RECONNECT_DELAY,
                "one millisecond under the floor is still under the floor"
            );
        }

        #[test]
        fn a_peer_value_exactly_at_the_floor_is_left_alone() {
            assert_eq!(
                next_reconnect_delay(0, Some(MIN_SSE_RECONNECT_DELAY)),
                MIN_SSE_RECONNECT_DELAY,
                "the bound is inclusive at its lower end"
            );
        }

        #[test]
        fn a_peer_value_just_above_the_floor_is_honoured_verbatim() {
            assert_eq!(
                next_reconnect_delay(0, Some(MIN_SSE_RECONNECT_DELAY + STEP)),
                MIN_SSE_RECONNECT_DELAY + STEP,
                "the floor must bound a hostile value, not overwrite a reasonable one — a peer \
                 that asks for a legitimate wait still gets the wait it asked for"
            );
        }

        #[test]
        fn a_peer_value_just_below_the_ceiling_is_honoured_verbatim() {
            assert_eq!(
                next_reconnect_delay(0, Some(MAX_SSE_RECONNECT_DELAY - STEP)),
                MAX_SSE_RECONNECT_DELAY - STEP,
                "the ceiling is likewise a bound, not an overwrite"
            );
        }

        #[test]
        fn a_peer_value_exactly_at_the_ceiling_is_left_alone() {
            assert_eq!(
                next_reconnect_delay(0, Some(MAX_SSE_RECONNECT_DELAY)),
                MAX_SSE_RECONNECT_DELAY,
                "the bound is inclusive at its upper end too"
            );
        }

        #[test]
        fn a_peer_value_above_the_ceiling_is_lowered_to_it() {
            assert_eq!(
                next_reconnect_delay(0, Some(MAX_SSE_RECONNECT_DELAY + STEP)),
                MAX_SSE_RECONNECT_DELAY,
                "an uncapped peer value parks a client's reader task for a duration the peer chose"
            );
        }

        #[test]
        fn a_saturating_peer_value_neither_panics_nor_escapes_the_ceiling() {
            assert_eq!(
                next_reconnect_delay(0, Some(Duration::MAX)),
                MAX_SSE_RECONNECT_DELAY,
                "`Duration::MAX` is reachable from the wire: `retry:` is parsed as u64 \
                 milliseconds and nothing about the parse bounds it"
            );
        }

        #[test]
        fn a_saturating_attempt_count_falls_back_to_the_ceiling() {
            assert_eq!(
                next_reconnect_delay(u32::MAX, None),
                MAX_SSE_RECONNECT_DELAY,
                "the exponential curve must SATURATE rather than overflow: an `unwrap` on the \
                 Duration conversion here would panic inside a client's reader task"
            );
        }

        #[test]
        fn the_computed_curve_never_falls_under_the_floor() {
            for attempt in 0..8u32 {
                let delay = next_reconnect_delay(attempt, None);
                assert!(
                    delay >= MIN_SSE_RECONNECT_DELAY && delay <= MAX_SSE_RECONNECT_DELAY,
                    "attempt {attempt} produced {delay:?}, outside the two-sided bound"
                );
            }
        }

        #[test]
        fn a_body_that_delivered_nothing_never_earns_a_fresh_budget() {
            assert!(
                !budget_reset_earned(false, RECONNECT_BUDGET_RESET_UPTIME),
                "uptime alone is not delivery: a stream that stayed up delivering nothing is a \
                 stream that is not working"
            );
            assert!(
                !budget_reset_earned(false, Duration::MAX),
                "and no amount of uptime changes that"
            );
        }

        #[test]
        fn a_one_frame_bounce_never_earns_a_fresh_budget() {
            assert!(
                !budget_reset_earned(true, Duration::ZERO),
                "this is the CR-01 shape exactly: one frame, then the body ends. Refunding here \
                 makes the reconnect loop unbounded for any budget value"
            );
            assert!(
                !budget_reset_earned(
                    true,
                    RECONNECT_BUDGET_RESET_UPTIME - Duration::from_millis(1)
                ),
                "one millisecond under the threshold is still a bounce"
            );
        }

        #[test]
        fn a_stream_that_stayed_up_and_delivered_earns_a_fresh_budget() {
            assert!(
                budget_reset_earned(true, RECONNECT_BUDGET_RESET_UPTIME),
                "the threshold is inclusive"
            );
            assert!(
                budget_reset_earned(true, RECONNECT_BUDGET_RESET_UPTIME * 120),
                "a stream that worked for an hour and then blinked must not inherit a spent \
                 budget — that is the case D-03 exists for"
            );
        }
    }

    // ==================================================================
    // Phase 118.2 / ALWAYS-PROPERTY — the incremental SSE reader under
    // arbitrary peer bytes.
    //
    // The in-tree, always-run half of the fuzz campaign that
    // `fuzz/fuzz_targets/streamable_sse_frames.rs` runs at length. Both drive
    // the SAME seam, `decode_sse_chunks_for_fuzz`, which is the PRODUCTION
    // decode sequence rather than a re-implementation of it — a target that
    // re-implements the code under test proves nothing about the code under
    // test.
    //
    // Deliberately NOT `#[ignore]`d, and so NOT selected by `make
    // test-property`'s `-- --ignored property_`. That mirrors the in-repo
    // precedent these are modelled on (`src/client/subscriptions.rs`'s
    // `proptest!` block, which is likewise always-run) and is strictly more
    // coverage: these arms run on every `cargo test` / `cargo nextest run`,
    // rather than only when someone remembers the property target.
    // ==================================================================

    mod sse_reader_properties {
        use super::*;

        /// The bound the property arms run the parser at.
        ///
        /// DELIBERATELY tiny, for the same reason the fuzz target's is:
        /// production bounds this path at 16 MiB
        /// ([`DEFAULT_MAX_COLLECTED_BODY_BYTES`]), and generated inputs of a few
        /// hundred bytes would never reach the discard-and-latch branch there.
        /// The branch is bound-agnostic, so a small bound loses no fidelity.
        const TINY_BOUND: usize = 64;

        /// Assert the two RETENTION bounds the reader must hold after every
        /// chunk.
        ///
        /// Non-vacuous by construction: it asserts a SIZE, not a latch. A
        /// "the overflow flag never clears" assertion cannot fail for any input
        /// at any bound, which is exactly how 20 000 green runs of the sibling
        /// campaign coexisted with an unbounded-growth defect.
        fn assert_retention_bounded(peaks: &[usize], tails: &[usize], bound: usize) {
            for (index, held) in peaks.iter().copied().enumerate() {
                assert!(
                    held <= bound,
                    "the parser retained {held} bytes after chunk {index} under a {bound}-byte \
                     bound (peaks: {peaks:?})"
                );
            }
            for (index, tail) in tails.iter().copied().enumerate() {
                assert!(
                    tail <= 3,
                    "the undecoded UTF-8 tail was {tail} bytes after chunk {index}; the longest \
                     incomplete character is 3 bytes, so anything more means take_utf8_prefix \
                     stopped draining (tails: {tails:?})"
                );
            }
        }

        proptest::proptest! {
            /// Arbitrary bytes from a peer never panic the reader, and never
            /// grow it past its bound.
            #[test]
            fn property_arbitrary_bytes_never_panic_or_grow_the_reader(
                bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
            ) {
                let (_outcomes, _overflowed, peaks, tails) =
                    decode_sse_chunks_for_fuzz(&[&bytes], TINY_BOUND);
                assert_retention_bounded(&peaks, &tails, TINY_BOUND);
            }

            /// The same, CHUNKED — so the SSE line buffer, the undecoded UTF-8
            /// tail and the overflow discard branch are all exercised across
            /// chunk boundaries, which is where a live stream actually splits.
            #[test]
            fn property_chunked_arbitrary_bytes_never_panic_or_grow_the_reader(
                bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512),
            ) {
                let chunks: Vec<&[u8]> = if bytes.is_empty() {
                    vec![&bytes[..]]
                } else {
                    bytes.chunks(7).collect()
                };
                let (_outcomes, _overflowed, peaks, tails) =
                    decode_sse_chunks_for_fuzz(&chunks, TINY_BOUND);
                assert_retention_bounded(&peaks, &tails, TINY_BOUND);
            }

            /// A VALID frame decodes to exactly one message no matter WHERE the
            /// peer splits it — including mid-character and mid-line.
            ///
            /// This is the arm that would catch a reader that lost or
            /// duplicated an event across a chunk boundary; the two above only
            /// bound memory.
            #[test]
            fn property_a_valid_frame_survives_any_chunk_split(
                split in 1usize..80,
            ) {
                // A multi-byte character inside the payload, so some splits land
                // mid-character.
                let frame = "id: e1\nevent: message\ndata: \
                             {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\
                             \"params\":{\"progressToken\":\"t\u{00e9}\",\"progress\":1}}\n\n";
                let raw = frame.as_bytes();
                let at = split.min(raw.len());
                let chunks: Vec<&[u8]> = vec![&raw[..at], &raw[at..]];
                let (outcomes, overflowed, _peaks, tails) =
                    decode_sse_chunks_for_fuzz(&chunks, DEFAULT_MAX_COLLECTED_BODY_BYTES);
                assert_eq!(
                    outcomes.len(),
                    1,
                    "a split at byte {at} yielded {} message(s), not 1",
                    outcomes.len()
                );
                assert!(
                    outcomes[0].is_ok(),
                    "a split at byte {at} corrupted the payload: {:?}",
                    outcomes[0]
                );
                assert!(
                    !overflowed.iter().any(|seen| *seen),
                    "a frame well under the bound must not overflow it"
                );
                assert!(
                    tails.iter().all(|tail| *tail <= 3),
                    "the undecoded UTF-8 tail must stay under one character: {tails:?}"
                );
            }

            /// The reconnect wait NEVER escapes its two-sided bound, for any
            /// attempt count and any peer-supplied `retry:` — including the
            /// zero, the saturating and the overflowing cases (CR-01,
            /// plan 14).
            ///
            /// Likewise NOT `#[ignore]`d, for the reason stated above this
            /// module: these run on every `cargo test` and every
            /// `cargo nextest run`, rather than only when someone remembers
            /// `make test-property`'s `-- --ignored property_` selector.
            #[test]
            fn property_next_reconnect_delay_stays_inside_both_bounds(
                attempt in proptest::prelude::any::<u32>(),
                retry_millis in proptest::option::of(proptest::prelude::any::<u64>()),
            ) {
                let server_retry = retry_millis.map(Duration::from_millis);
                let delay = next_reconnect_delay(attempt, server_retry);
                assert!(
                    delay >= MIN_SSE_RECONNECT_DELAY,
                    "attempt {attempt} with retry {retry_millis:?} produced {delay:?}, under the \
                     {MIN_SSE_RECONNECT_DELAY:?} floor — an unfloored wait is a request flood"
                );
                assert!(
                    delay <= MAX_SSE_RECONNECT_DELAY,
                    "attempt {attempt} with retry {retry_millis:?} produced {delay:?}, over the \
                     {MAX_SSE_RECONNECT_DELAY:?} ceiling — an uncapped wait parks the reader"
                );
            }

            /// The wait is a PURE function of its two arguments.
            ///
            /// The CONF-09 `idempotency` probe for the delay half: a
            /// scheduler decision that changed between two identical calls
            /// would make the reconnect schedule unreproducible, and
            /// unreproducible is exactly what a bounded schedule cannot be.
            /// The loop's own re-entry idempotency is bounded by the attempt
            /// counter, which plan 14 made monotonic by removing the
            /// unconditional refund.
            #[test]
            fn property_next_reconnect_delay_is_pure(
                attempt in proptest::prelude::any::<u32>(),
                retry_millis in proptest::option::of(proptest::prelude::any::<u64>()),
            ) {
                let server_retry = retry_millis.map(Duration::from_millis);
                assert_eq!(
                    next_reconnect_delay(attempt, server_retry),
                    next_reconnect_delay(attempt, server_retry),
                    "two identical calls disagreed for attempt {attempt}, retry {retry_millis:?}"
                );
            }
        }
    }
}
