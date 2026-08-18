//! MCP server implementation.

#[cfg(not(target_arch = "wasm32"))]
use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::TransportMessage;
#[cfg(not(target_arch = "wasm32"))]
use crate::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, GetPromptRequest,
    Implementation, InitializeResult, JSONRPCResponse, ListPromptsRequest, ListPromptsResult,
    ListResourceTemplatesRequest, ListResourceTemplatesResult, ListResourcesRequest,
    ListResourcesResult, ListToolsRequest, ListToolsResult, Notification, ProtocolVersion,
    ReadResourceRequest, Request, RequestId, ServerCapabilities, ServerNotification, ToolInfo,
};
#[cfg(not(target_arch = "wasm32"))]
use async_trait::async_trait;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use crate::runtime::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
// Scrubs the by-value `[u8; 32]` / `Vec<[u8; 32]>` setter parameters on
// `ServerBuilder` after their contents move into the zeroizing fields (D-113-P,
// copy 2 of 3). `zeroize` is only compiled in under `streamable-http`, so the
// import carries the same gate as the fields it serves.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
use zeroize::Zeroize;

// Core modules (currently native-only due to dependencies)
#[cfg(not(target_arch = "wasm32"))]
pub mod adapters;
#[cfg(not(target_arch = "wasm32"))]
pub mod builder;
#[cfg(not(target_arch = "wasm32"))]
// Dead by CONFIGURATION, not disuse: the dispatch paths that call into this
// module are gated behind the transport features, so a `default-features = false`
// build (as `pmcp-tasks` does) and a wasm32 build both compile the module with no
// callers. Scoped so genuine dead code is still caught in a normal build.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod core;
pub mod limits;

// Native-only modules (require tokio, threading, etc.)
#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod batch;
/// Builder-scoped middleware executor for workflow registration.
#[cfg(not(target_arch = "wasm32"))]
pub mod builder_middleware_executor;
#[cfg(not(target_arch = "wasm32"))]
pub mod cancellation;
/// Dynamic resource provider system for pattern-based resource routing.
#[cfg(not(target_arch = "wasm32"))]
pub mod dynamic_resources;
#[cfg(not(target_arch = "wasm32"))]
pub mod http_middleware;
/// Middleware executor abstraction for consistent tool execution.
#[cfg(not(target_arch = "wasm32"))]
pub mod middleware_executor;
// Warn-only emit-time validation of `structuredContent` against a declared
// `outputSchema` (no-op unless the `validation` feature is enabled).
//
// Deliberately NOT gated by target: the module compiles everywhere so dispatcher
// call sites stay plain one-liners. The second `#[cfg]` widens the module's
// visibility for the `fuzzing` feature ONLY, so
// `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` can reach
// `output_validation::fuzz_support` without any item becoming part of the
// shipped public API (`fuzzing` is in neither `default` nor `full`, so
// `cargo public-api` never sees it). This is verbatim the shape
// `server::request_state` and `server::task_dispatch` already use.
#[cfg(not(feature = "fuzzing"))]
pub(crate) mod output_validation;
/// Warn-only emit-time validation of `structuredContent` against a declared
/// `outputSchema` (no-op unless the `validation` feature is enabled).
#[cfg(feature = "fuzzing")]
pub mod output_validation;
/// Concrete `PeerHandle` implementation delegating to the
/// `ServerRequestDispatcher`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod peer_impl;
#[cfg(not(target_arch = "wasm32"))]
pub mod preset;
/// Progress reporting support for long-running operations.
#[cfg(not(target_arch = "wasm32"))]
pub mod progress;
// Server-owned `requestState` AEAD continuation tokens (Phase 113, HTTP-02).
//
// D-14 locks MRTR AEAD to native + `streamable-http`: `ring` is only enabled by
// that feature and the wasm server (`WasmServerCore`) gets no MRTR this phase.
// The second `#[cfg]` widens the module's visibility for the `fuzzing` feature
// ONLY, so `fuzz/fuzz_targets/fuzz_request_state.rs` can reach
// `request_state::fuzz_support` without any item becoming part of the shipped
// public API (`fuzzing` is in neither `default` nor `full`).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[cfg(not(feature = "fuzzing"))]
pub(crate) mod request_state;
/// Server-owned `requestState` AEAD continuation tokens (Phase 113, HTTP-02).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[cfg(feature = "fuzzing")]
pub mod request_state;
/// Outbound server-to-client request dispatcher with response correlation.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod server_request_dispatcher;
/// Simple prompt implementations with metadata support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_prompt;
/// Simple resource implementations with builder pattern support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_resources;
/// Simple tool implementations with schema support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_tool;
// Shared task-lifecycle dispatch unit used by both Server and ServerCore.
//
// The second `#[cfg]` widens the module's visibility for the `fuzzing` feature
// ONLY, so `fuzz/fuzz_targets/fuzz_tasks_update.rs` can reach
// `task_dispatch::fuzz_support` without any item becoming part of the shipped
// public API (`fuzzing` is in neither `default` nor `full`, so `cargo public-api`
// never sees it). This is verbatim the shape `server::request_state` already uses
// for `fuzz_request_state`, so the crate has ONE convention for a fuzz seam
// rather than two.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(feature = "fuzzing"))]
// Dead by CONFIGURATION, not disuse: the dispatch paths that call into this
// module are gated behind the transport features, so a `default-features = false`
// build (as `pmcp-tasks` does) and a wasm32 build both compile the module with no
// callers. Scoped so genuine dead code is still caught in a normal build.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub(crate) mod task_dispatch;
/// Shared task-lifecycle dispatch unit used by both Server and ServerCore.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "fuzzing")]
pub mod task_dispatch;
/// SDK-level task store trait and in-memory implementation.
#[cfg(not(target_arch = "wasm32"))]
pub mod task_store;
/// Task routing trait for MCP Tasks integration.
#[cfg(not(target_arch = "wasm32"))]
pub mod tasks;
/// Tool middleware for cross-cutting concerns in tool execution.
#[cfg(not(target_arch = "wasm32"))]
pub mod tool_middleware;

/// Observability infrastructure for tracing, metrics, and logging.
#[cfg(not(target_arch = "wasm32"))]
pub mod observability;
/// Workflow-based prompt system with type-safe handles and ergonomic builders.
#[cfg(not(target_arch = "wasm32"))]
pub mod workflow;

/// State extractor for `#[mcp_tool]` shared state injection.
#[cfg(not(target_arch = "wasm32"))]
pub mod state;

/// Typed tool implementations with automatic schema generation.
#[cfg(not(target_arch = "wasm32"))]
pub mod typed_tool;

/// Typed prompt implementations with automatic argument schema generation.
#[cfg(not(target_arch = "wasm32"))]
pub mod typed_prompt;

/// UI resource implementations for MCP Apps Extension (SEP-1865).
#[cfg(not(target_arch = "wasm32"))]
pub mod ui;

/// MCP Apps Extension - Interactive UI support for multiple MCP hosts.
///
/// Provides adapters for `ChatGPT` Apps, MCP Apps (SEP-1865), and MCP-UI.
#[cfg(all(not(target_arch = "wasm32"), feature = "mcp-apps"))]
pub mod mcp_apps;

/// Agent Skills (SEP-2640) — [`skills::Skill`] / [`skills::SkillReference`] /
/// [`skills::Skills`] plus a dual-surface `PromptHandler` fallback.
///
/// Gated on `feature = "skills"` AND `not(target_arch = "wasm32")`: the
/// module's contents consume [`ResourceHandler`] and [`PromptHandler`],
/// which are themselves non-wasm-only.
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
pub mod skills;

/// Re-export the public Skills DX types so callers can `use
/// pmcp::server::{Skill, SkillReference, Skills}` without descending
/// into the `skills::` submodule path. The canonical path remains
/// `pmcp::server::skills::*`.
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
pub use skills::{Skill, SkillReference, Skills};

/// Validation helpers for typed tools.
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;

/// Schema utilities for normalizing and inlining JSON schemas.
#[cfg(feature = "schema-generation")]
pub mod schema_utils;

/// Standard error codes for validation with client elicitation support.
#[cfg(not(target_arch = "wasm32"))]
pub mod error_codes;

/// Cross-platform path validation with security constraints.
#[cfg(not(target_arch = "wasm32"))]
pub mod path_validation;

/// WASM-compatible typed tools with automatic schema generation.
#[cfg(target_arch = "wasm32")]
pub mod wasm_typed_tool;

/// wasm32 stand-in for the native cancellation module.
///
/// wasm32 has no cancellation support, so this exposes only a zero-field
/// [`cancellation::RequestHandlerExtra`] so handler signatures stay identical
/// across targets.
#[cfg(target_arch = "wasm32")]
pub mod cancellation {
    /// Stub for WASM - no cancellation support
    #[derive(Debug, Clone, Default)]
    pub struct RequestHandlerExtra;
}
/// Axum Router convenience function for secure MCP server hosting.
#[cfg(feature = "streamable-http")]
pub mod axum_router;
#[cfg(not(target_arch = "wasm32"))]
pub mod dynamic;
#[cfg(not(target_arch = "wasm32"))]
pub mod elicitation;
#[cfg(not(target_arch = "wasm32"))]
pub mod notification_debouncer;
#[cfg(all(not(target_arch = "wasm32"), feature = "resource-watcher"))]
pub mod resource_watcher;
#[cfg(not(target_arch = "wasm32"))]
pub mod roots;
#[cfg(all(not(target_arch = "wasm32"), feature = "streamable-http"))]
pub mod streamable_http_server;
#[cfg(not(target_arch = "wasm32"))]
// Dead by CONFIGURATION, not disuse: the dispatch paths that call into this
// module are gated behind the transport features, so a `default-features = false`
// build (as `pmcp-tasks` does) and a wasm32 build both compile the module with no
// callers. Scoped so genuine dead code is still caught in a normal build.
#[cfg_attr(
    any(target_arch = "wasm32", not(feature = "streamable-http")),
    allow(dead_code)
)]
pub mod subscriptions;
/// Tower middleware layers for MCP HTTP security (DNS rebinding, security headers).
#[cfg(feature = "streamable-http")]
pub mod tower_layers;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;

// WASM-specific modules and types
#[cfg(target_arch = "wasm32")]
pub mod wasi_adapter;
#[cfg(target_arch = "wasm32")]
pub mod wasm_core;
#[cfg(target_arch = "wasm32")]
pub mod wasm_server;
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_server_tests;

// WASM-compatible protocol handler trait
#[cfg(target_arch = "wasm32")]
pub use wasi_protocol::ProtocolHandler;

#[cfg(target_arch = "wasm32")]
mod wasi_protocol {
    use crate::error::Result;
    use crate::types::{JSONRPCResponse, Notification, Request, RequestId};
    use async_trait::async_trait;

    /// Protocol-agnostic request handler trait for WASM.
    ///
    /// This is a simplified version of the ProtocolHandler trait that
    /// doesn't depend on native-only types like handlers and managers.
    #[async_trait(?Send)]
    pub trait ProtocolHandler {
        /// Handle a single request and return a response.
        async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse;

        /// Handle a notification (no response expected).
        async fn handle_notification(&self, notification: Notification) -> Result<()>;
    }
}

#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod core_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod task_dispatch_tests;

/// Output of a [`ToolHandler`] call: either a plain value the server wraps into a
/// `CallToolResult`, or a fully-formed `CallToolResult` the handler owns end-to-end.
///
/// A handler that implements only [`ToolHandler::handle`] (the common case) never
/// constructs this enum — the default [`ToolHandler::handle_output`] wraps the
/// returned value in [`ToolOutput::Payload`], preserving today's behavior exactly.
///
/// This is a *control* enum consumed inside the server dispatch tail, NOT a wire
/// type — it deliberately does not derive `Serialize`/`Deserialize`.
///
/// Marked `#[non_exhaustive]`: additional output modes may be added in a
/// backwards-compatible way, so downstream `match`es must include a wildcard arm.
#[cfg(not(target_arch = "wasm32"))]
#[non_exhaustive]
#[derive(Debug)]
pub enum ToolOutput {
    /// A plain value. The server runs it through the existing tail: response
    /// middleware (redaction/sanitization), the Phase 102 task create-path gate,
    /// and text-wrap / widget enrichment — byte-identical to returning a `Value`
    /// from [`ToolHandler::handle`] today.
    Payload(serde_json::Value),

    /// A fully-formed `CallToolResult` the handler owns.
    ///
    /// # ⚠️ BYPASS WARNING — this variant is sent to the wire VERBATIM
    ///
    /// The contained [`CallToolResult`] is
    /// serialized and returned to the client **exactly as provided**. It
    /// **BYPASSES**:
    /// - **response middleware** — redaction, sanitization, and audit hooks
    ///   (`ToolMiddleware::on_response`) DO NOT run for this variant;
    /// - **text-wrapping** — `content` is not synthesized from a stringified value;
    /// - **widget enrichment** — `structured_content` / `_meta` are not injected.
    ///
    /// The handler is therefore responsible for its OWN redaction and
    /// sanitization of both `content` and `_meta`, at the same trust level as
    /// returning a raw `Value` today. This is a deliberate, user-approved design
    /// choice (D-04a): a handler that needs to own the full envelope (e.g. to set
    /// `_meta[relatedTask]`) opts into owning its security posture too.
    ///
    /// What is **NOT** bypassed: **request** middleware
    /// (`ToolMiddleware::on_request`) still runs before the handler executes, and
    /// handler errors still route through the normal error path. Only the
    /// successful `Result` arm skips response middleware.
    ///
    /// See the phase migration guide for how to move a hand-written handler onto
    /// this variant safely.
    Result(crate::types::CallToolResult),
}

/// Handler for tool execution.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Handle a tool call with the given arguments.
    async fn handle(&self, args: Value, extra: cancellation::RequestHandlerExtra) -> Result<Value>;

    /// Get tool metadata including description and schema.
    /// Returns None to use default empty metadata.
    fn metadata(&self) -> Option<crate::types::ToolInfo> {
        None
    }

    /// Produce the tool's [`ToolOutput`] for a call.
    ///
    /// The DEFAULT delegates to [`ToolHandler::handle`] and wraps the returned
    /// value in [`ToolOutput::Payload`], so existing handlers (which implement
    /// only `handle`) keep their exact current behavior — response middleware,
    /// the create-path gate, and text-wrap all apply unchanged.
    ///
    /// Override this to return [`ToolOutput::Result`] and own the full
    /// `CallToolResult` envelope. Read the [`ToolOutput::Result`] docs FIRST —
    /// that path bypasses response middleware and you own your own redaction.
    async fn handle_output(
        &self,
        args: Value,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<ToolOutput> {
        self.handle(args, extra).await.map(ToolOutput::Payload)
    }
}

/// Handler for prompt generation.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait PromptHandler: Send + Sync {
    /// Generate a prompt with the given arguments.
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::GetPromptResult>;

    /// Get prompt metadata including description and arguments schema.
    /// Returns None to use default empty metadata.
    fn metadata(&self) -> Option<crate::types::PromptInfo> {
        None
    }
}

