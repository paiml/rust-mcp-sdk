//! Cancellation token infrastructure and the canonical
//! [`RequestHandlerExtra`] request-context struct handed to every tool,
//! prompt, and resource handler.
//!
//! [`RequestHandlerExtra`] carries two cross-cutting request-scoped surfaces:
//!
//! - `extensions: http::Extensions` — typed-key typemap for cross-middleware
//!   state transfer. Insert/retrieve typed values via
//!   [`RequestHandlerExtra::extensions_mut`] / [`RequestHandlerExtra::extensions`].
//! - `peer: Option<Arc<dyn PeerHandle>>` — server-to-client back-channel
//!   exposing `sample()`, `list_roots()`, and `progress_notify()` from inside
//!   tool/prompt/resource handlers. Non-wasm only. `None` when the enclosing
//!   [`crate::server::core::ServerCore`] was not configured with a
//!   [`crate::server::server_request_dispatcher::ServerRequestDispatcher`]
//!   (tests, custom integrations).
//!
//! # Semver posture
//!
//! [`RequestHandlerExtra`] is now `#[non_exhaustive]`. This is a **breaking
//! change** for any downstream crate that constructed `RequestHandlerExtra`
//! with a positional struct literal. It is **not breaking** for code that
//! uses [`RequestHandlerExtra::new`], [`RequestHandlerExtra::default`], or
//! the `.with_*(...)` builder chain. Migration path: switch positional
//! literals to
//! `RequestHandlerExtra::new(request_id, cancellation_token)
//!     .with_auth_context(...)
//!     .with_peer(...)`.
//!
//! # Known limitation: session-id plumbing
//!
//! The `session_id` field on [`RequestHandlerExtra`] is not populated at
//! dispatch time in v2.2 — [`crate::server::auth::AuthContext`] does not
//! carry a `session_id` and `ProtocolHandler::handle_request` does not thread
//! one. Peer session isolation is therefore enforced at the
//! [`crate::server::Server`] level: each `Server` instance owns one
//! dispatcher bound to one transport, so cross-session confusion requires
//! cross-process access which is out of the current threat model. Follow-on
//! work can plumb `session_id` through `ProtocolHandler::handle_request` when
//! rmcp-parity for session-scoped auth becomes a scheduled phase goal.
//!
//! # Usage
//!
//! ```rust,no_run
//! use pmcp::RequestHandlerExtra;
//!
//! #[derive(Clone)]
//! struct RequestContext { user_id: u64 }
//!
//! // Middleware populates cross-cutting state before the handler runs.
//! let mut extra = RequestHandlerExtra::default();
//! extra.extensions_mut().insert(RequestContext { user_id: 42 });
//!
//! // Handler retrieves the typed value.
//! let ctx = extra.extensions().get::<RequestContext>().cloned();
//! assert!(ctx.is_some());
//! ```
//!
//! For an end-to-end runnable demonstration see
//! `examples/s42_handler_extensions.rs` (extensions) and
//! `examples/s43_handler_peer_sample.rs` (peer.sample from inside a
//! `ToolHandler`).

use crate::error::Result;
use crate::server::progress::ProgressReporter;
use crate::types::{CancelledNotification, Notification};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio_util::sync::CancellationToken;

/// Manages cancellation tokens for requests.
pub struct CancellationManager {
    tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    notification_sender: Option<Arc<dyn Fn(Notification) + Send + Sync>>,
}

impl CancellationManager {
    /// Create a new cancellation manager.
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
        }
    }

    /// Set the notification sender.
    pub fn set_notification_sender(&mut self, sender: Arc<dyn Fn(Notification) + Send + Sync>) {
        self.notification_sender = Some(sender);
    }

    /// Create a cancellation token for a request.
    pub async fn create_token(&self, request_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        let mut tokens = self.tokens.write().await;
        tokens.insert(request_id, token.clone());
        token
    }

    /// Cancel a request by ID.
    pub async fn cancel_request(&self, request_id: String, reason: Option<String>) -> Result<()> {
        let token = {
            let mut tokens = self.tokens.write().await;
            tokens.remove(&request_id)
        };

        if let Some(token) = token {
            // Cancel the token
            token.cancel();

            // Send cancellation notification
            if let Some(sender) = &self.notification_sender {
                let notification =
                    Notification::Client(crate::types::ClientNotification::Cancelled(
                        CancelledNotification::new(crate::types::RequestId::String(
                            request_id.clone(),
                        ))
                        .with_reason(reason.unwrap_or_else(|| "Cancelled by server".to_string())),
                    ));
                sender(notification);
            }
        }

        Ok(())
    }

    /// Remove a completed request's token.
    pub async fn remove_token(&self, request_id: &str) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(request_id);
    }

    /// Check if a request is cancelled.
    pub async fn is_cancelled(&self, request_id: &str) -> bool {
        let tokens = self.tokens.read().await;
        tokens
            .get(request_id)
            .is_some_and(tokio_util::sync::CancellationToken::is_cancelled)
    }

    /// Get the cancellation token for a request.
    pub async fn get_token(&self, request_id: &str) -> Option<CancellationToken> {
        let tokens = self.tokens.read().await;
        tokens.get(request_id).cloned()
    }

    /// Clear all cancellation tokens.
    pub async fn clear(&self) {
        let mut tokens = self.tokens.write().await;
        // Cancel all active tokens
        for token in tokens.values() {
            token.cancel();
        }
        tokens.clear();
    }
}

impl Default for CancellationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancellationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationManager")
            .field(
                "active_tokens",
                &self.tokens.try_read().map_or(0, |t| t.len()),
            )
            .finish()
    }
}

/// The log level in force when nothing configured one for a request.
///
/// `info`, which means `debug` and finer are SUPPRESSED until a client asks for
/// them. Three reasons, in order of weight:
///
/// 1. It is the conventional default across MCP implementations, so a server
///    ported to pmcp behaves the way its operators already expect.
/// 2. A chatty handler cannot flood a client that never opted in. Verbosity is
///    something a client requests, not something a server imposes.
/// 3. The MCP conformance scenario emits at `info` or above, so a server that
///    never configures a level still passes it.
///
/// Raising a level is a per-request decision (see
/// [`RequestHandlerExtra::with_log_level`]); this constant only decides what
/// happens when no decision was made.
pub const DEFAULT_LOG_LEVEL: crate::types::LoggingLevel = crate::types::LoggingLevel::Info;

