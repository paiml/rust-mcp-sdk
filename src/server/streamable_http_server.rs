//! Streamable HTTP server implementation for MCP.
use crate::error::Result;
use crate::server::http_middleware::{
    adapters::{from_axum_with_limit, into_axum},
    ServerHttpContext, ServerHttpMiddlewareChain, ServerHttpResponse,
};
use crate::server::tower_layers::{AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer};
use crate::server::Server;
use crate::shared::http_constants::{
    APPLICATION_JSON, MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION, MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use crate::shared::TransportMessage;
use crate::types::{ClientRequest, Request};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::StreamExt;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// The v1 severance seam (SMPL-01 / SMPL-02).
//
// ONE module declaration, TWO source files, exactly one of which is compiled.
// `v1_session.rs` holds the real MCP 2025-11-25 session + SSE-resumability
// state; `v1_session_off.rs` is the null twin a `full-v2` build gets instead.
//
// Declaring the pair here — rather than sprinkling `#[cfg(feature =
// "v1-compat")]` through this 6,000-line file — means call sites below name
// `v1::…` unconditionally and never grow a feature gate of their own. A
// signature that drifts between the halves fails the build on one feature set,
// and `tests/v1_severability_tripwire.rs` covers the direction a build cannot
// see (the twin must declare nothing the real module does not).
//
// `#[path]` on a module declared in a non-`mod.rs` file resolves relative to
// THIS file's directory (`src/server/`), which is why both literals carry the
// `streamable_http_server/` prefix.
//
// `#[rustfmt::skip]` is load-bearing, not cosmetic: rustfmt explodes the
// `not(...)` form across four lines (it nests a list inside a list, unlike the
// positive form), and the severability tripwire matches the attribute as a
// single-line literal so a half-deleted pair is visible in a grep as well as in
// a build. Removing the skip silently defeats that match.
//
// `pub(crate)` on the module, not private: the items inside are `pub(crate)`
// (117-09 reaches them from `ServerState`), and `clippy::redundant_pub_crate`
// rejects `pub(crate)` items inside a PRIVATE module. Narrowing the module
// instead of the items would force `pub(super)` on both halves and lose the
// crate-level reachability the collapse needs.
// ---------------------------------------------------------------------------
#[rustfmt::skip]
#[cfg_attr(feature = "v1-compat", path = "streamable_http_server/v1_session.rs")]
#[cfg_attr(not(feature = "v1-compat"), path = "streamable_http_server/v1_session_off.rs")]
pub(crate) mod v1;

// ---------------------------------------------------------------------------
// The server-to-client channel (Phase 118.1 plan 10, CONF-07 / G-3).
//
// NOT a pair: inbound response correlation is era-agnostic, and the outbound
// half reaches the wire through `v1::route_to_session_stream`, whose zero-sized
// twin already answers "no stream" on a `full-v2` build. One module, both
// feature sets — see its own doc for the deadlock property it preserves.
// ---------------------------------------------------------------------------
pub(crate) mod peer_channel;

/// Event store trait for resumability support.
///
/// # This is NOT `crate::shared::event_store::EventStore`
///
/// There are TWO public traits called `EventStore` in this crate, and confusing
/// them is the obvious mistake. This one is transport-local, has THREE methods
/// (`store_event`, `replay_events_after`, `get_stream_for_event`), and is the
/// trait the crate-internal `v1::EventStoreHandle` alias erases for the v1
/// SSE-resumability path. The other
/// lives in `crate::shared::event_store`, has six methods, and is a separate
/// facility that plan 117-06 already gated behind `v1-compat` wholesale.
///
/// That path is a code span rather than an intra-doc link for the same reason as
/// `http_constants`'s `LAST_EVENT_ID`: this doc is UNGATED, the module it names
/// is not, so a link resolves to nothing under
/// `cargo doc --no-default-features --features full-v2` and rustdoc warns. Do not
/// "fix" any of the three spans below back into links.
///
/// # v1-only surface, deliberately NOT gated
///
/// Resumability exists only for MCP 2025-11-25 — the 2026-07-28 transport spec
/// states that resumable SSE streams via `Last-Event-ID` are not supported — so
/// this trait is v1-only surface. It is nonetheless compiled on BOTH feature
/// sets, and the reason is semver, not sequencing: this trait and
/// [`InMemoryEventStore`] are PUBLIC API, so REMOVING them is a major-version
/// change tracked as SMPL-F1 (pmcp 3.0).
///
/// What plan 117-13 gated is the config field that used to pin them
/// (`StreamableHttpServerConfig::event_store`) and every path that reaches
/// them; the type declarations stay nameable on both builds. See
/// `docs/v1-sunset-policy.md`.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event for later retrieval
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()>;

    /// Replay events after a given event ID
    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>>;

    /// Get stream ID for an event ID
    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>>;
}

/// Type alias for event list
type EventList = Vec<(String, TransportMessage)>;

/// Type alias for events map
type EventsMap = HashMap<String, EventList>;

/// In-memory event store implementation.
///
/// Implements this module's three-method [`EventStore`] trait — NOT the
/// six-method `crate::shared::event_store::EventStore`, and NOT the
/// same-named `crate::shared::event_store::InMemoryEventStore` (both spans, not
/// links: that module is gated behind `v1-compat` while this doc is not).
///
/// # Public path
///
/// This type is reachable at `pmcp::server::streamable_http_server::InMemoryEventStore`
/// and the example below exists to PIN that path: it is the concrete type the
/// public `StreamableHttpServerConfig::event_store` field takes, so moving or
/// re-exporting it elsewhere would be a MAJOR semver break. Phase 117 gates v1
/// surface without changing where any of it is reachable from on the default
/// (`v1-compat`) build, and this doctest fails to compile if that stops being
/// true.
///
/// The ungated half pins the TYPE PATH, which is public API on both feature sets
/// (see the `EventStore` trait doc for why removing it is a 3.0 change):
///
/// ```rust
/// use pmcp::server::streamable_http_server::InMemoryEventStore;
/// use std::sync::Arc;
///
/// let store = Arc::new(InMemoryEventStore::default());
/// assert_eq!(Arc::strong_count(&store), 1);
/// ```
#[cfg_attr(
    feature = "v1-compat",
    doc = r"
The `v1-compat` half pins the CONFIG WIRING, which is gated — this example does
not compile on `--no-default-features --features full-v2`, and that is the
severance being asserted rather than a bug:

```rust
use pmcp::server::streamable_http_server::{InMemoryEventStore, StreamableHttpServerConfig};
use std::sync::Arc;

let store = Arc::new(InMemoryEventStore::default());
let config = StreamableHttpServerConfig {
    event_store: Some(Arc::clone(&store)),
    ..Default::default()
};
assert!(config.event_store.is_some());
```
"
)]
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    /// Events by stream ID
    events: Arc<RwLock<EventsMap>>,
    /// Event ID to stream ID mapping
    event_to_stream: Arc<RwLock<HashMap<String, String>>>,
    /// Ordered list of all event IDs
    event_order: Arc<RwLock<Vec<String>>>,
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()> {
        let mut events = self.events.write();
        let stream_events = events.entry(stream_id.to_string()).or_default();
        stream_events.push((event_id.to_string(), message.clone()));

        self.event_to_stream
            .write()
            .insert(event_id.to_string(), stream_id.to_string());
        self.event_order.write().push(event_id.to_string());

        Ok(())
    }

    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>> {
        let event_order = self.event_order.read();
        let mut result = Vec::new();

        // Find the position of the last event
        let start_pos = event_order
            .iter()
            .position(|id| id == last_event_id)
            .map_or(0, |pos| pos + 1);

        // Collect all events after that position
        let events = self.events.read();
        let event_to_stream = self.event_to_stream.read();

        for i in start_pos..event_order.len() {
            let event_id = &event_order[i];
            if let Some(stream_id) = event_to_stream.get(event_id) {
                if let Some(stream_events) = events.get(stream_id) {
                    for (eid, msg) in stream_events {
                        if eid == event_id {
                            result.push((eid.clone(), msg.clone()));
                            break;
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>> {
        Ok(self.event_to_stream.read().get(event_id).cloned())
    }
}

/// Type alias for session callback.
///
/// v1-ONLY, and gated for a mechanical reason worth stating: its ONLY two uses
/// are `StreamableHttpServerConfig::on_session_initialized` and
/// `::on_session_closed`, both gated just below, so on a `full-v2` build the
/// alias is dead and `RUSTFLAGS="-D warnings"` says so. Plan 117-12 deferred it
/// here for exactly that reason — it could not be gated before the fields it
/// types were.
#[cfg(feature = "v1-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
type SessionCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Configuration for the streamable HTTP server.
///
/// # Four of these fields exist only on a `v1-compat` build
///
/// `session_id_generator`, `event_store`, `on_session_initialized` and
/// `on_session_closed` describe the MCP 2025-11-25 session lifecycle and its SSE
/// resumability. The 2026-07-28 transport is handshake-free and session-free and
/// states outright that resumable streams via `Last-Event-ID` are not supported,
/// so on a build without `v1-compat` those four fields are not merely unused —
/// they are not compiled, and neither is the machinery behind them (SMPL-02).
///
/// `enable_json_response`, `http_middleware`, `allowed_origins` and
/// `max_request_bytes` are era-neutral and present on every build.
///
/// ## Semver, stated plainly (plan 117-13, assumption A7)
///
/// Removing a public field is normally a MAJOR break. It is safe here for exactly
/// one reason: the build that lacks them, `full-v2`, is a brand-new feature that
/// no published consumer builds with. Every shipped configuration enables
/// `v1-compat` — it is in `default` and in `full` — so no existing code loses a
/// field.
///
/// **That argument expires the moment `full-v2` enters any published crate's
/// default feature set.** At that point this gating becomes a semver break and
/// must be scheduled as one (SMPL-F1, pmcp 3.0). Do not widen `full-v2`'s reach
/// without re-reading this paragraph. The policy is `docs/v1-sunset-policy.md`.
///
/// # Examples
///
/// Era-neutral configuration — compiles on every build:
///
/// ```rust
/// use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
///
/// // Name only the shared fields and let the rest default. Functional-update
/// // syntax is what keeps this example compiling on `full-v2`, where the four
/// // session fields do not exist to be named.
/// let config = StreamableHttpServerConfig {
///     enable_json_response: true,
///     max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
///     ..Default::default()
/// };
/// assert!(config.enable_json_response);
///
/// // For serverless / Lambda, prefer the constructor over a literal: it also
/// // picks the right CORS posture.
/// let stateless = StreamableHttpServerConfig::stateless();
/// assert!(stateless.enable_json_response);
/// ```
#[cfg_attr(
    feature = "v1-compat",
    doc = r#"
Stateful MCP 2025-11-25 configuration — `v1-compat` builds only:

```rust
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;

let config = StreamableHttpServerConfig {
    session_id_generator: Some(Box::new(|| {
        format!("session-{}", uuid::Uuid::new_v4())
    })),
    on_session_initialized: Some(Box::new(|session_id| {
        println!("Session started: {}", session_id);
    })),
    on_session_closed: Some(Box::new(|session_id| {
        println!("Session ended: {}", session_id);
    })),
    ..Default::default()
};
assert!(config.session_id_generator.is_some());
assert!(config.on_session_closed.is_some());
```
"#
)]
pub struct StreamableHttpServerConfig {
    /// Function to generate session IDs (None for stateless mode).
    ///
    /// v1-ONLY: 2026-07-28 has no session to mint an id for.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub session_id_generator: Option<Box<dyn Fn() -> String + Send + Sync>>,
    /// Enable JSON responses instead of SSE
    pub enable_json_response: bool,
    /// Event store for resumability (using concrete type for object safety).
    ///
    /// v1-ONLY: 2026-07-28 does not support resumable streams via
    /// `Last-Event-ID`, so there is nothing for a store to replay. Its presence
    /// here is also what pinned [`InMemoryEventStore`] into both builds until
    /// this field was gated.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub event_store: Option<Arc<InMemoryEventStore>>,
    /// Callback when session is initialized.
    ///
    /// v1-ONLY: there is no `initialize` handshake on 2026-07-28.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub on_session_initialized: Option<SessionCallback>,
    /// Callback when session is closed.
    ///
    /// v1-ONLY: there is no session to close, and `DELETE /` answers `405`.
    #[cfg(feature = "v1-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "v1-compat")))]
    pub on_session_closed: Option<SessionCallback>,
    /// HTTP middleware chain for request/response processing
    pub http_middleware: Option<Arc<ServerHttpMiddlewareChain>>,
    /// Allowed origins for CORS responses.
    ///
    /// When `Some`, replaces wildcard `*` with origin-locked CORS that
    /// reflects the request's `Origin` only when it appears in this set.
    /// When `None`, defaults to [`AllowedOrigins::localhost()`] at runtime.
    ///
    /// Used by the `StreamableHttpServer` path. The `pmcp::axum::router()`
    /// path uses [`crate::server::axum_router::RouterConfig::allowed_origins`]
    /// instead.
    pub allowed_origins: Option<AllowedOrigins>,
    /// Maximum request body size in bytes.
    ///
    /// Requests exceeding this limit are rejected with HTTP 413 before
    /// any JSON parsing occurs. Default: 4 MB (matches AWS API Gateway).
    pub max_request_bytes: usize,
}

impl std::fmt::Debug for StreamableHttpServerConfig {
    /// Written as STATEMENTS rather than a `.field(..).field(..)` chain.
    ///
    /// An attribute cannot be attached to one link of a method chain, so gating
    /// the four v1-only rows needs each of them to be its own statement. The
    /// alternative — two whole `fmt` bodies behind opposing `#[cfg]`s — would
    /// duplicate the four shared rows and let the two copies drift.
    ///
    /// The rendered field ORDER is unchanged on a `v1-compat` build, so this is
    /// not an observable change for any existing consumer.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("StreamableHttpServerConfig");
        #[cfg(feature = "v1-compat")]
        out.field("session_id_generator", &self.session_id_generator.is_some());
        out.field("enable_json_response", &self.enable_json_response);
        #[cfg(feature = "v1-compat")]
        out.field("event_store", &self.event_store.is_some());
        #[cfg(feature = "v1-compat")]
        out.field(
            "on_session_initialized",
            &self.on_session_initialized.is_some(),
        );
        #[cfg(feature = "v1-compat")]
        out.field("on_session_closed", &self.on_session_closed.is_some());
        out.field("http_middleware", &self.http_middleware.is_some());
        out.field("allowed_origins", &self.allowed_origins);
        out.field("max_request_bytes", &self.max_request_bytes);
        out.finish()
    }
}

impl Default for StreamableHttpServerConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "v1-compat")]
            session_id_generator: Some(Box::new(|| Uuid::new_v4().to_string())),
            enable_json_response: false,
            #[cfg(feature = "v1-compat")]
            event_store: Some(Arc::new(InMemoryEventStore::default())),
            #[cfg(feature = "v1-compat")]
            on_session_initialized: None,
            #[cfg(feature = "v1-compat")]
            on_session_closed: None,
            http_middleware: None,
            allowed_origins: None,
            max_request_bytes: crate::server::limits::DEFAULT_MAX_REQUEST_BYTES,
        }
    }
}

impl StreamableHttpServerConfig {
    /// Create a stateless configuration — no sessions, JSON responses.
    /// Ideal for Lambda and serverless deployments.
    /// Create a stateless configuration for serverless/Lambda deployments.
    ///
    /// Uses [`AllowedOrigins::any()`] because stateless servers are behind
    /// a reverse proxy (API Gateway, `CloudFront`) that handles CORS and
    /// origin validation at the edge. DNS rebinding protection adds no
    /// security value when the MCP server is only reachable via loopback
    /// within a Lambda sandbox or container.
    ///
    /// For servers directly exposed to the internet, use `Default::default()`
    /// instead (which defaults to `AllowedOrigins::localhost()`).
    pub fn stateless() -> Self {
        // Every field is named EXHAUSTIVELY — deliberately, not by oversight.
        // `..Default::default()` would be two `#[cfg]`s shorter, but functional
        // update syntax evaluates the base struct in full before moving the
        // non-overridden fields, so it would heap-allocate an
        // `Arc<InMemoryEventStore>` and a boxed UUID closure on every call and
        // immediately drop both. This is the serverless/Lambda constructor; a
        // cosmetic attribute count is not worth pure allocation waste on it.
        Self {
            #[cfg(feature = "v1-compat")]
            session_id_generator: None,
            enable_json_response: true,
            #[cfg(feature = "v1-compat")]
            event_store: None,
            #[cfg(feature = "v1-compat")]
            on_session_initialized: None,
            #[cfg(feature = "v1-compat")]
            on_session_closed: None,
            http_middleware: None,
            allowed_origins: Some(AllowedOrigins::any()),
            max_request_bytes: crate::server::limits::DEFAULT_MAX_REQUEST_BYTES,
        }
    }
}

/// Server state shared across routes.
#[derive(Clone)]
pub(crate) struct ServerState {
    server: Arc<tokio::sync::Mutex<Server>>,
    config: Arc<StreamableHttpServerConfig>,
    /// Pre-resolved allowed origins for CORS and DNS rebinding protection.
    allowed_origins: AllowedOrigins,
    /// Everything that exists ONLY for MCP 2025-11-25: the session map, the live
    /// SSE fan-out those sessions address, and the resumability event store.
    ///
    /// One field, not three, and its type comes from the `v1` paired module — so
    /// on a `full-v2` build it is a zero-sized twin and this struct allocates no
    /// session map at all. That is the STRUCTURAL half of SMPL-02 (D-03 / D-10):
    /// a property of the type, not of a runtime branch someone can forget to
    /// take. Call sites hand this field to a `v1::` operation and never reach
    /// past it: nothing outside the pair touches a session map, a stream map or
    /// the event store, and every operation returns an OWNED answer rather than
    /// a borrow the zero-sized twin could not produce.
    v1: v1::V1State,
    /// The server-to-client channel: the correlation authority, its outbound
    /// drain, and the per-session peer handles (CONF-07 / G-3).
    ///
    /// HERE, on `ServerState`, and deliberately NOT inside `Mutex<Server>`. A
    /// tool handler parked on a peer call holds that mutex for its whole
    /// duration, so a dispatcher reachable only through it could not be reached
    /// by the very response that would release the handler — the same deadlock
    /// in a different costume (T-118.1-10-01). `peer_channel`'s module doc
    /// records the property in full.
    peer_channel: peer_channel::PeerChannel,
}

/// Build the base MCP Router without any Tower layers applied.
///
/// Used by both [`StreamableHttpServer::start()`] and `pmcp::axum::router()`.
pub(crate) fn build_mcp_router(state: ServerState) -> Router<()> {
    Router::new()
        .route("/", post(handle_post_request))
        .route("/", get(handle_get_sse))
        .route("/", delete(handle_delete_session))
        .with_state(state)
}

/// Create a [`ServerState`] for the MCP router.
///
/// Used by `pmcp::axum::router()` to construct state without a full
/// [`StreamableHttpServer`].
pub(crate) fn make_server_state(
    server: Arc<tokio::sync::Mutex<Server>>,
    config: StreamableHttpServerConfig,
) -> ServerState {
    let allowed_origins = config
        .allowed_origins
        .clone()
        .unwrap_or_else(AllowedOrigins::localhost);
    // THE single `V1State` construction site, and deliberately `#[cfg]`-free:
    // the paired module supplies whichever half the feature set selected, and on
    // `full-v2` this line allocates nothing. It runs before `config` is moved
    // into the `Arc` because the real half type-erases `config.event_store` on
    // the way in.
    let v1 = v1::V1State::new(&config);
    let state = ServerState {
        server,
        config: Arc::new(config),
        allowed_origins,
        v1,
        peer_channel: peer_channel::PeerChannel::new(),
    };
    // Start the outbound server-to-client drain if we are already inside a Tokio
    // runtime. `pmcp::axum::router()` reaches this function synchronously and may
    // not be, in which case this declines and `StreamableHttpServer::start()`
    // makes the same idempotent call from inside one.
    peer_channel::ensure_outbound_drain(&state);
    state
}

/// A streamable HTTP server for MCP.
pub struct StreamableHttpServer {
    addr: SocketAddr,
    state: ServerState,
}

impl std::fmt::Debug for StreamableHttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpServer")
            .field("addr", &self.addr)
            .field("state", &"ServerState { ... }")
            .finish()
    }
}

/// Helper function to create JSON-RPC error response.
///
/// CORS headers are added by the `CorsLayer` Tower middleware, so this
/// function no longer needs to handle them.
fn create_error_response(status: StatusCode, code: i32, message: &str) -> Response {
    let error_body = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": null
    });

    (status, Json(error_body)).into_response()
}

// ===========================================================================
// v2 required-header gate (Plan 112-06, VERS-05 / D-05 / D-06 / D-11).
//
// The v2 verdict is Plan 04's RESOLVED `ProtocolContext.era`, CONSUMED here —
// this layer never runs a second independent era resolver (Pitfall 2). The
// streamable-HTTP inbound handler resolves the context ONCE (for this gate) and
// threads that SAME value into `Server::handle_request_with_context`, so
// dispatch is a pass-through, not a re-resolve.
//
// The classifier is decomposed into small single-responsibility helpers, each
// well under cognitive-complexity 25 (PMAT CI gate — WARNING 4), composed by a
// thin top-level `classify_v2_request`. Every new header-violation error sources
// its JSON-RPC code from `error_codes::` (VERS-06); no new bare -326xx literal.
// ===========================================================================

/// Upper bound on a RAW `Mcp-Name`/`Mcp-Method` header, which may carry the
/// `=?base64?…?=` sentinel expansion of a value bounded by
/// [`MAX_V2_HEADER_VALUE_LEN`]. Re-exported for the same single-source reason.
use crate::types::mrtr::MAX_HEADER_SENTINEL_LEN as MAX_V2_HEADER_SENTINEL_LEN;
/// Upper bound on a header value we will consider (`DoS` guard, T-112-13).
///
/// Re-exported from `types::mrtr` rather than redeclared: the ingress bound and the
/// `Mcp-Name` sentinel decoder's bound MUST be the same number, or a value in the gap
/// is admitted here and then rejected there as a malformed sentinel.
use crate::types::mrtr::MAX_HEADER_VALUE_LEN as MAX_V2_HEADER_VALUE_LEN;

// ---------------------------------------------------------------------------
// The resumability handle (Plan 113-08, HTTP-05).
//
// The era gates that decide whether sessions and resumability are live for a
// request moved into the `v1` paired module in plan 117-09: they are now
// `v1::sessions_active`, `v1::apply_session_header`, `v1::resumability_active`,
// `v1::resumability_store` and the two pure `_for` rules. Call them through
// `v1::`, unconditionally — there is no `#[cfg]` at any call site in this file.
// (`active_session_generator` was a seventh; plan 117-12 moved both of its
// callers into the pair, so it is now private to `v1_session.rs` and this file
// never names it.)
//
// `EventStoreHandle` is declared in `v1_session.rs`, not here: its only
// remaining users are in the real half, and on a `full-v2` build an alias
// declared here would be dead under `RUSTFLAGS="-D warnings"`. Its own rustdoc
// on the alias explains why, so it is not restated here.
//
// The [`EventStore`] trait, [`InMemoryEventStore`] and the `LAST_EVENT_ID`
// constant in `crate::shared::http_constants` are still compiled on BOTH feature
// sets and gated at their own declaration sites. They cannot live in the pair:
// the first two are public API whose path the `pub(crate)` pair would change, the
// public `StreamableHttpServerConfig::event_store` field pins the concrete store,
// and `InMemoryEventStore` is in the tripwire's `FORBIDDEN_STATE_TYPES` so the
// null twin may never declare it.
//
// Removal — as opposed to gating — is SMPL-F1 (pmcp 3.0), governed by
// `docs/v1-sunset-policy.md`.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Direct-response id ownership (Plan 113-08, HTTP-05).
//
// # The invariant, scoped precisely
//
//   Every DIRECT response to a live request carries THAT request's id, on BOTH
//   eras. A REPLAYED HISTORICAL EVENT is not a direct response and legitimately
//   retains its ORIGINAL id.
//
// The scoping is load-bearing. Stated as "every response id equals the live
// request id on both eras" the claim contradicts v1 resumability, whose entire
// purpose is to re-emit past events unchanged — so a literal implementation
// would either break v1 replay or make the assertion vacuous. The two behaviors
// are deliberately separated here so they are never conflated again:
//
//   * DIRECT response  -> assembled through `envelope_for_live_request`
//   * HISTORICAL event -> re-emitted verbatim by `v1::replay_sse_events_from_header`
//
// MRTR independently reinforces the direct half: a retry MUST use a different
// JSON-RPC id, so any id replay becomes immediately visible to the client.
//
// # Audit — every site in this transport that assembles, clones, caches or
// # stores a response, and its verdict
//
// | Site | Kind | Verdict |
// |------|------|---------|
// | `handle_fast_path_request` | direct | routed through `envelope_for_live_request` with the id captured at ingress |
// | `dispatch_message_with_middleware` (Public Request arm) | direct | routed through `envelope_for_live_request` |
// | `assemble_discover_response_fast` | direct | routed through `envelope_for_live_request` |
// | `assemble_discover_response_with_middleware` | direct | routed through `envelope_for_live_request` |
// | `build_response` | framing | dispatches an ALREADY-constructed envelope by transport mode; constructs none of its own |
// | `build_json_response` / `build_sse_response_from_single_message` | framing | serialize/frame one already-constructed envelope; construct none of their own |
// | `build_success_response_with_middleware` | framing | serializes one already-constructed envelope |
// | `v1::route_to_session_stream` inside `build_response` | routing | gated on `sessions_on`, so a v2 reply can never be handed to another caller's stream (the T-113-07 fix) |
// | `v1::store_response_event` | caching | gated on `resumability_active`; on v1 it retains a whole envelope, which is CORRECT — that is the historical-event record replay re-emits |
// | `v1::sse_event_for_message` | caching | same gate, same verdict |
// | `v1::replay_sse_events_from_header` | historical | re-emits stored events verbatim, ORIGINAL ids intact — intentional, and asserted by `v1_replayed_event_retains_original_id` |
// | `create_error_response_with_id` + `v2_gate_reject_response` + `map_unparsed_body_for_v2` | direct (error) | cannot use the constructor: `RequestId` has no `Null` variant and a JSON-RPC error for an unparseable body legitimately carries `id: null`. Their id comes from `raw_request_id(<the LIVE body>)`, never from a cache, so the invariant holds by construction |
// | `create_error_response` | direct (error) | pre-dispatch transport failure with no live id at all; emits `id: null`, unchanged since before v2 |
//
// No site was found reusing an envelope for a direct response. One site WAS
// found handing a direct response to the WRONG caller — the SSE-stream route
// above — and it is fixed in `build_response`.
// ---------------------------------------------------------------------------

/// **The ONE constructor for a direct JSON-RPC response envelope on this
/// transport.**
///
/// It takes the PAYLOAD (the `result`/`error` value) and the LIVE request id as
/// SEPARATE arguments, so a caller physically cannot pass a whole cached envelope
/// through and have its stale id survive. That argument shape is the actual
/// guarantee; the `debug_assert!` below is only belt and braces.
///
/// A source-audit comment plus a `debug_assert!` would catch a regression solely
/// in debug builds and solely if someone ran the right test (Codex Plan-08
/// MEDIUM). Making the id a mandatory, separately-supplied parameter makes the
/// stale-id response unconstructible instead.
///
/// This is deliberately NOT applied to a replayed historical event: see the
/// audit block above.
fn envelope_for_live_request(
    payload: crate::types::jsonrpc::ResponsePayload<serde_json::Value, crate::types::JSONRPCError>,
    live_id: crate::types::RequestId,
) -> crate::types::JSONRPCResponse {
    // No `debug_assert_eq!` that the response carries `live_id`: this function
    // CONSTRUCTS the response from `live_id`, so the assertion could not fail for
    // any input — the argument shape IS the guarantee. It also cost a `RequestId`
    // clone (a heap `String` for the UUID ids this transport uses) on every
    // direct response, because `debug_assert_eq!` compiles to a runtime `false`
    // branch rather than `#[cfg]` — so the binding survived into release builds.
    match payload {
        crate::types::jsonrpc::ResponsePayload::Result(result) => {
            crate::types::JSONRPCResponse::success(live_id, result)
        },
        crate::types::jsonrpc::ResponsePayload::Error(error) => {
            crate::types::JSONRPCResponse::error(live_id, error)
        },
    }
}

/// The decoded `MCP-Protocol-Version` header, classified for the era matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderProtocolVersion {
    /// Header not present.
    Absent,
    /// Present but non-UTF-8 or oversized — decoded without panicking.
    Malformed,
    /// Exactly `2026-07-28` (the v2 era).
    V2,
    /// Any other decodable value (v1 or unknown).
    Other,
}

/// The verdict of the v2 `_meta` gate over the RAW `MCP-Protocol-Version` header
/// and the RAW `params._meta` object.
///
/// # Three-way, deliberately — this replaced a 2x2 era matrix
///
/// The predecessor, `classify_era_cell`, was a 2x2 over
/// `(header_is_v2, meta_is_v2)`. That shape can express only "the two sides
/// agree" or "the two sides disagree", so an ABSENT required `_meta` key and a
/// PRESENT-but-disagreeing `protocolVersion` collapsed into the same
/// `HEADER_MISMATCH` cell. The spec allocates them DIFFERENT codes — `-32602`
/// for a missing required parameter, `-32020` for a header/body disagreement —
/// so the distinction has to exist in the type (Phase 118.1, gap G-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V2MetaVerdict {
    /// No v2 signal on either side, or a `_meta` carrier only the shared
    /// resolver can diagnose — defer to `resolve_raw_meta_protocol_context`
    /// (v1 passthrough, `MalformedMeta`, or the accept list).
    Defer,
    /// A REQUIRED reserved `_meta` key is ABSENT → `INVALID_PARAMS`.
    MissingRequired(&'static str),
    /// The header and `_meta` disagree about the protocol version →
    /// `HEADER_MISMATCH`. Evaluated BEFORE the accept list (gap G-8).
    Disagreement(&'static str),
    /// Both sides agree on `2026-07-28` → enforce the v2 header contract.
    Enforce,
}

// ---------------------------------------------------------------------------
// v2 HTTP status mapping (Plan 113-04, HTTP-01).
//
// The transport spec turns several JSON-RPC error codes into specific HTTP
// statuses on the v2 path — most notably "If the server does not implement the
// requested RPC method, it MUST respond with 404 Not Found and a JSON-RPC error
// with code -32601", which pmcp answered at HTTP 200 (v1 behavior) before this
// plan.
//
// The mapper below is CODE-driven, never call-site-driven: -32021 is emitted by
// dispatch (plan 09), not by the header gate, and a code that reaches the wire
// from anywhere must map identically. It is also era-gated: on v1 / a
// non-opted-in server every status is exactly what it was before.
// ---------------------------------------------------------------------------

/// The HTTP status the v2 transport requires for a JSON-RPC error `code`.
///
/// Values come from the centralized table (VERS-06); the per-constant rustdoc in
/// `error_codes.rs` is the single documented source for each mapping. Anything
/// not listed is handler semantics rather than a transport-layer rejection and
/// stays at HTTP 200 with the JSON-RPC error in the body.
fn v2_status_for_code(code: i32) -> StatusCode {
    use crate::types::protocol::error_codes as ec;
    match code {
        ec::METHOD_NOT_FOUND => StatusCode::NOT_FOUND,
        ec::HEADER_MISMATCH
        | ec::MISSING_REQUIRED_CLIENT_CAPABILITY
        | ec::UNSUPPORTED_PROTOCOL_VERSION
        | ec::PARSE_ERROR
        | ec::INVALID_REQUEST
        | ec::INVALID_PARAMS => StatusCode::BAD_REQUEST,
        _ => StatusCode::OK,
    }
}

/// Era-gated status for an error `code`: v2 uses [`v2_status_for_code`], every
/// other era keeps `v1_status` byte-for-byte.
fn status_for_error(
    era: Option<crate::types::protocol::Era>,
    code: i32,
    v1_status: StatusCode,
) -> StatusCode {
    if matches!(era, Some(crate::types::protocol::Era::V2)) {
        v2_status_for_code(code)
    } else {
        v1_status
    }
}

/// The JSON-RPC `id` of a raw request body, or `Null` when it has none.
///
/// Used so a v2 error envelope built BEFORE (or INSTEAD OF) a successful typed
/// parse still carries the ORIGINAL request id — HTTP-05 depends on it and plan
/// 08 asserts it. Never panics on adversarial input.
fn raw_request_id(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(serde_json::Value::Null)
}

/// Build a JSON-RPC error response with an explicit id and optional structured
/// `data`.
///
/// The v2 counterpart of [`create_error_response`], which hardcodes `id: null`.
/// Kept separate so no v1 response byte changes: only v2 paths call this.
fn create_error_response_with_id(
    status: StatusCode,
    id: serde_json::Value,
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> Response {
    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("message".to_string(), json!(message));
    if let Some(data) = data {
        error.insert("data".to_string(), data);
    }
    // Built through a `Map` rather than the `json!` macro because the macro
    // BORROWS its interpolated values, which would leave `id` passed-by-value
    // but never consumed.
    let mut body = serde_json::Map::new();
    body.insert("jsonrpc".to_string(), json!("2.0"));
    body.insert("error".to_string(), serde_json::Value::Object(error));
    body.insert("id".to_string(), id);
    (status, Json(serde_json::Value::Object(body))).into_response()
}

/// Re-map a pre-dispatch parse rejection onto the v2 status table.
///
/// The typed parse is where an UNKNOWN METHOD surfaces: `parse_request_or_internal`
/// answers `Error::method_not_found` for any method string that matches no
/// `ClientRequest` / `ServerRequest` variant, which the transport stringifies into
/// an "Invalid request" parse failure. On v1 that has always been HTTP 400 with
/// `-32700` and `id: null`, and it stays exactly that.
///
/// On v2 the spec is explicit: "If the server does not implement the requested RPC
/// method, it MUST respond with `404 Not Found` and a JSON-RPC error with code
/// `-32601`." A body whose method never deserializes therefore cannot be diagnosed
/// from an already-built TYPED response — this mapping has to happen at the RAW
/// level, from the body bytes, which is what this function does.
///
/// The era is resolved from the RAW `params._meta` (the same read the
/// `server/discover` ingress uses) because no typed request exists to read it
/// from. A body that is not a well-formed JSON-RPC request, or a server that is
/// not opted into v2, or a v1 request, all keep `v1_response` untouched.
///
/// KNOWN LIMITATION: a KNOWN method whose params fail to deserialize also reaches
/// `method_not_found` at this seam and is therefore reported as `-32601`/404 on
/// v2 rather than `-32602`/400. Distinguishing the two requires a method-string
/// table this layer does not own; plan 06 (MRTR param parse errors) adds the
/// precise per-parameter mapping.
///
/// # Its interaction with v2 METHOD RETIREMENT (Phase 118.1 plan 05)
///
/// Stated precisely, because the imprecise version is misleading in both
/// directions. This function runs on the PARSE-FAILURE branch of the ingress, so
/// it sits EARLIER in the pipeline than [`run_v2_header_gate`], not later — the
/// five [`V2_RETIRED_METHODS`] do not "short-circuit before" it. What is true is
/// that a retired method reaches `-32601`/404 on v2 by TWO disjoint routes:
///
/// * params that do NOT deserialize — this function, via the limitation above;
/// * params that DO deserialize — [`retire_v2_method`] at the header gate, which
///   is the route the variant-keyed predicate it replaced could never take.
///
/// Both routes emit the same code and the same status, so for those five methods
/// the limitation above is no longer observable on the wire; only the message
/// text differs. That coincidence is exactly why the official suite's
/// `initialize` and `logging/setLevel` retirement checks passed for the WRONG
/// reason before plan 05, and why `tests/v2_retired_methods.rs` — which sends
/// WELL-FORMED params — is the artifact that actually proves the retirement.
async fn map_unparsed_body_for_v2(
    state: &ServerState,
    raw_body: &[u8],
    v1_response: Response,
) -> Response {
    use crate::types::protocol::error_codes::METHOD_NOT_FOUND;
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(raw_body) else {
        return v1_response;
    };
    // Only a well-formed JSON-RPC REQUEST (method + id) can be an unknown-method
    // rejection; anything else keeps the v1 parse-error response.
    let Some(method) = envelope.get("method").and_then(serde_json::Value::as_str) else {
        return v1_response;
    };
    if envelope.get("id").is_none() {
        return v1_response;
    }
    // The SAME reader the header gate uses, so an unknown method is classified
    // against exactly the era its sibling requests would get. Reads the
    // ALREADY-PARSED `envelope` above rather than re-parsing `raw_body` — this
    // is an attacker-supplied body, and parsing it twice per request bought
    // nothing.
    let raw_meta = params_meta_of(Some(&envelope));
    let resolved = {
        let server = state.server.lock().await;
        server.resolve_raw_meta_protocol_context(raw_meta)
    };
    let Ok(Some(context)) = resolved else {
        return v1_response;
    };
    if context.era != crate::types::protocol::Era::V2 {
        return v1_response;
    }
    create_error_response_with_id(
        v2_status_for_code(METHOD_NOT_FOUND),
        envelope
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        METHOD_NOT_FOUND,
        &format!("Method not found: {method}"),
        None,
    )
}

/// The v2 status a built JSON-RPC response must carry, or `None` to keep the
/// status the response already has.
///
/// This is the CODE-driven half of the mapper: `MISSING_REQUIRED_CLIENT_CAPABILITY`
/// (-32021) is emitted by dispatch (plan 09), not by the header gate, so the
/// mapping cannot be attached at rejection call sites — it has to read the code
/// that is actually about to reach the wire.
fn v2_dispatch_response_status(
    era: Option<crate::types::protocol::Era>,
    response: &crate::types::JSONRPCResponse,
) -> Option<StatusCode> {
    if !matches!(era, Some(crate::types::protocol::Era::V2)) {
        return None;
    }
    let crate::types::jsonrpc::ResponsePayload::Error(ref error) = response.payload else {
        return None;
    };
    Some(v2_status_for_code(error.code))
}

/// Assemble the response for a [`V2GateOutcome::Reject`].
///
/// The status is code-driven via [`status_for_error`] with a `400` v1 floor (the
/// gate only rejects requests that already carry a v2 signal on one side, and
/// `400` is what Phase 112 returned for every such cell). The id is recovered
/// from the RAW body so a rejection that happened before — or instead of — a
/// successful typed parse still echoes the client's id.
fn v2_gate_reject_response(
    raw_body: &[u8],
    era: Option<crate::types::protocol::Era>,
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> Response {
    let status = status_for_error(era, code, StatusCode::BAD_REQUEST);
    create_error_response_with_id(status, raw_request_id(raw_body), code, message, data)
}

/// Outcome of the whole v2 gate for one request.
enum V2GateOutcome {
    /// Not a v2 request (v1 / non-opted-in) — dispatch normally, no v2 headers.
    Passthrough,
    /// Accepted v2 request — dispatch, then echo these headers outbound.
    EnforceOk { method: String, name: String },
    /// Rejected — build a 4xx structured JSON-RPC error with this code/message
    /// and, when the code defines one, a structured `error.data` payload.
    ///
    /// `data` is not optional decoration: `UNSUPPORTED_PROTOCOL_VERSION`
    /// (`-32022`) MUST carry a `supported` array so the client can pick a
    /// mutually supported version instead of probing, and
    /// `MISSING_REQUIRED_CLIENT_CAPABILITY` (`-32021`, emitted by dispatch in
    /// plan 09) MUST carry an object-shaped `requiredCapabilities`. A
    /// `(code, message)` pair alone cannot express either.
    Reject {
        code: i32,
        message: String,
        data: Option<serde_json::Value>,
    },
}

/// Decode the `MCP-Protocol-Version` header without panicking (T-112-13).
fn decode_version_header(headers: &HeaderMap) -> HeaderProtocolVersion {
    let Some(raw) = headers.get(MCP_PROTOCOL_VERSION) else {
        return HeaderProtocolVersion::Absent;
    };
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN {
        return HeaderProtocolVersion::Malformed;
    }
    match raw.to_str() {
        Err(_) => HeaderProtocolVersion::Malformed,
        Ok(s) if s == crate::types::protocol::PROTOCOL_VERSION_2026_07_28 => {
            HeaderProtocolVersion::V2
        },
        Ok(_) => HeaderProtocolVersion::Other,
    }
}

/// Read a header as a bounded UTF-8 string, or `None` if absent/malformed.
///
/// The bound is [`MAX_V2_HEADER_SENTINEL_LEN`], not `MAX_V2_HEADER_VALUE_LEN`:
/// `Mcp-Name` legitimately travels in the `=?base64?…?=` sentinel form, which is
/// a 4/3 expansion of the logical name. Admitting only the smaller bound here
/// would reject a conformant request whose name is within
/// `MAX_V2_HEADER_VALUE_LEN` but whose sentinel is not, which
/// [`crate::types::mrtr::decode_header_value`] would then never get to see. The
/// amplification bound is still enforced, on the DECODED value, by that decoder.
fn bounded_header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?;
    if raw.as_bytes().len() > MAX_V2_HEADER_SENTINEL_LEN {
        return None;
    }
    raw.to_str().ok().map(str::to_string)
}

// ---------------------------------------------------------------------------
// The v2 `_meta` required-key + agreement rule (Phase 118.1 plan 06, CONF-06,
// gaps G-6 and G-8).
//
// Every message below is a `&'static str` CONSTANT. None is ever assembled from
// peer-supplied bytes (T-118.1-06-01): the only client-controlled value that
// reaches the wire from this rule is `error.data.requested` on the accept-list
// rejection, which the conformance suite REQUIRES to echo the requested version
// and which is a plain string field, never interpolated into a message.
// ---------------------------------------------------------------------------

/// Rejection when a v2 request carries no `params._meta` object at all.
const ERR_META_ABSENT: &str = "v2 requests must carry a params._meta object with \
     io.modelcontextprotocol/protocolVersion and io.modelcontextprotocol/clientCapabilities";

/// Rejection when `params._meta` omits the reserved protocol-version key.
const ERR_META_NO_PROTOCOL_VERSION: &str =
    "params._meta is missing the required io.modelcontextprotocol/protocolVersion key";

/// Rejection when `params._meta` omits the reserved client-capabilities key.
///
/// `io.modelcontextprotocol/clientInfo` deliberately has NO counterpart: it is a
/// SHOULD, and the conformance suite has a dedicated MUST-SERVE check
/// (`RequestMetaClientInfoOptional`) for a request that omits it. Making the
/// required-key rule symmetric over all three reserved keys turns that PASSING
/// check red.
const ERR_META_NO_CLIENT_CAPABILITIES: &str =
    "params._meta is missing the required io.modelcontextprotocol/clientCapabilities key";

/// Rejection when the header claims v2 and `_meta` names a different version.
const ERR_HEADER_CLAIMS_V2: &str =
    "MCP-Protocol-Version header claims v2 but _meta protocolVersion disagrees";

/// Rejection when `_meta` claims v2 and the header does not.
const ERR_META_CLAIMS_V2: &str =
    "_meta claims v2 but MCP-Protocol-Version header is absent or not 2026-07-28";

/// The `io.modelcontextprotocol/protocolVersion` STRING in a RAW `_meta` value.
///
/// `None` covers three distinct inputs on purpose, because they are separated by
/// the two predicates below rather than here: no `_meta` at all, a `_meta` that
/// is not an object, and a version that is not a string.
fn raw_meta_protocol_version(meta: Option<&serde_json::Value>) -> Option<&str> {
    meta?
        .as_object()?
        .get(crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY)?
        .as_str()
}

/// Whether a RAW `_meta` value is an OBJECT that carries `key` at all.
///
/// Presence, not deserializability: an unusable VALUE at a present key is the
/// shared resolver's `MalformedMeta` (already `INVALID_PARAMS`), while an ABSENT
/// key is this layer's `MissingRequired`. Conflating them is the collapse G-6 is.
fn raw_meta_object_has_key(meta: Option<&serde_json::Value>, key: &str) -> bool {
    meta.and_then(serde_json::Value::as_object)
        .is_some_and(|object| object.contains_key(key))
}

/// The THREE-WAY verdict over the header and the RAW `params._meta`.
///
/// Pure, total, and non-panicking over adversarial input — it reads only an
/// already-decoded header classification and an already-parsed JSON value.
///
/// # The rule, in evaluation order
///
/// 1. **Neither side claims v2** → [`V2MetaVerdict::Defer`]. The v1 path and the
///    accept-list path are both downstream, and this rule is V2-ONLY: the shared
///    `resolve_protocol_context` serves BOTH eras and its absent-version arm IS
///    the v1 fallback, so a required-key rule planted there would reject every v1
///    request (T-118.1-06-03).
/// 2. **`params._meta` absent entirely** → `MissingRequired`.
/// 3. **`_meta` present but the protocol-version key absent** → `MissingRequired`.
///    A `_meta` that is not an object, or a version that is not a string, is
///    `Defer`red to the resolver's `MalformedMeta`, which already answers
///    `INVALID_PARAMS`.
/// 4. **The two sides name different versions** → `Disagreement`. This runs
///    BEFORE the accept list, which is the whole of gap G-8: an unsupported
///    version that DISAGREES with the header is a header/body disagreement
///    (`-32020`), and only an unsupported version the header AGREES with is an
///    accept-list rejection (`-32022`).
/// 5. Otherwise → [`V2MetaVerdict::Enforce`].
///
/// `clientCapabilities` is deliberately NOT checked here — see
/// [`require_v2_client_capabilities`] for where it lands and why.
fn classify_v2_meta_version(
    header: HeaderProtocolVersion,
    raw_meta: Option<&serde_json::Value>,
) -> V2MetaVerdict {
    use crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;
    let header_is_v2 = matches!(header, HeaderProtocolVersion::V2);
    let meta_version = raw_meta_protocol_version(raw_meta);
    let meta_is_v2 = meta_version == Some(crate::types::protocol::PROTOCOL_VERSION_2026_07_28);

    if !header_is_v2 && !meta_is_v2 {
        return V2MetaVerdict::Defer;
    }
    let Some(meta) = raw_meta else {
        return V2MetaVerdict::MissingRequired(ERR_META_ABSENT);
    };
    let Some(version) = meta_version else {
        // Exactly one shape is a missing-required-key rejection: an OBJECT that
        // simply lacks the key. The other two — key present but unusable, and a
        // non-object `_meta` — are both the resolver's MalformedMeta, so they
        // defer. Binding `meta` above means this no longer re-tests an `Option`
        // the line before it already proved is `Some`.
        return match meta.as_object() {
            Some(object) if !object.contains_key(RESERVED_PROTOCOL_VERSION_KEY) => {
                V2MetaVerdict::MissingRequired(ERR_META_NO_PROTOCOL_VERSION)
            },
            _ => V2MetaVerdict::Defer,
        };
    };
    if !(header_is_v2 && version == crate::types::protocol::PROTOCOL_VERSION_2026_07_28) {
        return V2MetaVerdict::Disagreement(if header_is_v2 {
            ERR_HEADER_CLAIMS_V2
        } else {
            ERR_META_CLAIMS_V2
        });
    }
    V2MetaVerdict::Enforce
}

/// Require `io.modelcontextprotocol/clientCapabilities` on an ACCEPTED v2
/// request.
///
/// # Why this is a SEPARATE step, evaluated LATE
///
/// The two checks in [`classify_v2_meta_version`] are prerequisites for
/// resolving the era at all: with no `protocolVersion` there is no era, and
/// without an era "is this method retired on v2?" is not a well-formed question.
/// `clientCapabilities` is different — it is a params requirement on a request
/// whose era is already settled, so it must NOT preempt
/// [`retire_v2_method`]. A method the 2026-07-28 schema REMOVED has no params
/// contract to violate; answering `-32602 your params are wrong` for a method
/// that does not exist would be both misleading and a regression against the
/// conformance suite's five `HttpServerMethodNotFound404*` checks.
///
/// The status is NOT chosen here: `INVALID_PARAMS` reaches the wire through
/// [`v2_status_for_code`], which maps it to `400`.
fn require_v2_client_capabilities(
    outcome: V2GateOutcome,
    raw_meta: Option<&serde_json::Value>,
) -> V2GateOutcome {
    if !matches!(outcome, V2GateOutcome::EnforceOk { .. }) {
        return outcome;
    }
    if raw_meta_object_has_key(
        raw_meta,
        crate::types::protocol::context::RESERVED_CLIENT_CAPABILITIES_KEY,
    ) {
        return outcome;
    }
    V2GateOutcome::Reject {
        code: crate::types::protocol::error_codes::INVALID_PARAMS,
        message: ERR_META_NO_CLIENT_CAPABILITIES.to_string(),
        data: None,
    }
}

/// Rejection when a v2 request omits one of the two UNIVERSALLY required headers.
///
/// Deliberately does NOT name `Mcp-Name`: since Phase 118 D-13 that header is
/// required only on name-bearing methods, so a message naming it here would send
/// an operator looking for the wrong missing header.
const ERR_MISSING_V2_HEADERS: &str =
    "v2 requests must carry Mcp-Method and MCP-Protocol-Version headers";

/// Rejection when a NAME-BEARING v2 method omits `Mcp-Name`.
///
/// Distinct from [`ERR_MISSING_V2_HEADERS`] so a rejection names its actual
/// cause. Collapsing the two back into one catch-all is a regression that
/// `require_v2_headers_truth_table` fails on.
const ERR_MISSING_MCP_NAME: &str =
    "Mcp-Name header is required: this method carries a routing name";

/// Require the v2 headers (VERS-05 / D-05); return `(method, name)`.
///
/// # The `Mcp-Name` header rule (Phase 118 D-13, as widened by D-18)
///
/// > `Mcp-Name` MUST be present on name-bearing methods — `tools/call` /
/// > `prompts/get` → `params.name`; `resources/read` → `params.uri`;
/// > `tasks/get` / `tasks/update` / `tasks/cancel` → `params.taskId` — and is
/// > OPTIONAL, and IGNORED, on every other v2 method.
///
/// This function enforces the PRESENCE half; [`cross_check_name`] enforces the
/// VALUE half and returns `Ok` immediately for a non-name-bearing method. Both
/// resolve "is this method name-bearing?" through the ONE shared predicate
/// [`is_name_bearing_method`], so the two halves cannot disagree.
///
/// # Phase-113 DRIFT-1 is REVERSED here (Phase 118 D-13)
///
/// The Phase-113 DRIFT-1 adjudication deliberately kept a STRICTER rule than the
/// transport spec: `Mcp-Name` had to be PRESENT on every v2 request (empty for a
/// name-less method), on the reasoning that a header a WAF can rely on always
/// being present is worth more than matching the laxer spec wording.
///
/// **Phase 118 D-13 reverses that adjudication.** The 2026-07-28 transport spec
/// requires the header only for name-bearing methods, and the official
/// `@modelcontextprotocol/conformance` suite emits it only for those. The
/// stricter rule therefore rejected effectively the ENTIRE v2 scored set with
/// `-32020` **before dispatch** — a conformant `tools/list` never reached a
/// handler (Phase 118 RESEARCH, Pitfall 1). Spec conformance won.
///
/// What is RETAINED: `Mcp-Method` and `MCP-Protocol-Version` stay mandatory on
/// every v2 request ([`cross_check_method`] is unchanged), and the strict
/// name/body cross-check in [`cross_check_name`] is unchanged wherever a name
/// exists. The relaxation is scoped by the name table, not by the caller.
///
/// # The D-18 widening
///
/// [`is_name_bearing_method`] now resolves through the COMBINED table
/// [`crate::types::mrtr::name_bearing_key`], so `tasks/get` / `tasks/update` /
/// `tasks/cancel` are VALIDATED as well as emitted. Before Phase 118 the client
/// emitted an `Mcp-Name` for those methods that the server never required and
/// never cross-checked — an emitter/validator asymmetry that contradicted D-13's
/// own principle.
///
/// # Backward compatibility with the Phase-113 client
///
/// A client that still emits `Mcp-Name: ""` for a name-less method is ACCEPTED:
/// absent and empty converge on the same carried value, because a stray value on
/// a non-name-bearing method is discarded (see the sanitization note below).
fn require_v2_headers(headers: &HeaderMap) -> std::result::Result<(String, String), &'static str> {
    // The two UNIVERSALLY-required headers, checked adjacently. Both failures
    // return the same error, so there is nothing to be gained by interleaving
    // them with the `Mcp-Method` extraction.
    if headers.get(MCP_PROTOCOL_VERSION).is_none() {
        return Err(ERR_MISSING_V2_HEADERS);
    }
    let Some(method) = bounded_header_str(headers, MCP_METHOD) else {
        return Err(ERR_MISSING_V2_HEADERS);
    };
    if !is_name_bearing_method(&method) {
        // SANITIZATION (Phase 118 D-20). The carried name is echoed straight back
        // out by `apply_v2_outbound_headers`, so whatever a client sent on a
        // method that carries no routing name is DISCARDED here rather than
        // propagated downstream or reflected — echoing an unvalidated,
        // attacker-supplied string is a pointless surface. It also makes an
        // absent `Mcp-Name` and a Phase-113 client's `Mcp-Name: ""` converge on
        // exactly the same carried value.
        return Ok((method, String::new()));
    }
    match bounded_header_str(headers, MCP_NAME) {
        Some(name) => Ok((method, name)),
        None => Err(ERR_MISSING_MCP_NAME),
    }
}

/// Cross-check `Mcp-Method` against the JSON-RPC body `method` (D-06).
fn cross_check_method(
    mcp_method: &str,
    body_method: Option<&str>,
) -> std::result::Result<(), &'static str> {
    match body_method {
        Some(bm) if bm == mcp_method => Ok(()),
        _ => Err("Mcp-Method header does not match the JSON-RPC body method"),
    }
}

/// Whether `method` carries a ROUTING NAME — the one predicate that decides both
/// whether `Mcp-Name` is required and whether its value is cross-checked (D-06).
///
/// # This is the SAME table the client's `Mcp-Name` emitter resolves through
///
/// [`crate::types::mrtr::name_bearing_key`] is the COMBINED table, and its own
/// rustdoc names it as the emitter's resolver
/// (`src/shared/streamable_http.rs`). Reading it here means the two ends of the
/// cross-check cannot disagree about which methods carry a name or which params
/// key holds it. It covers:
///
/// - `tools/call`, `prompts/get` → `params.name`
/// - `resources/read` → `params.uri`
/// - `tasks/get`, `tasks/update`, `tasks/cancel` → `params.taskId`
///
/// # Phase 118 D-18: this used to read the NARROWER table
///
/// Before Phase 118 this resolved through `crate::types::mrtr::logical_name_key`,
/// which covers only the three MRTR methods. The client already emitted an
/// `Mcp-Name` for `tasks/*` (through `name_bearing_key`) that the server neither
/// required nor cross-checked — an emitter/validator asymmetry that contradicted
/// the "required exactly where a method carries a routing name" principle D-13
/// is built on. D-18 closes it by pointing both ends at one table.
fn is_name_bearing_method(method: &str) -> bool {
    crate::types::mrtr::name_bearing_key(method).is_some()
}

/// Cross-check `Mcp-Name` against the request's logical name for name-bearing
/// methods (D-06). Name-less methods carry no name at all — [`require_v2_headers`]
/// has already discarded any value a client sent for one.
///
/// # The sentinel decode is load-bearing
///
/// A logical name that is not header-safe (non-ASCII, or containing an RFC 9110
/// field-value delimiter) MUST travel in the `=?base64?<b64>?=` sentinel form. A
/// verbatim comparison would therefore reject a legitimate conformant request, so
/// the header value is decoded through the SHARED codec
/// [`crate::types::mrtr::decode_header_value`] — the same one the client emitter
/// uses — before it is compared. A value that starts the sentinel but does not
/// decode is a malformed header, i.e. a `HEADER_MISMATCH` rejection, never a
/// silent pass.
fn cross_check_name(
    mcp_name: &str,
    method: &str,
    body_name: Option<&str>,
) -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) {
        return Ok(());
    }
    let Some(decoded) = crate::types::mrtr::decode_header_value(mcp_name) else {
        return Err("Mcp-Name header is a malformed =?base64?...?= sentinel value");
    };
    match body_name {
        Some(bn) if bn == decoded => Ok(()),
        _ => Err("Mcp-Name header does not match the request's logical name"),
    }
}

/// The thin top-level classifier over the full matrix (cog-safe composition).
///
/// The ONE construction of a header/body-mismatch rejection.
///
/// Two sites answer a `HEADER_MISMATCH`: [`run_v2_header_gate`] short-circuits a
/// [`V2MetaVerdict::Disagreement`] before the accept list (the G-8 ordering fix),
/// and [`classify_v2_request`]'s ENFORCE arm rejects a missing required header or
/// a failed cross-check. They must stay byte-identical on the wire, so they share
/// this constructor rather than each spelling out the same three fields.
///
/// Note the reachability asymmetry this makes visible: because the gate returns at
/// the disagreement check, [`classify_v2_request`] never sees a `Disagreement` in
/// production — that arm survives for the unit and property tests that call the
/// classifier directly over the full matrix.
fn header_mismatch_reject(message: &str) -> V2GateOutcome {
    V2GateOutcome::Reject {
        code: crate::types::protocol::error_codes::HEADER_MISMATCH,
        message: message.to_string(),
        data: None,
    }
}

/// Inputs: the [`V2MetaVerdict`] already computed from the header and the RAW
/// `params._meta` + the untrusted body `method`/`params.name`. Output: accept
/// (with echo headers) | reject(code) | passthrough. Pure and non-panicking —
/// property-tested.
///
/// The era verdict is a PARAMETER rather than something this function re-derives:
/// [`run_v2_header_gate`] computes it once, before the accept list, and threads
/// the same value here (D-11 / Pitfall 2 — one resolution per request).
fn classify_v2_request(
    headers: &HeaderMap,
    verdict: V2MetaVerdict,
    body_method: Option<&str>,
    body_name: Option<&str>,
) -> V2GateOutcome {
    use crate::types::protocol::error_codes::INVALID_PARAMS;
    // Every rejection the ENFORCE arm can produce is a missing-required-header
    // or a header/body mismatch, so they all carry `HEADER_MISMATCH` and no
    // structured `data`. The required-`_meta`-key rejections carry
    // `INVALID_PARAMS` instead — that difference is gap G-6.
    let reject = header_mismatch_reject;
    match verdict {
        V2MetaVerdict::Defer => V2GateOutcome::Passthrough,
        V2MetaVerdict::MissingRequired(msg) => V2GateOutcome::Reject {
            code: INVALID_PARAMS,
            message: msg.to_string(),
            data: None,
        },
        V2MetaVerdict::Disagreement(msg) => reject(msg),
        V2MetaVerdict::Enforce => {
            let (method, name) = match require_v2_headers(headers) {
                Ok(pair) => pair,
                Err(msg) => return reject(msg),
            };
            if let Err(msg) = cross_check_method(&method, body_method) {
                return reject(msg);
            }
            if let Err(msg) = cross_check_name(&name, &method, body_name) {
                return reject(msg);
            }
            V2GateOutcome::EnforceOk { method, name }
        },
    }
}

// ---------------------------------------------------------------------------
// v2 METHOD RETIREMENT (Phase 118.1 plan 05, CONF-05 / gap G-5).
//
// The 2026-07-28 core schema REMOVES five RPCs. Retirement is keyed on the
// method NAME STRING, not on a parsed `ClientRequest` variant: the variant match
// this replaced could only ever see requests whose params had already
// deserialized, which is not the shape a conformance client sends.
// ---------------------------------------------------------------------------

/// The per-request `_meta` key that REPLACES the `logging/setLevel` RPC.
///
/// Spelled here because `src/` owns no constant for it: the key is read by
/// APPLICATION code off the request metadata, never by a core dispatch arm, so
/// no production module has claimed it. It exists as a named constant purely so
/// the retirement table below has one `&'static str` per replacement and the
/// rejection message can never be assembled from peer-supplied bytes.
const LOG_LEVEL_REQUEST_META_KEY: &str = "io.modelcontextprotocol/logLevel";

/// The MCP 2025-11-25 RPC whose only purpose is to set a session's log level.
///
/// Spelled once and shared by the retirement table below and by the v1 capture
/// in [`capture_v1_set_level`]. Two literals would be two chances to disagree
/// about which method this is — and they would disagree SILENTLY, because a
/// capture keyed on a slightly different string simply never fires.
///
/// Matched by EXACT BYTE EQUALITY everywhere: no case folding, no trimming, no
/// prefix matching, the same rule [`v2_retired_method`] applies and for the same
/// reason (T-118.1-05-01). The method string arrives from an untrusted peer.
const LOG_LEVEL_SET_METHOD: &str = "logging/setLevel";

/// THE retirement set: every RPC the MCP `2026-07-28` core schema removes, paired
/// with the mechanism that replaces it (`None` where the schema offers none).
///
/// # One home, deliberately
///
/// Both POST entrypoints reach this table through the SINGLE
/// [`run_v2_header_gate`] call, so the fast path and the middleware path cannot
/// disagree about which methods are gone. That single-home property is the same
/// one the dispatch-level `dispatch_request_or_retire` seam used to provide; the
/// predicate simply moved earlier, to where the method STRING is available.
///
/// # Why exactly these five
///
/// Corroborated two independent ways:
///
/// 1. The vendored `schema/vendored/core-2026-07-28/schema.ts` method inventory
///    lists 21 methods, and all five below are ABSENT from it.
/// 2. The pinned official conformance suite iterates exactly this list and
///    requires HTTP `404` plus JSON-RPC `-32601` for each.
///
/// `notifications/initialized` is likewise absent from the schema and is
/// deliberately NOT here: it is a NOTIFICATION, so it carries no JSON-RPC id and
/// has no response envelope in which a `-32601` could be delivered.
///
/// # This is one of TWO homes for "does this method exist on this era"
///
/// The other is the tasks-extension predicate in
/// [`crate::server::task_dispatch`] — `V2_TASKS_METHOD_RETIRED` and the
/// `tasks_*_serves_on_era` functions — which keys on a typed `ClientRequest`
/// arm at the SHARED dispatch layer rather than on a method string at the HTTP
/// ingress. Both produce `METHOD_NOT_FOUND` and both map to 404, so they are the
/// same rule wearing two vocabularies.
///
/// **Which home does a new retirement belong in?**
///
/// - A method removed from the CORE `schema.ts` inventory → this table.
/// - A method scoped to an EXTENSION (tasks, and anything that follows it) →
///   the dispatch-side predicate, next to that extension's own era rules.
///
/// The split is deliberate but has a consequence worth knowing before you pick:
/// era is resolved from `params._meta`, which is transport-independent, while
/// this table lives in the HTTP transport. So a v2-opted-in stdio or WebSocket
/// server retires `tasks/list` (shared dispatch) yet still serves the five
/// below. Unifying them means hoisting this table to dispatch, which is blocked
/// on the ordering constraint documented at [`retire_v2_method`]: retirement
/// MUST run after `cross_check_method`, or a header/body smuggling attempt
/// becomes an indistinguishable routine 404.
const V2_RETIRED_METHODS: [(&str, Option<&str>); 5] = [
    (
        "initialize",
        Some(crate::types::protocol::SERVER_DISCOVER_METHOD),
    ),
    // No replacement: v2 has no liveness RPC at all. See `v2_retirement_message`.
    ("ping", None),
    (LOG_LEVEL_SET_METHOD, Some(LOG_LEVEL_REQUEST_META_KEY)),
    (
        "resources/subscribe",
        Some(crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD),
    ),
    (
        "resources/unsubscribe",
        Some(crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD),
    ),
];

/// The [`V2_RETIRED_METHODS`] entry `method` names, matched by EXACT BYTE
/// EQUALITY.
///
/// No case folding, no trimming, no prefix matching, no Unicode normalization
/// (T-118.1-05-01). The method string arrives from an untrusted peer at a point
/// EARLIER and less validated than typed parsing, so any leniency here is a way
/// to force — or to dodge — a retirement the literal wire value does not ask for.
/// `Initialize`, `` ping`` with a leading space and `logging/setlevel` are all
/// NOT retired, and a unit test asserts it.
///
/// Returns the TABLE's own `&'static str`, never the caller's slice, so a
/// rejection message built from this can never echo peer-supplied bytes
/// (T-118.1-05-02).
fn v2_retired_method(method: &str) -> Option<(&'static str, Option<&'static str>)> {
    V2_RETIRED_METHODS
        .iter()
        .copied()
        .find(|(retired, _)| *retired == method)
}

/// The `-32601` message a retired method answers with.
///
/// Both halves are `&'static str` from [`V2_RETIRED_METHODS`]. `ping` has no
/// replacement clause because the 2026-07-28 transport removed the liveness RPC
/// outright rather than relocating it, and inventing a substitute here would send
/// an operator looking for a method that does not exist.
fn v2_retirement_message(retired: &'static str, replacement: Option<&'static str>) -> String {
    replacement.map_or_else(
        || format!("Method not found: {retired} (retired in MCP 2026-07-28)"),
        |replacement| {
            format!("Method not found: {retired} (retired in MCP 2026-07-28; use {replacement})")
        },
    )
}

// ---------------------------------------------------------------------------
// The PER-REQUEST LOG LEVEL (Phase 118.2 plan 07, CONF-10 / D-10 / D-11 / D-12).
//
// Two eras, two sources, one answer:
//
//   * v1 sends the `logging/setLevel` RPC, which is captured HERE and stored
//     PER SESSION in `v1::V1State` (D-11);
//   * v2 retired that RPC and carries `io.modelcontextprotocol/logLevel` in the
//     request's `params._meta` instead, read per request (D-10);
//   * neither → `None`, and `DEFAULT_LOG_LEVEL` (`info`, D-12) applies at emit
//     time.
//
// # Why the capture is at the HTTP INGRESS and not in the dispatch arm
//
// The ingress is the ONE point that holds all four facts at once: the resolved
// era, the validated session id, the raw body, and a still-owned
// `ProtocolContext`. Resolving here and writing the ANSWER onto the context makes
// the read at emit time an `Option<LoggingLevel>` copy instead of a session-map
// lock per record — and it means the v1 dispatch arm (plan 08) has nothing to do
// but answer `Ok(json!({}))`.
//
// # Why the level is NOT stored on the server
//
// `ServerState::server` is one `Arc<tokio::sync::Mutex<Server>>` shared by every
// session. A level held there would let client B's `setLevel` change client A's
// filtering (T-118.2-07-01). The per-session home is also the correct lifetime:
// `setLevel` is a v1 RPC, and on a `full-v2` build `V1State` is a zero-sized twin
// that allocates nothing for a mechanism that build does not have.
//
// # `ServerCapabilities.logging` deliberately gates NOTHING here
//
// The sink is per request and the capability is advisory (RESEARCH Open
// Question 6). Gating on it would make a correctly-plumbed server silently mute
// because of a declaration it forgot to make.
// ---------------------------------------------------------------------------

/// Parse a PEER-SUPPLIED level value, or ignore it.
///
/// Deserializes through [`LoggingLevel`](crate::types::LoggingLevel)'s OWN serde
/// mapping (`rename_all = "lowercase"`), so this parse can never disagree with
/// the spelling the wire uses — a hand-written `match` over eight strings would
/// be a second mapping free to drift from the first.
///
/// # Malformed input is IGNORED, never echoed, never fatal
///
/// Three properties, each deliberate (T-118.2-07-03 / T-118.2-07-04, ASVS V5/V7):
///
/// * **No panic.** `Deserialize` is fallible and the error is discarded.
/// * **No echo.** The peer's bytes are never placed into an error message or a
///   log line, following this crate's established rule for errors derived from
///   peer input (`collected_body_over_cap`, `listen_overflow`). Nothing is
///   returned but a typed value the server already knew how to name.
/// * **No rejection.** A misspelled level falls back to the `info` default
///   rather than becoming a `-32602`. The key is an ADVISORY, per-request
///   diagnostic hint; failing a whole tool call because a hint was misspelled
///   converts a logging preference into an availability failure. The v1
///   `logging/setLevel` RPC is a request whose only purpose IS the level, so it
///   is the arguable case — but the 2026-07-28 conformance suite pins its
///   response to a literal `{}` (Pitfall 8), so it also stores-or-ignores and
///   still answers `{}`.
fn parse_peer_log_level(value: Option<&serde_json::Value>) -> Option<crate::types::LoggingLevel> {
    <crate::types::LoggingLevel as serde::Deserialize>::deserialize(value?).ok()
}

/// Store the level a v1 `logging/setLevel` request asked for, against ITS session.
///
/// A no-op unless ALL of the following hold, which is what keeps the write
/// session-scoped rather than server-scoped:
///
/// * sessions are live for this request (so never on v2, where the RPC is
///   retired and the era carries no session at all);
/// * the request has an already-VALIDATED session id — the transport passes the
///   id `v1::resolve_session_for_request` accepted, so an unknown id was answered
///   `404` long before this runs;
/// * the body's `method` is EXACTLY [`LOG_LEVEL_SET_METHOD`];
/// * `params.level` parses (see [`parse_peer_log_level`]).
///
/// The request then CONTINUES to dispatch unchanged — this is a capture, not a
/// short circuit. The stored level takes effect from this request onward,
/// including for any record the `setLevel` call's own dispatch arm emits.
///
/// `v1::set_session_log_level` is itself a no-op for an unknown session id, so
/// the denial-of-service control (T-118.2-07-02) holds even if a future caller reaches this
/// with an unvalidated id.
fn capture_v1_set_level(
    state: &ServerState,
    sessions_on: bool,
    session_id: Option<&str>,
    body: Option<&serde_json::Value>,
) {
    if !sessions_on {
        return;
    }
    let Some(session_id) = session_id else {
        return;
    };
    let method = body.and_then(|body| body.get("method")).and_then(|method| {
        method
            .as_str()
            .filter(|method| *method == LOG_LEVEL_SET_METHOD)
    });
    if method.is_none() {
        return;
    }
    let Some(level) = parse_peer_log_level(params_of(body).get("level")) else {
        return;
    };
    v1::set_session_log_level(&state.v1, session_id, level);
}

/// THE minimum log level for this request, captured and resolved in one place.
///
/// # Precedence
///
/// 1. **v2** — `params._meta["io.modelcontextprotocol/logLevel"]`, when the era
///    is [`Era::V2`](crate::types::protocol::Era). Per request; v2 is
///    session-free by construction, so nothing is stored and nothing can leak
///    into another request.
/// 2. **v1** — the level this SESSION last set, when sessions are live for the
///    request and it carries a session id.
/// 3. Otherwise **`None`**: leave `ProtocolContext::resolved_log_level` unset.
///
/// # `None` means two different things, and the ERA decides which
///
/// On **v1** it is "nothing overrode the default", so `DEFAULT_LOG_LEVEL`
/// (`info`, D-12) applies at emit time — MCP 2025-11-25 says the server MAY
/// decide which messages to send automatically.
///
/// On **v2** it is a PROHIBITION. SEP-2575 is explicit: *"If absent, the server
/// MUST NOT send any notifications/message"*. This function still answers `None`
/// for that case — the distinction is not expressible in its return type, and
/// inventing a tri-state here would push the era rule into every reader of
/// `resolved_log_level`. It is applied ONCE, at the far end, by
/// [`attach_request_log_sink`](crate::server::core::attach_request_log_sink),
/// which withholds the log SINK entirely for a v2 request with no resolved
/// level; a sinkless emit is silence (D-08). See that function for why the sink
/// rather than the level carries the rule.
///
/// The two arms cannot both apply: arm 2's `sessions_on` is `false` on v2 by
/// [`v1::sessions_active`], which is why this reads as a precedence list rather
/// than as a conflict rule.
///
/// The v1 CAPTURE runs first, unconditionally in source, so a `logging/setLevel`
/// is recorded before arm 2 reads it back — and so the twin's no-op write is
/// reached (rather than compiled out) on a `full-v2` build.
///
/// Called at BOTH POST ingress paths. See either call site for why "both or
/// neither" is load-bearing (T-118.2-07-06).
fn resolve_request_log_level(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    sessions_on: bool,
    session_id: Option<&str>,
    body: &PostBody<'_>,
) -> Option<crate::types::LoggingLevel> {
    // The SAME parse the v2 gate already paid for, serving both the capture and
    // the `_meta` read. Adversarial or non-JSON bytes yield `None` and every read
    // below simply finds nothing.
    let body = body.json();

    capture_v1_set_level(state, sessions_on, session_id, body);

    if era == Some(crate::types::protocol::Era::V2) {
        return parse_peer_log_level(
            params_meta_of(body).and_then(|meta| meta.get(LOG_LEVEL_REQUEST_META_KEY)),
        );
    }
    if sessions_on {
        return session_id.and_then(|session_id| v1::session_log_level(&state.v1, session_id));
    }
    None
}

/// Turn an ACCEPTED v2 request for a retired method into its `-32601` rejection.
///
/// # Why this runs AFTER the header matrix, not before it
///
/// It consults only [`V2GateOutcome::EnforceOk`], which
/// [`classify_v2_request`] produces only once `cross_check_method` has confirmed
/// the `Mcp-Method` header and the JSON-RPC body method are the SAME string. A
/// retirement decided before that cross-check would let a peer send
/// `Mcp-Method: tools/call` with a body method of `ping` and collect a `404`
/// instead of the `-32020` header/body disagreement — turning a smuggling signal
/// into a routine-looking refusal. Ordering it here keeps the desync fail-closed.
///
/// The status is NOT chosen here: `METHOD_NOT_FOUND` reaches the wire through
/// [`v2_status_for_code`], which maps it to `404`. A call-site status choice is
/// exactly the drift Phase 113 removed.
fn retire_v2_method(outcome: V2GateOutcome) -> V2GateOutcome {
    let V2GateOutcome::EnforceOk { ref method, .. } = outcome else {
        return outcome;
    };
    let Some((retired, replacement)) = v2_retired_method(method) else {
        return outcome;
    };
    V2GateOutcome::Reject {
        code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        message: v2_retirement_message(retired, replacement),
        data: None,
    }
}

/// Extract the untrusted `(method, logical-name)` pair from the raw JSON-RPC body.
///
/// Re-parses the raw bytes (the transport parse already succeeded) so the
/// cross-check compares the header against the LITERAL wire value a WAF would see
/// — the smuggling-relevant view (D-06). Never panics.
///
/// The logical name is resolved METHOD-AWARELY because different name-bearing
/// methods carry it in different params keys:
/// - `tools/call` → `params.name`
/// - `prompts/get` → `params.name`
/// - `resources/read` → `params.uri` (a [`ReadResourceRequest`](crate::types::ReadResourceRequest)
///   has a `uri` field and NO `name` field, so reading `params.name` would always
///   yield `None` and wrongly reject a standards-shaped `resources/read`)
/// - any other method → `None` (presence-only; `cross_check_name` returns Ok for
///   non-name-bearing methods)
///
/// Production goes through [`method_and_name_of`] instead: since Phase 113 plan
/// 06 the gate parses the raw body EXACTLY ONCE and shares that value with the
/// era read, this cross-check and the MRTR params read. This byte-slice wrapper
/// survives as the test entry point, so the existing wire-shape assertions keep
/// exercising the parse-and-read pair end to end.
#[cfg(test)]
fn extract_body_method_and_name(body: &[u8]) -> (Option<String>, Option<String>) {
    method_and_name_of(raw_body_json(body).as_ref())
}

/// [`extract_body_method_and_name`] over an ALREADY-PARSED body.
///
/// The gate parses the raw body exactly once and hands the value to each reader,
/// so the era read, the header cross-check and the MRTR params read can never
/// disagree about what the body says.
fn method_and_name_of(value: Option<&serde_json::Value>) -> (Option<String>, Option<String>) {
    let Some(value) = value else {
        return (None, None);
    };
    // Read through the ONE shared routing-pair reader — the same function the
    // CLIENT emits its `Mcp-Method` / `Mcp-Name` from. These two are halves of a
    // single cross-check; deriving them separately is how they drift.
    // Non-name-bearing methods yield `None` (presence-only cross-check).
    match crate::types::mrtr::frame_routing_pair(value) {
        Some((method, name)) => (Some(method.to_string()), name),
        None => (None, None),
    }
}

/// Emit the three v2 routing headers outbound WITHOUT panicking (T-112-13).
///
/// Sets `Mcp-Method`, `Mcp-Name` and forces `MCP-Protocol-Version` to the v2
/// value. Called on BOTH the success and structured-error response of an
/// accepted v2 request. On an unrepresentable value the individual insert is
/// skipped (caller already produced a valid response) rather than unwrapping.
///
/// `name` is whatever [`require_v2_headers`] carried forward, which is the EMPTY
/// STRING for any method with no routing name — so a stray inbound `Mcp-Name` is
/// never reflected back to its sender (Phase 118 D-20, T-118-53).
fn apply_v2_outbound_headers(headers: &mut HeaderMap, method: &str, name: &str) {
    if let Ok(v) = HeaderValue::from_str(method) {
        headers.insert(MCP_METHOD, v);
    }
    if let Ok(v) = HeaderValue::from_str(name) {
        headers.insert(MCP_NAME, v);
    }
    if let Ok(v) = HeaderValue::from_str(crate::types::protocol::PROTOCOL_VERSION_2026_07_28) {
        headers.insert(MCP_PROTOCOL_VERSION, v);
    }
}

/// Map a per-request version-negotiation failure to a structured gate rejection.
///
/// An UNSUPPORTED version is the spec's `UNSUPPORTED_PROTOCOL_VERSION` (-32022),
/// and its `error.data` MUST list the versions the server DOES accept so the
/// client can pick a mutually supported one and retry rather than probe. A
/// MALFORMED reserved `_meta` key is a bad method parameter, so it keeps the
/// `INVALID_PARAMS` mapping the shared dispatch resolver uses.
fn negotiation_error_to_gate_reject(
    error: &crate::types::protocol::context::ProtocolNegotiationError,
    accept_list: &[crate::types::ProtocolVersion],
) -> V2GateOutcome {
    use crate::types::protocol::context::ProtocolNegotiationError;
    use crate::types::protocol::error_codes::UNSUPPORTED_PROTOCOL_VERSION;
    match error {
        ProtocolNegotiationError::UnsupportedVersion(requested) => {
            let supported: Vec<&str> = accept_list.iter().map(|v| v.as_str()).collect();
            V2GateOutcome::Reject {
                code: UNSUPPORTED_PROTOCOL_VERSION,
                message: format!("Unsupported protocol version: {requested}"),
                data: Some(json!({ "requested": requested, "supported": supported })),
            }
        },
        ProtocolNegotiationError::MalformedMeta(_) => {
            let (code, message) = crate::server::core::negotiation_error_to_rejection(error);
            V2GateOutcome::Reject {
                code,
                message,
                data: None,
            }
        },
    }
}

/// The RAW `params._meta` object of a JSON-RPC request body, if it has one.
///
/// # Why the era is read from the RAW body and not from a typed field
///
/// A stateless v2 request has no `initialize` handshake, so `params._meta` is the
/// ONLY era channel — every method must be able to carry it. Reading it from a
/// typed `req._meta` field can only ever cover the three request structs that
/// HAVE such a field, and adding the field to the rest is a MAJOR semver break
/// (`cargo semver-checks` `constructible_struct_adds_field` on the `pub`,
/// all-`pub`-fields, constructible `ListToolsRequest` and friends). Reading the
/// body needs no public API change and covers every method, including the ones
/// plan 10 has not written yet (Phase-113 D-113-B / D-113-D resolution).
///
/// The SPEC spelling `_meta` wins; `meta` is accepted as a fallback so this reader
/// mirrors the `#[serde(rename = "_meta", alias = "meta")]` ingress contract the
/// typed structs carry (D-113-A) and the two can never disagree about what counts
/// as a `_meta` object. Never panics on adversarial bytes (T-112-13).
///
/// Test-only: every production caller now holds an already-parsed body and goes
/// through [`params_meta_of`] instead, so this byte-slice form survives purely as
/// the unit tests' entry point (its sibling `extract_body_method_and_name` is
/// `#[cfg(test)]` for the same reason).
#[cfg(test)]
fn raw_params_meta(body: &[u8]) -> Option<serde_json::Value> {
    // Binds the parse to a local so the borrow `params_meta_of` returns outlives
    // the call; this is the only caller that needs an OWNED `_meta`.
    let parsed = raw_body_json(body);
    params_meta_of(parsed.as_ref()).cloned()
}

/// Parse the raw JSON-RPC body ONCE. `None` for adversarial / non-JSON bytes.
fn raw_body_json(body: &[u8]) -> Option<serde_json::Value> {
    serde_json::from_slice::<serde_json::Value>(body).ok()
}

/// One POST body's raw bytes plus its JSON, parsed AT MOST ONCE per request.
///
/// # Why a memo and not an eager parse
///
/// Two stages of the POST pipeline read the parsed body — the v2 header gate
/// ([`run_v2_header_gate`]) and the log-level rule
/// ([`resolve_request_log_level`]) — and each used to call [`raw_body_json`]
/// itself, so every POST to an opted-in server walked the whole request body
/// through `serde_json` TWICE.
///
/// A single eager parse at the top of the handler would remove the duplication
/// and cost D-04: a server that never opted into `2026-07-28` short-circuits out
/// of the gate BEFORE the parse, and a request rejected by the gate, session
/// resolution, the legacy-version guard or auth is refused without an
/// attacker-sized body ever being parsed. The memo keeps both properties —
/// nothing parses until a reader actually asks, and the second ask is free.
///
/// [`std::sync::OnceLock`] rather than [`std::cell::OnceCell`] because this value
/// is borrowed across the gate's `.await` points, and a non-`Sync` cell would
/// make the handler futures non-`Send`.
struct PostBody<'a> {
    raw: &'a [u8],
    json: std::sync::OnceLock<Option<serde_json::Value>>,
}

impl<'a> PostBody<'a> {
    /// Wrap the raw request bytes. Parses nothing.
    fn new(raw: &'a [u8]) -> Self {
        Self {
            raw,
            json: std::sync::OnceLock::new(),
        }
    }

    /// The raw bytes, for the readers that must see the LITERAL wire value a WAF
    /// would see (D-06) rather than a re-serialization of the parse.
    fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// The parsed body, parsing on the FIRST call only.
    fn json(&self) -> Option<&serde_json::Value> {
        self.json.get_or_init(|| raw_body_json(self.raw)).as_ref()
    }
}

/// [`raw_params_meta`] over an ALREADY-PARSED body.
///
/// BORROWS out of `value` rather than cloning. Every consumer
/// (`classify_v2_meta_version`, `resolve_raw_meta_protocol_context`,
/// `require_v2_client_capabilities`) takes `Option<&Value>`, so a clone here was
/// per-request waste that grew when `require_v2_client_capabilities` began
/// mandating the nested `_meta.clientCapabilities` object. It also makes the
/// signature say what this function's contract already claimed: the gate reads
/// ONE shared value, not a copy of it.
fn params_meta_of(value: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    let params = value?.get("params")?;
    params
        .get(crate::types::mrtr::META_KEY)
        .or_else(|| params.get("meta"))
        .filter(|meta| !meta.is_null())
}

/// The raw top-level `params` value of an ALREADY-PARSED body, or `Null`.
///
/// `Null` is the "no MRTR fields" input for
/// [`crate::types::mrtr::extract_mrtr_params`], which returns the default
/// (both fields absent) for any non-object value.
fn params_of(value: Option<&serde_json::Value>) -> &serde_json::Value {
    const NO_PARAMS: &serde_json::Value = &serde_json::Value::Null;
    value.and_then(|v| v.get("params")).unwrap_or(NO_PARAMS)
}

// ---------------------------------------------------------------------------
// MRTR request params at v2 ingress (Plan 113-06, HTTP-03 / T-113-44).
//
// # Why the TRANSPORT does this extraction
//
// `inputResponses` and `requestState` are top-level `params` SIBLINGS of
// `name`/`arguments`/`uri` — they are NOT `_meta` keys. `GetPromptRequest` and
// `ReadResourceRequest` are `pub` structs with all-`pub` fields and are NOT
// `#[non_exhaustive]`, so giving them typed MRTR fields is a MAJOR semver break
// (`cargo semver-checks` `constructible_struct_adds_field` — the measured
// D-113-D finding that forced the raw-body route). Reading the fields off the
// already-parsed raw body needs ZERO public API change and is the SAME route
// Phase 112 already uses for the raw `params._meta` era signal.
//
// The read runs ONLY for an ACCEPTED v2 request: v1 and non-opted-in requests
// execute zero MRTR code (D-04).
// ---------------------------------------------------------------------------

/// Attach the raw-body MRTR params to an accepted v2 request **on an
/// MRTR-eligible method**, or turn a PRESENT-but-unusable field into an
/// `INVALID_PARAMS` rejection.
///
/// A malformed / oversized / wrong-shaped MRTR field must never be silently
/// treated as ABSENT: doing so lets an attacker skip the `requestState` verdict
/// table entirely (T-113-44). `extract_mrtr_params` therefore returns a
/// `Result`, and every `Err` short-circuits into the plan-04 rejection path,
/// which the code-driven status mapper renders as HTTP 400.
///
/// The client-facing message is the `MrtrParseError`'s `Display`, which names
/// the violated BOUND and never echoes attacker-supplied content; the
/// discriminated reason is logged server-side only.
///
/// # The method gate, and the defect it closes (Phase 114 plan 13)
///
/// [`mrtr_ingest`](crate::server::core::mrtr_ingest) already states the rule —
/// *"T-113-23: the spec confines MRTR to three methods. A `requestState`
/// presented on any other method is IGNORED — not verified, not errored"* — and
/// returns `Inert` for every non-eligible method. This EXTRACTION site had no
/// method awareness at all, so it applied MRTR's parse and MRTR's bounds to the
/// top-level `params` of **every** accepted v2 request. The two halves of one rule
/// disagreed.
///
/// That was not cosmetic. `tasks/update`'s entire payload IS `inputResponses`, so
/// the un-gated extraction judged that method's body at the TRANSPORT HEADER GATE
/// — before the router's era gate, before the `-32021` declaration gate and before
/// the `-32003` identity table. MEASURED over a real socket: an UNAUTHENTICATED
/// caller sending `tasks/update` with `"inputResponses": "not-an-object"` received
/// `-32602 "inputResponses must be an object"` instead of `-32003`, and an
/// UNDECLARING caller received it instead of `-32021` — i.e. a free parse of the
/// caller's own choosing on an unauthenticated path (T-114-64) and an inversion of
/// 114-09's documented gate order (T-114-63). The regression tests are
/// `malformed_params_from_an_unauthenticated_caller_yield_32003` and
/// `an_undeclaring_v2_caller_is_refused_before_the_params_parse` in
/// `tests/v2_tasks_update_routing.rs`.
///
/// The gate reads [`mrtr_eligible`](crate::types::mrtr::mrtr_eligible) — the SAME
/// predicate over the SAME `MRTR_METHODS` table `mrtr_ingest` reads, never a
/// second list. `method` is the already-resolved, override-aware body method that
/// [`classify_v2_request`] has just cross-checked against `Mcp-Method`, so this
/// adds no new read of the wire.
///
/// It is strictly NARROWING: for the three eligible methods nothing changes at
/// all, and no request that is accepted today becomes rejected. What changes is
/// that a non-eligible method's `inputResponses` / `requestState` are now IGNORED
/// here exactly as `mrtr_ingest` already ignores them, instead of being able to
/// reject the request.
fn attach_v2_mrtr_params(
    context: Option<crate::types::protocol::ProtocolContext>,
    outcome: V2GateOutcome,
    body_json: Option<&serde_json::Value>,
    method: Option<&str>,
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    // Only an ACCEPTED v2 request carries MRTR fields (D-04: zero era code on
    // v1 / non-opted-in, and a rejected request never reaches dispatch).
    if !matches!(outcome, V2GateOutcome::EnforceOk { .. }) {
        return (context, outcome);
    }
    // ...and only on a method MRTR applies to. See the rustdoc above.
    if !method.is_some_and(crate::types::mrtr::mrtr_eligible) {
        return (context, outcome);
    }
    let Some(ctx) = context else {
        return (None, outcome);
    };
    match crate::types::mrtr::extract_mrtr_params(params_of(body_json)) {
        Ok(mrtr) => (Some(ctx.with_mrtr_params(mrtr)), outcome),
        Err(reason) => {
            tracing::warn!(
                target: "mcp.http",
                reason = ?reason,
                "rejecting a v2 request whose MRTR params are present but unusable"
            );
            let message = reason.to_string();
            (
                Some(ctx),
                V2GateOutcome::Reject {
                    code: crate::types::protocol::error_codes::INVALID_PARAMS,
                    message,
                    data: None,
                },
            )
        },
    }
}

/// THE v2 header gate for the streamable-HTTP transport — one path, every method.
///
/// Resolves the per-request era from the RAW body's `params._meta` (see
/// [`raw_params_meta`]), then runs the D-04 passthrough short-circuit, the
/// negotiation-error mapping, and the [`classify_v2_request`] header/`_meta`
/// matrix. The resolved [`ProtocolContext`](crate::types::protocol::ProtocolContext)
/// it returns is the SAME value threaded into dispatch, so this layer resolves the
/// era exactly ONCE and dispatch never re-resolves it (D-11 / Pitfall 2).
///
/// `body_method_override` exists for the one ingress whose method is fixed by
/// classification rather than read from the wire: a `server/discover` request pins
/// `Some("server/discover")` so the header/body cross-check cannot be fooled by a
/// body whose `method` field disagrees with how the request was routed. Every
/// other caller passes `None` and the method comes from the body.
///
/// Before Phase 113 plan 04 there were TWO gates here — a typed one reading
/// `req._meta` for public requests and a raw one reading `params._meta` for
/// discover — which meant the two ingress paths could (and did) disagree about
/// which methods carried an era signal at all. There is now one.
async fn run_v2_header_gate(
    state: &ServerState,
    headers: &HeaderMap,
    body: &PostBody<'_>,
    body_method_override: Option<&str>,
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    V2GateOutcome,
) {
    // D-04, taken literally: a server that never opted into `2026-07-28` runs
    // ZERO era code. The accept-list check is a 1–2 element scan; the body parse
    // below is a full `serde_json` walk of an arbitrarily large request, and on a
    // v1-only server every byte of its output was discarded.
    {
        let server = state.server.lock().await;
        if !crate::types::protocol::context::is_v2_opted_in(server.supported_protocol_versions()) {
            return (None, V2GateOutcome::Passthrough);
        }
    }
    // ONE parse of the raw body, shared by the era read, the header cross-check,
    // the MRTR params read and the log-level rule downstream — they can never
    // disagree about what it says. Deliberately OUTSIDE the lock: parsing an
    // attacker-sized body while holding the server mutex would serialize every
    // other request behind it.
    let body_json = body.json();
    let raw_meta = params_meta_of(body_json);
    // THE THREE-WAY `_meta` VERDICT (CONF-06 / gaps G-6 + G-8), computed from the
    // RAW header and the RAW `_meta` — i.e. BEFORE the accept list below.
    //
    // ORDERING, which is the entire G-8 fix: a header/`_meta` DISAGREEMENT
    // short-circuits HERE, ahead of `resolve_raw_meta_protocol_context`. Left
    // where it used to sit — after the resolver — an unsupported version that
    // ALSO disagreed with the header failed the accept list first and was
    // reported as `-32022`, so the disagreement never got a chance to classify.
    // The accept list still runs, and still answers `-32022`, for the case where
    // the two sides AGREE on a version the server does not support.
    let verdict = classify_v2_meta_version(decode_version_header(headers), raw_meta);
    // The rejection is built INSIDE the lock scope so the accept-list is only
    // borrowed on the rare negotiation-failure branch. Cloning it into a `Vec`
    // on every request — under the server mutex — bought nothing: the happy path
    // never reads it.
    let resolved = {
        let server = state.server.lock().await;
        // D-04 IS EVALUATED FIRST, and the disagreement short-circuit is INSIDE
        // that guard rather than above it. A server that never opted into v2
        // enforces NOTHING — `resolve_raw_meta_protocol_context` answers
        // `Ok(None)` for it without inspecting `_meta` at all — so a
        // disagreement check hoisted above this lock would make the header/body
        // rule the ONE v2 rule a stock v1-only `Server::builder()` applies,
        // turning requests it used to serve on the v1 path into `-32020`. The
        // sibling `MissingRequired` verdict is already gated this way (it is
        // consumed by `classify_v2_request` further down, which is only reached
        // on an opted-in server), so this keeps the two halves of the same rule
        // in step.
        //
        // G-8's ORDERING is preserved: the short-circuit still runs BEFORE
        // `resolve_raw_meta_protocol_context` produces its accept-list
        // rejection, so an unsupported version that ALSO disagrees with the
        // header is still classified as a disagreement (`-32020`) rather than
        // as an accept-list failure (`-32022`).
        if crate::types::protocol::context::is_v2_opted_in(server.supported_protocol_versions()) {
            if let V2MetaVerdict::Disagreement(message) = verdict {
                return (None, header_mismatch_reject(message));
            }
        }
        server
            .resolve_raw_meta_protocol_context(raw_meta)
            .map_err(|err| {
                negotiation_error_to_gate_reject(&err, server.supported_protocol_versions())
            })
    };
    let context = match resolved {
        Ok(ctx) => ctx,
        Err(reject) => return (None, reject),
    };
    // `Ok(None)` == not opted in → zero enforcement (D-04).
    if context.is_none() {
        return (None, V2GateOutcome::Passthrough);
    }
    let (extracted_method, body_name) = method_and_name_of(body_json);
    let body_method = body_method_override.or(extracted_method.as_deref());
    let outcome = classify_v2_request(headers, verdict, body_method, body_name.as_deref());
    // v2 METHOD RETIREMENT (CONF-05 / G-5), keyed on the method STRING and
    // evaluated for every accepted v2 request — including the ones whose typed
    // parse would have succeeded and whose dispatch would therefore have SERVED
    // them. Runs after `classify_v2_request` on purpose; see `retire_v2_method`.
    let outcome = retire_v2_method(outcome);
    // The required `clientCapabilities` key (CONF-06 / G-6), applied to a request
    // that is still ACCEPTED after retirement. Deliberately after
    // `retire_v2_method`; see `require_v2_client_capabilities`.
    let outcome = require_v2_client_capabilities(outcome, raw_meta);
    // MRTR params (HTTP-03): read on the ACCEPTED v2 path only, and only for an
    // MRTR-ELIGIBLE method; a present but unusable field becomes an
    // `INVALID_PARAMS` rejection here, BEFORE dispatch. `body_method` is the
    // value `classify_v2_request` just cross-checked, reused rather than re-read.
    attach_v2_mrtr_params(context, outcome, body_json, body_method)
}

/// Crate-LOCAL ingress classification for the POST pipeline (Phase 112, VERS-04).
///
/// This is NOT the public [`TransportMessage`] enum — it never adds a variant to
/// that semver-sensitive type. It only distinguishes an internally-routed
/// `server/discover` request (which has no public enum variant) from every other
/// message, so both flow through the SAME POST stages (session → v2 header matrix
/// → legacy-version → auth → dispatch → event store → response assembly) and
/// `server/discover` is routed only at the final per-path response-assembly step
/// (the classify-then-continue design — no pipeline bypass).
enum HttpIngress {
    /// Any normal message (typed request, notification, or response) — the
    /// existing public-enum dispatch path, unchanged.
    Public(TransportMessage),
    /// A v2-only `server/discover` request, carrying the ORIGINAL request id.
    ///
    /// It does NOT carry a copy of `_meta`: since Phase 113 plan 04 the single
    /// [`run_v2_header_gate`] reads `params._meta` from the raw body for every
    /// ingress, so a second captured copy here would be a duplicate read that
    /// could drift.
    Discover { id: crate::types::RequestId },
    /// A `subscriptions/listen` request (Phase 113 plan 10, HTTP-04), carrying
    /// the ORIGINAL request id — which IS the stream's `subscriptionId` — and the
    /// RAW `params` value the served branch deserializes into
    /// [`SubscriptionsListenParams`](crate::types::subscriptions::SubscriptionsListenParams).
    ///
    /// Classified here rather than added as a public `ClientRequest` variant:
    /// Phase 112 established that discipline precisely to keep semver MINOR
    /// (`enum_variant_added` on a public exhaustive enum is a MAJOR break), and
    /// `cargo semver-checks` catches a regression. The params stay RAW because
    /// this classifier must never reject a body — a malformed `params` becomes a
    /// structured `-32602` in the served branch, after the header gate and auth
    /// have run, not a parse error before them.
    SubscriptionsListen {
        id: crate::types::RequestId,
        params: Option<serde_json::Value>,
    },
    /// A v2-only `tasks/update` request (Phase 114 plan 13, TASK-02), carrying the
    /// ORIGINAL request id and the RAW `params` the served branch gates over.
    ///
    /// Classified through the SHARED
    /// [`parse_request_or_internal`](crate::shared::protocol_helpers::parse_request_or_internal)
    /// seam — the `server/discover` route, not this file's `SubscriptionsListen`
    /// route. `subscriptions/listen` classifies HTTP-locally because it opens an
    /// HTTP STREAM and has no meaning off this transport; `tasks/update` is an
    /// ordinary request/response, so its classification belongs in `shared/` where
    /// a later plan can widen its transport reach without a semver break.
    ///
    /// Not a public `ClientRequest` variant for the reason Phase 112 recorded on
    /// [`Discover`](Self::Discover)'s sibling: `enum_variant_added` on a public
    /// exhaustive enum is a MAJOR break, and `cargo semver-checks` catches a
    /// regression. The params stay RAW because the classifier must never reject a
    /// body — a malformed `params` becomes a structured `-32602` in the served
    /// branch, AFTER the era, backend, declaration and auth gates have run, not a
    /// parse error before them.
    TasksUpdate {
        id: crate::types::RequestId,
        params: serde_json::Value,
    },
}

impl HttpIngress {
    /// Whether this ingress is an `initialize` request — the flag that decides
    /// session minting.
    ///
    /// `server/discover`, `subscriptions/listen` and `tasks/update` are non-init
    /// by construction (a stateless capability projection, a v2 stream opener and
    /// a v2 task-input delivery respectively).
    ///
    /// Both POST preambles derived this with the same inline `match` before plan
    /// 113.1; it lives here so the two paths cannot drift, and so a new
    /// `HttpIngress` variant has exactly one place to answer the question.
    fn is_initialize(&self) -> bool {
        match self {
            Self::Public(msg) => is_initialize_request(msg),
            Self::Discover { .. } | Self::SubscriptionsListen { .. } | Self::TasksUpdate { .. } => {
                false
            },
        }
    }
}

/// Classify a raw POST body as an internally-routed request, if it is one.
///
/// Three methods are internally routed, none of which has a public
/// `ClientRequest` variant: `server/discover` (Phase 112, VERS-04),
/// `subscriptions/listen` (Phase 113 plan 10, HTTP-04) and `tasks/update`
/// (Phase 114 plan 13, TASK-02). Never panics (T-112-13).
///
/// Every other input (malformed JSON, a batch/notification with no `id`, a
/// non-object, or any other method) returns `None`, so the caller falls through
/// to the existing public parse path with byte-identical behavior.
fn classify_http_ingress(body: &[u8]) -> Option<HttpIngress> {
    let req: crate::types::JSONRPCRequest<serde_json::Value> = serde_json::from_slice(body).ok()?;
    // `subscriptions/listen` has no typed request at all: it is answered either by
    // a long-lived SSE stream or by `-32601`, both assembled from the raw id and
    // params. Classified BEFORE the discover peek so the two internally-routed
    // methods share one entry point.
    if req.method == crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD {
        return Some(HttpIngress::SubscriptionsListen {
            id: req.id,
            params: req.params,
        });
    }
    // Fast reject: `server/discover` and `tasks/update` are the only remaining
    // internally-routed methods, so for ~100% of traffic we skip the typed
    // `parse_client_request` conversion and the `_meta` clone below.
    // `parse_request_or_internal` remains the authority for both (its
    // `IngressRequest::Internal(..)` arms are the only paths that yield `Discover`
    // / `TasksUpdate`), so this peek changes no classification — any other method
    // returned `None` before too, via `Public(_) => None`.
    //
    // Both spellings are read from the SINGLE-SOURCED constants; neither is
    // re-typed here.
    if req.method != crate::types::protocol::SERVER_DISCOVER_METHOD
        && req.method != crate::types::protocol::TASKS_UPDATE_METHOD
    {
        return None;
    }
    let (id, ingress) = crate::shared::protocol_helpers::parse_request_or_internal(req).ok()?;
    match ingress {
        // The inner match is exhaustive over `InternalClientRequest`, so adding a
        // future internally-routed method is a compile-time tripwire here.
        crate::shared::protocol_helpers::IngressRequest::Internal(internal) => match internal {
            crate::types::protocol::InternalClientRequest::ServerDiscover(_) => {
                Some(HttpIngress::Discover { id })
            },
            crate::types::protocol::InternalClientRequest::TasksUpdate { params } => {
                Some(HttpIngress::TasksUpdate { id, params })
            },
        },
        // A public request re-parsed here is DISCARDED; the caller re-parses it via
        // the existing `StdioTransport::parse_message` path so all non-discover
        // bytes (incl. parse-error responses) stay exactly as before.
        crate::shared::protocol_helpers::IngressRequest::Public(_) => None,
    }
}

impl StreamableHttpServer {
    /// Creates a new `StreamableHttpServer` with default config
    pub fn new(addr: SocketAddr, server: Arc<tokio::sync::Mutex<Server>>) -> Self {
        Self::with_config(addr, server, StreamableHttpServerConfig::default())
    }

    /// Creates a new `StreamableHttpServer` with custom config
    pub fn with_config(
        addr: SocketAddr,
        server: Arc<tokio::sync::Mutex<Server>>,
        config: StreamableHttpServerConfig,
    ) -> Self {
        let state = make_server_state(server, config);
        Self { addr, state }
    }

    /// Starts the server and returns the bound address and a task handle.
    ///
    /// Applies the same Tower layer security stack as
    /// [`pmcp::axum::router()`](crate::server::axum_router::router):
    /// - `CorsLayer` -- origin-locked CORS (no wildcard `*`)
    /// - [`DnsRebindingLayer`] -- Host/Origin header validation
    /// - [`SecurityHeadersLayer`] -- nosniff, DENY, no-store
    pub async fn start(self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        // Idempotent. `make_server_state` already tried, and succeeded whenever it
        // ran inside a runtime; this covers the `with_config`-outside-a-runtime
        // construction order.
        peer_channel::ensure_outbound_drain(&self.state);
        let allowed = self.state.allowed_origins.clone();
        let cors = crate::server::tower_layers::build_mcp_cors_layer(&allowed);

        // Layer ordering: CORS (outermost) -> DnsRebinding -> SecurityHeaders -> handler
        let app = build_mcp_router(self.state)
            .layer(SecurityHeadersLayer::default())
            .layer(DnsRebindingLayer::new(allowed))
            .layer(cors);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Ok((local_addr, server_task))
    }
}

/// The ONE `405 Method Not Allowed` body for a verb the MCP endpoint does not
/// serve.
///
/// Two callers, one answer:
///
/// * [`v2_method_not_allowed`] — the CONDITIONAL rejection. On a `v1-compat`
///   build it fires only when the request opted into `2026-07-28` and the
///   server accepts it; every other request falls through to the v1 body.
/// * `v1::handle_get_sse_body` / `v1::handle_delete_body` in
///   `v1_session_off.rs` — the UNCONDITIONAL answer. On a `full-v2` build there
///   is no v1 body to fall through to, so the verb is always refused.
///
/// It is `pub(crate)` for exactly the second caller. A twin that hand-rolled its
/// own `405` would be a second answer to the same question, free to drift from
/// this one on the next edit; the wire shape of a refused verb must not depend
/// on which half of the pair produced it.
///
/// The verb stays ROUTED in [`build_mcp_router`] on both feature sets. An
/// unrouted verb answers `404`, which is a different wire answer with a
/// different meaning ("no such endpoint" rather than "this endpoint does not
/// take this verb") — see `tests/v2_verbs_405_on_severed_build.rs`, which
/// asserts the distinction on the severed build.
/// # The `Allow` header
///
/// RFC 9110 section 15.5.6 is a MUST: "The origin server MUST generate an
/// `Allow` header field in a 405 response containing a list of the target
/// resource's currently supported methods." Intermediaries and generic HTTP
/// clients rely on it, and a `405` without it tells a caller only that it was
/// wrong, never what to do instead. Consolidating both `405` sites into this one
/// function is what made fixing it a single edit.
///
/// `POST, OPTIONS` is the honest list: `POST` is the MCP endpoint, and `OPTIONS`
/// is answered by the CORS layer. `GET` and `DELETE` are deliberately absent —
/// they are ROUTED (an unrouted verb would answer `404`, a different claim) but
/// they are not SUPPORTED on `2026-07-28`, and `Allow` enumerates support, not
/// routing.
///
/// This changes v1-compat wire bytes for the v2-REJECTION path only, which is a
/// path no v1 client reaches: it fires only when the request opted into
/// `2026-07-28`. `tests/v1_byte_identity_after_cut.rs` pins the v1
/// session-lifecycle responses and does not pin this one — verified by running
/// it after this change.
pub(crate) fn method_not_allowed_for_verb(verb: &str) -> Response {
    let mut response = create_error_response(
        StatusCode::METHOD_NOT_ALLOWED,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        &format!("HTTP {verb} is not supported on the MCP endpoint for protocol 2026-07-28"),
    );
    response
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST, OPTIONS"));
    response
}

/// Reject a v2 `GET` / `DELETE` with `405 Method Not Allowed`, or `None` to let
/// the existing v1 handler run.
///
/// Spec, verbatim: "HTTP GET or DELETE to the MCP endpoint: respond with
/// `405 Method Not Allowed`." Neither verb carries a body, so `_meta` is
/// unavailable and the ONLY era signal is the `MCP-Protocol-Version` header —
/// read through the existing non-panicking [`decode_version_header`], so an
/// oversized or non-UTF-8 value classifies as `Malformed` (v1 behavior) rather
/// than 405.
///
/// pmcp is dual-version, so the routes STAY registered: every other header value
/// reaches today's handler unchanged. The guard runs BEFORE header validation and
/// before session validation, so a v2 GET never touches session state or the
/// event store (T-113-18).
///
/// It is ALSO gated on `v2_opted_in`, the server's accept-list (D-04: a server
/// that never opted into `2026-07-28` runs zero era code). Without that gate a
/// client sending `MCP-Protocol-Version: 2026-07-28` at a v1-only server would
/// have its legitimate v1 SSE `GET` / session `DELETE` answered `405` by a server
/// that does not speak v2 at all.
///
/// Kept pure (no [`ServerState`]) so the RULE is unit-testable; the live wiring
/// is [`v2_verb_rejection`].
fn v2_method_not_allowed(headers: &HeaderMap, verb: &str, v2_opted_in: bool) -> Option<Response> {
    if !v2_opted_in || !matches!(decode_version_header(headers), HeaderProtocolVersion::V2) {
        return None;
    }
    Some(method_not_allowed_for_verb(verb))
}

/// [`v2_method_not_allowed`] against a live server.
///
/// The cheap header classification runs FIRST, so the overwhelmingly common v1
/// `GET`/`DELETE` never touches the server mutex to learn the accept-list.
async fn v2_verb_rejection(
    state: &ServerState,
    headers: &HeaderMap,
    verb: &str,
) -> Option<Response> {
    if !matches!(decode_version_header(headers), HeaderProtocolVersion::V2) {
        return None;
    }
    let opted_in = {
        let server = state.server.lock().await;
        crate::types::protocol::context::is_v2_opted_in(server.supported_protocol_versions())
    };
    v2_method_not_allowed(headers, verb, opted_in)
}

/// Validate `Content-Type: application/json` for POST.
fn validate_content_type_json(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Content-Type header is required",
        ));
    };
    let ct = content_type.to_str().unwrap_or("");
    if !ct.contains(APPLICATION_JSON) {
        return Err(create_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Content-Type must be application/json",
        ));
    }
    Ok(())
}

/// Validate `Accept: application/json` or `text/event-stream` for POST.
fn validate_accept_post(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(accept) = headers.get(header::ACCEPT) else {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header is required",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(APPLICATION_JSON) && !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header must include application/json or text/event-stream",
        ));
    }
    Ok(())
}

/// Validate `Accept: text/event-stream` for GET (SSE).
fn validate_accept_sse(headers: &HeaderMap) -> std::result::Result<(), Response> {
    let Some(accept) = headers.get(header::ACCEPT) else {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header is required for SSE",
        ));
    };
    let accept_str = accept.to_str().unwrap_or("");
    if !accept_str.contains(TEXT_EVENT_STREAM) {
        return Err(create_error_response(
            StatusCode::NOT_ACCEPTABLE,
            crate::types::protocol::error_codes::PARSE_ERROR,
            "Accept header must be text/event-stream for SSE",
        ));
    }
    Ok(())
}

/// Validate request headers and return appropriate error response.
///
/// Refactored in 75-01 Task 1a-A: per-header checks extracted to
/// [`validate_content_type_json`], [`validate_accept_post`], and
/// [`validate_accept_sse`] (P3).
fn validate_headers(headers: &HeaderMap, method: &str) -> std::result::Result<(), Response> {
    match method {
        "POST" => {
            validate_content_type_json(headers)?;
            validate_accept_post(headers)?;
        },
        "GET" => validate_accept_sse(headers)?,
        _ => {},
    }
    Ok(())
}

/// Build response with appropriate format (JSON or SSE).
/// Serialize a `TransportMessage` and re-parse as a `serde_json::Value`, or
/// return a 500 error response on failure.
fn serialize_response_as_json_value(
    response: &TransportMessage,
) -> std::result::Result<serde_json::Value, Response> {
    let json_bytes = crate::shared::StdioTransport::serialize_message(response).map_err(|e| {
        create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            &format!("Failed to serialize response: {}", e),
        )
    })?;
    tracing::debug!(
        target: "mcp.http",
        response = %String::from_utf8_lossy(&json_bytes),
        "HTTP response serialized bytes"
    );
    let json_value: serde_json::Value = serde_json::from_slice(&json_bytes).map_err(|e| {
        create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            &format!("Failed to parse JSON response: {}", e),
        )
    })?;
    Ok(json_value)
}

// ---------------------------------------------------------------------------
// CONF-07 / G-3's v2 half (D-16): the POST response body as a multi-frame SSE
// vehicle for `notifications/progress`.
//
// On v2 there are no sessions (`sessions_active_for(_, Some(Era::V2))` is false,
// pinned by `sessions_active_truth_table`) and GET answers 405 (pinned by
// `v2_verb_rejection`), so a POST's own response body is the ONLY channel a
// server-to-client notification can travel on. This block generalizes the
// one-shot `build_sse_response_from_single_message` into a builder that emits N
// queued progress frames followed by the result frame.
//
// The measured basis (plan 12 Task 1): the pinned conformance suite
// (0.2.0-alpha.11) reported `tools-call-with-progress` SUCCESS with
// `progressCount: 3` against a server answering exactly this shape. Its v2 client
// (`wt` in `dist/index.js`) pushes any frame with a `method` and no `id` onto its
// `notifications` array, and THROWS -32600 on a frame carrying both.
// ---------------------------------------------------------------------------

/// Maximum number of progress notifications one v2 request may queue onto its
/// POST response body.
///
/// # The number
///
/// **64.** The pinned suite's `tools-call-with-progress` asks for three, and
/// `ServerProgressReporter` rate-limits a well-behaved handler to ten per second,
/// so 64 admits roughly six seconds of continuous well-behaved reporting — far
/// past any realistic tool — while capping the per-request memory a hostile
/// handler can pin at 64 `Notification`s rather than at "however many it can emit
/// before the response is built".
///
/// # The overflow policy: DROP-NEWEST, never block, never grow
///
/// The queue is a **bounded** `tokio::sync::mpsc` channel and the producer is a
/// synchronous `Fn(Notification)` sink that cannot await, so it uses `try_send`:
/// when the queue is full the notification is DROPPED and a `warn!` is logged.
/// Progress is advisory by definition — the MCP spec says a receiver "is not
/// obligated to provide these notifications" — so dropping is a conformant
/// degradation, whereas blocking would let a slow reader stall a tool handler and
/// growing without limit is the memory-DoS class Phase 113 fixed.
///
/// `build_sse_response_from_single_message` uses `mpsc::unbounded_channel`; that
/// is sound for exactly-one-message and is deliberately NOT copied here
/// (T-118.1-12-01).
pub const V2_PROGRESS_QUEUE_CAPACITY: usize = 64;

/// A frame that may legally appear on a v2 POST response stream.
///
/// # This type IS the control for `HttpServerNoIndependentRequestsOnStream`
///
/// The suite's v2 client refuses a top-level server-to-client REQUEST on this
/// stream — it throws `-32600 "Server sent request '…' on response stream;
/// stateless lifecycle forbids this (use MRTR)"`. Server-to-client requests stay
/// MRTR on v2, which already works.
///
/// That constraint is enforced HERE, structurally: this enum has a notification
/// arm and a result arm and **no request arm**, so a request frame on a v2
/// response stream is not a bug to be avoided by review — it is unrepresentable.
/// A comment saying "do not send requests here" would not be a control
/// (T-118.1-12-02).
///
/// Contrast [`TransportMessage`], which carries a `Request { id, request }`
/// variant and therefore must NOT be the item type of this stream.
#[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
enum V2ResponseFrame {
    /// A one-way notification — today only `notifications/progress`.
    Notification(crate::types::Notification),
    /// The terminal result frame, carrying the LIVE request id (HTTP-05).
    Result(crate::types::JSONRPCResponse),
}

impl V2ResponseFrame {
    /// Project onto the wire type the shared serializer accepts.
    ///
    /// One-directional by construction: every `V2ResponseFrame` maps onto a
    /// `TransportMessage`, and nothing maps back, so the missing request arm
    /// cannot be reintroduced through this seam.
    fn into_transport_message(self) -> TransportMessage {
        match self {
            Self::Notification(notification) => TransportMessage::Notification(notification),
            Self::Result(response) => TransportMessage::Response(response),
        }
    }
}

/// The receiver half of one request's bounded progress queue.
///
/// Created BEFORE dispatch (so the sink handed to the handler has somewhere to
/// write) and drained AFTER it (so the frames are all present when the response
/// body is assembled). Owning it in a newtype keeps `FastPathDispatch` and
/// `MiddlewareDispatch` from growing a bare `mpsc::Receiver` field whose
/// element type nothing constrains.
pub(crate) struct V2ProgressQueue {
    receiver: mpsc::Receiver<crate::types::Notification>,
}

impl V2ProgressQueue {
    /// Take every notification queued so far, in emission order.
    ///
    /// Non-blocking: the handler has already run to completion by the time this
    /// is called (dispatch is awaited before the response is built), so the queue
    /// is complete and `try_recv` drains it deterministically. Nothing here waits
    /// on a producer, so a handler that emitted nothing costs one failed
    /// `try_recv`.
    fn drain(mut self) -> Vec<crate::types::Notification> {
        let mut frames = Vec::new();
        while let Ok(notification) = self.receiver.try_recv() {
            frames.push(notification);
        }
        frames
    }
}

/// Create one request's bounded progress queue.
///
/// Returns the sink to hand into the request's `TransportBackchannel` and the
/// receiver to drain when the response is assembled. See
/// [`V2_PROGRESS_QUEUE_CAPACITY`] for the bound and the drop-newest policy.
pub(crate) fn new_v2_progress_queue() -> (
    Arc<dyn Fn(crate::types::Notification) + Send + Sync>,
    V2ProgressQueue,
) {
    let (tx, receiver) = mpsc::channel(V2_PROGRESS_QUEUE_CAPACITY);
    let sink: Arc<dyn Fn(crate::types::Notification) + Send + Sync> =
        Arc::new(move |notification| {
            // DROP-NEWEST on a full queue. `try_send` never blocks, which is
            // required: this closure is synchronous and runs on the handler's
            // task.
            if let Err(e) = tx.try_send(notification) {
                tracing::warn!(
                    target: "mcp.http",
                    capacity = V2_PROGRESS_QUEUE_CAPACITY,
                    error = %e,
                    "v2 progress queue full or closed — dropping a progress notification"
                );
            }
        });
    (sink, V2ProgressQueue { receiver })
}

/// Append one SSE frame exactly as [`build_sse_response_from_single_message`]
/// emits it — `id: <uuid>`, `event: message`, `data: <serialized message>`.
///
/// Hand-rendered rather than delegating to `axum`'s [`Event`] because the
/// middleware path needs the same bytes as a complete `Vec<u8>` body and `Event`
/// exposes no serializer. `the_hand_rendered_framing_matches_axum_event` below
/// pins the two encodings together so this copy cannot drift.
///
/// Writes into a caller-owned buffer rather than returning a `String`: the
/// multi-frame body appends up to [`V2_PROGRESS_QUEUE_CAPACITY`] + 1 frames, and
/// a per-frame `String` immediately copied into the accumulator and dropped is
/// one allocation and one copy per frame for nothing.
///
/// The PAYLOAD is appended with `push_str`, not interpolated through the
/// formatter, and that is not a style choice: `tests/v2_bounded_reads_tripwire.rs`
/// reviews byte accumulation by scanning for a fixed needle vocabulary that
/// includes `push_str(` and not `write!(`. Spelling this append as a `write!`
/// interpolation moves the one genuinely payload-sized accumulation in this file
/// OUT of that reviewed population — silently, while the tripwire's allowlist
/// entry rots into a dead entry. The constant framing around it still goes
/// through `write!`, which is where the zero-allocation `Uuid` render belongs.
fn write_sse_frame(out: &mut String, frame: V2ResponseFrame) {
    use std::fmt::Write as _;

    let message = frame.into_transport_message();
    let json_bytes =
        crate::shared::StdioTransport::serialize_message(&message).unwrap_or_else(|e| {
            tracing::error!(target: "mcp.sse", error = %e, "Failed to serialize SSE message");
            Vec::new()
        });
    let json_str = String::from_utf8(json_bytes).unwrap_or_else(|_| "{}".to_string());
    // Writing into a String is infallible; the Result exists only for the fmt trait.
    let _ = write!(out, "id: {}\nevent: message\ndata: ", Uuid::new_v4());
    out.push_str(&json_str);
    out.push_str("\n\n");
}

/// Render the whole multi-frame body: every progress frame, then the result.
///
/// # Termination (T-118.1-12-03)
///
/// The body is COMPLETE before it is handed to axum. The handler has already run
/// to completion — dispatch is awaited before the response is assembled — so
/// there is no long-lived stream here, no keep-alive to schedule, and no
/// connection held open by a stalled handler. The stream terminates on the result
/// frame because the result frame is the last byte written.
fn render_v2_multi_frame_body(
    progress: Vec<crate::types::Notification>,
    result: crate::types::JSONRPCResponse,
) -> String {
    let mut body = String::new();
    for notification in progress {
        write_sse_frame(&mut body, V2ResponseFrame::Notification(notification));
    }
    write_sse_frame(&mut body, V2ResponseFrame::Result(result));
    body
}

/// Build the multi-frame SSE POST response.
///
/// Sets `text/event-stream` plus the same anti-buffering treatment the long-lived
/// listen stream uses, so an intermediary cannot coalesce the frames.
fn build_v2_multi_frame_sse_response(
    progress: Vec<crate::types::Notification>,
    result: crate::types::JSONRPCResponse,
) -> Response {
    let body = render_v2_multi_frame_body(progress, result);
    let mut response = (StatusCode::OK, body).into_response();
    apply_v2_multi_frame_headers(response.headers_mut());
    response
}

/// The `text/event-stream` + anti-buffering header set a multi-frame body needs.
///
/// ONE definition, applied by both the fast path (which mutates an already-built
/// axum `Response`) and the middleware path (which assembles a `HeaderMap` before
/// the chain runs), so the two cannot drift on content type or buffering.
fn apply_v2_multi_frame_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(TEXT_EVENT_STREAM),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(X_ACCEL_BUFFERING, HeaderValue::from_static("no"));
}

/// Whether this request may answer with a multi-frame SSE body.
///
/// All three must hold (T-118.1-12-05):
///
/// * the era is **v2** — v1 has a session stream and uses it (plan 11);
/// * JSON mode is **off** — an operator who configured JSON responses gets JSON;
/// * the client's `Accept` includes **`text/event-stream`** — switching a client
///   that asked only for JSON onto an event stream could break it or an
///   intermediary between them.
///
/// Evaluated BEFORE dispatch so an ineligible request never even allocates a
/// queue, which is what makes "a v2 call that emits no progress is byte-identical
/// to today" true by construction rather than by a later branch.
/// `accept` is the request's raw `Accept` value. Taken as a `&str` rather than a
/// `HeaderMap` so the fast path (axum `HeaderMap`) and the middleware path
/// (`ServerHttpRequest::get_header`) reach the SAME predicate instead of each
/// re-deriving eligibility from the shape of its own request type.
fn v2_multi_frame_eligible(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    accept: Option<&str>,
) -> bool {
    era == Some(crate::types::protocol::Era::V2)
        && !state.config.enable_json_response
        && accept.is_some_and(|accept| accept.contains(TEXT_EVENT_STREAM))
}

/// Attach a v2 progress sink to this request's context, returning the queue.
///
/// The v2 twin of [`peer_channel::attach_session_backchannel`]'s v1 branch, and
/// deliberately NOT a reuse of its closure: that one routes through
/// `v1::route_to_session_stream` keyed by `session_id`, which has no meaning on
/// an era with no sessions and would drop every frame silently (T-118.1-11-08).
/// What the two share is the ATTACHMENT POINT and the sink TYPE.
///
/// No peer is attached: a v2 server-to-client REQUEST stays MRTR, and this stream
/// structurally cannot carry one ([`V2ResponseFrame`]).
fn attach_v2_progress_sink(
    context: Option<crate::types::protocol::ProtocolContext>,
    eligible: bool,
) -> (
    Option<crate::types::protocol::ProtocolContext>,
    Option<V2ProgressQueue>,
) {
    if !eligible {
        return (context, None);
    }
    let Some(context) = context else {
        return (None, None);
    };
    let (sink, queue) = new_v2_progress_queue();
    // LAYERED onto whatever the context already carries, not substituted for it.
    // `attach_session_backchannel` returns early on v2 today, so in practice the
    // slot is empty here — but building a fresh `TransportBackchannel` would
    // DISCARD a peer handle if that ever stopped being true, and a silently
    // dropped peer is exactly the class of defect this phase is closing.
    let backchannel = context
        .transport_backchannel()
        .cloned()
        .unwrap_or_default()
        .with_notification_sink(sink);
    (
        Some(context.with_transport_backchannel(backchannel)),
        Some(queue),
    )
}

/// Decide whether this response becomes a multi-frame SSE body, and if so hand
/// back its frames.
///
/// Returns `None` — meaning "use the unchanged response builder" — in every case
/// but one:
///
/// * no queue (the request was not eligible, so none was created);
/// * the queue is EMPTY (the handler reported no progress), which is what makes a
///   no-progress v2 response byte-identical to today's;
/// * the response is not a `Response` envelope (unreachable on this path, but the
///   match states it rather than assuming it).
///
/// Takes `response_msg` BY VALUE and hands it back in the `Err` arm rather than
/// cloning it into the `Ok` arm: the response carries the whole tool result,
/// including any base64 image or audio blob, and both call sites own it and drop
/// it unused on the multi-frame branch. The common no-progress path pays one
/// failed `try_recv` and a move.
fn take_v2_progress_frames(
    queue: Option<V2ProgressQueue>,
    response_msg: TransportMessage,
) -> std::result::Result<
    (
        Vec<crate::types::Notification>,
        crate::types::JSONRPCResponse,
    ),
    TransportMessage,
> {
    let Some(queue) = queue else {
        return Err(response_msg);
    };
    // The envelope check comes FIRST. Draining before it would pull every queued
    // frame out of the receiver and then drop them on the floor in the `Err`
    // arm, silently losing progress the handler did emit; returning the message
    // untouched leaves the queue intact for whatever the caller does next.
    let result = match response_msg {
        TransportMessage::Response(result) => result,
        // Not a response envelope, so there is nothing to terminate the stream
        // with — the match states it rather than assuming it.
        other => return Err(other),
    };
    let progress = queue.drain();
    if progress.is_empty() {
        return Err(TransportMessage::Response(result));
    }
    Ok((progress, result))
}

/// Build an OK JSON response body from a `TransportMessage`.
fn build_json_response(response: &TransportMessage, trace_source: &'static str) -> Response {
    let json_value = match serialize_response_as_json_value(response) {
        Ok(v) => v,
        Err(error_response) => return error_response,
    };
    tracing::debug!(
        target: "mcp.http",
        source = trace_source,
        response = %serde_json::to_string(&json_value).unwrap_or_default(),
        "HTTP response (JSON mode)"
    );
    (StatusCode::OK, Json(json_value)).into_response()
}

/// Build an SSE streaming response from a single `TransportMessage`.
///
/// Each element of the stream is serialized via `StdioTransport` for
/// JSON-RPC-compat framing.
fn build_sse_response_from_single_message(response: TransportMessage) -> Response {
    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(response).unwrap();
    let stream = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(stream.map(|msg| {
        let event_id = Uuid::new_v4().to_string();
        let json_bytes =
            crate::shared::StdioTransport::serialize_message(&msg).unwrap_or_else(|e| {
                tracing::error!(target: "mcp.sse", error = %e, "Failed to serialize SSE message");
                Vec::new()
            });
        let json_str = String::from_utf8(json_bytes).unwrap_or_else(|_| "{}".to_string());
        Ok::<_, Infallible>(
            Event::default()
                .id(event_id)
                .event("message")
                .data(json_str),
        )
    }));
    sse.into_response()
}

/// Build response with appropriate format (JSON or SSE).
///
/// Refactored in 75-01 Task 1a-A (P1): extracted
/// [`serialize_response_as_json_value`], [`build_json_response`], and
/// [`build_sse_response_from_single_message`] so this function is a thin
/// per-mode dispatcher.
///
/// `session_id` is the RAW INBOUND `Mcp-Session-Id` header, and it selects which
/// open SSE stream (i.e. which CALLER) receives this reply. `sessions_on` is
/// therefore load-bearing, not cosmetic: without it a v2 POST that merely NAMES a
/// v1 caller's open session id had its response delivered into THAT caller's
/// stream — a direct response reaching a caller that never issued the request
/// (T-113-07), while the v2 caller got a bare `202 Accepted`. On v2 there is no
/// session, so there is no stream to route to and the reply always goes back to
/// the caller that asked for it.
fn build_response(
    state: &ServerState,
    response: TransportMessage,
    session_id: Option<&String>,
    sessions_on: bool,
) -> Response {
    if state.config.enable_json_response {
        return build_json_response(&response, "JSON mode");
    }
    // SSE streaming mode
    let Some(sid) = session_id.filter(|_| sessions_on) else {
        return build_json_response(&response, "SSE no-session fallback");
    };
    // A `v1::` OPERATION, not a borrow of the stream map: the zero-sized twin has
    // no map to lend out, so the seam hands ownership of the message across and
    // gets it back only when nothing took it.
    let Some(undelivered) = v1::route_to_session_stream(&state.v1, sid, response) else {
        return StatusCode::ACCEPTED.into_response();
    };
    build_sse_response_from_single_message(undelivered)
}

/// Validate that a provided protocol version is in the supported set.
fn validate_protocol_version_supported(
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    let Some(version) = protocol_version else {
        return Ok(());
    };
    if crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str()) {
        return Ok(());
    }
    Err(create_error_response(
        StatusCode::BAD_REQUEST,
        crate::types::protocol::error_codes::INVALID_REQUEST,
        &format!("Unsupported protocol version: {}", version),
    ))
}

/// Validate the `MCP-Protocol-Version` header (if any) against the supported
/// set and any negotiated session version.
///
/// Refactored in 75-01 Task 1a-A (P2): extracted
/// [`validate_protocol_version_supported`] and
/// [`v1::validate_protocol_version_matches_session`] as early-return chains.
fn validate_protocol_version(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    validate_protocol_version_supported(protocol_version)?;
    v1::validate_protocol_version_matches_session(state, era, session_id, protocol_version)
}

/// Handle POST requests
async fn handle_post_request(
    State(state): State<ServerState>,
    request: axum::extract::Request<Body>,
) -> impl IntoResponse {
    // Fast path: No HTTP middleware chain.
    // `Box::pin` both dispatch futures: the v2 header gate (Plan 112-06) grows the
    // POST future past clippy's large_future threshold; boxing keeps the axum
    // handler future small without changing behavior.
    if state.config.http_middleware.is_none() {
        return Box::pin(handle_post_fast_path(state, request)).await;
    }

    // Middleware path: Process through HTTP middleware chain
    Box::pin(handle_post_with_middleware(state, request)).await
}

/// Extract and validate authentication from headers.
async fn extract_and_validate_auth(
    state: &ServerState,
    headers: &HeaderMap,
) -> std::result::Result<Option<crate::server::auth::AuthContext>, Response> {
    let server = state.server.lock().await;
    if let Some(auth_provider) = server.get_auth_provider() {
        // Extract Authorization header
        let auth_header = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        // Validate the request and get auth context
        match auth_provider.validate_request(auth_header).await {
            Ok(ctx) => Ok(ctx),
            Err(e) => {
                // Auth validation failed - return 401 Unauthorized
                Err(create_error_response(
                    StatusCode::UNAUTHORIZED,
                    crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
                    &format!("Authentication failed: {}", e),
                ))
            },
        }
    } else {
        // No auth provider - try to extract auth from proxy headers (X-PMCP-*)
        // This is used when running behind a proxy that validates auth and forwards claims
        Ok(extract_auth_from_proxy_headers(headers))
    }
}

/// Extract authentication context from proxy-forwarded headers (X-PMCP-*)
///
/// When running behind the pmcp.run proxy or similar, the proxy validates OAuth
/// tokens and forwards user claims as X-PMCP-* headers. This function extracts
/// those headers into an `AuthContext`.
fn extract_auth_from_proxy_headers(
    headers: &HeaderMap,
) -> Option<crate::server::auth::AuthContext> {
    // Check for user ID header (required)
    let user_id = headers
        .get("x-pmcp-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())?;

    // Extract optional claims
    let email = headers
        .get("x-pmcp-user-email")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let name = headers
        .get("x-pmcp-user-name")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let groups = headers
        .get("x-pmcp-user-groups")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let tenant_id = headers
        .get("x-pmcp-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Build claims map
    let mut claims = std::collections::HashMap::new();
    if let Some(ref email) = email {
        claims.insert(
            "email".to_string(),
            serde_json::Value::String(email.clone()),
        );
    }
    if let Some(ref name) = name {
        claims.insert("name".to_string(), serde_json::Value::String(name.clone()));
    }
    if let Some(ref groups) = groups {
        // Parse comma-separated groups into a JSON array so that
        // AuthContext::groups() can deserialize it as Vec<String>.
        let groups_array: Vec<serde_json::Value> = groups
            .split(',')
            .map(|g| serde_json::Value::String(g.trim().to_string()))
            .filter(|v| v.as_str() != Some(""))
            .collect();
        claims.insert("groups".to_string(), serde_json::Value::Array(groups_array));
    }
    if let Some(ref tenant_id) = tenant_id {
        claims.insert(
            "tenant_id".to_string(),
            serde_json::Value::String(tenant_id.clone()),
        );
    }

    // pmcp.run mcp-proxy emits `x-pmcp-claim-custom-<kebab-suffix>: <value>` for every
    // Cognito `custom:*` user attribute it sees in the authorizer context (see
    // rust-mcp-sdk docs/proxy-contract.md). Re-insert each one into `claims` under
    // the canonical Cognito attribute name `custom:<snake_suffix>` so consumers
    // can read either via `ctx.claim::<T>("custom:foo")` or the raw `ctx.claims` map.
    //
    // mcp-proxy strips inbound `x-pmcp-claim-custom-*` from client requests before
    // injection, so every header observed here is platform-trusted.
    for (name, value) in headers {
        let Some(suffix) = name.as_str().strip_prefix("x-pmcp-claim-custom-") else {
            continue;
        };
        let Ok(val_str) = value.to_str() else {
            continue;
        };
        if suffix.is_empty() || val_str.is_empty() {
            continue;
        }
        let snake: String = suffix
            .chars()
            .map(|c| if c == '-' { '_' } else { c })
            .collect();
        claims.insert(
            format!("custom:{}", snake),
            serde_json::Value::String(val_str.to_string()),
        );
    }

    tracing::debug!(
        user_id = %user_id,
        email = ?email,
        "Extracted auth context from proxy headers"
    );

    Some(crate::server::auth::AuthContext {
        subject: user_id,
        scopes: vec![],
        claims,
        token: None,
        client_id: None,
        expires_at: None,
        authenticated: true,
    })
}

/// Extract session ID and protocol version headers from a raw axum `HeaderMap`.
///
/// Shared by both the fast path and middleware-path POST handlers so the two
/// entry points read the same two headers in the same way.
///
/// # Why this function is MIXED, and stays here (plan 117-12 handoff, closed by 117-13)
///
/// It reads two headers of opposite eras. `MCP-Protocol-Version` is v2-REQUIRED
/// (VERS-05), so this function cannot move into the pair; `Mcp-Session-Id` is
/// v1-only, so its read cannot stay inline. The split is therefore INSIDE the
/// function: the v1 read goes through [`v1::incoming_session_header`], whose twin
/// answers `None` without naming a header, and the v2 read stays exactly where it
/// was.
///
/// The consequence on a `full-v2` build is that `session_id` is `None` at the
/// SOURCE rather than being resolved away ten functions later. That is the same
/// value the pipeline already ended up with — every downstream consumer routes
/// through a `v1::` seam whose twin discards it — but produced by a build that
/// never read the header, which is what SMPL-02 asks for.
fn extract_session_and_protocol_headers(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let session_id = v1::incoming_session_header(headers);
    let protocol_version = headers
        .get(MCP_PROTOCOL_VERSION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    (session_id, protocol_version)
}

/// Resolved output of the v2 required-header gate for one request: the
/// `ProtocolContext` (consumed by dispatch) and the outbound-header echo.
type V2GateResolved = (
    Option<crate::types::protocol::ProtocolContext>,
    Option<(String, String)>,
);

/// Run the v2 required-header gate (VERS-05) for one request: resolve the
/// `ProtocolContext` ONCE (consumed by dispatch), classify the header/`_meta`
/// matrix fail-closed, and derive the outbound-header echo.
///
/// # Ordering — load-bearing, not stylistic
///
/// This MUST run BEFORE session resolution (Plan 113-04 / HTTP-01): the ERA
/// decides whether sessions apply at all, so it must be known before the first
/// session decision. It MUST also run BEFORE the legacy protocol-version check,
/// because an accepted v2 request carries `MCP-Protocol-Version: 2026-07-28`,
/// which the static-SUPPORTED check would otherwise reject.
///
/// v1 / non-opted-in → `Passthrough` (zero enforcement, D-04). A
/// `server/discover` ingress runs the SAME matrix via the raw-`_meta`
/// counterpart (finding #1).
///
/// Extracted in plan 113.1-01 (D-06 / D-09): both POST entrypoints carried this
/// block verbatim, so [`run_v2_header_gate`] now has exactly one call site. The
/// middleware path's extra error-hook step lives in the sibling
/// [`resolve_v2_gate_with_error_hook`], following this file's existing
/// plain-fn + `*_with_error_hook` convention.
async fn resolve_v2_gate(
    state: &ServerState,
    headers: &HeaderMap,
    body: &PostBody<'_>,
    ingress: &HttpIngress,
) -> std::result::Result<V2GateResolved, Response> {
    match ingress {
        // Only a REQUEST carries a header contract. `server/discover` pins its
        // method (it is routed by classification, not by the body's `method`
        // field); every other request — including `subscriptions/listen`, whose
        // body DOES carry its method — reads the method from the body.
        HttpIngress::Public(TransportMessage::Request { .. })
        | HttpIngress::Discover { .. }
        | HttpIngress::SubscriptionsListen { .. }
        | HttpIngress::TasksUpdate { .. } => {
            let method_override = matches!(ingress, HttpIngress::Discover { .. })
                .then_some(crate::types::protocol::SERVER_DISCOVER_METHOD);
            let (ctx, gate) = run_v2_header_gate(state, headers, body, method_override).await;
            match gate {
                V2GateOutcome::Reject {
                    code,
                    message,
                    data,
                } => {
                    let era = ctx.as_ref().map(|pc| pc.era);
                    Err(v2_gate_reject_response(
                        body.raw(),
                        era,
                        code,
                        &message,
                        data,
                    ))
                },
                V2GateOutcome::Passthrough => Ok((ctx, None)),
                V2GateOutcome::EnforceOk { method, name } => Ok((ctx, Some((method, name)))),
            }
        },
        HttpIngress::Public(_) => Ok((None, None)),
    }
}

/// Classify a `TransportMessage` as an `initialize` request or not.
///
/// Extracted so both POST handlers can short-circuit protocol-version
/// validation and session creation without re-implementing the `matches!`.
///
/// # Why this is NOT in the `v1` pair
///
/// It reads like v1-only machinery — `initialize` is the 2025-11-25 handshake and
/// the 2026-07-28 transport has none — and plan 117-12 did put it in the pair,
/// with a `const fn … -> false` twin. That was wrong, and the code review of this
/// phase caught it: this function holds **no v1 state at all**. It is a pure
/// `matches!` over a message that both feature sets can receive, because a
/// `full-v2` server still *serves* `initialize` — `v2_verb_rejection` is wired
/// only to GET and DELETE, so an `initialize` POST reaches `Server` core and is
/// dispatched normally.
///
/// With the twin in place, that POST took the non-init branch of
/// [`compute_outbound_protocol_version`] and echoed
/// `MCP-Protocol-Version: 2025-03-26` (the crate default) while its own
/// `InitializeResult` body carried the negotiated `2025-11-25` — a silent
/// protocol downgrade caused purely by the feature set the server was compiled
/// with, since `StreamableHttpTransport` stores the header value and replays it
/// on every subsequent request. A twin is only honest when the caller can
/// correctly handle the constant it returns; here it could not.
///
/// [`update_session_after_init`](v1::update_session_after_init) — the function
/// that actually touches the session map — stays in the pair and keeps its `()`
/// twin. Severance is about STATE, not about which era invented the concept.
///
/// `tests/v2_initialize_negotiated_version_header.rs` fails if either classifier
/// is pushed back into the pair.
fn is_initialize_request(message: &TransportMessage) -> bool {
    matches!(
        message,
        TransportMessage::Request { request: Request::Client(boxed), .. }
            if matches!(**boxed, ClientRequest::Initialize(_))
    )
}

/// Extract the negotiated protocol version from an `initialize` response.
///
/// Ungated for the same reason as [`is_initialize_request`], which see: this is a
/// `serde_json::from_value` over a response payload and holds no v1 state, while
/// a `full-v2` build still produces `InitializeResult` bodies whose
/// `protocolVersion` the outbound header must agree with.
fn extract_negotiated_version(response: &TransportMessage) -> Option<String> {
    if let TransportMessage::Response(ref json_resp) = response {
        if let crate::types::jsonrpc::ResponsePayload::Result(ref value) = json_resp.payload {
            if let Ok(init_result) =
                serde_json::from_value::<crate::types::InitializeResult>(value.clone())
            {
                return Some(init_result.protocol_version.0);
            }
        }
    }
    None
}

/// Compute the outbound `MCP-Protocol-Version` header value.
///
/// Used by both POST handlers to echo the negotiated version from an initialize
/// response, the session's recorded version for subsequent requests, or — when
/// there is no session to recover it from, as on every STATELESS deployment —
/// the version the client itself asserted on the request. Only a request that
/// names no version at all reaches `DEFAULT_PROTOCOL_VERSION`.
fn compute_outbound_protocol_version(
    state: &ServerState,
    response_session_id: Option<&String>,
    is_init_request: bool,
    negotiated_version: Option<&str>,
    asserted_version: Option<&str>,
) -> String {
    if is_init_request {
        return negotiated_version.map_or_else(
            || crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            std::string::ToString::to_string,
        );
    }
    if let Some(sid) = response_session_id {
        // A tracked session with no recorded version and an untracked session
        // both fall through to the default, exactly as before the collapse —
        // which is why `session_protocol_version` may return one `None` for both.
        if let Some(negotiated_version) = v1::session_protocol_version(&state.v1, sid.as_str()) {
            return negotiated_version;
        }
    }
    // A STATELESS server has no session to recover the negotiated version from,
    // so without this it advertised `DEFAULT_PROTOCOL_VERSION` on every request
    // after the handshake — a version LOWER than the one it had just negotiated,
    // which `StreamableHttpTransport` then latches and replays. The client is
    // dragged down with it, and nothing decided the downgrade except whether the
    // deployment was serverless. `tests/stateless_negotiated_version_header.rs`
    // is the live-HTTP fence; the sibling defect on the init branch is
    // `tests/v2_initialize_negotiated_version_header.rs`.
    //
    // The client asserted the version on THIS request, which is the only place
    // a session-less server can still read it.
    if let Some(known) = asserted_version.and_then(crate::types::protocol::known_protocol_version) {
        return known.to_string();
    }
    crate::DEFAULT_PROTOCOL_VERSION.to_string()
}

/// [`compute_outbound_protocol_version`] for a request that is NOT `initialize`.
///
/// Four of the six emission sites pass the same constant pair — `false, None` —
/// because they answer a method that can never be the handshake. Naming that
/// pair once removes it from four call sites and, more usefully, means the next
/// input threaded through this path costs ONE edit rather than four. Threading
/// `asserted_version` through cost five, which is what prompted this.
///
/// The two sites that pass a real runtime `is_init_request`
/// (`handle_fast_path_request` and `dispatch_message_with_middleware`) keep the
/// general form.
fn outbound_protocol_version_after_init(
    state: &ServerState,
    response_session_id: Option<&String>,
    asserted_version: Option<&str>,
) -> String {
    compute_outbound_protocol_version(state, response_session_id, false, None, asserted_version)
}

/// Best-effort error-hook dispatch for the middleware path.
///
/// Wraps the `http_middleware.handle_error` call so the caller can short-circuit
/// to a `Response` without a second level of match nesting. The middleware's
/// error hook is intentionally fire-and-forget (return value ignored) — we do
/// not want a misbehaving hook to mask the original failure.
async fn report_middleware_error(
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
    error_kind: &str,
) {
    let err = crate::Error::protocol_msg(error_kind);
    let _ = http_middleware.handle_error(&err, context).await;
}

/// Run request-side middleware and return an error response if rejected.
///
/// Consolidates the `process_request` + error-hook-then-return pattern used
/// at the top of [`handle_post_with_middleware`].
async fn run_request_middleware(
    http_middleware: &ServerHttpMiddlewareChain,
    server_request: &mut crate::server::http_middleware::ServerHttpRequest,
    context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if let Err(e) = http_middleware
        .process_request(server_request, context)
        .await
    {
        let _ = http_middleware.handle_error(&e, context).await;
        return Err(create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            &format!("Middleware rejected request: {}", e),
        ));
    }
    Ok(())
}

/// Parse a JSON-RPC message from raw bytes with middleware-aware error handling.
///
/// On parse failure, runs the request-side response middleware over a
/// manufactured 400 response so downstream observers (logging, metrics) still
/// see the failure.
async fn parse_transport_message_with_middleware(
    body: &[u8],
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> std::result::Result<HttpIngress, Response> {
    // Classify an internally-routed `server/discover` request first; every other
    // body keeps the existing middleware-aware parse + 400 assembly path.
    if let Some(ingress) = classify_http_ingress(body) {
        return Ok(ingress);
    }
    match crate::shared::StdioTransport::parse_message(body) {
        Ok(msg) => Ok(HttpIngress::Public(msg)),
        Err(e) => {
            let mut error_response = ServerHttpResponse::new(
                StatusCode::BAD_REQUEST,
                HeaderMap::new(),
                format!("{{\"error\":\"Invalid JSON: {}\"}}", e).into_bytes(),
            );
            let _ = http_middleware
                .process_response(&mut error_response, context)
                .await;
            Err(into_axum(error_response))
        },
    }
}

/// Extract and validate authentication for the middleware POST path.
///
/// Mirrors [`extract_and_validate_auth`] but wires the middleware error hook
/// into the 401 path. Returns `Ok(None)` when no auth provider is configured
/// (matching the existing middleware-path behavior, which does NOT fall back
/// to proxy-header extraction).
async fn extract_auth_with_middleware(
    state: &ServerState,
    server_request: &crate::server::http_middleware::ServerHttpRequest,
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> std::result::Result<Option<crate::server::auth::AuthContext>, Response> {
    let server = state.server.lock().await;
    let Some(auth_provider) = server.get_auth_provider() else {
        return Ok(None);
    };
    let auth_header = server_request.get_header("authorization");
    match auth_provider.validate_request(auth_header).await {
        Ok(ctx) => Ok(ctx),
        Err(e) => {
            let auth_error = crate::Error::authentication(format!("Authentication failed: {}", e));
            let _ = http_middleware.handle_error(&auth_error, context).await;
            Err(create_error_response(
                StatusCode::UNAUTHORIZED,
                crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
                &format!("Authentication failed: {}", e),
            ))
        },
    }
}

/// Assemble the JSON-RPC success response + headers, run response middleware,
/// and convert to an axum `Response`.
///
/// Returns either the built axum response or a 500 error response when
/// serialization fails.
async fn build_success_response_with_middleware(
    response_msg: &TransportMessage,
    response_session_id: Option<&String>,
    version_to_send: &str,
    sessions_on: bool,
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> Response {
    let response_body = match serde_json::to_vec(response_msg) {
        Ok(b) => b,
        Err(e) => {
            let serialization_error =
                crate::Error::internal(format!("Failed to serialize response: {}", e));
            let _ = http_middleware
                .handle_error(&serialization_error, context)
                .await;
            return create_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                &format!("Failed to serialize response: {}", e),
            );
        },
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, APPLICATION_JSON.parse().unwrap());
    v1::apply_session_header(&mut response_headers, response_session_id, sessions_on);
    response_headers.insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    finish_with_middleware(response_headers, response_body, http_middleware, context).await
}

/// Run an ALREADY-COMPLETE header set and body through the response middleware
/// chain and convert to an axum `Response`.
///
/// THE single site that calls [`ServerHttpMiddlewareChain::process_response`] on
/// the middleware POST path. Extracted so the multi-frame SSE branch cannot skip
/// it: that branch builds a complete byte buffer exactly like the JSON branch —
/// the handler has already run — so an operator's response middleware must see
/// it too. A branch that built its own `Response` and returned it directly would
/// silently disable every configured response transform for precisely the
/// requests that carry progress.
async fn finish_with_middleware(
    headers: HeaderMap,
    body: Vec<u8>,
    http_middleware: &ServerHttpMiddlewareChain,
    context: &ServerHttpContext,
) -> Response {
    let mut server_response = ServerHttpResponse::new(StatusCode::OK, headers, body);

    if let Err(e) = http_middleware
        .process_response(&mut server_response, context)
        .await
    {
        tracing::warn!("Response middleware processing failed: {}", e);
    }

    into_axum(server_response)
}

/// Fast path handler without HTTP middleware
/// Read the axum request body with enforced byte limit.
///
/// Returns the body bytes as a `String` on success, or a 413 error response
/// when the body exceeds `max_bytes`.
async fn read_body_with_limit(
    body: Body,
    max_bytes: usize,
) -> std::result::Result<String, Response> {
    let body_bytes = axum::body::to_bytes(body, max_bytes).await.map_err(|e| {
        create_error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            crate::types::protocol::error_codes::INVALID_REQUEST,
            &format!("Request body exceeds limit: {}", e),
        )
    })?;
    Ok(String::from_utf8_lossy(&body_bytes).to_string())
}

/// Parse a JSON-RPC message on the fast path, returning a 400 error response
/// on failure.
///
/// Classifies an internally-routed `server/discover` request as
/// [`HttpIngress::Discover`] (which then CONTINUES the pipeline); every other
/// body flows through the existing [`StdioTransport::parse_message`] path as
/// [`HttpIngress::Public`], so all non-discover parse bytes are byte-identical.
fn parse_transport_message_fast(body: &[u8]) -> std::result::Result<HttpIngress, Response> {
    if let Some(ingress) = classify_http_ingress(body) {
        return Ok(ingress);
    }
    crate::shared::StdioTransport::parse_message(body)
        .map(HttpIngress::Public)
        .map_err(|e| {
            create_error_response(
                StatusCode::BAD_REQUEST,
                crate::types::protocol::error_codes::PARSE_ERROR,
                &format!("Invalid JSON: {}", e),
            )
        })
}

/// Handle the successful-request arm on the fast path: dispatch to the
/// server, persist event, and attach session/version headers to the response.
/// Per-request dispatch inputs threaded into the fast-path handler.
///
/// Bundles the response-shaping flags with the Plan-04-resolved
/// `ProtocolContext` (threaded into dispatch, never re-resolved — Plan 06) and
/// the optional v2 outbound headers to echo on success AND error.
struct FastPathDispatch {
    is_init_request: bool,
    response_session_id: Option<String>,
    /// The `MCP-Protocol-Version` the client asserted — see the field of the
    /// same name on [`InternalResponseShape`].
    asserted_protocol_version: Option<String>,
    /// Plan-04-resolved `ProtocolContext`, CONSUMED at dispatch (D-11).
    protocol_context: Option<crate::types::protocol::ProtocolContext>,
    /// When `Some((method, name))`, this is an accepted v2 request whose
    /// response echoes `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version`.
    v2_outbound: Option<(String, String)>,
    /// [`v1::sessions_active`] for THIS request — gates the `Mcp-Session-Id`
    /// response header (HTTP-01).
    sessions_on: bool,
    /// This request's bounded v2 progress queue (CONF-07 / D-16), or `None` when
    /// the request is not eligible for a multi-frame SSE body.
    ///
    /// Created BEFORE dispatch so the handler's sink has somewhere to write, and
    /// carried here so the receiver survives to the response builder.
    progress_queue: Option<V2ProgressQueue>,
}

async fn handle_fast_path_request(
    state: &ServerState,
    id: crate::types::RequestId,
    request: Request,
    auth_context: Option<crate::server::auth::AuthContext>,
    dispatch: FastPathDispatch,
    session_id: Option<&String>,
) -> Response {
    let FastPathDispatch {
        is_init_request,
        response_session_id,
        asserted_protocol_version,
        protocol_context,
        v2_outbound,
        sessions_on,
        progress_queue,
    } = dispatch;

    let era = protocol_context.as_ref().map(|pc| pc.era);
    // Captured BEFORE dispatch consumes it: this is the LIVE request's id, and
    // it is the only id the direct response may carry (HTTP-05).
    let live_id = id.clone();
    // Thread the ALREADY-RESOLVED ProtocolContext into dispatch — the HTTP layer
    // resolved it once for the header gate; dispatch does NOT re-resolve (Plan 06
    // / D-11 / Pitfall 2). Every method the 2026-07-28 schema retired was already
    // refused by the v2 ingress gate, which ran before this.
    let json_response =
        dispatch_public_request(state, id, request, auth_context, protocol_context).await;

    tracing::debug!(
        target: "mcp.http",
        response = %serde_json::to_string(&json_response).unwrap_or_default(),
        "StreamableHttpServer response"
    );

    // Code-driven v2 status: an error the HANDLER produced (e.g. -32601 for an
    // unsupported method, or plan 09's -32021) maps to its spec HTTP status.
    // `None` on v1 / not-opted-in, so every legacy status is unchanged.
    let v2_status = v2_dispatch_response_status(era, &json_response);

    // Re-envelope the dispatch PAYLOAD onto the live id. Whatever produced the
    // payload — a handler, a cache, a shared `Arc` — it reaches the wire inside
    // an envelope that structurally cannot carry anyone else's id.
    let response_msg =
        TransportMessage::Response(envelope_for_live_request(json_response.payload, live_id));

    let negotiated_version = if is_init_request {
        let version = extract_negotiated_version(&response_msg);
        v1::update_session_after_init(state, response_session_id.as_ref(), version.clone());
        version
    } else {
        None
    };

    v1::store_response_event(state, era, response_session_id.as_ref(), &response_msg).await;

    // CONF-07 / D-16: if this v2 request queued progress, its response body
    // becomes a multi-frame SSE stream. `None` — no queue, or a queue the handler
    // never wrote to — falls through to the UNCHANGED builder, which is what
    // keeps a no-progress v2 response byte-identical to today's.
    let mut response = match take_v2_progress_frames(progress_queue, response_msg) {
        Ok((progress, result)) => build_v2_multi_frame_sse_response(progress, result),
        Err(response_msg) => build_response(state, response_msg, session_id, sessions_on),
    };

    v1::apply_session_header(
        response.headers_mut(),
        response_session_id.as_ref(),
        sessions_on,
    );

    let version_to_send = compute_outbound_protocol_version(
        state,
        response_session_id.as_ref(),
        is_init_request,
        negotiated_version.as_deref(),
        asserted_protocol_version.as_deref(),
    );
    response
        .headers_mut()
        .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    // v2 outbound headers (VERS-05): echoed on BOTH the handler's success and its
    // structured JSON-RPC error, built without panicking. Overwrites the
    // MCP-Protocol-Version above with the v2 value for an accepted v2 request.
    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }

    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }

    response
}

/// Assemble the `server/discover` response on the fast path (Phase 112, VERS-04).
///
/// Runs the SAME response tail as any fast-path request — projects via
/// [`Server::handle_discover`](crate::server::Server::handle_discover) (the ONE
/// shared `build_discover_response` era gate), stores the response event, builds
/// the response, and attaches session/version/outbound-v2 headers — preserving
/// the ORIGINAL request id. This is reached only AFTER session resolution, the v2
/// header matrix, legacy-version validation, and auth (classify-then-continue —
/// no pipeline bypass).
///
/// Response-shaping inputs shared by every INTERNALLY-ROUTED request/response
/// assembler, so the fast and middleware paths can never drift on session-header
/// gating or the v2 outbound echo.
///
/// Two methods use it today — `server/discover` (Phase 112) and `tasks/update`
/// (Phase 114 plan 13) — which is why it is not named after either. Both are
/// classified out of the public-enum path by
/// [`classify_http_ingress`] and both answer with a single JSON-RPC response, so
/// both run the identical response tail. `subscriptions/listen` deliberately does
/// NOT use it: it answers with a held-open SSE stream that has no complete body
/// and therefore no response-middleware or session-header step.
struct InternalResponseShape<'a> {
    /// The session id to echo, if any — already `None` on v2.
    response_session_id: Option<&'a String>,
    /// The `MCP-Protocol-Version` the CLIENT asserted on this request.
    ///
    /// The only source a STATELESS server has for the negotiated version, since
    /// it keeps no session to look one up in — see
    /// [`compute_outbound_protocol_version`].
    asserted_protocol_version: Option<&'a str>,
    /// `Some((method, name))` for an accepted v2 discover (VERS-05 echo).
    v2_outbound: Option<(String, String)>,
    /// [`v1::sessions_active`] for THIS request (HTTP-01).
    sessions_on: bool,
}

/// D-10 decision (finding #4): a v2 connection projects the server's
/// already-computed capabilities (incl. the `extensions` map); a v1 /
/// non-opted-in connection returns JSON-RPC `-32601` at HTTP 200 with the
/// original id. This `-32601@200` is a DELIBERATE, benign change from the
/// pre-112 incidental `PARSE_ERROR` 400 (`id: null`) — justified because
/// `server/discover` is a v2-only method NO conforming v1 client sends, so no
/// v1-relied-upon response byte changes (milestone byte-identity reconciled).
async fn assemble_discover_response_fast(
    state: &ServerState,
    id: crate::types::RequestId,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    shape: InternalResponseShape<'_>,
    session_id: Option<&String>,
) -> Response {
    let InternalResponseShape {
        response_session_id,
        asserted_protocol_version,
        v2_outbound,
        sessions_on,
    } = shape;
    let live_id = id.clone();
    let json_response = {
        let server = state.server.lock().await;
        server.handle_discover(id, protocol_context)
    };
    let era = protocol_context.map(|pc| pc.era);
    let v2_status = v2_dispatch_response_status(era, &json_response);
    // Same structural guarantee as every other direct response (HTTP-05).
    let response_msg =
        TransportMessage::Response(envelope_for_live_request(json_response.payload, live_id));

    v1::store_response_event(state, era, response_session_id, &response_msg).await;

    let mut response = build_response(state, response_msg, session_id, sessions_on);

    v1::apply_session_header(response.headers_mut(), response_session_id, sessions_on);

    // Discover is never an init request → compute the outbound version normally.
    let version_to_send =
        outbound_protocol_version_after_init(state, response_session_id, asserted_protocol_version);
    response
        .headers_mut()
        .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    // Echo the v2 outbound headers on an accepted v2 discover (VERS-05).
    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }

    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }

    response
}

// ===========================================================================
// `tasks/update` (Phase 114 plan 13, TASK-02).
// ===========================================================================

/// The four inputs `TaskDispatch::route_tasks_update` consumes, carried as ONE
/// value.
///
/// Bundled rather than passed as four parameters because the middleware assembler
/// would otherwise take 8 arguments and trip `clippy::too_many_arguments` (7) —
/// MEASURED in the plan-13 quality gate, not anticipated. The grouping is not
/// arbitrary: these are exactly the router's inputs, and
/// [`InternalResponseShape`] beside it is exactly the response tail's, so the two
/// assemblers below read as "route with these, then shape with those".
struct TasksUpdateCall<'a> {
    /// The ORIGINAL JSON-RPC request id.
    id: crate::types::RequestId,
    /// The request's `params`, RAW and undecoded — nothing between the wire and
    /// the router deserializes them.
    params: serde_json::Value,
    /// The context resolved ONCE at ingress and CONSUMED here (D-11).
    protocol_context: Option<&'a crate::types::protocol::ProtocolContext>,
    /// The value [`extract_and_validate_auth`] already produced.
    auth_context: Option<&'a crate::server::auth::AuthContext>,
}

/// Run the `tasks/update` GATE chain and produce its JSON-RPC response.
///
/// THE single place this transport reaches the tasks router for `tasks/update`,
/// shared by the fast and middleware assemblers below so they cannot drift on
/// which gates ran or in what order. It holds the server lock for exactly the
/// delegate call, the same way both `server/discover` assemblers do.
///
/// It contains NO gate itself. `Server::handle_tasks_update` is a thin delegate
/// onto `TaskDispatch::route_tasks_update`, which owns the whole ordered chain:
/// era → backend → client declaration (`-32021`) → auth (`-32003`) → params
/// (`-32602`). The `auth_context` is threaded through unchanged —
/// `tasks/update` is subject to the SAME auth as every other request on this
/// transport.
async fn tasks_update_json_response(
    state: &ServerState,
    call: &TasksUpdateCall<'_>,
) -> crate::types::JSONRPCResponse {
    let server = state.server.lock().await;
    server
        .handle_tasks_update(
            call.id.clone(),
            &call.params,
            call.auth_context,
            call.protocol_context,
        )
        .await
}

/// Assemble the `tasks/update` response on the fast path (TASK-02).
///
/// Structurally the twin of [`assemble_discover_response_fast`] and it shares that
/// function's [`InternalResponseShape`] and response tail verbatim in shape:
/// store the response event, build the response, attach session / version /
/// outbound-v2 headers, apply the code-driven v2 status. Reached only AFTER
/// session resolution, the v2 header matrix, legacy-version validation and auth —
/// classify-then-continue, no pipeline bypass.
///
/// # The v1 answer, and why it is a deliberate change
///
/// `tasks/update` does not exist on MCP 2025-11-25, so a v1 caller receives
/// JSON-RPC `-32601` at HTTP 200 with the ORIGINAL id, where before plan 13 the
/// unrecognised method produced a `PARSE_ERROR` at HTTP 400 with `id: null`. Same
/// decision, same justification as `server/discover`'s D-10 finding #4: no
/// conforming v1 client sends a v2-only method, so no v1-relied-upon response byte
/// moves.
async fn assemble_tasks_update_fast(
    state: &ServerState,
    call: TasksUpdateCall<'_>,
    shape: InternalResponseShape<'_>,
    session_id: Option<&String>,
) -> Response {
    let InternalResponseShape {
        response_session_id,
        asserted_protocol_version,
        v2_outbound,
        sessions_on,
    } = shape;
    let live_id = call.id.clone();
    let protocol_context = call.protocol_context;
    let json_response = tasks_update_json_response(state, &call).await;
    let era = protocol_context.map(|pc| pc.era);
    let v2_status = v2_dispatch_response_status(era, &json_response);
    // Same structural guarantee as every other direct response (HTTP-05).
    let response_msg =
        TransportMessage::Response(envelope_for_live_request(json_response.payload, live_id));

    v1::store_response_event(state, era, response_session_id, &response_msg).await;

    let mut response = build_response(state, response_msg, session_id, sessions_on);

    v1::apply_session_header(response.headers_mut(), response_session_id, sessions_on);

    // `tasks/update` is never an init request → compute the outbound version
    // normally.
    let version_to_send =
        outbound_protocol_version_after_init(state, response_session_id, asserted_protocol_version);
    response
        .headers_mut()
        .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

    // Echo the v2 outbound headers on BOTH success and structured error (VERS-05).
    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }

    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }

    response
}

/// Assemble the `tasks/update` response on the middleware path (TASK-02).
///
/// The middleware-path twin of [`assemble_tasks_update_fast`], differing ONLY in
/// the response-BUILDING step ([`build_success_response_with_middleware`] instead
/// of [`build_response`] + [`v1::apply_session_header`]) — this file's established
/// fast/middleware split. The gate chain is identical because both call the SAME
/// [`tasks_update_json_response`].
async fn assemble_tasks_update_with_middleware(
    state: &ServerState,
    call: TasksUpdateCall<'_>,
    shape: InternalResponseShape<'_>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    let InternalResponseShape {
        response_session_id,
        asserted_protocol_version,
        v2_outbound,
        sessions_on,
    } = shape;
    let live_id = call.id.clone();
    let protocol_context = call.protocol_context;
    let json_response = tasks_update_json_response(state, &call).await;
    let era = protocol_context.map(|pc| pc.era);
    let v2_status = v2_dispatch_response_status(era, &json_response);
    let response_msg =
        TransportMessage::Response(envelope_for_live_request(json_response.payload, live_id));

    v1::store_response_event(state, era, response_session_id, &response_msg).await;

    let version_to_send =
        outbound_protocol_version_after_init(state, response_session_id, asserted_protocol_version);

    let mut response = build_success_response_with_middleware(
        &response_msg,
        response_session_id,
        &version_to_send,
        sessions_on,
        http_middleware,
        http_context,
    )
    .await;

    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }
    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }
    response
}

// ===========================================================================
// `subscriptions/listen` (Plan 113-10, HTTP-04).
//
// # Two conformant configurations, one predicate
//
// The official conformance suite gates the requirement on capability
// advertisement (`src/scenarios/server/stateless.ts:975-1015`, quoted verbatim
// in [`advertises_subscriptions`]):
//
//   * advertise NONE of `tools.listChanged` / `prompts.listChanged` /
//     `resources.listChanged` / `resources.subscribe` -> `-32601` on
//     `subscriptions/listen` is a legitimate feature absence (SKIPPED). This is
//     pmcp's stateless enterprise DEFAULT, and it honors D-11.
//   * advertise ANY of them -> the stream MUST be served; rejecting the method
//     is a FAILURE ("claims a feature it does not serve").
//
// Both the `server/discover` projection (which publishes the capabilities) and
// this route gate read the ONE shared `advertises_subscriptions` predicate over
// the SAME `Server::capabilities()` value, so the advertisement and the
// implementation cannot drift. `tests/v2_subscriptions.rs` carries the live
// tripwire over all four capabilities individually.
//
// # `resources/subscribe` / `resources/unsubscribe` are retired on v2
//
// Both are GONE from the 2026-07-28 schema — the only surviving mention is the
// "Replaces the former `resources/subscribe` RPC" comment on
// `SubscriptionFilter.resourceSubscriptions`. On v2 they answer `404` + `-32601`
// through the [`V2_RETIRED_METHODS`] table at the v2 ingress, alongside the three
// other RPCs that era removes; the v1 path is completely untouched.
// ===========================================================================

/// Disable proxy response buffering so SSE frames reach the client immediately.
///
/// Spec, D-12 RESOLUTION item 6: servers "SHOULD set `X-Accel-Buffering: no`".
const X_ACCEL_BUFFERING: &str = "x-accel-buffering";

/// How often a quiet listen stream emits an SSE comment keep-alive.
const LISTEN_KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);

/// Dispatch a public request through the server core.
///
/// THE single dispatch seam both POST entrypoints call, threading the
/// ALREADY-RESOLVED `ProtocolContext` in rather than letting dispatch re-resolve
/// the era (D-11).
///
/// # It no longer retires anything, and that is the point
///
/// Until Phase 118.1 plan 05 this function ALSO carried the v2 retirement rule,
/// keyed on the parsed `ClientRequest` variant. A variant match can only ever see
/// a request whose params already deserialized, so it was structurally incapable
/// of retiring the shape a conformance client actually sends — `initialize` and
/// `logging/setLevel` with `_meta`-only params never reach a typed `Request` at
/// all. The rule therefore moved to [`retire_v2_method`], which reads the method
/// STRING at the v2 ingress; both POST entrypoints run that gate BEFORE they
/// reach this seam, so there is still exactly ONE place retirement is decided.
/// Do not reintroduce a second, variant-keyed predicate here.
async fn dispatch_public_request(
    state: &ServerState,
    id: crate::types::RequestId,
    request: Request,
    auth_context: Option<crate::server::auth::AuthContext>,
    protocol_context: Option<crate::types::protocol::ProtocolContext>,
) -> crate::types::JSONRPCResponse {
    let server = state.server.lock().await;
    server
        .handle_request_with_context(id, request, auth_context, protocol_context)
        .await
}

/// Everything the listen route needs from the server, read ONCE under the
/// server lock.
///
/// Holds only what cannot be derived: whether the route is advertised is
/// [`crate::types::subscriptions::advertises_subscriptions`] of `capabilities`,
/// so caching it here would be a second copy that can drift from its own source.
struct ListenServerView {
    /// The server's advertised capabilities, for the agreed-filter intersection
    /// and the advertisement gate.
    capabilities: crate::types::ServerCapabilities,
    /// The server identity the v2 result envelope publishes.
    info: crate::types::Implementation,
    /// The registry the accepted stream registers with.
    registry: Arc<crate::server::subscriptions::ListenRegistry>,
    /// Whether this server has an auth provider configured — the FAIL-CLOSED
    /// input to [`resolve_listen_principal`] (D-113-N).
    ///
    /// Read HERE, under the one lock acquisition this struct exists to make, and
    /// nowhere else on the listen path: a second `get_auth_provider()` call
    /// would be both a second lock and a second place the decision could drift
    /// from the MRTR ingress it now mirrors.
    has_auth_provider: bool,
}

/// Read the listen route's view of the server under ONE lock acquisition.
///
/// The registry is taken here rather than re-locking at registration time: the
/// whole point of this struct is that the listen route touches the server mutex
/// — which serializes all dispatch on this transport — exactly once.
async fn listen_server_view(state: &ServerState) -> ListenServerView {
    let server = state.server.lock().await;
    ListenServerView {
        capabilities: server.capabilities().clone(),
        info: server.info().clone(),
        registry: Arc::clone(server.listen_registry()),
        // The EXISTING public accessor (`src/server/mod.rs`), not a new seam and
        // not a widened field.
        has_auth_provider: server.get_auth_provider().is_some(),
    }
}

/// Assemble a JSON-RPC error for a `subscriptions/listen` request that is not
/// served, with the ORIGINAL request id.
///
/// Built through plan 08's [`envelope_for_live_request`] — the ONE direct-response
/// constructor on this transport — so a stale id is structurally unconstructible
/// here too. The status is code-driven via [`v2_dispatch_response_status`]: `404`
/// for `-32601` on v2, and the response's existing `200` on v1.
///
/// D-10 parity note: a v1 / non-opted-in `subscriptions/listen` previously fell
/// out of the typed parse as `400` + `-32700`. It now answers `-32601` at `200`,
/// the same DELIBERATE, benign change Phase 112 made for `server/discover` and
/// for the same reason — `subscriptions/listen` is a v2-only method that no
/// conforming v1 client sends, so no v1-relied-upon response byte changes.
fn listen_rejection_response(
    era: Option<crate::types::protocol::Era>,
    id: crate::types::RequestId,
    code: i32,
    message: String,
) -> Response {
    let response = envelope_for_live_request(
        crate::types::jsonrpc::ResponsePayload::Error(crate::types::jsonrpc::JSONRPCError {
            code,
            message,
            data: None,
        }),
        id,
    );
    let status = v2_dispatch_response_status(era, &response);
    let mut http = build_json_response(
        &TransportMessage::Response(response),
        "subscriptions/listen gate",
    );
    if let Some(status) = status {
        *http.status_mut() = status;
    }
    http
}

/// The acknowledgement frame — the FIRST message on every listen stream.
///
/// Its `notifications` field is the AGREED filter (the intersection of what was
/// requested and what this server supports), never a superset of the request,
/// and its `_meta` carries [`SUBSCRIPTION_ID_META_KEY`](crate::types::subscriptions::SUBSCRIPTION_ID_META_KEY).
///
/// It is a NOTIFICATION, not a result, so it cannot carry the v2 result envelope
/// ([`inject_v2_result_envelope`](crate::server::core::inject_v2_result_envelope)
/// returns early on a non-`Result` payload by design). The `_meta` it does carry
/// is built by the SAME `subscription_id_meta` helper the terminal result uses,
/// so the two can never disagree on the key spelling.
fn listen_ack_frame(
    agreed: &crate::types::subscriptions::SubscriptionFilter,
    subscription_id: &crate::types::RequestId,
) -> String {
    let params = crate::types::subscriptions::SubscriptionAcknowledgedParams::new(
        agreed.clone(),
        subscription_id,
    );
    json!({
        "jsonrpc": "2.0",
        "method": crate::types::subscriptions::ACKNOWLEDGED_METHOD,
        "params": params,
    })
    .to_string()
}

/// The graceful-teardown JSON-RPC response for a listen stream.
///
/// Routed through plan 09's [`inject_v2_result_envelope`](crate::server::core::inject_v2_result_envelope)
/// (which delegates to `own_reserved_result_fields`) exactly like every other v2
/// result, so `resultType` and `io.modelcontextprotocol/serverInfo` are identical
/// to any other v2 response instead of coming from a bespoke frame builder.
fn listen_terminal_result_frame(
    subscription_id: &crate::types::RequestId,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    server_info: &crate::types::Implementation,
) -> String {
    let result = crate::types::subscriptions::SubscriptionsListenResult::new(subscription_id);
    let mut response = envelope_for_live_request(
        crate::types::jsonrpc::ResponsePayload::Result(
            serde_json::to_value(result).unwrap_or_else(|_| json!({})),
        ),
        subscription_id.clone(),
    );
    crate::server::core::inject_v2_result_envelope(
        &mut response,
        protocol_context,
        server_info,
        crate::server::core::ResponseDisposition::Complete,
        // A listen teardown mints no reserved MRTR/tasks field.
        crate::server::core::ReservedFieldOwner::None,
        // `SubscriptionsListenResult` does not extend `CacheableResult` in the
        // 2026-07-28 schema, so this frame carries no caching hint (D-07).
        crate::types::caching::Cacheable::No,
    );
    serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string())
}

/// Frame one queued listen payload as an SSE event.
///
/// A [`ListenFrame::Comment`](crate::server::subscriptions::ListenFrame) becomes
/// an SSE comment line rather than a `message` event, which is how the
/// buffer-overflow notice reaches a client without impersonating a protocol
/// message.
fn listen_sse_event(frame: crate::server::subscriptions::ListenFrame) -> Event {
    match frame {
        crate::server::subscriptions::ListenFrame::Message(payload) => {
            Event::default().event("message").data(payload)
        },
        crate::server::subscriptions::ListenFrame::Comment(text) => Event::default().comment(text),
    }
}

/// Attach the listen stream's response headers: the v2 outbound echo (VERS-05),
/// `X-Accel-Buffering: no`, and no-transform caching.
///
/// The `Mcp-Session-Id` header is NEVER attached: there are no sessions on v2
/// (HTTP-01), so `v1::attach_sse_response_headers` — which requires one — is
/// deliberately not reused here. (Plain code span, not an intra-doc link: that
/// helper is private to the v1 half, so a link would not resolve from here.)
fn attach_listen_response_headers(response: &mut Response, v2_outbound: Option<&(String, String)>) {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(X_ACCEL_BUFFERING, HeaderValue::from_static("no"));
    if let Some((method, name)) = v2_outbound {
        apply_v2_outbound_headers(headers, method, name);
    }
}

/// The AGREED filter of a `subscriptions/listen` request, or the rejection that
/// answers it instead.
///
/// Extracted so [`assemble_subscriptions_listen`] stays a short pipeline well
/// under the cognitive-complexity gate.
fn resolve_agreed_filter(
    params: Option<serde_json::Value>,
    view: &ListenServerView,
) -> std::result::Result<crate::types::subscriptions::SubscriptionFilter, (i32, String)> {
    use crate::types::protocol::error_codes::INVALID_PARAMS;
    use crate::types::subscriptions::SubscriptionsListenParams;

    let Some(value) = params else {
        return Err((
            INVALID_PARAMS,
            "Invalid subscriptions/listen params: `notifications` is required".to_string(),
        ));
    };
    let parsed = serde_json::from_value::<SubscriptionsListenParams>(value).map_err(|e| {
        (
            INVALID_PARAMS,
            format!("Invalid subscriptions/listen params: {e}"),
        )
    })?;
    Ok(parsed
        .notifications
        .intersect_with_capabilities(&view.capabilities))
}

/// Resolve the listen stream's concurrency-accounting principal, FAIL-CLOSED.
///
/// The SIBLING this mirrors is `crate::server::core`'s `resolve_mrtr_principal`
/// (its `MrtrPrincipal` carries the same two inputs), so the two v2 ingress
/// paths on ONE server give the SAME answer to "what is an unauthenticated
/// caller":
///
/// * an `AuthContext` is present → its `subject`;
/// * no `AuthContext` but an auth provider IS configured → `None`, i.e. REFUSE;
/// * no auth provider at all → a fresh
///   [`anonymous_principal`](crate::server::subscriptions::anonymous_principal).
///
/// # The defect this closes (D-113-N)
///
/// Before this function the route minted a fresh `anon#N` whenever
/// `auth_context` was `None` with no `has_auth_provider` check, so on a server
/// whose provider ADMITS unauthenticated requests every unauthenticated listen
/// received a private, uncapped identity —
/// `MAX_LISTEN_STREAMS_PER_PRINCIPAL` never bound and one caller could hold all
/// `MAX_LISTEN_STREAMS_TOTAL` global slots, starving authenticated subscribers.
///
/// # Why the third row deliberately does NOT collapse onto MRTR's shared constant
///
/// This is a DECISION, not an oversight — do not "simplify" the two rows into
/// one. MRTR needs a STABLE principal string on a no-auth server because that
/// principal is AEAD additional-authenticated-data: a per-request `anon#N` would
/// make every round-2 `requestState` fail to verify, which is exactly why
/// `resolve_mrtr_principal` answers with one shared `ANONYMOUS_PRINCIPAL`. This
/// route has no such binding — its principal is ONLY a concurrency-accounting
/// key. Unifying them would silently drop a no-auth server from
/// `MAX_LISTEN_STREAMS_TOTAL` (64) concurrent streams to
/// `MAX_LISTEN_STREAMS_PER_PRINCIPAL` (4), which is the common local/dev
/// configuration and the one the shipped `s47_v2_stateless_mrtr` /
/// `s48_v2_mrtr_client` examples use. The regression guard is
/// `unauthenticated_listen_still_serves_on_a_server_with_no_auth_provider` in
/// `tests/v2_subscriptions.rs`.
fn resolve_listen_principal(
    auth_context: Option<&crate::server::auth::AuthContext>,
    has_auth_provider: bool,
) -> Option<String> {
    match (auth_context, has_auth_provider) {
        (Some(context), _) => Some(context.subject.clone()),
        (None, true) => None,
        (None, false) => Some(crate::server::subscriptions::anonymous_principal()),
    }
}

/// Serve — or conformantly reject — a `subscriptions/listen` request (HTTP-04).
///
/// THE single implementation both POST entrypoints call, so the fast and
/// middleware paths cannot drift on the gate, the agreed filter or the frame
/// order. The response-middleware chain is deliberately NOT run over a listen
/// stream: it processes a complete `Vec<u8>` body, and this response has no
/// complete body by construction.
///
/// # Rejection cases, in order
///
/// 1. era is not v2 -> `-32601` (`subscriptions/listen` does not exist on v1);
/// 2. no subscription-delivered capability advertised -> `-32601`, the
///    conformant-by-absence configuration;
/// 3. an unauthenticated caller on a server that HAS an auth provider ->
///    `-32003` (`AUTHENTICATION_REQUIRED`) at HTTP 200 (D-113-N). Placed HERE
///    deliberately: after the two `-32601` gates, so a v1 or capability-less
///    server keeps answering "no such method" rather than advertising that it
///    authenticates; before the params parse, so the refusal never depends on
///    an unauthenticated caller's body; and before `registry.register`, so a
///    refused caller never takes a permit. The decision itself lives in
///    [`resolve_listen_principal`], which mirrors the MRTR ingress;
/// 4. `params` that do not deserialize (`notifications` is REQUIRED) ->
///    `-32602`, AFTER the header gate and auth have already run;
/// 5. the per-principal or global concurrency cap is exhausted -> `-32005`
///    (`RATE_LIMITED`) at HTTP 200, carrying a JSON-RPC error body;
/// 6. a duplicate LIVE `(principal, subscriptionId)` -> ALSO `-32005` at HTTP
///    200. Since 113-18 all three refusals share the RETRYABLE `RATE_LIMITED`
///    code — the duplicate previously answered `-32600` at HTTP 400, the "do not
///    retry" class, for a condition that clears on its own — so the refusal
///    MESSAGE is the only discriminator (the `too many concurrent` substring is
///    load-bearing). The incumbent stream is untouched: the id belongs to the
///    caller, so the caller — not the server — resolves the collision by
///    choosing a free one
///    (see [`ListenRejection::code`](crate::server::subscriptions::ListenRejection)).
///
/// # The three closure triggers
///
/// A served stream closes on exactly one of:
/// * **client disconnect** — dropping the response drops the stream, drops the
///   moved-in `ListenGuard`, and RAII removes the registry entry and releases
///   both permits. No terminal result is sent: the peer is gone.
/// * **server shutdown** — [`Server::close_subscription_streams`](crate::server::Server::close_subscription_streams)
///   sends each stream its terminal [`SubscriptionsListenResult`](crate::types::subscriptions::SubscriptionsListenResult)
///   and then ends it. This is the ONLY trigger that sends a terminal result.
///   The result is pre-built HERE, at registration, because this is where the
///   shared v2 envelope helpers live.
/// * **buffer overflow** — a subscriber that fills its bounded channel is
///   disconnected after one terminal SSE comment (see `LISTEN_CHANNEL_CAPACITY`).
///
/// # Resumability
///
/// The stream never reads `Last-Event-ID` and never touches the event store: it
/// ASSERTS [`v1::resumability_active`] is already false for a v2 request (plan 08)
/// rather than re-deriving the rule.
async fn assemble_subscriptions_listen(
    state: &ServerState,
    id: crate::types::RequestId,
    params: Option<serde_json::Value>,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    v2_outbound: Option<(String, String)>,
    auth_context: Option<&crate::server::auth::AuthContext>,
) -> Response {
    use crate::server::subscriptions::{ListenFrame, ListenKey, LISTEN_CHANNEL_CAPACITY};
    use crate::types::protocol::error_codes::{AUTHENTICATION_REQUIRED, METHOD_NOT_FOUND};
    use crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD;

    let era = protocol_context.map(|pc| pc.era);
    if !matches!(era, Some(crate::types::protocol::Era::V2)) {
        return listen_rejection_response(
            era,
            id,
            METHOD_NOT_FOUND,
            format!("Method not found: {SUBSCRIPTIONS_LISTEN_METHOD}"),
        );
    }
    debug_assert!(
        !v1::resumability_active(state, era),
        "a v2 request already has resumability off (plan 08); the listen stream asserts that \
         rather than re-deriving it"
    );

    let view = listen_server_view(state).await;
    if !crate::types::subscriptions::advertises_subscriptions(&view.capabilities) {
        // The conformant-by-absence configuration (D-12 RESOLUTION): this server
        // advertises no subscription-delivered capability, so it has nothing to
        // serve here and the conformance suite records SKIPPED. The tripwire is
        // that `server/discover` publishes the SAME capabilities this predicate
        // just read.
        return listen_rejection_response(
            era,
            id,
            METHOD_NOT_FOUND,
            format!(
                "Method not found: {SUBSCRIPTIONS_LISTEN_METHOD} (this server advertises no \
                 subscription-delivered capability)"
            ),
        );
    }

    // AUTH PLUMBING (the ONE threading site — do not re-resolve elsewhere): the
    // POST pipeline already validated the request and produced this
    // `AuthContext` before dispatch, and it is passed straight in here. Both the
    // per-principal cap and the collision-free `ListenKey` key off its subject.
    //
    // FAIL-CLOSED (D-113-N): rejection case 3 above. `None` means "an auth
    // provider is configured and this caller presented nothing it accepted", the
    // same answer `resolve_mrtr_principal` gives the MRTR ingress on the same
    // server. `AUTHENTICATION_REQUIRED` is deliberately NOT in
    // `v2_status_for_code`'s 400 arm, so — exactly like the three `RATE_LIMITED`
    // listen refusals — it answers at HTTP 200 with a JSON-RPC error body.
    // Remapping -32003 to 401 would change the status of every other emitter of
    // that code across this transport, so `v2_status_for_code` stays untouched.
    let Some(principal) = resolve_listen_principal(auth_context, view.has_auth_provider) else {
        return listen_rejection_response(
            era,
            id,
            AUTHENTICATION_REQUIRED,
            format!(
                "{SUBSCRIPTIONS_LISTEN_METHOD} requires an authenticated caller on this server"
            ),
        );
    };

    let agreed = match resolve_agreed_filter(params, &view) {
        Ok(filter) => filter,
        Err((code, message)) => return listen_rejection_response(era, id, code, message),
    };

    let (sender, receiver) = mpsc::channel(LISTEN_CHANNEL_CAPACITY + 1);
    // The acknowledgement goes into the channel BEFORE the entry exists, so
    // nothing can possibly precede it — the spec MUST is structural here.
    if sender
        .try_send(ListenFrame::Message(listen_ack_frame(&agreed, &id)))
        .is_err()
    {
        return listen_rejection_response(
            era,
            id,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            "failed to queue the subscription acknowledgement".to_string(),
        );
    }

    let terminal = listen_terminal_result_frame(&id, protocol_context, &view.info);
    let registry = view.registry;
    let key = ListenKey {
        principal,
        request_id: id.clone(),
    };
    let guard = match registry.register(key, agreed, sender, terminal) {
        Ok(guard) => guard,
        Err(rejection) => {
            // The code is OWNED by the rejection itself rather than chosen
            // here, so this route can never disagree with
            // `ListenRejection::code`'s exhaustive table. As of 113-18 that
            // table answers all three refusals with the RETRYABLE
            // `RATE_LIMITED`; the discriminator is the MESSAGE, not the code.
            return listen_rejection_response(
                era,
                id,
                rejection.code(),
                rejection.message().to_string(),
            );
        },
    };

    // The guard is part of the stream's STATE, so a dropped SSE response drops
    // it and RAII reclaims the registry entry and both permits — there is no
    // unregister call anywhere that could be forgotten (T-113-63).
    let frames =
        futures_util::stream::unfold((receiver, guard), |(mut receiver, guard)| async move {
            receiver
                .recv()
                .await
                .map(|frame| (frame, (receiver, guard)))
        });

    let events = frames.map(|frame| Ok::<_, Infallible>(listen_sse_event(frame)));
    let mut response = Sse::new(events)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(LISTEN_KEEP_ALIVE_INTERVAL))
        .into_response();
    attach_listen_response_headers(&mut response, v2_outbound.as_ref());
    response
}

/// The fast path's legacy protocol-version guard.
///
/// Condition: `!is_init_request && !is_v2_request`, calling the PLAIN
/// [`validate_protocol_version`]. Legacy validation applies to v1 non-init
/// requests ONLY — an accepted v2 request is validated by the v2 gate that ran
/// before this (D-11 left v1 untouched). A v1 / non-opted-in `server/discover`
/// also flows through here, with no bypass.
///
/// # The asymmetry with its twin is DELIBERATE (D-08) — but it is not a
/// # difference in the PREDICATE
///
/// Stated precisely, because the imprecise version misleads:
/// [`guard_legacy_version_with_middleware`] spells its condition
/// `!is_v2_request` and passes `is_init_request` INTO
/// [`validate_protocol_version_with_error_hook`], which opens with
/// `if is_init_request { return Ok(()); }`. So **both guards evaluate the same
/// effective predicate, `!is_init_request && !is_v2_request`** — they just
/// spell it in different places.
///
/// What is genuinely asymmetric, and why there are two helpers:
///
/// 1. This path calls the PLAIN [`validate_protocol_version`], which has **no**
///    init handling of its own — so dropping `!is_init_request` from the
///    condition here WOULD change behavior. It cannot be "harmonised" toward
///    the middleware spelling.
/// 2. The middleware path additionally fires `report_middleware_error` on
///    failure, which is `async`. That is this file's standard
///    plain-fn + `*_with_error_hook` split, not a semantic divergence.
///
/// Extracted in plan 113.1-05 (D-08, D-10); wording corrected after review.
fn guard_legacy_version_fast(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    is_v2_request: bool,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if !is_init_request && !is_v2_request {
        validate_protocol_version(state, era, session_id, protocol_version)?;
    }
    Ok(())
}

/// Everything the fast path's read-and-classify preamble produces.
///
/// SIX fields — a different bundle at a different pipeline stage from
/// [`FastPathDispatch`], which carries five. Do not conflate them.
struct FastIngress {
    /// The request headers, consumed by the v2 gate, session resolution and auth.
    headers: HeaderMap,
    /// The read-and-capped body, still needed as raw bytes by the v2 gate.
    body: String,
    /// The classified ingress (public request, discover, or subscriptions listen).
    ingress: HttpIngress,
    /// `Mcp-Session-Id`, from [`extract_session_and_protocol_headers`].
    session_id: Option<String>,
    /// `MCP-Protocol-Version`, from the same call.
    protocol_version: Option<String>,
    /// Whether this is an `initialize` request — decides session minting.
    is_init_request: bool,
}

/// Read, validate, parse and classify a fast-path POST request.
///
/// The first stage of the pipeline: body read under the configured cap, header
/// validation, transport-message parse, and ingress classification. Every
/// failure is already a `Response`, including the v2 raw-level id recovery on a
/// parse error (an unknown v2 method must answer 404 + -32601 with the ORIGINAL
/// id even though its body never produced a typed request).
///
/// Extracted in plan 113.1-05 (D-10): this is a per-path helper by design, NOT
/// shared with the middleware twin — a shared preamble is the pipeline
/// unification D-06 rejects, and the two genuinely differ (the middleware path
/// runs conversion, context-building and the request-middleware chain first).
async fn read_and_classify_fast(
    state: &ServerState,
    request: axum::extract::Request<Body>,
) -> std::result::Result<FastIngress, Response> {
    let (parts, body) = request.into_parts();
    let headers = parts.headers;

    let body = read_body_with_limit(body, state.config.max_request_bytes).await?;

    validate_headers(&headers, "POST")?;

    let ingress = match parse_transport_message_fast(body.as_bytes()) {
        Ok(i) => i,
        // A v2 unknown method must be 404 + -32601 with the ORIGINAL id, even
        // though its body never produced a typed request (raw-level mapping).
        Err(response) => {
            return Err(map_unparsed_body_for_v2(state, body.as_bytes(), response).await)
        },
    };

    let (session_id, protocol_version) = extract_session_and_protocol_headers(&headers);
    let is_init_request = ingress.is_initialize();

    Ok(FastIngress {
        headers,
        body,
        ingress,
        session_id,
        protocol_version,
        is_init_request,
    })
}

/// Fast path handler without HTTP middleware.
///
/// Refactored in 75-01 Task 1a-A: extracted [`read_body_with_limit`],
/// [`parse_transport_message_fast`], and [`handle_fast_path_request`] so
/// this orchestrator is a thin early-return pipeline, sharing
/// [`extract_session_and_protocol_headers`], [`is_initialize_request`],
/// [`v1::resolve_session_for_request`], and [`compute_outbound_protocol_version`]
/// with the middleware path.
///
/// # The pipeline, in order (plans 113.1-01 and 113.1-05)
///
/// 1. [`read_and_classify_fast`] — body read under cap, header validation,
///    parse, ingress classification (113.1-05)
/// 2. [`resolve_v2_gate`] — the v2 required-header gate (113.1-01). **Runs
///    BEFORE session resolution and BEFORE the legacy version check**; see its
///    own rustdoc for why that ordering is load-bearing
/// 3. [`v1::resolve_session_for_request`] — session minting / validation
/// 4. [`guard_legacy_version_fast`] — the v1 protocol-version guard (113.1-05),
///    asymmetric with its middleware twin BY DESIGN (D-08)
/// 5. [`extract_and_validate_auth`] — authentication
/// 6. [`dispatch_message_fast`] — the 4-arm ingress dispatch (113.1-01), which
///    every arm reaches only downstream of step 5
///
/// **Complexity budget: cognitive 4** here plus **0** in
/// [`handle_post_fast_path_inner`] (pmat 3.15.0), down from 30 before phase
/// 113.1, against a hard gate of 25 and this phase's stricter target of 20.
/// The inner fn is a branch-free `?` pipeline, so pmat scores it 0 and does not
/// list it at all — it reports no cognitive-0 function, which is why a
/// per-function sweep appears to skip it.
/// Recorded so a later phase adding to this handler can see what it is spending.
async fn handle_post_fast_path(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> Response {
    // Every stage returns `Result<_, Response>`, so the pipeline is written with
    // `?` in an inner fn and both arms collapse to the same value here. The
    // alternative — a four-line `match { Ok(v) => v, Err(r) => return r }` per
    // stage — is the same control flow spelled out five times.
    match handle_post_fast_path_inner(state, request).await {
        Ok(response) | Err(response) => response,
    }
}

/// The fast-path pipeline proper. See [`handle_post_fast_path`] for the stage
/// list and the complexity budget.
async fn handle_post_fast_path_inner(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> std::result::Result<Response, Response> {
    let FastIngress {
        headers,
        body,
        ingress,
        session_id,
        protocol_version,
        is_init_request,
    } = read_and_classify_fast(&state, request).await?;

    // THE DEADLOCK FIX (T-118.1-10-01), and deliberately the SECOND statement of
    // this function. An inbound JSON-RPC RESPONSE is correlated and answered HERE,
    // before the three sites that take `state.server.lock()` — `run_v2_header_gate`
    // twice (the accept-list read and the negotiation read) and
    // `extract_and_validate_auth` once. A tool handler parked on a peer call holds
    // that same mutex for its whole duration, so an answer routed after any of
    // them could never arrive: one tool call would take the transport offline.
    //
    // ONLY responses bypass. A response carries no authority — it invokes no
    // method and can only resolve a correlation the SERVER minted — whereas
    // requests and notifications keep going through the full gate and auth
    // pipeline below, unchanged. Widening this would be an auth bypass.
    //
    // ONE shared function with the middleware twin, in the spirit of
    // `dispatch_request_or_retire`: the rule cannot drift between the two paths.
    if let Some(accepted) =
        peer_channel::try_route_inbound_response(&state, &ingress, session_id.as_deref()).await
    {
        return Ok(accepted);
    }

    // v2 required-header gate (VERS-05). The ordering constraints this call
    // carries — gate BEFORE session resolution and BEFORE the legacy
    // protocol-version check — are documented on `resolve_v2_gate` itself.
    //
    // `post_body` is THE request body for the rest of this pipeline: the gate and
    // the log-level rule below share its single lazy parse (see `PostBody`).
    let post_body = PostBody::new(body.as_bytes());
    let (protocol_context, v2_outbound) =
        resolve_v2_gate(&state, &headers, &post_body, &ingress).await?;
    let is_v2_request = v2_outbound.is_some();
    let era = protocol_context.as_ref().map(|pc| pc.era);
    let sessions_on = v1::sessions_active(&state, era);

    let response_session_id = v1::resolve_session_for_request(
        &state,
        era,
        is_init_request,
        session_id.clone(),
        protocol_version.clone(),
    )?;

    guard_legacy_version_fast(
        &state,
        era,
        is_init_request,
        is_v2_request,
        session_id.as_ref(),
        protocol_version.as_ref(),
    )?;

    let auth_context = extract_and_validate_auth(&state, &headers).await?;

    // The transport-side half of the server-to-client channel (CONF-07 / G-3):
    // bind a session-scoped peer + notification sink onto the ALREADY-RESOLVED
    // context, at the one site that knows which session this request arrived on.
    // A no-op when there is no live session or the era suppresses them, and
    // placed AFTER every era-gated stage so `era`, `sessions_on` and the legacy
    // version guard all read exactly what they read before.
    let protocol_context = peer_channel::attach_session_backchannel(
        &state,
        protocol_context,
        peer_channel::BackchannelSite {
            session_id: response_session_id.as_deref(),
            sessions_on,
            is_init_request,
        },
    )
    .await;

    // The v2 twin of the attachment above (CONF-07 / D-16), at the SAME point in
    // the pipeline and for the same reason: this is the one site that knows both
    // the resolved era and the client's `Accept`. Inert on v1 — that era has a
    // session stream and plan 11 already routes to it.
    let (protocol_context, progress_queue) = attach_v2_progress_sink(
        protocol_context,
        v2_multi_frame_eligible(
            &state,
            era,
            headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()),
        ),
    );

    // This request's minimum log level (CONF-10 / D-10 / D-11 / D-12), captured
    // and resolved by the one named rule and written onto the context — which is
    // the LAST point at which the context is still owned and mutable, and is
    // after every era-gated stage, so `era` and `sessions_on` read exactly what
    // they read above. `server::core::attach_request_log_sink` lifts it from
    // there onto the request's `RequestHandlerExtra` at both dispatch roots; this
    // file never constructs one and must not pretend to.
    //
    // The middleware twin carries this same block at the same position. BOTH or
    // NEITHER: a level honoured on one ingress path and ignored on the other is
    // indistinguishable from no level at all for whichever deployment shape takes
    // the other path (T-118.2-07-06).
    let request_log_level = resolve_request_log_level(
        &state,
        era,
        sessions_on,
        response_session_id.as_deref(),
        &post_body,
    );
    let protocol_context = match (protocol_context, request_log_level) {
        (Some(context), Some(level)) => Some(context.with_resolved_log_level(level)),
        (context, _) => context,
    };

    Ok(dispatch_message_fast(
        &state,
        ingress,
        FastPathDispatch {
            is_init_request,
            response_session_id,
            asserted_protocol_version: protocol_version,
            protocol_context,
            v2_outbound,
            sessions_on,
            progress_queue,
        },
        auth_context,
        session_id.as_ref(),
    )
    .await)
}

/// Build the HTTP middleware context from a middleware-adapted request.
fn build_middleware_context(
    server_request: &crate::server::http_middleware::ServerHttpRequest,
) -> ServerHttpContext {
    let session_id = server_request
        .get_header(MCP_SESSION_ID)
        .map(str::to_string);
    let request_id = server_request
        .get_header("x-request-id")
        .map_or_else(|| Uuid::new_v4().to_string(), str::to_string);
    ServerHttpContext {
        request_id,
        start_time: std::time::Instant::now(),
        session_id,
    }
}

/// Convert the axum request into a middleware `ServerHttpRequest`, handling
/// the body-size-limit failure path.
async fn convert_axum_to_middleware_request(
    request: axum::extract::Request<Body>,
    max_request_bytes: usize,
) -> std::result::Result<crate::server::http_middleware::ServerHttpRequest, Response> {
    let (parts, body) = request.into_parts();
    from_axum_with_limit(parts, body, max_request_bytes)
        .await
        .map_err(|e| {
            create_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                crate::types::protocol::error_codes::INVALID_REQUEST,
                &format!("Request body exceeds limit: {}", e),
            )
        })
}

/// Resolve the session ID and run the middleware error hook on failure.
///
/// Wraps [`v1::resolve_session_for_request`] so the caller doesn't have to
/// branch on `is_init_request` for the error-kind string.
async fn resolve_session_with_error_hook(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<Option<String>, Response> {
    match v1::resolve_session_for_request(state, era, is_init_request, session_id, protocol_version)
    {
        Ok(sid) => Ok(sid),
        Err(error_response) => {
            let kind = if is_init_request {
                "Session initialization failed"
            } else {
                "Session validation failed"
            };
            report_middleware_error(http_middleware, http_context, kind).await;
            Err(error_response)
        },
    }
}

/// Run protocol-version validation for non-init requests, wiring the middleware
/// error hook on failure. A no-op for init requests.
async fn validate_protocol_version_with_error_hook(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if is_init_request {
        return Ok(());
    }
    if let Err(error_response) = validate_protocol_version(state, era, session_id, protocol_version)
    {
        report_middleware_error(
            http_middleware,
            http_context,
            "Protocol version validation failed",
        )
        .await;
        return Err(error_response);
    }
    Ok(())
}

/// Run the v2 required-header gate and fire the middleware error hook on a
/// gate rejection.
///
/// Wraps [`resolve_v2_gate`] so the middleware path does not have to repeat the
/// gate's three-arm classification just to add one hook call. The ordering
/// constraints documented on [`resolve_v2_gate`] apply identically here.
async fn resolve_v2_gate_with_error_hook(
    state: &ServerState,
    headers: &HeaderMap,
    body: &PostBody<'_>,
    ingress: &HttpIngress,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<V2GateResolved, Response> {
    match resolve_v2_gate(state, headers, body, ingress).await {
        Ok(resolved) => Ok(resolved),
        Err(error_response) => {
            report_middleware_error(http_middleware, http_context, "v2 header gate rejected").await;
            Err(error_response)
        },
    }
}

/// Per-request dispatch inputs threaded into the middleware-path handler.
///
/// The middleware-path twin of [`FastPathDispatch`]: carries the Plan-04-resolved
/// `ProtocolContext` (CONSUMED at dispatch, never re-resolved) and the optional
/// v2 outbound headers to echo on success AND error.
struct MiddlewareDispatch {
    is_init_request: bool,
    response_session_id: Option<String>,
    /// The `MCP-Protocol-Version` the client asserted — see the field of the
    /// same name on [`InternalResponseShape`].
    asserted_protocol_version: Option<String>,
    protocol_context: Option<crate::types::protocol::ProtocolContext>,
    v2_outbound: Option<(String, String)>,
    /// [`v1::sessions_active`] for THIS request — gates the `Mcp-Session-Id`
    /// response header (HTTP-01).
    sessions_on: bool,
    /// This request's bounded v2 progress queue — see the fast-path twin.
    progress_queue: Option<V2ProgressQueue>,
}

/// Assemble the `server/discover` response on the middleware path (VERS-04).
///
/// The middleware-path twin of [`assemble_discover_response_fast`]: projects via
/// [`Server::handle_discover`](crate::server::Server::handle_discover), stores the
/// response event, runs the SAME response-middleware assembly every other
/// response runs ([`build_success_response_with_middleware`]), and echoes the v2
/// outbound headers on an accepted v2 discover — preserving the original id.
/// Reached only AFTER session, the v2 matrix, legacy-version validation, and auth
/// (no bypass). See [`assemble_discover_response_fast`] for the D-10 `-32601@200`
/// decision on v1 / non-opted-in discover.
async fn assemble_discover_response_with_middleware(
    state: &ServerState,
    id: crate::types::RequestId,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    shape: InternalResponseShape<'_>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    let InternalResponseShape {
        response_session_id,
        asserted_protocol_version,
        v2_outbound,
        sessions_on,
    } = shape;
    let live_id = id.clone();
    let json_response = {
        let server = state.server.lock().await;
        server.handle_discover(id, protocol_context)
    };
    let era = protocol_context.map(|pc| pc.era);
    let v2_status = v2_dispatch_response_status(era, &json_response);
    // Same structural guarantee as every other direct response (HTTP-05).
    let response_msg =
        TransportMessage::Response(envelope_for_live_request(json_response.payload, live_id));

    v1::store_response_event(state, era, response_session_id, &response_msg).await;

    // Discover is never an init request → compute the outbound version normally.
    let version_to_send =
        outbound_protocol_version_after_init(state, response_session_id, asserted_protocol_version);

    let mut response = build_success_response_with_middleware(
        &response_msg,
        response_session_id,
        &version_to_send,
        sessions_on,
        http_middleware,
        http_context,
    )
    .await;

    if let Some((method, name)) = &v2_outbound {
        apply_v2_outbound_headers(response.headers_mut(), method, name);
    }
    if let Some(status) = v2_status {
        *response.status_mut() = status;
    }
    response
}

/// Dispatch the classified ingress on the fast path.
///
/// Handles a public `Request` (server-handled + response assembly), a
/// `server/discover` ingress (the VERS-04 per-path assembly), a
/// `subscriptions/listen` ingress (the HTTP-04 held-open stream), and
/// `Notification` / `Response` (202 Accepted) in separate arms.
///
/// # Calling contract — auth has ALREADY succeeded
///
/// This helper assumes authentication is done: its `auth_context` parameter is
/// the value [`extract_and_validate_auth`] returned, and EVERY arm — including
/// `SubscriptionsListen` — therefore runs downstream of it. The access-control
/// property is that the caller invokes this only after that call returned `Ok`;
/// it is not the textual order of the match arms below, which are mutually
/// exclusive `HttpIngress` variants.
///
/// Extracted in plan 113.1-01 (D-06): the fast path held this match inline
/// while the middleware path already delegated to
/// [`dispatch_message_with_middleware`]. The twins now sit adjacent.
async fn dispatch_message_fast(
    state: &ServerState,
    ingress: HttpIngress,
    dispatch: FastPathDispatch,
    auth_context: Option<crate::server::auth::AuthContext>,
    session_id: Option<&String>,
) -> Response {
    match ingress {
        HttpIngress::Public(TransportMessage::Request { id, request }) => {
            // `dispatch` is forwarded whole: this arm needs every field, so
            // unpacking it here only to rebuild an identical struct would be an
            // identity round-trip of a ~1 KiB value. The arms below destructure
            // with `..` because they need only parts.
            //
            // `Box::pin`: the dispatch future crosses clippy's large_future
            // threshold once the v2 status mapping is threaded through it —
            // boxing keeps the handler future small without changing behavior
            // (same treatment the two POST entrypoints already get).
            Box::pin(handle_fast_path_request(
                state,
                id,
                request,
                auth_context,
                dispatch,
                session_id,
            ))
            .await
        },
        // Per-path response assembly (finding #3/#4): reached AFTER session, the v2
        // matrix, legacy-version validation, and auth — never an early return.
        HttpIngress::Discover { id, .. } => {
            let FastPathDispatch {
                response_session_id,
                asserted_protocol_version,
                protocol_context,
                v2_outbound,
                sessions_on,
                ..
            } = dispatch;
            assemble_discover_response_fast(
                state,
                id,
                protocol_context.as_ref(),
                InternalResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    asserted_protocol_version: asserted_protocol_version.as_deref(),
                    v2_outbound,
                    sessions_on,
                },
                session_id,
            )
            .await
        },
        // HTTP-04: the capability-gated listen route. Reached AFTER the same
        // session / v2-matrix / legacy-version / auth pipeline as every other
        // ingress — a held-open stream must not be a way around auth.
        HttpIngress::SubscriptionsListen { id, params } => {
            let FastPathDispatch {
                protocol_context,
                v2_outbound,
                ..
            } = dispatch;
            Box::pin(assemble_subscriptions_listen(
                state,
                id,
                params,
                protocol_context.as_ref(),
                v2_outbound,
                auth_context.as_ref(),
            ))
            .await
        },
        // TASK-02: the v2 task-input delivery route. Like every other arm here it
        // is reached AFTER the session / v2-matrix / legacy-version / auth
        // pipeline, and it carries `auth_context` into the router because the
        // `-32003` refusal is one of the router's five ordered gates.
        HttpIngress::TasksUpdate { id, params } => {
            let FastPathDispatch {
                response_session_id,
                asserted_protocol_version,
                protocol_context,
                v2_outbound,
                sessions_on,
                ..
            } = dispatch;
            Box::pin(assemble_tasks_update_fast(
                state,
                TasksUpdateCall {
                    id,
                    params,
                    protocol_context: protocol_context.as_ref(),
                    auth_context: auth_context.as_ref(),
                },
                InternalResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    asserted_protocol_version: asserted_protocol_version.as_deref(),
                    v2_outbound,
                    sessions_on,
                },
                session_id,
            ))
            .await
        },
        HttpIngress::Public(TransportMessage::Notification { .. }) => {
            StatusCode::ACCEPTED.into_response()
        },
        // UNREACHABLE BY CONSTRUCTION: `peer_channel::try_route_inbound_response`
        // matched and answered every response envelope as the second statement of
        // `handle_post_fast_path_inner`, before this pipeline began. The arm
        // survives only because the match must stay exhaustive — and it now ROUTES
        // through the identical correlation path rather than discarding, so a
        // response that ever reached here by some future route could not silently
        // lose a client's answer.
        HttpIngress::Public(TransportMessage::Response(ref response)) => {
            peer_channel::route_inbound_response(state, response, session_id.map(String::as_str))
                .await
        },
    }
}

/// Dispatch the classified ingress on the middleware path.
///
/// Handles a public `Request` (server-handled + response assembly), a
/// `server/discover` ingress (the VERS-04 per-path assembly), `Notification`
/// (202 Accepted), and `Response` (202 Accepted) in separate arms.
async fn dispatch_message_with_middleware(
    state: &ServerState,
    ingress: HttpIngress,
    dispatch: MiddlewareDispatch,
    auth_context: Option<crate::server::auth::AuthContext>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> Response {
    let MiddlewareDispatch {
        is_init_request,
        response_session_id,
        asserted_protocol_version,
        protocol_context,
        v2_outbound,
        sessions_on,
        progress_queue,
    } = dispatch;
    match ingress {
        HttpIngress::Discover { id, .. } => {
            assemble_discover_response_with_middleware(
                state,
                id,
                protocol_context.as_ref(),
                InternalResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    asserted_protocol_version: asserted_protocol_version.as_deref(),
                    v2_outbound,
                    sessions_on,
                },
                http_middleware,
                http_context,
            )
            .await
        },
        // HTTP-04: the capability-gated listen route (see the fast-path twin).
        HttpIngress::SubscriptionsListen { id, params } => {
            assemble_subscriptions_listen(
                state,
                id,
                params,
                protocol_context.as_ref(),
                v2_outbound,
                auth_context.as_ref(),
            )
            .await
        },
        // TASK-02: the v2 task-input delivery route (see the fast-path twin).
        HttpIngress::TasksUpdate { id, params } => {
            assemble_tasks_update_with_middleware(
                state,
                TasksUpdateCall {
                    id,
                    params,
                    protocol_context: protocol_context.as_ref(),
                    auth_context: auth_context.as_ref(),
                },
                InternalResponseShape {
                    response_session_id: response_session_id.as_ref(),
                    asserted_protocol_version: asserted_protocol_version.as_deref(),
                    v2_outbound,
                    sessions_on,
                },
                http_middleware,
                http_context,
            )
            .await
        },
        HttpIngress::Public(TransportMessage::Request { id, request }) => {
            let era = protocol_context.as_ref().map(|pc| pc.era);
            // Captured BEFORE dispatch consumes it (see the fast-path twin).
            let live_id = id.clone();
            // Thread the ALREADY-RESOLVED ProtocolContext into dispatch (Plan 06
            // / D-11): never re-resolved downstream. Every method the 2026-07-28
            // schema retired was already refused by the v2 ingress gate.
            let json_response =
                dispatch_public_request(state, id, request, auth_context, protocol_context).await;
            // Code-driven v2 status (see the fast-path twin).
            let v2_status = v2_dispatch_response_status(era, &json_response);
            // Same structural guarantee as every other direct response (HTTP-05).
            let response_msg = TransportMessage::Response(envelope_for_live_request(
                json_response.payload,
                live_id,
            ));

            let negotiated_version = if is_init_request {
                let version = extract_negotiated_version(&response_msg);
                v1::update_session_after_init(state, response_session_id.as_ref(), version.clone());
                version
            } else {
                None
            };

            v1::store_response_event(state, era, response_session_id.as_ref(), &response_msg).await;

            let version_to_send = compute_outbound_protocol_version(
                state,
                response_session_id.as_ref(),
                is_init_request,
                negotiated_version.as_deref(),
                asserted_protocol_version.as_deref(),
            );

            // CONF-07 / D-16, the middleware twin. The multi-frame body is a
            // COMPLETE byte buffer by the time it is built (the handler has
            // already run), so it goes through the SAME
            // `finish_with_middleware` seam the JSON twin uses — no streaming
            // contract is broken, and an operator's response middleware is not
            // silently skipped for exactly the responses that carry progress.
            let mut response = match take_v2_progress_frames(progress_queue, response_msg) {
                Ok((progress, result)) => {
                    // The session/version headers the JSON twin sets INSIDE
                    // `build_success_response_with_middleware` are applied
                    // here instead, so the two branches emit the same header
                    // set and only the body framing differs.
                    let mut headers = HeaderMap::new();
                    apply_v2_multi_frame_headers(&mut headers);
                    v1::apply_session_header(
                        &mut headers,
                        response_session_id.as_ref(),
                        sessions_on,
                    );
                    headers.insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());
                    finish_with_middleware(
                        headers,
                        render_v2_multi_frame_body(progress, result).into_bytes(),
                        http_middleware,
                        http_context,
                    )
                    .await
                },
                Err(response_msg) => {
                    build_success_response_with_middleware(
                        &response_msg,
                        response_session_id.as_ref(),
                        &version_to_send,
                        sessions_on,
                        http_middleware,
                        http_context,
                    )
                    .await
                },
            };

            // v2 outbound headers on BOTH success and structured error (VERS-05).
            if let Some((method, name)) = &v2_outbound {
                apply_v2_outbound_headers(response.headers_mut(), method, name);
            }
            if let Some(status) = v2_status {
                *response.status_mut() = status;
            }
            response
        },
        HttpIngress::Public(TransportMessage::Notification { .. }) => {
            StatusCode::ACCEPTED.into_response()
        },
        // UNREACHABLE BY CONSTRUCTION — see the fast-path twin. Routes rather
        // than discards, for the same reason.
        HttpIngress::Public(TransportMessage::Response(ref response)) => {
            peer_channel::route_inbound_response(state, response, response_session_id.as_deref())
                .await
        },
    }
}

/// The middleware path's legacy protocol-version guard.
///
/// Condition: `!is_v2_request` **ONLY**, passing `is_init_request` INTO
/// [`validate_protocol_version_with_error_hook`] rather than testing it here.
/// That wrapper's own rustdoc reads "A no-op for init requests" — the init check
/// is folded inside it BY DESIGN, and it also fires `report_middleware_error` on
/// failure, which the plain fast-path call cannot do.
///
/// # The asymmetry with its twin is DELIBERATE (D-08) — but it is not a
/// # difference in the PREDICATE
///
/// [`guard_legacy_version_fast`] spells its condition
/// `!is_init_request && !is_v2_request` and calls the PLAIN
/// [`validate_protocol_version`]. Because the wrapper this one calls already
/// returns early on `is_init_request`, **both guards evaluate the same
/// effective predicate** — the init test simply lives one layer deeper here.
///
/// The real reasons there are two helpers: the fast path's callee has no init
/// handling (so its condition must state it), and this path needs the `async`
/// `report_middleware_error` hook. That is the file's standard
/// plain-fn + `*_with_error_hook` split.
///
/// Extracted in plan 113.1-05 (D-08, D-10); wording corrected after review.
#[allow(clippy::too_many_arguments)]
async fn guard_legacy_version_with_middleware(
    state: &ServerState,
    era: Option<crate::types::protocol::Era>,
    is_init_request: bool,
    is_v2_request: bool,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
    http_middleware: &ServerHttpMiddlewareChain,
    http_context: &ServerHttpContext,
) -> std::result::Result<(), Response> {
    if !is_v2_request {
        validate_protocol_version_with_error_hook(
            state,
            era,
            is_init_request,
            session_id,
            protocol_version,
            http_middleware,
            http_context,
        )
        .await?;
    }
    Ok(())
}

/// Everything the middleware path's read-and-classify preamble produces.
///
/// SIX fields, not eight: the middleware path's headers and body stay reachable
/// through `server_request` rather than being lifted into separate bindings, so
/// carrying them again would duplicate state the handler already reads from
/// there. A different bundle at a different pipeline stage from
/// [`MiddlewareDispatch`], which carries five.
struct MwIngress {
    /// The converted middleware request — this path's headers/body carrier.
    server_request: crate::server::http_middleware::ServerHttpRequest,
    /// Built by [`build_middleware_context`]; threaded into every later hook.
    http_context: ServerHttpContext,
    /// The classified ingress (public request, discover, or subscriptions listen).
    ingress: HttpIngress,
    /// `Mcp-Session-Id`, from [`extract_session_and_protocol_headers`].
    session_id: Option<String>,
    /// `MCP-Protocol-Version`, from the same call.
    protocol_version: Option<String>,
    /// Whether this is an `initialize` request — decides session minting.
    is_init_request: bool,
}

/// Convert, run request middleware, validate, parse and classify a
/// middleware-path POST request.
///
/// The middleware twin of [`read_and_classify_fast`], and deliberately a
/// SEPARATE function rather than a shared preamble (D-06 rejects pipeline
/// unification). The divergence is real: this path converts the axum request,
/// builds the middleware context, runs the request-middleware chain, and hooks
/// `report_middleware_error` on header-validation failure — none of which the
/// fast path has.
///
/// Two orderings inside are load-bearing and must not be rearranged:
/// [`build_middleware_context`] runs BEFORE [`run_request_middleware`] (the
/// chain receives the context), and `report_middleware_error` runs AFTER the
/// [`validate_headers`] call it reports on.
///
/// Extracted in plan 113.1-05 (D-10).
async fn read_and_classify_with_middleware(
    state: &ServerState,
    request: axum::extract::Request<Body>,
    http_middleware: &ServerHttpMiddlewareChain,
) -> std::result::Result<MwIngress, Response> {
    let mut server_request =
        convert_axum_to_middleware_request(request, state.config.max_request_bytes).await?;

    let http_context = build_middleware_context(&server_request);

    run_request_middleware(http_middleware, &mut server_request, &http_context).await?;

    if let Err(error_response) = validate_headers(&server_request.headers, "POST") {
        report_middleware_error(http_middleware, &http_context, "Header validation failed").await;
        return Err(error_response);
    }

    let ingress = match parse_transport_message_with_middleware(
        &server_request.body,
        http_middleware,
        &http_context,
    )
    .await
    {
        Ok(i) => i,
        // A v2 unknown method must be 404 + -32601 with the ORIGINAL id, even
        // though its body never produced a typed request (raw-level mapping).
        Err(response) => {
            return Err(map_unparsed_body_for_v2(state, &server_request.body, response).await)
        },
    };

    let (session_id, protocol_version) =
        extract_session_and_protocol_headers(&server_request.headers);
    let is_init_request = ingress.is_initialize();

    Ok(MwIngress {
        server_request,
        http_context,
        ingress,
        session_id,
        protocol_version,
        is_init_request,
    })
}

/// Handler with HTTP middleware integration.
///
/// Refactored in 75-01 Task 1a-A: extracted
/// [`convert_axum_to_middleware_request`], [`build_middleware_context`],
/// [`run_request_middleware`], [`parse_transport_message_with_middleware`],
/// [`v1::resolve_session_for_request`], [`extract_auth_with_middleware`], and
/// [`dispatch_message_with_middleware`] so this orchestrator is a thin
/// early-return pipeline.
///
/// # The pipeline, in order (plans 113.1-01 and 113.1-05)
///
/// 1. [`read_and_classify_with_middleware`] — conversion, context build,
///    request-middleware chain, header validation, parse, classification
///    (113.1-05)
/// 2. [`resolve_v2_gate_with_error_hook`] — the v2 required-header gate plus the
///    middleware error hook (113.1-01). **Runs BEFORE session resolution and
///    BEFORE the legacy version check**; see [`resolve_v2_gate`]'s rustdoc for
///    why that ordering is load-bearing
/// 3. [`resolve_session_with_error_hook`] — session minting / validation
/// 4. [`guard_legacy_version_with_middleware`] — the v1 protocol-version guard
///    (113.1-05), asymmetric with its fast-path twin BY DESIGN (D-08)
/// 5. [`extract_auth_with_middleware`] — authentication
/// 6. [`dispatch_message_with_middleware`] — the ingress dispatch
///
/// **Complexity budget: cognitive 4** here plus **0** in
/// [`handle_post_with_middleware_inner`] (pmat 3.15.0), down from 31 before
/// phase 113.1, against a hard gate of 25 and this phase's stricter target of
/// 20. The inner fn is a branch-free `?` pipeline, so pmat scores it 0 and does
/// not list it at all — see [`handle_post_fast_path`] for the same note.
/// Recorded so a later phase adding to this handler can see what it is spending.
async fn handle_post_with_middleware(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> Response {
    // See [`handle_post_fast_path`] for why the pipeline lives in an inner fn.
    match handle_post_with_middleware_inner(state, request).await {
        Ok(response) | Err(response) => response,
    }
}

/// The middleware-path pipeline proper. See [`handle_post_with_middleware`] for
/// the stage list and the complexity budget.
async fn handle_post_with_middleware_inner(
    state: ServerState,
    request: axum::extract::Request<Body>,
) -> std::result::Result<Response, Response> {
    let http_middleware = state
        .config
        .http_middleware
        .as_ref()
        .expect("Middleware chain must exist");

    let MwIngress {
        server_request,
        http_context,
        ingress,
        session_id,
        protocol_version,
        is_init_request,
    } = read_and_classify_with_middleware(&state, request, http_middleware).await?;

    // The SAME deadlock fix as the fast-path twin, from the SAME position in the
    // pipeline and through the SAME shared function — see that call site for the
    // three bypassed lock sites and for why only responses may bypass them.
    if let Some(accepted) =
        peer_channel::try_route_inbound_response(&state, &ingress, session_id.as_deref()).await
    {
        return Ok(accepted);
    }

    // v2 required-header gate (VERS-05). The ordering constraints this call
    // carries — gate BEFORE session resolution and BEFORE the legacy
    // protocol-version check — are documented on `resolve_v2_gate` itself.
    // The fast-path twin's shared body — see that call site for why one lazy
    // parse is threaded through both the gate and the log-level rule.
    let post_body = PostBody::new(&server_request.body);
    let (protocol_context, v2_outbound) = resolve_v2_gate_with_error_hook(
        &state,
        &server_request.headers,
        &post_body,
        &ingress,
        http_middleware,
        &http_context,
    )
    .await?;
    let is_v2_request = v2_outbound.is_some();
    let era = protocol_context.as_ref().map(|pc| pc.era);
    let sessions_on = v1::sessions_active(&state, era);

    let response_session_id = resolve_session_with_error_hook(
        &state,
        era,
        is_init_request,
        session_id.clone(),
        protocol_version.clone(),
        http_middleware,
        &http_context,
    )
    .await?;

    guard_legacy_version_with_middleware(
        &state,
        era,
        is_init_request,
        is_v2_request,
        session_id.as_ref(),
        protocol_version.as_ref(),
        http_middleware,
        &http_context,
    )
    .await?;

    let auth_context =
        extract_auth_with_middleware(&state, &server_request, http_middleware, &http_context)
            .await?;

    // The SAME transport-side attachment as the fast-path twin, from the same
    // position in the pipeline — see that call site for why it sits here.
    let protocol_context = peer_channel::attach_session_backchannel(
        &state,
        protocol_context,
        peer_channel::BackchannelSite {
            session_id: response_session_id.as_deref(),
            sessions_on,
            is_init_request,
        },
    )
    .await;

    // The v2 progress queue (CONF-07 / D-16) — see the fast-path twin.
    let (protocol_context, progress_queue) = attach_v2_progress_sink(
        protocol_context,
        v2_multi_frame_eligible(
            &state,
            era,
            server_request.get_header(header::ACCEPT.as_str()),
        ),
    );

    // The SAME log-level capture and resolution as the fast-path twin, from the
    // same position in the pipeline and through the SAME named rule — see that
    // call site for why it sits here and why a level resolved on only ONE of the
    // two ingress paths would be a deployment-shape-dependent defect that no
    // single-path test can see (T-118.2-07-06).
    let request_log_level = resolve_request_log_level(
        &state,
        era,
        sessions_on,
        response_session_id.as_deref(),
        &post_body,
    );
    let protocol_context = match (protocol_context, request_log_level) {
        (Some(context), Some(level)) => Some(context.with_resolved_log_level(level)),
        (context, _) => context,
    };

    // `Box::pin` the dispatch future: the discover per-path assembly (Plan 112-10)
    // grows it past clippy's large_future threshold; boxing keeps the handler
    // future small without changing behavior. Pre-dates plan 113.1 and is kept —
    // unlike the fast path's, where an outer box was added by the extraction and
    // measured unnecessary (see `dispatch_message_fast`'s call site).
    Ok(Box::pin(dispatch_message_with_middleware(
        &state,
        ingress,
        MiddlewareDispatch {
            is_init_request,
            response_session_id,
            asserted_protocol_version: protocol_version,
            protocol_context,
            v2_outbound,
            sessions_on,
            progress_queue,
        },
        auth_context,
        http_middleware,
        &http_context,
    ))
    .await)
}

/// Handle GET requests for SSE streams.
///
/// # Split, not moved (plan 117-13)
///
/// This head is ALWAYS compiled. Everything after [`v2_verb_rejection`] is v1 —
/// SSE is a MCP 2025-11-25 transport feature and 2026-07-28 answers `405` — so
/// the body lives in the `v1` pair while the rejection stays here, reachable on
/// both feature sets.
///
/// That shape is what keeps the two 405s distinguishable:
///
/// * on a `v1-compat` build the rejection fires only for a request that opted
///   into 2026-07-28 at a server that accepts it, and every other GET runs the
///   real `v1::handle_get_sse_body`;
/// * on a `full-v2` build the rejection still fires for that same request, and
///   the twin body answers `405` for everything else — so GET is refused
///   unconditionally, but by way of a ROUTED handler rather than a missing route
///   (see [`method_not_allowed_for_verb`]).
///
/// The v1 pipeline it delegates to was extracted in 75-01 Task 1a-A
/// (`resolve_sse_session`, `replay_sse_events_from_header`,
/// `sse_event_for_message`, `attach_sse_response_headers`); those helpers are now
/// module-internal to the real half, because this is their only caller.
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(rejection) = v2_verb_rejection(&state, &headers, "GET").await {
        return rejection;
    }
    v1::handle_get_sse_body(&state, &headers).await
}

/// Handle DELETE requests to terminate sessions.
///
/// # Split, not moved (plan 117-13)
///
/// Same shape as [`handle_get_sse`]: the [`v2_verb_rejection`] head is always
/// compiled, and the session-teardown body — which only means anything where
/// sessions exist — lives in the `v1` pair. On a `full-v2` build the twin
/// answers `405` unconditionally; the route itself is never removed.
async fn handle_delete_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(rejection) = v2_verb_rejection(&state, &headers, "DELETE").await {
        return rejection;
    }
    v1::handle_delete_body(&state, &headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The era chokepoints moved into the `v1` paired module (plan 117-09).
    // Imported by name so every assertion below is UNCHANGED — a moved
    // function that needed its call sites edited would be a move that
    // changed behaviour.
    use super::v1::{apply_session_header, sessions_active_for};
    use crate::types::protocol::Era;

    /// `true` when the compiled half of the `v1` pair is the REAL one.
    ///
    /// The v1 chokepoints are a paired module: on `full-v2` `sessions_active_for`
    /// answers `false` for EVERY input and `apply_session_header` emits nothing,
    /// BY CONSTRUCTION. Expressing that as one const keeps the truth tables below
    /// running on BOTH feature sets — so a severed build pins the TWIN's answers
    /// instead of the tests simply vanishing on the build this phase exists to
    /// create.
    ///
    /// This is NOT the tautology `tests/v2_client_carries_no_session_on_severed_build.rs`
    /// was carrying (a `cfg!` assertion inside a file its own `#![cfg]` already
    /// guaranteed). Here `cfg!` selects an EXPECTED VALUE that genuinely differs
    /// between the two builds, and each assertion can fail on either one.
    const V1_HALF_IS_COMPILED: bool = cfg!(feature = "v1-compat");

    // -----------------------------------------------------------------------
    // Session era gate (Plan 113-04, HTTP-01).
    // -----------------------------------------------------------------------

    /// The full four-row truth table from the plan's `<behavior>` block.
    ///
    /// Runs on BOTH feature sets. The two rows that differ are expressed against
    /// [`V1_HALF_IS_COMPILED`], so on `full-v2` this test pins the TWIN's
    /// "always false" answer rather than being skipped.
    #[test]
    fn sessions_active_truth_table() {
        // A stateful config + a v2 request → sessions OFF (the whole point of
        // HTTP-01: the era overrides the build-time config).
        assert!(!sessions_active_for(true, Some(Era::V2)));
        // A stateful config + a v1 request → sessions ON, exactly as before —
        // and OFF on a build with no v1 to have a session for.
        assert_eq!(
            sessions_active_for(true, Some(Era::V1)),
            V1_HALF_IS_COMPILED
        );
        // A stateful config on a server NOT opted into v2 → sessions ON. `None`
        // means zero era code ran at all (D-04).
        assert_eq!(sessions_active_for(true, None), V1_HALF_IS_COMPILED);
        // An explicitly `stateless()` server stays stateless in every era.
        assert!(!sessions_active_for(false, Some(Era::V2)));
        assert!(!sessions_active_for(false, Some(Era::V1)));
        assert!(!sessions_active_for(false, None));
    }

    /// A v2 request NEVER has sessions, whatever the config says.
    #[test]
    fn v2_always_suppresses_sessions() {
        for cfg in [true, false] {
            assert!(
                !sessions_active_for(cfg, Some(Era::V2)),
                "v2 must be session-free with cfg_has_generator = {cfg}"
            );
        }
    }

    /// `apply_session_header` is the ONLY session-header emitter, and it emits
    /// nothing when sessions are inactive — defense in depth for HTTP-01.
    #[test]
    fn session_header_is_never_emitted_when_sessions_are_inactive() {
        let sid = "sess-123".to_string();

        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&sid), false);
        assert!(
            headers.get(MCP_SESSION_ID).is_none(),
            "sessions inactive → no Mcp-Session-Id"
        );

        // Sessions active → the id is echoed, on a build that HAS sessions. The
        // twin emits nothing for any input, which is the same claim stated
        // structurally rather than conditionally.
        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&sid), true);
        assert_eq!(
            headers.get(MCP_SESSION_ID).and_then(|v| v.to_str().ok()),
            V1_HALF_IS_COMPILED.then_some("sess-123"),
        );

        // No id to emit → nothing emitted, even with sessions active.
        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, None, true);
        assert!(headers.get(MCP_SESSION_ID).is_none());

        // A header-unrepresentable id is SKIPPED, never unwrapped (T-112-13).
        let bad = "bad\nvalue".to_string();
        let mut headers = HeaderMap::new();
        apply_session_header(&mut headers, Some(&bad), true);
        assert!(headers.get(MCP_SESSION_ID).is_none());
    }

    proptest::proptest! {
        /// The predicate never panics and is EXACTLY the stated boolean
        /// expression over arbitrary `(bool, Option<Era>)` inputs.
        #[test]
        fn sessions_active_is_exactly_its_stated_expression(
            cfg_has_generator in proptest::prelude::any::<bool>(),
            era_code in 0u8..3,
        ) {
            let era = match era_code {
                0 => None,
                1 => Some(Era::V1),
                _ => Some(Era::V2),
            };
            let expected =
                V1_HALF_IS_COMPILED && !matches!(era, Some(Era::V2)) && cfg_has_generator;
            proptest::prop_assert_eq!(sessions_active_for(cfg_has_generator, era), expected);
        }
    }

    #[test]
    fn extract_custom_claim_header_inserted_under_cognito_key() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "user-123".parse().unwrap());
        h.insert(
            "x-pmcp-claim-custom-primary-creator",
            "rosen".parse().unwrap(),
        );
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(
            ctx.claims.get("custom:primary_creator"),
            Some(&serde_json::Value::String("rosen".into())),
        );
    }

    #[test]
    // Why: spec sdk-issue-pmcp-claim-custom-extraction.md line 112 pins
    // this assertion byte-identically; clippy::unnecessary_get_then_check
    // would rewrite to !contains_key(...) which is semantically equivalent
    // but breaks the cross-repo verbatim invariant.
    #[allow(clippy::unnecessary_get_then_check)]
    fn extract_custom_claim_empty_value_dropped() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "user-123".parse().unwrap());
        h.insert("x-pmcp-claim-custom-empty", "".parse().unwrap());
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert!(ctx.claims.get("custom:empty").is_none());
    }

    #[test]
    fn extract_custom_claim_kebab_to_snake() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "u".parse().unwrap());
        h.insert(
            "x-pmcp-claim-custom-promo-code",
            "SUMMER25".parse().unwrap(),
        );
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(
            ctx.claims.get("custom:promo_code"),
            Some(&serde_json::Value::String("SUMMER25".into())),
        );
    }

    #[test]
    fn extract_custom_claim_coexists_with_standard_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-pmcp-user-id", "u".parse().unwrap());
        h.insert("x-pmcp-user-email", "u@example.com".parse().unwrap());
        h.insert("x-pmcp-user-groups", "g1,g2".parse().unwrap());
        h.insert("x-pmcp-claim-custom-tier", "gold".parse().unwrap());
        let ctx = extract_auth_from_proxy_headers(&h).expect("auth ctx");
        assert_eq!(ctx.subject, "u");
        assert_eq!(ctx.claims["email"], "u@example.com");
        assert_eq!(ctx.claims["custom:tier"], "gold");
    }

    // ======================================================================
    // v2 required-header classifier (Plan 112-06, VERS-05 / D-05 / D-06).
    // Unit + property coverage of the PURE, non-panicking gate helpers.
    // ======================================================================

    use crate::types::protocol::error_codes::{HEADER_MISMATCH, METHOD_NOT_FOUND};
    use crate::types::protocol::PROTOCOL_VERSION_2026_07_28 as V2;

    /// Build a `HeaderMap` from `(name, value)` pairs for classifier tests.
    fn headers_from(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            let name = http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            h.insert(name, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn decode_version_header_classifies_each_kind() {
        assert_eq!(
            decode_version_header(&headers_from(&[])),
            HeaderProtocolVersion::Absent
        );
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, V2)])),
            HeaderProtocolVersion::V2
        );
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, "2025-11-25")])),
            HeaderProtocolVersion::Other
        );
        // Oversized value → Malformed, never a panic.
        let big = "x".repeat(MAX_V2_HEADER_VALUE_LEN + 1);
        assert_eq!(
            decode_version_header(&headers_from(&[(MCP_PROTOCOL_VERSION, &big)])),
            HeaderProtocolVersion::Malformed
        );
    }

    /// A raw `params._meta` value carrying `version` (when `Some`) plus
    /// `clientCapabilities`, for the verdict truth table below.
    fn meta_value(version: Option<&str>) -> serde_json::Value {
        use crate::types::protocol::context::{
            RESERVED_CLIENT_CAPABILITIES_KEY, RESERVED_PROTOCOL_VERSION_KEY,
        };
        let mut meta = serde_json::Map::new();
        if let Some(version) = version {
            meta.insert(
                RESERVED_PROTOCOL_VERSION_KEY.to_string(),
                serde_json::json!(version),
            );
        }
        meta.insert(
            RESERVED_CLIENT_CAPABILITIES_KEY.to_string(),
            serde_json::json!({}),
        );
        serde_json::Value::Object(meta)
    }

    /// The FULL truth table of the three-way `_meta` verdict, as literal rows.
    ///
    /// This replaced `classify_era_cell_covers_every_matrix_cell`, whose 2x2
    /// shape could not express the row this table's second and third entries
    /// pin: an ABSENT required key is `MissingRequired` (`-32602`), NOT the
    /// `Disagreement` (`-32020`) the old matrix collapsed it into (gap G-6).
    #[test]
    fn classify_v2_meta_version_covers_every_row() {
        use HeaderProtocolVersion as H;
        use V2MetaVerdict as V;
        let unsupported = meta_value(Some("v999.0.0"));
        let legacy = meta_value(Some("2025-11-25"));
        let v2 = meta_value(Some(V2));
        let no_version = meta_value(None);
        let not_an_object = serde_json::json!("definitely not an object");
        let bad_version = serde_json::json!({
            crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY: 42,
        });

        // --- Defer: no v2 signal on either side. ---
        assert_eq!(classify_v2_meta_version(H::Absent, None), V::Defer);
        assert_eq!(classify_v2_meta_version(H::Other, None), V::Defer);
        assert_eq!(classify_v2_meta_version(H::Other, Some(&legacy)), V::Defer);
        assert_eq!(
            classify_v2_meta_version(H::Other, Some(&unsupported)),
            V::Defer,
            "an unsupported version with NO v2 signal is the accept list's business"
        );
        assert_eq!(
            classify_v2_meta_version(H::Absent, Some(&no_version)),
            V::Defer,
            "a v1 request with a `_meta` and no protocolVersion is NOT this rule's business"
        );

        // --- MissingRequired: a v2 signal plus an ABSENT required key. ---
        assert_eq!(
            classify_v2_meta_version(H::V2, None),
            V::MissingRequired(ERR_META_ABSENT)
        );
        assert_eq!(
            classify_v2_meta_version(H::V2, Some(&no_version)),
            V::MissingRequired(ERR_META_NO_PROTOCOL_VERSION)
        );

        // --- Defer: a v2 signal plus an unusable `_meta` carrier. ---
        assert_eq!(
            classify_v2_meta_version(H::V2, Some(&not_an_object)),
            V::Defer,
            "a non-object `_meta` is the resolver's MalformedMeta, not a missing key"
        );
        assert_eq!(
            classify_v2_meta_version(H::V2, Some(&bad_version)),
            V::Defer,
            "a present-but-unusable version is MalformedMeta, not a missing key"
        );

        // --- Disagreement: both sides present, naming different versions. ---
        assert_eq!(
            classify_v2_meta_version(H::V2, Some(&unsupported)),
            V::Disagreement(ERR_HEADER_CLAIMS_V2),
            "G-8: a disagreement is classified BEFORE the accept list"
        );
        assert_eq!(
            classify_v2_meta_version(H::V2, Some(&legacy)),
            V::Disagreement(ERR_HEADER_CLAIMS_V2)
        );
        assert_eq!(
            classify_v2_meta_version(H::Absent, Some(&v2)),
            V::Disagreement(ERR_META_CLAIMS_V2)
        );
        assert_eq!(
            classify_v2_meta_version(H::Other, Some(&v2)),
            V::Disagreement(ERR_META_CLAIMS_V2)
        );
        assert_eq!(
            classify_v2_meta_version(H::Malformed, Some(&v2)),
            V::Disagreement(ERR_META_CLAIMS_V2)
        );

        // --- Enforce: both sides agree on 2026-07-28. ---
        assert_eq!(classify_v2_meta_version(H::V2, Some(&v2)), V::Enforce);
    }

    /// `clientCapabilities` is REQUIRED on an accepted v2 request, `clientInfo`
    /// is NOT, and neither is judged on a request the gate did not accept.
    #[test]
    fn require_v2_client_capabilities_is_the_only_required_optional_key() {
        use crate::types::protocol::context::{
            RESERVED_CLIENT_CAPABILITIES_KEY, RESERVED_CLIENT_INFO_KEY,
        };
        use crate::types::protocol::error_codes::INVALID_PARAMS;

        let with_caps = serde_json::json!({ RESERVED_CLIENT_CAPABILITIES_KEY: {} });
        let info_only =
            serde_json::json!({ RESERVED_CLIENT_INFO_KEY: { "name": "c", "version": "1" } });

        assert!(matches!(
            require_v2_client_capabilities(accepted_v2(), Some(&with_caps)),
            V2GateOutcome::EnforceOk { .. }
        ));
        // clientInfo present, capabilities absent → rejected. clientInfo being
        // present is NOT a substitute: it is a SHOULD with its own MUST-SERVE
        // conformance check.
        let rejected = require_v2_client_capabilities(accepted_v2(), Some(&info_only));
        let V2GateOutcome::Reject { code, message, .. } = rejected else {
            panic!("a capabilities-less accepted v2 request must be rejected");
        };
        assert_eq!(code, INVALID_PARAMS);
        assert_eq!(message, ERR_META_NO_CLIENT_CAPABILITIES);
        let V2GateOutcome::Reject { code, data, .. } =
            require_v2_client_capabilities(accepted_v2(), None)
        else {
            panic!("an accepted v2 request with NO `_meta` at all must be rejected");
        };
        assert_eq!(code, INVALID_PARAMS);
        assert!(data.is_none(), "the required-key rejection carries no data");
        // A request that was NOT accepted is left exactly as it was, so this
        // step can never preempt method retirement or a header rejection.
        assert!(matches!(
            require_v2_client_capabilities(V2GateOutcome::Passthrough, None),
            V2GateOutcome::Passthrough
        ));
        let retired = V2GateOutcome::Reject {
            code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            message: "retired".to_string(),
            data: None,
        };
        assert!(matches!(
            require_v2_client_capabilities(retired, None),
            V2GateOutcome::Reject {
                code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                ..
            }
        ));
    }

    /// Every rejection message this rule can emit is a `&'static str` CONSTANT,
    /// so a rejection can never echo peer-supplied bytes (T-118.1-06-01).
    #[test]
    fn the_meta_rule_messages_are_constants_that_name_their_own_cause() {
        for message in [
            ERR_META_ABSENT,
            ERR_META_NO_PROTOCOL_VERSION,
            ERR_META_NO_CLIENT_CAPABILITIES,
            ERR_HEADER_CLAIMS_V2,
            ERR_META_CLAIMS_V2,
        ] {
            assert!(
                !message.is_empty(),
                "a rejection message must name its cause"
            );
        }
        assert!(ERR_META_NO_PROTOCOL_VERSION
            .contains(crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY));
        assert!(ERR_META_NO_CLIENT_CAPABILITIES
            .contains(crate::types::protocol::context::RESERVED_CLIENT_CAPABILITIES_KEY));
        assert!(
            !ERR_META_NO_CLIENT_CAPABILITIES
                .contains(crate::types::protocol::context::RESERVED_CLIENT_INFO_KEY),
            "clientInfo is a SHOULD and must not appear in a required-key message"
        );
    }

    /// Every row of the [`require_v2_headers`] truth table, including the two
    /// DISTINCT error strings (Phase 118 D-13 / D-18).
    ///
    /// Asserting the messages is deliberate: collapsing them back into one
    /// catch-all would make a rejection stop naming its own cause, and this test
    /// is what fails when that happens.
    #[test]
    fn require_v2_headers_truth_table() {
        let name_bearing = NAME_BEARING_METHODS[0];
        // Version + method + name on a name-bearing method → Ok, name carried.
        let ok = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, name_bearing),
            (MCP_NAME, "search"),
        ]);
        assert_eq!(
            require_v2_headers(&ok).unwrap(),
            (name_bearing.to_string(), "search".to_string())
        );
        // Missing Mcp-Name on a name-bearing method → Err naming Mcp-Name.
        let missing = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, name_bearing)]);
        assert_eq!(require_v2_headers(&missing), Err(ERR_MISSING_MCP_NAME));
        // Missing Mcp-Name on a name-LESS method → Ok (the D-13 change).
        for method in NAME_LESS_METHODS {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, method)]);
            assert_eq!(
                require_v2_headers(&h),
                Ok((method.to_string(), String::new()))
            );
            // A stray value on the same method is accepted and DISCARDED (D-20).
            let stray = headers_from(&[
                (MCP_PROTOCOL_VERSION, V2),
                (MCP_METHOD, method),
                (MCP_NAME, "attacker-supplied"),
            ]);
            assert_eq!(
                require_v2_headers(&stray),
                Ok((method.to_string(), String::new()))
            );
        }
        // Every name-bearing method — MRTR *and* tasks — still demands the header.
        for method in NAME_BEARING_METHODS {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, method)]);
            assert_eq!(require_v2_headers(&h), Err(ERR_MISSING_MCP_NAME));
        }
        // Missing Mcp-Method → the OTHER error, which must not name Mcp-Name.
        let no_method = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_NAME, "search")]);
        assert_eq!(require_v2_headers(&no_method), Err(ERR_MISSING_V2_HEADERS));
        // Missing MCP-Protocol-Version → same, for every method class.
        for method in NAME_BEARING_METHODS.iter().chain(NAME_LESS_METHODS.iter()) {
            let h = headers_from(&[(MCP_METHOD, method), (MCP_NAME, "search")]);
            assert_eq!(require_v2_headers(&h), Err(ERR_MISSING_V2_HEADERS));
        }
        assert!(
            !ERR_MISSING_V2_HEADERS.contains("Mcp-Name"),
            "the universally-required-headers message must not name a header that is \
             only conditionally required (Phase 118 D-13)"
        );
        assert!(ERR_MISSING_MCP_NAME.contains("Mcp-Name"));
        assert!(ERR_MISSING_MCP_NAME.contains("routing name"));
    }

    /// The literal contract for the COMBINED name table (Phase 118 D-18).
    ///
    /// The property test below uses `is_name_bearing_method` as its oracle, which
    /// by construction CANNOT detect a wrong table — the predicate under test
    /// would simply agree with itself. This test's oracle is instead a
    /// hand-written literal list, so a regression of D-18 (the predicate drifting
    /// back to `logical_name_key` and silently dropping the three `tasks/*` rows)
    /// fails HERE. Do not rewrite it to derive its list from the predicate.
    #[test]
    fn is_name_bearing_method_matches_the_literal_contract() {
        for method in NAME_BEARING_METHODS {
            assert!(
                is_name_bearing_method(method),
                "{method} carries a routing name and MUST be name-bearing (D-18)"
            );
        }
        for method in NAME_LESS_METHODS {
            assert!(
                !is_name_bearing_method(method),
                "{method} carries no routing name and MUST NOT be name-bearing"
            );
        }
    }

    #[test]
    fn cross_check_method_and_name_fail_closed() {
        assert!(cross_check_method("tools/call", Some("tools/call")).is_ok());
        assert!(cross_check_method("tools/call", Some("resources/read")).is_err());
        assert!(cross_check_method("tools/call", None).is_err());

        // name-bearing: must match params.name
        assert!(cross_check_name("search", "tools/call", Some("search")).is_ok());
        assert!(cross_check_name("search", "tools/call", Some("other")).is_err());
        assert!(cross_check_name("search", "tools/call", None).is_err());
        // name-less method: presence-only, body name irrelevant
        assert!(cross_check_name("anything", "tools/list", None).is_ok());
    }

    #[test]
    fn classify_v2_request_accepts_well_formed_v2() {
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, "search"),
        ]);
        let out = classify_v2_request(
            &h,
            V2MetaVerdict::Enforce,
            Some("tools/call"),
            Some("search"),
        );
        assert!(matches!(out, V2GateOutcome::EnforceOk { .. }));
    }

    #[test]
    fn classify_v2_request_rejects_method_body_mismatch() {
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, "search"),
        ]);
        // body method disagrees with Mcp-Method (smuggling)
        let out = classify_v2_request(
            &h,
            V2MetaVerdict::Enforce,
            Some("resources/read"),
            Some("search"),
        );
        assert!(matches!(
            out,
            V2GateOutcome::Reject {
                code: HEADER_MISMATCH,
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // The `Mcp-Name` header rule, in BOTH directions.
    //
    // RULE (Phase 118 D-13, widened by D-18 — REVERSES the Phase-113 DRIFT-1
    // adjudication): `Mcp-Name` MUST be present on the methods the COMBINED name
    // table names, and is OPTIONAL and IGNORED on every other v2 method. Its
    // VALUE is cross-checked wherever it is required.
    // -----------------------------------------------------------------------

    #[test]
    fn name_less_method_with_empty_mcp_name_is_enforce_ok() {
        // The Phase-113 client emits `Mcp-Name: ""` for a name-less method. That
        // client is still ACCEPTED after D-13 — this is the compatibility row.
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/list"),
            (MCP_NAME, ""),
        ]);
        let out = classify_v2_request(&h, V2MetaVerdict::Enforce, Some("tools/list"), None);
        assert!(
            matches!(out, V2GateOutcome::EnforceOk { .. }),
            "an EMPTY Mcp-Name on a name-less v2 method must be ACCEPTED"
        );
    }

    #[test]
    fn name_less_method_with_absent_mcp_name_is_accepted() {
        // Header OMITTED entirely. Before Phase 118 this was a `-32020` rejection
        // (the DRIFT-1 presence-on-every-request rule); D-13 reverses that,
        // because the official conformance suite sends exactly this shape.
        let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, "tools/list")]);
        let out = classify_v2_request(&h, V2MetaVerdict::Enforce, Some("tools/list"), None);
        assert!(
            matches!(out, V2GateOutcome::EnforceOk { .. }),
            "an ABSENT Mcp-Name on a name-LESS method must be ACCEPTED (D-13)"
        );
    }

    #[test]
    fn sentinel_encoded_mcp_name_matches_a_non_ascii_body_name() {
        let name = "日本語ツール";
        let encoded = crate::types::mrtr::encode_header_value(name);
        assert_ne!(encoded, name, "a non-ASCII name must be sentinel-encoded");

        // The pure cross-check decodes before comparing.
        assert!(cross_check_name(&encoded, "tools/call", Some(name)).is_ok());
        // ...and still rejects a genuine mismatch.
        assert!(cross_check_name(&encoded, "tools/call", Some("other")).is_err());

        // End to end through the classifier.
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/call"),
            (MCP_NAME, &encoded),
        ]);
        let out = classify_v2_request(&h, V2MetaVerdict::Enforce, Some("tools/call"), Some(name));
        assert!(matches!(out, V2GateOutcome::EnforceOk { .. }));
    }

    #[test]
    fn malformed_mcp_name_sentinel_is_a_header_mismatch() {
        // Opens the sentinel but never closes it / is not valid base64.
        for bad in ["=?base64?not-base64!!", "=?base64?%%%%?="] {
            assert!(
                cross_check_name(bad, "tools/call", Some("search")).is_err(),
                "malformed sentinel `{bad}` must be rejected"
            );
            let h = headers_from(&[
                (MCP_PROTOCOL_VERSION, V2),
                (MCP_METHOD, "tools/call"),
                (MCP_NAME, bad),
            ]);
            let out = classify_v2_request(
                &h,
                V2MetaVerdict::Enforce,
                Some("tools/call"),
                Some("search"),
            );
            assert!(matches!(
                out,
                V2GateOutcome::Reject {
                    code: HEADER_MISMATCH,
                    ..
                }
            ));
        }
    }

    // -----------------------------------------------------------------------
    // v2 HTTP status mapping (Plan 113-04).
    // -----------------------------------------------------------------------

    #[test]
    fn v2_status_table_covers_every_transport_code() {
        use crate::types::protocol::error_codes as ec;
        assert_eq!(
            v2_status_for_code(ec::METHOD_NOT_FOUND),
            StatusCode::NOT_FOUND
        );
        for code in [
            ec::HEADER_MISMATCH,
            ec::MISSING_REQUIRED_CLIENT_CAPABILITY,
            ec::UNSUPPORTED_PROTOCOL_VERSION,
            ec::PARSE_ERROR,
            ec::INVALID_REQUEST,
            ec::INVALID_PARAMS,
        ] {
            assert_eq!(
                v2_status_for_code(code),
                StatusCode::BAD_REQUEST,
                "{code} must map to 400 on v2"
            );
        }
        // Handler semantics stay at HTTP 200 with the error in the body.
        for code in [ec::INTERNAL_ERROR, ec::REQUEST_TIMEOUT, ec::V1_TASK_PENDING] {
            assert_eq!(v2_status_for_code(code), StatusCode::OK);
        }
    }

    #[test]
    fn status_mapping_is_era_gated_so_v1_is_untouched() {
        use crate::types::protocol::Era;
        // v1 and not-opted-in keep the caller's v1 status for EVERY code.
        for era in [None, Some(Era::V1)] {
            for code in [
                METHOD_NOT_FOUND,
                HEADER_MISMATCH,
                crate::types::protocol::error_codes::PARSE_ERROR,
            ] {
                assert_eq!(status_for_error(era, code, StatusCode::OK), StatusCode::OK);
            }
        }
        // v2 re-maps from the table.
        assert_eq!(
            status_for_error(Some(Era::V2), METHOD_NOT_FOUND, StatusCode::OK),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn raw_request_id_survives_a_body_that_never_typed_parses() {
        // Numeric, string and absent ids, plus adversarial bytes — never panics.
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","id":7,"method":"totally/unknown"}"#),
            serde_json::json!(7)
        );
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","id":"abc","method":"nope","params":{}}"#),
            serde_json::json!("abc")
        );
        assert_eq!(
            raw_request_id(br#"{"jsonrpc":"2.0","method":"notify"}"#),
            serde_json::Value::Null
        );
        assert_eq!(raw_request_id(b"{not json"), serde_json::Value::Null);
        assert_eq!(raw_request_id(&[0xff, 0xfe, 0x00]), serde_json::Value::Null);
    }

    #[test]
    fn v2_dispatch_status_reads_the_code_not_the_call_site() {
        use crate::types::jsonrpc::{JSONRPCError, ResponsePayload};
        use crate::types::protocol::Era;

        let error_response = |code: i32| crate::types::JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: crate::types::RequestId::Number(1),
            payload: ResponsePayload::Error(JSONRPCError {
                code,
                message: "x".to_string(),
                data: None,
            }),
        };

        // -32021 is emitted by DISPATCH (plan 09), never by the header gate, so
        // the mapping must be code-driven to reach it at all.
        assert_eq!(
            v2_dispatch_response_status(
                Some(Era::V2),
                &error_response(
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                )
            ),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            v2_dispatch_response_status(Some(Era::V2), &error_response(METHOD_NOT_FOUND)),
            Some(StatusCode::NOT_FOUND)
        );
        // v1 / not-opted-in → no re-map at all.
        assert_eq!(
            v2_dispatch_response_status(Some(Era::V1), &error_response(METHOD_NOT_FOUND)),
            None
        );
        assert_eq!(
            v2_dispatch_response_status(None, &error_response(METHOD_NOT_FOUND)),
            None
        );
        // A successful result is never re-mapped.
        let ok = crate::types::JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: crate::types::RequestId::Number(1),
            payload: ResponsePayload::Result(serde_json::json!({})),
        };
        assert_eq!(v2_dispatch_response_status(Some(Era::V2), &ok), None);
    }

    #[test]
    fn v2_method_not_allowed_only_fires_on_the_v2_version_header() {
        // v2 header on a v2-opted-in server → 405 on both verbs.
        for verb in ["GET", "DELETE"] {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2)]);
            let response = v2_method_not_allowed(&h, verb, true).expect("v2 must be 405");
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        }
        // Absent / v1 / unknown / malformed → the v1 handler runs unchanged.
        assert!(v2_method_not_allowed(&headers_from(&[]), "GET", true).is_none());
        assert!(v2_method_not_allowed(
            &headers_from(&[(MCP_PROTOCOL_VERSION, "2025-11-25")]),
            "GET",
            true
        )
        .is_none());
        let big = "x".repeat(MAX_V2_HEADER_VALUE_LEN + 1);
        assert!(v2_method_not_allowed(
            &headers_from(&[(MCP_PROTOCOL_VERSION, &big)]),
            "DELETE",
            true
        )
        .is_none());
        // D-04: a server that never opted into 2026-07-28 runs ZERO era code, so
        // its v1 GET/DELETE handlers stay reachable no matter what header the
        // client sends.
        for verb in ["GET", "DELETE"] {
            assert!(
                v2_method_not_allowed(&headers_from(&[(MCP_PROTOCOL_VERSION, V2)]), verb, false)
                    .is_none(),
                "{verb}: a non-opted-in server must not answer 405"
            );
        }
    }

    #[test]
    fn unsupported_version_reject_carries_a_supported_array() {
        use crate::types::protocol::context::ProtocolNegotiationError;
        let accept = vec![ProtocolVersion("2025-11-25".to_string()), v2_version()];
        let outcome = negotiation_error_to_gate_reject(
            &ProtocolNegotiationError::UnsupportedVersion("1999-01-01".to_string()),
            &accept,
        );
        let V2GateOutcome::Reject { code, data, .. } = outcome else {
            panic!("an unsupported version must reject");
        };
        assert_eq!(
            code,
            crate::types::protocol::error_codes::UNSUPPORTED_PROTOCOL_VERSION
        );
        let data = data.expect("UNSUPPORTED_PROTOCOL_VERSION MUST carry structured data");
        assert!(
            data["supported"].is_array(),
            "data.supported must be an ARRAY: {data}"
        );
        assert_eq!(data["supported"][0], "2025-11-25");
        assert_eq!(data["requested"], "1999-01-01");

        // A MALFORMED _meta keeps the shared INVALID_PARAMS mapping, no data.
        let outcome = negotiation_error_to_gate_reject(
            &ProtocolNegotiationError::MalformedMeta("bad"),
            &accept,
        );
        let V2GateOutcome::Reject { code, data, .. } = outcome else {
            panic!("malformed _meta must reject");
        };
        assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
        assert!(data.is_none());
    }

    #[test]
    fn extract_body_method_and_name_reads_wire_shape() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("tools/call"));
        assert_eq!(n.as_deref(), Some("search"));
        // Garbage bytes → (None, None), never a panic.
        assert_eq!(extract_body_method_and_name(b"not json"), (None, None));
    }

    #[test]
    fn extract_body_method_and_name_uses_uri_for_resources_read() {
        // resources/read carries its logical name in params.uri (NO params.name).
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"mem://greeting"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("resources/read"));
        assert_eq!(
            n.as_deref(),
            Some("mem://greeting"),
            "resources/read logical name must come from params.uri"
        );

        // prompts/get still resolves the logical name from params.name.
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"greeting"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("prompts/get"));
        assert_eq!(n.as_deref(), Some("greeting"));

        // tools/call remains params.name (unchanged).
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search"}}"#;
        let (m, n) = extract_body_method_and_name(body);
        assert_eq!(m.as_deref(), Some("tools/call"));
        assert_eq!(n.as_deref(), Some("search"));

        // A resources/read carrying only uri yields NO name under the old
        // params.name view — the regression guard for review finding #2.
        let body =
            br#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"file:///x"}}"#;
        let (_, n) = extract_body_method_and_name(body);
        assert_eq!(n.as_deref(), Some("file:///x"));
    }

    #[test]
    fn cross_check_name_accepts_resources_read_uri() {
        // A standards-shaped resources/read cross-checks Mcp-Name against the URI.
        let uri = "mem://greeting";
        assert!(cross_check_name(uri, "resources/read", Some(uri)).is_ok());
        // A disagreeing Mcp-Name is rejected.
        assert!(cross_check_name(uri, "resources/read", Some("mem://other")).is_err());
        // Absent body name (would happen if extraction wrongly read params.name)
        // still fails closed for the name-bearing method.
        assert!(cross_check_name(uri, "resources/read", None).is_err());
    }

    /// Every method the COMBINED table ([`crate::types::mrtr::name_bearing_key`])
    /// names, written out as a LITERAL.
    ///
    /// This list is a hand-written oracle on purpose. The property test below
    /// uses `is_name_bearing_method` as ITS oracle, which cannot detect a wrong
    /// table — the predicate under test would simply agree with itself. This
    /// literal is what pins the table's CONTENTS, and it is the arm that catches
    /// a regression of Phase 118 D-18 (the `tasks/*` rows silently disappearing
    /// because the predicate drifted back to `logical_name_key`).
    const NAME_BEARING_METHODS: [&str; 6] = [
        "tools/call",
        "prompts/get",
        "resources/read",
        "tasks/get",
        "tasks/update",
        "tasks/cancel",
    ];

    /// Representative v2 methods that carry NO routing name, so `Mcp-Name` is
    /// optional and ignored on them (Phase 118 D-13).
    const NAME_LESS_METHODS: [&str; 4] = [
        "tools/list",
        "ping",
        "completion/complete",
        "server/discover",
    ];

    /// The D-13/D-18 truth table, asserted over the composition site.
    ///
    /// `Mcp-Name` is required exactly where the COMBINED name table says the
    /// method carries a routing name, and a stray value on any other method is
    /// discarded (D-20).
    #[test]
    fn classify_v2_request_requires_mcp_name_only_on_name_bearing_methods() {
        // Non-name-bearing, NO Mcp-Name at all → accepted (THE D-13 CHANGE).
        for method in NAME_LESS_METHODS {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, method)]);
            let out = classify_v2_request(&h, V2MetaVerdict::Enforce, Some(method), None);
            assert!(
                matches!(out, V2GateOutcome::EnforceOk { .. }),
                "{method} carries no routing name, so a missing Mcp-Name must be accepted"
            );
        }
        // Name-bearing (MRTR *and* tasks), NO Mcp-Name → rejected.
        for method in NAME_BEARING_METHODS {
            let h = headers_from(&[(MCP_PROTOCOL_VERSION, V2), (MCP_METHOD, method)]);
            let out = classify_v2_request(&h, V2MetaVerdict::Enforce, Some(method), Some("x"));
            assert!(
                matches!(out, V2GateOutcome::Reject { .. }),
                "{method} carries a routing name, so a missing Mcp-Name must be rejected"
            );
        }
        // A tasks method with a DISAGREEING Mcp-Name is rejected (D-18: the
        // cross-check now reaches tasks, closing the emitter/validator asymmetry).
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tasks/get"),
            (MCP_NAME, "task-a"),
        ]);
        assert!(matches!(
            classify_v2_request(
                &h,
                V2MetaVerdict::Enforce,
                Some("tasks/get"),
                Some("task-b")
            ),
            V2GateOutcome::Reject { .. }
        ));
        // A tasks method whose Mcp-Name AGREES with params.taskId passes.
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tasks/get"),
            (MCP_NAME, "task-a"),
        ]);
        assert!(matches!(
            classify_v2_request(
                &h,
                V2MetaVerdict::Enforce,
                Some("tasks/get"),
                Some("task-a")
            ),
            V2GateOutcome::EnforceOk { .. }
        ));
        // A stray Mcp-Name on a name-less method is accepted AND discarded, so it
        // can neither branch downstream logic nor be echoed outbound (D-20).
        let h = headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, "tools/list"),
            (MCP_NAME, "attacker-supplied"),
        ]);
        match classify_v2_request(&h, V2MetaVerdict::Enforce, Some("tools/list"), None) {
            V2GateOutcome::EnforceOk { method, name } => {
                assert_eq!(method, "tools/list");
                assert_eq!(name, "", "a stray Mcp-Name must be sanitized to empty");
            },
            V2GateOutcome::Reject { code, message, .. } => {
                panic!("expected EnforceOk, got Reject({code}, {message})")
            },
            V2GateOutcome::Passthrough => panic!("expected EnforceOk, got Passthrough"),
        }
    }

    #[test]
    fn apply_v2_outbound_headers_sets_all_three_without_panic() {
        let mut h = HeaderMap::new();
        apply_v2_outbound_headers(&mut h, "tools/call", "search");
        assert_eq!(h.get(MCP_METHOD).unwrap(), "tools/call");
        assert_eq!(h.get(MCP_NAME).unwrap(), "search");
        assert_eq!(h.get(MCP_PROTOCOL_VERSION).unwrap(), V2);
    }

    /// One [`V2MetaVerdict`] per index, so a proptest can range over the whole
    /// three-way verdict rather than the old boolean `meta_is_v2`.
    fn verdict_case(kind: u8) -> V2MetaVerdict {
        match kind {
            0 => V2MetaVerdict::Defer,
            1 => V2MetaVerdict::MissingRequired(ERR_META_ABSENT),
            2 => V2MetaVerdict::Disagreement(ERR_HEADER_CLAIMS_V2),
            _ => V2MetaVerdict::Enforce,
        }
    }

    proptest::proptest! {
        /// The classifier NEVER panics over arbitrary header bytes + signal
        /// combinations, and holds the accept/reject invariants (T-112-13).
        #[test]
        fn v2_header_gate_proptest(
            header_kind in 0u8..4,
            verdict_kind in 0u8..4,
            have_method in proptest::bool::ANY,
            have_name in proptest::bool::ANY,
            method_val in "[a-z/]{0,20}",
            name_val in "[a-z]{0,20}",
            body_method in proptest::option::of("[a-z/]{0,20}"),
            body_name in proptest::option::of("[a-z]{0,20}"),
        ) {
            let mut pairs: Vec<(&str, String)> = Vec::new();
            match header_kind {
                0 => {}, // absent
                1 => pairs.push((MCP_PROTOCOL_VERSION, V2.to_string())),
                2 => pairs.push((MCP_PROTOCOL_VERSION, "2025-11-25".to_string())),
                _ => pairs.push((MCP_PROTOCOL_VERSION, "\u{ff}bogus".to_string())),
            }
            if have_method {
                pairs.push((MCP_METHOD, method_val.clone()));
            }
            if have_name {
                pairs.push((MCP_NAME, name_val.clone()));
            }
            let mut h = HeaderMap::new();
            for (k, v) in &pairs {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    let name = http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
                    h.insert(name, hv);
                }
            }

            let verdict = verdict_case(verdict_kind);

            // Must not panic.
            let out = classify_v2_request(&h, verdict, body_method.as_deref(), body_name.as_deref());

            match out {
                V2GateOutcome::Passthrough => {
                    // Only when the `_meta` verdict deferred.
                    proptest::prop_assert_eq!(verdict, V2MetaVerdict::Defer);
                },
                V2GateOutcome::EnforceOk { ref name, .. } => {
                    // Only when the verdict ENFORCES, `Mcp-Method` is present, and
                    // `Mcp-Name` is present OR the method carries no routing name
                    // (Phase 118 D-13, widened by D-18).
                    proptest::prop_assert_eq!(verdict, V2MetaVerdict::Enforce);
                    proptest::prop_assert!(have_method);
                    proptest::prop_assert!(have_name || !is_name_bearing_method(&method_val));
                    // A name is carried ONLY for a name-bearing method (D-20).
                    if !is_name_bearing_method(&method_val) {
                        proptest::prop_assert!(name.is_empty());
                    }
                },
                V2GateOutcome::Reject { code, .. } => {
                    // A required-`_meta`-key rejection is INVALID_PARAMS; every
                    // other rejection this classifier can produce is a header
                    // violation or a header/body disagreement.
                    if matches!(verdict, V2MetaVerdict::MissingRequired(_)) {
                        proptest::prop_assert_eq!(
                            code,
                            crate::types::protocol::error_codes::INVALID_PARAMS
                        );
                    } else {
                        proptest::prop_assert_eq!(code, HEADER_MISMATCH);
                    }
                },
            }
        }
    }

    /// A method strategy that MIXES the name-bearing table with arbitrary noise,
    /// so the property below reaches both classes rather than exercising one.
    ///
    /// The name-bearing arm is drawn from `NAME_BEARING_METHODS`, the literal
    /// list `is_name_bearing_method_matches_the_literal_contract` pins — not from
    /// the predicate — so a wrong table cannot make the property vacuous.
    fn any_v2_method() -> impl proptest::strategy::Strategy<Value = String> {
        use proptest::strategy::Strategy as _;
        proptest::prop_oneof![
            proptest::sample::select(NAME_BEARING_METHODS.as_slice()).prop_map(str::to_string),
            proptest::sample::select(NAME_LESS_METHODS.as_slice()).prop_map(str::to_string),
            "[a-z/]{0,20}",
        ]
    }

    proptest::proptest! {
        /// PROPERTY (Phase 118 D-13 / D-18), over the gate's whole TYPED input
        /// space: `require_v2_headers` returns `Ok` **iff** the version header is
        /// present AND `Mcp-Method` is present AND (`Mcp-Name` is present OR the
        /// method carries no routing name).
        ///
        /// The oracle is `is_name_bearing_method` — the SHARED table — so the
        /// property cannot drift from the predicate the server actually uses.
        /// What the property therefore CANNOT catch is a wrong table; that is
        /// `is_name_bearing_method_matches_the_literal_contract`'s job.
        #[test]
        fn require_v2_headers_is_exactly_its_truth_table(
            have_version in proptest::bool::ANY,
            have_method in proptest::bool::ANY,
            have_name in proptest::bool::ANY,
            method in any_v2_method(),
            name_val in "[a-zA-Z0-9._-]{0,40}",
        ) {
            let mut h = HeaderMap::new();
            if have_version {
                h.insert(
                    http::header::HeaderName::from_bytes(MCP_PROTOCOL_VERSION.as_bytes()).unwrap(),
                    HeaderValue::from_static(crate::types::protocol::PROTOCOL_VERSION_2026_07_28),
                );
            }
            if have_method {
                if let Ok(v) = HeaderValue::from_str(&method) {
                    h.insert(
                        http::header::HeaderName::from_bytes(MCP_METHOD.as_bytes()).unwrap(),
                        v,
                    );
                }
            }
            if have_name {
                h.insert(
                    http::header::HeaderName::from_bytes(MCP_NAME.as_bytes()).unwrap(),
                    HeaderValue::from_str(&name_val).unwrap(),
                );
            }

            let out = require_v2_headers(&h);
            let expected_ok =
                have_version && have_method && (have_name || !is_name_bearing_method(&method));
            proptest::prop_assert_eq!(out.is_ok(), expected_ok);

            if let Ok((got_method, got_name)) = out {
                proptest::prop_assert_eq!(&got_method, &method);
                if is_name_bearing_method(&got_method) {
                    proptest::prop_assert_eq!(&got_name, &name_val);
                } else {
                    // The D-20 sanitization: whatever arrived is DISCARDED.
                    proptest::prop_assert!(got_name.is_empty());
                }
            }
        }

        /// FUZZ, in CLAUDE.md's sanctioned proptest spelling: arbitrary header
        /// BYTES and arbitrary raw body BYTES reach the gate, and nothing panics.
        ///
        /// Header values are built with `HeaderValue::from_bytes`, so non-UTF-8
        /// and RFC 9110 delimiter bytes — which `HeaderValue::from_str` would
        /// never produce — actually arrive at `bounded_header_str`. Bodies go in
        /// as raw `Vec<u8>` through `extract_body_method_and_name`, so a body
        /// that is not JSON at all, or is JSON of the wrong shape, is covered.
        ///
        /// A `fuzz/` target is deliberately NOT used: these are private free
        /// functions, and reaching them from the `fuzz/` sub-workspace would mean
        /// widening pmcp's public API for a test.
        #[test]
        fn v2_header_gate_never_panics_on_arbitrary_bytes(
            version_bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..40),
            method_bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..40),
            name_bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..40),
            body_bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..120),
        ) {
            let mut h = HeaderMap::new();
            for (header_name, raw) in [
                (MCP_PROTOCOL_VERSION, &version_bytes),
                (MCP_METHOD, &method_bytes),
                (MCP_NAME, &name_bytes),
            ] {
                // Skip only what `HeaderValue` itself refuses to represent; every
                // byte string it accepts MUST reach the gate.
                if let Ok(value) = HeaderValue::from_bytes(raw) {
                    h.insert(
                        http::header::HeaderName::from_bytes(header_name.as_bytes()).unwrap(),
                        value,
                    );
                }
            }

            // Must not panic, whatever the bytes say.
            let _ = require_v2_headers(&h);

            // The raw-body reader is on the same unauthenticated path.
            let (body_method, body_name) = extract_body_method_and_name(&body_bytes);
            // ...and so is the three-way `_meta` verdict: it reads the SAME raw
            // body, so arbitrary bytes must reach it too and must not panic.
            let raw_meta = raw_params_meta(&body_bytes);
            let verdict = classify_v2_meta_version(decode_version_header(&h), raw_meta.as_ref());
            let out = classify_v2_request(
                &h,
                verdict,
                body_method.as_deref(),
                body_name.as_deref(),
            );
            // Every rejection is a structured outcome, never an unwind. The
            // required-key arm is INVALID_PARAMS; everything else is a header
            // violation or a header/body disagreement.
            if let V2GateOutcome::Reject { code, .. } = out {
                let expected = if matches!(verdict, V2MetaVerdict::MissingRequired(_)) {
                    crate::types::protocol::error_codes::INVALID_PARAMS
                } else {
                    HEADER_MISMATCH
                };
                proptest::prop_assert_eq!(code, expected);
            }
            // The capabilities step never panics on adversarial `_meta` either.
            let _ = require_v2_client_capabilities(out, raw_meta.as_ref());
        }
    }

    // ---- Phase 112 Plan 10: HttpIngress classification + raw-_meta gate ----

    use crate::types::ProtocolVersion;

    fn v2_version() -> ProtocolVersion {
        ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string())
    }

    /// Build a `ServerState` whose backing `Server` carries `accept` as its
    /// supported-protocol accept-list (the only field the raw gate consults).
    fn state_with_accept(accept: Vec<ProtocolVersion>) -> ServerState {
        let server = Server::builder()
            .name("raw-gate-test")
            .version("1.0.0")
            .with_supported_protocol_versions(accept)
            .build()
            .expect("server builds");
        make_server_state(
            Arc::new(tokio::sync::Mutex::new(server)),
            StreamableHttpServerConfig::default(),
        )
    }

    /// `server/discover` is NOT a name-bearing method — its logical name is
    /// presence-only, so it must not appear in `is_name_bearing_method`.
    #[test]
    fn server_discover_is_not_name_bearing() {
        assert!(!is_name_bearing_method("server/discover"));
    }

    /// A well-formed `server/discover` body classifies as `HttpIngress::Discover`
    /// carrying the original id; any other method or malformed input classifies
    /// as `Public`/`None` (never `Discover`), and never panics.
    ///
    /// The `_meta` is NOT captured here — since Plan 113-04 the single
    /// [`run_v2_header_gate`] reads it from the raw body for every ingress, so a
    /// copy on this variant would be a duplicate read that could drift.
    ///
    /// RENAMED in Phase 114 plan 13 (was `..._server_discover_only`): `only` was
    /// true when `server/discover` was the sole method reaching the
    /// `parse_request_or_internal` peek, and `tasks/update` now reaches it too.
    /// The sibling below covers that method; a name asserting an exclusivity that
    /// no longer holds is the stale-marker failure class 113-29 recorded.
    #[test]
    fn classify_http_ingress_routes_server_discover() {
        let body = br#"{"jsonrpc":"2.0","id":7,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}"#;
        let ingress = classify_http_ingress(body).expect("server/discover classifies");
        match ingress {
            HttpIngress::Discover { id } => {
                assert_eq!(id, crate::types::RequestId::from(7i64));
                // The gate reads the era from the SAME bytes, independently.
                assert_eq!(
                    raw_params_meta(body).unwrap()["io.modelcontextprotocol/protocolVersion"],
                    "2026-07-28"
                );
            },
            HttpIngress::Public(_)
            | HttpIngress::SubscriptionsListen { .. }
            | HttpIngress::TasksUpdate { .. } => {
                panic!("server/discover must classify as Discover")
            },
        }

        // A normal method is NOT a discover ingress.
        let tools = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x"}}"#;
        assert!(classify_http_ingress(tools).is_none());
        // A notification (no id) is NOT a discover ingress.
        let notif = br#"{"jsonrpc":"2.0","method":"server/discover"}"#;
        assert!(classify_http_ingress(notif).is_none());
        // Garbage never panics and never classifies as Discover.
        assert!(classify_http_ingress(b"not json").is_none());
    }

    /// A `tasks/update` body classifies as `HttpIngress::TasksUpdate` carrying the
    /// ORIGINAL id and its params VERBATIM (Phase 114 plan 13, TASK-02).
    ///
    /// The params below are deliberately NOT a well-formed `tasks/update` payload.
    /// Classifying them anyway is the property: the classifier must never reject a
    /// body, because a malformed one has to become a `-32602` in the served branch
    /// AFTER the era, backend, declaration and auth gates — not a parse error
    /// before them, which is what an unauthenticated caller would otherwise see
    /// instead of `-32003`.
    #[test]
    fn classify_http_ingress_routes_tasks_update_with_raw_params() {
        let body = br#"{"jsonrpc":"2.0","id":"u-1","method":"tasks/update","params":{"taskId":42,"junk":[1]}}"#;
        let ingress = classify_http_ingress(body).expect("tasks/update classifies");
        match ingress {
            HttpIngress::TasksUpdate { id, params } => {
                assert_eq!(id, crate::types::RequestId::from("u-1".to_string()));
                assert_eq!(
                    params,
                    serde_json::json!({ "taskId": 42, "junk": [1] }),
                    "the params must reach the served branch UNDECODED"
                );
            },
            HttpIngress::Public(_)
            | HttpIngress::Discover { .. }
            | HttpIngress::SubscriptionsListen { .. } => {
                panic!("tasks/update must classify as TasksUpdate")
            },
        }

        // A notification (no id) is NOT an ingress — it has nothing to answer to.
        let notif = br#"{"jsonrpc":"2.0","method":"tasks/update","params":{}}"#;
        assert!(classify_http_ingress(notif).is_none());
    }

    /// A JSON-RPC body for `method` carrying a v2 `params._meta` under `key`.
    ///
    /// `io.modelcontextprotocol/clientCapabilities` is present because since
    /// Phase 118.1 plan 06 it is a REQUIRED key on every accepted v2 request
    /// (CONF-06 / gap G-6); a body without it is now a `-32602`, which would make
    /// every `EnforceOk` assertion below fail for a reason unrelated to its
    /// subject.
    fn v2_body_bytes(method: &str, key: &str) -> Vec<u8> {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": { key: {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            } },
        })
        .to_string()
        .into_bytes()
    }

    /// The three headers a v2 request for `method` sends (name-less → empty).
    fn v2_headers_for(method: &str) -> HeaderMap {
        headers_from(&[
            (MCP_PROTOCOL_VERSION, V2),
            (MCP_METHOD, method),
            (MCP_NAME, ""),
        ])
    }

    /// `raw_params_meta` reads the SPEC spelling, accepts the legacy `meta`
    /// alias, and never panics on adversarial input.
    #[test]
    fn raw_params_meta_reads_the_spec_spelling_and_the_legacy_alias() {
        let expected = serde_json::json!({ "k": "v" });
        assert_eq!(
            raw_params_meta(
                br#"{"jsonrpc":"2.0","id":1,"method":"m","params":{"_meta":{"k":"v"}}}"#
            ),
            Some(expected.clone())
        );
        assert_eq!(
            raw_params_meta(
                br#"{"jsonrpc":"2.0","id":1,"method":"m","params":{"meta":{"k":"v"}}}"#
            ),
            Some(expected.clone()),
            "the legacy `meta` spelling is accepted, mirroring the typed serde alias"
        );
        // The SPEC spelling wins when both are present.
        assert_eq!(
            raw_params_meta(
                br#"{"jsonrpc":"2.0","id":1,"method":"m","params":{"_meta":{"k":"v"},"meta":{"k":"other"}}}"#
            ),
            Some(expected)
        );
        // Absent / null / no params / garbage → None, never a panic.
        assert_eq!(raw_params_meta(br#"{"jsonrpc":"2.0","params":{}}"#), None);
        assert_eq!(
            raw_params_meta(br#"{"jsonrpc":"2.0","params":{"_meta":null}}"#),
            None
        );
        assert_eq!(raw_params_meta(br#"{"jsonrpc":"2.0","id":1}"#), None);
        assert_eq!(raw_params_meta(b"not json"), None);
        assert_eq!(raw_params_meta(&[0xff, 0xfe, 0x00]), None);
    }

    // -------------------------------------------------------------------
    // MRTR params at v2 ingress (Plan 113-06, HTTP-03 / T-113-44).
    // -------------------------------------------------------------------

    /// An accepted-v2 gate outcome, for the `attach_v2_mrtr_params` tests.
    fn accepted_v2() -> V2GateOutcome {
        V2GateOutcome::EnforceOk {
            method: "tools/call".to_string(),
            name: "search".to_string(),
        }
    }

    /// A v2 `ProtocolContext`, for the `attach_v2_mrtr_params` tests.
    fn v2_context() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(crate::types::protocol::Era::V2, v2_version())
    }

    /// The method [`mrtr_body`] builds — one of the three MRTR-ELIGIBLE methods,
    /// which is what makes the extraction run at all (Phase 114 plan 13).
    ///
    /// Spelled through the production predicate rather than asserted by comment:
    /// if `tools/call` ever left `MRTR_METHODS`, every test below would start
    /// passing vacuously, and this catches that instead.
    fn mrtr_test_method() -> &'static str {
        assert!(
            crate::types::mrtr::mrtr_eligible("tools/call"),
            "these tests exercise the MRTR extraction, which only runs for an eligible method"
        );
        "tools/call"
    }

    /// Body bytes for a `tools/call` carrying arbitrary extra top-level params.
    fn mrtr_body(extra: &serde_json::Value) -> Vec<u8> {
        let mut params = serde_json::json!({ "name": "search", "arguments": {} });
        if let (Some(target), Some(source)) = (params.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
        serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": params,
        })
        .to_string()
        .into_bytes()
    }

    /// The MRTR params of an accepted v2 body land on the threaded context.
    #[test]
    fn attach_v2_mrtr_params_lands_the_fields_on_the_context() {
        let body = mrtr_body(&serde_json::json!({
            "requestState": "opaque-token",
            "inputResponses": { "user_name": { "action": "accept" } },
        }));
        let parsed = raw_body_json(&body);
        let (ctx, outcome) = attach_v2_mrtr_params(
            Some(v2_context()),
            accepted_v2(),
            parsed.as_ref(),
            Some(mrtr_test_method()),
        );
        assert!(matches!(outcome, V2GateOutcome::EnforceOk { .. }));
        let ctx = ctx.expect("context survives");
        assert_eq!(ctx.request_state_token(), Some("opaque-token"));
        assert!(ctx.input_responses().is_some());
    }

    /// A v1 / non-accepted body never gets MRTR params extracted (D-04).
    #[test]
    fn attach_v2_mrtr_params_skips_a_non_accepted_request() {
        let body = mrtr_body(&serde_json::json!({ "requestState": "opaque-token" }));
        let parsed = raw_body_json(&body);
        for outcome in [
            V2GateOutcome::Passthrough,
            V2GateOutcome::Reject {
                code: crate::types::protocol::error_codes::HEADER_MISMATCH,
                message: "nope".to_string(),
                data: None,
            },
        ] {
            let (ctx, _) = attach_v2_mrtr_params(
                Some(v2_context()),
                outcome,
                parsed.as_ref(),
                Some(mrtr_test_method()),
            );
            assert!(
                ctx.expect("context survives")
                    .request_state_token()
                    .is_none(),
                "MRTR extraction must not run outside the accepted v2 path"
            );
        }
    }

    /// A body with NO MRTR fields yields the default (both absent), which
    /// dispatch treats identically to no context-carried MRTR at all.
    #[test]
    fn attach_v2_mrtr_params_absent_fields_are_the_default() {
        let body = mrtr_body(&serde_json::json!({}));
        let parsed = raw_body_json(&body);
        let (ctx, outcome) = attach_v2_mrtr_params(
            Some(v2_context()),
            accepted_v2(),
            parsed.as_ref(),
            Some(mrtr_test_method()),
        );
        assert!(matches!(outcome, V2GateOutcome::EnforceOk { .. }));
        let ctx = ctx.expect("context survives");
        assert!(ctx.request_state_token().is_none());
        assert!(ctx.input_responses().is_none());
    }

    /// Every PRESENT-but-unusable MRTR shape is REJECTED with `INVALID_PARAMS`,
    /// never silently treated as absent (T-113-44).
    #[test]
    fn attach_v2_mrtr_params_rejects_every_malformed_shape() {
        use crate::types::mrtr::{
            MAX_INPUT_RESPONSES, MAX_INPUT_RESPONSE_BYTES, MAX_INPUT_RESPONSE_DEPTH,
            MAX_REQUEST_STATE_LEN,
        };
        let mut too_many = serde_json::Map::new();
        for index in 0..=MAX_INPUT_RESPONSES {
            too_many.insert(
                format!("k{index}"),
                serde_json::json!({ "action": "accept" }),
            );
        }
        let mut chunky = serde_json::Map::new();
        for index in 0..8 {
            chunky.insert(
                format!("k{index}"),
                serde_json::json!({
                    "action": "accept",
                    "content": { "v": "z".repeat(MAX_INPUT_RESPONSE_BYTES - 1_000) }
                }),
            );
        }
        let mut nested = serde_json::json!("leaf");
        for _ in 0..(MAX_INPUT_RESPONSE_DEPTH + 4) {
            nested = serde_json::json!({ "n": nested });
        }
        let cases = [
            // requestState not a string
            serde_json::json!({ "requestState": 42 }),
            // requestState over the length bound
            serde_json::json!({ "requestState": "x".repeat(MAX_REQUEST_STATE_LEN + 1) }),
            // inputResponses not an object
            serde_json::json!({ "inputResponses": [] }),
            // too many inputResponses entries
            serde_json::json!({ "inputResponses": too_many }),
            // one entry over the per-entry byte bound
            serde_json::json!({ "inputResponses": {
                "big": { "action": "accept",
                         "content": { "v": "y".repeat(MAX_INPUT_RESPONSE_BYTES + 1) } } } }),
            // entries over the TOTAL byte bound
            serde_json::json!({ "inputResponses": chunky }),
            // one entry over the depth bound
            serde_json::json!({ "inputResponses": {
                "deep": { "action": "accept", "content": { "v": nested } } } }),
            // an entry matching none of the three permitted result shapes
            serde_json::json!({ "inputResponses": { "bad": { "totally": "wrong" } } }),
        ];
        for case in cases {
            let body = mrtr_body(&case);
            let parsed = raw_body_json(&body);
            let (_, outcome) = attach_v2_mrtr_params(
                Some(v2_context()),
                accepted_v2(),
                parsed.as_ref(),
                Some(mrtr_test_method()),
            );
            let V2GateOutcome::Reject { code, .. } = outcome else {
                panic!("a present-but-unusable MRTR field must REJECT, got a pass for {case}");
            };
            assert_eq!(
                code,
                crate::types::protocol::error_codes::INVALID_PARAMS,
                "malformed MRTR maps to -32602 for {case}"
            );
            // …and -32602 renders as HTTP 400 on the v2 status table.
            assert_eq!(
                v2_status_for_code(code),
                StatusCode::BAD_REQUEST,
                "a malformed MRTR field is a 400"
            );
        }
    }

    /// A NON-MRTR-eligible method's top-level `inputResponses` / `requestState`
    /// are IGNORED here, not parsed and not rejected (Phase 114 plan 13).
    ///
    /// This is the regression test for the two halves of one rule disagreeing.
    /// `mrtr_ingest` has always returned `Inert` for a non-eligible method
    /// ("T-113-23: the spec confines MRTR to three methods"); this EXTRACTION site
    /// had no method awareness, so it judged every accepted v2 request's params
    /// against MRTR's bounds at the transport header gate — ahead of every
    /// dispatch-layer gate, including auth.
    ///
    /// `tasks/update` is the method where that mattered: its ENTIRE payload is
    /// `inputResponses`, so an unauthenticated caller's malformed body produced
    /// `-32602` where 114-09's order requires `-32003`. The end-to-end proof lives
    /// in `tests/v2_tasks_update_routing.rs`; this is the unit-level statement of
    /// the same fact.
    #[test]
    fn attach_v2_mrtr_params_ignores_a_non_eligible_method() {
        // The exact shape that rejects on `tools/call` two tests above.
        let malformed = serde_json::json!({ "inputResponses": "not-an-object" });
        for method in ["tasks/update", "tasks/get", "tools/list", "server/discover"] {
            assert!(
                !crate::types::mrtr::mrtr_eligible(method),
                "{method} must be outside MRTR_METHODS for this test to mean anything"
            );
            let body = serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": method,
                "params": { "taskId": "t-1", "inputResponses": "not-an-object" },
            })
            .to_string()
            .into_bytes();
            let parsed = raw_body_json(&body);
            let (ctx, outcome) = attach_v2_mrtr_params(
                Some(v2_context()),
                accepted_v2(),
                parsed.as_ref(),
                Some(method),
            );
            assert!(
                matches!(outcome, V2GateOutcome::EnforceOk { .. }),
                "{method} is not an MRTR method, so its params must not be judged here \
                 (T-114-63/T-114-64); {malformed} was rejected"
            );
            let ctx = ctx.expect("context survives");
            assert!(
                ctx.input_responses().is_none(),
                "{method} must carry NO MRTR-decoded inputResponses on the context"
            );
            assert!(
                ctx.request_state_token().is_none(),
                "{method} must carry NO MRTR requestState on the context"
            );
        }
    }

    /// A method that is absent, or unresolvable from the body, is treated as NOT
    /// eligible — fail-closed on the extraction, which is the safe direction here
    /// because the extraction can only REJECT.
    #[test]
    fn attach_v2_mrtr_params_skips_an_unresolvable_method() {
        let body = mrtr_body(&serde_json::json!({ "requestState": "opaque-token" }));
        let parsed = raw_body_json(&body);
        let (ctx, outcome) =
            attach_v2_mrtr_params(Some(v2_context()), accepted_v2(), parsed.as_ref(), None);
        assert!(matches!(outcome, V2GateOutcome::EnforceOk { .. }));
        assert!(ctx
            .expect("context survives")
            .request_state_token()
            .is_none());
    }

    /// The client-facing rejection names the BOUND, never the offending value
    /// (T-113-10 — no attacker-controlled content echoed back).
    #[test]
    fn attach_v2_mrtr_params_rejection_never_echoes_the_offending_value() {
        let secret = "x".repeat(crate::types::mrtr::MAX_REQUEST_STATE_LEN + 1);
        let body = mrtr_body(&serde_json::json!({
            "inputResponses": { "super-secret-key": { "totally": "wrong" } },
            "requestState": secret,
        }));
        let parsed = raw_body_json(&body);
        let (_, outcome) = attach_v2_mrtr_params(
            Some(v2_context()),
            accepted_v2(),
            parsed.as_ref(),
            Some(mrtr_test_method()),
        );
        let V2GateOutcome::Reject { message, .. } = outcome else {
            panic!("expected a rejection");
        };
        assert!(
            !message.contains("super-secret-key"),
            "message leaked an attacker-supplied key: {message}"
        );
        assert!(
            !message.contains(&secret),
            "message leaked the attacker-supplied value"
        );
    }

    /// THE memo claim: one parse, however many readers ask for it.
    ///
    /// Before `PostBody` the v2 gate and the log-level rule each called
    /// `raw_body_json` themselves, so every POST to an opted-in server walked the
    /// whole request body through `serde_json` twice. Comparing the two borrows by
    /// ADDRESS is what makes this a fence rather than a restatement: a re-parse
    /// hands back a different allocation, so weakening the memo into a
    /// parse-per-call reds this test. It pins the MECHANISM, not the call sites —
    /// a reader that goes back to calling `raw_body_json` directly instead of
    /// asking `PostBody` is a review catch, not a test catch.
    #[test]
    fn post_body_parses_once_and_not_before_it_is_asked() {
        let raw = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let body = PostBody::new(raw);
        assert!(
            body.json.get().is_none(),
            "construction must parse NOTHING — D-04's non-opted-in short-circuit and every \
             pre-auth rejection refuse an attacker-sized body without ever parsing it"
        );

        let first = body.json().expect("a well-formed body parses");
        let second = body.json().expect("the memo still holds it");
        assert!(
            std::ptr::eq(first, second),
            "the second read must BE the first read's value, not an equal-looking re-parse"
        );
        assert_eq!(
            body.raw(),
            raw,
            "the raw bytes are handed through verbatim, for the readers that must see the \
             literal wire value (D-06)"
        );
    }

    /// Adversarial bytes are `None` rather than a panic — and that `None` is
    /// memoized too, so a malformed body is not re-parsed once per reader either.
    #[test]
    fn post_body_memoizes_a_failed_parse() {
        let body = PostBody::new(b"not json at all");
        assert!(body.json().is_none(), "non-JSON bytes read as nothing");
        assert!(body.json().is_none(), "and keep reading as nothing");
        assert!(
            body.json.get().is_some(),
            "the memo is FILLED with the `None` outcome, so the failed parse is paid for once"
        );
    }

    /// D-04 ordering: a NON-opted-in server short-circuits to Passthrough EVEN
    /// WITH a v2 `_meta` present — it must NOT reject as an unsupported version
    /// (the v2 `_meta` is never inspected).
    #[tokio::test]
    async fn v2_gate_non_opted_in_passes_through() {
        let state = state_with_accept(vec![ProtocolVersion("2025-11-25".to_string())]);
        let headers = headers_from(&[(MCP_PROTOCOL_VERSION, V2)]);
        let body = v2_body_bytes("server/discover", "_meta");
        let (ctx, outcome) = run_v2_header_gate(
            &state,
            &headers,
            &PostBody::new(&body),
            Some(crate::types::protocol::SERVER_DISCOVER_METHOD),
        )
        .await;
        assert!(ctx.is_none(), "non-opted-in resolves no context");
        assert!(
            matches!(outcome, V2GateOutcome::Passthrough),
            "non-opted-in + v2 _meta must Passthrough, not Reject"
        );
    }

    /// D-113-B, the whole point of the raw-body read: EVERY method can be a v2
    /// request, including the list-shaped ones that carry no typed `_meta` field
    /// (and cannot be given one without a MAJOR semver break).
    #[tokio::test]
    async fn v2_gate_accepts_every_method_from_the_raw_body() {
        let state = state_with_accept(vec![
            ProtocolVersion("2025-11-25".to_string()),
            v2_version(),
        ]);
        for method in [
            "tools/list",
            "prompts/list",
            "resources/list",
            "resources/templates/list",
            "completion/complete",
        ] {
            let body = v2_body_bytes(method, "_meta");
            let (ctx, outcome) =
                run_v2_header_gate(&state, &v2_headers_for(method), &PostBody::new(&body), None)
                    .await;
            assert_eq!(
                ctx.map(|c| c.era),
                Some(crate::types::protocol::Era::V2),
                "{method} must resolve to the v2 era from its raw params._meta"
            );
            assert!(
                matches!(outcome, V2GateOutcome::EnforceOk { .. }),
                "{method} must be accepted as a v2 request"
            );
        }
    }

    /// The discover ingress runs the SAME gate, with its method PINNED by
    /// classification rather than read from the body.
    #[tokio::test]
    async fn v2_gate_discover_pins_its_method() {
        let state = state_with_accept(vec![
            ProtocolVersion("2025-11-25".to_string()),
            v2_version(),
        ]);
        let headers = v2_headers_for(crate::types::protocol::SERVER_DISCOVER_METHOD);
        // A body whose `method` field disagrees cannot fool the cross-check:
        // the override pins how the request was actually routed.
        let body = v2_body_bytes("tools/call", "_meta");
        let (ctx, outcome) = run_v2_header_gate(
            &state,
            &headers,
            &PostBody::new(&body),
            Some(crate::types::protocol::SERVER_DISCOVER_METHOD),
        )
        .await;
        assert_eq!(ctx.map(|c| c.era), Some(crate::types::protocol::Era::V2));
        assert!(matches!(outcome, V2GateOutcome::EnforceOk { .. }));
    }

    /// An opted-in server sees a v2 `_meta` with NO `MCP-Protocol-Version` header
    /// rejected by the SAME matrix cell that rejects a tools/call with the same
    /// defect.
    #[tokio::test]
    async fn v2_gate_v2_meta_without_header_rejects() {
        let state = state_with_accept(vec![
            ProtocolVersion("2025-11-25".to_string()),
            v2_version(),
        ]);
        // No MCP-Protocol-Version header → conflict cell → Reject.
        let headers = headers_from(&[(MCP_METHOD, "tools/list"), (MCP_NAME, "")]);
        let body = v2_body_bytes("tools/list", "_meta");
        let (_ctx, outcome) =
            run_v2_header_gate(&state, &headers, &PostBody::new(&body), None).await;
        assert!(matches!(outcome, V2GateOutcome::Reject { .. }));
    }

    // -----------------------------------------------------------------------
    // Resumability era gate (Plan 113-08, HTTP-05).
    // -----------------------------------------------------------------------

    /// The v1 RESUMABILITY unit tests — the region of this module that is a
    /// statement about MCP 2025-11-25 rather than about the transport.
    ///
    /// Gated because it is v1 all the way down, in two independent ways:
    ///
    /// * it does not COMPILE on `full-v2` — `v1::resumability_store`,
    ///   `v1::EventStoreHandle`, `V1State::event_store` and `LAST_EVENT_ID` are
    ///   all severed, so `cargo test -p pmcp --no-default-features --features
    ///   full-v2` was a hard build failure until this split (the aggregate
    ///   command a developer naturally reaches for), and
    /// * it would not PASS if it did — `resumability_active_for(true, Some(V1))`
    ///   is `true` here and `false` on the twin BY CONSTRUCTION, which is the
    ///   severance working rather than a regression.
    ///
    /// Everything OUTSIDE this submodule stays ungated on purpose: those tests
    /// are era-neutral and now RUN on the severed build, which is where the
    /// coverage this phase exists to create actually comes from. The runtime v2
    /// behaviour these tests cannot speak to is proven by
    /// `tests/v2_verbs_405_on_severed_build.rs`,
    /// `tests/v2_client_carries_no_session_on_severed_build.rs` and
    /// `tests/v2_initialize_negotiated_version_header.rs`, all of which CI runs
    /// via `scripts/run-severance-proofs.sh`.
    #[cfg(feature = "v1-compat")]
    mod v1_resumability {
        use super::super::v1::{resumability_active_for, resumability_store};
        use super::*;
        use crate::shared::http_constants::LAST_EVENT_ID;

        /// A `ServerState` accepting BOTH eras, which every resumability test needs
        /// (the v1 half is what keeps the v2 zero-traffic assertions non-vacuous).
        fn dual_era_state() -> ServerState {
            state_with_accept(vec![
                ProtocolVersion(crate::LATEST_PROTOCOL_VERSION.to_string()),
                v2_version(),
            ])
        }

        /// Build a POST for the private fast-path handler — the real POST pipeline,
        /// with no socket in the way.
        fn post_request(extra: &[(&str, &str)], body: &str) -> axum::extract::Request<Body> {
            let mut builder = axum::http::Request::builder()
                .method("POST")
                .uri("/")
                .header(header::CONTENT_TYPE, APPLICATION_JSON)
                .header(
                    header::ACCEPT,
                    crate::shared::http_constants::ACCEPT_STREAMABLE,
                );
            for (name, value) in extra {
                builder = builder.header(*name, *value);
            }
            builder
                .body(Body::from(body.to_string()))
                .expect("request builds")
        }

        /// The three v2 headers plus any extras, as `(&str, &str)` pairs.
        fn v2_post_headers<'a>(
            method: &'a str,
            extra: &[(&'a str, &'a str)],
        ) -> Vec<(&'a str, &'a str)> {
            let mut headers = vec![
                (MCP_PROTOCOL_VERSION, V2),
                (MCP_METHOD, method),
                (MCP_NAME, ""),
            ];
            headers.extend_from_slice(extra);
            headers
        }
        /// An [`EventStore`] that records how many times it was written to and how
        /// many times it was replayed from.
        ///
        /// Asserting "no replay happened" by observing a normal 200 response is weak:
        /// the response looks identical whether replay ran and produced nothing or
        /// never ran at all. The spy is the DIRECT evidence, and its v1 counterpart
        /// (which must record NON-zero) is what keeps the v2 zero assertion honest.
        #[derive(Debug, Default)]
        struct SpyEventStore {
            stores: std::sync::atomic::AtomicUsize,
            replays: std::sync::atomic::AtomicUsize,
        }

        impl SpyEventStore {
            fn stores(&self) -> usize {
                self.stores.load(std::sync::atomic::Ordering::SeqCst)
            }

            fn replays(&self) -> usize {
                self.replays.load(std::sync::atomic::Ordering::SeqCst)
            }
        }

        #[async_trait]
        impl EventStore for SpyEventStore {
            async fn store_event(
                &self,
                _stream_id: &str,
                _event_id: &str,
                _message: &TransportMessage,
            ) -> Result<()> {
                self.stores
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }

            async fn replay_events_after(
                &self,
                _last_event_id: &str,
            ) -> Result<Vec<(String, TransportMessage)>> {
                self.replays
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(Vec::new())
            }

            async fn get_stream_for_event(&self, _event_id: &str) -> Result<Option<String>> {
                Ok(None)
            }
        }

        /// A dual-era state whose event store is a [`SpyEventStore`].
        ///
        /// The spy is injected on `ServerState`, not on the public config, because
        /// `StreamableHttpServerConfig::event_store` is pinned to the concrete
        /// `InMemoryEventStore` and widening that public field would be a MAJOR semver
        /// break (see [`v1::EventStoreHandle`]).
        fn spy_state() -> (ServerState, Arc<SpyEventStore>) {
            let spy = Arc::new(SpyEventStore::default());
            let mut state = dual_era_state();
            state.v1.event_store = Some(spy.clone() as v1::EventStoreHandle);
            (state, spy)
        }

        /// The full four-row truth table from the plan's `<behavior>` block.
        #[test]
        fn resumability_active_truth_table() {
            // A configured store + a v2 request → resumability OFF (HTTP-05).
            assert!(!resumability_active_for(true, Some(Era::V2)));
            // A configured store + a v1 request → ON, exactly as before.
            assert!(resumability_active_for(true, Some(Era::V1)));
            // A configured store on a server NOT opted into v2 → ON (D-04).
            assert!(resumability_active_for(true, None));
            // No store configured → OFF in every era.
            assert!(!resumability_active_for(false, Some(Era::V2)));
            assert!(!resumability_active_for(false, Some(Era::V1)));
            assert!(!resumability_active_for(false, None));
        }

        /// A v2 request NEVER has resumability, whatever the config says.
        #[test]
        fn v2_always_suppresses_resumability() {
            for cfg in [true, false] {
                assert!(
                    !resumability_active_for(cfg, Some(Era::V2)),
                    "v2 must be resumability-free with cfg_has_event_store = {cfg}"
                );
            }
        }

        /// [`resumability_store`] is the gated borrow: it hands out the store on v1
        /// and `None` on v2, from the very SAME state.
        #[test]
        fn resumability_store_is_the_gated_borrow() {
            let (state, _spy) = spy_state();
            assert!(
                resumability_store(&state, Some(Era::V1)).is_some(),
                "v1 keeps the store"
            );
            assert!(
                resumability_store(&state, None).is_some(),
                "a non-opted-in server keeps the store"
            );
            assert!(
                resumability_store(&state, Some(Era::V2)).is_none(),
                "v2 can never reach the store"
            );
        }

        proptest::proptest! {
            /// The predicate never panics and is EXACTLY the stated boolean
            /// expression over arbitrary `(bool, Option<Era>)` inputs.
            #[test]
            fn resumability_active_is_exactly_its_stated_expression(
                cfg_has_event_store in proptest::prelude::any::<bool>(),
                era_code in 0u8..3,
            ) {
                let era = match era_code {
                    0 => None,
                    1 => Some(Era::V1),
                    _ => Some(Era::V2),
                };
                let expected = !matches!(era, Some(Era::V2)) && cfg_has_event_store;
                proptest::prop_assert_eq!(
                    resumability_active_for(cfg_has_event_store, era),
                    expected
                );
            }
        }

        /// A v1 `initialize` exchange writes to the event store — the NON-VACUITY
        /// anchor for every zero assertion below.
        #[tokio::test]
        async fn spy_records_store_traffic_for_a_v1_exchange() {
            let (state, spy) = spy_state();
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": crate::LATEST_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": { "name": "v1", "version": "1.0.0" },
                },
            })
            .to_string();

            let response = handle_post_fast_path(state, post_request(&[], &body)).await;
            assert_eq!(response.status(), StatusCode::OK, "v1 initialize is served");
            assert!(
                spy.stores() > 0,
                "a v1 exchange MUST still write to the event store — otherwise the \
                 v2 zero assertions are vacuous"
            );
        }

        /// The direct evidence for HTTP-05: a v2 exchange produces ZERO event-store
        /// writes and ZERO replays (T-113-29 / T-113-30).
        #[tokio::test]
        async fn spy_records_zero_event_store_traffic_for_a_v2_exchange() {
            let (state, spy) = spy_state();

            let response = handle_post_fast_path(
                state,
                post_request(
                    &v2_post_headers("tools/list", &[(LAST_EVENT_ID, "12345")]),
                    &String::from_utf8(v2_body_bytes("tools/list", "_meta")).unwrap(),
                ),
            )
            .await;

            assert_eq!(
                response.status(),
                StatusCode::OK,
                "a v2 request carrying Last-Event-ID is served NORMALLY"
            );
            assert_eq!(spy.stores(), 0, "a v2 exchange must write NOTHING");
            assert_eq!(spy.replays(), 0, "a v2 exchange must replay NOTHING");
        }

        /// A v1 GET carrying `Last-Event-ID` DOES replay — the non-vacuity anchor
        /// for the replay half, and the guard that v1 resumability is unchanged
        /// (T-113-19).
        #[tokio::test]
        async fn spy_records_replay_for_a_v1_get_with_last_event_id() {
            let (state, spy) = spy_state();
            let headers = headers_from(&[
                (http::header::ACCEPT.as_str(), TEXT_EVENT_STREAM),
                (LAST_EVENT_ID, "evt-1"),
            ]);
            let response = handle_get_sse(State(state), headers).await.into_response();

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                spy.replays(),
                1,
                "a v1 GET with Last-Event-ID must still replay"
            );
        }

        /// ...while the SAME GET on v2 is `405` and never reaches the store at all.
        #[tokio::test]
        async fn spy_records_zero_replay_for_a_v2_get() {
            let (state, spy) = spy_state();
            let headers = headers_from(&[
                (http::header::ACCEPT.as_str(), TEXT_EVENT_STREAM),
                (MCP_PROTOCOL_VERSION, V2),
                (LAST_EVENT_ID, "evt-1"),
            ]);
            let response = handle_get_sse(State(state), headers).await.into_response();

            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(spy.replays(), 0, "a v2 GET must never replay");
            assert_eq!(spy.stores(), 0);
        }

        /// Open a real v1 SSE stream and return its minted session id.
        async fn open_v1_sse_stream(state: &ServerState) -> String {
            let headers = headers_from(&[(http::header::ACCEPT.as_str(), TEXT_EVENT_STREAM)]);
            let response = handle_get_sse(State(state.clone()), headers)
                .await
                .into_response();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "v1 GET opens an SSE stream"
            );
            response
                .headers()
                .get(MCP_SESSION_ID)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
                .expect("a v1 SSE GET mints and echoes a session id")
        }

        /// **The discovery-cache bug class, at the transport layer.**
        ///
        /// `build_response` routes a reply into the v1 SSE stream registered for
        /// `sid` keyed on the
        /// RAW INBOUND `Mcp-Session-Id` header — not on the era-resolved
        /// `response_session_id`, which is always `None` on v2. So a v2 POST that
        /// merely NAMES a v1 caller's open session id had its response delivered into
        /// THAT caller's stream (and written into the event store on the way), while
        /// the v2 caller got a bare `202 Accepted`.
        ///
        /// That is simultaneously T-113-07 (a response reaching a caller that did not
        /// issue it), T-113-29 and T-113-30 (v2 traffic reaching the event store).
        #[tokio::test]
        async fn v2_response_is_never_routed_into_a_session_sse_stream() {
            let state = dual_era_state();
            let victim_session = open_v1_sse_stream(&state).await;

            let response = handle_post_fast_path(
                state.clone(),
                post_request(
                    &v2_post_headers("tools/list", &[(MCP_SESSION_ID, victim_session.as_str())]),
                    &String::from_utf8(v2_body_bytes("tools/list", "_meta")).unwrap(),
                ),
            )
            .await;

            assert_ne!(
                response.status(),
                StatusCode::ACCEPTED,
                "a v2 response must NEVER be handed to a session SSE stream — \
                 202 Accepted means it went to the v1 caller instead of this one"
            );
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "the v2 caller must get its OWN response back"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Direct-response id ownership (Plan 113-08, HTTP-05).
    // -----------------------------------------------------------------------

    /// The constructor takes a PAYLOAD, so a stale envelope's id cannot survive:
    /// re-enveloping a cached response with a different live id yields the live
    /// id and the SAME payload, on both the result and error arms.
    #[test]
    fn envelope_for_live_request_restamps_a_cached_payload() {
        use crate::types::jsonrpc::ResponsePayload;

        // A response cached from an EARLIER caller.
        let cached = crate::types::JSONRPCResponse::success(
            crate::types::RequestId::Number(1),
            serde_json::json!({ "cached": true }),
        );
        let live = envelope_for_live_request(
            cached.payload.clone(),
            crate::types::RequestId::String("caller-2".to_string()),
        );
        assert_eq!(live.id, crate::types::RequestId::String("caller-2".into()));
        assert_eq!(live.jsonrpc, "2.0");
        match (&cached.payload, &live.payload) {
            (ResponsePayload::Result(before), ResponsePayload::Result(after)) => {
                assert_eq!(before, after, "the PAYLOAD survives verbatim");
            },
            _ => panic!("the result arm must stay a result"),
        }

        // The error arm is re-stamped identically.
        let cached_error = crate::types::JSONRPCResponse::error(
            crate::types::RequestId::Number(1),
            crate::types::JSONRPCError::new(
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                "nope",
            ),
        );
        let live_error =
            envelope_for_live_request(cached_error.payload, crate::types::RequestId::Number(99));
        assert_eq!(live_error.id, crate::types::RequestId::Number(99));
        let ResponsePayload::Error(error) = live_error.payload else {
            panic!("the error arm must stay an error");
        };
        assert_eq!(
            error.code,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND
        );
    }

    proptest::proptest! {
        /// Whatever id goes in comes out — the constructor never invents,
        /// coerces or drops one, and never panics.
        #[test]
        fn envelope_for_live_request_always_carries_the_supplied_id(
            numeric in proptest::prelude::any::<bool>(),
            number in proptest::prelude::any::<i64>(),
            text in "[a-zA-Z0-9-]{0,32}",
            is_error in proptest::prelude::any::<bool>(),
        ) {
            let live_id = if numeric {
                crate::types::RequestId::Number(number)
            } else {
                crate::types::RequestId::String(text)
            };
            let payload = if is_error {
                crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::JSONRPCError::new(-1, "e"),
                )
            } else {
                crate::types::jsonrpc::ResponsePayload::Result(serde_json::json!({ "k": "v" }))
            };
            let response = envelope_for_live_request(payload, live_id.clone());
            proptest::prop_assert_eq!(response.id, live_id);
        }
    }

    proptest::proptest! {
        /// The raw-body ingress classifier NEVER panics over arbitrary bytes, and
        /// a non-`server/discover` method NEVER classifies as Discover (T-112-13).
        #[test]
        fn classify_http_ingress_never_panics(
            raw in proptest::collection::vec(proptest::num::u8::ANY, 0..512),
            method in "[a-z/]{0,24}",
            oversized in proptest::bool::ANY,
        ) {
            // Arbitrary bytes: must not panic.
            let _ = classify_http_ingress(&raw);

            // A structured request with an arbitrary method: only server/discover
            // may ever classify as Discover.
            let meta_val = if oversized { "x".repeat(20_000) } else { "2026-07-28".to_string() };
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": meta_val } }
            });
            let bytes = serde_json::to_vec(&body).unwrap();
            let classified = classify_http_ingress(&bytes);
            if method != "server/discover" {
                proptest::prop_assert!(
                    !matches!(classified, Some(HttpIngress::Discover { .. })),
                    "non-discover method {} must never classify as Discover",
                    method
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // v2 METHOD RETIREMENT (Phase 118.1 plan 05, CONF-05 / gap G-5).
    //
    // Nested in a module NAMED after the production surface so
    // `cargo test --lib -- retirement` actually selects these tests rather
    // than passing vacuously (the plan-09 lesson). A SIBLING of
    // `subscriptions_listen`, where the two-variant predecessor lived: the
    // rule stopped being a `resources/*` concern once it covered all five
    // methods the 2026-07-28 schema removes.
    // -------------------------------------------------------------------
    mod v2_method_retirement {
        use super::*;

        /// The retirement set is EXACTLY the five methods the 2026-07-28 core
        /// schema removes, spelled as a hand-written literal.
        ///
        /// The oracle is deliberately independent of `V2_RETIRED_METHODS`: a test
        /// that iterated the table under test could only ever agree with itself,
        /// and would keep passing if a sixth entry were added or `ping` were
        /// dropped. This literal is what pins the CONTENTS.
        #[test]
        fn the_retirement_set_is_exactly_the_five_schema_removed_methods() {
            const EXPECTED: [&str; 5] = [
                "initialize",
                "ping",
                "logging/setLevel",
                "resources/subscribe",
                "resources/unsubscribe",
            ];
            for method in EXPECTED {
                assert!(
                    v2_retired_method(method).is_some(),
                    "{method} is absent from the 2026-07-28 schema and MUST be retired on v2"
                );
            }
            assert_eq!(
                V2_RETIRED_METHODS.len(),
                EXPECTED.len(),
                "the table grew or shrank without this oracle moving with it"
            );
            // Methods the schema KEEPS are never retired — including
            // `subscriptions/listen`, the replacement, which retiring would make
            // v2 unable to subscribe at all.
            for kept in [
                "tools/list",
                "tools/call",
                "resources/read",
                "completion/complete",
                crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD,
                crate::types::protocol::SERVER_DISCOVER_METHOD,
            ] {
                assert!(
                    v2_retired_method(kept).is_none(),
                    "{kept} survives in the 2026-07-28 schema and MUST NOT be retired"
                );
            }
        }

        /// T-118.1-05-01: the match is EXACT BYTE EQUALITY over a peer-supplied
        /// string, so no near miss is retired.
        ///
        /// The method name is read at a point earlier and less validated than
        /// typed parsing. Any case folding, trimming or prefix matching added
        /// here would let a peer force a `404` on a method the schema keeps, or
        /// dodge one on a method it removed.
        #[test]
        fn near_miss_method_strings_are_not_retired() {
            for near_miss in [
                "Initialize",
                "INITIALIZE",
                " ping",
                "ping ",
                "ping\n",
                "logging/setlevel",
                "logging/SetLevel",
                "resources/subscribe2",
                "xresources/subscribe",
                "initialize\u{0}",
                // A Unicode look-alike: Cyrillic small 'р' (U+0440), not ASCII 'p'.
                "\u{440}ing",
                "",
            ] {
                assert!(
                    v2_retired_method(near_miss).is_none(),
                    "{near_miss:?} is not one of the five retired methods and must not match"
                );
            }
        }

        /// T-118.1-05-02: the rejection names the TABLE's constant, never the
        /// peer's bytes, and every entry's replacement is a real successor.
        #[test]
        fn the_retirement_message_names_the_constant_and_its_replacement() {
            let (retired, replacement) =
                v2_retired_method("resources/subscribe").expect("it is retired");
            assert_eq!(
                replacement,
                Some(crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD)
            );
            let message = v2_retirement_message(retired, replacement);
            assert!(message.contains("resources/subscribe"), "{message}");
            assert!(message.contains("subscriptions/listen"), "{message}");

            // `ping` has NO replacement, so the message must not invent one.
            let (retired, replacement) = v2_retired_method("ping").expect("it is retired");
            assert_eq!(replacement, None);
            let message = v2_retirement_message(retired, replacement);
            assert!(message.contains("ping"), "{message}");
            assert!(
                !message.contains("use "),
                "ping has no successor RPC; the message must not point at one: {message}"
            );

            assert_eq!(
                v2_retired_method("initialize").expect("it is retired").1,
                Some(crate::types::protocol::SERVER_DISCOVER_METHOD),
                "v2 replaces the initialize handshake with the discover projection"
            );
            assert_eq!(
                v2_retired_method("logging/setLevel")
                    .expect("it is retired")
                    .1,
                Some(LOG_LEVEL_REQUEST_META_KEY),
            );
        }

        /// `retire_v2_method` converts an ACCEPTED v2 request for a retired method
        /// into `METHOD_NOT_FOUND`, and leaves every other outcome untouched.
        ///
        /// The `Passthrough` row is the v1 guarantee in miniature: a v1 request
        /// never reaches `EnforceOk`, so retirement can never fire on it.
        #[test]
        fn retire_v2_method_only_rewrites_an_accepted_retired_method() {
            let retired = retire_v2_method(V2GateOutcome::EnforceOk {
                method: "logging/setLevel".to_string(),
                name: String::new(),
            });
            let V2GateOutcome::Reject { code, message, .. } = retired else {
                panic!("an accepted v2 logging/setLevel must be REJECTED");
            };
            assert_eq!(code, METHOD_NOT_FOUND);
            assert!(message.contains("logging/setLevel"), "{message}");

            // A method the schema keeps stays accepted.
            assert!(matches!(
                retire_v2_method(V2GateOutcome::EnforceOk {
                    method: "tools/list".to_string(),
                    name: String::new(),
                }),
                V2GateOutcome::EnforceOk { .. }
            ));
            // A non-accepted outcome is returned verbatim: retirement must not
            // convert a header/body disagreement into a routine-looking 404.
            assert!(matches!(
                retire_v2_method(V2GateOutcome::Passthrough),
                V2GateOutcome::Passthrough
            ));
            assert!(matches!(
                retire_v2_method(V2GateOutcome::Reject {
                    code: HEADER_MISMATCH,
                    message: "desync".to_string(),
                    data: None,
                }),
                V2GateOutcome::Reject {
                    code: HEADER_MISMATCH,
                    ..
                }
            ));
        }
    }

    // -------------------------------------------------------------------
    // `subscriptions/listen` gate + wire frames (Plan 113-10, HTTP-04).
    //
    // Nested in a module NAMED after the production surface so
    // `cargo test --lib -- subscriptions` actually selects these tests rather
    // than passing vacuously (the plan-09 lesson).
    // -------------------------------------------------------------------
    mod subscriptions_listen {
        use super::*;
        use crate::types::capabilities::{
            PromptCapabilities, ResourceCapabilities, ToolCapabilities,
        };
        use crate::types::subscriptions::{
            advertises_subscriptions, SubscriptionFilter, ACKNOWLEDGED_METHOD,
            SUBSCRIPTIONS_LISTEN_METHOD, SUBSCRIPTION_ID_META_KEY,
        };
        use crate::types::{Implementation, RequestId, ServerCapabilities};

        /// A `ServerCapabilities` advertising exactly ONE of the four
        /// subscription-delivered capabilities, or none for `None`.
        fn only(which: Option<&str>) -> ServerCapabilities {
            let mut caps = ServerCapabilities::default();
            match which {
                Some("tools.listChanged") => {
                    caps.tools = Some(ToolCapabilities {
                        list_changed: Some(true),
                    });
                },
                Some("prompts.listChanged") => {
                    caps.prompts = Some(PromptCapabilities {
                        list_changed: Some(true),
                    });
                },
                Some("resources.listChanged") => {
                    caps.resources = Some(ResourceCapabilities {
                        subscribe: None,
                        list_changed: Some(true),
                    });
                },
                Some("resources.subscribe") => {
                    caps.resources = Some(ResourceCapabilities {
                        subscribe: Some(true),
                        list_changed: None,
                    });
                },
                _ => {},
            }
            caps
        }

        /// The capabilities `server/discover` actually PUBLISHES for `caps`.
        fn projected_capabilities(caps: &ServerCapabilities) -> ServerCapabilities {
            let response = crate::server::core::build_discover_response(
                RequestId::Number(1),
                crate::server::core::DiscoverSource::from(caps),
                &Implementation::new("s", "1"),
                Some(&v2_context()),
            );
            let crate::types::jsonrpc::ResponsePayload::Result(value) = response.payload else {
                panic!("a v2 discover projects a result");
            };
            serde_json::from_value(value["capabilities"].clone())
                .expect("the projection deserializes back into ServerCapabilities")
        }

        #[test]
        fn discover_projection_and_listen_gate_read_the_same_predicate() {
            // THE tripwire, at the unit level: whatever `server/discover`
            // publishes is exactly what the listen gate reads, for each of the
            // four capabilities INDIVIDUALLY plus the advertise-nothing default.
            for which in [
                None,
                Some("tools.listChanged"),
                Some("prompts.listChanged"),
                Some("resources.listChanged"),
                Some("resources.subscribe"),
            ] {
                let caps = only(which);
                let expected = which.is_some();
                assert_eq!(
                    advertises_subscriptions(&caps),
                    expected,
                    "gate verdict for {which:?}"
                );
                assert_eq!(
                    advertises_subscriptions(&projected_capabilities(&caps)),
                    expected,
                    "the discover projection must agree with the gate for {which:?}"
                );
            }
        }

        #[test]
        fn classify_http_ingress_routes_subscriptions_listen() {
            let body = serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": SUBSCRIPTIONS_LISTEN_METHOD,
                "params": { "notifications": { "toolsListChanged": true } },
            }))
            .unwrap();
            let Some(HttpIngress::SubscriptionsListen { id, params }) =
                classify_http_ingress(&body)
            else {
                panic!("subscriptions/listen classifies as its own ingress");
            };
            assert_eq!(id, RequestId::Number(7), "the ORIGINAL id is preserved");
            assert_eq!(
                params.expect("params carried through")["notifications"]["toolsListChanged"],
                json!(true)
            );
        }

        #[test]
        fn classify_http_ingress_leaves_other_methods_alone() {
            for method in ["tools/call", "resources/subscribe", "initialize"] {
                let body = serde_json::to_vec(&json!({
                    "jsonrpc": "2.0", "id": 1, "method": method, "params": {},
                }))
                .unwrap();
                assert!(
                    !matches!(
                        classify_http_ingress(&body),
                        Some(HttpIngress::SubscriptionsListen { .. })
                    ),
                    "{method} must not classify as a listen ingress"
                );
            }
        }

        #[test]
        fn the_ack_frame_is_the_acknowledged_notification() {
            let agreed = SubscriptionFilter {
                tools_list_changed: Some(true),
                ..SubscriptionFilter::default()
            };
            let frame: serde_json::Value =
                serde_json::from_str(&listen_ack_frame(&agreed, &RequestId::Number(1)))
                    .expect("the ack frame is JSON");
            assert_eq!(frame["jsonrpc"], json!("2.0"));
            assert_eq!(frame["method"], json!(ACKNOWLEDGED_METHOD));
            assert!(
                frame.get("id").is_none(),
                "the acknowledgement is a NOTIFICATION, so it carries no id"
            );
            assert_eq!(
                frame["params"]["notifications"],
                json!({ "toolsListChanged": true })
            );
            assert_eq!(
                frame["params"]["_meta"][SUBSCRIPTION_ID_META_KEY],
                json!(1),
                "the subscriptionId equals the listen request's JSON-RPC id"
            );
        }

        #[test]
        fn the_terminal_result_goes_through_the_shared_v2_envelope() {
            let info = Implementation::new("listen-server", "9.9");
            let frame: serde_json::Value = serde_json::from_str(&listen_terminal_result_frame(
                &RequestId::Number(3),
                Some(&v2_context()),
                &info,
            ))
            .expect("the terminal frame is JSON");
            assert_eq!(frame["id"], json!(3), "the response id is the listen id");
            assert_eq!(
                frame["result"]["_meta"][SUBSCRIPTION_ID_META_KEY],
                json!(3),
                "SubscriptionsListenResult._meta carries the REQUIRED subscriptionId"
            );
            assert_eq!(
                frame["result"]["resultType"],
                json!("complete"),
                "resultType comes from the SHARED envelope helper, not a bespoke builder"
            );
            assert_eq!(
                frame["result"]["_meta"][crate::server::core::RESERVED_SERVER_INFO_KEY]["name"],
                json!("listen-server"),
                "serverInfo comes from the SHARED envelope helper too"
            );
        }
    }

    // -------------------------------------------------------------------
    // CONF-07 / G-3's v2 half (D-16): the multi-frame SSE POST response.
    //
    // Named after the production surface so `cargo test --lib -- v2_multi_frame`
    // selects these rather than passing vacuously.
    //
    // The WIRE-level proof lives in `tests/v2_sse_progress.rs`, which drives a
    // real server over real HTTP. What is proved HERE is the pair of properties
    // an end-to-end test cannot reach: the queue's exact bound (the progress
    // reporter's 100 ms rate limit stands between a handler and the queue, so no
    // handler can push a queue to capacity through the public path), and the
    // byte-equality of the hand-rendered framing with axum's own.
    // -------------------------------------------------------------------
    mod v2_multi_frame_sse {
        use super::*;
        use crate::types::notifications::{ProgressNotification, ProgressToken};
        use crate::types::{JSONRPCResponse, Notification, RequestId};

        /// A progress notification carrying `progress` as its value.
        fn progress(value: f64) -> Notification {
            Notification::Progress(ProgressNotification::new(
                ProgressToken::String("tok".to_string()),
                value,
                None,
            ))
        }

        /// T-118.1-12-01, measured DIRECTLY: the queue admits exactly
        /// [`V2_PROGRESS_QUEUE_CAPACITY`] notifications and drops the rest.
        ///
        /// The sink is driven straight, bypassing `ServerProgressReporter` —
        /// whose 100 ms rate limit would otherwise be the thing under test. This
        /// is the assertion that distinguishes "bounded" from "we never happened
        /// to overflow it".
        #[test]
        fn the_progress_queue_is_bounded_at_its_stated_capacity() {
            let overflow = 500;
            let (sink, queue) = new_v2_progress_queue();
            for step in 0..(V2_PROGRESS_QUEUE_CAPACITY + overflow) {
                #[allow(clippy::cast_precision_loss)]
                sink(progress(step as f64));
            }
            let drained = queue.drain();
            assert_eq!(
                drained.len(),
                V2_PROGRESS_QUEUE_CAPACITY,
                "the per-request progress queue must cap at its stated bound, not grow"
            );
        }

        /// DROP-NEWEST, stated in the constant's rustdoc and asserted here: the
        /// frames that survive are the EARLIEST ones.
        #[test]
        fn the_overflow_policy_is_drop_newest() {
            let (sink, queue) = new_v2_progress_queue();
            for step in 0..(V2_PROGRESS_QUEUE_CAPACITY * 2) {
                #[allow(clippy::cast_precision_loss)]
                sink(progress(step as f64));
            }
            let drained = queue.drain();
            let last = drained.last().expect("the queue is non-empty");
            let Notification::Progress(last) = last else {
                panic!("only progress notifications were pushed");
            };
            #[allow(clippy::cast_precision_loss)]
            let expected_last = (V2_PROGRESS_QUEUE_CAPACITY - 1) as f64;
            assert!(
                (last.progress - expected_last).abs() < f64::EPSILON,
                "drop-newest keeps the EARLIEST frames: the last survivor should be \
                 {expected_last}, got {}",
                last.progress
            );
        }

        /// An empty queue drains to nothing, which is what makes a no-progress v2
        /// response fall through to the UNCHANGED builder.
        #[test]
        fn an_unused_queue_drains_empty() {
            let (_sink, queue) = new_v2_progress_queue();
            assert!(
                queue.drain().is_empty(),
                "a handler that never reported progress leaves the queue empty"
            );
        }

        /// The hand-rendered framing is BYTE-IDENTICAL to what axum's `Sse` +
        /// `Event` produce for the same message — modulo the random event id,
        /// which is regenerated per frame by construction.
        ///
        /// This is the control for the one place plan 12 does not reuse the axum
        /// type: the middleware path needs a complete `Vec<u8>` body and `Event`
        /// exposes no serializer, so the framing is written out by hand. Without
        /// this test that copy could silently drift from
        /// [`build_sse_response_from_single_message`]'s.
        #[tokio::test]
        async fn the_hand_rendered_framing_matches_axum_event() {
            let message = TransportMessage::Response(JSONRPCResponse::success(
                RequestId::Number(7),
                json!({"ok": true}),
            ));

            let axum_response = build_sse_response_from_single_message(message.clone());
            let axum_bytes = axum::body::to_bytes(axum_response.into_body(), 64 * 1024)
                .await
                .expect("the one-shot SSE body reads");
            let axum_body = String::from_utf8(axum_bytes.to_vec()).expect("utf8");

            let TransportMessage::Response(response) = message else {
                unreachable!("constructed as a Response above")
            };
            let hand_body = {
                let mut rendered = String::new();
                write_sse_frame(&mut rendered, V2ResponseFrame::Result(response));
                rendered
            };

            // Strip the `id:` line from both — it is a fresh UUID each time.
            let strip_id = |body: &str| {
                body.lines()
                    .filter(|line| !line.starts_with("id:"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert_eq!(
                strip_id(&hand_body),
                strip_id(&axum_body),
                "the hand-rendered frame must match axum's `Event` framing byte for byte \
                 (modulo the per-frame uuid); hand={hand_body:?} axum={axum_body:?}"
            );
        }

        /// The multi-frame body puts every notification BEFORE the result, and
        /// terminates on the result (T-118.1-12-03).
        #[test]
        fn the_body_ends_on_the_result_frame() {
            let response = JSONRPCResponse::success(RequestId::Number(1), json!({"ok": true}));
            let body = render_v2_multi_frame_body(vec![progress(1.0), progress(2.0)], response);

            let data: Vec<&str> = body
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect();
            assert_eq!(
                data.len(),
                3,
                "two notifications plus the result; body: {body}"
            );
            assert!(
                data[0].contains("notifications/progress")
                    && data[1].contains("notifications/progress"),
                "the notifications come first; body: {body}"
            );
            assert!(
                data[2].contains("\"result\""),
                "and the LAST frame is the result; body: {body}"
            );
        }
    }
}