/// Handler for resource access.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    /// Read a resource at the given URI.
    async fn read(
        &self,
        uri: &str,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::ReadResourceResult>;

    /// List available resources.
    async fn list(
        &self,
        _cursor: Option<String>,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::ListResourcesResult>;
}

/// Handler for message sampling (LLM operations).
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SamplingHandler: Send + Sync {
    /// Create a message using the language model.
    async fn create_message(
        &self,
        params: crate::types::CreateMessageParams,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::CreateMessageResult>;
}

/// MCP server implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::{Server, ServerCapabilities, ToolHandler};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct MyTool;
///
/// #[async_trait]
/// impl ToolHandler for MyTool {
///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
///         Ok(serde_json::json!({"result": "success"}))
///     }
/// }
///
/// # async fn example() -> pmcp::Result<()> {
/// let server = Server::builder()
///     .name("my-server")
///     .version("1.0.0")
///     .tool("my-tool", MyTool)
///     .build()?;
///
/// server.run_stdio().await?;
/// # Ok(())
/// # }
/// ```
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub struct Server {
    info: Implementation,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    tool_infos: HashMap<String, ToolInfo>,
    /// Cached URI-to-tool-meta index for widget resource `_meta` propagation.
    uri_to_tool_meta: HashMap<String, serde_json::Map<String, serde_json::Value>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    resources: Option<Arc<dyn ResourceHandler>>,
    /// Completion provider backing `completion/complete` (Phase 118.1-04,
    /// CONF-05). Mirrors `ServerCore`'s field of the same name so BOTH native
    /// dispatchers consult the same registered seam through the same shared
    /// unit. `None` still answers the spec shape with an empty `values` array.
    completions: Option<Arc<dyn crate::types::completable::CompletionProviderTrait>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,
    initialized: Arc<RwLock<bool>>,
    /// Channel for sending notifications
    notification_tx: Option<mpsc::Sender<Notification>>,
    /// Cancellation manager for request cancellation
    cancellation_manager: cancellation::CancellationManager,
    /// Roots manager for directory/URI registration
    roots_manager: Arc<RwLock<roots::RootsManager>>,
    /// Subscription manager for resource subscriptions (v1 `resources/subscribe`)
    subscription_manager: Arc<RwLock<subscriptions::SubscriptionManager>>,
    /// The v2 `subscriptions/listen` stream registry (Phase 113, HTTP-04).
    ///
    /// Shared by the streamable-HTTP transport (which REGISTERS a stream) and
    /// [`send_notification`](Self::send_notification) (which FANS OUT to it), so
    /// a change notification emitted through the server's real notification path
    /// reaches every live listen stream whose agreed filter covers it. Empty —
    /// and therefore a no-op — on any server that never served a v2 listen
    /// request, which is every v1 server.
    listen_registry: Arc<subscriptions::ListenRegistry>,
    /// Elicitation manager for user input requests
    elicitation_manager: Option<Arc<elicitation::ElicitationManager>>,
    /// Outbound server-to-client request dispatcher with response correlation.
    /// Wired in `Server::run`; `None` outside the run lifecycle.
    #[allow(clippy::struct_field_names)]
    server_request_dispatcher: Option<Arc<server_request_dispatcher::ServerRequestDispatcher>>,
    /// Cached peer handle built alongside the dispatcher so dispatch sites
    /// clone the Arc rather than allocating a new `DispatchPeerHandle` per
    /// request. `None` outside the run lifecycle.
    peer_handle: Option<Arc<dyn crate::shared::peer::PeerHandle>>,
    /// Authentication provider for validating requests
    auth_provider: Option<Arc<dyn auth::AuthProvider>>,
    /// Tool authorizer for fine-grained access control
    tool_authorizer: Option<Arc<dyn auth::ToolAuthorizer>>,
    /// Tool middleware chain for cross-cutting concerns in tool execution
    #[cfg(not(target_arch = "wasm32"))]
    tool_middleware_chain: Arc<RwLock<tool_middleware::ToolMiddlewareChain>>,
    /// HTTP middleware chain for `StreamableHttpServer` (configured via `ServerBuilder`)
    #[cfg(feature = "streamable-http")]
    http_middleware: Option<Arc<http_middleware::ServerHttpMiddlewareChain>>,
    /// Legacy experimental task router backend (fall-through path). Mirrors
    /// `ServerCore`'s field; presence backs the `tasks/*` endpoints over the
    /// router. Both backends feed the shared `task_dispatch` unit.
    #[cfg(not(target_arch = "wasm32"))]
    task_router: Option<Arc<dyn crate::server::tasks::TaskRouter>>,
    /// Standard task store backend (polling path). Mirrors `ServerCore`'s field;
    /// presence flips the `tasks` capability on at `build()` and backs the
    /// `tasks/*` endpoints + create-path via the shared `task_dispatch` unit.
    #[cfg(not(target_arch = "wasm32"))]
    task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,
    /// Per-tool TOUT-02 double-wrap tripwire opt-out set (D-08). A tool named
    /// here has the tripwire suppressed at the Payload wrap site. Threaded from
    /// `ServerBuilder::suppress_double_wrap_check`; `ServerCore` carries an
    /// IDENTICAL set so both dispatchers consult the same suppression rule.
    #[cfg(not(target_arch = "wasm32"))]
    suppress_double_wrap: HashSet<String>,
    /// Configured protocol-version accept-list (Phase 112, VERS-01/02). Mirrors
    /// `ServerCore`'s field so this high-level `Server` dispatch site resolves the
    /// per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// through the SAME shared resolver. Default is v1-only (excludes
    /// `2026-07-28`) — an un-opted-in server is byte-for-byte unchanged.
    supported_protocol_versions: Vec<ProtocolVersion>,
    /// The server-owned `requestState` codec (Phase 113, HTTP-02), resolved
    /// EXACTLY ONCE at [`ServerBuilder::build`] time. `None` for a server that
    /// did not opt into v2 — such a server reads no MRTR env var and pays
    /// nothing (D-04). Deliberately an instance field, never a process-global:
    /// see [`request_state`] for why.
    #[cfg(feature = "streamable-http")]
    request_state_codec: Option<Arc<request_state::RequestStateCodec>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("info", &self.info)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("sampling", &self.sampling.is_some())
            .field("initialized", &self.initialized)
            .finish()
    }
}

// Every accessor in this block is read by the transport dispatch (the v2 envelope
// and the `subscriptions/listen` + `tasks/update` paths). A transport-less build
// compiles the block with no readers, which is a property of the feature set, not
// of the code.
#[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
#[cfg(not(target_arch = "wasm32"))]
impl Server {
    /// The server-owned `requestState` codec, or `None` when this server did not
    /// opt into the v2 (`2026-07-28`) era.
    ///
    /// Resolved once at build time; the production consumers
    /// (`core::mrtr_ingest`'s `verify` and `core::mrtr_egress`'s `mint`) borrow
    /// it from server state rather than reaching for a process-global.
    #[cfg(feature = "streamable-http")]
    pub(crate) fn request_state_codec(&self) -> Option<&request_state::RequestStateCodec> {
        self.request_state_codec.as_deref()
    }

    /// The server's already-computed capabilities.
    ///
    /// A READ-ONLY borrow — the SAME value
    /// [`handle_discover`](Self::handle_discover) projects onto the wire. The
    /// `subscriptions/listen` gate reads it through
    /// [`advertises_subscriptions`](crate::types::subscriptions::advertises_subscriptions)
    /// so the advertisement and the implementation cannot drift (HTTP-04).
    pub(crate) fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// The server's own [`Implementation`] identity.
    ///
    /// Borrowed by the v2 response envelope so a `subscriptions/listen` terminal
    /// result carries the same `io.modelcontextprotocol/serverInfo` every other
    /// v2 result carries.
    pub(crate) fn info(&self) -> &Implementation {
        &self.info
    }

    /// Check if a tool exists
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Check if a prompt exists
    pub fn has_prompt(&self, name: &str) -> bool {
        self.prompts.contains_key(name)
    }

    /// Get a prompt handler by name.
    ///
    /// Returns a borrowed reference to the registered prompt handler `Arc`,
    /// or `None` if no prompt with that name has been registered. Callers
    /// who need ownership can `Arc::clone(...)` the returned reference.
    ///
    /// # Handler-level testing pattern
    ///
    /// This accessor is the public API surface for the handler-level
    /// integration testing pattern documented in the testing chapter of
    /// the PMCP book: build a `Server`, retrieve the handler by name,
    /// invoke `.handle(...).await` directly with a synthetic
    /// `RequestHandlerExtra`. It exercises handler logic in isolation
    /// without spinning up a transport.
    ///
    /// # What this pattern skips
    ///
    /// This pattern exercises handler logic only. The JSONRPC dispatch
    /// path (`Server::handle_request`) is bypassed, so `auth_provider`,
    /// `tool_authorizer`, and `tool_middleware` are **not** invoked. For
    /// full-pipeline tests that exercise the security pipeline, drive a
    /// real transport (stdio or streamable-http) with a `pmcp::Client`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    /// use async_trait::async_trait;
    /// use pmcp::{PromptHandler, Server};
    /// use pmcp::types::{GetPromptResult, PromptMessage, Content};
    /// use pmcp::types::content::Role;
    ///
    /// struct GreetingPrompt;
    ///
    /// #[async_trait]
    /// impl PromptHandler for GreetingPrompt {
    ///     async fn handle(
    ///         &self,
    ///         args: HashMap<String, String>,
    ///         _extra: pmcp::RequestHandlerExtra,
    ///     ) -> pmcp::Result<GetPromptResult> {
    ///         let who = args.get("name").cloned().unwrap_or_else(|| "world".to_string());
    ///         Ok(GetPromptResult::new(
    ///             vec![PromptMessage::new(Role::User, Content::text(format!("Hello, {}!", who)))],
    ///             Some("Greeting prompt".to_string()),
    ///         ))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("demo")
    ///     .version("0.1")
    ///     .prompt_arc("greet", Arc::new(GreetingPrompt))
    ///     .build()?;
    ///
    /// let handler = server.get_prompt("greet").expect("registered above");
    /// let mut args = HashMap::new();
    /// args.insert("name".to_string(), "claude".to_string());
    /// let result = handler
    ///     .handle(args, pmcp::RequestHandlerExtra::default())
    ///     .await?;
    /// assert_eq!(result.messages.len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_prompt(&self, name: &str) -> Option<&Arc<dyn PromptHandler>> {
        self.prompts.get(name)
    }

    /// Get a tool handler by name.
    ///
    /// Returns a borrowed reference to the registered tool handler `Arc`,
    /// or `None` if no tool with that name has been registered. Callers
    /// who need ownership can `Arc::clone(...)` the returned reference.
    ///
    /// # Handler-level testing pattern
    ///
    /// This accessor is the public API surface for the handler-level
    /// integration testing pattern: build a `Server`, retrieve the
    /// handler by name, invoke `.handle(...).await` directly with a
    /// synthetic `RequestHandlerExtra`. It exercises handler logic in
    /// isolation without spinning up a transport, which is the primary
    /// shape downstream toolkit authors use to assert on a built
    /// `pmcp::Server`'s registered handlers.
    ///
    /// # What this pattern skips
    ///
    /// This pattern exercises handler logic only. The JSONRPC dispatch
    /// path (`Server::handle_request`) is bypassed, so `auth_provider`,
    /// `tool_authorizer`, and `tool_middleware` are **not** invoked. For
    /// full-pipeline tests that exercise the security pipeline, drive a
    /// real transport (stdio or streamable-http) with a `pmcp::Client`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use async_trait::async_trait;
    /// use pmcp::{Server, ToolHandler};
    /// use serde_json::Value;
    ///
    /// struct EchoTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for EchoTool {
    ///     async fn handle(
    ///         &self,
    ///         args: Value,
    ///         _extra: pmcp::RequestHandlerExtra,
    ///     ) -> pmcp::Result<Value> {
    ///         Ok(serde_json::json!({ "echoed": args }))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("demo")
    ///     .version("0.1")
    ///     .tool_arc("echo", Arc::new(EchoTool))
    ///     .build()?;
    ///
    /// let handler = server.get_tool("echo").expect("registered above");
    /// let result = handler
    ///     .handle(serde_json::json!({"msg": "hi"}), pmcp::RequestHandlerExtra::default())
    ///     .await?;
    /// assert_eq!(result, serde_json::json!({"echoed": {"msg": "hi"}}));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn ToolHandler>> {
        self.tools.get(name)
    }

    /// Get the HTTP middleware chain configured via `ServerBuilder`.
    ///
    /// Returns the HTTP middleware chain that was set using
    /// `ServerBuilder::with_http_middleware()`. This can be used when
    /// creating a `StreamableHttpServer`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "streamable-http")]
    /// # {
    /// use pmcp::Server;
    /// use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     // ... with_http_middleware() called here
    ///     .build()?;
    ///
    /// let config = StreamableHttpServerConfig {
    ///     http_middleware: server.http_middleware(),
    ///     ..Default::default()
    /// };
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    #[cfg(feature = "streamable-http")]
    pub fn http_middleware(&self) -> Option<Arc<http_middleware::ServerHttpMiddlewareChain>> {
        self.http_middleware.clone()
    }

    /// Get the authentication provider configured via `ServerBuilder`.
    ///
    /// Returns the authentication provider that was set using
    /// `ServerBuilder::auth_provider()`. This can be used by transport
    /// layers to validate incoming requests and extract auth context.
    pub fn get_auth_provider(&self) -> Option<Arc<dyn auth::AuthProvider>> {
        self.auth_provider.clone()
    }

    /// Build tool and resource registries for workflow expansion.
    ///
    /// Creates `HashMap` registries that can be used to build an `ExpansionContext`
    /// for converting workflow prompts to protocol types. The registries are
    /// automatically populated from all registered tools and resources.
    ///
    /// Returns a tuple of (`tools_map`, `resources_map`) that can be used with
    /// `ExpansionContext`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::Server;
    /// use pmcp::server::workflow::{InternalPromptMessage, ToolHandle, PromptContent, conversion::ExpansionContext};
    /// use pmcp::types::Role;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Build registries from registered tools/resources
    /// let (tools, resources) = server.build_expansion_registries();
    ///
    /// // Create expansion context
    /// let ctx = ExpansionContext {
    ///     tools: &tools,
    ///     resources: &resources,
    /// };
    ///
    /// // Use it to convert workflow prompts to protocol types
    /// let msg = InternalPromptMessage::new(
    ///     Role::System,
    ///     PromptContent::ToolHandle(ToolHandle::new("my_tool"))
    /// );
    /// let protocol_msg = msg.to_protocol(&ctx)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_expansion_registries(
        &self,
    ) -> (
        HashMap<Arc<str>, workflow::conversion::ToolInfo>,
        HashMap<Arc<str>, workflow::conversion::ResourceInfo>,
    ) {
        use std::collections::HashMap;

        // Build tools map from registered tool handlers
        let mut tools_map = HashMap::new();
        for (name, handler) in &self.tools {
            if let Some(metadata) = handler.metadata() {
                tools_map.insert(
                    Arc::from(name.as_str()),
                    workflow::conversion::ToolInfo {
                        name: metadata.name,
                        description: metadata.description.unwrap_or_default(),
                        input_schema: metadata.input_schema,
                    },
                );
            }
        }

        // Build resources map (currently empty - resources don't have metadata())
        // This could be enhanced in the future when resources have better metadata
        let resources_map = HashMap::new();

        (tools_map, resources_map)
    }

    /// Send a notification.
    ///
    /// Sends a notification to the connected client. Notifications are one-way
    /// messages that don't expect a response.
    ///
    /// # Arguments
    ///
    /// * `notification` - The server notification to send
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ServerNotification, ProgressNotification, ProgressToken};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Send a progress notification
    /// let progress = ProgressNotification::new(
    ///     ProgressToken::String("task-123".to_string()),
    ///     50.0,
    ///     Some("Processing...".to_string()),
    /// );
    ///
    /// server.send_notification(ServerNotification::Progress(progress)).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_notification(&self, notification: ServerNotification) {
        // HTTP-04: fan out to every live v2 `subscriptions/listen` stream FIRST,
        // then take the existing v1 transport path unchanged. The registry is
        // empty on any server that never served a listen request, so this is a
        // map lookup on a v1 server and no wire byte changes there.
        self.listen_registry.fan_out(&notification);
        if let Some(tx) = &self.notification_tx {
            let _ = tx.send(Notification::Server(notification)).await;
        }
    }

    /// Gracefully close every open `subscriptions/listen` stream (HTTP-04).
    ///
    /// Call this from a shutdown handler: each stream receives its
    /// [`SubscriptionsListenResult`](crate::types::subscriptions::SubscriptionsListenResult)
    /// as the JSON-RPC response and is then ended. This is the ONLY one of the
    /// three closure triggers that can send a terminal result — a client
    /// disconnect cannot (the peer is gone) and the buffer-overflow policy cannot
    /// (the buffer is full); both simply end the stream.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = pmcp::Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // ... on shutdown:
    /// server.close_subscription_streams();
    /// # Ok(())
    /// # }
    /// ```
    pub fn close_subscription_streams(&self) {
        self.listen_registry.close_all();
    }

    /// The v2 `subscriptions/listen` registry this server fans notifications out
    /// to.
    ///
    /// The streamable-HTTP transport clones this `Arc` to register a stream; the
    /// `Arc` is cloned under the server lock and the lock is released
    /// immediately, so a held-open stream never holds the server mutex.
    pub(crate) fn listen_registry(&self) -> &Arc<subscriptions::ListenRegistry> {
        &self.listen_registry
    }

    /// Get client capabilities.
    ///
    /// Returns the capabilities that the client declared during initialization.
    /// This can be used to check if the client supports specific features.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Check client capabilities after initialization
    /// if let Some(capabilities) = server.get_client_capabilities().await {
    ///     if capabilities.sampling.is_some() {
    ///         println!("Client supports LLM sampling requests");
    ///     }
    ///     if capabilities.elicitation.is_some() {
    ///         println!("Client supports user input requests");
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// - `Some(ClientCapabilities)` if the client has been initialized
    /// - `None` if the client hasn't initialized yet
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Check if the server is initialized.
    ///
    /// Returns true if the initialization handshake with a client has completed.
    /// The server must be initialized before it can process most requests.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// if server.is_initialized().await {
    ///     println!("Server is ready to handle requests");
    /// } else {
    ///     println!("Waiting for client initialization");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
    /// Create a new server builder.
    ///
    /// Returns a `ServerBuilder` for configuring and constructing a new MCP server.
    /// The builder pattern allows you to set server information, capabilities,
    /// and register handlers before building the final server instance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct HelloTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for HelloTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(serde_json::json!({"message": "Hello, World!"}))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("greeting-server")
    ///     .version("1.0.0")
    ///     .tool("hello", HelloTool{})
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Run the server with stdio transport.
    ///
    /// Starts the server using stdin/stdout for communication.
    /// This is the standard way to run MCP servers as they communicate
    /// via JSON-RPC over stdio.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct EchoTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for EchoTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(args) // Echo the input
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("echo-server")
    ///     .version("1.0.0")
    ///     .tool("echo", EchoTool{})
    ///     .build()?;
    ///
    /// // This will run indefinitely, handling client requests
    /// server.run_stdio().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security: task isolation
    ///
    /// Stdio (and any transport that does not resolve a per-request
    /// `AuthContext`) carries no authenticated principal, so every `tasks/*`
    /// request on a [`task_store`](ServerBuilder::task_store)-backed server is
    /// owned by the single `"local"` owner — there is NO per-user task isolation.
    /// This is correct for single-user CLI use; for multi-tenant deployments use
    /// an HTTP transport whose auth layer populates the OAuth subject.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stdio transport fails to initialize
    /// - Communication with the client fails
    /// - The server encounters an unrecoverable error
    pub async fn run_stdio(self) -> Result<()> {
        let transport = crate::shared::StdioTransport::new();
        self.run(transport).await
    }

    /// Run the server with a custom transport.
    ///
    /// Starts the server using a custom transport implementation.
    /// This allows for different communication mechanisms beyond stdio,
    /// such as TCP sockets, `WebSockets`, or other protocols.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport implementation to use for communication
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, StdioTransport, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct CalculatorTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for CalculatorTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         let a = args["a"].as_f64().unwrap_or(0.0);
    ///         let b = args["b"].as_f64().unwrap_or(0.0);
    ///         Ok(serde_json::json!({"result": a + b}))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("calculator-server")
    ///     .version("1.0.0")
    ///     .tool("add", CalculatorTool{})
    ///     .build()?;
    ///
    /// let transport = StdioTransport::new();
    /// server.run(transport).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Security: task isolation
    ///
    /// Per-user `tasks/*` isolation requires the transport to resolve a per-request
    /// `AuthContext` carrying the OAuth subject. Transports without one (stdio and
    /// the like) own every task under the single `"local"` owner — no per-user
    /// isolation. Use an authenticating HTTP transport for multi-tenant task servers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transport fails to initialize or operate
    /// - Communication with the client fails
    /// - The server encounters an unrecoverable error
    pub async fn run<T: crate::shared::Transport + 'static>(mut self, transport: T) -> Result<()> {
        let (notification_tx, notification_rx) = mpsc::channel(100);
        self.notification_tx = Some(notification_tx);

        // Hook cancellation manager to send notifications via the same channel,
        // through the SINGLE `notification_tx`-to-sink conversion
        // ([`Server::notification_tx_sink`]). This site used to carry its own
        // `try_send` closure — a third copy of the same three lines, and a third
        // chance to disagree about the send discipline.
        if let Some(sender) = self.notification_tx_sink() {
            self.cancellation_manager.set_notification_sender(sender);
        }

        // Outbound server-to-client request channel + dispatcher. Drain task
        // wraps each `(correlation_id, ServerRequest)` as
        // `TransportMessage::Request` and forwards to the transport;
        // `handle_transport_message` routes responses back through
        // `dispatcher.handle_response`.
        let (outbound_tx, outbound_rx) =
            mpsc::channel::<(String, crate::types::ServerRequest)>(100);
        let dispatcher = Arc::new(
            server_request_dispatcher::ServerRequestDispatcher::new_with_channel(outbound_tx),
        );
        let peer: Arc<dyn crate::shared::peer::PeerHandle> = Arc::new(
            crate::server::peer_impl::DispatchPeerHandle::new(dispatcher.clone()),
        );
        self.peer_handle = Some(peer);
        self.server_request_dispatcher = Some(dispatcher);

        let server = Arc::new(self);

        // Transport Actor design (Phase 108, D-01/D-02): the transport is OWNED
        // by exactly one actor task and is NEVER wrapped in a shared
        // `Arc<RwLock<T>>`. ALL outbound frames (responses, server-requests,
        // notifications) funnel through a single UNBOUNDED `send_tx`; inbound
        // requests go to a SINGLE sequential worker via an UNBOUNDED
        // `request_tx`. The receive/drain path therefore never blocks on request
        // execution, request-queue capacity, or a transport write-lock, so an
        // in-tool `peer.sample()` / `.list_roots()` round-trip cannot deadlock
        // the loop. Request handling stays serialized (one worker) — zero
        // behavior change for existing single-request servers.
        let (send_tx, send_rx) = mpsc::unbounded_channel::<TransportMessage>();
        let (request_tx, request_rx) = mpsc::unbounded_channel::<(RequestId, Request)>();

        Self::spawn_notification_handler(send_tx.clone(), notification_rx);
        server_request_dispatcher::spawn_server_request_drain(send_tx.clone(), outbound_rx);
        Self::spawn_request_worker(server.clone(), request_rx, send_tx.clone());
        let actor = tokio::spawn(Self::run_transport_actor(
            server.clone(),
            transport,
            send_rx,
            request_tx,
        ));

        // Return once the actor task ends (transport closed / all senders gone).
        Self::run_main_loop(actor).await
    }

    /// Attach a peer handle to `extra`, preferring the REQUEST-SCOPED one.
    ///
    /// No-op on wasm32, and — when neither source is present — outside the
    /// `run()` lifecycle, exactly as before.
    ///
    /// # Precedence: the request-scoped transport handle wins (T-118.1-11-04)
    ///
    /// Two sources can supply a peer and they are NOT equivalent:
    ///
    /// 1. `self.peer_handle` — a SINGLE field on `Server`, set once by
    ///    [`Server::run`] for the in-process actor loop. One transport, one
    ///    client, so one handle says everything there is to say.
    /// 2. the [`TransportBackchannel`](crate::types::protocol::context::TransportBackchannel)
    ///    riding THIS request's `ProtocolContext`, attached by the
    ///    `StreamableHTTP` transport at the one site that knows which session the
    ///    request arrived on.
    ///
    /// On a MULTIPLEXED transport (1) cannot express "the session that issued
    /// this request": a handle set there is shared by every concurrent session,
    /// so one client's `sampling/createMessage` would be delivered to whichever
    /// session the global handle happened to be bound to — the T-113-07
    /// misbinding class. (2) is constructed per request and bound to the
    /// originating session, so it is always the more specific answer and is
    /// therefore read FIRST.
    ///
    /// The in-process path is untouched: `Server::run` attaches no backchannel,
    /// so the `self.peer_handle` fallback below is what runs there.
    ///
    /// # Ordering
    ///
    /// Every dispatch site calls this AFTER its `tool_authorizer` check, so an
    /// unauthorized caller returns before a handler body ever runs and therefore
    /// never sees `extra.peer()` — the invariant stated at `src/shared/peer.rs`.
    ///
    /// Delegates to [`crate::server::core::attach_request_peer`], the ONE unit
    /// `ServerCore::attach_peer` also calls — the precedence rule is defined
    /// once and merely invoked here (twin-site parity).
    ///
    /// # It attaches the LOG SINK too (Phase 118.2, CONF-10)
    ///
    /// The name is kept for its call-site history, but this is now the single
    /// post-authorization site where ALL of a request's server-to-client
    /// capability handles are attached: the peer, the log sink, and the resolved
    /// log level. They share one site DELIBERATELY — a second method a future
    /// dispatch site had to remember to call is exactly the drift the shared
    /// units exist to prevent. The `ServerCore` twin does the same, in the same
    /// order, through the same two units.
    #[inline]
    fn attach_peer(
        &self,
        extra: crate::server::cancellation::RequestHandlerExtra,
    ) -> crate::server::cancellation::RequestHandlerExtra {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let extra = crate::server::core::attach_request_peer(extra, self.peer_handle.as_ref());
            // The fallback is passed as a THUNK: `attach_request_log_sink` prefers
            // this request's `TransportBackchannel` sink and never reads it on any
            // HTTP-served request, so building it eagerly allocated one
            // `Arc<dyn Fn(..)>` per dispatch only to drop it.
            crate::server::core::attach_request_log_sink(extra, || self.notification_tx_sink())
        }
        #[cfg(target_arch = "wasm32")]
        {
            extra
        }
    }

    /// The server-wide notification channel expressed as a sink, or `None` when
    /// this server has no channel.
    ///
    /// The ONE place in the crate that turns `self.notification_tx` into an
    /// `Arc<dyn Fn(Notification) + Send + Sync>`. Two consumers read it — the
    /// progress-reporter path via [`Server::progress_notification_sink`] and the
    /// log-sink path via [`Server::attach_peer`] — and a second `try_send`
    /// closure would be a second chance to disagree about the send discipline.
    ///
    /// Both consumers call it only once they have decided they need it: the
    /// `Arc` allocation happens on the branch that uses the value, never
    /// speculatively ahead of the request-scoped sink that outranks it.
    ///
    /// `try_send` and a discarded result, deliberately: this is a bounded
    /// channel, and a full channel must never block or fail a handler. The cost
    /// is that a saturated channel drops records silently, which is why
    /// `RequestHandlerExtra::log`'s `Ok(())` is documented as NOT being delivery
    /// acknowledgement.
    ///
    /// It is `None` on every HTTP-served server, because `StreamableHttpServer`
    /// never calls [`Server::run`] — which is exactly why the request-scoped
    /// `TransportBackchannel` sink has to win over it.
    #[inline]
    fn notification_tx_sink(&self) -> Option<Arc<dyn Fn(Notification) + Send + Sync>> {
        let tx = self.notification_tx.as_ref()?.clone();
        Some(Arc::new(move |notification| {
            let _ = tx.try_send(notification);
        }))
    }

    /// The one-way notification sink this request's progress reporter emits
    /// through, or `None` when the request has no vehicle at all.
    ///
    /// # Precedence mirrors [`Server::attach_peer`], for the same reason
    ///
    /// 1. the `TransportBackchannel`'s `notification_sink` on THIS request's
    ///    `ProtocolContext` — session-bound, supplied by the `StreamableHTTP`
    ///    transport at the one site that knows which session the request arrived
    ///    on;
    /// 2. `self.notification_tx`, the server-wide channel assigned by
    ///    [`Server::run`] and by nothing else.
    ///
    /// The second is `None` on every HTTP-served server, because
    /// `StreamableHttpServer` never calls `Server::run()`. That is precisely why
    /// `extra.report_progress(..)` was silently inert over HTTP before phase
    /// 118.1: `RequestHandlerExtra::report_progress` returns `Ok(())` when the
    /// reporter is `None`, so the gap produced no error anywhere.
    ///
    /// The sink is handed through with NO adapter — the transport chose its type
    /// to be `ServerProgressReporter::new`'s second parameter verbatim.
    #[inline]
    fn progress_notification_sink(
        &self,
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))] protocol_context: Option<
            &crate::types::protocol::ProtocolContext,
        >,
    ) -> Option<Arc<dyn Fn(Notification) + Send + Sync>> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(sink) = protocol_context
            .and_then(crate::types::protocol::ProtocolContext::transport_backchannel)
            .and_then(crate::types::protocol::context::TransportBackchannel::notification_sink)
        {
            return Some(Arc::clone(sink));
        }
        // The single `notification_tx`-to-sink conversion lives in
        // `notification_tx_sink`; the log-sink path reads the same one.
        self.notification_tx_sink()
    }

    /// Build this request's progress reporter, or `None` when the client asked
    /// for no progress or the request carries no notification vehicle.
    ///
    /// The `progress_token` lookup is unchanged: a request with no
    /// `params._meta.progressToken` still gets no reporter, so a handler that
    /// calls `extra.report_progress(..)` anyway stays silent. Only the SENDER
    /// resolution moved — see [`Server::progress_notification_sink`].
    ///
    /// ONE construction site for all three dispatchers (tools, prompts,
    /// resources): they had three byte-identical copies, and a fourth would have
    /// been the one that kept reading `self.notification_tx` alone.
    #[inline]
    fn progress_reporter_for(
        &self,
        meta: Option<&crate::types::protocol::RequestMeta>,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> Option<Arc<dyn crate::server::progress::ProgressReporter>> {
        // DELIBERATE, and NOT to be "unified" with the log path: Phase 118.2
        // D-07 removed the progress-token gate from the LOG sink only. Progress
        // stays opt-in — a client that sent no `progressToken` has nothing to
        // correlate progress notifications with — while `notifications/message`
        // is unconditional. See `core::attach_request_log_sink`.
        let token = meta.and_then(|meta| meta.progress_token.as_ref())?;
        let sink = self.progress_notification_sink(protocol_context)?;
        let reporter = crate::server::progress::ServerProgressReporter::new(token.clone(), sink);
        Some(Arc::new(reporter) as Arc<dyn crate::server::progress::ProgressReporter>)
    }

    /// Spawn the outgoing-notification forwarder.
    ///
    /// Forwards each queued [`Notification`] onto the transport actor's
    /// unbounded `send_tx` as a [`TransportMessage::Notification`]. It no longer
    /// touches the transport directly — all outbound framing is serialized by
    /// the single actor task, so notifications can never starve on a transport
    /// write-lock held across an in-flight `receive()`.
    fn spawn_notification_handler(
        send_tx: mpsc::UnboundedSender<TransportMessage>,
        mut notification_rx: mpsc::Receiver<Notification>,
    ) {
        tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                if send_tx
                    .send(TransportMessage::Notification(notification))
                    .is_err()
                {
                    // Actor gone — nothing left to forward to.
                    break;
                }
            }
        });
    }

    /// Spawn the single sequential request worker.
    ///
    /// Consumes inbound requests one at a time and forwards each response back
    /// through the actor's `send_tx`. Exactly ONE worker runs per server, so
    /// request handling stays serialized exactly as before this refactor. The
    /// actor merely decouples receiving from handling: a handler that awaits a
    /// peer round-trip no longer blocks the receive path, so the correlated
    /// client response can be read and routed while the handler is parked.
    fn spawn_request_worker(
        server: Arc<Self>,
        mut request_rx: mpsc::UnboundedReceiver<(RequestId, Request)>,
        send_tx: mpsc::UnboundedSender<TransportMessage>,
    ) {
        tokio::spawn(async move {
            while let Some((id, request)) = request_rx.recv().await {
                let response = server.handle_request(id, request, None).await;
                if send_tx.send(TransportMessage::Response(response)).is_err() {
                    // Actor gone — stop draining.
                    break;
                }
            }
        });
    }

    /// Own the transport and interleave receive + send from a single task.
    ///
    /// The actor `select!`s over (a) the outbound `send_rx` and (b)
    /// `transport.receive()`. Inbound frames are routed by kind: a `Response`
    /// resolves the awaiting dispatcher IMMEDIATELY (unblocking an in-tool
    /// `peer.sample()`), a `Request` is queued to the sequential worker, and a
    /// `Notification` is handled inline. Routing NEVER touches the transport.
    ///
    /// When the send branch wins, the in-flight `receive()` future is DROPPED,
    /// so [`crate::shared::Transport::receive`] MUST be cancel-safe (see the
    /// trait's `# Cancellation` contract) — the stock `StdioTransport` persists
    /// its partial line across calls for exactly this reason. The real
    /// `transport.send(..)` runs AFTER the `select!` block so the receive
    /// future's mutable borrow is released first; this also keeps the borrow
    /// checker satisfied for the single `&mut self` transport object.
    async fn run_transport_actor<T: crate::shared::Transport + 'static>(
        server: Arc<Self>,
        mut transport: T,
        mut send_rx: mpsc::UnboundedReceiver<TransportMessage>,
        request_tx: mpsc::UnboundedSender<(RequestId, Request)>,
    ) {
        loop {
            let mut outbound: Option<TransportMessage> = None;
            tokio::select! {
                biased;
                maybe_frame = send_rx.recv() => {
                    match maybe_frame {
                        Some(frame) => outbound = Some(frame),
                        // All senders dropped — shut the actor down.
                        None => break,
                    }
                },
                received = transport.receive() => {
                    match received {
                        Ok(message) => {
                            if Self::route_inbound_message(&server, &request_tx, message)
                                .await
                                .is_break()
                            {
                                break;
                            }
                        },
                        Err(e) => {
                            Self::log_error(&format!("Transport receive error: {}", e)).await;
                            break;
                        },
                    }
                },
            }
            if let Some(frame) = outbound {
                if let Err(e) = transport.send(frame).await {
                    Self::log_error(&format!("Transport send error: {}", e)).await;
                    break;
                }
            }
        }
    }

    /// Route one inbound transport frame. Never touches the transport, so it is
    /// safe to call from inside the actor's `select!` receive arm. Returns
    /// [`std::ops::ControlFlow::Break`] only when the request worker is gone.
    async fn route_inbound_message(
        server: &Arc<Self>,
        request_tx: &mpsc::UnboundedSender<(RequestId, Request)>,
        message: TransportMessage,
    ) -> std::ops::ControlFlow<()> {
        match message {
            TransportMessage::Request { id, request } => {
                // Unbounded queue: the receive branch MUST NOT await capacity
                // (the anti-deadlock invariant). A send error means the worker
                // is gone, so the actor should stop.
                if request_tx.send((id, request)).is_err() {
                    return std::ops::ControlFlow::Break(());
                }
            },
            TransportMessage::Response(response) => {
                Self::route_response(server, response).await;
            },
            TransportMessage::Notification(notification) => {
                Self::route_notification(server, notification).await;
            },
        }
        std::ops::ControlFlow::Continue(())
    }

    /// Route a correlated client response through the dispatcher so a pending
    /// in-tool peer dispatch resolves. Behavior is lifted verbatim from the
    /// former `handle_transport_message` response arm — only the caller changed.
    async fn route_response(server: &Arc<Self>, response: JSONRPCResponse) {
        let Some(dispatcher) = &server.server_request_dispatcher else {
            Self::log_warning("Server received response but no dispatcher configured").await;
            return;
        };
        let correlation_id = response.id.to_string();
        let payload = match &response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(value) => value.clone(),
            crate::types::jsonrpc::ResponsePayload::Error(err) => {
                // Represent errors as a JSON object so callers can distinguish —
                // dispatch() returns the Value as-is.
                serde_json::to_value(err).unwrap_or(Value::Null)
            },
        };
        if let Err(e) = dispatcher.handle_response(&correlation_id, payload).await {
            Self::log_warning(&format!(
                "Failed to route response {}: {}",
                correlation_id, e
            ))
            .await;
        }
    }

    /// Handle an inbound notification (client cancellation). A failed
    /// cancellation lookup is logged rather than tearing down the actor loop.
    async fn route_notification(server: &Arc<Self>, notification: Notification) {
        if let Notification::Client(crate::types::ClientNotification::Cancelled(params)) =
            &notification
        {
            let request_id = params.request_id.to_string();
            if let Err(e) = server
                .cancellation_manager
                .cancel_request_silent(request_id)
                .await
            {
                Self::log_warning(&format!("Failed to process cancellation: {}", e)).await;
            }
        }
        Self::log_debug("Server received notification").await;
    }

    /// Log an error message.
    async fn log_error(message: &str) {
        crate::log(crate::types::LogLevel::Error, message, None).await;
    }

    /// Log a warning message.
    async fn log_warning(message: &str) {
        crate::log(crate::types::LogLevel::Warning, message, None).await;
    }

    /// Log a debug message.
    async fn log_debug(message: &str) {
        crate::log(crate::types::LogLevel::Debug, message, None).await;
    }

    /// Await the transport actor task; `run()` returns once it ends (transport
    /// closed or all outbound senders dropped). This is the shutdown join point.
    async fn run_main_loop(actor: tokio::task::JoinHandle<()>) -> Result<()> {
        if let Err(e) = actor.await {
            Self::log_error(&format!("Transport actor task ended abnormally: {}", e)).await;
        }
        Ok(())
    }

    /// Resolve the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// ONCE at this dispatch site's ingress via the SAME shared resolver
    /// `ServerCore` uses — the twin wiring (Pitfall 3). `Ok(None)` for a
    /// non-opted-in server (zero era-detection, D-04).
    ///
    /// `pub(crate)` so the streamable-HTTP layer (Plan 06) can resolve ONCE for
    /// its header gate and thread the SAME value into
    /// [`handle_request_with_context`](Self::handle_request_with_context) — the
    /// HTTP layer CONSUMES the resolved era, it never runs a second resolver
    /// (D-11 / Pitfall 2).
    /// The server's configured protocol-version accept-list.
    ///
    /// `pub(crate)` so the streamable-HTTP layer can put it in an
    /// `UNSUPPORTED_PROTOCOL_VERSION` (-32022) rejection's
    /// `error.data.supported` — the spec requires the rejection to tell the
    /// client which versions it COULD have asked for, so it can pick a mutually
    /// supported one instead of probing.
    pub(crate) fn supported_protocol_versions(&self) -> &[ProtocolVersion] {
        &self.supported_protocol_versions
    }

    pub(crate) fn resolve_ingress_protocol_context(
        &self,
        request: &Request,
    ) -> std::result::Result<
        Option<crate::types::protocol::ProtocolContext>,
        crate::types::protocol::context::ProtocolNegotiationError,
    > {
        crate::server::core::resolve_ingress_protocol_context(
            &self.supported_protocol_versions,
            request,
        )
    }

    /// Resolve the per-request `ProtocolContext` from a request's RAW
    /// `params._meta` value.
    ///
    /// # This is the era resolver the streamable-HTTP transport uses, for EVERY method
    ///
    /// Introduced in Phase 112 for the `server/discover` ingress (which has no
    /// parsed [`Request`] to read a typed field from), and generalized in Phase
    /// 113 plan 04 to every method (finding D-113-B).
    ///
    /// The typed
    /// [`resolve_ingress_protocol_context`](Self::resolve_ingress_protocol_context)
    /// can only see the three request structs that carry a `_meta` FIELD, so a
    /// stateless v2 `tools/list` — which has no handshake and therefore no other
    /// era channel — could not be expressed at all. Widening those `pub` structs
    /// would have been a MAJOR semver break (`cargo semver-checks`
    /// `constructible_struct_adds_field`), and the v2.5 milestone is scoped
    /// additive; reading the raw body needs no public API change and covers every
    /// method, so the HTTP transport routes ALL era detection through here.
    ///
    /// Mirrors the same non-opted-in short-circuit (D-04): a server that has NOT
    /// opted into v2 returns `Ok(None)` WITHOUT inspecting `_meta` at all, so the
    /// v1 request path runs zero era detection.
    pub(crate) fn resolve_raw_meta_protocol_context(
        &self,
        raw_meta: Option<&serde_json::Value>,
    ) -> std::result::Result<
        Option<crate::types::protocol::ProtocolContext>,
        crate::types::protocol::context::ProtocolNegotiationError,
    > {
        if !crate::types::protocol::context::is_v2_opted_in(&self.supported_protocol_versions) {
            return Ok(None);
        }
        crate::types::protocol::context::resolve_protocol_context(
            &self.supported_protocol_versions,
            raw_meta,
        )
    }

    /// Handle the v2 `server/discover` request (Phase 112, VERS-04, D-09/D-10).
    ///
    /// The production discover caller: the streamable-HTTP transport classifies a
    /// `server/discover` POST as `HttpIngress::Discover` and, at the per-path
    /// response-assembly step, calls this THIN delegate. It projects the server's
    /// already-computed capabilities (incl. the `extensions` map) read-only via
    /// the ONE shared [`build_discover_response`](crate::server::core::build_discover_response)
    /// free fn — one projection/one envelope path, no duplicate capability type,
    /// no `is_initialized` mutation. The era gate inside the free fn yields the v2
    /// projection for an `Era::V2` context and `-32601` for v1 / non-opted-in.
    pub(crate) fn handle_discover(
        &self,
        id: RequestId,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        crate::server::core::build_discover_response(
            id,
            // The SINGLE accept-list source (G-7): the same slice
            // `negotiation_error_to_gate_reject` puts in an
            // `UNSUPPORTED_PROTOCOL_VERSION` rejection's `error.data.supported`
            // becomes the result's `supportedVersions`. There is no second list
            // to drift from.
            crate::server::core::DiscoverSource::new(
                &self.capabilities,
                self.supported_protocol_versions(),
            ),
            &self.info,
            protocol_context,
        )
    }

    /// Handle the v2 `tasks/update` request (Phase 114 plan 13, TASK-02).
    ///
    /// The production `tasks/update` caller, and a THIN delegate exactly like
    /// [`handle_discover`](Self::handle_discover) beside it: the streamable-HTTP
    /// transport classifies a `tasks/update` POST as `HttpIngress::TasksUpdate`
    /// and, at the per-path response-assembly step, calls this. It constructs the
    /// SHARED [`TaskDispatch`](crate::server::task_dispatch::TaskDispatch) over
    /// this server's own backends — the same borrow-struct
    /// [`handle_client_request`](Self::handle_client_request) builds for the four
    /// `ClientRequest` tasks methods — and hands off.
    ///
    /// **It defines no gate of its own.** The era gate, the backend gate, the
    /// `-32021` client-declaration gate, the `-32003` identity table, the `-32602`
    /// params check, the four `inputResponses` bounds and the kind-directed decode
    /// all live in
    /// [`TaskDispatch::route_tasks_update`](crate::server::task_dispatch::TaskDispatch::route_tasks_update),
    /// in that order. This function's entire job is to pass the ALREADY-RESOLVED
    /// `auth_context` and `ProtocolContext` through unchanged, which is also what
    /// keeps it from ever re-reading `params._meta` for a second answer.
    ///
    /// `params` are the RAW value the classifier carried. Nothing between the wire
    /// and the router deserializes them, so a malformed body becomes a structured
    /// `-32602` AFTER the gates rather than a parse error before them — and the
    /// `inputResponses` map reaches the route UNDECODED, which is what lets the
    /// route bound it and then type it against the kinds the SERVER recorded
    /// rather than against whichever overlapping shape happened to fit (D-113-O).
    ///
    /// `async` since plan 114-14: the delivery reads the task record and writes
    /// the responses.
    ///
    /// # The v2 result envelope is injected HERE, for the same reason `server/discover`'s is
    ///
    /// `tasks/update` rides the crate-private internal-request route, so it does
    /// NOT pass through `process_client_request`, which is where every
    /// `ClientRequest` result gets its `resultType` + `_meta.serverInfo`. Left
    /// alone, the `UpdateTaskResult` acknowledgement would reach the wire as a
    /// bare `{}` — and the extension says its `resultType` field MUST be
    /// `"complete"`. `build_discover_response` solved the identical problem the
    /// identical way (Phase 112), so the internal route has ONE shape rather than
    /// two.
    ///
    /// [`ReservedFieldOwner::None`](crate::server::core::ReservedFieldOwner) is
    /// named explicitly and is correct: the acknowledgement is EMPTY, so this
    /// route mints no reserved result field at all — no `inputRequests` (that is
    /// `tasks/get`'s, plan 114-11) and no `requestState` (the tasks surface has no
    /// continuation token, D-17). A future change that made this ack non-empty
    /// would have to state its own owner here rather than inherit one.
    ///
    /// The call is a no-op on v1 and for every ERROR payload, so all seven of the
    /// route's refusals are byte-unchanged by it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) async fn handle_tasks_update(
        &self,
        id: RequestId,
        params: &serde_json::Value,
        auth_context: Option<&auth::AuthContext>,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        let mut response = self
            .task_dispatch()
            .route_tasks_update(id, params, auth_context, protocol_context)
            .await;
        crate::server::core::inject_v2_result_envelope(
            &mut response,
            protocol_context,
            &self.info,
            crate::server::core::ResponseDisposition::Complete,
            crate::server::core::ReservedFieldOwner::None,
            // The `tasks/update` acknowledgement is an `UpdateTaskResult`, which
            // does NOT extend `CacheableResult` — the tasks surface carries no
            // caching hint at all in the 2026-07-28 schema (D-07), and the
            // `ttlMs` that DOES live on `TaskV2` is a task LIFETIME, a different
            // concept in a different module (D-10). So this route gains neither
            // key, on either era.
            crate::types::caching::Cacheable::No,
        );
        response
    }

    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<auth::AuthContext>,
    ) -> JSONRPCResponse {
        // Resolve the per-request ProtocolContext ONCE at ingress (opted-in
        // only — D-04), through the single shared resolver, and thread it into
        // dispatch. Never re-derived downstream (D-11).
        let protocol_context = match self.resolve_ingress_protocol_context(&request) {
            Ok(ctx) => ctx,
            Err(negotiation_error) => {
                let (code, message) =
                    crate::server::core::negotiation_error_to_rejection(&negotiation_error);
                return JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    payload: crate::types::jsonrpc::ResponsePayload::Error(
                        crate::types::jsonrpc::JSONRPCError {
                            code,
                            message,
                            data: None,
                        },
                    ),
                };
            },
        };
        self.handle_request_with_context(id, request, auth_context, protocol_context)
            .await
    }

    /// Dispatch a request with an ALREADY-RESOLVED `ProtocolContext` threaded in.
    ///
    /// This is the pass-through seam Plan 06 relies on: the streamable-HTTP layer
    /// resolves the `ProtocolContext` ONCE (via
    /// [`resolve_ingress_protocol_context`](Self::resolve_ingress_protocol_context))
    /// for its header gate, then passes that SAME value here so dispatch does NOT
    /// re-resolve `_meta` — one authoritative era per request (D-11 / Pitfall 2).
    /// [`handle_request`](Self::handle_request) is the thin wrapper that resolves
    /// then calls this.
    pub(crate) async fn handle_request_with_context(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        // MRTR ingress (Plan 113-06, HTTP-03) — the SAME shared helper
        // `ServerCore` calls (twin-site parity; this site never defines its
        // own). Verifies a presented `requestState` against the live principal
        // and originating request, then folds the D-15 verdict into the context
        // threaded into dispatch. Inert on v1 / non-opted-in / non-eligible
        // requests, so the legacy path is byte-for-byte unchanged.
        #[cfg(feature = "streamable-http")]
        let (mrtr, protocol_context) = match crate::server::core::MrtrRound::begin(
            &request,
            protocol_context,
            auth_context.as_ref().map(|ctx| ctx.subject.as_str()),
            self.auth_provider.is_some(),
            self.request_state_codec(),
        ) {
            Ok(resolved) => resolved,
            // The single-source envelope builder, rather than a hand-written
            // `JSONRPCResponse` literal re-spelling `"2.0"` and `data: None`.
            Err((code, message)) => {
                return crate::server::task_dispatch::error_response(id, code, message)
            },
        };

        // Capture the cacheability claim BEFORE the `match` below: arm 1 binds
        // `ref boxed_req` but arm 2 MOVES `boxed_req`, so `request` is gone by
        // the time the injection at the bottom of this function runs. Twin of
        // the `ServerCore` capture — and it CALLS the shared classifier in
        // `core.rs` rather than defining a second table, which is the twin-site
        // parity rule this file follows everywhere else.
        let cacheable = crate::server::core::request_is_cacheable(&request);

        // G-9 / CONF-08 (Phase 118.1-08): fold the v1 `initialize` handshake's
        // advertised capabilities into the context threaded into DISPATCH — the
        // SAME shared unit `ServerCore` calls, never a second copy (twin-site
        // parity). The fold owns the lock, so the guard that keeps v2 traffic
        // off it (T-118.1-08-02) lives there too rather than being re-spelled
        // here; the read guard drops inside, before the `Initialize` arm below
        // takes the WRITE lock a few lines down. The EGRESS keeps the UNFOLDED
        // `protocol_context`: the fold is a handler-visibility concern, not a
        // wire-shape one.
        let dispatch_context = crate::server::core::fold_v1_handshake_capabilities(
            protocol_context.clone(),
            &self.client_capabilities,
            &self.supported_protocol_versions,
        )
        .await;

        // The SECOND envelope claimant (Phase 114 plan 11), twin of the
        // `ServerCore` site: the `tasks/*` routes and the `tools/call` create
        // path state their own `resultType` and reserved-field ownership from the
        // site that writes them. `NONE` for every other dispatch.
        let mut dispatch_claim = crate::server::core::DispatchEnvelopeClaim::NONE;
        let mut response = match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };
                // Store client capabilities
                *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
                *self.initialized.write().await = true;

                let negotiated_version =
                    crate::negotiate_protocol_version(&init_req.protocol_version);

                let result = InitializeResult {
                    protocol_version: ProtocolVersion(negotiated_version.to_string()),
                    // Twin-site parity (114-05, D-02): the SAME shared v1
                    // projection `ServerCore::handle_initialize` uses — this
                    // site never defines its own. Without it the build-time
                    // tasks-extension entry, which is the v2 negotiation home,
                    // leaks onto the v1 `initialize` wire of every tasks server.
                    capabilities: crate::server::core::project_capabilities_for_v1(
                        &self.capabilities,
                    ),
                    server_info: self.info.clone(),
                    instructions: None,
                };
                JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    payload: crate::types::jsonrpc::ResponsePayload::Result(
                        serde_json::to_value(result).unwrap(),
                    ),
                }
            },
            // `Box::pin`: the MRTR ingress/egress locals (Plan 113-06) push this
            // dispatch future past clippy's `large_futures` threshold. Boxing the
            // inner future keeps every CALLER of this method small without
            // changing behavior — the same treatment the two POST entrypoints and
            // the discover assembly already get.
            Request::Client(boxed_req) => {
                Box::pin(self.handle_client_request(
                    id,
                    *boxed_req,
                    auth_context,
                    dispatch_context,
                    &mut dispatch_claim,
                ))
                .await
            },
            Request::Server(_) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::jsonrpc::JSONRPCError {
                        code: crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                        message: "Server requests not supported by server".to_string(),
                        data: None,
                    },
                ),
            },
        };

        // Twin-site MRTR egress (Plan 113-06): the SAME shared helper `ServerCore`
        // calls. Converts a handler's "I need more input" signal into an
        // `input_required` result carrying a freshly minted `requestState`, and
        // STRIPS the pmcp-internal signal key on every other path.
        #[cfg(feature = "streamable-http")]
        let (disposition, reserved_field_owner) = mrtr.finish(
            &mut response,
            protocol_context.as_ref(),
            self.request_state_codec(),
        );
        #[cfg(not(feature = "streamable-http"))]
        let (disposition, reserved_field_owner) = {
            // No `mrtr_egress` on this build — strip the reserved signal key
            // here so it cannot reach the wire (see `core::scrub_mrtr_signal`).
            crate::server::core::scrub_mrtr_signal(&mut response);
            (
                crate::server::core::ResponseDisposition::Complete,
                crate::server::core::ReservedFieldOwner::None,
            )
        };

        // Twin-site v2 envelope injection (VERS-07 / D-07 / D-08) plus the
        // caching-hint projection (SCHM-03): the ONE shared helper in `core.rs`
        // — the envelope half is v2-only, object-results-only, collision-safe,
        // so v1 / non-opted-in responses stay byte-identical; the caching half
        // runs on both eras, ensuring on v2 and STRIPPING on v1 (D-11). The
        // reserved-field owner comes from the egress that minted the fields,
        // never from the disposition (Phase 114 plan 10) — folded with the
        // dispatch's own claim through the SAME named rule `ServerCore` uses
        // (Phase 114 plan 11).
        let claim = dispatch_claim.or_egress(disposition, reserved_field_owner);
        crate::server::core::inject_v2_result_envelope(
            &mut response,
            protocol_context.as_ref(),
            &self.info,
            claim.disposition,
            claim.owner,
            cacheable,
        );
        response
    }

    async fn handle_client_request(
        &self,
        id: RequestId,
        request: ClientRequest,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
        dispatch_claim: &mut crate::server::core::DispatchEnvelopeClaim,
    ) -> JSONRPCResponse {
        // ADAPTER (a) — tasks/* dispatch at the post-auth assembly layer.
        //
        // The four `tasks/*` variants are served by the SHARED `task_dispatch`
        // unit (the SAME `route_tasks_endpoint` `ServerCore` uses), returning a
        // full `JSONRPCResponse` DIRECTLY — no `JSONRPCResponse -> Result<Value>`
        // round-trip and no double-wrap, so the FROZEN `-32002` pending code
        // survives unchanged (T-102-07).
        //
        // SECURITY (T-102-04): this interception sits DOWNSTREAM of auth
        // resolution — `auth_context` is the already-resolved context the
        // transport layer passed into `handle_request`, threaded here unchanged.
        // The tasks/* path is therefore subject to the SAME auth as every other
        // request; owner-scoping inside `route_tasks_endpoint` derives the owner
        // from this `AuthContext` ONLY (never client params), enforcing
        // cross-owner isolation (T-102-05).
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            request,
            ClientRequest::TasksGet(_)
                | ClientRequest::TasksResult(_)
                | ClientRequest::TasksList(_)
                | ClientRequest::TasksCancel(_)
        ) {
            let (response, claim) = self
                .task_dispatch()
                .route_tasks_endpoint(
                    id,
                    &request,
                    auth_context.as_ref(),
                    // The context resolved ONCE at transport ingress, CONSUMED
                    // here. Its `era` is read by the `tasks/result` pending
                    // refusal, so that a v2 request cannot elicit the
                    // spec-prohibited `-32002` (Finding 11;
                    // `task_dispatch::is_v1_task_era`), and by the two v2
                    // retirement gates for `tasks/list` / `tasks/result`
                    // (TASK-03; `task_dispatch::tasks_list_serves_on_era`); its
                    // `client_capabilities` are read by the v2
                    // extension-declaration gate (TASK-05). Passing the whole
                    // context is what keeps this dispatcher from ever re-reading
                    // `params._meta` for a second answer. Every gate lives in
                    // `task_dispatch`, never here.
                    protocol_context.as_ref(),
                )
                .await;
            // The claim travels WITH the response: a v2 `tasks/get` on an
            // `input_required` task owns the top-level `inputRequests` the
            // reserved-field registry would otherwise strip (114-10 row 23).
            *dispatch_claim = claim;
            return response;
        }

        // ADAPTER (b) — `logging/setLevel`, era-branched (Phase 118.2-08, D-13).
        //
        // Intercepted HERE, above `process_client_request`, for the same
        // structural reason ADAPTER (a) above intercepts `tasks/*`: this
        // method's v2 answer is a JSON-RPC ERROR with a SPECIFIC code, and
        // `Self::create_response` flattens EVERY `Err` returned by
        // `process_client_request` to `-32603 INTERNAL_ERROR`. A `-32601`
        // therefore cannot travel through that function's
        // `Result<serde_json::Value>` return type at all — the same
        // `JSONRPCResponse -> Result<Value>` round-trip the `tasks/*` adapter
        // exists to avoid. Making `create_response` code-aware instead would
        // silently change the wire code of every other handler that returns a
        // `Error::Protocol`, which is not this plan's change to make.
        //
        // The ANSWER itself is not computed here: it comes from the single
        // shared unit in `server/core.rs`, the very same one `ServerCore`'s
        // dispatch arm calls. One era branch, two roots — that is D-13.
        if matches!(request, ClientRequest::SetLoggingLevel { .. }) {
            return crate::server::core::set_logging_level_response(
                id,
                protocol_context.as_ref().map(|ctx| ctx.era),
            );
        }

        let result = self
            .process_client_request(
                id.clone(),
                request,
                auth_context,
                protocol_context,
                dispatch_claim,
            )
            .await;
        Self::create_response(id, result)
    }

    /// Process a client request and return the result.
    ///
    /// `dispatch_claim` is the out-param the `tools/call` create path writes its
    /// v2 envelope claim into (Phase 114 plan 11); every other arm leaves it as
    /// the caller set it.
    async fn process_client_request(
        &self,
        request_id: RequestId,
        request: ClientRequest,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
        dispatch_claim: &mut crate::server::core::DispatchEnvelopeClaim,
    ) -> Result<serde_json::Value> {
        match request {
            ClientRequest::Initialize(_) => {
                // Already handled above
                unreachable!("Initialize should be handled separately")
            },
            ClientRequest::ListTools(req) => self.handle_list_tools(req),
            ClientRequest::CallTool(req) => {
                self.handle_call_tool(
                    request_id,
                    req,
                    auth_context,
                    protocol_context,
                    dispatch_claim,
                )
                .await
            },
            ClientRequest::ListPrompts(req) => self.handle_list_prompts(req),
            ClientRequest::GetPrompt(req) => {
                self.handle_get_prompt(request_id, req, auth_context, protocol_context)
                    .await
            },
            ClientRequest::ListResources(req) => {
                self.handle_list_resources(request_id, req, auth_context, protocol_context)
                    .await
            },
            ClientRequest::ReadResource(req) => {
                self.handle_read_resource(request_id, req, auth_context, protocol_context)
                    .await
            },
            ClientRequest::ListResourceTemplates(req) => {
                Self::handle_list_resource_templates(self, req)
            },
            // `completion/complete` (Phase 118.1-04, CONF-05 / G-4) — its OWN
            // arm, no longer inside the catch-all below. The shared unit is
            // DEFINED in `server/core.rs` and merely CALLED here, per the
            // twin-site parity rule stated at `src/server/core.rs`'s MRTR
            // section: `mod.rs` calls these helpers, it never defines its own.
            ClientRequest::Complete(req) => {
                crate::server::core::complete_completion(self.completions.as_ref(), &req)
                    .await
                    .and_then(|result| serde_json::to_value(result).map_err(Into::into))
            },
            // `logging/setLevel` (Phase 118.2-08, CONF-10 / D-13) — its OWN
            // arm, no longer inside the residual below, and answering from the
            // SAME shared unit in `server/core.rs` that `ServerCore`'s dispatch
            // arm calls.
            //
            // Reached only when a caller drives this function DIRECTLY. On the
            // production path `handle_client_request`'s ADAPTER (b) has already
            // answered — it must, because the v2 half of the era branch is a
            // `-32601` and `Self::create_response` flattens every `Err` from
            // this function to `-32603`. The v1 half is the whole of what this
            // arm can express, and it is spelled by calling the shared unit
            // rather than by re-typing `json!({})`, so a future change to the
            // measured shape (Pitfall 8) lands in exactly one place.
            ClientRequest::SetLoggingLevel { level: _ } => {
                Ok(crate::server::core::set_logging_level_v1_result())
            },
            // RESIDUAL, recorded rather than silently unified (Phase 118.1-04,
            // RESEARCH Open Question 4): these THREE methods STILL diverge
            // between the two native dispatchers. Here they answer
            // `json!({})`; `ServerCore`'s `_ =>` arm
            // (`src/server/core.rs`, the arm immediately after the
            // `ClientRequest::SetLoggingLevel` one) answers `-32601 Method not
            // supported`. Only this dispatcher is on the HTTP path, so only
            // this side is measured by the official conformance suite. G-5
            // (`resources/subscribe`, `resources/unsubscribe`,
            // `logging/setLevel`, `ping` retirement on v2) is the requirement
            // that owns them; unifying them here would smuggle a behaviour
            // change in behind a conformance fix.
            //
            // HISTORY — a residual is recorded, never silently unified, and
            // never silently SHRUNK either: `logging/setLevel` was the FOURTH
            // method on this arm and left it in Phase 118.2-08 under D-13,
            // because the official suite measures that method and the two roots
            // disagreed about it. `ping` in particular must stay here: it
            // already carries a recorded 118.1 v2 behaviour change (HTTP 404 /
            // `-32601` at the transport gate) and a second, differently-shaped
            // retirement at this layer would be a new divergence, not a fix.
            ClientRequest::Subscribe(_) | ClientRequest::Unsubscribe(_) | ClientRequest::Ping => {
                Ok(serde_json::json!({}))
            },
            ClientRequest::CreateMessage(req) => {
                self.handle_create_message(request_id, *req, protocol_context)
                    .await
            },
            // Note: Elicitation responses are now handled as the response to
            // ServerRequest::ElicitationCreate in the JSON-RPC response flow,
            // not as a separate client request variant.
            // Task requests (experimental MCP Tasks). On non-wasm these are
            // intercepted UPSTREAM in `handle_client_request` (adapter (a)) and
            // served by the shared `task_dispatch` unit, so they never reach
            // here. This arm is the wasm32 fall-through (the task lifecycle is
            // non-wasm-gated): the endpoints are genuinely unsupported there.
            ClientRequest::TasksGet(_)
            | ClientRequest::TasksResult(_)
            | ClientRequest::TasksList(_)
            | ClientRequest::TasksCancel(_) => Err(crate::Error::protocol(
                crate::ErrorCode::METHOD_NOT_FOUND,
                "Tasks not supported on this build",
            )),
        }
    }

    /// Borrow this server's task backends as a [`TaskDispatch`] for the shared
    /// task-lifecycle unit. Single construction point so the `tasks/*` and
    /// create-path call sites don't re-inline the struct literal.
    #[cfg(not(target_arch = "wasm32"))]
    fn task_dispatch(&self) -> crate::server::task_dispatch::TaskDispatch<'_> {
        crate::server::task_dispatch::TaskDispatch {
            task_store: &self.task_store,
            task_router: &self.task_router,
            // The EXISTING public accessor, not a new field and not a widened
            // one — the same read `listen_server_view` makes for
            // `subscriptions/listen` (D-113-N), now feeding the SAME identity
            // table for `tasks/*` (TASK-05).
            has_auth_provider: self.get_auth_provider().is_some(),
        }
    }

    /// Create a JSON-RPC response from a result.
    fn create_response(id: RequestId, result: Result<serde_json::Value>) -> JSONRPCResponse {
        match result {
            Ok(value) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Result(value),
            },
            Err(e) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::jsonrpc::JSONRPCError {
                        code: crate::types::protocol::error_codes::INTERNAL_ERROR,
                        message: e.to_string(),
                        data: None,
                    },
                ),
            },
        }
    }

    fn handle_list_tools(&self, _req: ListToolsRequest) -> Result<Value> {
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        Ok(serde_json::to_value(ListToolsResult {
            tools,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })?)
    }

    async fn handle_call_tool(
        &self,
        request_id: RequestId,
        req: CallToolRequest,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
        dispatch_claim: &mut crate::server::core::DispatchEnvelopeClaim,
    ) -> Result<Value> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Tool '{}' not found", req.name)))?;

        // Capture the create-path inputs BEFORE `req` is partially moved
        // (arguments are consumed by the middleware/handler below). The
        // create-path gate reads:
        //   - the ERA's create trigger — `req.task` on v1, the client's
        //     per-request tasks-extension declaration on v2 (plan 114-12), and
        //   - the tool's declared `TaskSupport` (from the cached `tool_infos`).
        // The SHARED `maybe_build_task_created` enforces the FULL gate
        // internally — we pass these RAW facts, never a pre-filtered precondition.
        // `CreateTrigger::resolve` is the ONE place the era picks a trigger, so
        // this dispatcher cannot implement a trigger `ServerCore` misses.
        #[cfg(not(target_arch = "wasm32"))]
        let create_trigger = crate::server::task_dispatch::CreateTrigger::resolve(
            protocol_context.as_ref().map(|ctx| ctx.era),
            req.task.is_some(),
            protocol_context.as_ref(),
        );
        #[cfg(not(target_arch = "wasm32"))]
        let tool_task_support = self
            .tool_infos
            .get(&req.name)
            .and_then(|info| info.execution.as_ref())
            .and_then(|exec| exec.task_support);
        #[cfg(not(target_arch = "wasm32"))]
        let create_path_id = request_id.clone();

        let request_id_str = request_id.to_string();
        let cancellation_token = self
            .cancellation_manager
            .create_token(request_id_str.clone())
            .await;

        // Auth context now comes from the transport layer
        // Validate authentication if auth provider is configured
        let validated_auth_context = if let Some(auth_provider) = &self.auth_provider {
            // If auth_context was provided by transport, use it; otherwise validate
            if auth_context.is_some() {
                auth_context
            } else {
                // Fallback: try to validate without headers (for backward compatibility)
                auth_provider.validate_request(None).await?
            }
        } else {
            auth_context // No auth provider, just use what was provided
        };

        // Check tool authorization if tool authorizer is configured
        if let (Some(auth_ctx), Some(authorizer)) = (&validated_auth_context, &self.tool_authorizer)
        {
            if !authorizer.can_access_tool(auth_ctx, &req.name).await? {
                return Err(Error::protocol(
                    crate::error::ErrorCode::AUTHENTICATION_REQUIRED,
                    format!("Access denied for tool '{}'", req.name),
                ));
            }
        }

        // The request-scoped progress reporter — the channel
        // `extra.report_progress(..)` actually reads. Resolved BEFORE
        // `protocol_context` is moved into `extra` below, because the transport's
        // session-bound sink rides on it (Phase 118.1 plan 11).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter =
            self.progress_reporter_for(req._meta.as_ref(), protocol_context.as_ref());

        // Clone the validated auth context for the create-path owner resolution
        // (the original is moved into `extra` below). This guarantees the
        // create-path scopes the minted task to the SAME owner the tool ran as.
        #[cfg(not(target_arch = "wasm32"))]
        let create_path_auth = validated_auth_context.clone();

        // Capture the ALREADY-RESOLVED era before `protocol_context` is moved into
        // `extra` below, so the create-path owner binding reads the SAME ingress
        // value the handler does and never re-parses `params._meta` (Phase 112).
        // The twin of the `ServerCore` capture on its own `CallTool` arm.
        #[cfg(not(target_arch = "wasm32"))]
        let create_path_era = protocol_context.as_ref().map(|ctx| ctx.era);

        // Same capture-before-move reason: the emit-time outputSchema validator
        // is era-branched (Phase 115 D-01) and `protocol_context` is moved into
        // `extra` below. UN-cfg'd — unlike `create_path_era` — because the
        // validation call site compiles on wasm32 too. The twin of the
        // `ServerCore` capture on its own `CallTool` arm.
        let validation_era = protocol_context.as_ref().map(|ctx| ctx.era);

        // Propagate the request's `_meta` object (raw JSON incl. namespaced
        // `other` keys) so handlers can read it via `extra.request_meta` in the
        // high-level `Server` path too (ServerCore already wires this at core.rs).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let request_meta_value = crate::server::core::request_meta_to_value(req._meta.as_ref());

        let mut extra = self.attach_peer(
            crate::server::cancellation::RequestHandlerExtra::new(
                request_id.to_string(),
                cancellation_token,
            )
            .with_auth_context(validated_auth_context)
            .with_progress_reporter(progress_reporter)
            // Surface whether the client requested task augmentation so handlers
            // can branch on `extra.is_task_request()` in the high-level `Server`
            // path too (ServerCore already wires this at core.rs). Additive: the
            // dispatcher's own task-creation decision still reads `req.task`.
            .with_task_request(req.task.clone())
            .with_request_meta(request_meta_value)
            // Thread the once-at-ingress resolved protocol context (Phase 112) —
            // the twin of the ServerCore wiring so handlers read the SAME
            // era/identity on both dispatch sites.
            .with_protocol_context(protocol_context),
        );

        // D-03.3 (TOUT-01): clone the interior-mutable result-`_meta` slot BEFORE
        // `extra` is moved into `handle_output`, so any `extra.set_result_meta(..)`
        // the handler performs can be drained back onto the Payload-path result.
        #[cfg(not(target_arch = "wasm32"))]
        let result_meta_handle = extra.result_meta_handle();

        // Execute tool with middleware (native-only)
        #[cfg(not(target_arch = "wasm32"))]
        let dispatch_output = {
            // Create tool context for middleware
            let context = tool_middleware::ToolContext::new(&req.name, &request_id_str);

            // Clone arguments for middleware processing
            let mut args = req.arguments;

            // Process request through tool middleware chain.
            // Middleware rejection short-circuits tool execution. REQUEST
            // middleware runs BEFORE the handler for EVERY tool, regardless of the
            // ToolOutput variant it will return (kept here so it fires on both the
            // Payload and the verbatim Result path).
            self.tool_middleware_chain
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Execute the tool. `handle_output` returns `Result<ToolOutput>`; the
            // SHARED `resolve_tool_output` (D-05) is the SINGLE place that decides
            // Payload-vs-Result and encodes the response-middleware-bypass rule, so
            // this dispatcher and `ServerCore` can never drift on it.
            let output = handler.handle_output(args, extra).await;
            let mut resolved = task_dispatch::resolve_tool_output(output);

            // Why: `ToolOutput::Result` deliberately BYPASSES RESPONSE middleware
            // (D-04 + D-04a — USER-APPROVED and LOCKED: "keep the bypass, harden
            // it"). The handler owns the full envelope, including its own
            // redaction/sanitization, at the same trust level as returning a raw
            // Value today. RESPONSE middleware (redaction/sanitization/audit) +
            // `handle_tool_error` therefore run ONLY for the Payload/error arm
            // below; REQUEST middleware already fired above for EVERY tool, and a
            // handler `Err(_)` still routes through `handle_tool_error` via this
            // arm (the bypass is scoped to the successful `Verbatim` arm only).
            if let task_dispatch::DispatchOutput::Middleware(ref mut result) = resolved {
                // Process response through tool middleware chain
                if let Err(e) = self
                    .tool_middleware_chain
                    .read()
                    .await
                    .process_response(&req.name, result, &context)
                    .await
                {
                    // Log error but continue with original result
                    tracing::warn!("Tool response middleware processing failed: {}", e);
                }

                // If tool execution failed, call handle_tool_error
                if let Err(ref e) = result {
                    self.tool_middleware_chain
                        .read()
                        .await
                        .handle_tool_error(&req.name, e, &context)
                        .await;
                }
            }

            resolved
        };

        // On WASM, execute tool directly without middleware
        #[cfg(target_arch = "wasm32")]
        let result = handler.handle(req.arguments, extra).await;

        // Token cleanup is unconditional (success or failure) and does not
        // touch the outcome, so it runs once before the value/error split —
        // preserving cleanup parity for the verbatim `ToolOutput::Result` arm.
        self.cancellation_manager
            .remove_token(&request_id_str)
            .await;
        let result = match dispatch_output {
            // VERBATIM (D-04 + D-04a): the handler owns the full `CallToolResult`
            // envelope — emit it to the wire as-is. RESPONSE middleware, the
            // create-path gate, text-wrap, and widget enrichment are ALL bypassed
            // (mirrors the `ToolRejected` verbatim early-return below, which also
            // returns after the unconditional token cleanup).
            //
            // D-06 (Phase 118.1) RECLASSIFIES exactly one clause of D-04a: the
            // bypass covers the response PIPELINE, not the handler's own
            // `extra.set_result_meta(..)`. Those keys come from the same handler
            // that authored this envelope, at the same trust level, so draining
            // them here merges a handler's two `_meta` sources rather than
            // reintroducing server-side rewriting. Handler-key-wins precedence,
            // never a whole-map replace. Twin of the `ServerCore` arm.
            task_dispatch::DispatchOutput::Verbatim(call_result) => {
                #[cfg(not(target_arch = "wasm32"))]
                let call_result = {
                    let mut call_result = call_result;
                    if let Some(handler_meta) = result_meta_handle.take_result_meta() {
                        crate::server::cancellation::merge_result_meta(
                            &mut call_result,
                            handler_meta,
                        );
                    }
                    call_result
                };
                return Ok(serde_json::to_value(call_result)?);
            },
            task_dispatch::DispatchOutput::Middleware(result) => match result {
                Ok(v) => v,
                // `Error::ToolRejected` is an APPLICATION-level rejection (e.g.
                // Code Mode policy: a SELECT missing its LIMIT), not a protocol
                // fault. Map it to a successful `CallToolResult { isError: true }`
                // (message → content, details → structuredContent) so the model
                // reads the reason and retries with corrected input, instead of
                // `?`-propagating a JSON-RPC error that reads as a server crash.
                // All other errors keep propagating as protocol errors.
                Err(Error::ToolRejected { message, details }) => {
                    return Ok(serde_json::to_value(CallToolResult::rejected(
                        message, details,
                    ))?);
                },
                Err(e) => return Err(e),
            },
        };

        // CREATE-PATH (Phase 102, HTASK-02; era-aware trigger from plan 114-12):
        // a `tools/call` whose era trigger fired over the high-level `Server`
        // mints a store task and returns a `CreateTaskResult` envelope. The
        // SHARED `maybe_build_task_created` gate is the SINGLE source of truth:
        // it returns `Some` ONLY when the era's trigger fired (v1: the `task`
        // field; v2: the client's tasks-extension declaration) AND a store
        // backend exists AND the tool's `TaskSupport ∈ {Required, Optional}` AND
        // the produced value is task-shaped (`taskId` + `status`); otherwise
        // `None` (fall through to a normal `CallToolResult`, no leakage — incl.
        // `Forbidden`/`None`).
        //
        // The store mints the canonical id (D-STORE-MINTS-ID); the tool's
        // fabricated `taskId` is never trusted on the wire. We pass the RAW
        // facts (`create_trigger`, `tool_task_support`) — the gate enforces the
        // complete precondition internally. The gate returns a full
        // `JSONRPCResponse`; we decompose it back into this fn's `Result<Value>`
        // contract (the caller re-wraps with the SAME request id via
        // `create_response`, so the id is preserved and `-32603` store errors
        // surface as JSON-RPC errors).
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some((response, claim)) = self
                .task_dispatch()
                .maybe_build_task_created(
                    create_path_id,
                    &result,
                    tool_task_support,
                    create_trigger,
                    create_path_auth.as_ref(),
                    create_path_era,
                )
                .await
            {
                // On v2 this is the ONE response in the whole surface that earns
                // `resultType: "task"`; the claim is what carries that fact past
                // the `Result<Value>` contract this fn is bound to.
                *dispatch_claim = claim;
                return match response.payload {
                    crate::types::jsonrpc::ResponsePayload::Result(value) => Ok(value),
                    // The create-path only emits `-32603` store errors here; the
                    // caller's `create_response` re-wraps `Err` as `-32603`, so
                    // the code is preserved. Surface the store's message.
                    crate::types::jsonrpc::ResponsePayload::Error(err) => {
                        Err(crate::Error::Protocol {
                            code: crate::error::ErrorCode(err.code),
                            message: err.message,
                            data: err.data,
                        })
                    },
                };
            }
        }

        // TOUT-02 double-wrap tripwire: BEFORE stringifying `result` into text
        // content, WARN (+ debug_assert in debug/CI) if it structurally resembles
        // an already-built `CallToolResult` — the silent double-wrap bug. Honors
        // the per-tool `suppress_double_wrap_check` opt-out (D-08). Non-wasm only
        // (the `task_dispatch` unit is non-wasm, matching the create-path above).
        #[cfg(not(target_arch = "wasm32"))]
        task_dispatch::double_wrap_tripwire(
            &req.name,
            &result,
            self.suppress_double_wrap.contains(req.name.as_str()),
        );

        // Build CallToolResult, adding structured_content for widget tools and
        // for tools with a declared outputSchema (MCP spec: a tool that
        // declares an outputSchema SHOULD return structuredContent conforming
        // to it). The text voice always carries the serialized value so
        // text-only clients keep working.
        let text = result.to_string();
        let mut call_result = CallToolResult::new(vec![crate::types::Content::text(text)]);

        if let Some(info) = self.tool_infos.get(&req.name) {
            // A declared outputSchema means structuredContent is emitted below
            // (via widget enrichment or the schema bridge) — validate the value
            // against it regardless of which branch does the emitting.
            if let Some(schema) = &info.output_schema {
                output_validation::warn_on_schema_mismatch(
                    &req.name,
                    schema,
                    &result,
                    validation_era,
                );
            }
            if info.widget_meta().is_some() {
                call_result = call_result.with_widget_enrichment(info, result);
            } else if info.output_schema.is_some() {
                call_result = call_result.with_structured_content(result);
            }
        }

        // D-03.3: drain any handler-set result `_meta` (via extra.set_result_meta)
        // and merge it onto the Payload-built envelope with handler-key-wins
        // precedence (unrelated widget/native keys preserved). The verbatim
        // `ToolOutput::Result` arm above returns earlier and still owns its
        // content, its redaction and its bypass of the response pipeline — but
        // since D-06 (Phase 118.1) it performs this SAME drain against its own
        // envelope before returning, so `set_result_meta` is no longer silently
        // dropped there. By the time control reaches this line the slot has
        // therefore only ever been filled by a Payload-path handler.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(handler_meta) = result_meta_handle.take_result_meta() {
            crate::server::cancellation::merge_result_meta(&mut call_result, handler_meta);
        }

        Ok(serde_json::to_value(call_result)?)
    }

    fn handle_list_prompts(&self, _req: ListPromptsRequest) -> Result<Value> {
        let prompts = self
            .prompts
            .iter()
            .map(|(name, handler)| {
                // Use prompt metadata if provided, otherwise use defaults
                if let Some(mut info) = handler.metadata() {
                    // Ensure the name matches the registered name
                    info.name.clone_from(name);
                    info
                } else {
                    crate::types::PromptInfo::new(name)
                }
            })
            .collect::<Vec<_>>();

        Ok(serde_json::to_value(ListPromptsResult {
            prompts,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })?)
    }

    async fn handle_get_prompt(
        &self,
        request_id: RequestId,
        req: GetPromptRequest,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<Value> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Prompt '{}' not found", req.name)))?;

        let request_id_str = request_id.to_string();
        let cancellation_token = self
            .cancellation_manager
            .create_token(request_id_str.clone())
            .await;

        // The request-scoped progress reporter — the SAME resolution the
        // tools/call dispatcher makes, so a prompt handler over v1 HTTP emits on
        // the session stream too (Phase 118.1 plan 11).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter =
            self.progress_reporter_for(req._meta.as_ref(), protocol_context.as_ref());

        // Propagate the request `_meta` (raw JSON) and the once-at-ingress
        // resolved protocol context so prompt handlers read
        // era/client_info/trace_context via `extra` on the high-level `Server`
        // path too — the twin of the ServerCore wiring (Phase 112, mirrors the
        // handle_call_tool twin).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let request_meta_value = crate::server::core::request_meta_to_value(req._meta.as_ref());

        let extra = self.attach_peer(
            crate::server::cancellation::RequestHandlerExtra::new(
                request_id_str.clone(),
                cancellation_token,
            )
            .with_auth_context(auth_context)
            .with_progress_reporter(progress_reporter)
            .with_request_meta(request_meta_value)
            .with_protocol_context(protocol_context),
        );
        let result = match handler.handle(req.arguments, extra).await {
            Ok(v) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Ok(v)
            },
            Err(e) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Err(e)
            },
        }?;
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_list_resources(
        &self,
        request_id: RequestId,
        req: ListResourcesRequest,
        auth_context: Option<auth::AuthContext>,
        // THREADED, not resolved here (Phase 118.1-08, G-9) — the twin of the
        // `ServerCore` site. `ListResourcesRequest` carries no `_meta`, so the
        // context can only arrive from the caller.
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<Value> {
        if let Some(handler) = &self.resources {
            let request_id_str = request_id.to_string();
            let cancellation_token = self
                .cancellation_manager
                .create_token(request_id_str.clone())
                .await;
            let extra = self.attach_peer(
                crate::server::cancellation::RequestHandlerExtra::new(
                    request_id_str.clone(),
                    cancellation_token,
                )
                .with_auth_context(auth_context)
                .with_protocol_context(protocol_context),
            );
            let mut result = match handler.list(req.cursor, extra).await {
                Ok(v) => {
                    self.cancellation_manager
                        .remove_token(&request_id_str)
                        .await;
                    Ok(v)
                },
                Err(e) => {
                    self.cancellation_manager
                        .remove_token(&request_id_str)
                        .await;
                    Err(e)
                },
            }?;
            // Enrich ResourceInfo with tool _meta for widget resources
            if !self.uri_to_tool_meta.is_empty() {
                for resource in &mut result.resources {
                    if let Some(tool_meta) = self.uri_to_tool_meta.get(&resource.uri) {
                        let meta = resource.meta.get_or_insert_with(serde_json::Map::new);
                        crate::types::ui::deep_merge(meta, tool_meta.clone());
                    }
                }
            }
            Ok(serde_json::to_value(result)?)
        } else {
            Ok(serde_json::to_value(ListResourcesResult {
                resources: vec![],
                next_cursor: None,
                ttl_ms: None,
                cache_scope: None,
            })?)
        }
    }

    async fn handle_read_resource(
        &self,
        request_id: RequestId,
        req: ReadResourceRequest,
        auth_context: Option<auth::AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<Value> {
        let handler = self
            .resources
            .as_ref()
            .ok_or_else(|| Error::not_found("No resource handler configured".to_string()))?;

        let request_id_str = request_id.to_string();
        let cancellation_token = self
            .cancellation_manager
            .create_token(request_id_str.clone())
            .await;

        // The request-scoped progress reporter — the SAME resolution the
        // tools/call dispatcher makes, so a resource read over v1 HTTP emits on
        // the session stream too (Phase 118.1 plan 11).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter =
            self.progress_reporter_for(req._meta.as_ref(), protocol_context.as_ref());

        // Propagate the request `_meta` (raw JSON) and the once-at-ingress
        // resolved protocol context so resource handlers read
        // era/client_info/trace_context via `extra` on the high-level `Server`
        // path too — the twin of the ServerCore wiring (Phase 112).
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let request_meta_value = crate::server::core::request_meta_to_value(req._meta.as_ref());

        let extra = self.attach_peer(
            crate::server::cancellation::RequestHandlerExtra::new(
                request_id_str.clone(),
                cancellation_token,
            )
            .with_auth_context(auth_context)
            .with_progress_reporter(progress_reporter)
            .with_request_meta(request_meta_value)
            .with_protocol_context(protocol_context),
        );
        let mut result = match handler.read(&req.uri, extra).await {
            Ok(v) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Ok(v)
            },
            Err(e) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Err(e)
            },
        }?;
        // Merge tool descriptor keys into content _meta for widget resources
        if !self.uri_to_tool_meta.is_empty() {
            for content in &mut result.contents {
                if let crate::types::Content::Resource { uri, meta, .. } = content {
                    if let Some(tool_meta) = self.uri_to_tool_meta.get(uri.as_str()) {
                        let content_meta = meta.get_or_insert_with(serde_json::Map::new);
                        crate::types::ui::deep_merge(content_meta, tool_meta.clone());
                    }
                }
            }
        }
        Ok(serde_json::to_value(result)?)
    }

    #[allow(clippy::unused_self)]
    fn handle_list_resource_templates(&self, _req: ListResourceTemplatesRequest) -> Result<Value> {
        Ok(serde_json::to_value(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })?)
    }

    async fn handle_create_message(
        &self,
        request_id: RequestId,
        req: crate::types::CreateMessageParams,
        // THREADED, not resolved here (Phase 118.1-08, G-9). This arm serves an
        // INBOUND `sampling/createMessage` — a `ClientRequest` variant, so a
        // client handshake absolutely does have meaning here and the site is
        // THREAD-THEN-FOLD, not a NO-OP. (The server-to-client direction is a
        // `ServerRequest` handled by the peer dispatcher, which builds no
        // `RequestHandlerExtra` at all.) `CreateMessageParams` carries no
        // `_meta`, so the context can only arrive from the caller.
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<Value> {
        let handler = self
            .sampling
            .as_ref()
            .ok_or_else(|| Error::not_found("No sampling handler configured".to_string()))?;

        let request_id_str = request_id.to_string();
        let cancellation_token = self
            .cancellation_manager
            .create_token(request_id_str.clone())
            .await;
        let extra = self.attach_peer(
            crate::server::cancellation::RequestHandlerExtra::new(
                request_id_str.clone(),
                cancellation_token,
            )
            .with_protocol_context(protocol_context),
        );
        let result = match handler.create_message(req, extra).await {
            Ok(v) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Ok(v)
            },
            Err(e) => {
                self.cancellation_manager
                    .remove_token(&request_id_str)
                    .await;
                Err(e)
            },
        }?;
        Ok(serde_json::to_value(result)?)
    }

    /// Register a root directory or URI that the server has access to.
    ///
    /// This method allows the server to announce to clients that it has
    /// access to specific file system roots or URIs. This is useful for
    /// resource handlers that need to expose filesystem access or other
    /// URI-based resources.
    ///
    /// # Arguments
    ///
    /// * `uri` - The root URI to register (e.g., `file:///home/user/project`)
    /// * `name` - Optional human-readable name for the root
    ///
    /// # Returns
    ///
    /// An unregister function that can be called to remove the root registration.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Register a project root
    /// let unregister = server.register_root(
    ///     "file:///home/user/project",
    ///     Some("My Project".to_string())
    /// ).await?;
    ///
    /// // Later, unregister the root
    /// unregister();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_root(
        &self,
        uri: impl Into<String>,
        name: Option<String>,
    ) -> Result<impl FnOnce() + Send + 'static> {
        let mut roots_manager = self.roots_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            roots_manager.set_notification_sender({
                let tx = tx.clone();
                move |server_notification| {
                    let _ = tx.try_send(Notification::Server(server_notification));
                }
            });
        }
        roots_manager.register_root(uri.into(), name).await
    }

    /// Get the list of registered roots.
    ///
    /// Returns a list of all currently registered root URIs and their
    /// associated names. Roots are directories or URIs that the server
    /// has announced access to.
    ///
    /// # Returns
    ///
    /// A vector of `Root` objects containing URI and optional name.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Register some roots
    /// server.register_root("file:///home/user/project1", Some("Project 1".to_string())).await?;
    /// server.register_root("file:///home/user/project2", None).await?;
    ///
    /// // Get the list of roots
    /// let roots = server.get_roots().await;
    /// println!("Registered {} roots", roots.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_roots(&self) -> Vec<roots::Root> {
        let roots_manager = self.roots_manager.read().await;
        roots_manager.get_roots().await
    }

    /// Subscribe a client to resource updates.
    ///
    /// This method allows the server to track which clients are interested
    /// in updates to specific resources. When a resource changes, the server
    /// can notify all subscribed clients.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to subscribe to
    /// * `client_id` - Identifier for the subscribing client
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Subscribe client to resource updates
    /// server.subscribe_resource(
    ///     "file:///project/file.txt".to_string(),
    ///     "client-123".to_string()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn subscribe_resource(&self, uri: String, client_id: String) -> Result<()> {
        if uri.is_empty() || client_id.is_empty() {
            return Err(Error::invalid_params("URI and client_id must not be empty"));
        }

        let mut subscription_manager = self.subscription_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            subscription_manager.set_notification_sender({
                let tx = tx.clone();
                move |notification| {
                    let _ = tx.try_send(Notification::Server(notification));
                }
            });
        }

        subscription_manager.subscribe(uri, client_id).await
    }

    /// Cancel a request that is currently being processed.
    ///
    /// This method allows the server to cancel ongoing requests, which is
    /// useful for implementing request timeouts or client-requested cancellations.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to cancel
    /// * `reason` - Optional reason for cancellation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("cancel-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Cancel a request
    /// server.cancel_request(
    ///     "request-123".to_string(),
    ///     Some("User requested cancellation".to_string())
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cancel_request(&self, request_id: String, reason: Option<String>) -> Result<()> {
        if request_id.is_empty() {
            return Err(Error::invalid_params("Request ID must not be empty"));
        }

        self.cancellation_manager
            .cancel_request(request_id, reason)
            .await
    }

    /// Unsubscribe a client from resource updates.
    ///
    /// This method removes a client's subscription to a specific resource,
    /// so they will no longer receive notifications when that resource changes.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to unsubscribe from
    /// * `client_id` - Identifier for the client to unsubscribe
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Unsubscribe client from resource updates
    /// server.unsubscribe_resource(
    ///     "file:///project/file.txt".to_string(),
    ///     "client-123".to_string()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unsubscribe_resource(&self, uri: String, client_id: String) -> Result<()> {
        if uri.is_empty() || client_id.is_empty() {
            return Err(Error::invalid_params("URI and client_id must not be empty"));
        }

        let subscription_manager = self.subscription_manager.read().await;
        subscription_manager.unsubscribe(uri, client_id).await
    }

    /// Notify subscribers that a resource has been updated.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource that was updated
    ///
    /// # Returns
    ///
    /// The number of subscribers that were notified.
    pub async fn notify_resource_updated(&self, uri: String) -> Result<usize> {
        let mut subscription_manager = self.subscription_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            subscription_manager.set_notification_sender({
                let tx = tx.clone();
                move |notification| {
                    let _ = tx.try_send(Notification::Server(notification));
                }
            });
        }
        subscription_manager.notify_resource_updated(uri).await
    }
}