/// Extra context passed to request handlers.
#[derive(Clone)]
#[non_exhaustive]
pub struct RequestHandlerExtra {
    /// Cancellation token for the request
    pub cancellation_token: CancellationToken,
    /// Request ID
    pub request_id: String,
    /// Session ID
    pub session_id: Option<String>,
    /// Authentication info
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    /// Validated authentication context (if auth is enabled)
    pub auth_context: Option<crate::server::auth::AuthContext>,
    /// Custom metadata for middleware (e.g., OAuth tokens, session data)
    ///
    /// **Security Note**: Metadata may contain sensitive values like OAuth tokens.
    /// The Debug implementation redacts these values to prevent accidental logging.
    pub metadata: HashMap<String, String>,
    /// Optional progress reporter for this request
    #[allow(dead_code)]
    pub progress_reporter: Option<Arc<dyn ProgressReporter>>,
    /// Request-scoped notification sink for `notifications/message` records
    /// emitted by [`RequestHandlerExtra::log`] / [`RequestHandlerExtra::log_with_data`].
    ///
    /// UNGATED by any progress token — a log record is not progress, and a
    /// client that never sent a `progressToken` still gets its logs. `None`
    /// when the enclosing dispatch path attached no sink (unit-test fixtures,
    /// `RequestHandlerExtra::default()`, transports without a back-channel);
    /// emission is then a silent no-op returning `Ok(())`. See
    /// [`RequestHandlerExtra::log`] for the accepted cost of that silence.
    #[allow(dead_code)]
    pub log_sink: Option<Arc<dyn Fn(Notification) + Send + Sync>>,
    /// The log level resolved for this request, if anything set one.
    ///
    /// `None` means nothing set one, and the effective level falls back to
    /// [`DEFAULT_LOG_LEVEL`]. Records strictly below the effective level are
    /// dropped by [`RequestHandlerExtra::log`] before anything is constructed.
    #[allow(dead_code)]
    pub log_level: Option<crate::types::LoggingLevel>,
    /// Task augmentation request from the client (MCP Tasks).
    ///
    /// When `Some`, the client supports async task polling and requested
    /// task-augmented behavior for this call. The tool handler can check
    /// this to decide between a sync path (await and return results) or
    /// an async path (create task, return immediately).
    ///
    /// When `None`, the client does not support tasks or did not request
    /// task mode — the tool should return results synchronously.
    pub task_request: Option<serde_json::Value>,
    /// The request's `_meta` object as raw JSON (MCP `_meta`).
    ///
    /// Populated by the dispatcher from the incoming `tools/call` request so
    /// handlers can read arbitrary namespaced `_meta` keys (beyond the typed
    /// `progress_token`/`_task_id`) without a typed dependency on `RequestMeta`.
    /// `None` when the request carried no `_meta`.
    pub request_meta: Option<serde_json::Value>,
    /// The per-request protocol context resolved once at ingress (MCP v2.5
    /// version plumbing).
    ///
    /// When `Some`, the dispatcher resolved the negotiated protocol version into
    /// an [`Era`](crate::types::protocol::Era) plus optional self-reported client
    /// identity and surfaced it here so handlers can era-gate behavior via the
    /// typed [`era`](RequestHandlerExtra::era) /
    /// [`protocol_version`](RequestHandlerExtra::protocol_version) /
    /// [`client_info`](RequestHandlerExtra::client_info) /
    /// [`client_capabilities`](RequestHandlerExtra::client_capabilities)
    /// accessors instead of reading ambient session state. `None` when no
    /// context was resolved (unit-test fixtures, pre-v2.5 dispatch paths).
    ///
    /// **Security:** the client identity carried here is SELF-REPORTED and
    /// informational only — see the accessor rustdoc. It MUST NOT be used as an
    /// authorization anchor; real identity binds to the OAuth token.
    pub protocol_context: Option<crate::types::protocol::ProtocolContext>,
    /// Typed request-scoped state for middleware→handler transfer.
    ///
    /// Inserting values requires `T: Clone + Send + Sync + 'static`. Debug prints type names only,
    /// not values, making this safe for logging. Cloning `RequestHandlerExtra` clones the entire
    /// extensions map — prefer `Arc<T>` for large values.
    pub extensions: http::Extensions,
    /// Server-to-client back-channel for `sample` / `list_roots` / `progress_notify`.
    ///
    /// `None` when the request originated from a code path (wasm dispatch, unit-test
    /// fixture, any `ServerCore::new(...)` without
    /// [`crate::server::core::ServerCore::with_server_request_dispatcher`]) that did
    /// not configure a `ServerRequestDispatcher`. Tool/prompt/resource handlers
    /// running under a fully-configured `ServerCore` receive a populated peer handle
    /// that routes `sample` / `list_roots` / `progress_notify` back to the originating
    /// client.
    ///
    /// # Session isolation
    /// Each `Server` instance owns its own dispatcher, so peer handles are scoped to
    /// the enclosing `Server` — cross-session confusion requires cross-process access.
    #[cfg(not(target_arch = "wasm32"))]
    pub peer: Option<Arc<dyn crate::shared::peer::PeerHandle>>,
    /// Handler-authored result `_meta` accumulator (interior-mutable).
    ///
    /// `RequestHandlerExtra` is moved BY VALUE into
    /// [`ToolHandler::handle_output`](crate::server::ToolHandler::handle_output),
    /// so a handler cannot write back to a field the dispatcher still owns. This
    /// shared `Arc<std::sync::Mutex<Option<..>>>` slot bridges the gap: the
    /// dispatcher clones the `Arc` before the move (via
    /// [`RequestHandlerExtra::result_meta_handle`]) and drains it after the
    /// handler returns, merging the drained keys onto the built `CallToolResult`.
    /// See [`RequestHandlerExtra::set_result_meta`]. Cloning `RequestHandlerExtra`
    /// shares the SAME slot (the `Arc` survives the by-value clone, mirroring
    /// `peer`), so any clone the handler receives writes to the observed slot.
    #[cfg(not(target_arch = "wasm32"))]
    result_meta: Arc<std::sync::Mutex<Option<serde_json::Map<String, serde_json::Value>>>>,
}

impl RequestHandlerExtra {
    /// Create new handler extra context.
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
            progress_reporter: None,
            log_sink: None,
            log_level: None,
            task_request: None,
            request_meta: None,
            protocol_context: None,
            extensions: http::Extensions::new(),
            #[cfg(not(target_arch = "wasm32"))]
            peer: None,
            #[cfg(not(target_arch = "wasm32"))]
            result_meta: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set the auth info.
    pub fn with_auth_info(mut self, auth_info: Option<crate::types::auth::AuthInfo>) -> Self {
        self.auth_info = auth_info;
        self
    }

    /// Set the auth context.
    pub fn with_auth_context(
        mut self,
        auth_context: Option<crate::server::auth::AuthContext>,
    ) -> Self {
        self.auth_context = auth_context;
        self
    }

    /// Attach a progress reporter.
    pub fn with_progress_reporter(
        mut self,
        progress_reporter: Option<Arc<dyn ProgressReporter>>,
    ) -> Self {
        self.progress_reporter = progress_reporter;
        self
    }

    /// Attach the request-scoped notification sink for log records.
    ///
    /// The construction seam the dispatch layer uses to wire
    /// [`log`](Self::log) / [`log_with_data`](Self::log_with_data) to whatever
    /// back-channel this request has. Without it both emitters are silent
    /// no-ops that still return `Ok(())`.
    #[must_use]
    pub fn with_log_sink(mut self, sink: Arc<dyn Fn(Notification) + Send + Sync>) -> Self {
        self.log_sink = Some(sink);
        self
    }

    /// Set the log level for this request.
    ///
    /// Records strictly below `level` are dropped by the emitters. When this is
    /// never called the effective level is [`DEFAULT_LOG_LEVEL`].
    #[must_use]
    pub fn with_log_level(mut self, level: crate::types::LoggingLevel) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Set the task request from the client's `tools/call` params.
    ///
    /// When present, the tool handler knows the client supports task-augmented
    /// responses and can choose an async path (create task, return immediately)
    /// instead of awaiting the full result.
    pub fn with_task_request(mut self, task_request: Option<serde_json::Value>) -> Self {
        self.task_request = task_request;
        self
    }

    /// Attach the request's `_meta` object (raw JSON) for handler inspection.
    ///
    /// Handlers can then read arbitrary namespaced `_meta` keys (e.g. team-guard
    /// depth/ancestor state) via `extra.request_meta`. Pass `None` when the
    /// request carried no `_meta`.
    #[must_use]
    pub fn with_request_meta(mut self, meta: Option<serde_json::Value>) -> Self {
        self.request_meta = meta;
        self
    }

    /// Attach the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// resolved at ingress (MCP v2.5 version plumbing).
    ///
    /// Populated by the dispatch layer (Plan 04) after negotiating the protocol
    /// version; handlers then read the era and client identity via the typed
    /// [`era`](Self::era) / [`protocol_version`](Self::protocol_version) /
    /// [`client_info`](Self::client_info) /
    /// [`client_capabilities`](Self::client_capabilities) accessors. Pass `None`
    /// when no context was resolved.
    #[must_use]
    pub fn with_protocol_context(
        mut self,
        ctx: Option<crate::types::protocol::ProtocolContext>,
    ) -> Self {
        self.protocol_context = ctx;
        self
    }

    /// Returns the resolved protocol [`Era`](crate::types::protocol::Era) for
    /// this request, or `None` when no [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// was attached.
    ///
    /// **Security:** the era is derived from the negotiated protocol version —
    /// it is a behavioral switch, NOT an identity or authorization signal.
    #[must_use]
    pub fn era(&self) -> Option<crate::types::protocol::Era> {
        self.protocol_context.as_ref().map(|ctx| ctx.era)
    }

    /// Returns the exact negotiated
    /// [`ProtocolVersion`](crate::types::ProtocolVersion) for this request, or
    /// `None` when no [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// was attached.
    #[must_use]
    pub fn protocol_version(&self) -> Option<&crate::types::ProtocolVersion> {
        self.protocol_context
            .as_ref()
            .map(|ctx| &ctx.negotiated_version)
    }

    /// Returns the client's SELF-REPORTED implementation info, or `None` when
    /// absent.
    ///
    /// **Security — self-reported, not for authorization:** this value is the
    /// client-supplied `clientInfo` surfaced verbatim from initialization. It is
    /// informational ONLY (telemetry, feature-hints, logging) and MUST NOT be
    /// used as an authorization anchor or trusted identity. Real identity binds
    /// to the OAuth token, enforced in Phase 114 (TASK-05). No authorization
    /// decision is made from this accessor in this phase.
    #[must_use]
    pub fn client_info(&self) -> Option<&crate::types::Implementation> {
        self.protocol_context
            .as_ref()
            .and_then(|ctx| ctx.client_info.as_ref())
    }

    /// Returns the client's SELF-REPORTED advertised capabilities, or `None`
    /// when absent.
    ///
    /// # Both eras reach this accessor
    ///
    /// On MCP `2026-07-28` the value comes from the per-request
    /// `_meta["io.modelcontextprotocol/clientCapabilities"]`, resolved once at
    /// ingress. On MCP `2025-11-25` it comes from the `initialize` handshake,
    /// folded into the same `ProtocolContext` at the dispatch root by
    /// `core::fold_v1_handshake_capabilities` (Phase 118.1-08, G-9). This method
    /// reads ONLY `protocol_context.client_capabilities` in either case — it
    /// deliberately does NOT reach for the server-level `client_capabilities`
    /// lock, because it is a sync method on a value already moved into the
    /// handler while the dispatch path holds that lock, and that shape
    /// deadlocks.
    ///
    /// Absent both signals the answer is `None`, never a fabricated default.
    ///
    /// **Security — self-reported, not for authorization:** like
    /// [`client_info`](Self::client_info), these capabilities are client-supplied
    /// and informational ONLY. They MUST NOT be used as an authorization anchor;
    /// real identity binds to the OAuth token (Phase 114 / TASK-05).
    #[must_use]
    pub fn client_capabilities(&self) -> Option<&crate::types::ClientCapabilities> {
        self.protocol_context
            .as_ref()
            .and_then(|ctx| ctx.client_capabilities.as_ref())
    }

    /// The client's answers to a PREVIOUS round's `inputRequests`
    /// (MCP 2026-07-28 multi-round-trip elicitation, HTTP-03).
    ///
    /// Returns `None` when the request carried no `inputResponses`, on v1, and
    /// on the re-elicitation path (an unknown-key or expired `requestState` is
    /// re-run as a pristine FIRST call, so nothing MRTR-shaped is observable).
    ///
    /// **Security — CLIENT-SUPPLIED and UNTRUSTED.** Every value here came off
    /// the wire. It is bounded in count, per-entry size, total size and nesting
    /// depth at transport ingress, and each entry is proven to be one of the
    /// three spec-permitted result shapes — but nothing validates it against the
    /// schema the handler asked for. A handler MUST schema-validate an entry
    /// before acting on it, exactly as it would validate tool arguments. Compare
    /// [`mrtr_continuation`](Self::mrtr_continuation), which is server-minted.
    #[must_use]
    pub fn input_responses(&self) -> Option<&crate::types::mrtr::InputResponses> {
        self.protocol_context.as_ref()?.input_responses()
    }