/// Trait for types annotated with `#[mcp_server]`.
///
/// Generated by the `#[mcp_server]` proc macro. Provides bulk registration of
/// tools and prompts via `register()`. Users should call `.mcp_server(instance)`
/// on the builder instead of implementing this trait manually.
///
/// # Examples
///
/// ```rust,ignore
/// use pmcp::ServerBuilder;
///
/// #[mcp_server]
/// impl MyServer {
///     #[mcp_tool(description = "Query data")]
///     async fn query(&self, args: QueryArgs) -> Result<Value> { /* ... */ }
///
///     #[mcp_prompt(description = "Generate query")]
///     async fn query_prompt(&self, args: PromptArgs) -> Result<GetPromptResult> { /* ... */ }
/// }
///
/// let server = MyServer { db };
/// let builder = ServerBuilder::new()
///     .mcp_server(server);
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub trait McpServer {
    /// Register all tools and prompts from this server on the builder.
    fn register(self, builder: ServerBuilder) -> ServerBuilder;
}

/// Builder for creating servers.
#[cfg(not(target_arch = "wasm32"))]
pub struct ServerBuilder {
    name: Option<String>,
    version: Option<String>,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    resources: Option<Arc<dyn ResourceHandler>>,
    /// Completion provider backing `completion/complete` (Phase 118.1-04,
    /// CONF-05), set via [`Self::completions`]. The twin of
    /// `ServerCoreBuilder`'s slot of the same name: a provider registered
    /// through EITHER builder family reaches its own dispatcher.
    completions: Option<Arc<dyn crate::types::completable::CompletionProviderTrait>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    /// Cancellation manager for request cancellation
    cancellation_manager: cancellation::CancellationManager,
    /// Roots manager for directory/URI registration
    roots_manager: roots::RootsManager,
    /// Authentication provider for validating requests
    auth_provider: Option<Arc<dyn auth::AuthProvider>>,
    /// Tool authorizer for fine-grained access control
    tool_authorizer: Option<Arc<dyn auth::ToolAuthorizer>>,
    /// Tool protection requirements to be applied at build time
    tool_protections: HashMap<String, Vec<String>>,
    /// Tool middleware chain for cross-cutting concerns
    #[cfg(not(target_arch = "wasm32"))]
    tool_middlewares: Vec<Arc<dyn tool_middleware::ToolMiddleware>>,
    /// HTTP middleware chain for `StreamableHttpServer`
    #[cfg(feature = "streamable-http")]
    http_middleware: Option<Arc<http_middleware::ServerHttpMiddlewareChain>>,
    /// Host layers for MCP Apps metadata enrichment (e.g., `ChatGPT`)
    #[cfg(feature = "mcp-apps")]
    host_layers: Vec<crate::types::mcp_apps::HostType>,
    /// Optional website URL for the server implementation (MCP 2025-11-25)
    website_url: Option<String>,
    /// Optional icons for the server implementation (MCP 2025-11-25)
    icons: Option<Vec<crate::types::protocol::IconInfo>>,
    /// Accumulated SEP-2640 Agent Skills. The registry is finalized into a
    /// single `SkillsHandler` exactly once at `.build()` time so chained
    /// `.skill(...)` / `.skills(...)` calls never produce nested wrappers.
    #[cfg(feature = "skills")]
    pending_skills: Option<skills::Skills>,
    /// Legacy experimental task router backend (set via [`Self::with_task_store`]).
    #[cfg(not(target_arch = "wasm32"))]
    task_router: Option<Arc<dyn crate::server::tasks::TaskRouter>>,
    /// Standard task store backend (set via [`Self::task_store`]). Presence
    /// auto-advertises the `tasks` capability at `build()`.
    #[cfg(not(target_arch = "wasm32"))]
    task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,
    /// Tool names opting out of the TOUT-02 double-wrap tripwire (D-08), set via
    /// [`Self::suppress_double_wrap_check`]. Carried into the built `Server` and
    /// consulted at the Payload wrap site.
    #[cfg(not(target_arch = "wasm32"))]
    suppress_double_wrap: HashSet<String>,
    /// Configured protocol-version accept-list (Phase 112, VERS-01/02). Defaults
    /// to the v1-only legacy set (excludes `2026-07-28`); overridden via
    /// [`Self::with_supported_protocol_versions`].
    supported_protocol_versions: Vec<ProtocolVersion>,
    /// Explicit `requestState` minting key (Phase 113, HTTP-02), set via
    /// [`Self::with_request_state_key`]. When present it overrides
    /// `PMCP_REQUEST_STATE_KEY` entirely.
    ///
    /// Copy 1 of 3 (D-113-P): held as a
    /// [`SecretKey`](crate::server::request_state::SecretKey), never as bare
    /// `[u8; 32]`, so the destructor rides on the value and scrubs on drop —
    /// including on every early-`?` path out of [`Self::build`]. Reverting this
    /// to bare bytes is caught at COMPILE time by
    /// `server_builder_request_state_key_field_is_the_zeroizing_type`.
    #[cfg(feature = "streamable-http")]
    request_state_key: Option<request_state::SecretKey>,
    /// Rotated-out `requestState` keys accepted for VERIFICATION only, set via
    /// [`Self::with_request_state_previous_keys`].
    ///
    /// Copy 1 of 3 (D-113-P), the rotated-out half: each element scrubs itself
    /// when the `Vec` drops.
    #[cfg(feature = "streamable-http")]
    request_state_previous_keys: Vec<request_state::SecretKey>,
    /// Explicit continuation lifetime, set via [`Self::with_request_state_ttl`].
    /// Beats both the 300-second default and `PMCP_REQUEST_STATE_TTL_SECS` (D-05).
    #[cfg(feature = "streamable-http")]
    request_state_ttl: Option<std::time::Duration>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for ServerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerBuilder")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("sampling", &self.sampling.is_some())
            .finish()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ServerBuilder {
    /// Create a new server builder.
    ///
    /// Creates a new `ServerBuilder` with default capabilities and no handlers.
    /// Use the builder methods to configure the server before calling `build()`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::ServerBuilder;
    ///
    /// let builder = ServerBuilder::new();
    /// ```
    ///
    /// This is equivalent to using the default implementation:
    ///
    /// ```rust,no_run
    /// use pmcp::ServerBuilder;
    ///
    /// let builder = ServerBuilder::default();
    /// ```
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            capabilities: ServerCapabilities::default(),
            tools: HashMap::new(),
            prompts: HashMap::new(),
            resources: None,
            completions: None,
            sampling: None,
            cancellation_manager: cancellation::CancellationManager::new(),
            roots_manager: roots::RootsManager::new(),
            auth_provider: None,
            tool_authorizer: None,
            tool_protections: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            tool_middlewares: Vec::new(),
            #[cfg(feature = "streamable-http")]
            http_middleware: None,
            #[cfg(feature = "mcp-apps")]
            host_layers: Vec::new(),
            website_url: None,
            icons: None,
            #[cfg(feature = "skills")]
            pending_skills: None,
            #[cfg(not(target_arch = "wasm32"))]
            task_router: None,
            #[cfg(not(target_arch = "wasm32"))]
            task_store: None,
            #[cfg(not(target_arch = "wasm32"))]
            suppress_double_wrap: HashSet::new(),
            supported_protocol_versions: crate::types::protocol::context::default_accept_list(),
            #[cfg(feature = "streamable-http")]
            request_state_key: None,
            #[cfg(feature = "streamable-http")]
            request_state_previous_keys: Vec::new(),
            #[cfg(feature = "streamable-http")]
            request_state_ttl: None,
        }
    }

    /// Configure the shared `requestState` minting key (Phase 113, HTTP-02, D-03).
    ///
    /// With no call, the key is resolved from `PMCP_REQUEST_STATE_KEY`; when that
    /// variable is unset the server generates a per-process key and WARNs at build
    /// time (D-04). Calling this overrides the environment entirely, which is what
    /// makes deterministic integration tests and multiple differently-configured
    /// servers in one process possible.
    ///
    /// The key must be shared byte-for-byte by every instance behind a load
    /// balancer that should be able to resume each other's multi-round-trip
    /// requests.
    ///
    /// Has no effect on a server that did not opt into the v2 (`2026-07-28`) era.
    ///
    /// The parameter type is deliberately still `[u8; 32]`: the SDK owns the
    /// copy it takes, not the caller's (D-113-P, T-113-121).
    #[cfg(feature = "streamable-http")]
    #[must_use]
    pub fn with_request_state_key(mut self, mut key: [u8; 32]) -> Self {
        // Closes copy 1 of 3 (D-113-P): the FIELD now scrubs on drop.
        self.request_state_key = Some(request_state::SecretKey::new(key));
        // Closes copy 2 of 3 (D-113-P): this by-value parameter's OWN stack
        // slot. `[u8; 32]` is `Copy`, so the line above copied out of it and
        // left the caller's key bytes sitting here.
        key.zeroize();
        self
    }

    /// Accept rotated-out `requestState` keys for VERIFICATION only.
    ///
    /// Tokens minted under a listed key still verify, but new tokens are always
    /// minted under the current key — so a rotation does not strand in-flight
    /// continuations. With no call, only the current key is accepted.
    ///
    /// Has no effect on a server that did not opt into the v2 (`2026-07-28`) era.
    #[cfg(feature = "streamable-http")]
    #[must_use]
    pub fn with_request_state_previous_keys(mut self, mut keys: Vec<[u8; 32]>) -> Self {
        // Closes copy 1 of 3 (D-113-P), rotated-out half.
        self.request_state_previous_keys = keys
            .iter()
            .copied()
            .map(request_state::SecretKey::new)
            .collect();
        // Closes copy 2 of 3 (D-113-P): the by-value `Vec`'s own heap buffer,
        // which the copy above read out of and would otherwise return to the
        // allocator holding every rotated-out key in the clear. `Vec::zeroize`
        // scrubs the initialized elements AND the spare capacity.
        keys.zeroize();
        self
    }

    /// Configure the `requestState` continuation lifetime (D-05).
    ///
    /// With no call, the lifetime is `PMCP_REQUEST_STATE_TTL_SECS` if parseable,
    /// else 300 seconds. A builder value beats both.
    ///
    /// Has no effect on a server that did not opt into the v2 (`2026-07-28`) era.
    #[cfg(feature = "streamable-http")]
    #[must_use]
    pub fn with_request_state_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.request_state_ttl = Some(ttl);
        self
    }

    /// Opt into a protocol-version accept-list (Phase 112, VERS-01/02; D-02/D-04).
    ///
    /// The high-level `Server` twin of
    /// [`ServerCoreBuilder::with_supported_protocol_versions`](crate::server::builder::ServerCoreBuilder::with_supported_protocol_versions).
    /// With no call, the server is v1-only and behaves exactly as today. An empty
    /// accept-list falls back to the v1-only default (never all-reject).
    #[must_use]
    pub fn with_supported_protocol_versions(
        mut self,
        versions: impl IntoIterator<Item = ProtocolVersion>,
    ) -> Self {
        self.supported_protocol_versions =
            crate::types::protocol::context::normalize_accept_list(versions);
        self
    }

    /// Set the server name.
    ///
    /// The server name identifies this MCP server implementation.
    /// This is required and will be sent to clients during initialization.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the server
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// let server = Server::builder()
    ///     .name("file-manager")
    ///     .version("1.0.0")
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the server version.
    ///
    /// The server version identifies this specific version of the MCP server.
    /// This is required and will be sent to clients during initialization.
    ///
    /// # Arguments
    ///
    /// * `version` - The version string (e.g., "1.0.0", "2.1.3-beta")
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// let server = Server::builder()
    ///     .name("data-processor")
    ///     .version("2.1.0")
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the website URL for the server implementation (MCP 2025-11-25).
    pub fn website_url(mut self, url: impl Into<String>) -> Self {
        self.website_url = Some(url.into());
        self
    }

    /// Set icons for the server implementation (MCP 2025-11-25).
    pub fn with_icons(mut self, icons: Vec<crate::types::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set server capabilities.
    ///
    /// Configures the capabilities that this server supports.
    /// Capabilities inform clients about which MCP features are available.
    ///
    /// # Arguments
    ///
    /// * `capabilities` - The server capabilities to advertise
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ServerCapabilities, ToolCapabilities};
    ///
    /// let mut capabilities = ServerCapabilities::default();
    /// capabilities.tools = Some(ToolCapabilities {
    ///     list_changed: Some(true),
    /// });
    ///
    /// let server = Server::builder()
    ///     .name("advanced-server")
    ///     .version("1.0.0")
    ///     .capabilities(capabilities)
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Add a tool handler.
    ///
    /// Registers a tool that clients can call via the tools/call method.
    /// Tools are the primary way servers provide functionality to clients.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool (used by clients to call it)
    /// * `handler` - The handler implementation for this tool
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct FileListTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for FileListTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         let path = args["path"].as_str().unwrap_or(".");
    ///         // List files in path...
    ///         Ok(serde_json::json!({"files": ["file1.txt", "file2.txt"]}))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .tool("list_files", FileListTool{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
        self.tools.insert(name.into(), Arc::new(handler));

        // Update capabilities to include tools
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a tool handler with an Arc.
    ///
    /// This variant lets the caller share the handler `Arc` between the
    /// builder and an external in-process handler map (e.g., a downstream
    /// toolkit's handler registry) without writing a delegating wrapper
    /// shim. Behavior is otherwise identical to [`Self::tool`]: the first
    /// registration auto-enables `capabilities.tools`.
    pub fn tool_arc(mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) -> Self {
        let name = name.into();
        self.tools.insert(name, handler);

        // Update capabilities to include tools
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Register all tools and prompts from an `#[mcp_server]` annotated type.
    ///
    /// This is the ergonomic counterpart to individually registering tools and
    /// prompts. The server instance provides shared state via `&self` to all
    /// tool and prompt methods.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::ServerBuilder;
    ///
    /// #[mcp_server]
    /// impl MyServer {
    ///     #[mcp_tool(description = "Query data")]
    ///     async fn query(&self, args: QueryArgs) -> Result<Value> { /* ... */ }
    ///
    ///     #[mcp_prompt(description = "Generate query")]
    ///     async fn query_prompt(&self, args: PromptArgs) -> Result<GetPromptResult> { /* ... */ }
    /// }
    ///
    /// let server = MyServer { db };
    /// let builder = ServerBuilder::new()
    ///     .name("my-server")
    ///     .mcp_server(server);
    /// ```
    pub fn mcp_server<T: McpServer>(self, server: T) -> Self {
        server.register(self)
    }

    /// Add a type-safe tool handler with automatic schema generation.
    ///
    /// This method provides first-class support for creating tools with:
    /// - Automatic JSON schema generation from Rust types
    /// - Compile-time type safety
    /// - Runtime validation
    /// - Field descriptions from doc comments
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct EchoArgs {
    ///     /// The message to echo
    ///     message: String,
    ///     /// Optional prefix
    ///     prefix: Option<String>,
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), pmcp::Error> {
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed("echo", |args: EchoArgs, _| {
    ///         Box::pin(async move {
    ///             let message = match args.prefix {
    ///                 Some(p) => format!("{}: {}", p, args.message),
    ///                 None => args.message,
    ///             };
    ///             Ok(serde_json::json!({ "message": message }))
    ///         })
    ///     })
    ///     .build();
    /// # Ok::<(), pmcp::Error>(())
    /// # }
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed<T, F, Fut>(mut self, name: impl Into<String>, handler: F) -> Self
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        F: Fn(T, crate::RequestHandlerExtra) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = crate::Result<serde_json::Value>> + Send + 'static,
    {
        use crate::server::typed_tool::TypedTool;
        use std::pin::Pin;

        let name_str = name.into();

        // Wrap the handler to return Pin<Box<dyn Future>>
        let wrapped_handler = move |args: T,
                                    extra: crate::RequestHandlerExtra|
              -> Pin<
            Box<dyn std::future::Future<Output = crate::Result<serde_json::Value>> + Send>,
        > { Box::pin(handler(args, extra)) };

        let tool = TypedTool::new(name_str.clone(), wrapped_handler);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a type-safe tool handler with automatic schema generation and description.
    ///
    /// This is a convenience overload that allows setting a description directly
    /// without needing to chain `.with_description()`.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct EchoArgs {
    ///     /// The message to echo
    ///     message: String,
    ///     /// Optional prefix
    ///     prefix: Option<String>,
    /// }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_with_description(
    ///         "echo",
    ///         "Echoes back a message with an optional prefix",
    ///         |args: EchoArgs, _| {
    ///             Box::pin(async move {
    ///                 let message = match args.prefix {
    ///                     Some(p) => format!("{}: {}", p, args.message),
    ///                     None => args.message,
    ///                 };
    ///                 Ok(serde_json::json!({ "message": message }))
    ///             })
    ///         }
    ///     );
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_with_description<T, F, Fut>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        F: Fn(T, crate::RequestHandlerExtra) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = crate::Result<serde_json::Value>> + Send + 'static,
    {
        use crate::server::typed_tool::TypedTool;
        use std::pin::Pin;

        let name_str = name.into();

        // Wrap the handler to return Pin<Box<dyn Future>>
        let wrapped_handler = move |args: T,
                                    extra: crate::RequestHandlerExtra|
              -> Pin<
            Box<dyn std::future::Future<Output = crate::Result<serde_json::Value>> + Send>,
        > { Box::pin(handler(args, extra)) };

        let tool = TypedTool::new(name_str.clone(), wrapped_handler).with_description(description);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a synchronous type-safe tool handler with automatic schema generation.
    ///
    /// Similar to `tool_typed` but for synchronous handlers.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct MathArgs {
    ///     /// First number
    ///     a: f64,
    ///     /// Second number
    ///     b: f64,
    ///     /// Operation to perform
    ///     op: String,
    /// }
    ///
    /// # fn main() -> Result<(), pmcp::Error> {
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_sync("calculator", |args: MathArgs, _| {
    ///         let result = match args.op.as_str() {
    ///             "add" => args.a + args.b,
    ///             "subtract" => args.a - args.b,
    ///             "multiply" => args.a * args.b,
    ///             "divide" => args.a / args.b,
    ///             _ => return Err(pmcp::Error::Validation("Unknown operation".into())),
    ///         };
    ///         Ok(serde_json::json!({ "result": result }))
    ///     })
    ///     .build();
    /// # Ok::<(), pmcp::Error>(())
    /// # }
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_sync<T, F>(mut self, name: impl Into<String>, handler: F) -> Self
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        F: Fn(T, crate::RequestHandlerExtra) -> crate::Result<serde_json::Value>
            + Send
            + Sync
            + 'static,
    {
        use crate::server::typed_tool::TypedSyncTool;
        let name_str = name.into();
        let tool = TypedSyncTool::new(name_str.clone(), handler);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a synchronous type-safe tool handler with automatic schema generation and description.
    ///
    /// This is a convenience overload that allows setting a description directly
    /// without needing to chain `.with_description()`.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct MathArgs {
    ///     /// First number
    ///     a: f64,
    ///     /// Second number
    ///     b: f64,
    ///     /// Operation to perform
    ///     op: String,
    /// }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_sync_with_description(
    ///         "calculator",
    ///         "Performs synchronous mathematical operations",
    ///         |args: MathArgs, _| {
    ///             let result = match args.op.as_str() {
    ///                 "add" => args.a + args.b,
    ///                 "subtract" => args.a - args.b,
    ///                 "multiply" => args.a * args.b,
    ///                 "divide" => args.a / args.b,
    ///                 _ => return Err(pmcp::Error::Validation("Unknown operation".into())),
    ///             };
    ///             Ok(serde_json::json!({ "result": result }))
    ///         }
    ///     );
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_sync_with_description<T, F>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        F: Fn(T, crate::RequestHandlerExtra) -> crate::Result<serde_json::Value>
            + Send
            + Sync
            + 'static,
    {
        use crate::server::typed_tool::TypedSyncTool;
        let name_str = name.into();
        let tool = TypedSyncTool::new(name_str.clone(), handler).with_description(description);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a type-safe tool handler with both input and output typing.
    ///
    /// This method provides full type safety for both input and output types,
    /// which is useful for testing, documentation, and API contracts.
    /// Note that output schemas are not part of the MCP protocol but can be
    /// valuable for development and integration testing.
    ///
    /// # Type Parameters
    ///
    /// * `TIn` - Input type that implements `JsonSchema`, `Deserialize`, `Send`, `Sync`
    /// * `TOut` - Output type that implements `JsonSchema`, `Serialize`, `Send`, `Sync`
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::{ServerBuilder, TypedToolWithOutput};
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(JsonSchema, Deserialize)]
    /// struct MathInput { a: f64, b: f64, op: String }
    ///
    /// #[derive(JsonSchema, Serialize)]
    /// struct MathOutput { result: f64, operation: String }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_with_output::<MathInput, MathOutput>("math", |args, _| {
    ///         Box::pin(async move {
    ///             let result = match args.op.as_str() {
    ///                 "add" => args.a + args.b,
    ///                 "subtract" => args.a - args.b,
    ///                 _ => return Err(pmcp::Error::Validation("Unknown operation".into())),
    ///             };
    ///             Ok(MathOutput {
    ///                 result,
    ///                 operation: args.op,
    ///             })
    ///         })
    ///     });
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_with_output<TIn, TOut>(
        mut self,
        name: impl Into<String>,
        handler: impl Fn(
                TIn,
                crate::RequestHandlerExtra,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<TOut>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        TIn: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        TOut: serde::Serialize + schemars::JsonSchema + Send + Sync + 'static,
    {
        use crate::server::typed_tool::TypedToolWithOutput;

        let name_str = name.into();
        let tool = TypedToolWithOutput::new(name_str.clone(), handler);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Register a tool whose async closure returns a full [`CallToolResult`] the
    /// handler owns end-to-end, emitted to the wire **VERBATIM**.
    ///
    /// This mirrors [`tool_typed_with_output`](Self::tool_typed_with_output) but
    /// fixes the return type to
    /// [`CallToolResult`], so a handler can attach
    /// task augmentation (`CallToolResult::with_related_task(...)`), custom
    /// `_meta`, structured content, or an error envelope in ONE call — no
    /// hand-written [`ToolHandler`] `impl` required. The input
    /// arg type `TIn` deserializes from the tool arguments exactly as with
    /// [`tool_typed`](Self::tool_typed).
    ///
    /// # ⚠️ BYPASS WARNING — the returned result is sent to the wire VERBATIM
    ///
    /// The closure's [`CallToolResult`] is routed
    /// through [`ToolOutput::Result`] and
    /// therefore **BYPASSES response middleware** — redaction, sanitization, and
    /// audit hooks (`ToolMiddleware::on_response`) DO NOT run — as well as
    /// text-wrapping and widget enrichment. The handler owns its OWN redaction
    /// and sanitization of both `content` and `_meta`, at the same trust level as
    /// returning a raw `Value` today (D-04a). **Request** middleware still runs
    /// before the handler, and handler errors still route through the normal
    /// error path.
    ///
    /// To advertise a human-readable tool description in `tools/list`, use
    /// [`tool_with_result_and_description`](Self::tool_with_result_and_description).
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use pmcp::types::CallToolResult;
    /// use pmcp::types::tasks::TaskMetadata;
    /// use pmcp::types::Content;
    /// use schemars::JsonSchema;
    /// use serde::Deserialize;
    ///
    /// #[derive(JsonSchema, Deserialize)]
    /// struct RunArgs { job: String }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_with_result("start_job", |args: RunArgs, _extra| {
    ///         Box::pin(async move {
    ///             Ok(CallToolResult::new(vec![Content::text(
    ///                 format!("started {}", args.job),
    ///             )])
    ///             .with_related_task(TaskMetadata::new("t1")))
    ///         })
    ///     })
    ///     .build();
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_with_result<TIn>(
        mut self,
        name: impl Into<String>,
        handler: impl Fn(
                TIn,
                crate::RequestHandlerExtra,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = crate::Result<crate::types::CallToolResult>>
                        + Send,
                >,
            > + Send
            + Sync
            + 'static,
    ) -> Self
    where
        TIn: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
    {
        use crate::server::typed_tool::TypedToolWithResult;

        let name_str = name.into();
        let tool = TypedToolWithResult::new(name_str.clone(), handler);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// [`tool_with_result`](Self::tool_with_result) WITH a human-readable
    /// description advertised in `tools/list`.
    ///
    /// Identical to [`tool_with_result`](Self::tool_with_result) — including
    /// the **BYPASS WARNING** documented there (the returned
    /// [`CallToolResult`] goes to the wire
    /// VERBATIM and skips response middleware) — but also sets the tool
    /// description, mirroring
    /// [`tool_typed_with_description`](Self::tool_typed_with_description).
    /// A description materially improves LLM tool selection; prefer this
    /// overload over the description-less
    /// [`tool_with_result`](Self::tool_with_result).
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use pmcp::types::CallToolResult;
    /// use pmcp::types::tasks::TaskMetadata;
    /// use pmcp::types::Content;
    /// use schemars::JsonSchema;
    /// use serde::Deserialize;
    ///
    /// #[derive(JsonSchema, Deserialize)]
    /// struct RunArgs { job: String }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_with_result_and_description(
    ///         "start_job",
    ///         "Start a background export job and return its task handle",
    ///         |args: RunArgs, _extra| {
    ///             Box::pin(async move {
    ///                 Ok(CallToolResult::new(vec![Content::text(
    ///                     format!("started {}", args.job),
    ///                 )])
    ///                 .with_related_task(TaskMetadata::new("t1")))
    ///             })
    ///         },
    ///     )
    ///     .build();
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_with_result_and_description<TIn>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: impl Fn(
                TIn,
                crate::RequestHandlerExtra,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = crate::Result<crate::types::CallToolResult>>
                        + Send,
                >,
            > + Send
            + Sync
            + 'static,
    ) -> Self
    where
        TIn: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
    {
        use crate::server::typed_tool::TypedToolWithResult;

        let name_str = name.into();
        let tool =
            TypedToolWithResult::new(name_str.clone(), handler).with_description(description);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a type-safe tool handler with both input and output typing and description.
    ///
    /// This is a convenience overload that allows setting a description directly
    /// without needing to chain `.with_description()`.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(JsonSchema, Deserialize)]
    /// struct MathInput { a: f64, b: f64, op: String }
    ///
    /// #[derive(JsonSchema, Serialize)]
    /// struct MathOutput { result: f64, operation: String }
    ///
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_with_output_and_description::<MathInput, MathOutput>(
    ///         "math",
    ///         "Performs basic mathematical operations on two numbers",
    ///         |args, _| {
    ///             Box::pin(async move {
    ///                 let result = match args.op.as_str() {
    ///                     "add" => args.a + args.b,
    ///                     "subtract" => args.a - args.b,
    ///                     _ => return Err(pmcp::Error::Validation("Unknown operation".into())),
    ///                 };
    ///                 Ok(MathOutput { result, operation: args.op })
    ///             })
    ///         }
    ///     );
    /// # }
    /// ```
    #[cfg(feature = "schema-generation")]
    pub fn tool_typed_with_output_and_description<TIn, TOut>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: impl Fn(
                TIn,
                crate::RequestHandlerExtra,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<TOut>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> Self
    where
        TIn: serde::de::DeserializeOwned + schemars::JsonSchema + Send + Sync + 'static,
        TOut: serde::Serialize + schemars::JsonSchema + Send + Sync + 'static,
    {
        use crate::server::typed_tool::TypedToolWithOutput;

        let name_str = name.into();
        let tool =
            TypedToolWithOutput::new(name_str.clone(), handler).with_description(description);
        self.tools.insert(name_str, Arc::new(tool));

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a prompt handler.
    ///
    /// Registers a prompt that clients can retrieve via the prompts/get method.
    /// Prompts provide templates that clients can use for various tasks.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt (used by clients to retrieve it)
    /// * `handler` - The handler implementation for this prompt
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, PromptHandler, GetPromptResult, PromptMessage, Content};
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    ///
    /// struct CodeReviewPrompt;
    ///
    /// #[async_trait]
    /// impl PromptHandler for CodeReviewPrompt {
    ///     async fn handle(&self, args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
    ///         let language = args.get("language").map(|s| s.as_str()).unwrap_or("unknown");
    ///         Ok(GetPromptResult::new(
    ///             vec![PromptMessage::user(pmcp::Content::text(format!(
    ///                 "Please review this {} code:",
    ///                 language
    ///             )))],
    ///             Some(format!("Code review prompt for {}", language)),
    ///         ))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("code-server")
    ///     .version("1.0.0")
    ///     .prompt("code_review", CodeReviewPrompt{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn prompt(
        mut self,
        name: impl Into<String>,
        handler: impl PromptHandler + 'static,
    ) -> Self {
        self.prompts.insert(name.into(), Arc::new(handler));

        // Update capabilities to include prompts
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a prompt handler with an Arc.
    ///
    /// This variant lets the caller share the handler `Arc` between the
    /// builder and an external in-process handler map (e.g., a downstream
    /// toolkit's handler registry) without writing a delegating wrapper
    /// shim. Behavior is otherwise identical to [`Self::prompt`]: the first
    /// registration auto-enables `capabilities.prompts`.
    pub fn prompt_arc(mut self, name: impl Into<String>, handler: Arc<dyn PromptHandler>) -> Self {
        let name = name.into();
        self.prompts.insert(name, handler);

        // Update capabilities to include prompts
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Register a workflow-based prompt with automatic validation.
    ///
    /// This method validates the workflow before registration and converts it
    /// to a prompt handler. The workflow's instructions become the prompt messages,
    /// and the workflow's arguments become the prompt arguments.
    ///
    /// # Arguments
    ///
    /// * `workflow` - The workflow definition to register as a prompt
    ///
    /// # Errors
    ///
    /// Returns an error if the workflow validation fails (e.g., undefined bindings,
    /// undefined prompt arguments, etc.).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ServerBuilder};
    /// use pmcp::server::workflow::{SequentialWorkflow, InternalPromptMessage};
    /// use pmcp::types::Role;
    ///
    /// # fn main() -> pmcp::Result<()> {
    /// let workflow = SequentialWorkflow::new(
    ///     "code_review_workflow",
    ///     "Review code with multiple steps"
    /// )
    /// .argument("code", "Code to review", true)
    /// .instruction(InternalPromptMessage::new(
    ///     Role::System,
    ///     "You are a code reviewer. Review the provided code carefully."
    /// ));
    ///
    /// let server = Server::builder()
    ///     .name("code-server")
    ///     .version("1.0.0")
    ///     .prompt_workflow(workflow)?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prompt_workflow(mut self, workflow: workflow::SequentialWorkflow) -> Result<Self> {
        // Validate the workflow before registration
        workflow
            .validate()
            .map_err(|e| Error::Validation(format!("Workflow validation failed: {}", e)))?;

        // Build tool and resource registries from currently registered handlers
        // Note: This captures the current state of registered tools/resources
        let mut tools = std::collections::HashMap::new();
        for (name, handler) in &self.tools {
            if let Some(metadata) = handler.metadata() {
                tools.insert(
                    Arc::from(name.as_str()),
                    workflow::conversion::ToolInfo {
                        name: metadata.name,
                        description: metadata.description.unwrap_or_default(),
                        input_schema: metadata.input_schema,
                    },
                );
            }
        }

        // Build tool handlers map for workflow execution
        // Clone Arc references for shared ownership
        let mut tool_handlers: std::collections::HashMap<Arc<str>, Arc<dyn ToolHandler>> =
            std::collections::HashMap::new();
        for (name, handler) in &self.tools {
            tool_handlers.insert(Arc::from(name.as_str()), Arc::clone(handler));
        }

        // Get the workflow name before moving it
        let name = workflow.name().to_string();

        // Create workflow prompt handler with tool execution and resource fetching capability
        // Note: Workflow prompts in ServerBuilder do not currently execute tool middleware.
        // For middleware support in workflow tool execution, use ServerCoreBuilder.
        let handler = workflow::WorkflowPromptHandler::new(
            workflow,
            tools,
            tool_handlers,
            self.resources.clone(),
        );

        // Register as a prompt
        self.prompts.insert(name, Arc::new(handler));

        // Update capabilities to include prompts
        // This ensures prompts/list returns the workflow prompts
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        Ok(self)
    }

    /// Set the resource handler.
    ///
    /// Registers a resource handler that provides access to server resources.
    /// Resources allow clients to read files, configurations, or other data.
    ///
    /// # Arguments
    ///
    /// * `handler` - The resource handler implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ResourceHandler, ReadResourceResult, ListResourcesResult, ResourceInfo};
    /// use async_trait::async_trait;
    ///
    /// struct FileResourceHandler;
    ///
    /// #[async_trait]
    /// impl ResourceHandler for FileResourceHandler {
    ///     async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<ReadResourceResult> {
    ///         // Read file content...
    ///         Ok(ReadResourceResult::new(vec![pmcp::Content::text("File content here")]))
    ///     }
    ///
    ///     async fn list(&self, _cursor: Option<String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<ListResourcesResult> {
    ///         Ok(ListResourcesResult::new(vec![
    ///             pmcp::ResourceInfo::new("file://example.txt", "example.txt")
    ///                 .with_description("Example file")
    ///                 .with_mime_type("text/plain"),
    ///         ]))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .resources(FileResourceHandler{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn resources(mut self, handler: impl ResourceHandler + 'static) -> Self {
        self.resources = Some(Arc::new(handler));

        // Update capabilities to include resources
        // Use Some(false) instead of None to ensure fields serialize properly
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }

        self
    }

    /// Set the resource handler with an Arc.
    ///
    /// This variant lets the caller share the handler `Arc` between the
    /// builder and an external in-process handler map without writing a
    /// delegating wrapper. Behavior is otherwise identical to
    /// [`Self::resources`]: the first registration auto-enables
    /// `capabilities.resources`.
    pub fn resources_arc(mut self, handler: Arc<dyn ResourceHandler>) -> Self {
        self.resources = Some(handler);

        // Update capabilities to include resources
        // Use Some(false) instead of None to ensure fields serialize properly
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }

        self
    }

    /// Set the completion provider backing `completion/complete`.
    ///
    /// The twin of
    /// [`ServerCoreBuilder::completions`](crate::server::builder::ServerCoreBuilder::completions) —
    /// same name, same signature, same single-provider shape — so a provider
    /// registered through EITHER builder family reaches its own dispatcher. A
    /// slot on one family with the dispatch arm on the other's server would be
    /// an unreachable seam that still answered the spec shape, which is exactly
    /// the false green this pair exists to prevent.
    ///
    /// A SINGLE, server-wide provider (the [`Self::resources`] shape, not the
    /// name-keyed [`Self::prompt`] shape): the spec routes every
    /// `completion/complete` to one seam and passes the `ref` as data. The
    /// reference reaches the provider through
    /// [`CompletionRequest::context`](crate::types::completable::CompletionRequest::context)
    /// under the key `ref/prompt` or `ref/resource`.
    ///
    /// Registering a provider auto-advertises `capabilities.completions`.
    /// Not registering one is NOT an error: `completion/complete` still answers
    /// `{"completion": {"values": []}}`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Server;
    /// use pmcp::types::completable::StaticCompletionProvider;
    ///
    /// let server = Server::builder()
    ///     .name("completion-server")
    ///     .version("1.0.0")
    ///     .completions(StaticCompletionProvider::from_strings(vec![
    ///         "alpha".to_string(),
    ///         "beta".to_string(),
    ///     ]))
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    #[must_use]
    pub fn completions(
        self,
        provider: impl crate::types::completable::CompletionProviderTrait + 'static,
    ) -> Self {
        self.completions_arc(Arc::new(provider))
    }

    /// Set the completion provider with an Arc.
    ///
    /// This variant lets the caller share the provider `Arc` with something
    /// outside the builder. Behavior is otherwise identical to
    /// [`Self::completions`].
    #[must_use]
    pub fn completions_arc(
        mut self,
        provider: Arc<dyn crate::types::completable::CompletionProviderTrait>,
    ) -> Self {
        self.completions = Some(provider);

        // Update capabilities to include completions.
        // Use Some(default) instead of None to ensure the field serializes.
        if self.capabilities.completions.is_none() {
            self.capabilities.completions = Some(crate::types::CompletionCapabilities::default());
        }

        self
    }

    /// Register a single SEP-2640 Agent Skill.
    ///
    /// Convenience over [`Self::skills`] for the single-skill case. The skill
    /// is accumulated and finalized into a `SkillsHandler` exactly once at
    /// [`Self::build`] time, then composed with any `.resources(...)`
    /// handler set on this builder.
    ///
    /// # Panics
    ///
    /// Panics at `.build()` time if multiple registered skills resolve to
    /// the same `skill://` URI. Use [`Self::try_skills`] with a pre-built
    /// [`skills::Skills`] registry to surface duplicates as a `Result`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::{Server, server::skills::Skill};
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .skill(Skill::new("hello", "# Hello skill"))
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    #[cfg(feature = "skills")]
    #[must_use]
    pub fn skill(self, skill: skills::Skill) -> Self {
        self.skills(skills::Skills::new().add(skill))
    }

    /// Register a registry of SEP-2640 Agent Skills.
    ///
    /// Merges into any prior accumulated skills (a previous `.skill(...)` or
    /// `.skills(...)` call). The accumulated registry is finalized into a
    /// single `SkillsHandler` exactly once at [`Self::build`] time, then
    /// composed at most once with any `.resources(...)` handler.
    ///
    /// # Panics
    ///
    /// Panics at `.build()` if two registered skills resolve to the same
    /// `skill://` URI. Use [`Self::try_skills`] for fallible registration.
    #[cfg(feature = "skills")]
    #[must_use]
    pub fn skills(mut self, skills_registry: skills::Skills) -> Self {
        let merged = match self.pending_skills.take() {
            Some(prior) => prior.merge(skills_registry),
            None => skills_registry,
        };
        self.pending_skills = Some(merged);
        skills::set_skills_capabilities(&mut self.capabilities);
        self
    }

    /// Fallible variant of [`Self::skills`] — returns `Err` immediately if
    /// the merged registry would contain duplicate URIs. Useful for
    /// runtime-dynamic registration where panicking is unacceptable.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the merged registry would
    /// produce duplicate `skill://` URIs.
    #[cfg(feature = "skills")]
    pub fn try_skills(mut self, skills_registry: skills::Skills) -> Result<Self> {
        let merged = match self.pending_skills.take() {
            Some(prior) => prior.merge(skills_registry),
            None => skills_registry,
        };
        // Probe by cloning + into_handler; discard the handler. The real
        // construction happens in `.build()` once everything is settled.
        merged.clone().into_handler()?;
        self.pending_skills = Some(merged);
        skills::set_skills_capabilities(&mut self.capabilities);
        Ok(self)
    }

    /// Register a skill AND a parallel prompt that returns the same content.
    ///
    /// The dual-surface bootstrap: both surfaces are derived from one
    /// [`skills::Skill`] value so they cannot drift. The byte-equality
    /// between surfaces is asserted by the skills integration test.
    #[cfg(feature = "skills")]
    #[must_use]
    pub fn bootstrap_skill_and_prompt(
        self,
        skill: skills::Skill,
        prompt_name: impl Into<String>,
    ) -> Self {
        let prompt_handler = skills::SkillPromptHandler::new(skill.clone());
        self.skill(skill).prompt(prompt_name, prompt_handler)
    }

    /// Set the sampling handler.
    ///
    /// Registers a sampling handler that provides LLM functionality.
    /// This allows the server to act as a language model provider.
    ///
    /// # Arguments
    ///
    /// * `handler` - The sampling handler implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, SamplingHandler, CreateMessageParams, CreateMessageResult};
    /// use async_trait::async_trait;
    ///
    /// struct MockLLM;
    ///
    /// #[async_trait]
    /// impl SamplingHandler for MockLLM {
    ///     async fn create_message(&self, params: CreateMessageParams, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<CreateMessageResult> {
    ///         // Process the messages and generate a response
    ///         Ok(CreateMessageResult::new(pmcp::Content::text("Generated response"), "mock-llm-v1")
    ///             .with_usage(pmcp::TokenUsage::new(10, 5, 15))
    ///             .with_stop_reason("end_of_text"))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("llm-server")
    ///     .version("1.0.0")
    ///     .sampling(MockLLM{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn sampling(mut self, handler: impl SamplingHandler + 'static) -> Self {
        self.sampling = Some(Arc::new(handler));
        // Enable sampling capability
        self.capabilities.sampling = Some(crate::types::SamplingCapabilities::default());
        self
    }

    /// Set the sampling handler with an Arc.
    ///
    /// This variant lets the caller share the handler `Arc` between the
    /// builder and an external in-process handler map without writing a
    /// delegating wrapper. Uses the donor's `if is_none` capability
    /// auto-enable so an explicit prior `.capabilities(custom)` is not
    /// clobbered by a later `_arc` registration.
    pub fn sampling_arc(mut self, handler: Arc<dyn SamplingHandler>) -> Self {
        self.sampling = Some(handler);

        // Update capabilities to include sampling
        if self.capabilities.sampling.is_none() {
            self.capabilities.sampling = Some(crate::types::SamplingCapabilities::default());
        }

        self
    }

    /// Build the server.
    ///
    /// Constructs the final Server instance from the configured builder.
    /// This validates that required fields (name and version) are set.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct PingTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for PingTool {
    ///     async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(serde_json::json!({"response": "pong"}))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("ping-server")
    ///     .version("1.0.0")
    ///     .tool("ping", PingTool{})
    ///     .build()?;
    ///
    /// // Server is now ready to run
    /// // server.run_stdio().await?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    /// Set the authentication provider.
    ///
    /// Configures an authentication provider that will validate incoming requests.
    /// When set, the server will use this provider to authenticate requests before
    /// processing them.
    ///
    /// # Arguments
    ///
    /// * `provider` - The authentication provider implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, auth::ProxyProvider};
    ///
    /// let auth_provider = ProxyProvider::with_upstream("https://oauth.example.com");
    ///
    /// let server = Server::builder()
    ///     .name("secure-server")
    ///     .version("1.0.0")
    ///     .auth_provider(auth_provider)
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn auth_provider(mut self, provider: impl auth::AuthProvider + 'static) -> Self {
        self.auth_provider = Some(Arc::new(provider));
        self
    }

    /// Set the authentication provider with an Arc.
    ///
    /// This variant lets the caller share the provider `Arc` between the
    /// builder and an external in-process registry without writing a
    /// delegating wrapper. Behavior is otherwise identical to
    /// [`Self::auth_provider`].
    pub fn auth_provider_arc(mut self, provider: Arc<dyn auth::AuthProvider>) -> Self {
        self.auth_provider = Some(provider);
        self
    }

    /// Set the tool authorizer.
    ///
    /// Configures a tool authorizer for fine-grained access control.
    /// The authorizer determines which tools authenticated users can access
    /// based on their authentication context.
    ///
    /// # Arguments
    ///
    /// * `authorizer` - The tool authorization implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, auth::ScopeBasedAuthorizer};
    ///
    /// let authorizer = ScopeBasedAuthorizer::new()
    ///     .require_scopes("sensitive_tool", vec!["admin".to_string()])
    ///     .default_scopes(vec!["read".to_string()]);
    ///
    /// let server = Server::builder()
    ///     .name("secure-server")
    ///     .version("1.0.0")
    ///     .tool_authorizer(authorizer)
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn tool_authorizer(mut self, authorizer: impl auth::ToolAuthorizer + 'static) -> Self {
        if !self.tool_protections.is_empty() {
            // Log a warning - custom authorizer supersedes protect_tool() configurations
            tracing::warn!(
                target: "mcp.auth",
                "Setting a custom tool_authorizer clears any previous protect_tool() configurations"
            );
            self.tool_protections.clear();
        }
        self.tool_authorizer = Some(Arc::new(authorizer));
        self
    }

    /// Set the tool authorizer with an Arc.
    ///
    /// This variant lets the caller share the authorizer `Arc` between
    /// the builder and an external in-process registry without writing a
    /// delegating wrapper. Mirrors [`Self::tool_authorizer`]'s
    /// protection-clearing semantics: if any prior `protect_tool()`
    /// configurations exist, they are cleared and a `tracing::warn!` is
    /// emitted under target `"mcp.auth"`, since a custom authorizer
    /// supersedes scope-based tool protections.
    pub fn tool_authorizer_arc(mut self, authorizer: Arc<dyn auth::ToolAuthorizer>) -> Self {
        if !self.tool_protections.is_empty() {
            // Log a warning - custom authorizer supersedes protect_tool() configurations
            tracing::warn!(
                target: "mcp.auth",
                "Setting a custom tool_authorizer clears any previous protect_tool() configurations"
            );
            self.tool_protections.clear();
        }
        self.tool_authorizer = Some(authorizer);
        self
    }

    /// Protect a specific tool with required scopes.
    ///
    /// This is a convenience method that creates or updates a scope-based authorizer
    /// to require specific scopes for accessing the named tool.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to protect
    /// * `scopes` - The required scopes for accessing this tool
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// let server = Server::builder()
    ///     .name("secure-server")
    ///     .version("1.0.0")
    ///     .protect_tool("delete_data", vec!["admin".to_string(), "write".to_string()])
    ///     .protect_tool("read_data", vec!["read".to_string()])
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn protect_tool(mut self, tool_name: impl Into<String>, scopes: Vec<String>) -> Self {
        // Store the tool protection requirements to be applied at build time
        self.tool_protections.insert(tool_name.into(), scopes);
        self
    }

    /// Add tool middleware for cross-cutting concerns.
    ///
    /// Tool middleware allows you to inject cross-cutting concerns into tool execution,
    /// such as OAuth token injection, logging, metrics, or request transformation.
    /// Middleware is executed in the order it's added, both for request processing
    /// (before tool execution) and response processing (after tool execution).
    ///
    /// This method brings middleware support to the high-level `ServerBuilder` API,
    /// enabling developers to use both typed tool registration AND middleware without
    /// dropping down to the lower-level `ServerCoreBuilder` API.
    ///
    /// # Arguments
    ///
    /// * `middleware` - The middleware implementation to add to the chain
    ///
    /// # Examples
    ///
    /// ## OAuth Token Injection Middleware
    ///
    /// ```rust,no_run
    /// use pmcp::server::tool_middleware::{ToolMiddleware, ToolContext};
    /// use pmcp::server::cancellation::RequestHandlerExtra;
    /// use pmcp::Server;
    /// use std::sync::Arc;
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct OAuthInjectionMiddleware;
    ///
    /// #[async_trait]
    /// impl ToolMiddleware for OAuthInjectionMiddleware {
    ///     async fn on_request(
    ///         &self,
    ///         _tool_name: &str,
    ///         _args: &mut Value,
    ///         extra: &mut RequestHandlerExtra,
    ///         _context: &ToolContext,
    ///     ) -> pmcp::Result<()> {
    ///         // Extract OAuth token from auth_context and inject into metadata
    ///         if let Some(auth_ctx) = extra.auth_context() {
    ///             if let Some(token) = &auth_ctx.token {
    ///                 extra.set_metadata("oauth_token".to_string(), token.clone());
    ///             }
    ///         }
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("oauth-server")
    ///     .version("1.0.0")
    ///     .tool_middleware(Arc::new(OAuthInjectionMiddleware))
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    ///
    /// ## Combining with Typed Tools
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::Server;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct ListGamesArgs {
    ///     filter: Option<String>,
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("game-server")
    ///     .version("1.0.0")
    ///     .tool_typed_with_description(
    ///         "list_games",
    ///         "List all available games",
    ///         |args: ListGamesArgs, extra| {
    ///             Box::pin(async move {
    ///                 // Access OAuth token injected by middleware
    ///                 let _token = extra.get_metadata("oauth_token");
    ///                 Ok(serde_json::json!({"games": []}))
    ///             })
    ///         }
    ///     )
    ///     // .tool_middleware(Arc::new(oauth_middleware))  // Works with typed tools!
    ///     .build()?;
    /// # }
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    ///
    /// # Middleware Execution Order
    ///
    /// Multiple middleware are executed in FIFO order for requests and FIFO for responses:
    ///
    /// ```text
    /// Request:  Middleware1 → Middleware2 → Tool Handler
    /// Response: Tool Handler → Middleware1 → Middleware2
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tool_middleware(mut self, middleware: Arc<dyn tool_middleware::ToolMiddleware>) -> Self {
        self.tool_middlewares.push(middleware);
        self
    }

    /// Enable observability for this server.
    ///
    /// This adds observability middleware that provides:
    /// - Distributed tracing with trace/span IDs
    /// - Request/response event logging
    /// - Metrics emission (duration, count, errors)
    ///
    /// The backend is automatically selected based on the configuration:
    /// - "console" - Pretty or JSON output to stdout (development)
    /// - "cloudwatch" - AWS `CloudWatch` EMF format (production)
    /// - "null" - Discards all events (testing)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    /// use pmcp::server::observability::ObservabilityConfig;
    ///
    /// // Development: console output with pretty printing
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(ObservabilityConfig::development())
    ///     .build()?;
    ///
    /// // Production: CloudWatch with EMF metrics
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(ObservabilityConfig::production())
    ///     .build()?;
    ///
    /// // Auto-detect environment (Lambda vs local)
    /// let config = if std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok() {
    ///     ObservabilityConfig::production()
    /// } else {
    ///     ObservabilityConfig::development()
    /// };
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(config)
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_observability(mut self, config: observability::ObservabilityConfig) -> Self {
        if !config.enabled {
            return self;
        }

        // Create backend based on configuration
        let backend: Arc<dyn observability::ObservabilityBackend> = match config.backend.as_str() {
            "cloudwatch" => Arc::new(observability::CloudWatchBackend::new(
                config.cloudwatch.clone(),
            )),
            "null" => Arc::new(observability::NullBackend),
            _ => Arc::new(observability::ConsoleBackend::new(config.console.pretty)),
        };

        // Get server name for middleware (use placeholder if not yet set)
        let server_name = self.name.clone().unwrap_or_else(|| "unknown".to_string());

        // Create and add the observability middleware
        let middleware =
            observability::McpObservabilityMiddleware::new(server_name, config, backend);
        self.tool_middlewares.push(Arc::new(middleware));

        self
    }

    /// Enable observability with a custom backend.
    ///
    /// Use this when you need a custom backend implementation (e.g., Datadog, custom metrics).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::Server;
    /// use pmcp::server::observability::{ObservabilityConfig, ObservabilityBackend};
    /// use std::sync::Arc;
    ///
    /// struct MyCustomBackend;
    ///
    /// #[async_trait]
    /// impl ObservabilityBackend for MyCustomBackend {
    ///     // ... custom implementation
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability_backend(
    ///         ObservabilityConfig::development(),
    ///         Arc::new(MyCustomBackend),
    ///     )
    ///     .build()?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_observability_backend(
        mut self,
        config: observability::ObservabilityConfig,
        backend: Arc<dyn observability::ObservabilityBackend>,
    ) -> Self {
        if !config.enabled {
            return self;
        }

        // Get server name for middleware (use placeholder if not yet set)
        let server_name = self.name.clone().unwrap_or_else(|| "unknown".to_string());

        // Create and add the observability middleware
        let middleware =
            observability::McpObservabilityMiddleware::new(server_name, config, backend);
        self.tool_middlewares.push(Arc::new(middleware));

        self
    }

    /// Add a description to a tool (Note: Limited support).
    ///
    /// **Important**: Due to the immutable design of tool handlers, this method
    /// cannot retroactively add descriptions to already-registered tools.
    ///
    /// **Recommended**: Use the `*_with_description` variants instead:
    /// - `.tool_typed_with_description()`
    /// - `.tool_typed_sync_with_description()`
    /// - `.tool_typed_with_output_and_description()`
    ///
    /// This method is provided for API completeness but will log warnings
    /// when used, encouraging migration to the preferred approaches.
    ///
    /// # Preferred Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "schema-generation")]
    /// # {
    /// use pmcp::ServerBuilder;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    /// struct MathArgs { a: f64, b: f64 }
    ///
    /// // Preferred: Use the direct description variants
    /// let server = ServerBuilder::new()
    ///     .name("example")
    ///     .tool_typed_with_description(
    ///         "add",
    ///         "Adds two numbers together",
    ///         |args: MathArgs, _| {
    ///             Box::pin(async move {
    ///                 Ok(serde_json::json!({ "result": args.a + args.b }))
    ///             })
    ///         }
    ///     )
    ///     .build();
    /// # }
    /// ```
    #[deprecated(
        since = "1.6.0",
        note = "Use tool_typed_with_description() and similar variants instead"
    )]
    pub fn with_tool_description(
        self,
        tool_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let tool_name = tool_name.into();
        let _description = description.into();

        tracing::warn!(
            "with_tool_description('{}') called but cannot modify immutable tools. \
            Use tool_typed_with_description() variants instead.",
            tool_name
        );

        self
    }

    /// Configure HTTP middleware chain for `StreamableHttpServer`.
    ///
    /// This is a convenience method that stores the HTTP middleware chain
    /// so it can be retrieved later when creating a `StreamableHttpServer`.
    ///
    /// # Arguments
    ///
    /// * `middleware` - The HTTP middleware chain
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "streamable-http")]
    /// # fn example() -> Result<(), pmcp::Error> {
    /// use pmcp::Server;
    /// use pmcp::server::http_middleware::{ServerHttpLoggingMiddleware, ServerHttpMiddlewareChain};
    /// use std::sync::Arc;
    ///
    /// let mut http_chain = ServerHttpMiddlewareChain::new();
    /// http_chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));
    ///
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_http_middleware(Arc::new(http_chain))
    ///     .build()?;
    ///
    /// // Later when creating StreamableHttpServer:
    /// // let config = StreamableHttpServerConfig {
    /// //     http_middleware: server.http_middleware(),
    /// //     ..Default::default()
    /// // };
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "streamable-http")]
    pub fn with_http_middleware(
        mut self,
        middleware: Arc<http_middleware::ServerHttpMiddlewareChain>,
    ) -> Self {
        self.http_middleware = Some(middleware);
        self
    }

    /// Add a host layer for MCP Apps metadata enrichment.
    ///
    /// Host layers enrich tool `_meta` at build time with host-specific keys.
    /// For example, `HostType::ChatGpt` adds `openai/outputTemplate` and
    /// `openai/widgetAccessible` derived from the standard `ui.resourceUri`.
    ///
    /// This is opt-in — standard MCP Apps hosts (Claude Desktop, etc.) work
    /// without any host layer. Duplicates are ignored.
    #[cfg(feature = "mcp-apps")]
    pub fn with_host_layer(mut self, host: crate::types::mcp_apps::HostType) -> Self {
        if !self.host_layers.contains(&host) {
            self.host_layers.push(host);
        }
        self
    }

    /// Build the server.
    ///
    /// Constructs the final Server instance from the configured builder.
    /// This validates that required fields (name and version) are set.
    ///
    /// # Errors
    ///
    /// Register a [`TaskStore`](crate::server::task_store::TaskStore) for MCP
    /// Tasks on the high-level HTTP-facing `Server` (RECOMMENDED tools-as-Tasks
    /// path).
    ///
    /// This is the recommended, all-typed path for exposing a tool as an async
    /// MCP Task over the `Server` / `StreamableHttpServer` path: pair a
    /// task-capable [`TypedTool`](crate::server::typed_tool::TypedTool) (marked
    /// [`with_task_support(TaskSupport::Required)`](crate::types::ToolExecution::with_task_support))
    /// with a store here, and the SDK serves `tasks/*` typed from the store —
    /// you never hand-write `tasks/*` wire JSON, and the store mints the task id.
    /// For the legacy experimental router path, use [`Self::with_task_store`]
    /// (which takes a [`TaskRouter`](crate::server::tasks::TaskRouter), NOT a
    /// `TaskStore`).
    ///
    /// When a task store is registered, the server:
    /// - **Auto-advertises** `ServerCapabilities.tasks` (with list and cancel
    ///   support) in `initialize` — the mere presence of a store flips the
    ///   capability on, unless an explicit `tasks` capability was already
    ///   configured (additive-only; an explicit value is preserved verbatim).
    /// - Handles the `tasks/*` surface via the store. The method set is
    ///   ERA-DEPENDENT (Phase 114): v1 (2025-11-25) serves `tasks/get`,
    ///   `tasks/result`, `tasks/list` and `tasks/cancel`; v2 (2026-07-28)
    ///   serves `tasks/get`, `tasks/update` and `tasks/cancel`, and answers
    ///   `-32601` for the two retired methods
    /// - Resolves task owner from auth context. **v1** falls back through OAuth
    ///   subject → client ID → session ID; **v2** has no session to fall back
    ///   to and binds fail-closed on an auth-configured server (TASK-05, D-07)
    ///
    /// A tool declaring
    /// [`TaskSupport::Required`](crate::types::tools::TaskSupport::Required)
    /// with NO store (or router) makes [`Self::build`] return an `Err`, rather
    /// than advertising a hollow `tasks` capability whose endpoints cannot work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
    /// use pmcp::server::typed_tool::TypedTool;
    /// use pmcp::types::{TaskSupport, ToolExecution};
    ///
    /// # fn build() -> pmcp::Result<()> {
    /// let task_tool = TypedTool::new_with_schema(
    ///     "summarize",
    ///     serde_json::json!({ "type": "object" }),
    ///     |_args: serde_json::Value, _extra| {
    ///         Box::pin(async { Ok(serde_json::json!({ "status": "completed" })) })
    ///     },
    /// )
    /// .with_description("Summarize asynchronously as an MCP Task")
    /// .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));
    ///
    /// let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .tool("summarize", task_tool)
    ///     .task_store(store) // presence of a store auto-advertises the `tasks` capability
    ///     .build()?;
    /// # let _ = server;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn task_store(mut self, store: Arc<dyn crate::server::task_store::TaskStore>) -> Self {
        // Capability advertisement is centralized in `build()` (see
        // `task_dispatch::apply_tasks_capability_rule`). Registering a store
        // records the backend; it does NOT itself set `capabilities.tasks`, so
        // an explicitly-configured capability is never clobbered (additive-only,
        // per D-CAPABILITY-ENDPOINT-BACKED).
        self.task_store = Some(store);
        self
    }

    /// Register a legacy experimental
    /// [`TaskRouter`](crate::server::tasks::TaskRouter) for MCP Tasks on the
    /// high-level `Server`.
    ///
    /// NAMING NOTE: despite the `with_task_store` name, this setter accepts a
    /// **[`TaskRouter`](crate::server::tasks::TaskRouter)** (the legacy,
    /// experimental router-backed path), NOT a
    /// [`TaskStore`](crate::server::task_store::TaskStore). The setter for an
    /// actual `TaskStore` (the RECOMMENDED polling path) is
    /// [`Self::task_store`]. This carried-over naming mirrors
    /// `ServerCoreBuilder::with_task_store`; the API is additive-only, so the
    /// confusing pair is documented here rather than renamed.
    ///
    /// Registering a router auto-configures the `experimental.tasks` capability
    /// from the router's `task_capabilities()`.
    ///
    /// **That advertisement is v1-only (Phase 114).** `experimental.tasks` is
    /// the 2025-11-25 spelling; a v2 (2026-07-28) client never sees it, because
    /// `project_capabilities_for_v2` strips both `experimental` and
    /// `capabilities.tasks` and v2 declares tasks through the `extensions` map
    /// key `io.modelcontextprotocol/tasks` instead (plan 114-05).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_task_store(mut self, router: Arc<dyn crate::server::tasks::TaskRouter>) -> Self {
        // Auto-configure experimental.tasks capability from the router.
        let experimental = self
            .capabilities
            .experimental
            .get_or_insert_with(HashMap::new);
        experimental.insert("tasks".to_string(), router.task_capabilities());

        self.task_router = Some(router);
        self
    }

    /// Opt a tool OUT of the TOUT-02 double-wrap tripwire (D-08).
    ///
    /// The tripwire WARNs (every build) and `debug_assert!`-fails (debug/CI)
    /// when a tool returns a `ToolOutput::Payload` `Value` that STRUCTURALLY
    /// resembles an already-built `CallToolResult` (a non-empty `content` array
    /// of `Content`, or a `_meta` related-task envelope) — the silent
    /// double-wrap bug. Naming a tool here suppresses that check for it.
    ///
    /// SUPPRESSION SHOULD BE RARE AND REVIEWED: it disables a safety tripwire for
    /// one tool whose LEGITIMATE payload happens to trip the heuristic. Prefer
    /// returning [`ToolOutput::Result`] so the
    /// handler owns the full envelope verbatim, rather than suppressing. Reach
    /// for this only when a tool genuinely produces a plain `Value` that mimics a
    /// result shape and cannot be restructured.
    ///
    /// The same suppression set is carried into `ServerCore`, so both native
    /// dispatchers honor the opt-out identically (no drift).
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn suppress_double_wrap_check(mut self, name: impl Into<String>) -> Self {
        self.suppress_double_wrap.insert(name.into());
        self
    }

    /// Returns an error if:
    /// - The server name is not set
    /// - The server version is not set
    /// - A tool declares `TaskSupport::Required` but no `TaskStore`/`TaskRouter`
    ///   backend is configured (see [`Self::task_store`])
    #[allow(unused_mut)] // `self` is mutated only on non-wasm (capability rule).
    pub fn build(mut self) -> Result<Server> {
        let name = self
            .name
            .ok_or_else(|| crate::Error::validation("Server name is required"))?;
        let version = self
            .version
            .ok_or_else(|| crate::Error::validation("Server version is required"))?;

        // Apply tool protections
        let tool_authorizer = if !self.tool_protections.is_empty() {
            if self.tool_authorizer.is_some() {
                // If there's an existing authorizer and tool protections are specified,
                // this is a configuration error
                return Err(crate::Error::validation(
                    "Cannot use protect_tool() with a custom tool_authorizer. \
                     Either use protect_tool() to configure scope-based authorization, \
                     or provide a custom ToolAuthorizer implementation, but not both.",
                ));
            }
            // Create a ScopeBasedAuthorizer with all the tool protections
            let mut authorizer = auth::ScopeBasedAuthorizer::new();
            for (tool_name, scopes) in self.tool_protections {
                authorizer = authorizer.require_scopes(tool_name, scopes);
            }
            Some(Arc::new(authorizer) as Arc<dyn auth::ToolAuthorizer>)
        } else {
            self.tool_authorizer
        };

        // Initialize tool middleware chain
        #[cfg(not(target_arch = "wasm32"))]
        let tool_middleware_chain = {
            let mut chain = tool_middleware::ToolMiddlewareChain::new();
            for middleware in self.tool_middlewares {
                chain.add(middleware);
            }
            Arc::new(RwLock::new(chain))
        };

        // Build tool_infos cache at construction time (mirrors ServerCore pattern)
        let tool_infos: HashMap<String, ToolInfo> = self
            .tools
            .iter()
            .map(|(name, handler)| {
                let info = handler.metadata().unwrap_or_else(|| {
                    ToolInfo::new(
                        name.clone(),
                        None,
                        serde_json::json!({"type": "object", "properties": {}}),
                    )
                });
                (name.clone(), info)
            })
            .collect();

        // Apply host layer enrichment to tool _meta (e.g., ChatGPT openai/* keys)
        #[cfg(feature = "mcp-apps")]
        let tool_infos = {
            let mut infos = tool_infos;
            for host in &self.host_layers {
                for info in infos.values_mut() {
                    if let Some(meta) = info._meta.as_mut() {
                        core::enrich_meta_for_host(meta, *host);
                    }
                }
            }
            infos
        };

        // Build URI-to-tool-meta index for widget resource _meta propagation
        let uri_to_tool_meta = core::build_uri_to_tool_meta(&tool_infos);

        // Apply the SHARED endpoint-backed `tasks`-capability rule (the SAME
        // free fn `ServerCoreBuilder::build` uses) now that `tool_infos` is
        // finalized: a store-backed `Server` auto-advertises `tasks`, and a
        // `TaskSupport::Required` tool with no backend is a build-time error.
        // Runs BEFORE `self.capabilities` is moved into the `Server` literal.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let has_backend = self.task_store.is_some() || self.task_router.is_some();
            crate::server::task_dispatch::apply_tasks_capability_rule(
                &mut self.capabilities,
                &tool_infos,
                has_backend,
            )?;
        }

        // Finalize accumulated skills exactly once and compose with the
        // user's `.resources(...)` slot if both are set. `.resources(...)`
        // itself stays "last write wins" — composition lives here so the
        // setter's semantics are unchanged for callers that don't use
        // skills.
        #[cfg(feature = "skills")]
        let final_resources: Option<Arc<dyn ResourceHandler>> =
            builder::finalize_skills_resources(self.pending_skills, self.resources);
        #[cfg(not(feature = "skills"))]
        let final_resources = self.resources;

        // HTTP-04: advertising ANY subscription-delivered capability opts this
        // server into serving `subscriptions/listen`, whose registry is
        // INSTANCE-LOCAL. Warn at BUILD time — this is startup, and a silent
        // under-delivery behind a load balancer surfaces no error at runtime
        // (T-113-64).
        //
        // Gated on the v2 opt-in as well as the capability: `subscriptions/listen`
        // is a 2026-07-28-only route, so a v1-only server can never serve it and
        // the warning would be FALSE. It is not a rare corner either —
        // `ServerCapabilities::tools_only()` sets `tools.listChanged = true`, so
        // without this gate essentially every existing pmcp server would print a
        // warning about a stream it does not implement (D-04: zero era behaviour
        // on a non-opted-in server).
        if crate::types::protocol::context::is_v2_opted_in(&self.supported_protocol_versions)
            && crate::types::subscriptions::advertises_subscriptions(&self.capabilities)
        {
            tracing::warn!(
                target: "mcp.subscriptions",
                "a subscription-delivered capability is advertised, so subscriptions/listen \
                 will be SERVED; its registry is INSTANCE-LOCAL, so notifications generated on \
                 another instance are not delivered — supported for single-instance or \
                 sticky-routed deployments only. Polling over Tasks remains the recommended \
                 pmcp enterprise mechanism (D-11)."
            );
        }

        // Resolve the server-owned `requestState` codec EXACTLY ONCE, here at
        // BUILD time (Phase 113, HTTP-02). A malformed CONFIGURED key fails the
        // build; an UNSET key falls back to a per-process key with a WARN emitted
        // from inside `from_env`, which is a genuine STARTUP warning because this
        // is startup. A v1-only server gets `None` and reads no env var at all.
        //
        // Both key arguments go BY REFERENCE, which closes copy 3 of 3
        // (D-113-P): the by-value form manufactured an unscrubbed stack copy on
        // every call. Because they are borrowed rather than moved, the two
        // fields are still owned by `self` here and drop through the zeroizing
        // destructor — on this path AND on every early `?` above, none of which
        // moves the key material anywhere.
        #[cfg(feature = "streamable-http")]
        let request_state_codec = request_state::resolve_codec_at_build(
            &self.supported_protocol_versions,
            self.request_state_key.as_ref(),
            &self.request_state_previous_keys,
            self.request_state_ttl,
        )?;

        Ok(Server {
            info: {
                let mut info = Implementation::new(&name, &version);
                if let Some(url) = self.website_url {
                    info = info.with_website_url(url);
                }
                if let Some(icons) = self.icons {
                    info = info.with_icons(icons);
                }
                info
            },
            capabilities: self.capabilities,
            tools: self.tools,
            tool_infos,
            uri_to_tool_meta,
            prompts: self.prompts,
            resources: final_resources,
            completions: self.completions,
            sampling: self.sampling,
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            notification_tx: None,
            cancellation_manager: self.cancellation_manager,
            roots_manager: Arc::new(RwLock::new(self.roots_manager)),
            subscription_manager: Arc::new(RwLock::new(subscriptions::SubscriptionManager::new())),
            listen_registry: Arc::new(subscriptions::ListenRegistry::new()),
            elicitation_manager: None,
            server_request_dispatcher: None,
            peer_handle: None,
            auth_provider: self.auth_provider,
            tool_authorizer,
            #[cfg(not(target_arch = "wasm32"))]
            tool_middleware_chain,
            #[cfg(feature = "streamable-http")]
            http_middleware: self.http_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            task_router: self.task_router,
            #[cfg(not(target_arch = "wasm32"))]
            task_store: self.task_store,
            #[cfg(not(target_arch = "wasm32"))]
            suppress_double_wrap: self.suppress_double_wrap,
            supported_protocol_versions: self.supported_protocol_versions,
            #[cfg(feature = "streamable-http")]
            request_state_codec,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::ResponsePayload, ClientCapabilities, InitializeRequest, ServerCapabilities,
        TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::time::timeout;

    // -- requestState key material (D-113-P) --------------------------------

    /// COMPILE-LEVEL guard on the FIELD TYPES, not on behaviour.
    ///
    /// The twin of `builder.rs`'s `request_state_key_field_is_the_zeroizing_type`.
    /// D-113-P named only `ServerCoreBuilder`; `ServerBuilder` carried the
    /// identical defect on the path most users actually take, so both need the
    /// guard. Reverting either field to bare `[u8; 32]` fails to compile here.
    #[cfg(feature = "streamable-http")]
    #[test]
    fn server_builder_request_state_key_field_is_the_zeroizing_type() {
        use crate::server::request_state::SecretKey;
        let builder = ServerBuilder::new()
            .with_request_state_key([0x11; 32])
            .with_request_state_previous_keys(vec![[0x22; 32]]);

        let key: &Option<SecretKey> = &builder.request_state_key;
        let previous: &Vec<SecretKey> = &builder.request_state_previous_keys;

        assert_eq!(key.as_deref(), Some(&[0x11u8; 32]));
        assert_eq!(previous.len(), 1);
        assert_eq!(**previous.first().expect("one previous key"), [0x22u8; 32]);
    }

    /// The plumbing regression guard for `ServerBuilder`: a server configured
    /// with a key plus a rotated-out key must still mint under the current key
    /// and verify.
    #[cfg(feature = "streamable-http")]
    #[test]
    fn a_server_with_zeroizing_key_fields_still_mints_and_verifies() {
        use crate::server::request_state::{key_id_of, RequestBinding, Verdict};

        const CURRENT: [u8; 32] = [0x11; 32];
        const ROTATED: [u8; 32] = [0x22; 32];

        let server = Server::builder()
            .name("t")
            .version("1")
            .with_supported_protocol_versions([
                ProtocolVersion("2026-07-28".to_string()),
                ProtocolVersion("2025-11-25".to_string()),
            ])
            .with_request_state_key(CURRENT)
            .with_request_state_previous_keys(vec![ROTATED])
            .build()
            .expect("server builds");

        let codec = server
            .request_state_codec()
            .expect("a v2 server has a codec");
        let params = json!({ "name": "t", "arguments": { "a": 1 } });
        let binding = RequestBinding::from_request("alice", "tools/call", &params)
            .expect("a two-level fixture is far inside the canonical depth cap");
        let token = codec
            .mint(&json!({ "step": 1 }), &binding, 0, None)
            .expect("mint");
        assert!(
            matches!(codec.verify(&token, &binding), Verdict::Ok(_)),
            "the zeroizing field type must not disturb the key plumbing"
        );
        assert!(codec.accepting_key_ids().contains(&key_id_of(&ROTATED)));
    }

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        messages: Arc<Mutex<Vec<TransportMessage>>>,
        responses: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_requests(requests: Vec<TransportMessage>) -> Self {
            Self {
                messages: Arc::new(Mutex::new(requests)),
                responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_request(&self, request: TransportMessage) {
            self.messages.lock().unwrap().push(request);
        }

        #[allow(dead_code)]
        fn get_sent_responses(&self) -> Vec<TransportMessage> {
            self.responses.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.responses.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let mut messages = self.messages.lock().unwrap();
            messages
                .pop()
                .map_or_else(|| Err(Error::protocol_msg("No more messages")), Ok)
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }

        fn is_connected(&self) -> bool {
            !self.messages.lock().unwrap().is_empty()
        }

        fn transport_type(&self) -> &'static str {
            "mock"
        }
    }

    /// Mock tool handler for testing
    struct MockTool {
        result: Value,
    }

    impl MockTool {
        fn new(result: Value) -> Self {
            Self { result }
        }
    }

    #[async_trait]
    impl ToolHandler for MockTool {
        async fn handle(
            &self,
            _args: Value,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<Value> {
            Ok(self.result.clone())
        }
    }

    /// Mock prompt handler for testing
    struct MockPrompt {
        result: crate::types::GetPromptResult,
    }

    impl MockPrompt {
        fn new(result: crate::types::GetPromptResult) -> Self {
            Self { result }
        }
    }

    #[async_trait]
    impl PromptHandler for MockPrompt {
        async fn handle(
            &self,
            _args: HashMap<String, String>,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::GetPromptResult> {
            Ok(self.result.clone())
        }
    }

    /// Mock resource handler for testing
    struct MockResource {
        resources: Vec<crate::types::ResourceInfo>,
        contents: HashMap<String, crate::types::ReadResourceResult>,
    }

    impl MockResource {
        fn new() -> Self {
            Self {
                resources: Vec::new(),
                contents: HashMap::new(),
            }
        }

        fn with_resource(mut self, uri: String, content: crate::types::ReadResourceResult) -> Self {
            self.contents.insert(uri, content);
            self
        }
    }

    #[async_trait]
    impl ResourceHandler for MockResource {
        async fn read(
            &self,
            uri: &str,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ReadResourceResult> {
            self.contents
                .get(uri)
                .cloned()
                .ok_or_else(|| Error::not_found(format!("Resource '{}' not found", uri)))
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ListResourcesResult> {
            Ok(crate::types::ListResourcesResult {
                resources: self.resources.clone(),
                next_cursor: None,
                ttl_ms: None,
                cache_scope: None,
            })
        }
    }

    #[test]
    fn test_server_builder() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        assert_eq!(server.info.name, "test-server");
        assert_eq!(server.info.version, "1.0.0");
        assert!(server.tools.contains_key("test-tool"));
    }

    #[test]
    fn test_server_builder_validation() {
        // Missing name
        let result = Server::builder().version("1.0.0").build();
        assert!(result.is_err());

        // Missing version
        let result = Server::builder().name("test-server").build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let init_request = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities::minimal(),
                client_info: Implementation::new("test-client", "1.0.0"),
            }))),
        };

        let transport = MockTransport::with_requests(vec![init_request]);
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        // Test server run for a short time
        let server_handle = tokio::spawn(async move {
            let _ = timeout(std::time::Duration::from_millis(100), server.run(transport)).await;
        });

        // Wait for server to process
        let _ = timeout(std::time::Duration::from_millis(200), server_handle).await;
    }

    #[tokio::test]
    async fn test_server_capabilities() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        assert!(!server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_none());
    }

    #[tokio::test]
    async fn test_server_notifications() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        // Send notification (should not panic even without transport)
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    #[test]
    fn test_server_builder_with_all_handlers() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
            _meta: None,
        };

        let resource_content =
            crate::types::ReadResourceResult::new(vec![crate::types::Content::text(
                "Hello, world!",
            )]);

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .prompt("test-prompt", MockPrompt::new(prompt_result))
            .resources(
                MockResource::new().with_resource("test://uri".to_string(), resource_content),
            )
            .build()
            .unwrap();

        assert!(server.tools.contains_key("test-tool"));
        assert!(server.prompts.contains_key("test-prompt"));
        assert!(server.resources.is_some());
    }

    #[tokio::test]
    async fn test_handle_request_initialize() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        assert_eq!(response.id, RequestId::from(1i64));
        match response.payload {
            ResponsePayload::Result(_) => {
                assert!(server.is_initialized().await);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_list_tools() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_call_tool() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test-tool".to_string(),
            arguments: json!({"input": "test"}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(!call_result.is_error);
                assert_eq!(call_result.content.len(), 1);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    /// Tool that reports the ingress-resolved era back through its result so a
    /// dispatch test can prove ingress→handler protocol-context threading on the
    /// high-level `Server` dispatch site.
    struct EraProbeServerTool;

    #[async_trait]
    impl ToolHandler for EraProbeServerTool {
        async fn handle(
            &self,
            _args: Value,
            extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<Value> {
            Ok(json!({ "era": extra.era().map(|e| format!("{e:?}")) }))
        }
    }

    fn probe_server_era(result: &Value) -> Value {
        let text = result["content"][0]["text"]
            .as_str()
            .expect("probe result carries text content");
        serde_json::from_str::<Value>(text).expect("probe text is JSON")["era"].clone()
    }

    fn v2_probe_call() -> Request {
        let meta = crate::types::protocol::RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            json!("2026-07-28"),
        );
        Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "probe".to_string(),
            arguments: json!({}),
            _meta: Some(meta),
            task: None,
        })))
    }

    /// Cross-site parity: the high-level `Server` dispatch site resolves the SAME
    /// v2 era as `ServerCore` for identical `_meta` (both use the one shared
    /// resolver), visible in the handler (Pitfall 3, twin wiring).
    #[tokio::test]
    async fn test_server_dispatch_resolves_v2_era_parity() {
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;
        use crate::types::ProtocolVersion;

        let server = Server::builder()
            .name("probe-server")
            .version("1.0.0")
            .tool("probe", EraProbeServerTool)
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(1i64), v2_probe_call(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                assert_eq!(probe_server_era(&result), json!("V2"));
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    /// Twin-site envelope parity (VERS-07): the high-level `Server` dispatch
    /// site injects the SAME v2 `resultType`/`serverInfo` envelope `ServerCore`
    /// does — via the ONE shared `core::inject_v2_result_envelope` helper — on a
    /// v2 object result.
    #[tokio::test]
    async fn test_server_dispatch_injects_v2_result_envelope_parity() {
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;
        use crate::types::ProtocolVersion;

        let server = Server::builder()
            .name("envelope-server")
            .version("3.2.1")
            .tool("probe", EraProbeServerTool)
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();

        // v2 request → envelope injected.
        let response = server
            .handle_request(RequestId::from(1i64), v2_probe_call(), None)
            .await;
        let ResponsePayload::Result(v) = response.payload else {
            panic!("expected result");
        };
        assert_eq!(v["resultType"], "complete");
        // Plan 113-09 Task 3: the schema places server identity INSIDE
        // `result._meta`, not at the top level.
        let server_info = &v["_meta"][crate::server::core::RESERVED_SERVER_INFO_KEY];
        assert_eq!(server_info["name"], "envelope-server");
        assert_eq!(server_info["version"], "3.2.1");
        assert!(
            v.get("serverInfo").is_none(),
            "the envelope must not write a top-level serverInfo: {v}"
        );
    }

    /// v1 byte-identity at the twin site: a non-opted-in `Server` gains NO
    /// `resultType`/`serverInfo` even with a v2 `_meta` signal (D-07).
    #[tokio::test]
    async fn test_server_dispatch_v1_no_envelope() {
        let server = Server::builder()
            .name("v1-envelope-server")
            .version("1.0.0")
            .tool("probe", EraProbeServerTool)
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(1i64), v2_probe_call(), None)
            .await;
        let ResponsePayload::Result(v) = response.payload else {
            panic!("expected result");
        };
        assert!(v.get("resultType").is_none(), "v1 must not gain resultType");
        assert!(v.get("serverInfo").is_none(), "v1 must not gain serverInfo");
        assert!(
            v.get("_meta").is_none(),
            "v1 must not gain the _meta the v2 envelope creates: {v}"
        );
    }

    /// A non-opted-in high-level `Server` runs zero era-detection: the handler
    /// reads `era()==None` even with a v2 `_meta` signal (D-04 parity).
    #[tokio::test]
    async fn test_server_dispatch_non_opted_in_yields_none() {
        let server = Server::builder()
            .name("v1-server")
            .version("1.0.0")
            .tool("probe", EraProbeServerTool)
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(1i64), v2_probe_call(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                assert_eq!(probe_server_era(&result), Value::Null);
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    #[tokio::test]
    async fn test_handle_call_tool_rejected_is_iserror_not_protocol_error() {
        // A handler returning `Error::tool_rejected` must surface through the
        // streamable-HTTP `Server` path as a SUCCESSFUL `CallToolResult`
        // with `isError: true` (message → content, details → structuredContent),
        // NOT a JSON-RPC protocol error. This is the Code Mode policy-rejection
        // envelope (e.g. "SELECT missing LIMIT") observed by `pmcp-sql-server`.
        struct RejectingTool;
        #[async_trait]
        impl ToolHandler for RejectingTool {
            async fn handle(
                &self,
                _args: Value,
                _extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<Value> {
                Err(Error::tool_rejected(
                    "SELECT statements must declare a LIMIT",
                    Some(json!({ "violations": [{ "rule": "missing_limit" }] })),
                ))
            }
        }

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("reject", RejectingTool)
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "reject".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(call_result.is_error, "tool_rejected must set isError: true");
                let text = call_result
                    .content
                    .iter()
                    .find_map(|c| match c {
                        crate::types::Content::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                assert!(
                    text.contains("must declare a LIMIT"),
                    "content must carry the rejection message, got: {text}"
                );
                let sc = call_result
                    .structured_content
                    .expect("structuredContent must carry the violation detail");
                assert_eq!(sc["violations"][0]["rule"], "missing_limit");
            },
            ResponsePayload::Error(e) => panic!(
                "tool_rejected must NOT be a protocol error, got {}: {}",
                e.code, e.message
            ),
        }
    }

    #[tokio::test]
    async fn test_handle_call_tool_not_found() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "nonexistent-tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_list_prompts() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
            _meta: None,
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .prompt("test-prompt", MockPrompt::new(prompt_result))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let list_result: ListPromptsResult = serde_json::from_value(result).unwrap();
                assert_eq!(list_result.prompts.len(), 1);
                assert_eq!(list_result.prompts[0].name, "test-prompt");
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_get_prompt() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
            _meta: None,
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .prompt("test-prompt", MockPrompt::new(prompt_result.clone()))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name: "test-prompt".to_string(),
            arguments: HashMap::new(),
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let get_result: crate::types::GetPromptResult =
                    serde_json::from_value(result).unwrap();
                assert_eq!(get_result.description, prompt_result.description);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 112-09 (Gap B): the high-level `Server` twin threads protocol_context
    // + request_meta into prompt/resource handlers. Enters through the REAL
    // dispatch entrypoint (`process_client_request`), NOT the leaf handlers.
    // -----------------------------------------------------------------------
    #[derive(Clone, Debug, Default, PartialEq)]
    struct DispatchCaptured {
        era: Option<crate::types::protocol::Era>,
        has_client_info: bool,
        traceparent: Option<String>,
    }

    struct DispatchCapturingPrompt(Arc<Mutex<Option<DispatchCaptured>>>);

    #[async_trait]
    impl PromptHandler for DispatchCapturingPrompt {
        async fn handle(
            &self,
            _args: HashMap<String, String>,
            extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::GetPromptResult> {
            *self.0.lock().unwrap() = Some(DispatchCaptured {
                era: extra.era(),
                has_client_info: extra.client_info().is_some(),
                traceparent: extra.trace_context().map(|t| t.traceparent),
            });
            Ok(crate::types::GetPromptResult::new(vec![], None))
        }
    }

    struct DispatchCapturingResource(Arc<Mutex<Option<DispatchCaptured>>>);

    #[async_trait]
    impl ResourceHandler for DispatchCapturingResource {
        async fn read(
            &self,
            _uri: &str,
            extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ReadResourceResult> {
            *self.0.lock().unwrap() = Some(DispatchCaptured {
                era: extra.era(),
                has_client_info: extra.client_info().is_some(),
                traceparent: extra.trace_context().map(|t| t.traceparent),
            });
            Ok(crate::types::ReadResourceResult::new(vec![
                crate::types::Content::text("ok"),
            ]))
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ListResourcesResult> {
            Ok(crate::types::ListResourcesResult {
                resources: vec![],
                next_cursor: None,
                ttl_ms: None,
                cache_scope: None,
            })
        }
    }

    fn dispatch_v2_meta() -> crate::types::protocol::RequestMeta {
        crate::types::protocol::RequestMeta::new().with_meta(
            "traceparent",
            serde_json::json!("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
        )
    }

    fn dispatch_v2_context() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(
            crate::types::protocol::Era::V2,
            ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
        )
        .with_client_info(crate::types::Implementation::new("test-client", "9.9.9"))
    }

    #[tokio::test]
    async fn prompt_resource_protocol_context_via_dispatch_server() {
        use crate::types::protocol::Era;

        let pcap = Arc::new(Mutex::new(None));
        let rcap = Arc::new(Mutex::new(None));
        let server = Server::builder()
            .name("dispatch-server")
            .version("1.0.0")
            .prompt("greeting", DispatchCapturingPrompt(pcap.clone()))
            .resources(DispatchCapturingResource(rcap.clone()))
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();

        // --- v2 dispatch through process_client_request: era==V2, client_info,
        // and a populated trace_context (proves .with_request_meta threading).
        server
            .process_client_request(
                RequestId::from(1i64),
                ClientRequest::GetPrompt(GetPromptRequest {
                    name: "greeting".to_string(),
                    arguments: HashMap::new(),
                    _meta: Some(dispatch_v2_meta()),
                }),
                None,
                Some(dispatch_v2_context()),
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();
        server
            .process_client_request(
                RequestId::from(2i64),
                ClientRequest::ReadResource(ReadResourceRequest {
                    uri: "mem://greeting".to_string(),
                    _meta: Some(dispatch_v2_meta()),
                }),
                None,
                Some(dispatch_v2_context()),
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();

        for cap in [&pcap, &rcap] {
            let c = cap.lock().unwrap().clone().expect("handler ran");
            assert_eq!(c.era, Some(Era::V2));
            assert!(c.has_client_info);
            assert_eq!(
                c.traceparent.as_deref(),
                Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01")
            );
        }

        // --- opted-in v1 fallback: era==Some(V1) (distinct from None).
        let pcap = Arc::new(Mutex::new(None));
        let rcap = Arc::new(Mutex::new(None));
        let server = Server::builder()
            .name("dispatch-server")
            .version("1.0.0")
            .prompt("greeting", DispatchCapturingPrompt(pcap.clone()))
            .resources(DispatchCapturingResource(rcap.clone()))
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();
        let v1 = crate::types::protocol::ProtocolContext::new(
            Era::V1,
            ProtocolVersion("2025-11-25".to_string()),
        );
        server
            .process_client_request(
                RequestId::from(3i64),
                ClientRequest::GetPrompt(GetPromptRequest {
                    name: "greeting".to_string(),
                    arguments: HashMap::new(),
                    _meta: None,
                }),
                None,
                Some(v1.clone()),
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();
        server
            .process_client_request(
                RequestId::from(4i64),
                ClientRequest::ReadResource(ReadResourceRequest {
                    uri: "mem://greeting".to_string(),
                    _meta: None,
                }),
                None,
                Some(v1),
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();
        assert_eq!(pcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));
        assert_eq!(rcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));

        // --- non-opted-in (protocol_context == None): era==None.
        let pcap = Arc::new(Mutex::new(None));
        let rcap = Arc::new(Mutex::new(None));
        let server = Server::builder()
            .name("dispatch-server")
            .version("1.0.0")
            .prompt("greeting", DispatchCapturingPrompt(pcap.clone()))
            .resources(DispatchCapturingResource(rcap.clone()))
            .build()
            .unwrap();
        server
            .process_client_request(
                RequestId::from(5i64),
                ClientRequest::GetPrompt(GetPromptRequest {
                    name: "greeting".to_string(),
                    arguments: HashMap::new(),
                    _meta: None,
                }),
                None,
                None,
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();
        server
            .process_client_request(
                RequestId::from(6i64),
                ClientRequest::ReadResource(ReadResourceRequest {
                    uri: "mem://greeting".to_string(),
                    _meta: None,
                }),
                None,
                None,
                &mut crate::server::core::DispatchEnvelopeClaim::default(),
            )
            .await
            .unwrap();
        assert_eq!(pcap.lock().unwrap().clone().unwrap().era, None);
        assert_eq!(rcap.lock().unwrap().clone().unwrap().era, None);
    }

    #[tokio::test]
    async fn test_handle_list_resources() {
        let resource_content =
            crate::types::ReadResourceResult::new(vec![crate::types::Content::text(
                "Hello, world!",
            )]);

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(
                MockResource::new().with_resource("test://uri".to_string(), resource_content),
            )
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor: None },
        )));
        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let resources_result: ListResourcesResult = serde_json::from_value(result).unwrap();
                assert_eq!(resources_result.resources.len(), 0); // MockResource has empty list by default
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_read_resource() {
        let resource_content =
            crate::types::ReadResourceResult::new(vec![crate::types::Content::text(
                "Hello, world!",
            )]);

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(
                MockResource::new()
                    .with_resource("test://uri".to_string(), resource_content.clone()),
            )
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "test://uri".to_string(),
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let read_result: crate::types::ReadResourceResult =
                    serde_json::from_value(result).unwrap();
                assert_eq!(read_result.contents.len(), 1);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_read_resource_not_found() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(MockResource::new())
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "nonexistent://uri".to_string(),
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::Ping));
        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Result(_) => {
                // Success
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_server_request() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Server(Box::new(crate::types::ServerRequest::CreateMessage(
            Box::new(crate::types::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
                tools: None,
                tool_choice: None,
            }),
        )));
        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert_eq!(error.code, -32601);
                assert!(error.message.contains("not supported"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }

    // Tests for tool middleware support in ServerBuilder
    #[tokio::test]
    async fn test_server_builder_with_tool_middleware() {
        use crate::server::tool_middleware::{ToolContext, ToolMiddleware};
        use std::sync::atomic::{AtomicBool, Ordering};

        // Create a simple middleware that sets a flag when called
        struct TestMiddleware {
            called: Arc<AtomicBool>,
        }

        #[async_trait]
        impl ToolMiddleware for TestMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                extra: &mut crate::server::cancellation::RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                self.called.store(true, Ordering::SeqCst);
                extra.set_metadata("middleware_executed".to_string(), "true".to_string());
                Ok(())
            }
        }

        let middleware_called = Arc::new(AtomicBool::new(false));
        let middleware = Arc::new(TestMiddleware {
            called: Arc::clone(&middleware_called),
        });

        // Build server with middleware
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test_tool", MockTool::new(json!({"result": "success"})))
            .tool_middleware(middleware)
            .build()
            .unwrap();

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Verify middleware was called
        assert!(middleware_called.load(Ordering::SeqCst));

        // Verify tool executed successfully
        match response.payload {
            ResponsePayload::Result(_) => {}, // Success
            ResponsePayload::Error(e) => panic!("Expected success, got error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_server_builder_multiple_middlewares() {
        use crate::server::tool_middleware::{ToolContext, ToolMiddleware};
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Middleware that increments a counter
        struct CounterMiddleware {
            counter: Arc<AtomicUsize>,
            id: usize,
        }

        #[async_trait]
        impl ToolMiddleware for CounterMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                extra: &mut crate::server::cancellation::RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                let count = self.counter.fetch_add(1, Ordering::SeqCst);
                extra.set_metadata(format!("middleware_{}_order", self.id), count.to_string());
                Ok(())
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let middleware1 = Arc::new(CounterMiddleware {
            counter: Arc::clone(&counter),
            id: 1,
        });
        let middleware2 = Arc::new(CounterMiddleware {
            counter: Arc::clone(&counter),
            id: 2,
        });

        // Build server with multiple middlewares
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test_tool", MockTool::new(json!({"result": "success"})))
            .tool_middleware(middleware1)
            .tool_middleware(middleware2)
            .build()
            .unwrap();

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let _response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Verify both middlewares were called in order
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_server_builder_middleware_with_typed_tools() {
        use crate::server::tool_middleware::{ToolContext, ToolMiddleware};
        use std::sync::atomic::{AtomicBool, Ordering};

        // Middleware that injects OAuth token
        struct OAuthMiddleware {
            called: Arc<AtomicBool>,
        }

        #[async_trait]
        impl ToolMiddleware for OAuthMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                extra: &mut crate::server::cancellation::RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                self.called.store(true, Ordering::SeqCst);
                extra.set_metadata("oauth_token".to_string(), "test-token-123".to_string());
                Ok(())
            }
        }

        // Tool that verifies OAuth token was injected
        struct OAuthVerifyTool;

        #[async_trait]
        impl ToolHandler for OAuthVerifyTool {
            async fn handle(
                &self,
                _args: Value,
                extra: crate::server::cancellation::RequestHandlerExtra,
            ) -> Result<Value> {
                // Verify OAuth token was injected by middleware
                let token = extra.get_metadata("oauth_token");
                assert!(token.is_some());
                assert_eq!(token.unwrap(), "test-token-123");
                Ok(json!({"success": true}))
            }
        }

        let middleware_called = Arc::new(AtomicBool::new(false));
        let middleware = Arc::new(OAuthMiddleware {
            called: Arc::clone(&middleware_called),
        });

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("typed_tool", OAuthVerifyTool)
            .tool_middleware(middleware)
            .build()
            .unwrap();

        // Call the typed tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "typed_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Verify middleware was called
        assert!(middleware_called.load(Ordering::SeqCst));

        // Verify tool executed successfully
        match response.payload {
            ResponsePayload::Result(_) => {}, // Success
            ResponsePayload::Error(e) => panic!("Expected success, got error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_server_builder_middleware_error_handling() {
        use crate::server::tool_middleware::{ToolContext, ToolMiddleware};

        // Middleware that rejects requests
        struct RejectMiddleware;

        #[async_trait]
        impl ToolMiddleware for RejectMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                _extra: &mut crate::server::cancellation::RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                Err(Error::validation("Middleware rejected request"))
            }
        }

        // Build server with rejecting middleware
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test_tool", MockTool::new(json!({"result": "success"})))
            .tool_middleware(Arc::new(RejectMiddleware))
            .build()
            .unwrap();

        // Call the tool
        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test_tool".to_string(),
            arguments: json!({}),
            _meta: None,
            task: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), request, None)
            .await;

        // Verify request was rejected by middleware
        match response.payload {
            ResponsePayload::Error(e) => {
                assert!(e.message.contains("Middleware rejected request"));
            },
            ResponsePayload::Result(_) => panic!("Expected error from middleware"),
        }
    }

    #[tokio::test]
    async fn test_server_builder_auto_capabilities_serialization() {
        // Test that ServerBuilder (used by Server::builder()) auto-sets capabilities
        // with proper serialization values
        let server = Server::builder()
            .name("test")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "ok"})))
            .prompt(
                "test-prompt",
                MockPrompt::new(crate::types::GetPromptResult {
                    description: None,
                    messages: vec![],
                    _meta: None,
                }),
            )
            .resources(MockResource::new())
            .build()
            .unwrap();

        let caps = &server.capabilities;
        let json = serde_json::to_value(caps).unwrap();

        // Verify tools capability is present and properly structured
        let tools = json.get("tools").expect("tools should be present in JSON");
        assert!(tools.is_object(), "tools should be an object");
        let list_changed = tools.get("listChanged");
        assert!(
            list_changed.is_some(),
            "listChanged should be present in tools"
        );
        assert_eq!(
            list_changed.unwrap(),
            &serde_json::json!(false),
            "listChanged should be false"
        );

        // Verify prompts capability
        let prompts = json
            .get("prompts")
            .expect("prompts should be present in JSON");
        assert!(prompts.is_object(), "prompts should be an object");
        assert!(
            prompts.get("listChanged").is_some(),
            "listChanged should be present in prompts"
        );

        // Verify resources capability
        let resources = json
            .get("resources")
            .expect("resources should be present in JSON");
        assert!(resources.is_object(), "resources should be an object");
        assert!(
            resources.get("listChanged").is_some() || resources.get("subscribe").is_some(),
            "resources should have fields"
        );

        println!(
            "Serialized capabilities: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );
    }

    /// Behavioral test for `ServerBuilder::tool_authorizer_arc` — the only
    /// non-mechanical lift among Phase 82's six `_arc` lifts.
    ///
    /// `tool_authorizer_arc` mirrors `tool_authorizer()`'s protection-clearing
    /// semantics: chaining `.protect_tool(...)` BEFORE `.tool_authorizer_arc(...)`
    /// must clear `tool_protections` so that `.build()` does NOT hit the
    /// mixed-config rejection branch at mod.rs `build()` (which fires if
    /// `tool_protections` is non-empty AND `tool_authorizer` is set).
    /// This test fills the verification gap source-greps cannot — proving
    /// the `.clear()` call actually fires.
    #[tokio::test]
    async fn tool_authorizer_arc_clears_tool_protections_and_allows_build() {
        // Define a no-op custom ToolAuthorizer for the test.
        struct NoopAuthorizer;
        #[async_trait]
        impl crate::server::auth::ToolAuthorizer for NoopAuthorizer {
            async fn can_access_tool(
                &self,
                _auth: &crate::server::auth::AuthContext,
                _tool_name: &str,
            ) -> crate::Result<bool> {
                Ok(true)
            }
            async fn required_scopes_for_tool(
                &self,
                _tool_name: &str,
            ) -> crate::Result<Vec<String>> {
                Ok(vec![])
            }
        }

        // Build a server with BOTH protect_tool AND tool_authorizer_arc.
        // Without the clearing semantic, build() would return the
        // "Cannot use protect_tool() with a custom tool_authorizer" error.
        let builder = ServerBuilder::new()
            .name("test")
            .version("1")
            .protect_tool("delete", vec!["admin".to_string()])
            .tool_authorizer_arc(Arc::new(NoopAuthorizer));

        // ASSERT 1: tool_protections was cleared by tool_authorizer_arc(),
        // visible because we are inside the same module and have access
        // to the private field.
        assert!(
            builder.tool_protections.is_empty(),
            "tool_authorizer_arc() must clear tool_protections to mirror tool_authorizer()"
        );

        // ASSERT 2: build() succeeds — the mixed-config rejection branch
        // does NOT fire because protections was cleared.
        let build_result = builder.build();
        assert!(
            build_result.is_ok(),
            "build() should succeed after tool_authorizer_arc() clears protections; got Err({:?})",
            build_result.err()
        );
    }
}

#[cfg(test)]
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
mod skills_builder_tests {
    use super::*;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::skills::{Skill, SkillReference, Skills};
    use crate::types::Content;
    use async_trait::async_trait;

    // ── Test 2.1a: single skill via ServerBuilder (public path) ──────
    #[test]
    fn test_2_1a_skill_method_single_skill_via_server_builder() {
        let server = Server::builder()
            .name("test")
            .version("1.0")
            .skill(Skill::new("foo", "body"))
            .build()
            .unwrap();
        assert!(server.capabilities.resources.is_some());
    }

    // ── Test 2.2 (ServerBuilder): extensions capability ──────────────
    #[test]
    fn test_2_2_server_builder_skills_sets_extensions_capability() {
        let server = Server::builder()
            .name("test")
            .version("1.0")
            .skills(Skills::new().add(Skill::new("a", "")))
            .build()
            .unwrap();
        let ext = server
            .capabilities
            .extensions
            .as_ref()
            .expect("extensions should be set");
        assert_eq!(
            ext.get("io.modelcontextprotocol/skills"),
            Some(&serde_json::json!({}))
        );
    }

    // ── Test 2.3 (ServerBuilder): resources capability ───────────────
    #[test]
    fn test_2_3_server_builder_skills_sets_resources_capability() {
        let server = Server::builder()
            .name("test")
            .version("1.0")
            .skills(Skills::new().add(Skill::new("a", "")))
            .build()
            .unwrap();
        let r = server
            .capabilities
            .resources
            .as_ref()
            .expect("resources should be set");
        assert_eq!(r.subscribe, Some(false));
        assert_eq!(r.list_changed, Some(false));
    }

    // ── Test 2.4a: skills compose with existing resources (ServerBuilder) ─
    struct DocsHandler;
    #[async_trait]
    impl ResourceHandler for DocsHandler {
        async fn read(
            &self,
            uri: &str,
            _extra: RequestHandlerExtra,
        ) -> Result<crate::types::ReadResourceResult> {
            Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                format!("DOCS:{uri}"),
            )]))
        }
        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<crate::types::ListResourcesResult> {
            Ok(crate::types::ListResourcesResult::new(vec![
                crate::types::ResourceInfo::new("docs://handbook", "handbook"),
            ]))
        }
    }

    #[test]
    fn test_2_4a_server_builder_skills_compose_with_existing_resources() {
        let server = Server::builder()
            .name("t")
            .version("1.0")
            .resources(DocsHandler)
            .skill(Skill::new("a", "skill-a"))
            .build()
            .unwrap();
        // Capability state must reflect both surfaces.
        assert!(server.capabilities.resources.is_some());
        let ext = server.capabilities.extensions.as_ref().unwrap();
        assert!(ext.contains_key("io.modelcontextprotocol/skills"));
    }

    // ── Test 2.7 (ServerBuilder): bootstrap_skill_and_prompt ──────────
    #[test]
    fn test_2_7_server_builder_bootstrap_skill_and_prompt() {
        let server = Server::builder()
            .name("t")
            .version("1.0")
            .bootstrap_skill_and_prompt(Skill::new("c", "body-c"), "my_prompt")
            .build()
            .unwrap();
        assert!(server.has_prompt("my_prompt"));
        assert!(server.capabilities.prompts.is_some());
        let ext = server
            .capabilities
            .extensions
            .as_ref()
            .expect("extensions should be set");
        assert!(ext.contains_key("io.modelcontextprotocol/skills"));
        assert!(server.capabilities.resources.is_some());
    }

    // ── Test 2.8: wire-level dual-surface invariant via ServerBuilder ─
    #[tokio::test]
    async fn test_2_8_bootstrap_skill_and_prompt_byte_equal_invariant() {
        let skill = Skill::new("x", "A").with_reference(SkillReference::new(
            "ref1.md",
            "text/markdown",
            "refbody",
        ));
        let expected_text = skill.as_prompt_text();

        let server = Server::builder()
            .name("t")
            .version("1.0")
            .bootstrap_skill_and_prompt(skill, "x")
            .build()
            .unwrap();

        let prompt = server
            .get_prompt("x")
            .expect("prompt 'x' must be registered");
        let result = prompt
            .handle(HashMap::new(), RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.messages.len(), 1);
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, &expected_text),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 2.9 (ServerBuilder): duplicate URI panics at .build() ───
    #[test]
    #[should_panic(expected = "duplicate")]
    fn test_2_9_server_builder_skills_panics_on_duplicate_uri_at_build() {
        let _ = Server::builder()
            .name("t")
            .version("1.0")
            .skill(Skill::new("x", "a"))
            .skill(Skill::new("x", "b"))
            .build()
            .unwrap();
    }

    // ── Test 2.9a (ServerBuilder): try_skills returns Err on duplicate ─
    #[test]
    fn test_2_9a_server_builder_try_skills_returns_err_on_duplicate() {
        let res = Server::builder().name("t").version("1.0").try_skills(
            Skills::new()
                .add(Skill::new("x", "a"))
                .add(Skill::new("x", "b")),
        );
        assert!(res.is_err());
        match res {
            Err(crate::Error::Validation(_)) => {},
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for duplicate"),
        }
    }

    // ── Test 2.10 (ServerBuilder): capability merge preserves extensions ─
    #[test]
    fn test_2_10_server_builder_capability_merge_preserves_pre_existing_extensions() {
        let mut caps = crate::types::ServerCapabilities::default();
        let mut ext = HashMap::new();
        ext.insert("some.other/ext".to_string(), serde_json::json!({"foo": 1}));
        caps.extensions = Some(ext);

        let server = Server::builder()
            .name("t")
            .version("1.0")
            .capabilities(caps)
            .skill(Skill::new("a", ""))
            .build()
            .unwrap();
        let ext = server.capabilities.extensions.as_ref().unwrap();
        assert!(ext.contains_key("some.other/ext"));
        assert!(ext.contains_key("io.modelcontextprotocol/skills"));
    }

    // ── Test 2.11 (ServerBuilder): accumulator — all skills reachable ─
    #[test]
    fn test_2_11_server_builder_accumulator_repeated_skill_calls_all_reachable() {
        // Build a server with three .skill calls and confirm prompts/caps wire up.
        let server = Server::builder()
            .name("t")
            .version("1.0")
            .skill(Skill::new("a", "body-a"))
            .skill(Skill::new("b", "body-b"))
            .bootstrap_skill_and_prompt(Skill::new("c", "body-c"), "c_prompt")
            .build()
            .unwrap();
        assert!(server.has_prompt("c_prompt"));
        assert!(server.capabilities.resources.is_some());
    }

    // ── Test 2.5a (ServerBuilder): .resources() semantics unchanged ──
    #[test]
    fn test_2_5a_server_builder_resources_replace_unchanged_no_skills() {
        struct A;
        #[async_trait]
        impl ResourceHandler for A {
            async fn read(
                &self,
                _uri: &str,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ReadResourceResult> {
                Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                    "A",
                )]))
            }
            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult::new(vec![]))
            }
        }
        struct B;
        #[async_trait]
        impl ResourceHandler for B {
            async fn read(
                &self,
                _uri: &str,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ReadResourceResult> {
                Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                    "B",
                )]))
            }
            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult::new(vec![]))
            }
        }

        let server = Server::builder()
            .name("t")
            .version("1.0")
            .resources(A)
            .resources(B)
            .build()
            .unwrap();
        // No skills registered → no composition. Capabilities reflect resources only.
        assert!(server.capabilities.resources.is_some());
        // Skills extension should NOT have been auto-set.
        let ext = server.capabilities.extensions.as_ref();
        if let Some(ext_map) = ext {
            assert!(!ext_map.contains_key("io.modelcontextprotocol/skills"));
        }
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tool_output_tests {
    use super::*;
    use crate::server::cancellation::RequestHandlerExtra;
    use async_trait::async_trait;

    /// A handler that implements ONLY `handle` (no `handle_output` override) —
    /// the common case. It must route through the default `handle_output` and
    /// come back as `ToolOutput::Payload` equal to what `handle` returned.
    struct PlainHandler;

    #[async_trait]
    impl ToolHandler for PlainHandler {
        async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(serde_json::json!({ "echo": args }))
        }
    }

    #[tokio::test]
    async fn default_handle_output_delegates_to_handle_as_payload() {
        let handler = PlainHandler;
        let extra = RequestHandlerExtra::new("req-1".to_string(), Default::default());
        let args = serde_json::json!({ "n": 42 });

        let value_via_handle = handler.handle(args.clone(), extra.clone()).await.unwrap();
        let output = handler.handle_output(args, extra).await.unwrap();

        match output {
            ToolOutput::Payload(v) => assert_eq!(
                v, value_via_handle,
                "default handle_output must wrap handle()'s value as Payload"
            ),
            other => panic!("expected ToolOutput::Payload, got {other:?}"),
        }
    }
}