    /// The DECRYPTED continuation state from a VERIFIED `requestState` token
    /// (MCP 2026-07-28 multi-round-trip elicitation, HTTP-03).
    ///
    /// **Security — SERVER-MINTED and TRUSTED.** This value is whatever the
    /// handler itself sealed on a previous round. It reaches here only after the
    /// server-owned AEAD codec authenticated the token against the authenticated
    /// principal, the live method and a digest of the request's salient
    /// parameters, so a tampered, cross-principal or cross-request token never
    /// produces a `Some` — it produces a JSON-RPC error instead.
    ///
    /// `None` on a first call, on v1, and inside a re-run handler.
    #[must_use]
    pub fn mrtr_continuation(&self) -> Option<&serde_json::Value> {
        self.protocol_context.as_ref()?.mrtr_continuation()
    }

    /// The multi-round-trip round counter carried by a verified `requestState`.
    ///
    /// A handler uses this to decide it has asked enough times and should fail
    /// or degrade rather than elicit again (D-09). Counts from `0` for the round
    /// the FIRST continuation was minted in; `None` on a first call, on v1, and
    /// inside a re-run handler.
    #[must_use]
    pub fn mrtr_round(&self) -> Option<u8> {
        self.protocol_context.as_ref()?.mrtr_round()
    }

    /// Extracts the W3C trace-context self-reported in the request `_meta`
    /// (MCP v2.5, VERS-09), reading the existing
    /// [`request_meta`](Self::request_meta) — no dedicated field is stored.
    ///
    /// Returns `Some` only when `request_meta` carries an in-bounds
    /// `traceparent` string; `None` when `request_meta` is `None` or lacks a
    /// valid `traceparent`. Delegates to
    /// [`TraceContext::from_meta`](crate::types::protocol::TraceContext::from_meta).
    ///
    /// **Security — raw, bounded, untrusted:** the returned
    /// `traceparent`/`tracestate`/`baggage` values are RAW and UNVALIDATED
    /// self-reported client data, only length-bounded at ingress
    /// (`MAX_TRACE_VALUE_LEN`) per the Plan-01 contract — see
    /// [`TraceContext`](crate::types::protocol::TraceContext). They MUST NOT be
    /// treated as trusted or safe to interpolate into logs/queries without
    /// independent sanitization.
    #[must_use]
    pub fn trace_context(&self) -> Option<crate::types::protocol::TraceContext> {
        crate::types::protocol::TraceContext::from_meta(self.request_meta.as_ref()?)
    }

    /// Returns a reference to the typed extensions map.
    ///
    /// # Example
    /// ```rust,no_run
    /// use pmcp::RequestHandlerExtra;
    /// let extra = RequestHandlerExtra::default();
    /// let _: Option<&String> = extra.extensions().get::<String>();
    /// ```
    pub fn extensions(&self) -> &http::Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the typed extensions map.
    ///
    /// # Example
    /// ```rust,no_run
    /// use pmcp::RequestHandlerExtra;
    /// let mut extra = RequestHandlerExtra::default();
    /// extra.extensions_mut().insert(42u64);
    /// ```
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        &mut self.extensions
    }

    /// Attach a peer handle for server-to-client RPCs.
    ///
    /// When set, tool/prompt/resource handlers can invoke `extra.peer().unwrap().sample(...)`
    /// (or `list_roots` / `progress_notify`) to make outbound requests back to the
    /// originating client. The peer handle is populated by the enclosing
    /// `ServerCore` at each dispatch site when a
    /// [`crate::server::server_request_dispatcher::ServerRequestDispatcher`]
    /// has been attached — tests or ad-hoc constructions that skip the dispatcher
    /// leave this as `None`, and handlers should treat `None` as "no client
    /// available for back-channel".
    ///
    /// # Example
    /// ```rust,no_run
    /// # #[cfg(not(target_arch = "wasm32"))]
    /// # {
    /// use pmcp::RequestHandlerExtra;
    /// use std::sync::Arc;
    /// # fn build_peer() -> Arc<dyn pmcp::PeerHandle> { unimplemented!() }
    /// let extra = RequestHandlerExtra::default().with_peer(build_peer());
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_peer(mut self, peer: Arc<dyn crate::shared::peer::PeerHandle>) -> Self {
        self.peer = Some(peer);
        self
    }

    /// Returns the peer handle, if one was attached.
    ///
    /// Handlers should treat `None` as "no client back-channel available" and
    /// skip any sample/list_roots-dependent code paths gracefully.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn peer(&self) -> Option<&Arc<dyn crate::shared::peer::PeerHandle>> {
        self.peer.as_ref()
    }

    /// Merge handler-authored keys into the result `_meta` for this call.
    ///
    /// This is the lowest-friction way for an existing hand-written handler (one
    /// that returns a plain `Value` and does NOT override
    /// [`ToolHandler::handle_output`](crate::server::ToolHandler::handle_output))
    /// to attach task augmentation or custom `_meta` without owning the full
    /// envelope. The keys are accumulated in an interior-mutable slot and merged
    /// onto the outgoing `CallToolResult`'s `_meta` after the handler returns.
    ///
    /// Takes `&self`: the slot is interior-mutable, so this works even though the
    /// handler receives `RequestHandlerExtra` by value (the slot `Arc` is shared
    /// with the clone the dispatcher retained before the by-value move).
    ///
    /// # Merge precedence
    ///
    /// Keys MERGE — never a whole-map replace:
    /// - a handler-set key **overwrites** a same-name key (whether set by an
    ///   earlier `set_result_meta` call or emitted by widget/native enrichment);
    /// - all unrelated existing `_meta` keys (widget/native emission) are
    ///   **preserved**;
    /// - repeated calls accumulate into the same slot — a later call's colliding
    ///   keys win, non-colliding keys from both calls survive.
    ///
    /// # Path scope
    ///
    /// This applies on **both** tool-output paths, on **both** dispatchers
    /// (`Server` and `ServerCore`):
    ///
    /// - **Payload path** — the keys merge onto the envelope the dispatcher
    ///   builds (text-wrap / widget enrichment / `outputSchema` bridge).
    /// - **[`ToolOutput::Result`](crate::server::ToolOutput::Result) (verbatim)
    ///   path** — since D-06 (Phase 118.1) the keys merge onto the envelope the
    ///   HANDLER authored, immediately before it is emitted. That arm still
    ///   bypasses response middleware, the task create-path gate and the
    ///   text-wrap tail (D-04 / D-04a); the bypass covers the response
    ///   *pipeline*, not the handler's own `_meta`, which is authored by the same
    ///   handler at the same trust level as the envelope itself. Before D-06 the
    ///   keys were silently DROPPED here.
    ///
    /// A verbatim handler may of course still set `_meta` on the value directly,
    /// e.g. via
    /// [`CallToolResult::with_related_task`](crate::types::CallToolResult::with_related_task);
    /// when it does both, the merge precedence above decides — the
    /// `set_result_meta` key wins the collision, and the envelope's unrelated
    /// keys survive.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_result_meta(&self, meta: serde_json::Map<String, serde_json::Value>) {
        let mut guard = self
            .result_meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = guard.get_or_insert_with(serde_json::Map::new);
        for (key, value) in meta {
            slot.insert(key, value);
        }
    }

    /// Clone the interior-mutable result-`_meta` slot into a drain handle.
    ///
    /// The dispatcher calls this BEFORE moving `self` into `handle_output`, then
    /// drains the handle via [`ResultMetaHandle::take_result_meta`] after the
    /// handler returns. Keeping slot access behind these small methods means
    /// dispatch code never touches the lock directly.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn result_meta_handle(&self) -> ResultMetaHandle {
        ResultMetaHandle(Arc::clone(&self.result_meta))
    }

    /// Returns `true` if the client requested task-augmented behavior.
    pub fn is_task_request(&self) -> bool {
        self.task_request.is_some()
    }

    /// Get the auth context if available.
    pub fn auth_context(&self) -> Option<&crate::server::auth::AuthContext> {
        self.auth_context.as_ref()
    }

    /// Get metadata value by key.
    ///
    /// Metadata is typically set by middleware (e.g., OAuth token injection).
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Set metadata value.
    ///
    /// This is typically used by middleware to inject data for tools to consume.
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Check if the request has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Wait for cancellation.
    pub async fn cancelled(&self) {
        self.cancellation_token.cancelled().await;
    }

    /// Report progress if a reporter is available.
    pub async fn report_progress(
        &self,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> crate::Result<()> {
        if let Some(rep) = &self.progress_reporter {
            rep.report_progress(progress, total, message).await
        } else {
            Ok(())
        }
    }

    /// Report percentage progress (0-100) if available.
    pub async fn report_percent(&self, percent: f64, message: Option<String>) -> crate::Result<()> {
        if let Some(rep) = &self.progress_reporter {
            rep.report_percent(percent, message).await
        } else {
            Ok(())
        }
    }

    /// Emit an MCP `notifications/message` log record to this request's client.
    ///
    /// The first PRODUCTION constructor of
    /// [`ServerNotification::LogMessage`](crate::types::ServerNotification::LogMessage).
    /// Synchronous on purpose: the sink is a synchronous
    /// `Arc<dyn Fn(Notification) + Send + Sync>`, no trait is involved, and
    /// making this `async` would force every logging call site into an `.await`
    /// to buy nothing.
    ///
    /// # `Ok(())` is NOT delivery acknowledgement
    ///
    /// `Ok(())` means the record was handed to whatever sink this request has,
    /// or that there was none — it does NOT mean the client received it. The
    /// sink's type is `Fn(Notification) -> ()`: it returns unit and therefore
    /// *cannot* report failure, and the fallback `notification_tx` path in
    /// `Server` likewise ignores its own `try_send` result. Do not build retry
    /// logic on this `Result`. It exists so that a future "sink refused" signal
    /// is an additive change rather than a breaking one.
    ///
    /// # No sink means silence, not an error
    ///
    /// With no sink attached the call is a no-op returning `Ok(())`, exactly as
    /// [`report_progress`](Self::report_progress) is. That keeps a handler
    /// callable outside a server —
    /// [`RequestHandlerExtra::default()`](Self::default) is documented for
    /// testing and simple tool invocations, and a handler that logs must not
    /// become un-unit-testable. The accepted cost, stated plainly: a MISPLUMBED
    /// transport looks identical to a quiet handler. The conformance fence — a
    /// test asserting records actually arrive over the wire — is what catches
    /// that, so this is a production-diagnostics hole rather than a false green
    /// in the gate.
    ///
    /// # Level filtering
    ///
    /// A record strictly below the effective level (this request's
    /// [`with_log_level`](Self::with_log_level), else [`DEFAULT_LOG_LEVEL`]) is
    /// dropped before anything is constructed. Comparison is on the typed
    /// [`LoggingLevel`](crate::types::LoggingLevel), whose declaration order is
    /// syslog severity order — never on the serialized strings, where
    /// `"critical" < "debug"` inverts the filter.
    ///
    /// # No rate limiting, and what that does and does not promise
    ///
    /// Unlike progress reporting, this emitter applies NO rate limit. Progress
    /// values are idempotent — dropping an intermediate one loses nothing
    /// because the next supersedes it — whereas each log record is the only
    /// copy of its information, so a limiter would silently delete evidence.
    ///
    /// That is a statement about the EMITTER, not a promise that no record can
    /// ever be dropped downstream. On MCP 2026-07-28 the vehicle a record rides
    /// is the request's bounded progress queue: an `mpsc::channel(64)` with an
    /// explicit DROP-NEWEST `try_send` policy, because the sink closure is
    /// synchronous and must not block. A handler emitting more than that
    /// capacity of notifications in one call LOSES THE EXCESS, and every one of
    /// those calls still returns `Ok(())`. A handler that must not lose records
    /// should keep a single call under the queue capacity.
    ///
    /// # When records go nowhere on 2026-07-28
    ///
    /// That vehicle is attached only when the request is multi-frame eligible —
    /// era is 2026-07-28, the server is not configured for JSON responses, and
    /// the request's `Accept` header includes `text/event-stream`. A 2026-07-28
    /// client that asked for JSON only therefore receives no log records at all.
    /// That is correct behaviour for a client that declined the streaming
    /// channel, not a bug.
    ///
    /// # Capabilities
    ///
    /// Emission is deliberately NOT gated on
    /// [`ServerCapabilities::logging`](crate::types::ServerCapabilities). The
    /// sink is per-request and the capability is advisory; gating here would
    /// make a correctly-plumbed server silently mute. Servers SHOULD still
    /// declare the capability so clients know to expect records.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::LoggingLevel;
    /// use pmcp::RequestHandlerExtra;
    ///
    /// // No server, so no sink — the call succeeds and emits nothing.
    /// let extra = RequestHandlerExtra::default();
    /// assert!(extra.log(LoggingLevel::Info, "starting work").is_ok());
    /// ```
    pub fn log(&self, level: crate::types::LoggingLevel, message: impl Into<String>) -> Result<()> {
        self.emit_log_record(level, message.into(), None);
        Ok(())
    }

    /// Emit a `notifications/message` log record carrying structured data.
    ///
    /// Identical to [`log`](Self::log) in every respect — same synchronous
    /// emission, same level filtering, same no-sink silence, and the same
    /// `Ok(())`-is-not-acknowledgement caveat — except that `data` is attached
    /// to the record's `data` member.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::types::LoggingLevel;
    /// use pmcp::RequestHandlerExtra;
    /// use serde_json::json;
    ///
    /// let extra = RequestHandlerExtra::default();
    /// assert!(extra
    ///     .log_with_data(LoggingLevel::Warning, "retrying", json!({ "attempt": 2 }))
    ///     .is_ok());
    /// ```
    pub fn log_with_data(
        &self,
        level: crate::types::LoggingLevel,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Result<()> {
        self.emit_log_record(level, message.into(), Some(data));
        Ok(())
    }

    /// The one place the filter rule and the record shape live.
    ///
    /// Both public emitters delegate here so the level comparison exists exactly
    /// once — two copies of a filter is two chances to get the direction wrong.
    ///
    /// Returns `()`, not `Result<()>`: nothing here can fail today, and a
    /// private helper that wraps an infallible body in `Ok` is a lie the
    /// compiler cannot catch. The PUBLIC emitters still return `Result<()>` —
    /// that is a deliberate evolution seam so a future "sink refused" signal is
    /// additive rather than breaking, and it is not a claim that this body has
    /// a failure mode.
    ///
    /// # `data` is always populated, even when the caller supplied none
    ///
    /// `LoggingMessageNotificationParams` declares `data` REQUIRED with no
    /// `message` member at all (`schema/vendored/core-2026-07-28/schema.ts`), and
    /// the official reference client's `LoggingMessageNotificationParamsSchema`
    /// spells it `z.unknown()` — which is NON-OPTIONAL under the bundled zod v4.
    /// A frame without `data` therefore fails to parse and is dropped on the
    /// floor: measured, at the pinned suite, as
    /// `invalid_type at params.data: expected nonoptional, received undefined`.
    ///
    /// So when the caller used [`log`](Self::log) and supplied no data, `data` is
    /// set to the message string. `message` still rides alongside, as a pmcp
    /// extension the schema permits (it does not close `additionalProperties`)
    /// and the reference client tolerates (it strips unknown members rather than
    /// rejecting them). When the caller used
    /// [`log_with_data`](Self::log_with_data), their value is passed through
    /// VERBATIM and is never overwritten by the message.
    ///
    /// Do not "simplify" the default away: without it,
    /// `2025-11-25:tools-call-with-logging` scores 0/2 and the suite's
    /// `WireSchemaValid` check reports
    /// `LoggingMessageNotification/params: must have required property 'data'`.
    ///
    /// Both early returns below stay AHEAD of this: a record below the effective
    /// level, or one with no sink attached, still constructs no payload at all.
    fn emit_log_record(
        &self,
        level: crate::types::LoggingLevel,
        message: String,
        data: Option<serde_json::Value>,
    ) {
        let effective = self.log_level.unwrap_or(DEFAULT_LOG_LEVEL);
        if level < effective {
            // Below the bar: return before constructing anything at all.
            return;
        }

        let Some(sink) = self.log_sink.as_ref() else {
            // D-08: no sink is silence, not an error.
            return;
        };

        // `data` is REQUIRED by the wire schema, so a caller who supplied none
        // gets the message string. See this function's rustdoc for the
        // measurement; the short version is that the official reference client
        // drops a frame without `data` on the floor.
        //
        // The `clone` is in the `None` arm ONLY and is not a borrow-checker
        // concession: the frame genuinely carries the text twice, under `data`
        // and under `message`, so exactly one extra allocation is the floor. The
        // `Some` arm moves the caller's value and clones nothing.
        let data = data.unwrap_or_else(|| serde_json::Value::String(message.clone()));

        // `logger` is deliberately left `None`. Synthesising one from the tool
        // name would be a guess, and a guessed logger category is worse than an
        // absent one because it looks authoritative.
        let params = crate::types::LogMessageParams::new(level, message).with_data(data);

        sink(Notification::Server(
            crate::types::ServerNotification::LogMessage(params),
        ));
    }

    /// Report count-based progress if available.
    pub async fn report_count(
        &self,
        current: usize,
        total: usize,
        message: Option<String>,
    ) -> crate::Result<()> {
        if let Some(rep) = &self.progress_reporter {
            rep.report_count(current, total, message).await
        } else {
            Ok(())
        }
    }
}

impl Default for RequestHandlerExtra {
    /// Create a default `RequestHandlerExtra` for testing and simple tool invocations.
    ///
    /// Uses a generated UUID as `request_id` and an uncancellable (never-cancelled)
    /// `CancellationToken`. Not suitable for production use where cancellation
    /// tracking is needed.
    fn default() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: None,
            auth_info: None,
            auth_context: None,
            metadata: HashMap::new(),
            progress_reporter: None,
            log_sink: None,
            log_level: None,
            task_request: None,
            request_meta: None,
            protocol_context: None,
            extensions: http::Extensions::new(),
            #[cfg(not(target_arch = "wasm32"))]
            peer: None,
            #[cfg(not(target_arch = "wasm32"))]
            result_meta: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

/// Drain handle for the handler-authored result-`_meta` slot.
///
/// Cloned cheaply out of [`RequestHandlerExtra::result_meta_handle`] BEFORE the
/// `RequestHandlerExtra` is moved into
/// [`ToolHandler::handle_output`](crate::server::ToolHandler::handle_output),
/// then drained by the dispatcher after the handler returns. Encapsulates all
/// lock access so the dispatchers never touch `Mutex` internals directly.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub(crate) struct ResultMetaHandle(
    Arc<std::sync::Mutex<Option<serde_json::Map<String, serde_json::Value>>>>,
);

#[cfg(not(target_arch = "wasm32"))]
impl ResultMetaHandle {
    /// Take the accumulated handler-set `_meta`, leaving the slot empty.
    ///
    /// Returns `None` when the handler never called
    /// [`RequestHandlerExtra::set_result_meta`] — so a handler that does not opt
    /// in injects no `_meta` (no regression). Recovers from a poisoned lock
    /// rather than panicking (T-104-04-03): the slot holds handler-owned data at
    /// the same trust level as the returned value.
    pub(crate) fn take_result_meta(&self) -> Option<serde_json::Map<String, serde_json::Value>> {
        let mut guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take()
    }
}

/// Merge handler-authored `_meta` (drained via
/// [`ResultMetaHandle::take_result_meta`]) onto a dispatch-built
/// [`CallToolResult`](crate::types::CallToolResult) with **handler-key-wins**
/// precedence: a handler key overwrites the same-name existing key, while all
/// unrelated (widget/native) keys are preserved. Never a whole-map replace.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn merge_result_meta(
    result: &mut crate::types::CallToolResult,
    meta: serde_json::Map<String, serde_json::Value>,
) {
    if meta.is_empty() {
        return;
    }
    #[allow(clippy::used_underscore_binding)]
    let slot = result._meta.get_or_insert_with(serde_json::Map::new);
    for (key, value) in meta {
        slot.insert(key, value);
    }
}