// ===========================================================================
// `attach_peer` precedence and authorization ordering.
//
// Phase 118.1 plan 11. Two claims, both of which need crate-internal access —
// `attach_peer` is private and `Server::peer_handle` is only ever set by
// `Server::run()` — so they live here rather than in an integration test:
//
//   * T-118.1-11-04: when BOTH a global `Server::peer_handle` and a
//     request-scoped `TransportBackchannel` peer are configured, the
//     request-scoped one wins. A global handle cannot express WHICH session
//     issued the request, so on a multiplexed transport it is the wrong answer.
//   * `src/shared/peer.rs`'s authorization invariant: tool-level authz runs
//     BEFORE the peer is wired, so a refused caller never reaches a handler body
//     and therefore never sees `extra.peer()`.
// ===========================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
mod peer_precedence_tests {
    use super::*;
    use crate::server::auth::AuthContext;
    use crate::shared::peer::PeerHandle;
    use crate::types::protocol::context::TransportBackchannel;
    use crate::types::protocol::{Era, ProtocolContext, ProtocolVersion};
    use crate::types::roots::{ListRootsResult, Root};
    use crate::types::sampling::{CreateMessageParams, CreateMessageResult};
    use crate::types::ProgressToken;
    use crate::RequestHandlerExtra;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A peer that reports WHICH source supplied it, through the one method
    /// with an observable, source-specific answer.
    struct NamedPeer(&'static str);

    #[async_trait]
    impl PeerHandle for NamedPeer {
        async fn sample(&self, _params: CreateMessageParams) -> Result<CreateMessageResult> {
            Err(Error::protocol(
                crate::ErrorCode::METHOD_NOT_FOUND,
                "not the method under test",
            ))
        }

        async fn list_roots(&self) -> Result<ListRootsResult> {
            Ok(ListRootsResult {
                roots: vec![Root {
                    uri: format!("file:///{}", self.0),
                    name: Some(self.0.to_string()),
                }],
            })
        }

        async fn progress_notify(
            &self,
            _token: ProgressToken,
            _progress: f64,
            _total: Option<f64>,
            _message: Option<String>,
        ) -> Result<()> {
            Ok(())
        }
    }

    /// The name the attached peer answers with, or `None` if none was attached.
    async fn attached_peer_name(
        extra: &crate::server::cancellation::RequestHandlerExtra,
    ) -> Option<String> {
        let peer = extra.peer()?;
        let roots = peer.list_roots().await.expect("the fixture peer answers");
        roots.roots.first().and_then(|r| r.name.clone())
    }

    /// A `ProtocolContext` carrying a request-scoped peer named `name`.
    fn context_with_peer(name: &'static str) -> ProtocolContext {
        let peer: Arc<dyn PeerHandle> = Arc::new(NamedPeer(name));
        ProtocolContext::new(
            Era::V1,
            ProtocolVersion(crate::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
        )
        .with_transport_backchannel(TransportBackchannel::new().with_peer(peer))
    }

    fn bare_server() -> Server {
        Server::builder()
            .name("attach-peer-precedence")
            .version("1.0.0")
            .build()
            .expect("server builds")
    }

    fn extra_with_context(
        context: Option<ProtocolContext>,
    ) -> crate::server::cancellation::RequestHandlerExtra {
        crate::server::cancellation::RequestHandlerExtra::new(
            "req-attach-peer".to_string(),
            crate::server::cancellation::RequestHandlerExtra::default().cancellation_token,
        )
        .with_protocol_context(context)
    }

    /// THE precedence claim (T-118.1-11-04). Both sources configured at once.
    #[tokio::test]
    async fn the_request_scoped_peer_wins_over_the_global_peer_handle() {
        let mut server = bare_server();
        server.peer_handle = Some(Arc::new(NamedPeer("global")));

        let extra = server.attach_peer(extra_with_context(Some(context_with_peer(
            "request-scoped",
        ))));

        assert_eq!(
            attached_peer_name(&extra).await.as_deref(),
            Some("request-scoped"),
            "a request-scoped transport handle must win: the global `peer_handle` is a SINGLE \
             field and cannot express which session issued this request (T-118.1-11-04)"
        );
    }

    /// The fallback is untouched: the in-process `Server::run` path attaches no
    /// backchannel, so it must still see the global handle.
    #[tokio::test]
    async fn the_global_handle_still_applies_when_no_backchannel_rides_the_context() {
        let mut server = bare_server();
        server.peer_handle = Some(Arc::new(NamedPeer("global")));

        let with_no_context = server.attach_peer(extra_with_context(None));
        assert_eq!(
            attached_peer_name(&with_no_context).await.as_deref(),
            Some("global"),
            "with no protocol context at all the global handle must still apply"
        );

        let bare_context = ProtocolContext::new(
            Era::V1,
            ProtocolVersion(crate::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
        );
        let with_peerless_context = server.attach_peer(extra_with_context(Some(bare_context)));
        assert_eq!(
            attached_peer_name(&with_peerless_context).await.as_deref(),
            Some("global"),
            "a context with no backchannel must fall through to the global handle"
        );
    }

    /// Neither source configured: still a no-op, exactly as before.
    #[tokio::test]
    async fn attach_peer_is_a_no_op_when_neither_source_is_configured() {
        let server = bare_server();
        let extra = server.attach_peer(extra_with_context(None));
        assert!(
            extra.peer().is_none(),
            "with no global handle and no backchannel, `extra.peer()` stays None"
        );
    }

    /// A request-scoped peer applies even with NO global handle — the
    /// `StreamableHTTP` case, where `Server::run()` never ran.
    #[tokio::test]
    async fn the_request_scoped_peer_applies_with_no_global_handle_at_all() {
        let server = bare_server();
        let extra = server.attach_peer(extra_with_context(Some(context_with_peer("transport"))));
        assert_eq!(
            attached_peer_name(&extra).await.as_deref(),
            Some("transport"),
            "the HTTP transport never calls `Server::run()`, so the request-scoped handle is \
             the ONLY source there"
        );
    }

    // -----------------------------------------------------------------------
    // Authorization ordering.
    // -----------------------------------------------------------------------

    /// Records whether its body ever ran, and reports the peer it saw.
    struct EntryRecordingTool(Arc<AtomicBool>);

    #[async_trait]
    impl ToolHandler for EntryRecordingTool {
        async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
            self.0.store(true, Ordering::SeqCst);
            Ok(serde_json::json!({ "saw_peer": extra.peer().is_some() }))
        }
    }

    /// Refuses every tool.
    struct DenyAll;

    #[async_trait]
    impl crate::server::auth::ToolAuthorizer for DenyAll {
        async fn can_access_tool(&self, _auth: &AuthContext, _tool: &str) -> Result<bool> {
            Ok(false)
        }

        async fn required_scopes_for_tool(&self, _tool_name: &str) -> Result<Vec<String>> {
            Ok(Vec::new())
        }
    }

    /// An unauthorized caller never reaches the handler BODY, so it can never
    /// observe `extra.peer()` — regardless of which peer source is configured.
    ///
    /// The ordering this measures is structural: `handle_call_tool` runs the
    /// `tool_authorizer` check (`src/server/mod.rs`, immediately after the
    /// auth-context resolution) and only afterwards calls `attach_peer`.
    #[tokio::test]
    async fn an_unauthorized_caller_never_reaches_the_handler_body() {
        let entered = Arc::new(AtomicBool::new(false));
        let mut server = Server::builder()
            .name("authz-before-peer")
            .version("1.0.0")
            .tool("guarded", EntryRecordingTool(entered.clone()))
            .tool_authorizer(DenyAll)
            .build()
            .expect("server builds");
        // BOTH peer sources configured, so a leak through either would show up.
        server.peer_handle = Some(Arc::new(NamedPeer("global")));

        let mut claim = crate::server::core::DispatchEnvelopeClaim::default();
        let result = server
            .handle_call_tool(
                RequestId::from(1i64),
                CallToolRequest {
                    name: "guarded".to_string(),
                    arguments: serde_json::json!({}),
                    task: None,
                    _meta: None,
                },
                Some(AuthContext::new("someone")),
                Some(context_with_peer("request-scoped")),
                &mut claim,
            )
            .await;

        assert!(result.is_err(), "a denied tool call must return an error");
        assert!(
            !entered.load(Ordering::SeqCst),
            "the handler body must never run for an unauthorized caller — authz runs BEFORE \
             `attach_peer`, so a refused caller never sees `extra.peer()`"
        );
    }
}

// ===========================================================================
// `attach_request_log_sink` at the `Server` root — the TWIN of
// `core_log_sink_tests` in `src/server/core.rs` (Phase 118.2 plan 06, CONF-10 /
// D-07).
//
// Same crate-internal-access reason as `peer_precedence_tests` above:
// `attach_peer`, `notification_tx_sink`, `progress_reporter_for` and
// `Server::notification_tx` are all private, and `TransportBackchannel` /
// `ProtocolContext::with_resolved_log_level` are `pub(crate)`. An integration
// test can construct none of them.
//
// The claims measured here that the `ServerCore` side CANNOT measure:
//
//   * the `notification_tx`-derived fallback exists on this root and nowhere
//     else, and the request-scoped sink still beats it;
//   * D-07: the progress-token gate moved OFF the log sink and STAYED on the
//     progress reporter. One request, both answers, in one test.
// ===========================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
mod log_sink_precedence_tests {
    use super::*;
    use crate::types::protocol::context::TransportBackchannel;
    use crate::types::protocol::{Era, ProtocolContext, ProtocolVersion};
    use crate::types::LoggingLevel;
    use std::sync::Mutex;

    /// A sink that records every notification handed to it.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<Notification>>>);

    impl Capture {
        fn sink(&self) -> Arc<dyn Fn(Notification) + Send + Sync> {
            let slot = Arc::clone(&self.0);
            Arc::new(move |notification| {
                slot.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(notification);
            })
        }

        fn len(&self) -> usize {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        }
    }

    fn bare_context() -> ProtocolContext {
        ProtocolContext::new(
            Era::V1,
            ProtocolVersion(crate::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
        )
    }

    fn context_with_sink(capture: &Capture) -> ProtocolContext {
        bare_context().with_transport_backchannel(
            TransportBackchannel::new().with_notification_sink(capture.sink()),
        )
    }

    fn extra_with_context(
        context: Option<ProtocolContext>,
    ) -> crate::server::cancellation::RequestHandlerExtra {
        crate::server::cancellation::RequestHandlerExtra::new(
            "req-attach-log-sink".to_string(),
            crate::server::cancellation::RequestHandlerExtra::default().cancellation_token,
        )
        .with_protocol_context(context)
    }

    fn bare_server() -> Server {
        Server::builder()
            .name("attach-log-sink-precedence")
            .version("1.0.0")
            .build()
            .expect("server builds")
    }

    /// The ROOT claim on this side: `Server::attach_peer` wires the log sink from
    /// the server-wide `notification_tx` when the request carries no
    /// back-channel of its own — the in-process `Server::run` path.
    #[tokio::test]
    async fn the_server_root_attaches_its_notification_tx_derived_fallback_log_sink() {
        let mut server = bare_server();
        let (tx, mut rx) = mpsc::channel(4);
        server.notification_tx = Some(tx);

        let extra = server.attach_peer(extra_with_context(Some(bare_context())));
        extra
            .log(
                LoggingLevel::Warning,
                "through the notification_tx fallback",
            )
            .expect("the emitter always returns Ok");

        let received = rx.try_recv().expect("the fallback sink must deliver");
        match received {
            Notification::Server(crate::types::ServerNotification::LogMessage(params)) => {
                assert_eq!(params.message, "through the notification_tx fallback");
                assert_eq!(params.level, LoggingLevel::Warning);
            },
            other => panic!("expected a LogMessage notification, got {other:?}"),
        }
    }

    /// The precedence rule measured on THIS root too, with both sources live at
    /// once — the `ServerCore` twin cannot run this, because it has no
    /// `notification_tx` to lose to.
    #[tokio::test]
    async fn the_request_scoped_sink_wins_over_the_notification_tx_fallback() {
        let mut server = bare_server();
        let (tx, mut rx) = mpsc::channel(4);
        server.notification_tx = Some(tx);
        let request_scoped = Capture::default();

        let extra =
            server.attach_peer(extra_with_context(Some(context_with_sink(&request_scoped))));
        extra
            .log(LoggingLevel::Warning, "which sink received me?")
            .expect("the emitter always returns Ok");

        assert_eq!(
            request_scoped.len(),
            1,
            "the session-bound transport sink must win at the `Server` root exactly as it does at \
             the `ServerCore` root (T-118.2-06-02/03)"
        );
        assert!(
            rx.try_recv().is_err(),
            "the server-wide channel must NOT also receive one session's record"
        );
    }

    /// D-07, stated as a test: ONE request with NO `progressToken` gets a LIVE
    /// log sink and a `None` progress reporter.
    ///
    /// The gate moved off the sink and stayed on the reporter. Unifying the two
    /// would either silence logs for every client that never asked for progress,
    /// or make progress notifications unconditional — a client that sent no token
    /// has nothing to correlate them with (T-118.2-06-04).
    #[tokio::test]
    async fn the_progress_token_gate_still_applies_to_progress_only() {
        let mut server = bare_server();
        let (tx, _rx) = mpsc::channel(4);
        server.notification_tx = Some(tx);
        let capture = Capture::default();
        let context = context_with_sink(&capture);

        assert!(
            server.progress_reporter_for(None, Some(&context)).is_none(),
            "no `params._meta.progressToken` must still mean no progress reporter"
        );

        let extra = server.attach_peer(extra_with_context(Some(context)));
        assert!(
            extra.log_sink.is_some(),
            "the log sink is UNGATED by the progress token — a client that never asked for \
             progress must still receive `notifications/message` (D-07)"
        );
        extra
            .log(LoggingLevel::Info, "no progress token on this request")
            .expect("the emitter always returns Ok");
        assert_eq!(
            capture.len(),
            1,
            "the record must reach the client even though this request has no progress reporter"
        );
    }

    /// There is exactly ONE `notification_tx`-to-sink conversion, and both
    /// consumers read it. Measured behaviourally rather than by grep: the
    /// progress path and the log path must produce sinks that reach the SAME
    /// channel with the SAME non-blocking discipline.
    #[tokio::test]
    async fn the_progress_and_log_paths_share_one_notification_tx_sink() {
        let mut server = bare_server();
        let (tx, mut rx) = mpsc::channel(4);
        server.notification_tx = Some(tx);

        let via_progress = server
            .progress_notification_sink(None)
            .expect("a server with a notification_tx has a sink");
        let via_log = server
            .notification_tx_sink()
            .expect("the same server has the same sink");

        via_progress(Notification::Server(
            crate::types::ServerNotification::LogMessage(crate::types::LogMessageParams::new(
                LoggingLevel::Info,
                "via progress".to_string(),
            )),
        ));
        via_log(Notification::Server(
            crate::types::ServerNotification::LogMessage(crate::types::LogMessageParams::new(
                LoggingLevel::Info,
                "via log".to_string(),
            )),
        ));

        assert!(rx.try_recv().is_ok(), "the progress-derived sink delivers");
        assert!(rx.try_recv().is_ok(), "the log-derived sink delivers too");
    }
}