impl std::fmt::Debug for RequestHandlerExtra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // List of sensitive metadata keys that should be redacted
        const SENSITIVE_KEYS: &[&str] = &[
            "oauth_token",
            "access_token",
            "refresh_token",
            "api_key",
            "secret",
            "password",
            "bearer_token",
            "auth_token",
        ];

        // Create a redacted version of metadata
        let redacted_metadata: HashMap<String, String> = self
            .metadata
            .iter()
            .map(|(k, v)| {
                let is_sensitive = SENSITIVE_KEYS
                    .iter()
                    .any(|sensitive| k.to_lowercase().contains(sensitive));
                if is_sensitive {
                    (k.clone(), "[REDACTED]".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();

        let mut debug = f.debug_struct("RequestHandlerExtra");
        debug
            .field("cancellation_token", &self.cancellation_token)
            .field("request_id", &self.request_id)
            .field("session_id", &self.session_id)
            .field("auth_info", &self.auth_info)
            .field("auth_context", &self.auth_context)
            .field("metadata", &redacted_metadata)
            .field("task_request", &self.task_request.is_some())
            .field("request_meta", &self.request_meta)
            .field("protocol_context", &self.protocol_context)
            // Presence only — a sink is a closure with no useful Debug, and
            // whether one is attached is exactly the fact a developer chasing
            // "my logs went nowhere" needs (see `RequestHandlerExtra::log`).
            .field("log_sink", &self.log_sink.is_some())
            .field("log_level", &self.log_level)
            .field("extensions", &self.extensions);
        #[cfg(not(target_arch = "wasm32"))]
        debug.field("peer", &self.peer.as_ref().map(|_| "Arc<dyn PeerHandle>"));
        debug.finish()
    }
}

impl CancellationManager {
    /// Cancel a request silently (no notification sent to the client).
    pub async fn cancel_request_silent(&self, request_id: String) -> Result<()> {
        let token = {
            let mut tokens = self.tokens.write().await;
            tokens.remove(&request_id)
        };
        if let Some(token) = token {
            token.cancel();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_cancel_token() {
        let manager = CancellationManager::new();

        // Create a token
        let token = manager.create_token("test-request".to_string()).await;
        assert!(!token.is_cancelled());

        // Cancel the request
        manager
            .cancel_request("test-request".to_string(), None)
            .await
            .unwrap();

        // Token should be cancelled
        assert!(token.is_cancelled());

        // Token should be removed from manager
        assert!(manager.get_token("test-request").await.is_none());
    }

    #[tokio::test]
    async fn test_cancel_with_reason() {
        let manager = CancellationManager::new();

        // Set up notification tracking
        let notifications = Arc::new(RwLock::new(Vec::new()));
        let notifications_clone = notifications.clone();

        let mut manager = manager;
        manager.set_notification_sender(Arc::new(move |notif| {
            let notifications = notifications_clone.clone();
            tokio::spawn(async move {
                notifications.write().await.push(notif);
            });
        }));

        // Create and cancel with reason
        let _token = manager.create_token("test-request".to_string()).await;
        manager
            .cancel_request("test-request".to_string(), Some("Test reason".to_string()))
            .await
            .unwrap();

        // Give notification time to be sent
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Check notification was sent
        let notifs = notifications.read().await;
        assert_eq!(notifs.len(), 1);

        if let Notification::Client(crate::types::ClientNotification::Cancelled(cancelled)) =
            &notifs[0]
        {
            assert_eq!(
                cancelled.request_id,
                crate::types::RequestId::String("test-request".to_string())
            );
            assert_eq!(cancelled.reason, Some("Test reason".to_string()));
        } else {
            panic!("Expected Cancelled notification");
        }
    }

    #[tokio::test]
    async fn test_remove_token() {
        let manager = CancellationManager::new();

        // Create a token
        let token = manager.create_token("test-request".to_string()).await;
        assert!(manager.get_token("test-request").await.is_some());

        // Remove the token
        manager.remove_token("test-request").await;
        assert!(manager.get_token("test-request").await.is_none());

        // Token should still be valid (not cancelled)
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn test_clear_all_tokens() {
        let manager = CancellationManager::new();

        // Create multiple tokens
        let token1 = manager.create_token("request1".to_string()).await;
        let token2 = manager.create_token("request2".to_string()).await;
        let token3 = manager.create_token("request3".to_string()).await;

        // Clear all tokens
        manager.clear().await;

        // All tokens should be cancelled
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
        assert!(token3.is_cancelled());

        // Manager should have no tokens
        assert!(manager.get_token("request1").await.is_none());
        assert!(manager.get_token("request2").await.is_none());
        assert!(manager.get_token("request3").await.is_none());
    }

    #[tokio::test]
    async fn test_request_handler_extra() {
        let token = CancellationToken::new();
        let extra = RequestHandlerExtra::new("test-req".to_string(), token.clone())
            .with_session_id(Some("session-123".to_string()));

        assert_eq!(extra.request_id, "test-req");
        assert_eq!(extra.session_id, Some("session-123".to_string()));
        assert!(!extra.is_cancelled());

        // Cancel the token
        token.cancel();
        assert!(extra.is_cancelled());
    }

    #[tokio::test]
    async fn test_metadata_redaction_in_debug() {
        let token = CancellationToken::new();
        let mut extra = RequestHandlerExtra::new("test-req".to_string(), token);

        // Add sensitive and non-sensitive metadata
        extra.set_metadata("oauth_token".to_string(), "secret-token-123".to_string());
        extra.set_metadata("access_token".to_string(), "bearer-xyz".to_string());
        extra.set_metadata("user_id".to_string(), "user-456".to_string());
        extra.set_metadata("request_count".to_string(), "42".to_string());

        // Get debug representation
        let debug_output = format!("{:?}", extra);

        // Verify sensitive values are redacted
        assert!(
            debug_output.contains("[REDACTED]"),
            "Expected redacted values in: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("secret-token-123"),
            "OAuth token should be redacted: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("bearer-xyz"),
            "Access token should be redacted: {}",
            debug_output
        );

        // Verify non-sensitive values are not redacted
        assert!(
            debug_output.contains("user-456"),
            "Non-sensitive metadata should not be redacted: {}",
            debug_output
        );
        assert!(
            debug_output.contains("42"),
            "Non-sensitive metadata should not be redacted: {}",
            debug_output
        );
    }

    #[tokio::test]
    async fn test_extensions_default_empty() {
        let extra = RequestHandlerExtra::default();
        assert!(extra.extensions().get::<String>().is_none());
    }

    #[tokio::test]
    async fn test_extensions_insert_overwrite_returns_old() {
        let mut extra = RequestHandlerExtra::default();
        assert_eq!(extra.extensions_mut().insert(42u64), None);
        assert_eq!(extra.extensions_mut().insert(99u64), Some(42u64));
        assert_eq!(extra.extensions().get::<u64>(), Some(&99u64));
    }

    #[tokio::test]
    async fn test_protocol_context_era_and_identity_accessors() {
        use crate::types::protocol::{Era, ProtocolContext};
        use crate::types::{Implementation, ProtocolVersion};

        // No context attached => all accessors return None.
        let bare = RequestHandlerExtra::new("req-none".to_string(), CancellationToken::new());
        assert!(bare.era().is_none());
        assert!(bare.protocol_version().is_none());
        assert!(bare.client_info().is_none());
        assert!(bare.client_capabilities().is_none());

        // Attach a v2 ProtocolContext with client identity.
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()))
            .with_client_info(Implementation::new("acme-client", "1.2.3"))
            .with_client_capabilities(crate::types::ClientCapabilities::default());
        let extra = RequestHandlerExtra::new("req-v2".to_string(), CancellationToken::new())
            .with_protocol_context(Some(ctx));

        assert_eq!(extra.era(), Some(Era::V2));
        assert_eq!(
            extra.protocol_version().map(ProtocolVersion::as_str),
            Some("2026-07-28")
        );
        let info = extra.client_info().expect("client_info attached");
        assert_eq!(info.name, "acme-client");
        assert_eq!(info.version, "1.2.3");
        assert!(extra.client_capabilities().is_some());
    }

    #[tokio::test]
    async fn test_trace_context_from_request_meta() {
        use serde_json::json;

        // request_meta carrying full W3C trace values round-trips through
        // trace_context().
        let extra = RequestHandlerExtra::new("req-trace".to_string(), CancellationToken::new())
            .with_request_meta(Some(json!({
                "traceparent": "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
                "tracestate": "rojo=00f067aa0ba902b7",
                "baggage": "userId=alice"
            })));
        let tc = extra.trace_context().expect("traceparent present => Some");
        assert_eq!(
            tc.traceparent,
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
        assert_eq!(tc.tracestate.as_deref(), Some("rojo=00f067aa0ba902b7"));
        assert_eq!(tc.baggage.as_deref(), Some("userId=alice"));

        // Absent request_meta => None.
        let bare = RequestHandlerExtra::new("req-bare".to_string(), CancellationToken::new());
        assert!(bare.trace_context().is_none());

        // request_meta without a traceparent => None.
        let no_tp = RequestHandlerExtra::new("req-no-tp".to_string(), CancellationToken::new())
            .with_request_meta(Some(json!({ "tracestate": "a=1" })));
        assert!(no_tp.trace_context().is_none());
    }

    #[tokio::test]
    async fn test_debug_extensions_prints_type_names_only() {
        let mut extra = RequestHandlerExtra::default();
        extra
            .extensions_mut()
            .insert("SECRET_VALUE_DO_NOT_LEAK".to_string());
        let debug_out = format!("{:?}", extra);
        // http::Extensions Debug prints type names, not field values
        assert!(!debug_out.contains("SECRET_VALUE_DO_NOT_LEAK"));
    }
}

/// Property tests for the `_meta` merge both dispatchers now call on BOTH
/// output paths (D-06, Phase 118.1 plan 09).
///
/// The verbatim `ToolOutput::Result` arm merges handler-authored keys into an
/// envelope the HANDLER built, which can already carry widget, native and
/// related-task keys. A whole-map replace there would silently destroy them, so
/// the merge invariants are stated as properties over arbitrary key sets rather
/// than over the handful of literals the integration fences happen to use
/// (T-118.1-09-04).
#[cfg(all(test, not(target_arch = "wasm32")))]
mod merge_result_meta_properties {
    use super::merge_result_meta;
    use crate::types::CallToolResult;
    use proptest::prelude::*;
    use serde_json::{Map, Value};

    /// Arbitrary `_meta`-shaped map: reverse-DNS-ish keys, scalar values.
    fn meta_map_strategy() -> impl Strategy<Value = Map<String, Value>> {
        proptest::collection::hash_map("[a-z]{1,6}/[a-z]{1,6}", 0u64..1000, 0..8).prop_map(|m| {
            m.into_iter()
                .map(|(k, v)| (k, Value::from(v)))
                .collect::<Map<String, Value>>()
        })
    }

    proptest! {
        /// Handler keys win; every existing key that the handler did NOT name
        /// survives untouched; the result is exactly the union of both key sets.
        #[test]
        fn merge_is_a_union_with_handler_keys_winning(
            existing in meta_map_strategy(),
            handler in meta_map_strategy(),
        ) {
            let mut result = CallToolResult::new(vec![]);
            if !existing.is_empty() {
                result = result.with_meta(existing.clone());
            }

            merge_result_meta(&mut result, handler.clone());

            if existing.is_empty() && handler.is_empty() {
                // Nothing to merge and nothing pre-existing: no `_meta` is
                // fabricated, so a no-opt-in handler stays byte-identical.
                prop_assert!(result._meta.is_none());
                return Ok(());
            }

            let merged = result._meta.as_ref().expect("_meta present after a non-empty merge");

            for (key, value) in &handler {
                prop_assert_eq!(merged.get(key), Some(value), "handler key must win: {}", key);
            }
            for (key, value) in &existing {
                if !handler.contains_key(key) {
                    prop_assert_eq!(
                        merged.get(key),
                        Some(value),
                        "unrelated existing key must survive: {}",
                        key
                    );
                }
            }

            let mut union: std::collections::BTreeSet<&String> = existing.keys().collect();
            union.extend(handler.keys());
            prop_assert_eq!(merged.len(), union.len(), "merge must add no keys of its own");
        }

        /// Merging is idempotent: applying the same handler map twice cannot
        /// drift (repeated drains on a retried dispatch stay safe).
        #[test]
        fn merge_is_idempotent(
            existing in meta_map_strategy(),
            handler in meta_map_strategy(),
        ) {
            let mut once = CallToolResult::new(vec![]);
            let mut twice = CallToolResult::new(vec![]);
            if !existing.is_empty() {
                once = once.with_meta(existing.clone());
                twice = twice.with_meta(existing);
            }

            merge_result_meta(&mut once, handler.clone());
            merge_result_meta(&mut twice, handler.clone());
            merge_result_meta(&mut twice, handler);

            prop_assert_eq!(once._meta, twice._meta);
        }
    }
}
