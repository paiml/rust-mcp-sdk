//! Transport-independent MCP server core implementation.
//!
//! This module provides the core server functionality that is decoupled from
//! transport mechanisms, enabling deployment to various environments including
//! WASM/WASI targets.

use crate::error::{Error, Result};
use crate::server::limits::PayloadLimits;
use crate::shared::middleware::{EnhancedMiddlewareChain, MiddlewareContext};
use crate::shared::protocol_helpers::{create_notification, create_request};
// `ResponsePayload` is needed by the wasm-only envelope-builder branch (the
// non-wasm path delegates to `task_dispatch`) and by the test module.
// `JSONRPCError` is needed only by the wasm-only branch.
#[cfg(any(target_arch = "wasm32", test))]
use crate::types::jsonrpc::ResponsePayload;
#[cfg(target_arch = "wasm32")]
use crate::types::JSONRPCError;
// The 2026-07-28 caching-hint projection (115-06, SCHM-03). DEFINED in
// `crate::types::caching`, never here: `src/server/core.rs` is
// `cfg(not(target_arch = "wasm32"))`-shaped and `src/server/wasm_server.rs` is
// `cfg(target_arch = "wasm32")`, so a projector living in either server module
// would be structurally unreachable from the other — which is exactly how a v1
// leak on the wasm dispatcher would have shipped. This file CALLS it.
use crate::types::caching::{project_caching_hints, Cacheable};
use crate::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, Content, GetPromptRequest,
    GetPromptResult, Implementation, InitializeRequest, InitializeResult, JSONRPCResponse,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest,
    ListToolsResult, Notification, PromptInfo, ProtocolVersion, ReadResourceRequest,
    ReadResourceResult, Request, RequestId, ServerCapabilities, ToolInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
use std::sync::Arc;

use crate::runtime::RwLock;

#[cfg(not(target_arch = "wasm32"))]
use super::auth::{AuthContext, AuthProvider, ToolAuthorizer};
#[cfg(not(target_arch = "wasm32"))]
use super::cancellation::{CancellationManager, RequestHandlerExtra};
#[cfg(not(target_arch = "wasm32"))]
use super::roots::RootsManager;
#[cfg(not(target_arch = "wasm32"))]
use super::subscriptions::SubscriptionManager;
#[cfg(not(target_arch = "wasm32"))]
use super::tasks::TaskRouter;
#[cfg(not(target_arch = "wasm32"))]
use super::tool_middleware::{ToolContext, ToolMiddlewareChain};
use super::{PromptHandler, ResourceHandler, SamplingHandler, ToolHandler};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::tools::TaskSupport;

/// Protocol-agnostic request handler trait.
///
/// This trait defines the core interface for handling MCP protocol requests
/// without any dependency on transport mechanisms. Implementations can be
/// deployed to various environments including WASM/WASI.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle a single request and return a response.
    ///
    /// This method processes MCP requests in a stateless manner without
    /// knowledge of the underlying transport mechanism.
    ///
    /// # Parameters
    ///
    /// * `id` - The request ID from the JSON-RPC request
    /// * `request` - The MCP protocol request to handle
    /// * `auth_context` - Optional authentication context from the transport layer
    ///
    /// The `auth_context` parameter enables OAuth token pass-through from the
    /// transport layer to tool middleware, allowing tools to authenticate with
    /// backend services using the user's credentials.
    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse;

    /// Handle a notification (no response expected).
    ///
    /// Notifications are one-way messages that don't require a response.
    async fn handle_notification(&self, notification: Notification) -> Result<()>;

    /// Get server capabilities.
    ///
    /// Returns the capabilities that this server supports.
    fn capabilities(&self) -> &ServerCapabilities;

    /// Get server information.
    ///
    /// Returns metadata about the server implementation.
    fn info(&self) -> &Implementation;
}

/// Protocol handler trait for WASM environments (single-threaded).
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait ProtocolHandler {
    /// Handle a single request and return a response.
    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse;

    /// Handle a notification (no response expected).
    async fn handle_notification(&self, notification: Notification) -> Result<()>;

    /// Get server capabilities.
    fn capabilities(&self) -> &ServerCapabilities;

    /// Get server information.
    fn info(&self) -> &Implementation;
}

/// Enrich a tool's `_meta` with host-specific keys.
///
/// Reads the standard `ui.resourceUri` and adds host-specific aliases.
/// For `ChatGpt`, this adds `openai/outputTemplate`, `openai/widgetAccessible`,
/// and default `openai/toolInvocation/*` messages. Uses `entry().or_insert` so
/// server-provided values are never overwritten.
#[cfg(feature = "mcp-apps")]
pub(crate) fn enrich_meta_for_host(
    meta: &mut serde_json::Map<String, serde_json::Value>,
    host: crate::types::mcp_apps::HostType,
) {
    use crate::types::mcp_apps::HostType;

    if host == HostType::ChatGpt {
        // Extract URI from standard nested key
        if let Some(uri) = meta
            .get("ui")
            .and_then(|v| v.get("resourceUri"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            meta.entry("openai/outputTemplate".to_string())
                .or_insert_with(|| serde_json::Value::String(uri));
            meta.entry("openai/widgetAccessible".to_string())
                .or_insert(serde_json::Value::Bool(true));
            meta.entry("openai/toolInvocation/invoking".to_string())
                .or_insert_with(|| serde_json::Value::String("Running...".into()));
            meta.entry("openai/toolInvocation/invoked".to_string())
                .or_insert_with(|| serde_json::Value::String("Done".into()));
        }
    }
    // Claude, McpUi, Generic: no enrichment needed (standard keys only)
}

/// Keys to propagate from tool `_meta` to resource `_meta` via the URI index.
///
/// Includes the standard `ui` nested object and all `openai/*` descriptor keys
/// (which are only present if a host layer was applied). Display-only keys
/// (`openai/widgetPrefersBorder`, `openai/widgetDescription`, `openai/widgetCSP`,
/// `openai/widgetDomain`) are excluded to avoid breaking `ChatGPT`'s Templates.
const RESOURCE_PROPAGATION_PREFIXES: &[&str] = &[
    "openai/outputTemplate",
    "openai/toolInvocation/",
    "openai/widgetAccessible",
];

/// Build a URI-to-tool-meta index from registered tool metadata.
///
/// Maps resource URIs (from `ui.resourceUri` nested key) to the linked tool's
/// propagation-eligible `_meta` keys. Used to auto-propagate widget descriptor
/// keys onto `ResourceInfo` during `resources/list` and `resources/read`.
/// When multiple tools share the same URI, first tool registered wins.
pub(crate) fn build_uri_to_tool_meta(
    tool_infos: &HashMap<String, ToolInfo>,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for info in tool_infos.values() {
        if let Some(meta) = info.widget_meta() {
            // Index by standard nested ui.resourceUri key
            let uri = meta
                .get("ui")
                .and_then(|v| v.get("resourceUri"))
                .and_then(|v| v.as_str());
            if let Some(uri) = uri {
                // Collect propagation-eligible keys
                let propagated: serde_json::Map<String, serde_json::Value> = meta
                    .iter()
                    .filter(|(k, _)| {
                        RESOURCE_PROPAGATION_PREFIXES
                            .iter()
                            .any(|prefix| k.starts_with(prefix))
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                // First tool registered wins (per user decision).
                // Skip empty propagation maps to avoid `_meta: {}` on resources/list.
                if !propagated.is_empty() {
                    map.entry(uri.to_string()).or_insert(propagated);
                }
            }
        }
    }
    map
}

// ===========================================================================
// `completion/complete` (Phase 118.1-04, CONF-05 / G-4).
//
// ONE shared unit, called from BOTH native dispatch sites — `ServerCore` below
// and the high-level `Server` in `server/mod.rs`. That is the Phase-109/112
// twin-site parity rule this file already states for the MRTR unit further
// down: `mod.rs` CALLS this helper, it never defines its own.
//
// Before this unit existed the two dispatchers DISAGREED on the method:
// `Server` answered `json!({})` from a five-method catch-all and `ServerCore`
// answered `-32601` from its `_ =>` arm. Only `Server` is on the HTTP path, so
// the official conformance suite could only ever measure half the defect.
// ===========================================================================

/// The spec's `@maxItems 100` bound on `CompleteResult.completion.values`.
///
/// Source: `schema/vendored/core-2026-07-28/schema.ts:2644-2663`, which
/// annotates `values: string[]` with `@maxItems 100` in prose that no generated
/// Rust type carries. A completion provider is driven by peer-chosen input
/// (`ref` + a partial `argument.value`), so an unbounded array copied straight
/// out of the provider is a denial-of-service surface as well as a conformance
/// violation (T-118.1-04-01).
const MAX_COMPLETION_VALUES: usize = 100;

/// Shape a `completion/complete` answer from an optional registered provider.
///
/// # The no-provider contract
///
/// A server with no completion provider registered answers a SUCCESS carrying
/// an EMPTY array — never an error, never a bare `{}`. The pinned conformance
/// suite's own comment sanctions exactly this ("completion support can be
/// minimal or return empty arrays"), and it is the shape the overwhelming
/// majority of servers will emit, so it is the default rather than a special
/// case.
///
/// # The `@maxItems 100` bound
///
/// `values` is TRUNCATED to [`MAX_COMPLETION_VALUES`] rather than rejected: a
/// provider returning 150 candidates is not malformed, it is simply more
/// specific than the wire allows, and refusing the whole answer would turn a
/// working completion into an error. Truncation is reported honestly:
///
/// - `has_more` is set when the provider itself reported more OR when this
///   function dropped elements, so a client never reads a truncated list as
///   exhaustive.
/// - `total` is `Some(n)` ONLY when the provider returned everything it had
///   (`has_more == false` on its side), in which case `n` is the true total.
///   When the provider itself claims more exist, the true total is unknown and
///   `total` stays `None` — inventing one would be worse than omitting an
///   optional field.
///
/// # The ref
///
/// [`CompletionRequest`](crate::types::completable::CompletionRequest) carries
/// no dedicated ref slot, so the reference is threaded through its `context`
/// map under the spec's own discriminator spelling — `ref/prompt` or
/// `ref/resource` (`CompletionReference`'s serde renames). A provider that
/// needs to know WHICH prompt or resource is being completed reads it there;
/// one that does not can ignore it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn complete_completion(
    provider: Option<&Arc<dyn crate::types::completable::CompletionProviderTrait>>,
    request: &crate::types::protocol::CompleteRequest,
) -> Result<crate::types::protocol::CompleteResult> {
    use crate::types::protocol::{CompleteResult, CompletionResult};

    let Some(provider) = provider else {
        return Ok(CompleteResult {
            completion: CompletionResult::new(Vec::new()),
        });
    };

    let response = provider.complete(completion_request_for(request)).await?;
    Ok(CompleteResult {
        completion: bound_completion_values(response),
    })
}

/// Project a wire [`CompleteRequest`](crate::types::protocol::CompleteRequest)
/// onto the SDK's provider-facing
/// [`CompletionRequest`](crate::types::completable::CompletionRequest).
#[cfg(not(target_arch = "wasm32"))]
fn completion_request_for(
    request: &crate::types::protocol::CompleteRequest,
) -> crate::types::completable::CompletionRequest {
    use crate::types::protocol::CompletionReference;

    let mut context = HashMap::new();
    match &request.r#ref {
        CompletionReference::Prompt { name } => {
            context.insert(COMPLETION_REF_PROMPT_KEY.to_string(), name.clone());
        },
        CompletionReference::Resource { uri } => {
            context.insert(COMPLETION_REF_RESOURCE_KEY.to_string(), uri.clone());
        },
    }
    crate::types::completable::CompletionRequest {
        argument: request.argument.name.clone(),
        partial: request.argument.value.clone(),
        context,
    }
}

/// The `context` key carrying a `ref/prompt` reference's prompt name.
///
/// Spelled exactly as the wire discriminator (`CompletionReference`'s serde
/// rename), so a provider matching on it is matching on the protocol's own
/// vocabulary rather than on an SDK-invented alias.
const COMPLETION_REF_PROMPT_KEY: &str = "ref/prompt";

/// The `context` key carrying a `ref/resource` reference's URI.
const COMPLETION_REF_RESOURCE_KEY: &str = "ref/resource";

/// Apply the spec's `@maxItems 100` bound to a provider's answer.
///
/// Split out of [`complete_completion`] so the bound has a unit-testable seam
/// that does not require constructing a provider — the truncation arithmetic is
/// the part a regression would silently break.
#[cfg(not(target_arch = "wasm32"))]
fn bound_completion_values(
    response: crate::types::completable::CompletionResponse,
) -> crate::types::protocol::CompletionResult {
    use crate::types::protocol::CompletionResult;

    let available = response.completions.len();
    let truncated = available > MAX_COMPLETION_VALUES;
    let values: Vec<String> = response
        .completions
        .into_iter()
        .take(MAX_COMPLETION_VALUES)
        .map(|item| item.value)
        .collect();

    let mut result = CompletionResult::new(values).with_has_more(response.has_more || truncated);
    if !response.has_more {
        // The provider returned everything it had, so `available` IS the total —
        // including the elements this function just dropped.
        result = result.with_total(available);
    }
    result
}

/// The message this file's dispatch root answers a method it does not serve.
///
/// ONE literal, read by the `_ =>` catch-all in
/// [`ServerCore::handle_request_internal`] AND by
/// [`set_logging_level_response`]'s v2 retirement, so `logging/setLevel` is
/// refused in exactly the same words as its three residual siblings (`ping`,
/// `resources/subscribe`, `resources/unsubscribe`) on this same root rather
/// than acquiring a message of its own.
///
/// # Why this does NOT reuse the HTTP layer's wording
///
/// The HTTP ingress builds a richer string from `V2_RETIRED_METHODS`
/// (`"Method not found: logging/setLevel (retired in MCP 2026-07-28; use
/// io.modelcontextprotocol/logLevel)"`). That table lives in
/// `crate::server::streamable_http_server`, which is gated behind
/// `feature = "streamable-http"` and so cannot be referenced from this module
/// at all. What the two layers DO share is the thing that matters on the wire:
/// the same decision and the same code,
/// [`error_codes::METHOD_NOT_FOUND`](crate::types::protocol::error_codes::METHOD_NOT_FOUND).
/// Only the human-readable text differs — exactly the split
/// `map_unparsed_body_for_v2`'s rustdoc already records for the two
/// pre-existing v2 rejection routes ("Both routes emit the same code and the
/// same status … only the message text differs").
pub(crate) const METHOD_NOT_SUPPORTED_MESSAGE: &str = "Method not supported";

/// Whether `logging/setLevel` is RETIRED for this era.
///
/// `None` — no era resolved, i.e. a server that never opted in to protocol
/// negotiation — is treated as v1, which is the era such a server has always
/// been on. Retiring a method for a caller whose era was never determined would
/// break every non-opted-in server on the SDK's own default path.
pub(crate) const fn set_logging_level_is_retired(era: Option<crate::types::protocol::Era>) -> bool {
    matches!(era, Some(crate::types::protocol::Era::V2))
}

/// The v1 answer to `logging/setLevel`: a **literal empty object**.
///
/// # Pitfall 8 — this is a MEASURED constraint, not a style choice
///
/// The pinned official conformance suite's `2025-11-25:logging-set-level`
/// scenario does
///
/// ```text
/// const r = await n.request('logging/setLevel', { level: 'info' });
/// r && Object.keys(r).length > 0 && i.push('Expected empty object {} response')
/// ```
///
/// so ANY non-empty object is a FAILURE — including an acknowledgement object
/// and including an echo of the level that was just set. That scenario is
/// currently GREEN and is one of the entries in `BLOCKING_GREEN_SCENARIOS`;
/// returning anything richer here would shrink the blocking surface the
/// conformance gate claims, i.e. a regression dressed as an improvement.
///
/// Echoing the stored level would additionally hand a caller a READ of session
/// state through a WRITE endpoint (T-118.2-08-02).
///
/// The level itself is already stored, per session, by the HTTP ingress
/// (`streamable_http_server::capture_v1_set_level`, Phase 118.2-07). This
/// dispatch path's only job is to ANSWER.
pub(crate) fn set_logging_level_v1_result() -> serde_json::Value {
    serde_json::json!({})
}

/// The ONE answer both native dispatch roots give `logging/setLevel` (D-13).
///
/// Before Phase 118.2-08 the two roots disagreed about a method the official
/// conformance suite measures: [`ServerCore`]'s `_ =>` catch-all answered
/// `-32601 Method not supported` on BOTH eras, while
/// `Server::process_client_request` answered `json!({})` on both. Phase 118.1
/// spent real effort collapsing exactly this class of divergence for
/// `completion/complete`; this function is the same fix for the same shape of
/// defect.
///
/// | Era | Answer |
/// |-----|--------|
/// | `V2` | JSON-RPC error [`METHOD_NOT_FOUND`](crate::types::protocol::error_codes::METHOD_NOT_FOUND) — the RPC is retired, and `io.modelcontextprotocol/logLevel` in `params._meta` replaces it |
/// | `V1` / no era resolved | success carrying [`set_logging_level_v1_result`] — a literal `{}` |
///
/// # Why the roots CALL this rather than each writing the branch
///
/// `src/server/mod.rs` calls the units defined here; it never defines its own.
/// Two copies of an era branch are two chances to disagree, and the disagreement
/// would be silent — which is precisely the state this function replaces.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn set_logging_level_response(
    id: RequestId,
    era: Option<crate::types::protocol::Era>,
) -> JSONRPCResponse {
    if set_logging_level_is_retired(era) {
        return crate::server::task_dispatch::error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            METHOD_NOT_SUPPORTED_MESSAGE.to_string(),
        );
    }
    crate::server::task_dispatch::success_response(id, set_logging_level_v1_result())
}

/// Core server implementation without transport dependencies.
///
/// This struct contains all the business logic for an MCP server without
/// any coupling to specific transport mechanisms. It can be used with
/// various transport adapters to deploy to different environments.
#[allow(dead_code)]
#[allow(missing_debug_implementations)]
pub struct ServerCore {
    /// Server metadata
    info: Implementation,

    /// Server capabilities
    capabilities: ServerCapabilities,

    /// Registered tool handlers
    tools: HashMap<String, Arc<dyn ToolHandler>>,

    /// Registered prompt handlers
    prompts: HashMap<String, Arc<dyn PromptHandler>>,

    /// Cached tool metadata (populated at registration, immutable)
    tool_infos: HashMap<String, ToolInfo>,

    /// Cached URI-to-tool-meta index for widget resource `_meta` propagation.
    /// Maps resource URIs (from `ui.resourceUri`) to propagation-eligible `_meta` keys.
    uri_to_tool_meta: HashMap<String, serde_json::Map<String, serde_json::Value>>,

    /// Cached prompt metadata (populated at registration, immutable)
    prompt_infos: HashMap<String, PromptInfo>,

    /// Resource handler (optional)
    resources: Option<Arc<dyn ResourceHandler>>,

    /// Sampling handler (optional)
    sampling: Option<Arc<dyn SamplingHandler>>,

    /// Completion provider backing `completion/complete` (optional, Phase
    /// 118.1-04 / CONF-05).
    ///
    /// A SINGLE, non-keyed slot in the shape of `resources` above, not the
    /// name-keyed `prompts` map: the spec routes every `completion/complete` to
    /// one server-wide provider and passes the `ref` as data, so a per-name
    /// registry would be inventing a dispatch dimension the protocol does not
    /// have. Threaded from
    /// [`ServerCoreBuilder::completions`](crate::server::builder::ServerCoreBuilder::completions)
    /// via [`ServerCore::with_completions`]; the high-level `Server` carries an
    /// IDENTICALLY-shaped field so both dispatchers consult the same seam.
    ///
    /// `None` still answers the spec shape — an empty `values` array — rather
    /// than an error. See [`complete_completion`].
    #[cfg(not(target_arch = "wasm32"))]
    completions: Option<Arc<dyn crate::types::completable::CompletionProviderTrait>>,

    /// Client capabilities (set during initialization)
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,

    /// Server initialization state
    initialized: Arc<RwLock<bool>>,

    /// Cancellation manager for request cancellation
    cancellation_manager: CancellationManager,

    /// Roots manager for directory/URI registration
    roots_manager: Arc<RwLock<RootsManager>>,

    /// Subscription manager for resource subscriptions
    subscription_manager: Arc<RwLock<SubscriptionManager>>,

    /// Authentication provider (optional)
    auth_provider: Option<Arc<dyn AuthProvider>>,

    /// Tool authorizer for fine-grained access control (optional)
    tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,

    /// Protocol middleware chain for request/response/notification processing
    protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,

    /// Tool middleware chain for cross-cutting concerns in tool execution
    #[cfg(not(target_arch = "wasm32"))]
    tool_middleware: Arc<RwLock<ToolMiddlewareChain>>,

    /// Task router for experimental MCP Tasks support (optional)
    #[cfg(not(target_arch = "wasm32"))]
    task_router: Option<Arc<dyn TaskRouter>>,

    /// Task store for MCP Tasks with polling (standard capability path)
    #[cfg(not(target_arch = "wasm32"))]
    task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,

    /// Per-tool TOUT-02 double-wrap tripwire opt-out set (D-08). A tool named
    /// here has the tripwire suppressed at the Payload wrap tail. Populated via
    /// [`ServerCore::with_suppress_double_wrap`] from
    /// `ServerCoreBuilder::suppress_double_wrap_check`; the high-level `Server`
    /// carries an IDENTICAL set so both dispatchers consult the same rule.
    #[cfg(not(target_arch = "wasm32"))]
    suppress_double_wrap: HashSet<String>,

    /// Stateless mode flag for serverless deployments
    ///
    /// When true, the server skips initialization state checking, allowing
    /// requests to be processed without requiring an initialize call first.
    /// This is essential for stateless environments like AWS Lambda, Cloudflare
    /// Workers, and other serverless platforms where each request may create
    /// a fresh server instance.
    ///
    /// Default: false (maintains backward compatibility)
    stateless_mode: bool,

    /// Payload and resource limits for denial-of-service protection
    payload_limits: PayloadLimits,

    /// The configured protocol-version accept-list (Phase 112, VERS-01/02).
    ///
    /// Defaults to the v1-only legacy set ([`default_accept_list`](crate::types::protocol::context::default_accept_list),
    /// which EXCLUDES `2026-07-28`) unless the author opts into v2 via
    /// [`ServerCoreBuilder::with_supported_protocol_versions`](crate::server::builder::ServerCoreBuilder::with_supported_protocol_versions).
    /// Read at ingress to decide whether to run era-detection at all
    /// ([`is_v2_opted_in`](Self::is_v2_opted_in)) and to enforce the accept-list
    /// in the shared resolver. A non-opted-in server behaves exactly as today.
    supported_protocol_versions: Vec<ProtocolVersion>,

    /// The server-owned `requestState` codec (Phase 113, HTTP-02).
    ///
    /// Resolved EXACTLY ONCE at
    /// [`ServerCoreBuilder::build`](crate::server::builder::ServerCoreBuilder::build)
    /// time and threaded in via [`ServerCore::with_request_state_codec`] — never a
    /// process-global `OnceLock`, so two differently-configured cores can coexist
    /// in one process and integration tests can inject a deterministic key and
    /// clock. `None` for a core that did not opt into the v2 (`2026-07-28`) era:
    /// such a core reads no MRTR environment variable and pays nothing (D-04).
    #[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
    request_state_codec: Option<Arc<crate::server::request_state::RequestStateCodec>>,

    /// Outbound server-to-client request dispatcher.
    ///
    /// Populated by the enclosing `Server` via
    /// [`ServerCore::with_server_request_dispatcher`]. Consumed at dispatch
    /// sites to construct per-request peer handles via `attach_peer`.
    /// `None` preserves the graceful-fallback contract for every existing
    /// `ServerCore::new()` call site.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::struct_field_names)]
    server_request_dispatcher:
        Option<Arc<crate::server::server_request_dispatcher::ServerRequestDispatcher>>,

    /// Cached peer handle built alongside the dispatcher.
    /// One Arc allocation at setup time; dispatch sites clone this Arc
    /// (refcount bump) rather than constructing a fresh `DispatchPeerHandle`
    /// per request.
    #[cfg(not(target_arch = "wasm32"))]
    peer_handle: Option<Arc<dyn crate::shared::peer::PeerHandle>>,
}

/// Outcome of a tool handler call — either a normal result or a task creation.
enum ToolCallOutcome {
    /// Standard tool result wrapped as `CallToolResult`
    Result(CallToolResult),
    /// Tool returned a Task-shaped value — returned as `CreateTaskResult` with `_meta`.
    ///
    /// Carries the raw task-shaped tool `Value`. The shared
    /// `task_dispatch::TaskDispatch::build_task_created_response` re-extracts the
    /// task id and the terminal [`CallToolResult`] from this value (store mints the
    /// canonical id; terminal result drives synchronous-completion persistence).
    #[cfg(not(target_arch = "wasm32"))]
    TaskCreated { task_value: Value },
}

impl ServerCore {
    /// Create a new `ServerCore` with the given configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        info: Implementation,
        capabilities: ServerCapabilities,
        tools: HashMap<String, Arc<dyn ToolHandler>>,
        prompts: HashMap<String, Arc<dyn PromptHandler>>,
        tool_infos: HashMap<String, ToolInfo>,
        prompt_infos: HashMap<String, PromptInfo>,
        resources: Option<Arc<dyn ResourceHandler>>,
        sampling: Option<Arc<dyn SamplingHandler>>,
        auth_provider: Option<Arc<dyn AuthProvider>>,
        tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
        protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,
        #[cfg(not(target_arch = "wasm32"))] tool_middleware: Arc<RwLock<ToolMiddlewareChain>>,
        #[cfg(not(target_arch = "wasm32"))] task_router: Option<Arc<dyn TaskRouter>>,
        #[cfg(not(target_arch = "wasm32"))] task_store: Option<
            Arc<dyn crate::server::task_store::TaskStore>,
        >,
        stateless_mode: bool,
        payload_limits: PayloadLimits,
    ) -> Self {
        let uri_to_tool_meta = build_uri_to_tool_meta(&tool_infos);
        Self {
            info,
            capabilities,
            tools,
            prompts,
            tool_infos,
            uri_to_tool_meta,
            prompt_infos,
            resources,
            sampling,
            #[cfg(not(target_arch = "wasm32"))]
            completions: None,
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            cancellation_manager: CancellationManager::new(),
            roots_manager: Arc::new(RwLock::new(RootsManager::new())),
            subscription_manager: Arc::new(RwLock::new(SubscriptionManager::new())),
            auth_provider,
            tool_authorizer,
            protocol_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            tool_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            task_router,
            #[cfg(not(target_arch = "wasm32"))]
            task_store,
            #[cfg(not(target_arch = "wasm32"))]
            suppress_double_wrap: HashSet::new(),
            stateless_mode,
            payload_limits,
            supported_protocol_versions: crate::types::protocol::context::default_accept_list(),
            #[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
            request_state_codec: None,
            #[cfg(not(target_arch = "wasm32"))]
            server_request_dispatcher: None,
            #[cfg(not(target_arch = "wasm32"))]
            peer_handle: None,
        }
    }

    /// Attach a server-to-client request dispatcher.
    ///
    /// The dispatcher is the outbound-plus-correlation layer consumed at
    /// handler dispatch sites so tool handlers can invoke
    /// `extra.peer()?.sample(...)` mid-execution. Calling this is optional —
    /// when absent, existing behaviour (no peer handle) is preserved.
    ///
    /// Also constructs and caches a reusable `Arc<dyn PeerHandle>` so
    /// per-request dispatch only clones the Arc (refcount bump), not
    /// allocating a new `DispatchPeerHandle` each time.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_server_request_dispatcher(
        mut self,
        dispatcher: Arc<crate::server::server_request_dispatcher::ServerRequestDispatcher>,
    ) -> Self {
        let peer: Arc<dyn crate::shared::peer::PeerHandle> = Arc::new(
            crate::server::peer_impl::DispatchPeerHandle::new(dispatcher.clone()),
        );
        self.peer_handle = Some(peer);
        self.server_request_dispatcher = Some(dispatcher);
        self
    }

    /// Carry the per-tool TOUT-02 double-wrap tripwire opt-out set (D-08) from
    /// the builder into the running `ServerCore`.
    ///
    /// Threaded from `ServerCoreBuilder::build` so the tripwire at the Payload
    /// wrap tail consults the SAME suppression set the high-level `Server` uses —
    /// the two dispatchers can never drift on which tools are suppressed. An
    /// empty set (the default) preserves the tripwire for every tool.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_suppress_double_wrap(mut self, suppress: HashSet<String>) -> Self {
        self.suppress_double_wrap = suppress;
        self
    }

    /// Carry the `completion/complete` provider (Phase 118.1-04, CONF-05) from
    /// the builder into the running `ServerCore`.
    ///
    /// Threaded from
    /// [`ServerCoreBuilder::completions`](crate::server::builder::ServerCoreBuilder::completions)
    /// rather than added to [`ServerCore::new`]'s already-eighteen-argument
    /// signature, matching [`Self::with_suppress_double_wrap`] and
    /// [`Self::with_server_request_dispatcher`]. `None` (the default) still
    /// answers the spec `CompleteResult` shape with an empty `values` array.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_completions(
        mut self,
        provider: Option<Arc<dyn crate::types::completable::CompletionProviderTrait>>,
    ) -> Self {
        self.completions = provider;
        self
    }

    /// Carry the configured protocol-version accept-list (Phase 112, VERS-01/02)
    /// from the builder into the running `ServerCore`.
    ///
    /// Threaded from [`ServerCoreBuilder::build`](crate::server::builder::ServerCoreBuilder::build)
    /// so ingress era-resolution reads the exact set the author opted into. The
    /// builder guarantees a non-empty list (an explicitly-empty accept-list falls
    /// back to the v1-only default), so this never installs an all-reject server.
    #[must_use]
    pub(crate) fn with_supported_protocol_versions(
        mut self,
        versions: Vec<ProtocolVersion>,
    ) -> Self {
        self.supported_protocol_versions = versions;
        self
    }

    /// Carry the server-owned `requestState` codec (Phase 113, HTTP-02) from the
    /// builder into the running `ServerCore`.
    ///
    /// Threaded from [`ServerCoreBuilder::build`](crate::server::builder::ServerCoreBuilder::build),
    /// which resolves the codec exactly once. `None` means "this core did not opt
    /// into v2" — the MRTR paths are then never reachable.
    #[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) fn with_request_state_codec(
        mut self,
        codec: Option<Arc<crate::server::request_state::RequestStateCodec>>,
    ) -> Self {
        self.request_state_codec = codec;
        self
    }

    /// The server-owned `requestState` codec, or `None` when this core did not
    /// opt into the v2 (`2026-07-28`) era.
    ///
    /// Read on the production MRTR path by [`mrtr_ingest`] (verify) and
    /// [`mrtr_egress`] (mint) — borrowed from server state, never a
    /// process-global.
    #[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
    pub(crate) fn request_state_codec(
        &self,
    ) -> Option<&crate::server::request_state::RequestStateCodec> {
        self.request_state_codec.as_deref()
    }

    /// The configured protocol-version accept-list read at ingress (test-only
    /// accessor; production reads the field directly via the shared resolver).
    #[cfg(test)]
    pub(crate) fn supported_protocol_versions(&self) -> &[ProtocolVersion] {
        &self.supported_protocol_versions
    }

    /// Whether this server opted into the v2 (`2026-07-28`) era (test-only
    /// convenience over [`context::is_v2_opted_in`](crate::types::protocol::context::is_v2_opted_in);
    /// production resolves opt-in inside the shared ingress resolver).
    #[cfg(test)]
    pub(crate) fn is_v2_opted_in(&self) -> bool {
        crate::types::protocol::context::is_v2_opted_in(&self.supported_protocol_versions)
    }

    /// Resolve the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// ONCE at native ingress (Phase 112, VERS-01) via the shared free
    /// [`resolve_ingress_protocol_context`] both dispatch surfaces call. The
    /// `Err` is mapped to a structured rejection by the caller.
    fn resolve_ingress_protocol_context(
        &self,
        request: &Request,
    ) -> std::result::Result<
        Option<crate::types::protocol::ProtocolContext>,
        crate::types::protocol::context::ProtocolNegotiationError,
    > {
        resolve_ingress_protocol_context(&self.supported_protocol_versions, request)
    }

    /// Attach a peer handle to `extra`, preferring the REQUEST-SCOPED one.
    ///
    /// No-op on wasm32 (peer is non-wasm), and — when neither source is present
    /// — when no dispatcher is attached, exactly as before.
    ///
    /// # Precedence: the request-scoped transport handle wins (T-118.1-11-04)
    ///
    /// Two sources can supply a peer and they are NOT equivalent:
    ///
    /// 1. `self.peer_handle` — a SINGLE field on this core, set once by the
    ///    in-process actor loop. One transport, one client, so one handle says
    ///    everything there is to say.
    /// 2. the `TransportBackchannel` riding THIS request's `ProtocolContext`,
    ///    attached by the `StreamableHTTP` transport at the one site that knows
    ///    which session the request arrived on.
    ///
    /// On a MULTIPLEXED transport (1) cannot express "the session that issued
    /// this request": a handle set there is shared by every concurrent session,
    /// so one client's `sampling/createMessage` would be delivered to whichever
    /// session the global handle happened to be bound to — the T-113-07
    /// misbinding class. (2) is constructed per request and bound to the
    /// originating session, so it is always the more specific answer and is
    /// therefore read FIRST.
    ///
    /// The in-process path is untouched: that loop attaches no backchannel, so
    /// the `self.peer_handle` fallback below is what runs there.
    ///
    /// # Ordering
    ///
    /// Every dispatch site calls this AFTER its `tool_authorizer` check, so an
    /// unauthorized caller returns before a handler body ever runs and therefore
    /// never sees `extra.peer()` — the invariant stated at `src/shared/peer.rs`.
    ///
    /// Delegates to [`attach_request_peer`], the ONE unit `Server::attach_peer`
    /// also calls: the two dispatch roots must never disagree about which peer a
    /// handler sees, and sharing the body is what makes that structural rather
    /// than a claim two comments make about each other.
    ///
    /// # It attaches the LOG SINK too (Phase 118.2, CONF-10)
    ///
    /// The name is kept for its call-site history, but this is now the single
    /// post-authorization site where ALL of a request's server-to-client
    /// capability handles are attached: the peer, the log sink, and the resolved
    /// log level. They share one site DELIBERATELY. A second method that a future
    /// dispatch site had to remember to call is precisely the drift this file
    /// spends its comments preventing — every existing and future `attach_peer`
    /// caller gets the log sink for free, and cannot get one without the other.
    #[inline]
    fn attach_peer(&self, extra: RequestHandlerExtra) -> RequestHandlerExtra {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let extra = attach_request_peer(extra, self.peer_handle.as_ref());
            // The fallback is a literal `None`, and that asymmetry with `Server`
            // is real rather than an oversight: `ServerCore` has NO
            // `notification_tx` of any kind — that field exists only on
            // `Server` — so a request-scoped `TransportBackchannel` is the ONLY
            // sink this root can ever supply. Do not "fix" this by inventing a
            // channel here; the transport owns the back-channel.
            attach_request_log_sink(extra, || None)
        }
        #[cfg(target_arch = "wasm32")]
        {
            extra
        }
    }

    /// Get the configured payload limits.
    pub fn payload_limits(&self) -> &PayloadLimits {
        &self.payload_limits
    }

    /// Check if the server is initialized.
    pub async fn is_initialized(&self) -> bool {
        contract_pre_session_lifecycle!();
        *self.initialized.read().await
    }

    /// Get client capabilities if available.
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Handle initialization request.
    async fn handle_initialize(&self, init_req: &InitializeRequest) -> Result<InitializeResult> {
        contract_pre_session_lifecycle!();
        // Store client capabilities
        *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
        *self.initialized.write().await = true;

        let negotiated_version = crate::negotiate_protocol_version(&init_req.protocol_version);

        Ok(InitializeResult {
            protocol_version: ProtocolVersion(negotiated_version.to_string()),
            // Era projection (114-05, D-02): the stored struct carries what BOTH
            // eras want; this boundary emits the v1 view. See
            // `project_capabilities_for_v1`.
            capabilities: project_capabilities_for_v1(&self.capabilities),
            server_info: self.info.clone(),
            instructions: None,
        })
    }

    /// Handle list tools request.
    async fn handle_list_tools(&self, _req: &ListToolsRequest) -> Result<ListToolsResult> {
        contract_pre_tool_dispatch_integrity!();
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }

    /// Handle call tool request.
    async fn handle_call_tool(
        &self,
        req: &CallToolRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<ToolCallOutcome> {
        contract_pre_tool_dispatch_integrity!();
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Tool '{}' not found", req.name)))?;

        // Authorization check with tool_authorizer if available
        if let Some(authorizer) = &self.tool_authorizer {
            if let Some(ref auth_ctx) = auth_context {
                if !authorizer.can_access_tool(auth_ctx, &req.name).await? {
                    return Err(Error::authentication(format!(
                        "User not authorized to call tool '{}'",
                        req.name
                    )));
                }
            }
        }

        // Capture the ERA-AWARE create trigger BEFORE `protocol_context` is moved
        // into `extra` below (plan 114-12). `CreateTrigger::resolve` is the ONE
        // place the era picks a trigger — v1 reads `CallToolRequest.task`, v2
        // reads the client's tasks-extension declaration off this
        // already-resolved context — so this dispatcher and the high-level
        // `Server` can never implement different triggers. The declaration is
        // read from the resolved `ProtocolContext`, never by re-parsing
        // `params._meta` (Phase 112).
        #[cfg(not(target_arch = "wasm32"))]
        let create_trigger = crate::server::task_dispatch::CreateTrigger::resolve(
            protocol_context.as_ref().map(|ctx| ctx.era),
            req.task.is_some(),
            protocol_context.as_ref(),
        );

        // Same capture-before-move reason as `create_trigger` above: the
        // emit-time outputSchema validator is era-branched (Phase 115 D-01),
        // and `protocol_context` is moved into `extra` below. UN-cfg'd because
        // the validation call site compiles on wasm32 too.
        let validation_era = protocol_context.as_ref().map(|ctx| ctx.era);

        // Create request handler extra data with auth_context and task request.
        // Middleware below takes `&mut extra`, so bind as mut.
        let request_id = format!("tool_{}", req.name);
        let mut extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_task_request(req.task.clone())
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            // Thread the once-at-ingress resolved protocol context so handlers
            // read era/identity via extra.era()/client_info() (Phase 112).
            .with_protocol_context(protocol_context),
        );

        // D-03.3 (TOUT-01): clone the result-`_meta` slot before `extra` moves
        // into `handle_output` (see the high-level `Server` dispatcher for the
        // twin); drained onto the Payload envelope after the handler returns.
        #[cfg(not(target_arch = "wasm32"))]
        let result_meta_handle = extra.result_meta_handle();

        // Execute tool with or without middleware depending on platform
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            // Create tool context for middleware
            let context = ToolContext::new(&req.name, &request_id);

            // Clone arguments for middleware processing
            let mut args = req.arguments.clone();

            // Process request through tool middleware chain.
            // Middleware rejection short-circuits tool execution (on_error already
            // called by chain). REQUEST middleware runs BEFORE the handler for
            // EVERY tool, regardless of the ToolOutput variant it returns.
            self.tool_middleware
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Enforce tool argument size limit (post-middleware, so inflated args are caught)
            if self.payload_limits.max_tool_args_bytes < usize::MAX {
                let args_size = json_serialized_len(&args)?;
                if args_size > self.payload_limits.max_tool_args_bytes {
                    return Err(Error::validation(format!(
                        "Tool arguments for '{}' exceed size limit ({} bytes > {} max)",
                        req.name, args_size, self.payload_limits.max_tool_args_bytes
                    )));
                }
            }

            // Execute the tool. `handle_output` returns `Result<ToolOutput>`; the
            // SHARED `resolve_tool_output` (D-05) is the SINGLE place that decides
            // Payload-vs-Result and encodes the response-middleware-bypass rule, so
            // this dispatcher and the high-level `Server` can never drift on it.
            let output = handler.handle_output(args, extra).await;
            match crate::server::task_dispatch::resolve_tool_output(output) {
                // VERBATIM (D-04 + D-04a — USER-APPROVED and LOCKED: "keep the
                // bypass, harden it"): the handler owns the full `CallToolResult`
                // envelope, including its own redaction/sanitization. Emit it as-is
                // — bypassing RESPONSE middleware (redaction/sanitization/audit),
                // the create-path gate, and the text-wrap / widget-enrichment tail.
                // REQUEST middleware already fired above for every tool, and a
                // handler `Err(_)` still routes through the Middleware arm below.
                //
                // D-06 (Phase 118.1) RECLASSIFIES exactly one clause of D-04a:
                // the bypass covers the response PIPELINE, not the handler's own
                // `extra.set_result_meta(..)`. Those keys are authored by the same
                // handler that authored this envelope, at the same trust level, so
                // draining them here merges a handler's two `_meta` sources rather
                // than reintroducing server-side rewriting. Handler-key-wins
                // precedence, never a whole-map replace. G-3's elicitation wiring
                // runs through this arm, which is why the drop was load-bearing.
                crate::server::task_dispatch::DispatchOutput::Verbatim(call_result) => {
                    let mut call_result = call_result;
                    if let Some(handler_meta) = result_meta_handle.take_result_meta() {
                        crate::server::cancellation::merge_result_meta(
                            &mut call_result,
                            handler_meta,
                        );
                    }
                    return Ok(ToolCallOutcome::Result(call_result));
                },
                crate::server::task_dispatch::DispatchOutput::Middleware(mut result) => {
                    // Process response through tool middleware chain (Payload/error only)
                    if let Err(e) = self
                        .tool_middleware
                        .read()
                        .await
                        .process_response(&req.name, &mut result, &context)
                        .await
                    {
                        // Log error but continue with original result
                        tracing::warn!("Tool response middleware processing failed: {}", e);
                    }

                    // If tool execution failed, call handle_tool_error
                    if let Err(ref e) = result {
                        self.tool_middleware
                            .read()
                            .await
                            .handle_tool_error(&req.name, e, &context)
                            .await;
                    }

                    result
                },
            }
        };

        #[cfg(target_arch = "wasm32")]
        let result = {
            // On WASM, execute tool directly without middleware
            let args = req.arguments.clone();
            handler.handle(args, extra).await
        };

        // Convert result to CallToolResult.
        //
        // `Error::ToolRejected` is an APPLICATION-level rejection (e.g. Code
        // Mode policy: a SELECT missing its LIMIT), not a protocol fault. Map
        // it to a successful `CallToolResult { isError: true }` so the model
        // reads the reason + suggestions and retries with corrected input —
        // rather than `?`-propagating it into a JSON-RPC error that reads as a
        // server crash. All other errors keep propagating as protocol errors.
        let value = match result {
            Ok(value) => value,
            Err(crate::error::Error::ToolRejected { message, details }) => {
                return Ok(ToolCallOutcome::Result(CallToolResult::rejected(
                    message, details,
                )));
            },
            Err(e) => return Err(e),
        };
        let tool_info = self.tool_infos.get(&req.name);

        // Task detection: return CreateTaskResult only when the SHARED,
        // era-aware create gate says so.
        //
        // This site used to carry its OWN copy of the rule — a `req.task.is_some()
        // && self.task_store.is_some() && …` expression plus a second copy of the
        // task-shape check, under a comment admitting it was the "same shape gate
        // as `task_dispatch::maybe_build_task_created`". That is exactly the
        // divergent second copy the task_dispatch module doc forbids, and it is
        // how one era's trigger gets implemented in `Server` and missed in
        // `ServerCore` (T-114-58). The predicate now lives in ONE expression,
        // `TaskDispatch::create_gate`, reached from both dispatchers; only the
        // RESPONSE building differs (this dispatcher returns a `ToolCallOutcome`
        // and builds its envelope one frame up, at the `CallTool` arm).
        //
        // The trigger is era-aware (plan 114-12): v1 reads `CallToolRequest.task`,
        // v2 reads the client's per-request tasks-extension declaration off the
        // already-resolved `ProtocolContext`.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let tool_task_support = tool_info
                .as_ref()
                .and_then(|info| info.execution.as_ref())
                .and_then(|exec| exec.task_support.as_ref())
                .copied();

            // Warn when a Required tool is called without task-augmented request
            if req.task.is_none() && matches!(tool_task_support, Some(TaskSupport::Required)) {
                tracing::warn!(
                    tool = req.name.as_str(),
                    "Tool declares taskSupport=Required but client did not send task field; returning CallToolResult for compatibility"
                );
            }

            match self
                .task_dispatch()
                .create_gate(create_trigger, tool_task_support, &value)
            {
                crate::server::task_dispatch::CreateGate::Create => {
                    // The shared create-path re-extracts the task id + terminal
                    // CallToolResult from the value, so only the raw value
                    // crosses here.
                    return Ok(ToolCallOutcome::TaskCreated { task_value: value });
                },
                crate::server::task_dispatch::CreateGate::NotTaskShaped => {
                    // Tool declares task support but didn't return a Task — fall through to normal path
                    // (handles the "optional" case where the tool might not create a task).
                    tracing::debug!(
                        tool = req.name.as_str(),
                        "Tool declares taskSupport but returned non-Task value; using normal CallToolResult path"
                    );
                },
                crate::server::task_dispatch::CreateGate::Closed => {},
            }
        }

        // TOUT-02 double-wrap tripwire: BEFORE this tail text-wraps `value` into
        // content, WARN (+ debug_assert in debug/CI) if it structurally resembles
        // an already-built `CallToolResult` — the silent double-wrap bug. Honors
        // the per-tool `suppress_double_wrap_check` opt-out (D-08) via the SAME
        // suppression set the high-level `Server` uses, so the two dispatchers
        // never drift. Non-wasm only (mirrors the create-path gate above).
        #[cfg(not(target_arch = "wasm32"))]
        crate::server::task_dispatch::double_wrap_tripwire(
            &req.name,
            &value,
            self.suppress_double_wrap.contains(req.name.as_str()),
        );

        // A declared outputSchema means structuredContent is emitted below
        // (via widget enrichment or the schema bridge) — validate the value
        // against it regardless of which branch does the emitting.
        if let Some(schema) = tool_info.and_then(|i| i.output_schema.as_ref()) {
            crate::server::output_validation::warn_on_schema_mismatch(
                &req.name,
                schema,
                &value,
                validation_era,
            );
        }

        let call_result = if let Some(info) = tool_info.filter(|i| i.widget_meta().is_some()) {
            // Widget tool: structured data goes in structuredContent,
            // text is a brief summary to avoid duplication in `ChatGPT`
            let summary = summarize_structured_output(&value);
            CallToolResult::new(vec![Content::text(summary)]).with_widget_enrichment(info, value)
        } else if tool_info.is_some_and(|i| i.output_schema.is_some()) {
            // Declared outputSchema: bridge it to the wire (MCP spec — a tool
            // that declares an outputSchema SHOULD return structuredContent
            // conforming to it). Dual-emit (compact text voice, matching the
            // high-level `Server` dispatcher) keeps text-only clients working.
            CallToolResult::structured(value)
        } else {
            let text = serde_json::to_string_pretty(&value)?;
            CallToolResult::new(vec![Content::text(text)])
        };

        // D-03.3: drain any handler-set result `_meta` onto the Payload envelope
        // with handler-key-wins precedence. The create-path and error arms
        // returned earlier and are still `_meta`-free; the Verbatim arm also
        // returned earlier but now performs this SAME drain against its own
        // envelope (D-06), so the slot is already empty by the time control could
        // reach here on that path. Shadow-rebind so the wasm branch, which never
        // sets the slot, needs no `mut`.
        #[cfg(not(target_arch = "wasm32"))]
        let call_result = {
            let mut call_result = call_result;
            if let Some(handler_meta) = result_meta_handle.take_result_meta() {
                crate::server::cancellation::merge_result_meta(&mut call_result, handler_meta);
            }
            call_result
        };

        Ok(ToolCallOutcome::Result(call_result))
    }

    /// Handle list prompts request.
    async fn handle_list_prompts(&self, _req: &ListPromptsRequest) -> Result<ListPromptsResult> {
        let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();

        tracing::debug!(
            target: "mcp.prompts",
            count = prompts.len(),
            "Returning prompts"
        );

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }

    /// Handle get prompt request.
    async fn handle_get_prompt(
        &self,
        req: &GetPromptRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<GetPromptResult> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Prompt '{}' not found", req.name)))?;

        // Create request handler extra data with auth_context, the request `_meta`
        // (so handlers read trace-context/namespaced keys via extra), and the
        // once-at-ingress resolved protocol context (so handlers read
        // era/client_info via extra.era()/client_info() — Phase 112, mirrors
        // handle_call_tool).
        let request_id = format!("prompt_{}", req.name);
        let extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            .with_protocol_context(protocol_context),
        );

        handler.handle(req.arguments.clone(), extra).await
    }

    /// Handle list resources request.
    async fn handle_list_resources(
        &self,
        req: &ListResourcesRequest,
        auth_context: Option<AuthContext>,
        // THREADED, not resolved here (Phase 118.1-08, G-9): this site had no
        // `ProtocolContext` at all, so a v1 client's handshake capabilities could
        // never reach a `resources/list` handler. `ListResourcesRequest` carries
        // no `_meta`, so the context can only arrive from the caller.
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<ListResourcesResult> {
        let mut result = match &self.resources {
            Some(handler) => {
                let request_id = "list_resources".to_string();
                let extra = self.attach_peer(
                    RequestHandlerExtra::new(
                        request_id.clone(),
                        self.cancellation_manager
                            .create_token(request_id.clone())
                            .await,
                    )
                    .with_auth_context(auth_context)
                    .with_protocol_context(protocol_context),
                );
                handler.list(req.cursor.clone(), extra).await?
            },
            None => ListResourcesResult {
                resources: vec![],
                next_cursor: None,
                ttl_ms: None,
                cache_scope: None,
            },
        };

        // Enrich ResourceInfo items with tool _meta for widget resources.
        // Only resources with URIs in the uri_to_tool_meta index (built from
        // tool _meta at construction) receive _meta -- non-widget resources
        // are unaffected.
        if !self.uri_to_tool_meta.is_empty() {
            for resource in &mut result.resources {
                if let Some(tool_meta) = self.uri_to_tool_meta.get(&resource.uri) {
                    let meta = resource.meta.get_or_insert_with(serde_json::Map::new);
                    crate::types::ui::deep_merge(meta, tool_meta.clone());
                }
            }
        }

        Ok(result)
    }

    /// Handle read resource request.
    async fn handle_read_resource(
        &self,
        req: &ReadResourceRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<ReadResourceResult> {
        let handler = self.resources.as_ref().ok_or_else(|| {
            Error::internal(format!("Resource handler not available for '{}'", req.uri))
        })?;

        // Thread the request `_meta` + once-at-ingress resolved protocol context
        // into `extra` so resource handlers read era/client_info/trace_context on
        // a v2 connection (Phase 112, mirrors handle_call_tool / handle_get_prompt).
        let request_id = format!("read_{}", req.uri);
        let extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            .with_protocol_context(protocol_context),
        );

        let mut result = handler.read(&req.uri, extra).await?;

        // Merge tool descriptor keys into content _meta for widget resources.
        // Display keys (from ChatGptAdapter/WidgetMeta) are already in content
        // meta. Descriptor keys (openai/outputTemplate, openai/widgetAccessible,
        // etc.) come from the linked tool's _meta via the uri_to_tool_meta index.
        if !self.uri_to_tool_meta.is_empty() {
            for content in &mut result.contents {
                if let Content::Resource { uri, meta, .. } = content {
                    if let Some(tool_meta) = self.uri_to_tool_meta.get(uri.as_str()) {
                        let content_meta = meta.get_or_insert_with(serde_json::Map::new);
                        crate::types::ui::deep_merge(content_meta, tool_meta.clone());
                    }
                }
            }
        }

        Ok(result)
    }

    /// Handle list resource templates request.
    async fn handle_list_resource_templates(
        &self,
        _req: &ListResourceTemplatesRequest,
    ) -> Result<ListResourceTemplatesResult> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        })
    }

    /// Create an error response.
    ///
    /// Delegates to the SINGLE-SOURCE envelope builder in `task_dispatch` so the
    /// shared task unit and `ServerCore` cannot drift (Concern #3 — envelope drift).
    fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
        contract_pre_error_code_mapping!();
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::server::task_dispatch::error_response(id, code, message)
        }
        #[cfg(target_arch = "wasm32")]
        {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: ResponsePayload::Error(JSONRPCError {
                    code,
                    message,
                    data: None,
                }),
            }
        }
    }

    /// Create a success response.
    ///
    /// Delegates to the SINGLE-SOURCE envelope builder in `task_dispatch` so the
    /// shared task unit and `ServerCore` cannot drift (Concern #3 — envelope drift).
    fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::server::task_dispatch::success_response(id, result)
        }
        #[cfg(target_arch = "wasm32")]
        {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: ResponsePayload::Result(result),
            }
        }
    }
}

// Implement MiddlewareExecutor for ServerCore to enable workflow tool execution
// with consistent middleware application
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl crate::server::middleware_executor::MiddlewareExecutor for ServerCore {
    async fn execute_tool_with_middleware(
        &self,
        tool_name: &str,
        mut args: Value,
        mut extra: RequestHandlerExtra,
    ) -> Result<Value> {
        // Get the tool handler
        let handler = self
            .tools
            .get(tool_name)
            .ok_or_else(|| Error::internal(format!("Tool '{}' not found", tool_name)))?;

        // Authorization check with tool_authorizer if available
        if let Some(authorizer) = &self.tool_authorizer {
            if let Some(ref auth_ctx) = extra.auth_context {
                if !authorizer.can_access_tool(auth_ctx, tool_name).await? {
                    return Err(Error::authentication(format!(
                        "User not authorized to call tool '{}'",
                        tool_name
                    )));
                }
            }
        }

        // Create tool context for middleware
        let context = ToolContext::new(tool_name, &extra.request_id);

        // Process request through tool middleware chain
        // Middleware rejection short-circuits tool execution (on_error already called by chain)
        self.tool_middleware
            .read()
            .await
            .process_request(tool_name, &mut args, &mut extra, &context)
            .await?;

        // Execute the tool with potentially modified args and extra
        let mut result = handler.handle(args, extra).await;

        // Process response through tool middleware chain
        if let Err(e) = self
            .tool_middleware
            .read()
            .await
            .process_response(tool_name, &mut result, &context)
            .await
        {
            // Log error but continue with original result
            tracing::warn!("Tool response middleware processing failed: {}", e);
        }

        // If tool execution failed, call handle_tool_error
        if let Err(ref e) = result {
            self.tool_middleware
                .read()
                .await
                .handle_tool_error(tool_name, e, &context)
                .await;
        }

        result
    }
}

/// The wire result of a v2 `server/discover` request (Phase 112, VERS-04).
///
/// Phase 113 (CLNT-01) MOVED this type to
/// [`crate::types::protocol::ServerDiscoverResult`] and made it public: it is now
/// the return type of [`Client::server_discover`](crate::Client::server_discover),
/// and the client compiles on `wasm32` where this whole module is `cfg`-ed out.
/// The re-export keeps every existing in-crate reference (and this module's
/// tests) working against the one shared definition.
pub(crate) use crate::types::protocol::ServerDiscoverResult;

/// The `experimental` sub-key some MCP 2025-11-25 servers advertise the task
/// lifecycle under, before it became a first-class `capabilities.tasks` field.
///
/// It is suppressed on v2 for the same reason `capabilities.tasks` is: on
/// MCP 2026-07-28 tasks live in the Extensions Track, and an `experimental.tasks`
/// flag on a v2 wire tells a client to use a negotiation home that does not exist
/// there. Only THIS key is removed — this phase does not own any other
/// `experimental` entry, and suppressing the whole block was explicitly rejected
/// (D-02).
const EXPERIMENTAL_TASKS_KEY: &str = "tasks";

/// Project a server's already-computed capabilities onto the MCP 2026-07-28
/// wire (plan 114-05, D-02).
///
/// Returns a CLONE. The caller's `capabilities` are never mutated — see
/// [`discover_result_from_capabilities`] for why that matters.
///
/// Two edits, both suppressions:
///
/// - `capabilities.tasks` is cleared. On v2 the task lifecycle is an EXTENSION,
///   negotiated under
///   [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY) in
///   the `extensions` map, and `capabilities.tasks` is a v1 spelling a v2 client
///   has no rule for reading.
/// - the `tasks` key is removed from `capabilities.experimental`, if present.
///
/// `extensions` is emitted UNCHANGED, tasks entry included: the entry is written
/// once, at build time, by the single shared
/// `task_dispatch::apply_tasks_capability_rule`, so this projection reads it
/// rather than re-deriving it. If the resulting `experimental` map is left empty
/// it is emitted as `{}` rather than dropped — "omit an emptied map" would be a
/// second, unrelated wire-shape rule this phase does not own, and an empty object
/// advertises nothing.
fn project_capabilities_for_v2(capabilities: &ServerCapabilities) -> ServerCapabilities {
    let mut projected = capabilities.clone();
    projected.tasks = None;
    if let Some(experimental) = projected.experimental.as_mut() {
        experimental.remove(EXPERIMENTAL_TASKS_KEY);
    }
    projected
}

/// Project a server's already-computed capabilities onto the MCP 2025-11-25
/// `initialize` wire (plan 114-05, D-02 / T-114-16).
///
/// Returns a CLONE, for the same per-server-versus-per-request-era reason
/// [`discover_result_from_capabilities`] spells out.
///
/// # Why this exists — a MEASURED leak, not a precaution
///
/// `task_dispatch::apply_tasks_capability_rule` runs at BUILD time, where no era
/// exists, and writes the tasks-extension entry into the ONE
/// [`ServerCapabilities`] both eras are served from. `initialize` serializes that
/// struct verbatim, so without this projection a v1 `initialize` against a
/// tasks-backed server gains
/// `"extensions":{"io.modelcontextprotocol/tasks":{}}` — measured on the real
/// wire by `tests/v2_tasks_negotiation.rs::v1_initialize_stays_byte_identical`
/// before this fn existed. That is a v1 wire change on every tasks server that
/// exists today, which is exactly the lock D-02 holds.
///
/// The struct carries what both eras could want; each serialization boundary
/// decides what its era SEES. `server/discover` suppresses the v1 spellings;
/// `initialize` suppresses the v2 one. The two projections are mirrors, and a
/// reader can check both at once.
///
/// # Only the AUTO-ADVERTISED value is removed
///
/// The entry is dropped only when its value is EXACTLY what
/// [`tasks_extension_value()`](crate::server::task_dispatch::tasks_extension_value)
/// writes — the empty object. An operator who configured a non-empty value under
/// that key authored something distinguishable, and silently deleting an
/// operator's own configuration from the wire is worse than carrying it: the
/// additive-only discipline this phase is built on cuts both ways.
///
/// If removing the entry empties the map, the map itself is dropped rather than
/// emitted as `"extensions":{}`. A map that contained nothing but the
/// auto-advertised entry did not exist before the rule created it, so leaving an
/// empty object behind would itself be the byte change this projection prevents.
///
/// One residual case is stated rather than hidden: an operator who explicitly
/// configured exactly `{}` under this key BEFORE plan 114-05 loses it from the
/// v1 `initialize` wire, because that value is by construction indistinguishable
/// from the auto-advertised one. It is still served on v2, where the key means
/// something.
///
/// `WasmServer`'s `initialize` deliberately does NOT call this: the whole task
/// subsystem (including the capability rule) is `#[cfg(not(target_arch =
/// "wasm32"))]`, so no wasm build can auto-gain the entry, and applying the
/// projection there could only ever remove an operator's own key.
pub(crate) fn project_capabilities_for_v1(capabilities: &ServerCapabilities) -> ServerCapabilities {
    let auto_advertised = crate::server::task_dispatch::tasks_extension_value();
    let mut projected = capabilities.clone();

    // ONE pass, ONE clone, mirroring [`project_capabilities_for_v2`] above.
    // Removing the entry can leave an empty map behind, and that map is then
    // dropped — but ONLY on the branch that actually removed the entry, so a
    // map the operator authored as empty is carried through untouched, exactly
    // as it was before this projection existed.
    let emptied_by_removal = projected.extensions.as_mut().is_some_and(|extensions| {
        if extensions.get(crate::types::capabilities::TASKS_EXTENSION_KEY) != Some(&auto_advertised)
        {
            return false;
        }
        extensions.remove(crate::types::capabilities::TASKS_EXTENSION_KEY);
        extensions.is_empty()
    });
    if emptied_by_removal {
        projected.extensions = None;
    }
    projected
}

/// The server-side inputs the `server/discover` projection reads (Phase 118.1,
/// G-7).
///
/// Bundles the two values that must come from the SAME server: the
/// already-computed capabilities, and the protocol accept-list the version gate
/// rejects against. They travel together because the conformance suite
/// correlates them — every element of an unsupported-version rejection's
/// `error.data.supported` must also appear in the discover result's
/// `supportedVersions` — so an API that let a caller supply one without the
/// other would be an API that let the two drift.
///
/// Both fields are borrowed, so this is a zero-copy view a caller assembles at
/// the call site; it owns nothing and stores nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DiscoverSource<'a> {
    /// The server's already-computed capabilities (incl. the `extensions` map).
    pub(crate) capabilities: &'a ServerCapabilities,
    /// The server's configured protocol accept-list — the SINGLE source for
    /// both `supportedVersions` and an `UNSUPPORTED_PROTOCOL_VERSION`
    /// rejection's `error.data.supported`.
    pub(crate) supported_versions: &'a [ProtocolVersion],
}

impl<'a> DiscoverSource<'a> {
    /// Bundle a server's capabilities with its protocol accept-list.
    pub(crate) fn new(
        capabilities: &'a ServerCapabilities,
        supported_versions: &'a [ProtocolVersion],
    ) -> Self {
        Self {
            capabilities,
            supported_versions,
        }
    }
}

/// Capabilities-only source for unit tests that inspect nothing but the
/// PROJECTED CAPABILITIES, advertising an empty accept-list.
///
/// # Why this is `#[cfg(test)]`, and why that is the point
///
/// Gating the defaulting conversion to test builds is what makes the
/// single-source rule STRUCTURAL rather than a convention: in a non-test build
/// this impl does not exist, so the only way to reach
/// [`build_discover_response`] is to name an accept-list explicitly. A
/// production caller therefore CANNOT accidentally publish a defaulted or empty
/// `supportedVersions` — it would not compile.
///
/// The empty slice is deliberate over a plausible default. A test that ends up
/// reading `supportedVersions` through this conversion fails loudly on the empty
/// array instead of passing against a real-looking list it never asked for.
#[cfg(test)]
impl<'a> From<&'a ServerCapabilities> for DiscoverSource<'a> {
    fn from(capabilities: &'a ServerCapabilities) -> Self {
        Self {
            capabilities,
            supported_versions: &[],
        }
    }
}

/// Isolated conversion fn producing the [`ServerDiscoverResult`] wire shape
/// (Phase 112, VERS-04; era projection added by plan 114-05, D-02).
///
/// This is the SINGLE place the discover wire shape is assembled: it projects
/// the already-computed `capabilities` (including `extensions`) and `info`
/// read-only — never recomputing capabilities and never triggering any
/// initialize-style side effect. Keeping the shape behind one fn means a
/// final-spec change is localized (Codex MEDIUM — "server/discover wire shape is
/// provisional").
///
/// # The projection is PER-REQUEST-ERA and MUST NOT mutate stored capabilities
///
/// [`project_capabilities_for_v2`] works on a CLONE, deliberately. A server's
/// `capabilities` are per-SERVER while the projection is per-REQUEST-ERA: one
/// pmcp binary serves both eras, so mutating the stored struct here would make
/// the first v2 `server/discover` permanently change what every subsequent v1
/// `initialize` client sees. That is a cross-request state leak, not an
/// optimisation — do not "avoid the clone" by taking `&mut`.
///
/// # Why the projection is applied unconditionally rather than era-gated here
///
/// `server/discover` is a v2-ONLY method: [`build_discover_response`] answers
/// `-32601` for any request that is not `Era::V2` BEFORE reaching this fn, so
/// every wire shape assembled here is by construction a v2 one. The v1
/// `initialize` response does not flow through this fn at all — it is built from
/// `self.capabilities.clone()` directly, in `ServerCore::handle_initialize`
/// (`core.rs`), `Server::handle_request` (`server/mod.rs`) and
/// `WasmServer` (`server/wasm_server.rs`). That is why v1 bytes are frozen by
/// leaving the v1 path untouched rather than by adding a branch here.
///
/// The anti-pattern this deliberately avoids: doing the suppression as a serde
/// change in `src/types/capabilities.rs`. That would alter the `initialize`
/// bytes of every existing tasks server on every era, which is exactly the lock
/// D-02 exists to hold.
pub(crate) fn discover_result_from_capabilities(
    source: DiscoverSource<'_>,
    info: &Implementation,
    negotiated_version: String,
) -> ServerDiscoverResult {
    ServerDiscoverResult {
        protocol_version: negotiated_version,
        capabilities: project_capabilities_for_v2(source.capabilities),
        server_info: info.clone(),
        // The accept-list is COPIED from the caller's source, never rebuilt
        // here: `error.data.supported` reads the same slice, and the suite
        // asserts the two agree (G-7).
        supported_versions: source
            .supported_versions
            .iter()
            .map(|version| version.as_str().to_string())
            .collect(),
        ttl_ms: None,
        cache_scope: None,
    }
}

/// Internal disposition discriminator for the v2 `resultType` envelope
/// (Phase 112, VERS-07 / D-08).
///
/// This is NOT a public field on any Result struct — handlers keep returning
/// today's types (semver-safe, zero public-API churn). This phase only ever
/// emits [`ResponseDisposition::Complete`]; the [`InputRequired`](Self::InputRequired)
/// and [`Task`](Self::Task) variants are the concrete path Phases 113 and 114
/// select at dispatch: they thread a non-default disposition with the response
/// and the SAME serialization helper ([`inject_v2_result_envelope`]) emits it,
/// without touching this envelope code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseDisposition {
    /// The result is a final, complete result (the default; absent-means-complete).
    Complete,
    /// The result requests further input before it can complete (Phase 113).
    ///
    /// Selected by `mrtr_egress` when a handler signalled that it needs more
    /// input; the shared helper below then emits it as the wire `resultType`.
    /// The conditional `allow` is feature-scoped: MRTR (and therefore the only
    /// constructor of this variant) is `streamable-http`-only by D-14, and with
    /// that feature on — what every lint and build gate uses — it is live code.
    #[cfg_attr(not(feature = "streamable-http"), allow(dead_code))]
    InputRequired,
    // Why there is NO `allow(dead_code)` here any more: Phase 114 plan 11 wired
    // this variant into production. `DispatchEnvelopeClaim::TASK_CREATED`
    // constructs it, `TaskDispatch::build_task_created_response` returns that
    // claim on v2, and both dispatch sites fold it into
    // `inject_v2_result_envelope`. `server::task_dispatch` is gated only on
    // `not(target_arch = "wasm32")` and on no feature at all, so the constructor
    // is present on EVERY native build and the allow would now be hiding
    // nothing. Measured with `-D warnings` under `--features full`,
    // `--no-default-features` and `--no-default-features --features
    // streamable-http` before it was removed.
    /// The result is a task handle rather than a terminal result (Phase 114).
    ///
    /// The ONLY response that earns it is a `tools/call` that returned a task
    /// handle instead of a result. A `tasks/get` is an ordinary complete result
    /// ABOUT a task and carries `"complete"` even when its body inlines a
    /// terminal `result` — see [`DispatchEnvelopeClaim::TASK_CREATED`].
    Task,
}

impl ResponseDisposition {
    /// The wire `resultType` discriminator string.
    ///
    /// All three values come from `types::mrtr`'s reserved-spelling block, not
    /// from literals here: the Phase-114 CLIENT decoder branches on the same
    /// `"task"` / `"complete"` strings and compiles on `wasm32`, where this
    /// module does not exist at all. One declaration, two readers.
    pub(crate) fn as_wire_str(self) -> &'static str {
        match self {
            Self::Complete => crate::types::mrtr::COMPLETE_RESULT_TYPE,
            Self::InputRequired => crate::types::mrtr::INPUT_REQUIRED_RESULT_TYPE,
            Self::Task => crate::types::mrtr::TASK_RESULT_TYPE,
        }
    }
}

/// WHICH egress minted the reserved result fields on this response
/// (Phase 114, plan 10 — DQ2).
///
/// This is an EXPLICIT input to [`own_reserved_result_fields`], threaded from
/// the egress that did the minting. It replaces a flag the registry used to
/// DERIVE from the [`ResponseDisposition`]:
///
/// ```text
/// let mrtr_owned = disposition == ResponseDisposition::InputRequired;
/// ```
///
/// # Why the derivation had to go
///
/// It was correct while `mrtr_egress` was the ONLY minter of `requestState` and
/// `inputRequests`. Phase 114 adds a second legitimate minter whose disposition
/// is `complete`: a v2 `tasks/get` on an `input_required` TASK is a complete
/// JSON-RPC result — the task is waiting, not the request — and the ext-tasks
/// schema makes `inputRequests` a REQUIRED top-level key of it
/// (`$defs.InputRequiredTask.required`). Under the derived flag that required
/// field was SILENTLY deleted, with a `tracing::warn!` rather than an error, so
/// an integration test asserting only "the request succeeded" passed against a
/// response a conformant client rejects.
///
/// # Why a named enum and not a `bool`
///
/// A bare `owns_reserved_fields: bool` at a call site reads as "true means
/// allowed" and is exactly the parameter a future refactor flips by accident. A
/// named variant forces every call site to state WHICH egress it is, and it lets
/// the grant be per-KEY per-OWNER rather than a single all-or-nothing flag —
/// which is what keeps `requestState` MRTR-only (see [`Self::may_emit`]).
///
/// Two alternatives were considered and rejected during planning: re-adding the
/// field after stripping it, and special-casing by method string. Both re-create
/// the per-site divergence the single-registry design exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservedFieldOwner {
    /// No egress minted the reserved fields — strip every one of them.
    ///
    /// The default posture of every ordinary result. A handler that wrote one of
    /// these keys onto its own result is forging a protocol field, and the
    /// registry removes it and says so.
    None,
    /// The MRTR egress minted them (`requestState` AND `inputRequests`).
    ///
    /// Selected by `seal_input_required`, at the site that writes both keys.
    ///
    /// The conditional `allow` is feature-scoped and the scope was MEASURED, not
    /// guessed: `seal_input_required` is `streamable-http`-only by D-14 and the
    /// `pmcp::testing` seam is `testing`-only, so this variant is dead only when
    /// NEITHER feature is on. With `--no-default-features` and `-D warnings`,
    /// dropping the allow reports `variants Mrtr and TasksDispatch are never
    /// constructed`; with `--features streamable-http` alone it reports only
    /// `TasksDispatch`.
    #[cfg_attr(
        not(any(feature = "streamable-http", feature = "testing")),
        allow(dead_code)
    )]
    Mrtr,
    /// The v2 tasks dispatch minted `inputRequests`, and ONLY `inputRequests`.
    ///
    /// The tasks surface has no continuation token: the persisted task record
    /// replaces the sealed continuation (D-17), so no key material is introduced
    /// and a tasks result carrying `requestState` is still a strip.
    ///
    /// Constructed by the v2 `tasks/get` dispatch — plan 114-11 wired it
    /// (`DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED`), closing D-114-H.
    ///
    // The `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` this variant
    // carried while the `pmcp::testing` seam was its only constructor is GONE:
    // `server::task_dispatch` is gated only on `not(target_arch = "wasm32")` and
    // on no feature, so the production constructor is present on every native
    // build. Re-measured with `-D warnings` under `--features full`,
    // `--no-default-features` and `--no-default-features --features
    // streamable-http` before removal.
    TasksDispatch,
}

impl ReservedFieldOwner {
    /// Whether this owner may publish the reserved top-level result key `field`.
    ///
    /// The grant is per-KEY per-OWNER, never per-key-globally. "Always allow
    /// `inputRequests`" would fix the tasks case and simultaneously hand every
    /// tool handler the ability to forge an input-request set (T-114-45), and
    /// granting the tasks owner `requestState` would let a surface with no
    /// continuation publish something shaped like one (T-114-44).
    fn may_emit(self, field: &str) -> bool {
        match self {
            Self::None => false,
            Self::Mrtr => {
                field == crate::types::mrtr::REQUEST_STATE_KEY
                    || field == crate::types::mrtr::INPUT_REQUESTS_KEY
            },
            Self::TasksDispatch => field == crate::types::mrtr::INPUT_REQUESTS_KEY,
        }
    }
}

/// The v2 result-envelope claim a DISPATCH makes about the response it produced
/// (Phase 114, plan 11).
///
/// A second, independent claimant alongside the MRTR egress. `mrtr_egress`
/// already returns `(ResponseDisposition, ReservedFieldOwner)` from the site that
/// mints its reserved fields; the tasks dispatch needs to say the same two things
/// from ITS minting site, and it sits several frames below the one place that
/// calls [`inject_v2_result_envelope`]. This struct is what travels those frames.
///
/// # Why a threaded claim rather than a re-derivation at the envelope
///
/// The envelope could, in principle, look at the response and guess. It must
/// not: DQ2 rejected deriving ownership from the [`ResponseDisposition`] (the
/// measured row-23 defect) and from the method string (which re-creates the
/// per-site divergence the single registry exists to prevent). A claim made
/// WHERE THE WRITE HAPPENS is the only form that cannot be wrong about what was
/// written.
///
/// # The two claims are disjoint by construction
///
/// `MRTR_METHODS` carries no `tasks/*` row, and a `tools/call` that becomes a
/// task returns a `CreateTaskResult` rather than an MRTR `input_required`
/// result, so no response is minted by both claimants.
/// [`Self::or_egress`] states the precedence anyway rather than assuming it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DispatchEnvelopeClaim {
    /// The wire `resultType` this dispatch produced.
    pub(crate) disposition: ResponseDisposition,
    /// Which egress minted the reserved top-level result fields.
    pub(crate) owner: ReservedFieldOwner,
}

impl DispatchEnvelopeClaim {
    /// The default: an ordinary complete result that minted no reserved field.
    pub(crate) const NONE: Self = Self {
        disposition: ResponseDisposition::Complete,
        owner: ReservedFieldOwner::None,
    };

    /// A v2 `tasks/get` on an `input_required` task.
    ///
    /// The disposition is `complete` — the JSON-RPC REQUEST completed; it is the
    /// TASK that is waiting — while the result legitimately carries a top-level
    /// `inputRequests`, which is the whole reason ownership is a separate input
    /// from the disposition (see [`ReservedFieldOwner`]).
    pub(crate) const TASKS_INPUT_REQUIRED: Self = Self {
        disposition: ResponseDisposition::Complete,
        owner: ReservedFieldOwner::TasksDispatch,
    };

    /// A `tools/call` that returned a task handle instead of a result.
    ///
    /// `resultType: "task"` is a TOOL-CALL disposition and belongs to this
    /// response only. A `tasks/get` — even one whose body is full of task fields,
    /// even one inlining a terminal `result` — is an ordinary complete result
    /// ABOUT a task and carries `"complete"`.
    pub(crate) const TASK_CREATED: Self = Self {
        disposition: ResponseDisposition::Task,
        owner: ReservedFieldOwner::None,
    };

    /// Fold the MRTR egress's own claim over this dispatch claim.
    ///
    /// The egress wins when it made a claim at all, because it PHYSICALLY
    /// rewrote the result body and minted key material into it; otherwise the
    /// dispatch's claim stands. On every non-MRTR response the egress returns
    /// exactly [`Self::NONE`], so the common path is a pass-through.
    pub(crate) fn or_egress(
        self,
        disposition: ResponseDisposition,
        owner: ReservedFieldOwner,
    ) -> Self {
        let egress = Self { disposition, owner };
        if egress == Self::NONE {
            self
        } else {
            egress
        }
    }
}

impl Default for DispatchEnvelopeClaim {
    fn default() -> Self {
        Self::NONE
    }
}

/// The reserved `result._meta` key the v2 envelope publishes the server's
/// [`Implementation`] under.
///
/// `schema/draft/schema.ts` places server identity inside the result's
/// `ResultMetaObject`, NOT as a top-level result key, and the conformance suite
/// reads it from there. Phase 112 verified PRESENCE and had no conformance
/// harness, so it attached the value one level too high; Phase 113 owns the v2
/// response path and is the first phase graded by conformance, which makes this
/// the cheap moment to correct it rather than a wire-visible break later.
///
/// This is the RESPONSE-side sibling of the REQUEST-side reserved keys
/// (`RESERVED_PROTOCOL_VERSION_KEY`, `RESERVED_CLIENT_INFO_KEY`,
/// `RESERVED_CLIENT_CAPABILITIES_KEY`) in `crate::types::protocol::context`,
/// which live on `params._meta`. Same `io.modelcontextprotocol/*` namespace,
/// opposite direction — hence the separate home next to the envelope that is its
/// only writer.
///
/// It is a SERVER-OWNED reserved field: see [`own_reserved_result_fields`] for
/// the full registry.
pub(crate) const RESERVED_SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// Inject the v2-only response envelope (`resultType` + `serverInfo`) and
/// project the `2026-07-28` caching hints, at the single era-gated
/// serialization boundary (Phase 112 VERS-07 / D-07 / D-08; Phase 115 SCHM-03).
///
/// This is the ONE shared implementation BOTH native dispatch sites
/// (`core.rs` and `server/mod.rs`) call — not a per-site copy. The envelope
/// model is pinned (Codex HIGH #5):
///
/// - era != V2 (or no resolved context) → the response is byte-identical to the
///   pre-v2 wire **except that the two v2-only caching-hint keys are STRIPPED
///   if a handler set them** (see the next bullet). No key is ever ADDED on a
///   legacy wire, and the golden fixtures in `tests/v1_lists_golden.rs` pin
///   that.
/// - the `ttlMs` / `cacheScope` caching hints (`2026-07-28` `CacheableResult`,
///   D-07 / D-08) are projected by [`project_caching_hints`] on **BOTH** eras:
///   ENSURED on v2 (a handler-set value survives verbatim; an unset one gets
///   the safe defaults `0` / `"private"`, so the SDK emits a conformant but
///   INERT cache posture on every v2 list/read response whether or not the
///   author thought about caching), and actively REMOVED on every other era.
///   The strip is not an ensure-only omission: D-11 makes "a v1 wire never
///   carries a v2 field" the severability precedent for Phases 116-119, so a
///   handler that sets a hint and then serves a legacy client must still emit a
///   byte-identical legacy response. `cacheable` says whether this result is
///   one of the six that extend `CacheableResult`; see
///   [`request_is_cacheable`] for the shared classifier both dispatchers use.
/// - error responses / notifications (no `result`) → NO injection, NO
///   projection.
/// - `result` is a JSON object → the SERVER-OWNED reserved fields are asserted
///   over it by [`own_reserved_result_fields`]; every other key, including every
///   non-reserved `_meta` key, is left exactly as the handler wrote it.
/// - `result` is scalar/array/null → left unchanged (cannot key a non-object;
///   no in-scope v2 method returns a non-object).
///
/// `owner` states WHICH egress minted the reserved result fields, and
/// `cacheable` states whether the result carries caching hints. Neither has a
/// default and every call site names both, so a result that no egress minted —
/// and a result that is not a `CacheableResult` — cannot acquire either by
/// omission. See [`ReservedFieldOwner`] and
/// [`Cacheable`](crate::types::caching::Cacheable).
///
/// `cacheable` is a claim about the request's METHOD, so it is DOWNGRADED here
/// whenever `disposition` is not [`ResponseDisposition::Complete`]: an
/// `input_required` body is an `InputRequiredResult` and a task body is a task
/// handle, and in the `2026-07-28` schema neither extends `CacheableResult`. See
/// the inline comment at the downgrade for why `resources/read` makes that
/// reachable rather than theoretical.
///
/// # Not the final mutation
///
/// This function is NOT the last thing that touches the response.
/// [`ServerCore::handle_request`] calls
/// `process_response_with_context(&mut response, &context)` (`src/server/core.rs`,
/// immediately after this call) and `src/shared/middleware.rs`'s
/// `process_response_with_context` takes `response: &mut JSONRPCResponse` — so a
/// registered response middleware CAN add, alter or remove `ttlMs`,
/// `cacheScope`, `resultType` or `serverInfo` after the projection has run. The
/// twin site in `src/server/mod.rs` has the same ordering by way of its caller.
///
/// **Response middleware MUST NOT mutate `ttlMs`, `cacheScope`, `resultType` or
/// `serverInfo`.** These are server-owned wire fields with exactly one writer;
/// a middleware that needs to influence cacheability must set `ttl_ms` /
/// `cache_scope` on the result TYPE before dispatch returns
/// (`ListToolsResult::with_ttl_ms`, `with_cache_scope`, and the equivalents on
/// the other five `CacheableResult` extenders), not rewrite the serialized
/// value afterwards.
///
/// The call was deliberately NOT moved after the middleware chain: doing so
/// would change what middleware OBSERVES about Phase 114's `resultType` /
/// `serverInfo`, which is a v2 behaviour change outside SCHM-03's scope. The
/// current ordering is measured by
/// `response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation`
/// so a future reorder registers as a deliberate decision rather than a silent
/// conformance change, is fenced by a source tripwire in 115-08, and is booked
/// as a deferred item by 115-10.
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
    owner: ReservedFieldOwner,
    cacheable: Cacheable,
) {
    // Only success results carry the envelope; errors / notifications do not.
    // This guard runs BEFORE the era gate now, because the caching projection
    // is era-agnostic (ensure on v2, strip on everything else) while the
    // `resultType` / `serverInfo` envelope stays strictly v2-only.
    let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload else {
        return;
    };

    // A non-object result (scalar/array/null) cannot carry a key — leave it.
    if !value.is_object() {
        return;
    }

    // D-07 again, at the one place that knows BOTH facts. `cacheable` is derived
    // from the REQUEST METHOD, so it describes the method's COMPLETE result type
    // — but a non-`Complete` disposition means the body on the wire is NOT that
    // type. `input_required` is an `InputRequiredResult` (Phase 113 MRTR) and a
    // task response is a task handle (Phase 114); in the vendored `2026-07-28`
    // schema BOTH `extends Result`, not `CacheableResult`. `resources/read` is
    // the concrete case that makes this reachable: it is the only method that is
    // simultaneously MRTR-eligible ([`client_request_mrtr_eligible`]) and
    // `Cacheable::Yes` ([`request_is_cacheable`]), so without this downgrade a
    // v2 `resources/read` answered with an MRTR signal emits `ttlMs` /
    // `cacheScope` on an `InputRequiredResult`.
    //
    // Only the ENSURE half is affected in practice. Both suppressed dispositions
    // are v2-only constructions — MRTR is `Era::V2`-gated at `mrtr_egress`, and
    // `DispatchEnvelopeClaim::TASK_CREATED` is minted only on the v2 tasks create
    // path — so on a non-v2 era `disposition` is always `Complete` and the D-11
    // strip still sees the caller's claim verbatim.
    let cacheable = match disposition {
        ResponseDisposition::Complete => cacheable,
        ResponseDisposition::InputRequired | ResponseDisposition::Task => Cacheable::No,
    };

    // BOTH eras: ensure the hints on v2, strip them on v1 / no-context (D-11).
    // The wire keys themselves are written ONLY inside `types::caching` (D-12).
    project_caching_hints(value, protocol_context.map(|c| c.era), cacheable);

    // v2-only: a v1 (or non-opted-in) response gains no envelope key.
    if matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        own_reserved_result_fields(value, server_info, disposition, owner);
    }
}

/// Classify a request as producing a `CacheableResult` or not (115-06, SCHM-03).
///
/// ONE shared table, called from BOTH native dispatch sites — the Phase-109/112
/// twin-site parity rule: `server/mod.rs` CALLS this, it never defines its own
/// copy. Two tables would be two places for the classification to rot, and a
/// drift between them would show up only as a missing hint on one transport.
///
/// The five `Cacheable::Yes` rows are exactly the `ClientRequest` variants whose
/// results extend `CacheableResult` in the vendored `2026-07-28` schema:
/// `tools/list`, `resources/list`, `resources/templates/list`, `resources/read`
/// and `prompts/list`.
///
/// # `server/discover` is deliberately absent
///
/// `DiscoverResult` is the SIXTH `CacheableResult` extender, but it does not
/// ride the `ClientRequest` route at all — `server/discover` is carried by the
/// crate-private `InternalClientRequest` and answered by
/// [`build_discover_response`], which names `Cacheable::Yes` at its own call
/// site. Adding a row here for a variant that cannot occur would be a lie about
/// where the claim is made.
///
/// # No wildcard arm
///
/// Every variant is enumerated, exactly as [`client_request_mrtr_eligible`] two
/// screens down already does and for the same reason: a `_ =>` catch-all makes
/// "fail-closed" silent, and the two failure directions are NOT symmetric here.
/// A SPURIOUS hint on an unexpected method would be a
/// cross-authorization-context data-leak vector (T-115-17), while a MISSING hint
/// on a v2 method whose result DOES extend `CacheableResult` ships a knowingly
/// non-conformant response — `ttlMs` and `cacheScope` are REQUIRED on the v2
/// projection. Neither is a defect a reviewer should have to notice by absence,
/// so adding a `ClientRequest` variant breaks this build until someone classifies
/// it.
pub(crate) fn request_is_cacheable(request: &Request) -> Cacheable {
    let Request::Client(boxed) = request else {
        // A `ServerRequest` is refused by both dispatchers with -32601; it has
        // no result at all, let alone a cacheable one.
        return Cacheable::No;
    };
    match boxed.as_ref() {
        // `tools/list` → ListToolsResult extends CacheableResult.
        ClientRequest::ListTools(_) => Cacheable::Yes,
        // `resources/list` → ListResourcesResult extends CacheableResult.
        ClientRequest::ListResources(_) => Cacheable::Yes,
        // `resources/templates/list` → ListResourceTemplatesResult extends it.
        ClientRequest::ListResourceTemplates(_) => Cacheable::Yes,
        // `resources/read` → ReadResourceResult extends CacheableResult.
        ClientRequest::ReadResource(_) => Cacheable::Yes,
        // `prompts/list` → ListPromptsResult extends CacheableResult.
        ClientRequest::ListPrompts(_) => Cacheable::Yes,
        // NOT cacheable — enumerated explicitly, no wildcard arm, so a future
        // variant forces a decision here rather than inheriting one by silence.
        ClientRequest::Initialize(_)
        | ClientRequest::CallTool(_)
        | ClientRequest::GetPrompt(_)
        | ClientRequest::Subscribe(_)
        | ClientRequest::Unsubscribe(_)
        | ClientRequest::Complete(_)
        | ClientRequest::CreateMessage(_)
        | ClientRequest::TasksGet(_)
        | ClientRequest::TasksResult(_)
        | ClientRequest::TasksList(_)
        | ClientRequest::TasksCancel(_)
        | ClientRequest::SetLoggingLevel { .. }
        | ClientRequest::Ping => Cacheable::No,
    }
}

/// The `_meta` object of a result, creating it when absent.
///
/// ONE helper for THREE result shapes. `CallToolResult._meta`,
/// `GetPromptResult._meta` and `ReadResourceResult._meta` have three different
/// Rust types (`Option<Value>`, `Option<Map>`, `Option<Value>`), which makes
/// "merge without clobbering" ambiguous at the type level; doing the merge on
/// the SERIALIZED JSON instead means there are no per-type special cases and the
/// envelope stays method-agnostic.
///
/// Returns `None` only when `result` itself is not a JSON object. A handler that
/// set `_meta` to a NON-object (a string, an array, a number) gets it REPLACED
/// with an object and a warning logged — the alternative is either dropping the
/// server-owned reserved keys or emitting a `_meta` the spec says must be an
/// object.
pub(crate) fn result_meta_object_mut(
    result: &mut Value,
) -> Option<&mut serde_json::Map<String, Value>> {
    use crate::types::mrtr::META_KEY;
    let object = result.as_object_mut()?;
    let existing_is_object = matches!(object.get(META_KEY), Some(Value::Object(_)));
    if !existing_is_object {
        if object.contains_key(META_KEY) {
            tracing::warn!(
                target: "mcp.v2",
                "a handler set result._meta to a non-object; replacing it with an object so \
                 the server-owned reserved keys can be attached"
            );
        }
        object.insert(META_KEY.to_string(), Value::Object(serde_json::Map::new()));
    }
    object.get_mut(META_KEY).and_then(Value::as_object_mut)
}

/// Assert SERVER OWNERSHIP over the closed set of reserved result fields
/// (T-113-59 / T-113-60).
///
/// # The authoritative reserved-field registry
///
/// | Reserved key | Location | Server behavior when a handler already set it |
/// |---|---|---|
/// | `resultType` | top-level result | OVERWRITE with the disposition the server computed |
/// | `io.modelcontextprotocol/serverInfo` | `result._meta` | OVERWRITE with the server's real `Implementation` |
/// | `requestState` | top-level result | REMOVE unless the **MRTR** egress minted it — that is its ONLY legitimate minter |
/// | `inputRequests` | top-level result | REMOVE unless the **MRTR** egress minted it (on an `input_required` result) **or** the **v2 tasks dispatch** did (on a `tasks/get` for an `input_required` task, where the ext-tasks schema marks it a REQUIRED top-level field) |
/// | `dev.pmcp/mrtr` | `result._meta` | REMOVE always |
///
/// `inputRequests` is the one reserved key with TWO legitimate minters, which is
/// why ownership is an explicit [`ReservedFieldOwner`] input rather than a single
/// boolean: the grant is per-KEY per-OWNER. The tasks dispatch may publish
/// `inputRequests` and may NOT publish `requestState` — the tasks surface has no
/// continuation token, because the persisted task record replaces the sealed
/// continuation (D-17).
///
/// A TOP-LEVEL `serverInfo` is deliberately NOT in the registry: it is a
/// legitimate schema field of `ServerDiscoverResult` and `InitializeResult`, and
/// removing or overwriting it would corrupt those results. The reserved location
/// is [`RESERVED_SERVER_INFO_KEY`] inside `_meta`, and only that one is owned.
///
/// Every OTHER `_meta` key a handler set is preserved untouched — ownership
/// applies to this enumerated set only. [`MRTR_SIGNAL_META_KEY`](crate::types::mrtr::MRTR_SIGNAL_META_KEY)
/// carries a copy of this table in its rustdoc, since it is the key handler
/// authors actually type.
///
/// # Why OVERWRITE rather than the collision-safe `entry().or_insert`
///
/// Phase 112 preserved a handler-supplied value, which is exactly backwards for
/// a reserved field. Under that rule a handler could set
/// `resultType: "input_required"` on `tools/list` and sail straight past the
/// [`client_request_mrtr_eligible`] tripwire — the enum match gates what the
/// SERVER emits, not what a handler smuggles into its own result object — or
/// spoof `serverInfo` to impersonate another server. Both are server identity /
/// protocol-envelope claims, not handler payload, so the server states them
/// unconditionally and logs a `tracing::warn!` naming the field it overrode, so
/// a handler author sees the mistake instead of it failing silently.
///
/// # Ownership is an INPUT, never derived from the disposition
///
/// `owner` is supplied by the egress that did the minting — see
/// [`ReservedFieldOwner`] for the measured defect that removing the derivation
/// fixed. It is deliberately a separate parameter from `disposition`: the two
/// facts are independent, and the case that proves it is the v2 tasks dispatch,
/// whose disposition is `complete` while it legitimately owns `inputRequests`.
pub(crate) fn own_reserved_result_fields(
    result: &mut Value,
    server_info: &Implementation,
    disposition: ResponseDisposition,
    owner: ReservedFieldOwner,
) {
    let wire_result_type = disposition.as_wire_str();
    if let Some(object) = result.as_object_mut() {
        if object
            .get(crate::types::mrtr::RESULT_TYPE_KEY)
            .is_some_and(|existing| existing != wire_result_type)
        {
            tracing::warn!(
                target: "mcp.v2",
                field = crate::types::mrtr::RESULT_TYPE_KEY,
                "overwrote a handler-supplied reserved result field with the server-computed \
                 value"
            );
        }
        object.insert(
            crate::types::mrtr::RESULT_TYPE_KEY.to_string(),
            Value::String(wire_result_type.to_string()),
        );
        // Per-KEY, per-OWNER. The loop visits every reserved top-level key on
        // every path, so a new owner cannot silently gain a key by being added
        // to the enum — it has to say so in `may_emit`.
        for field in [
            crate::types::mrtr::REQUEST_STATE_KEY,
            crate::types::mrtr::INPUT_REQUESTS_KEY,
        ] {
            if owner.may_emit(field) {
                continue;
            }
            if object.remove(field).is_some() {
                tracing::warn!(
                    target: "mcp.v2",
                    field,
                    "removed a handler-supplied reserved result field from a result this \
                     egress did not mint"
                );
            }
        }
    }

    // `_meta` is CREATED here when absent (v2 object results only — the era gate
    // is above), and MERGED into when the handler already set other keys. The
    // reserved key is INSERTED, so a handler-supplied server identity is
    // overwritten rather than respected.
    let Some(meta) = result_meta_object_mut(result) else {
        return;
    };
    // Defense in depth for the internal signal key. `mrtr_egress` — which strips
    // it unconditionally — is `streamable-http`-only by D-14, so on a build
    // without that feature NOTHING else would remove it from a v2 result.
    if meta
        .remove(crate::types::mrtr::MRTR_SIGNAL_META_KEY)
        .is_some()
    {
        tracing::warn!(
            target: "mcp.v2",
            field = crate::types::mrtr::MRTR_SIGNAL_META_KEY,
            "removed the pmcp-internal MRTR signal from an outgoing result"
        );
    }
    if meta.contains_key(RESERVED_SERVER_INFO_KEY) {
        tracing::warn!(
            target: "mcp.v2",
            field = RESERVED_SERVER_INFO_KEY,
            "overwrote a handler-supplied reserved _meta field with the server's real \
             Implementation"
        );
    }
    meta.insert(
        RESERVED_SERVER_INFO_KEY.to_string(),
        serde_json::to_value(server_info).unwrap_or(Value::Null),
    );
}

/// Build the v2 `server/discover` response (Phase 112, VERS-04, D-09/D-10).
///
/// The SINGLE shared projection consumed by BOTH the production HTTP caller
/// (`Server::handle_discover` → the streamable-HTTP `HttpIngress::Discover`
/// classifier) and the discover unit tests — there is exactly one projection and
/// one envelope path, no duplicate capability type and no `#[allow(dead_code)]`
/// wrapper.
///
/// A READ-ONLY projection of the server's already-computed `capabilities`
/// (including the `extensions` map) via the isolated
/// [`discover_result_from_capabilities`] conversion fn — it never recomputes
/// capabilities and never triggers an initialize-style side effect (no
/// `is_initialized` mutation). It is era-gated: only an `Era::V2` request is
/// served; a v1 / non-opted-in request receives standard `-32601`
/// method-not-found (D-10), the same reject the public `parse_request` produces
/// for `server/discover`.
///
/// # `source` carries the accept-list, and production MUST name one (G-7)
///
/// [`DiscoverSource`] bundles the capabilities with the server's protocol
/// accept-list, because `supportedVersions` on the result and
/// `error.data.supported` on an unsupported-version rejection are required to be
/// the same list. The `impl Into<_>` seam admits a capabilities-only source, but
/// that conversion is `#[cfg(test)]`, so a PRODUCTION caller has no way to reach
/// this fn without naming an accept-list explicitly.
pub(crate) fn build_discover_response(
    id: RequestId,
    source: DiscoverSource<'_>,
    info: &Implementation,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
) -> JSONRPCResponse {
    // Era gate (D-10): v2 only. A v1 / non-opted-in request is method-not-found.
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return ServerCore::error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            "Method not found: server/discover".to_string(),
        );
    }

    let negotiated_version = protocol_context.map_or_else(
        || crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
        |ctx| ctx.negotiated_version.as_str().to_string(),
    );

    // Read-only projection of the ALREADY-COMPUTED capabilities — no recompute.
    let result = discover_result_from_capabilities(source, info, negotiated_version);
    let mut response = ServerCore::success_response(id, serde_json::to_value(result).unwrap());
    // Parity: the v2 object result carries resultType + serverInfo via the SAME
    // shared envelope helper every other v2 result uses. `server/discover` mints
    // no reserved MRTR/tasks field, so it owns none of them.
    inject_v2_result_envelope(
        &mut response,
        protocol_context,
        info,
        ResponseDisposition::Complete,
        ReservedFieldOwner::None,
        // `server/discover` is `DiscoverResult extends CacheableResult` in the
        // 2026-07-28 schema, and it is the FIRST call a v2 client makes, so it
        // must carry the hints. SCHM-03's requirement text says "five" list/read
        // results; this is the measured SIXTH (115-RESEARCH § Finding 5, asserted
        // by `tests/v2_core_schema_facts.rs`). Excluding it would ship a
        // knowingly non-conformant v2 `server/discover`.
        //
        // It is also why `request_is_cacheable` has no `server/discover` row:
        // this method does not ride the `ClientRequest` route at all — it is
        // answered here, and names its own claim.
        Cacheable::Yes,
    );
    response
}

// ===========================================================================
// MRTR ingress + egress (Plan 113-06, HTTP-02 / HTTP-03).
//
// ONE shared unit, called from BOTH native dispatch sites — `ServerCore` below
// and the high-level `Server` in `server/mod.rs`. That is the Phase-109/112
// twin-site parity rule: `mod.rs` CALLS these helpers, it never defines its own.
//
// D-14 confines the AEAD `requestState` codec to native + `streamable-http`, so
// the whole unit carries that gate and a build without the feature runs zero
// MRTR code.
// ===========================================================================

/// The principal a server with NO auth provider configured binds v2
/// state-bearing artifacts to.
///
/// Such a deployment has no principals to separate — every caller arrives as the
/// same (absent) identity — so collapsing them onto one NAMED constant is honest
/// rather than lossy, and it means the principal expression has exactly one
/// source and no session-id branch (T-113-06). The TTL and the
/// originating-request binding remain the residual replay controls.
///
/// A server that DOES configure an auth provider never reaches this value: an
/// unauthenticated request is refused outright (T-113-22 / T-114-37).
///
/// # Why this is not `streamable-http`-gated
///
/// It was, while MRTR was its only consumer. Plan 114-09 gave the v2 `tasks/*`
/// owner binding the SAME identity table, and `task_dispatch` is gated
/// `not(wasm32)` WITHOUT the feature — so a build without `streamable-http`
/// still needs this constant. Widening the gate is what lets `task_dispatch`
/// REUSE the value instead of writing a second `""` literal, which would be a
/// second source of truth for "the anonymous bucket".
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const ANONYMOUS_PRINCIPAL: &str = "";

/// The ONE client-facing message for every `requestState` authentication
/// failure.
///
/// Tamper, wrong principal and cross-request replay are deliberately
/// indistinguishable to the client: all three live in the AEAD's additional
/// authenticated data and fail `ring`'s constant-time tag check, and telling the
/// client WHICH one failed would be a discrimination oracle (T-113-10). The
/// discriminated reason is `tracing::warn!`-logged server-side only.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_REJECT_MESSAGE: &str = "invalid requestState";

/// The SERVER-side ceiling on MRTR rounds (D-113-L).
///
/// A "round" is one gather→resend cycle: the server answers `input_required`
/// carrying a `requestState` minted at `round + 1`, and the client resends the
/// same request presenting it. [`seal_input_required`] mints that increment;
/// this constant is what the increment is finally compared against.
///
/// # Why a server-side bound exists at all
///
/// It did not, until D-113-L. The D-09 round counter had exactly ONE enforcement
/// point in the tree — `DEFAULT_MRTR_ROUND_LIMIT` (8) in `src/client/mod.rs` —
/// i.e. the security counter was enforced solely by the party it exists to
/// constrain. A non-pmcp or hostile client simply ignored its own limit, resent
/// indefinitely, and the server obligingly re-ran the handler and re-minted every
/// time until the counter SATURATED at 255, at which point a handler trying to
/// self-limit on [`RequestHandlerExtra::mrtr_round`] could no longer distinguish
/// round 255 from round 3000. A bound only the attacker enforces is not a bound.
///
/// # Why 16
///
/// Exactly TWICE `DEFAULT_MRTR_ROUND_LIMIT`. A default-configured pmcp client
/// gives up at 8 and therefore can never trip this ceiling; the 2x headroom
/// leaves room for a deliberately raised client limit while still bounding an
/// absent one. That relationship is a checked invariant rather than a comment
/// two files apart: the integration test
/// `a_flow_within_the_client_default_limit_is_unaffected` in `tests/v2_mrtr.rs`
/// drives a full 8-round flow and fails if this value is ever lowered past it.
///
/// # Why a constant and not a builder knob
///
/// Deliberately deferred. A per-server ceiling would have to be threaded through
/// `ServerCoreBuilder` and `ServerBuilder` and carried into both dispatch sites —
/// a config-surface change with its own semver, precedence and default-value
/// questions, none of which is what closes D-113-L. What closes D-113-L is a
/// bound the SERVER enforces. Making that bound tunable later is additive and
/// cannot reintroduce the defect, because the enforcement point will already
/// exist; shipping the knob first would have left the same hole behind a
/// configuration default.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) const MAX_MRTR_ROUNDS: u8 = 16;

/// The client-facing message for a request refused at [`MAX_MRTR_ROUNDS`].
///
/// Deliberately NOT [`MRTR_REJECT_MESSAGE`]. That message is generic because
/// telling a client WHICH of tamper / wrong-principal / cross-request replay
/// failed would be a discrimination oracle (T-113-10). This refusal happens
/// AFTER the AEAD tag check passed — the caller is provably the principal the
/// continuation was minted for, on the request it was minted for — so naming the
/// ceiling discloses nothing the caller could not already count for itself, and
/// it saves an operator a debugging session against an opaque `-32602`.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_ROUND_CEILING_MESSAGE: &str =
    "this request exceeded the server's multi-round-trip round limit";

/// The client-facing message for the MINT-site round backstop.
///
/// Distinct from [`MRTR_ROUND_CEILING_MESSAGE`] on purpose: reaching the ingress
/// refusal is normal server operation against a misbehaving client, whereas
/// reaching the mint backstop means the ingress bound was bypassed, which is a
/// server-internal invariant violation. Two messages keep the two situations
/// distinguishable in an operator's logs.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_ROUND_CEILING_INVARIANT_MESSAGE: &str =
    "a requestState continuation cannot be minted past the server's round limit";

/// The client-facing message for a request whose params cannot be canonicalized
/// for the AEAD binding (D-113-M).
///
/// Deliberately NOT [`MRTR_REJECT_MESSAGE`]. Genericity buys secrecy only where
/// there is something to keep secret: the generic message exists so a client
/// cannot distinguish tamper from wrong-principal from cross-request replay
/// (T-113-10). This condition is STRUCTURAL and entirely client-side — the caller
/// chose the nesting depth and can measure it itself — so it is not an
/// authentication oracle, and a specific message is what lets an operator tell a
/// too-deep payload from a forged token.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_UNCANONICALIZABLE_MESSAGE: &str =
    "these request params nest too deeply to be bound to a multi-round-trip continuation";

/// The message for the MINT-site canonicalization backstop.
///
/// Distinct from [`MRTR_UNCANONICALIZABLE_MESSAGE`] for the same reason
/// [`MRTR_ROUND_CEILING_INVARIANT_MESSAGE`] is distinct from
/// [`MRTR_ROUND_CEILING_MESSAGE`]: reaching the egress refusal is normal
/// operation against a deep payload, whereas reaching the mint backstop means the
/// egress precheck was bypassed, which is a server-internal invariant violation.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_UNCANONICALIZABLE_INVARIANT_MESSAGE: &str =
    "the originating request could not be canonicalized for the continuation binding";

/// The identity inputs a v2 state-bearing artifact is bound to.
///
/// `AuthContext::subject` is the ONLY identity anchor — never `clientInfo`
/// (self-reported), never a `client_id` (per-APPLICATION, OAuth `azp`), never a
/// session id (v2 has none). Carried as a `&str` so every call site can pass the
/// SAME value without cloning the whole `AuthContext`.
///
/// Named for MRTR because MRTR minted it (Phase 113). Since plan 114-09 it is
/// also the input to the v2 `tasks/*` owner binding — ONE identity table for
/// every v2 ingress path on the server, so the name is now historical rather
/// than a scope statement. See [`resolve_mrtr_principal`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct MrtrPrincipal<'a> {
    /// `AuthContext::subject`, or `None` when the request produced no
    /// `AuthContext`.
    pub authenticated_subject: Option<&'a str>,
    /// Whether this server has an auth provider configured — the fail-closed
    /// input (T-113-22).
    pub has_auth_provider: bool,
}

/// Resolve the v2 principal, FAIL-CLOSED — the SINGLE identity table.
///
/// | `authenticated_subject` | `has_auth_provider` | result |
/// |---|---|---|
/// | `Some(subject)` | any | `Some(subject)` |
/// | `None` | `true` | `None` — REFUSE |
/// | `None` | `false` | <code>Some([ANONYMOUS_PRINCIPAL])</code> |
///
/// Row 2 is the fail-closed row: a state-bearing artifact must not be mintable
/// or redeemable by an unauthenticated caller on a server that expects
/// authentication (T-113-22).
///
/// # Two callers, one table (plan 114-09)
///
/// 1. MRTR ingress/egress, which binds a `requestState` continuation to it
///    (Phase 113, this file), and
/// 2. [`TaskDispatch::resolve_owner`](crate::server::task_dispatch::TaskDispatch::resolve_owner),
///    which binds a v2 TASK owner to it (Phase 114, TASK-05).
///
/// The argument is the same one in both cases: a continuation and a task record
/// are both server-held state a later request redeems, so "who may redeem it"
/// must have exactly one answer per server. A second `match` over the same two
/// inputs — however carefully copied — is a second answer waiting to drift, and
/// (the 114-08 lesson) it also destroys the negative control that would prove
/// either copy load-bearing.
///
/// `subscriptions/listen`'s `resolve_listen_principal` deliberately does NOT
/// collapse onto this function; its third row is a concurrency-accounting key,
/// not an identity, and the reason is written at that function.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn resolve_mrtr_principal(principal: MrtrPrincipal<'_>) -> Option<&str> {
    match (principal.authenticated_subject, principal.has_auth_provider) {
        (Some(subject), _) => Some(subject),
        (None, true) => None,
        (None, false) => Some(ANONYMOUS_PRINCIPAL),
    }
}

/// Whether this [`ClientRequest`] variant may carry an `input_required` result
/// — the COMPILE-TIME half of the eligibility tripwire (T-113-23).
///
/// The spec is explicit: "Servers **MUST NOT** send `InputRequiredResult`
/// responses on any other client requests." Two independent mechanisms enforce
/// that, and both must agree:
///
/// * this EXHAUSTIVE no-wildcard match over the request ENUM, and
/// * the [`MRTR_METHODS`](crate::types::mrtr::MRTR_METHODS) string table that
///   [`mrtr_eligible`](crate::types::mrtr::mrtr_eligible) reads.
///
/// The absence of a wildcard arm is the point: a future `ClientRequest` variant
/// is a `non-exhaustive patterns` COMPILE ERROR here, forcing its author to
/// classify it explicitly rather than inheriting "not eligible" by silence — the
/// same discipline [`extract_request_meta_value`] applies to the `_meta` signal.
/// The three eligible arms DERIVE their answer from the table rather than
/// returning a bare `true`, so the enum and the table cannot drift apart in the
/// permissive direction either; `enum_eligibility_agrees_with_the_method_table`
/// pins the other direction.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) fn client_request_mrtr_eligible(request: &ClientRequest) -> bool {
    use crate::types::mrtr::{
        mrtr_eligible, CALL_TOOL_METHOD, GET_PROMPT_METHOD, READ_RESOURCE_METHOD,
    };
    match request {
        // Eligible — and eligible BECAUSE the one method table says so.
        ClientRequest::CallTool(_) => mrtr_eligible(CALL_TOOL_METHOD),
        ClientRequest::GetPrompt(_) => mrtr_eligible(GET_PROMPT_METHOD),
        ClientRequest::ReadResource(_) => mrtr_eligible(READ_RESOURCE_METHOD),
        // NOT eligible — enumerated explicitly, no wildcard arm.
        ClientRequest::Initialize(_)
        | ClientRequest::ListTools(_)
        | ClientRequest::ListPrompts(_)
        | ClientRequest::ListResources(_)
        | ClientRequest::ListResourceTemplates(_)
        | ClientRequest::Subscribe(_)
        | ClientRequest::Unsubscribe(_)
        | ClientRequest::Complete(_)
        | ClientRequest::CreateMessage(_)
        | ClientRequest::TasksGet(_)
        | ClientRequest::TasksResult(_)
        | ClientRequest::TasksList(_)
        | ClientRequest::TasksCancel(_)
        | ClientRequest::SetLoggingLevel { .. }
        | ClientRequest::Ping => false,
    }
}

/// The `(method, live params)` pair MRTR binds a `requestState` token to.
///
/// Derived from the TYPED request dispatch will ACTUALLY execute, never from an
/// attacker-echoed copy of the params (T-113-03) — so a token minted for one
/// tool + arguments cannot verify against another.
///
/// Returns `None` for every request outside the three MRTR-eligible methods,
/// which is what makes a `requestState` presented on e.g. `tools/list` inert
/// rather than verified (T-113-23).
///
/// # The strip half of the D-15 strip-and-re-run mechanic
///
/// [`splice_mrtr_params`](crate::types::mrtr::splice_mrtr_params) with the
/// DEFAULT removes `inputResponses` and `requestState` unconditionally. On this
/// path they are already absent — the typed request structs deliberately do not
/// model them (D-113-D) — so this is belt-and-braces: the params handed to the
/// digest, and therefore the shape a re-run handler is bound to, can never carry
/// a client-echoed MRTR field even if the salient whitelist is widened later.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) fn mrtr_binding_parts(request: &Request) -> Option<(&'static str, Value)> {
    let Request::Client(boxed) = request else {
        return None;
    };
    // The compile-time tripwire runs FIRST, so an unclassified future variant
    // can never reach the serde-derived resolution below.
    if !client_request_mrtr_eligible(boxed.as_ref()) {
        return None;
    }
    // Derive (method, params) from serde, NOT from a hand-written match.
    //
    // `ClientRequest` is `#[serde(tag = "method", content = "params")]`, so this
    // IS the canonical variant->wire mapping and cannot fall behind a new
    // variant. The previous three-arm match re-spelled the method strings and
    // the salient keys that `MRTR_METHODS` already owns, which made the failure
    // mode silent AND security-relevant: adding a fourth table row made
    // `mrtr_eligible` and `logical_name_key` correct while this function still
    // returned `None`, so `mrtr_ingest` short-circuited to `Inert` and a
    // presented `requestState` was NEVER VERIFIED.
    let mut frame = serde_json::to_value(boxed.as_ref()).ok()?;
    // Resolve through the table so the returned `&'static str` IS the row's own
    // spelling — adding a row is now the only edit a new MRTR method needs.
    let method = crate::types::mrtr::mrtr_method_static(frame.get("method")?.as_str()?)?;
    let mut params = frame.get_mut("params").map_or(Value::Null, Value::take);
    // The digest whitelists only the row's salient keys, so the extra fields the
    // serialized form carries (`_meta`, `task`) never reach it — the bound shape
    // is byte-identical to the hand-built one.
    crate::types::mrtr::splice_mrtr_params(
        &mut params,
        &crate::types::mrtr::MrtrRequestParams::default(),
    );
    Some((method, params))
}

/// The routing decision for a presented `requestState` — LOCKED by D-15.
///
/// | Verdict | Route | Why |
/// |---------|-------|-----|
/// | no verdict — the params could not be CANONICALIZED | [`Reject`](Self::Reject) | D-113-M — the binding is computed BEFORE `verify`, so a request nested past the canonicalization depth cap never reaches the table at all. Refused rather than verified against a digest that would identify a whole class of requests instead of this one |
/// | `Ok(c)` whose round is at or past [`MAX_MRTR_ROUNDS`] | [`Reject`](Self::Reject) | D-113-L — the SERVER half of the D-09 bound. Refused here, before dispatch, so the handler is never invoked on the refused round |
/// | `Ok(c)` | [`Proceed`](Self::Proceed) | resume from the decrypted continuation |
/// | `AuthFailed` | [`Reject`](Self::Reject) | conformance `sep-2322-reject-tampered-state`: a complete result OR a re-prompt is a FAILURE |
/// | `UnknownKey` | [`Reelicit`](Self::Reelicit) `{ round: 0 }` | D-04 degraded path — another instance's per-process key, nothing is decryptable, so start over |
/// | `Expired(c)` whose round is at or past [`MAX_MRTR_ROUNDS`] | [`Reject`](Self::Reject) | D-113-L — expiry must not LAUNDER a round past the ceiling. The same round-preservation that stops a hostile server resetting the bound (T-113-49) would otherwise become the bypass, since a server can always let its own tokens expire |
/// | `Expired(c)` | [`Reelicit`](Self::Reelicit) `{ round: c.round }` | D-05/D-15 — authentic, so the round SURVIVES and a hostile server cannot reset the client's D-09 bound by letting tokens expire (T-113-49) |
///
/// # `UnknownKey` resetting to round 0 is NOT a ceiling bypass
///
/// A client that wants a fresh round counter does not need a forged key id: it
/// can simply send the request WITHOUT a `requestState` and start a new
/// operation, which any client may always do and which no server can prevent.
/// `UnknownKey` is therefore indistinguishable from a legitimate fresh start,
/// and refusing it would break the D-04 multi-instance degradation path — a real
/// cost paid for no security gain (T-113-113, disposition ACCEPT). Recorded here
/// so it is not re-litigated on the next read.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub(crate) enum MrtrIngest {
    /// MRTR does not apply to this request — dispatch is byte-for-byte unchanged.
    Inert,
    /// The token verified: resume with this continuation and round.
    Proceed {
        /// The DECRYPTED, server-minted continuation state.
        continuation: Value,
        /// The round the token was minted in.
        round: u8,
        /// The server's OWN record of which kind it requested under each
        /// `inputRequests` key, read back out of the verified continuation
        /// (D-113-O). `None` means the token predates the field — see
        /// [`Continuation::kinds`](crate::server::request_state::Continuation).
        ///
        /// It arrives here only on the `Ok` verdict, i.e. only AFTER the AEAD
        /// tag check passed, which is what makes it trustworthy input to a
        /// policy decision: the client can neither choose it nor alter it.
        kinds: Option<crate::types::mrtr::InputRequestKinds>,
    },
    /// The token failed authentication, the verified continuation is at or past
    /// [`MAX_MRTR_ROUNDS`], the request could not be canonicalized, or the
    /// client's `inputResponses` do not match the kinds the server requested:
    /// answer a JSON-RPC error and NEVER invoke the handler.
    Reject {
        /// The JSON-RPC error code (always `INVALID_PARAMS`).
        code: i32,
        /// The client-facing message: [`MRTR_REJECT_MESSAGE`] — deliberately
        /// generic — for an authentication failure;
        /// [`MRTR_ROUND_CEILING_MESSAGE`] — deliberately specific — for a
        /// round-ceiling refusal, which happens only AFTER the token verified;
        /// and [`MRTR_UNCANONICALIZABLE_MESSAGE`] — also specific, because the
        /// condition is structural and client-chosen rather than an
        /// authentication outcome.
        ///
        /// Owned rather than `&'static str` because the kind-mismatch refusal
        /// (D-113-O) NAMES the offending key, and that key is only known at
        /// runtime. It comes from the sealed continuation, never from the
        /// request — see
        /// [`InputResponseTypingError`](crate::types::mrtr::InputResponseTypingError)
        /// for why one variant may name its key and the other may not.
        message: String,
    },
    /// Strip the MRTR fields and RE-RUN the original handler from scratch, so
    /// the response carries real `inputRequests` the client can answer.
    Reelicit {
        /// The round to carry into the freshly minted token — `0` for an unknown
        /// key, the decrypted `round` for an expired one.
        round: u8,
    },
}

/// Inputs to [`mrtr_ingest`], bundled so both dispatch sites pass the same shape.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) struct MrtrIngestInputs<'a> {
    /// The [`mrtr_binding_parts`] pair for this request.
    pub target: Option<&'a (&'static str, Value)>,
    /// The once-at-ingress resolved protocol context, carrying the transport's
    /// raw MRTR params.
    pub protocol_context: Option<&'a crate::types::protocol::ProtocolContext>,
    /// The identity inputs (see [`MrtrPrincipal`]).
    pub principal: MrtrPrincipal<'a>,
    /// The SERVER-OWNED codec, borrowed from server state — never a global.
    pub codec: Option<&'a crate::server::request_state::RequestStateCodec>,
}

/// Verify a presented `requestState` against the LIVE principal and originating
/// request, and route the verdict per D-15.
///
/// Short-circuits to [`MrtrIngest::Inert`] — running zero MRTR code — when the
/// era is not v2, when the method is not MRTR-eligible, when no token was
/// presented, or when this server holds no codec.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) fn mrtr_ingest(inputs: &MrtrIngestInputs<'_>) -> MrtrIngest {
    // v1 / non-opted-in requests run ZERO MRTR code (D-04).
    let Some(context) = inputs.protocol_context else {
        return MrtrIngest::Inert;
    };
    if context.era != crate::types::protocol::Era::V2 {
        return MrtrIngest::Inert;
    }
    // T-113-23: the spec confines MRTR to three methods. A `requestState`
    // presented on any other method is IGNORED — not verified, not errored.
    let Some(target) = inputs.target else {
        return MrtrIngest::Inert;
    };
    if !crate::types::mrtr::mrtr_eligible(target.0) {
        return MrtrIngest::Inert;
    }
    // No token → nothing to verify. A request carrying `inputResponses` alone
    // still reaches the handler with them populated.
    let Some(token) = context.request_state_token() else {
        return MrtrIngest::Inert;
    };
    let Some(principal) = resolve_mrtr_principal(inputs.principal) else {
        tracing::warn!(
            target: "mcp.mrtr",
            method = target.0,
            "refused a state-bearing request from an unauthenticated caller on an \
             auth-configured server"
        );
        return MrtrIngest::Reject {
            code: crate::types::protocol::error_codes::INVALID_PARAMS,
            message: MRTR_REJECT_MESSAGE.to_string(),
        };
    };
    // A server with no codec never opted into v2 continuations.
    let Some(codec) = inputs.codec else {
        return MrtrIngest::Inert;
    };
    // REFUSAL POINT 1 of 2 for D-113-M — the VERIFY path.
    //
    // A token was presented and the request cannot be identified. Fail CLOSED: a
    // request whose identity cannot be computed must not be granted a verification
    // attempt at all, because the only thing that would distinguish it from any
    // other over-deep request is the very digest that could not be computed.
    let binding = match crate::server::request_state::RequestBinding::from_request(
        principal, target.0, &target.1,
    ) {
        Ok(binding) => binding,
        Err(error) => {
            tracing::warn!(
                target: "mcp.mrtr",
                method = target.0,
                max_depth = error.max,
                "refused a state-bearing request whose params nest past the \
                 canonicalization depth limit — such a request has no digest that \
                 identifies it rather than a class of requests (D-113-M)"
            );
            return MrtrIngest::Reject {
                code: crate::types::protocol::error_codes::INVALID_PARAMS,
                message: MRTR_UNCANONICALIZABLE_MESSAGE.to_string(),
            };
        },
    };
    route_mrtr_verdict(codec.verify(token, &binding), target.0)
}

/// ENFORCEMENT POINT A for [`MAX_MRTR_ROUNDS`] — refuse a VERIFIED continuation
/// that has reached the server's round ceiling (D-113-L).
///
/// `round` here has already survived the AEAD tag check, so it is server-minted
/// and integrity-protected: it is trustworthy input to a policy decision in a way
/// that nothing else on the request is.
///
/// Extracted rather than inlined at the two call sites because
/// [`route_mrtr_verdict`] exists precisely to hold `mrtr_ingest`'s cognitive
/// complexity down, and a bound duplicated inline in two match arms is a bound
/// that can be half-removed.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn refuse_past_round_ceiling(round: u8, method: &str) -> Option<MrtrIngest> {
    if round < MAX_MRTR_ROUNDS {
        return None;
    }
    tracing::warn!(
        target: "mcp.mrtr",
        method,
        round,
        max_rounds = MAX_MRTR_ROUNDS,
        "refused a requestState at or past the server's round ceiling — this client \
         resent past the bound its own round limit should have stopped it at (D-113-L)"
    );
    Some(MrtrIngest::Reject {
        code: crate::types::protocol::error_codes::INVALID_PARAMS,
        message: MRTR_ROUND_CEILING_MESSAGE.to_string(),
    })
}

/// The D-15 verdict table, isolated so [`mrtr_ingest`] stays well under
/// cognitive-complexity 25.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn route_mrtr_verdict(verdict: crate::server::request_state::Verdict, method: &str) -> MrtrIngest {
    use crate::server::request_state::Verdict;
    match verdict {
        Verdict::Ok(continuation) => {
            if let Some(refusal) = refuse_past_round_ceiling(continuation.round, method) {
                return refusal;
            }
            MrtrIngest::Proceed {
                continuation: continuation.state,
                round: continuation.round,
                kinds: continuation.kinds,
            }
        },
        Verdict::AuthFailed => {
            tracing::warn!(
                target: "mcp.mrtr",
                method,
                "rejected a requestState that failed authentication — tampered, minted \
                 for a different principal, or replayed onto a different request"
            );
            MrtrIngest::Reject {
                code: crate::types::protocol::error_codes::INVALID_PARAMS,
                message: MRTR_REJECT_MESSAGE.to_string(),
            }
        },
        Verdict::UnknownKey => {
            tracing::warn!(
                target: "mcp.mrtr",
                method,
                "requestState carries a key id this instance does not hold — re-eliciting \
                 from round 0 (D-04 multi-instance degradation)"
            );
            MrtrIngest::Reelicit { round: 0 }
        },
        // Authentic, so the round survives (T-113-49) — and so, therefore, must
        // the ceiling: re-eliciting at or past it would turn the very property
        // that stops a hostile server resetting the bound into the bypass, since
        // letting one's own tokens expire is entirely within a server's gift
        // (T-113-112).
        Verdict::Expired(continuation) => {
            if let Some(refusal) = refuse_past_round_ceiling(continuation.round, method) {
                return refusal;
            }
            MrtrIngest::Reelicit {
                round: continuation.round,
            }
        },
    }
}

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
impl MrtrIngest {
    /// Fold this verdict into the [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// threaded into dispatch, returning it plus the round to carry to egress.
    ///
    /// `Err((code, message))` is a [`Reject`](Self::Reject): the caller answers a
    /// JSON-RPC error and the handler is NEVER invoked.
    ///
    /// [`Reelicit`](Self::Reelicit) STRIPS every MRTR signal from the context, so
    /// the re-run handler observes `input_responses()`, `mrtr_continuation()` and
    /// `mrtr_round()` all `None` — a pristine FIRST call. MRTR-participating
    /// handlers must therefore be idempotent up to the point of their first
    /// `input_required` return, which is inherently true: a handler that returned
    /// `input_required` had not completed the operation.
    ///
    /// [`Proceed`](Self::Proceed) additionally RE-TYPES the client's
    /// `inputResponses` against the kinds the verified continuation records
    /// (D-113-O) — see [`retype_verified_input_responses`].
    pub(crate) fn apply(
        self,
        context: Option<crate::types::protocol::ProtocolContext>,
    ) -> std::result::Result<(Option<crate::types::protocol::ProtocolContext>, u8), (i32, String)>
    {
        match self {
            Self::Inert => Ok((context, 0)),
            Self::Proceed {
                continuation,
                round,
                kinds,
            } => {
                let context = match context {
                    Some(ctx) => Some(
                        retype_verified_input_responses(ctx, kinds.as_ref())?
                            .with_verified_continuation(continuation, round),
                    ),
                    None => None,
                };
                Ok((context, round))
            },
            Self::Reelicit { round } => Ok((
                context.map(crate::types::protocol::ProtocolContext::without_mrtr),
                round,
            )),
            Self::Reject { code, message } => Err((code, message)),
        }
    }
}

/// Re-type the client's `inputResponses` against the kinds the SERVER recorded
/// requesting, replacing the untagged guess ingress made (D-113-O).
///
/// # Why this runs here and not at transport ingress
///
/// The kinds live inside the AEAD-sealed continuation, so they are readable only
/// after `codec.verify` returned [`Verdict::Ok`](crate::server::request_state::Verdict).
/// Ingress parses `params` long before that and cannot know them — which is why it
/// guessed, and why the guess could be wrong in a way nothing detected. Running
/// here means the kinds this enforces against have already passed the AEAD tag
/// check: the client can neither choose them nor alter them.
///
/// # The three outcomes
///
/// | Continuation's `kinds` | Behaviour |
/// |------------------------|-----------|
/// | `None` (minted by a pre-D-113-O build) | keep the untagged values — the documented rolling-deploy degradation |
/// | `Some(map)`, every answered key present and decodable as its kind | replace with the kind-directed typing |
/// | `Some(map)`, any key missing or any value undecodable | REJECT with `INVALID_PARAMS` |
///
/// A decode failure is a rejection, never a fallback to the untagged guess:
/// falling back is exactly the defect.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn retype_verified_input_responses(
    context: crate::types::protocol::ProtocolContext,
    kinds: Option<&crate::types::mrtr::InputRequestKinds>,
) -> std::result::Result<crate::types::protocol::ProtocolContext, (i32, String)> {
    // No answers on this round: the client presented a token and asked the
    // handler to continue without answering anything. Nothing to re-type, and
    // nothing to reject — a handler that still needs an answer will simply ask
    // again, which is the pre-existing `sep-2322-missing-response` behaviour.
    let Some(raw) = context.input_responses_raw() else {
        return Ok(context);
    };
    let retyped =
        crate::types::mrtr::retype_input_responses_for_kinds(raw, kinds).map_err(|error| {
            // The message is SPECIFIC rather than the generic
            // `MRTR_REJECT_MESSAGE`, for the same reason the round-ceiling
            // refusal is: it fires only AFTER the AEAD tag check passed, so it
            // is not an authentication oracle. What it may and may not name is
            // decided by `InputResponseTypingError`'s own `Display`, which
            // distinguishes a server-assigned key from a client-chosen one.
            tracing::warn!(
                target: "mcp.mrtr",
                unsolicited = matches!(
                    error,
                    crate::types::mrtr::InputResponseTypingError::Unsolicited { .. }
                ),
                "rejected an inputResponses entry that does not match the input request \
                 the server recorded making — before D-113-O this was silently \
                 reclassified and the handler re-elicited forever"
            );
            (
                crate::types::protocol::error_codes::INVALID_PARAMS,
                error.to_string(),
            )
        })?;
    // `None` is the pre-kinds degradation: leave the untagged typing in place.
    Ok(match retyped {
        Some(typed) => context.with_kind_directed_input_responses(typed),
        None => context,
    })
}

/// Inputs to [`mrtr_egress`], bundled so both dispatch sites pass the same shape.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) struct MrtrEgressInputs<'a> {
    /// The [`mrtr_binding_parts`] pair for this request.
    pub target: Option<&'a (&'static str, Value)>,
    /// The once-at-ingress resolved protocol context.
    pub protocol_context: Option<&'a crate::types::protocol::ProtocolContext>,
    /// The identity inputs (see [`MrtrPrincipal`]).
    pub principal: MrtrPrincipal<'a>,
    /// The SERVER-OWNED codec, borrowed from server state.
    pub codec: Option<&'a crate::server::request_state::RequestStateCodec>,
    /// The round [`MrtrIngest::apply`] resolved; the fresh token is minted at
    /// `round + 1`, unless that would exceed [`MAX_MRTR_ROUNDS`], in which case
    /// [`seal_input_required`] refuses to mint at all (D-113-L).
    pub round: u8,
}

/// The outcome of the UNCONDITIONAL internal-signal strip.
///
/// Three states rather than an `Option`, because "the reserved key was present
/// but did not parse" must not collapse into "no signal": a handler that meant
/// to return `input_required` and got the shape wrong would otherwise ship a
/// silently EMPTY success for an operation it never completed.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub(crate) enum StrippedSignal {
    /// The reserved key was not present.
    Absent,
    /// A well-formed signal was removed from `_meta`.
    Present(Box<crate::types::mrtr::MrtrSignal>),
    /// The reserved key was present but is not a well-formed `MrtrSignal`.
    Malformed,
}

/// Take the pmcp-INTERNAL MRTR signal off a result's `_meta`, on EVERY path.
///
/// The removal is unconditional — v1, non-eligible method, ineligible era, all
/// of it — because [`MRTR_SIGNAL_META_KEY`](crate::types::mrtr::MRTR_SIGNAL_META_KEY)
/// carries the handler's PLAINTEXT continuation. Publishing it would hand the
/// client the very state the AEAD token exists to seal. An `_meta` emptied by
/// the removal is dropped, so a signalling handler's wire shape matches a
/// non-signalling one exactly.
///
/// This runs BEFORE any era or eligibility branch in [`mrtr_egress`]; there is
/// no path on which publishing the key is correct, so there is no path on which
/// this is skipped (T-113-31 / T-113-60).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) fn strip_mrtr_signal(result: &mut Value) -> StrippedSignal {
    // The removal itself lives in `types::mrtr`, UNGATED, so the
    // `not(streamable-http)` build can reach it too — see `scrub_mrtr_signal`.
    let Some(raw) = crate::types::mrtr::remove_mrtr_signal(result) else {
        return StrippedSignal::Absent;
    };
    serde_json::from_value(raw).map_or(StrippedSignal::Malformed, |signal| {
        StrippedSignal::Present(Box::new(signal))
    })
}

/// The `not(streamable-http)` counterpart of [`strip_mrtr_signal`].
///
/// [`MrtrSignal`](crate::types::mrtr::MrtrSignal) and
/// [`MRTR_SIGNAL_META_KEY`](crate::types::mrtr::MRTR_SIGNAL_META_KEY) are `pub`
/// on every build, but [`mrtr_egress`] — the only thing that strips them — is
/// `streamable-http`-only by D-14, and [`own_reserved_result_fields`]'s
/// defense-in-depth removal only runs on a **v2** result. Without this, a handler
/// on a stdio-only build that wrote the reserved key would publish its PLAINTEXT
/// continuation verbatim, which is exactly what the module doc promises never
/// happens ("STRIPS the key on EVERY path before serialization — v1 included").
#[cfg(not(feature = "streamable-http"))]
pub(crate) fn scrub_mrtr_signal(response: &mut JSONRPCResponse) {
    if let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload {
        if crate::types::mrtr::remove_mrtr_signal(value).is_some() {
            tracing::warn!(
                target: "mcp.mrtr",
                field = crate::types::mrtr::MRTR_SIGNAL_META_KEY,
                "removed the pmcp-internal MRTR signal from an outgoing result"
            );
        }
    }
}

/// The MRTR target for this response, or `None` when `input_required` is
/// FORBIDDEN here.
///
/// Two independent gates, both of which must pass: the era must be v2, and the
/// dispatched request must be one of the three methods the spec allows an
/// `InputRequiredResult` on. `inputs.target` is itself produced by
/// [`mrtr_binding_parts`], whose first gate is the exhaustive no-wildcard
/// [`client_request_mrtr_eligible`] match — so a future `ClientRequest` variant
/// cannot reach here without an explicit classification (T-113-23).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn eligible_mrtr_target<'a>(inputs: &MrtrEgressInputs<'a>) -> Option<&'a (&'static str, Value)> {
    if !matches!(
        inputs.protocol_context.map(|ctx| ctx.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return None;
    }
    inputs
        .target
        .filter(|target| crate::types::mrtr::mrtr_eligible(target.0))
}

/// Replace the response with a JSON-RPC error, discarding whatever the handler
/// produced.
///
/// Used for every fail-closed MRTR egress path: a half-emitted `input_required`
/// (requests without a token, or a token without requests) is strictly worse
/// than an error, because the client cannot resume from it and cannot tell that
/// it should not try (T-113-33).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn fail_mrtr_egress(
    response: &mut JSONRPCResponse,
    code: i32,
    message: String,
    data: Option<Value>,
) -> (ResponseDisposition, ReservedFieldOwner) {
    response.payload =
        crate::types::jsonrpc::ResponsePayload::Error(crate::types::jsonrpc::JSONRPCError {
            code,
            message,
            data,
        });
    (ResponseDisposition::Complete, ReservedFieldOwner::None)
}

/// Convert a handler's MRTR signal into a wire `input_required` result.
///
/// Returns the [`ResponseDisposition`] the shared envelope helper should emit,
/// paired with the [`ReservedFieldOwner`] that minted the reserved result
/// fields. Both travel to [`inject_v2_result_envelope`] together: the owner is
/// stated by the code that WRITES the keys (`seal_input_required`), never
/// re-derived downstream from the disposition.
///
/// # The order of operations is load-bearing
///
/// 1. **Strip, unconditionally.** [`strip_mrtr_signal`] runs before any era or
///    eligibility branch, so the pmcp-internal key and its plaintext
///    continuation cannot reach the wire on ANY path (T-113-31 / T-113-60).
/// 2. **Fail loudly where MRTR is impossible.** A signal on v1, on a
///    non-opted-in request, or on a method outside the three eligible ones is a
///    server BUG — no legitimate handler writes the reserved key — so it becomes
///    an `INTERNAL_ERROR` rather than a silently mangled "complete" result.
/// 3. **Check declared client capabilities BEFORE minting.** A rejected result
///    costs zero cryptographic work, and the server never asks a client for
///    something it cannot answer (T-113-32).
///    **(3b) Refuse a request that cannot be BOUND.** Params nested past the
///    canonicalization depth cap have no digest that identifies them, so no
///    continuation may be minted against them — `INVALID_PARAMS`, because the
///    cause is the client's params, not a server bug (D-113-M). Sits with the
///    capability precheck for the same reason: a refused result costs zero
///    cryptographic work.
/// 4. **Mint, then write.** A mint failure is an error, never a partial result.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) fn mrtr_egress(
    response: &mut JSONRPCResponse,
    inputs: &MrtrEgressInputs<'_>,
) -> (ResponseDisposition, ReservedFieldOwner) {
    // (1) UNCONDITIONAL strip — before the era check, before the eligibility
    // check, on v1 as well as v2.
    let stripped = match response.payload {
        crate::types::jsonrpc::ResponsePayload::Result(ref mut value) => strip_mrtr_signal(value),
        crate::types::jsonrpc::ResponsePayload::Error(_) => StrippedSignal::Absent,
    };
    let signal = match stripped {
        StrippedSignal::Absent => return (ResponseDisposition::Complete, ReservedFieldOwner::None),
        StrippedSignal::Malformed => {
            tracing::error!(
                target: "mcp.mrtr",
                method = inputs.target.map(|target| target.0),
                "a handler wrote the reserved MRTR signal key with a payload that is not a \
                 well-formed MrtrSignal"
            );
            return fail_mrtr_egress(
                response,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                MRTR_MALFORMED_SIGNAL_MESSAGE.to_string(),
                None,
            );
        },
        StrippedSignal::Present(signal) => signal,
    };
    // Below this line the signal can only be CONSUMED, never leaked.

    // (2) A signal where MRTR is impossible is a server bug — fail loudly.
    let Some(target) = eligible_mrtr_target(inputs) else {
        tracing::error!(
            target: "mcp.mrtr",
            method = inputs.target.map(|target| target.0),
            "a handler signalled input_required where the spec forbids it — on v1, on a \
             non-opted-in request, or on a method outside tools/call, prompts/get and \
             resources/read"
        );
        return fail_mrtr_egress(
            response,
            crate::types::protocol::error_codes::INTERNAL_ERROR,
            MRTR_FORBIDDEN_PATH_MESSAGE.to_string(),
            None,
        );
    };

    // (3) Declared-capability precheck, BEFORE any minting.
    if let Some(rejection) = reject_undeclared_capabilities(&signal, inputs, target.0) {
        return fail_mrtr_egress(
            response,
            crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
            rejection.0,
            Some(rejection.1),
        );
    }

    // (3b) REFUSAL POINT 2 of 2 for D-113-M — the MINT path.
    //
    // `INVALID_PARAMS`, not `INTERNAL_ERROR`: the condition is caused entirely by
    // the params the CLIENT sent, so it belongs with the sibling MRTR reject
    // rather than in the `INTERNAL_ERROR` mint-failure channel below, which is
    // reserved for server bugs. Placed before the mint for the same reason step
    // (3) is: a request that cannot be bound costs zero cryptographic work.
    if let Err(error) = crate::types::mrtr::salient_param_digest(target.0, &target.1) {
        tracing::warn!(
            target: "mcp.mrtr",
            method = target.0,
            max_depth = error.max,
            "refused to mint a continuation for params that nest past the \
             canonicalization depth limit — no requestState was minted (D-113-M)"
        );
        return fail_mrtr_egress(
            response,
            crate::types::protocol::error_codes::INVALID_PARAMS,
            MRTR_UNCANONICALIZABLE_MESSAGE.to_string(),
            None,
        );
    }

    // (4) Mint and write.
    match seal_input_required(response, &signal, target, inputs) {
        Ok(minted) => minted,
        Err(reason) => {
            tracing::error!(target: "mcp.mrtr", reason, "could not emit an input_required result");
            fail_mrtr_egress(
                response,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                reason.to_string(),
                None,
            )
        },
    }
}

/// The client-facing message for a signal on a path where MRTR is impossible.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_FORBIDDEN_PATH_MESSAGE: &str =
    "the server produced an input_required signal on a request that cannot carry one";

/// The client-facing message for a reserved-key payload that is not an
/// `MrtrSignal`.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MRTR_MALFORMED_SIGNAL_MESSAGE: &str = "the server produced a malformed input_required signal";

/// The client-facing message for `-32021`.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
const MISSING_CAPABILITY_MESSAGE: &str =
    "the server needs a client capability this client did not declare";

/// Reject the whole response when any `inputRequests` entry needs a capability
/// or submode the client did not declare (T-113-32).
///
/// Returns `Some((message, data))` for a rejection, where `data` is
/// `{"requiredCapabilities": <ClientCapabilities OBJECT>}` — an OBJECT such as
/// `{"elicitation": {}}`, never an array and never a list of strings. Emitting
/// an array here is a wire-contract violation the official conformance suite
/// grades.
///
/// **All-or-nothing.** A partial `inputRequests` map with the undeclared entries
/// silently dropped is NOT an option: the spec's MUST NOT is about the whole
/// result, and a client answering a subset would resume a continuation the
/// handler cannot complete.
///
/// # `clientCapabilities` is NOT an authorization input
///
/// The declared capabilities are CLIENT-SUPPLIED and trivially forgeable. They
/// say only what the client can ANSWER, never what it is allowed to reach. No
/// access decision may read them; the AEAD `requestState` binding and
/// [`resolve_mrtr_principal`] are the identity controls.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn reject_undeclared_capabilities(
    signal: &crate::types::mrtr::MrtrSignal,
    inputs: &MrtrEgressInputs<'_>,
    method: &str,
) -> Option<(String, Value)> {
    let declared = inputs
        .protocol_context
        .and_then(|context| context.client_capabilities.as_ref());
    let missing = missing_client_capabilities(&signal.input_requests, declared)?;
    let required =
        serde_json::to_value(&missing).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    tracing::warn!(
        target: "mcp.mrtr",
        method,
        required = %required,
        "refused to emit inputRequests for a capability the client did not declare — no \
         requestState was minted"
    );
    Some((
        MISSING_CAPABILITY_MESSAGE.to_string(),
        serde_json::json!({ "requiredCapabilities": required }),
    ))
}

/// One capability-or-submode an `inputRequests` map needs.
///
/// A five-variant enum in a set rather than five `bool` fields: clippy's
/// `struct_excessive_bools` caps a struct at three, and the set shape says the
/// thing directly — these are the members of a domain, not independent switches.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MissingCapability {
    /// No `elicitation` capability at all was declared.
    Elicitation,
    /// `elicitation` was declared, but without URL-mode support.
    ElicitationUrl,
    /// No `sampling` capability at all was declared.
    Sampling,
    /// `sampling` was declared, but without tool-augmented support.
    SamplingTools,
    /// No `roots` capability was declared.
    Roots,
}

/// Which client capabilities an `inputRequests` map needs but the client did not
/// declare.
///
/// Accumulated as a SET rather than as partially-built capability objects, so
/// the "two entries both need elicitation" case does not require merging two
/// [`ElicitationCapabilities`](crate::types::capabilities::ElicitationCapabilities)
/// values.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[derive(Debug, Default)]
struct MissingCapabilities(std::collections::BTreeSet<MissingCapability>);

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
impl MissingCapabilities {
    /// Note whatever `request` needs and `declared` lacks.
    fn note(
        &mut self,
        request: &crate::types::mrtr::InputRequest,
        declared: Option<&crate::types::ClientCapabilities>,
    ) {
        match request {
            crate::types::mrtr::InputRequest::Elicitation(params) => {
                self.note_elicitation(params, declared.and_then(|caps| caps.elicitation.as_ref()));
            },
            crate::types::mrtr::InputRequest::Sampling(params) => {
                self.note_sampling(params, declared.and_then(|caps| caps.sampling.as_ref()));
            },
            crate::types::mrtr::InputRequest::ListRoots => {
                if declared.is_none_or(|caps| caps.roots.is_none()) {
                    self.0.insert(MissingCapability::Roots);
                }
            },
        }
    }

    /// Elicitation is SUBMODE-aware: a form entry needs only the capability
    /// object, a URL entry needs the declared object to carry URL support.
    ///
    /// The submode signal read here is
    /// [`ElicitationCapabilities::url`](crate::types::capabilities::ElicitationCapabilities::url),
    /// which exists in the shipped 2025-11-25 capability type. `113-SPEC-RECHECK.md`
    /// records the Phase-113 spec verdict as PENDING, so plan 12 must re-verify
    /// that the final 2026-07-28 schema still expresses URL support as this
    /// sub-field before any Phase-113 requirement is flipped complete.
    fn note_elicitation(
        &mut self,
        params: &crate::types::elicitation::ElicitRequestParams,
        declared: Option<&crate::types::capabilities::ElicitationCapabilities>,
    ) {
        match (params, declared) {
            (crate::types::elicitation::ElicitRequestParams::Form { .. }, None) => {
                self.0.insert(MissingCapability::Elicitation);
            },
            // Nothing declared AND a URL entry: BOTH are missing. Reporting only
            // the base capability made `requiredCapabilities` incomplete, so a
            // client that declared exactly what it was told (form-only
            // elicitation) was refused again on its next attempt.
            (crate::types::elicitation::ElicitRequestParams::Url { .. }, None) => {
                self.0.insert(MissingCapability::Elicitation);
                self.0.insert(MissingCapability::ElicitationUrl);
            },
            (crate::types::elicitation::ElicitRequestParams::Form { .. }, Some(_)) => {},
            // Declared, but form-only: the SUBMODE is what is missing.
            (crate::types::elicitation::ElicitRequestParams::Url { .. }, Some(caps)) => {
                if caps.url.is_none() {
                    self.0.insert(MissingCapability::ElicitationUrl);
                }
            },
        }
    }

    /// Sampling requires the `sampling` capability, and a tool-augmented request
    /// additionally requires the client's declared `sampling.tools` sub-field.
    fn note_sampling(
        &mut self,
        params: &crate::types::sampling::CreateMessageParams,
        declared: Option<&crate::types::capabilities::SamplingCapabilities>,
    ) {
        let needs_tools = params.tools.is_some() || params.tool_choice.is_some();
        let tools_declared = declared.is_some_and(|caps| caps.tools.is_some());
        if declared.is_none() {
            self.0.insert(MissingCapability::Sampling);
        }
        if needs_tools && !tools_declared {
            self.0.insert(MissingCapability::SamplingTools);
        }
    }

    /// Project the set into a `ClientCapabilities` OBJECT carrying ONLY what is
    /// missing, or `None` when nothing is.
    fn into_capabilities(self) -> Option<crate::types::ClientCapabilities> {
        if self.0.is_empty() {
            return None;
        }
        let empty = || Value::Object(serde_json::Map::new());
        let has = |capability| self.0.contains(&capability);
        let mut missing = crate::types::ClientCapabilities::default();
        if has(MissingCapability::Elicitation) || has(MissingCapability::ElicitationUrl) {
            missing.elicitation = Some(crate::types::capabilities::ElicitationCapabilities {
                form: None,
                url: has(MissingCapability::ElicitationUrl).then(empty),
            });
        }
        if has(MissingCapability::Sampling) || has(MissingCapability::SamplingTools) {
            missing.sampling = Some(crate::types::capabilities::SamplingCapabilities {
                models: None,
                context: None,
                tools: has(MissingCapability::SamplingTools).then(empty),
            });
        }
        if has(MissingCapability::Roots) {
            missing.roots = Some(crate::types::capabilities::RootsCapabilities::default());
        }
        Some(missing)
    }
}

/// The capabilities `requests` needs that `declared` does not offer, or `None`
/// when every kind and submode is declared.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn missing_client_capabilities(
    requests: &crate::types::mrtr::InputRequests,
    declared: Option<&crate::types::ClientCapabilities>,
) -> Option<crate::types::ClientCapabilities> {
    let mut missing = MissingCapabilities::default();
    for request in requests.values() {
        missing.note(request, declared);
    }
    missing.into_capabilities()
}

/// Mint the continuation and write the two SERVER-OWNED `input_required` fields
/// onto the result.
///
/// `inputRequests` and `requestState` are INSERTED (overwriting), never
/// `entry().or_insert`-ed: they are server-owned reserved fields, and a
/// handler-supplied value must never survive. `resultType` is deliberately NOT
/// written here — [`inject_v2_result_envelope`] is its single writer.
///
/// The spec requires an `InputRequiredResult` to carry at least one of
/// `inputRequests` or `requestState`. Both are written unconditionally here and
/// a mint failure short-circuits before either is, so the obligation holds by
/// construction (T-113-33).
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
fn seal_input_required(
    response: &mut JSONRPCResponse,
    signal: &crate::types::mrtr::MrtrSignal,
    target: &(&'static str, Value),
    inputs: &MrtrEgressInputs<'_>,
) -> std::result::Result<(ResponseDisposition, ReservedFieldOwner), &'static str> {
    // ENFORCEMENT POINT B for `MAX_MRTR_ROUNDS` (D-113-L): the mint-site
    // backstop.
    //
    // UNREACHABLE while `refuse_past_round_ceiling` is intact at the ingress
    // verdict — a continuation at or past the ceiling is refused there, so
    // `inputs.round` cannot arrive here above `MAX_MRTR_ROUNDS - 1`. It exists so
    // that a future refactor of the verdict table cannot silently DELETE the
    // bound; that exact failure mode is what D-113-L already demonstrated once,
    // when the only enforcement lived in a different crate module and nothing on
    // the server compared the minted round to anything.
    //
    // Its `INTERNAL_ERROR` classification — `mrtr_egress` routes this
    // `Err(&'static str)` through `fail_mrtr_egress` — is correct PRECISELY
    // because reaching it means that internal invariant is broken. The client
    // did nothing new; the server did.
    let next_round = inputs.round.saturating_add(1);
    if next_round > MAX_MRTR_ROUNDS {
        return Err(MRTR_ROUND_CEILING_INVARIANT_MESSAGE);
    }
    let principal = resolve_mrtr_principal(inputs.principal)
        .ok_or("a requestState continuation cannot be minted for an unauthenticated caller")?;
    let codec = inputs
        .codec
        .ok_or("this server has no requestState codec configured")?;
    // The D-113-M mint-site BACKSTOP.
    //
    // UNREACHABLE BY CONSTRUCTION while `mrtr_egress` step (3b) is intact: that
    // step computes the digest for this exact `target` and refuses before calling
    // here, so `from_request` cannot fail on the same value moments later. It is
    // retained against a future reordering that moved or deleted step (3b) —
    // the same two-point structure, and the same reason, as the round-ceiling
    // backstop above.
    //
    // Its `INTERNAL_ERROR` classification (via `mrtr_egress`'s `Err(&'static str)`
    // routing) is correct PRECISELY because reaching it means that internal
    // ordering invariant broke. Step (3b) owns the client-caused
    // `INVALID_PARAMS` answer.
    let binding =
        crate::server::request_state::RequestBinding::from_request(principal, target.0, &target.1)
            .map_err(|_| MRTR_UNCANONICALIZABLE_INVARIANT_MESSAGE)?;
    // The kinds map (D-113-O). This is the ONE place in the SDK where the
    // requested kinds are known, so it is the one place they can be sealed: the
    // map is built from the handler's OWN `inputRequests`, via
    // `InputRequest::kind()`, and never from anything on the client's request.
    //
    // `Some(...)` unconditionally, INCLUDING when the handler asked for nothing.
    // `None` means "minted by a build that predates this field" and selects the
    // untagged degradation; a handler that signalled an empty `inputRequests`
    // asked for nothing, which is a different statement and must reject every
    // answer rather than accept arbitrary ones. See `Continuation::kinds`.
    let kinds: crate::types::mrtr::InputRequestKinds = signal
        .input_requests
        .iter()
        .map(|(key, request)| (key.clone(), request.kind()))
        .collect();
    let token = codec
        .mint(&signal.continuation, &binding, next_round, Some(kinds))
        .map_err(|_| "the requestState continuation could not be sealed")?;
    let input_requests = serde_json::to_value(&signal.input_requests)
        .map_err(|_| "the handler's inputRequests map is not serializable")?;

    let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload else {
        return Err("an input_required signal cannot ride on an error response");
    };
    let result = value
        .as_object_mut()
        .ok_or("an input_required result must be a JSON object")?;
    // `resultType` is deliberately NOT written here: the returned
    // `ResponseDisposition::InputRequired` is threaded to
    // `inject_v2_result_envelope`, which is the single writer of that key.
    result.insert(
        crate::types::mrtr::INPUT_REQUESTS_KEY.to_string(),
        input_requests,
    );
    result.insert(
        crate::types::mrtr::REQUEST_STATE_KEY.to_string(),
        Value::String(token),
    );
    // The ownership claim is made HERE, at the two `insert` calls it describes,
    // and travels with the disposition to the registry. It is not re-derived
    // downstream from the disposition — that derivation is the row-23 defect.
    Ok((ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr))
}

/// One request's MRTR round, owned across the dispatch `.await`.
///
/// # Why this exists
///
/// `mrtr_ingest` and `mrtr_egress` were always shared, but the ~120 lines of
/// PLUMBING around them — derive the target, build [`MrtrPrincipal`], clone the
/// subject so it outlives `auth_context` moving into dispatch, `.apply()` the
/// verdict, map the rejection, thread `round` to egress, rebuild the principal —
/// were copy-pasted at BOTH dispatch sites (`ServerCore::handle_request` and
/// `Server::handle_request_with_context`), ~85% byte-identical, kept in step by a
/// "twin-site parity" comment rather than by the compiler.
///
/// That was survivable while egress could only fail to emit `input_required`.
/// It stopped being survivable when plan 09 gave egress four fail-closed exits
/// (malformed signal, forbidden method, undeclared capability, mint failure):
/// a site that assembled `MrtrEgressInputs` even slightly differently would emit
/// DIVERGENT wire-visible JSON-RPC errors depending on which dispatch path the
/// request took. Now there is one assembly, so divergence is structurally
/// impossible instead of review-enforced.
///
/// Holds no borrows, so nothing is kept alive across the handler `.await` beyond
/// the binding egress genuinely needs.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub(crate) struct MrtrRound {
    /// The `(method, params)` binding this request is bound to, or `None` when
    /// the request is v1 / non-opted-in / not MRTR-eligible.
    target: Option<(&'static str, Value)>,
    /// The ONE owned identity anchor, so egress rebuilds the SAME binding after
    /// `auth_context` has moved into dispatch.
    subject: Option<String>,
    /// Whether this server has an auth provider configured (fail-closed input).
    has_auth_provider: bool,
    /// The round a verified continuation arrived on; egress mints at `round + 1`.
    round: u8,
}

#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
impl MrtrRound {
    /// Ingress: verify a presented `requestState` and fold the D-15 verdict into
    /// the context threaded into dispatch.
    ///
    /// `Err((code, message))` is a rejection the caller turns into its own
    /// site-appropriate error response — the ONE thing that legitimately differs
    /// between the two dispatch sites.
    ///
    /// Inert on v1 / non-opted-in / non-eligible requests, so the legacy path is
    /// byte-for-byte unchanged.
    pub(crate) fn begin(
        request: &Request,
        context: Option<crate::types::protocol::ProtocolContext>,
        auth_subject: Option<&str>,
        has_auth_provider: bool,
        codec: Option<&crate::server::request_state::RequestStateCodec>,
    ) -> std::result::Result<(Self, Option<crate::types::protocol::ProtocolContext>), (i32, String)>
    {
        // Era-gated before the binding is derived, so v1 runs ZERO MRTR code and
        // pays no deep clone of the request params.
        let target = context
            .as_ref()
            .filter(|context| context.era == crate::types::protocol::Era::V2)
            .and_then(|_| mrtr_binding_parts(request));
        // Owned once, used by BOTH halves. `mrtr_ingest` short-circuits to
        // `Inert` on a `None` target before it ever reads the principal, so
        // deriving the subject from the target here is observationally identical
        // to the previous split (ingest borrowed from `auth_context`, egress read
        // this owned copy).
        let subject = target
            .as_ref()
            .and_then(|_| auth_subject.map(str::to_string));
        let (context, round) = mrtr_ingest(&MrtrIngestInputs {
            target: target.as_ref(),
            protocol_context: context.as_ref(),
            principal: MrtrPrincipal {
                authenticated_subject: subject.as_deref(),
                has_auth_provider,
            },
            codec,
        })
        .apply(context)?;
        Ok((
            Self {
                target,
                subject,
                has_auth_provider,
                round,
            },
            context,
        ))
    }

    /// Egress: mint an `input_required` continuation, or strip-only.
    ///
    /// The SINGLE assembly of [`MrtrEgressInputs`] — see the type docs for why
    /// having two was a wire-divergence hazard.
    ///
    /// Returns the disposition AND the [`ReservedFieldOwner`], which both
    /// dispatch sites hand straight to [`inject_v2_result_envelope`].
    pub(crate) fn finish(
        &self,
        response: &mut JSONRPCResponse,
        context: Option<&crate::types::protocol::ProtocolContext>,
        codec: Option<&crate::server::request_state::RequestStateCodec>,
    ) -> (ResponseDisposition, ReservedFieldOwner) {
        mrtr_egress(
            response,
            &MrtrEgressInputs {
                target: self.target.as_ref(),
                protocol_context: context,
                principal: MrtrPrincipal {
                    authenticated_subject: self.subject.as_deref(),
                    has_auth_provider: self.has_auth_provider,
                },
                codec,
                round: self.round,
            },
        )
    }
}

#[async_trait]
impl ProtocolHandler for ServerCore {
    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse {
        // Convert Request to JSONRPCRequest for middleware processing
        let mut jsonrpc_request = create_request(id.clone(), request.clone());

        // Create middleware context with request_id, method, and start_time
        let context = MiddlewareContext::with_request_id(id.to_string());
        context.set_metadata("method".to_string(), jsonrpc_request.method.clone());

        // Process request through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_request_with_context(&mut jsonrpc_request, &context)
            .await
        {
            // Middleware rejected the request (on_error already called by chain)
            return Self::error_response(
                id,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                e.to_string(),
            );
        }

        // Resolve the per-request ProtocolContext ONCE at native ingress
        // (opted-in servers only — D-04). This is the single authoritative
        // resolution threaded through dispatch; the HTTP layer (Plan 06) resolves
        // once for its header gate and passes the same value in, never re-derived.
        let protocol_context = match self.resolve_ingress_protocol_context(&request) {
            Ok(ctx) => ctx,
            Err(negotiation_error) => {
                let (code, message) = negotiation_error_to_rejection(&negotiation_error);
                return Self::error_response(id, code, message);
            },
        };

        // MRTR ingress (Plan 113-06, HTTP-03): verify a presented `requestState`
        // against the LIVE principal and originating request through the ONE
        // shared helper `server/mod.rs` also calls, and fold the D-15 verdict
        // into the context threaded into dispatch. Inert on v1 / non-opted-in /
        // non-eligible requests, so the legacy path is unchanged.
        #[cfg(feature = "streamable-http")]
        let (mrtr, protocol_context) = match MrtrRound::begin(
            &request,
            protocol_context,
            auth_context.as_ref().map(|ctx| ctx.subject.as_str()),
            self.auth_provider.is_some(),
            self.request_state_codec(),
        ) {
            Ok(resolved) => resolved,
            Err((code, message)) => return Self::error_response(id, code, message),
        };

        // Capture the cacheability claim while `request` is still HERE.
        //
        // The same shape as the `CreateTrigger::resolve(...)` capture above: the
        // fact is derivable only from the request, and the request is MOVED into
        // `handle_request_internal` a few lines below, so the claim has to be
        // taken now or not at all. By the time `inject_v2_result_envelope` runs,
        // the response is an opaque `serde_json::Value` and the method is out of
        // scope. This binding sits OUTSIDE the `#[cfg(feature =
        // "streamable-http")]` MRTR block so it exists on builds with and
        // without that feature.
        //
        // `request_is_cacheable` is the ONE shared table `server/mod.rs` calls
        // too — this site never classifies on its own.
        let cacheable = request_is_cacheable(&request);

        // G-9 / CONF-08 (Phase 118.1-08): fold the v1 `initialize` handshake's
        // advertised capabilities into the context threaded into DISPATCH, so
        // every downstream `RequestHandlerExtra` construction site sees them
        // without nine copies of this read. THE ONE lock read per dispatch, and
        // the fold owns it — including the guard that keeps v2 traffic from
        // touching the lock at all (T-118.1-08-02) — so `server/mod.rs` shares
        // the whole unit rather than re-spelling its impure half (twin-site
        // parity). The read guard is dropped inside the fold, well before
        // `handle_initialize` below takes the write lock. The EGRESS below
        // deliberately keeps the UNFOLDED `protocol_context`: the fold is a
        // handler-visibility concern, not a wire-shape one.
        let dispatch_context = fold_v1_handshake_capabilities(
            protocol_context.clone(),
            &self.client_capabilities,
            &self.supported_protocol_versions,
        )
        .await;

        // Execute the actual request handling with auth_context.
        //
        // `dispatch_claim` is the SECOND envelope claimant (Phase 114 plan 11):
        // the tasks routes and the create path state their own `resultType` and
        // reserved-field ownership from the site that writes them, several frames
        // below here. It stays `NONE` for every other dispatch.
        let mut dispatch_claim = DispatchEnvelopeClaim::NONE;
        let mut response = self
            .handle_request_internal(
                id.clone(),
                request,
                auth_context,
                dispatch_context,
                &mut dispatch_claim,
            )
            .await;

        // MRTR egress (Plan 113-06): convert a handler's "I need more input"
        // signal into an `input_required` result carrying a freshly minted
        // `requestState`, and STRIP the pmcp-internal signal key on every other
        // path so it never reaches the wire.
        #[cfg(feature = "streamable-http")]
        let (disposition, reserved_field_owner) = mrtr.finish(
            &mut response,
            protocol_context.as_ref(),
            self.request_state_codec(),
        );
        #[cfg(not(feature = "streamable-http"))]
        let (disposition, reserved_field_owner) = {
            // No `mrtr_egress` on this build, so the unconditional strip has to
            // happen here or the reserved key reaches the wire (see
            // `scrub_mrtr_signal`).
            scrub_mrtr_signal(&mut response);
            (ResponseDisposition::Complete, ReservedFieldOwner::None)
        };

        // Inject the v2-only response envelope (resultType + serverInfo) and
        // project the caching hints at the era-gated serialization boundary
        // (VERS-07 / D-07 / D-08, SCHM-03). The ENVELOPE half is a no-op for
        // v1 / non-opted-in responses (byte-identical) and for
        // error/notification/non-object results; the caching half runs on both
        // eras — ensure on v2, STRIP on v1 (D-11).
        //
        // The two claimants are folded through ONE named rule so precedence is
        // stated rather than implied by argument order.
        let claim = dispatch_claim.or_egress(disposition, reserved_field_owner);
        inject_v2_result_envelope(
            &mut response,
            protocol_context.as_ref(),
            &self.info,
            claim.disposition,
            claim.owner,
            cacheable,
        );

        // Process response through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_response_with_context(&mut response, &context)
            .await
        {
            // Log error but return the response anyway
            tracing::warn!("Response middleware processing failed: {}", e);
        }

        response
    }

    async fn handle_notification(&self, notification: Notification) -> Result<()> {
        // Convert Notification to JSONRPCNotification for middleware processing
        let mut jsonrpc_notification = create_notification(notification.clone());

        // Create middleware context with method and start_time (no request_id for notifications)
        let context = MiddlewareContext::default();
        context.set_metadata("method".to_string(), jsonrpc_notification.method.clone());

        // Process notification through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_notification_with_context(&mut jsonrpc_notification, &context)
            .await
        {
            // Log error but continue
            tracing::warn!("Notification middleware processing failed: {}", e);
        }

        // Handle the actual notification (current implementation does nothing)
        self.handle_notification_internal(notification).await
    }

    fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    fn info(&self) -> &Implementation {
        &self.info
    }
}

impl ServerCore {
    /// Bind this request to a task owner — ERA-AWARE, fail-closed on v2.
    ///
    /// A thin delegation to
    /// [`TaskDispatch::resolve_owner`](crate::server::task_dispatch::TaskDispatch::resolve_owner),
    /// which holds the rule ONCE for both dispatchers. On v1 (and on a request
    /// carrying no era code at all) the answer is byte-identical to what it has
    /// always been: the [`TaskRouter`] priority chain, or the OAuth subject with
    /// a store-only backend, or the shared `"local"` bucket. On v2 it is the
    /// three-row identity table, whose middle row answers
    /// [`OwnerBinding::Refused`](crate::server::task_dispatch::OwnerBinding::Refused)
    /// — an unauthenticated caller on a server that HAS an auth provider binds
    /// no owner at all (TASK-05, T-114-37).
    ///
    /// `era` is the ALREADY-RESOLVED per-request era (Phase 112); this function
    /// never re-reads `params._meta` to recover it.
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_task_owner(
        &self,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> crate::server::task_dispatch::OwnerBinding {
        // Delegate to the shared TaskDispatch unit (owner-resolution lives there,
        // once, for both dispatchers).
        self.task_dispatch().resolve_owner(auth_context, era)
    }

    /// Borrow this `ServerCore`'s task backends into the shared dispatch unit.
    #[cfg(not(target_arch = "wasm32"))]
    fn task_dispatch(&self) -> crate::server::task_dispatch::TaskDispatch<'_> {
        crate::server::task_dispatch::TaskDispatch {
            task_store: &self.task_store,
            task_router: &self.task_router,
            // The SAME read `MrtrRound::begin` already makes on this type — no
            // new field and no new accessor, feeding the SAME identity table
            // (TASK-05 / T-113-22).
            has_auth_provider: self.auth_provider.is_some(),
        }
    }

    /// Build the `tools/call` create-task response for a `TaskCreated` outcome.
    ///
    /// Per `D-STORE-MINTS-ID` (review finding #3): when a [`TaskStore`] is
    /// configured the store mints the canonical task id via `store.create()`;
    /// that store-minted id is reflected on the WIRE in BOTH
    /// `CreateTaskResult.task.taskId` AND the `_meta.relatedTask.taskId`
    /// envelope (never the tool's fabricated id). When the terminal `result`
    /// is present (synchronous completion) it is persisted via
    /// `store.set_result()` and the task is transitioned `Working -> Completed`
    /// BEFORE the response returns, so a subsequent `tasks/get` shows
    /// `Completed`.
    ///
    /// Falls back to the legacy tool-fabricated envelope only when no store is
    /// configured (preserves prior behavior for router-only servers).
    #[cfg(not(target_arch = "wasm32"))]
    async fn build_task_created_response(
        &self,
        id: RequestId,
        task_value: Value,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> (JSONRPCResponse, DispatchEnvelopeClaim) {
        // Delegate to the shared TaskDispatch unit. It RE-EXTRACTS the task id and
        // the terminal result from `task_value` internally (store mints the id;
        // `extract_terminal_result` recovers the terminal CallToolResult), so the
        // store-minted-id and synchronous-completion-persistence invariants live in
        // exactly one place.
        //
        // `era` reaches the SAME owner-binding table every `tasks/*` route uses, so
        // a task is created under exactly the rule that later governs who may read
        // it — and on v2's refuse row nothing is minted at all (T-114-37).
        self.task_dispatch()
            .build_task_created_response(id, task_value, auth_context, era)
            .await
    }

    /// Handle a `tasks/result` request.
    ///
    /// Per review finding #2 (store-vs-router precedence): serves from the
    /// configured [`TaskStore`] FIRST when it `supports_results()`, but FALLS
    /// THROUGH to the [`TaskRouter`](crate::server::tasks::TaskRouter) on store
    /// `NotFound`/unsupported — never a hard error when a router can serve it.
    /// When the store has no result and NO router is configured, returns a
    /// SPECIFIED "task not completed" error (`-32002`), distinct from the
    /// truly-no-backend `-32601`.
    /// Internal request handler without middleware processing.
    ///
    /// `dispatch_claim` is an OUT parameter: the two dispatches that mint a
    /// non-default v2 envelope (the `tasks/*` routes and the `tools/call` create
    /// path) write their claim into it, and `handle_request` folds it with the
    /// MRTR egress's own claim. It is an out-param rather than a changed return
    /// type because only two of this function's ~two dozen arms have anything to
    /// say, and the rest must stay untouched.
    async fn handle_request_internal(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
        dispatch_claim: &mut DispatchEnvelopeClaim,
    ) -> JSONRPCResponse {
        contract_pre_session_lifecycle!();
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };

                match self.handle_initialize(init_req).await {
                    Ok(result) => Self::success_response(id, serde_json::to_value(result).unwrap()),
                    Err(e) => Self::error_response(
                        id,
                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                        e.to_string(),
                    ),
                }
            },
            Request::Client(ref boxed_req) => {
                // Check if server is initialized for server requests (skip in stateless mode
                // and on v2, which has no `initialize` handshake at all — see
                // `v1_initialize_gate_applies`).
                if v1_initialize_gate_applies(
                    self.stateless_mode,
                    protocol_context.as_ref().map(|ctx| ctx.era),
                ) && !self.is_initialized().await
                {
                    return Self::error_response(
                        id,
                        // FROZEN wire value -32002 (byte-identical); read from the
                        // centralized table by name (Pitfall 6). Unreachable on v2
                        // by the predicate above, which is what keeps this
                        // spec-prohibited code off the v2 wire (Finding 11).
                        crate::types::protocol::error_codes::V1_TASK_PENDING,
                        "Server not initialized. Call initialize first.".to_string(),
                    );
                }

                match boxed_req.as_ref() {
                    ClientRequest::ListTools(req) => match self.handle_list_tools(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        ),
                    },
                    ClientRequest::CallTool(req) => {
                        // Capture the ALREADY-RESOLVED era before `protocol_context`
                        // is moved into `handle_call_tool` below, so both create-path
                        // owner bindings on this arm read the SAME ingress value and
                        // neither has to re-parse `params._meta` (Phase 112).
                        #[cfg(not(target_arch = "wasm32"))]
                        let call_tool_era = protocol_context.as_ref().map(|ctx| ctx.era);
                        // Check for task-augmented call: explicit task field or tool requires task
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(ref task_router) = self.task_router {
                            // Determine if this tool requires task augmentation
                            let tool_execution = self
                                .tool_infos
                                .get(&req.name)
                                .and_then(|m| m.execution.as_ref());
                            let needs_task = req.task.is_some() || {
                                let exec_value =
                                    tool_execution.and_then(|e| serde_json::to_value(e).ok());
                                task_router.tool_requires_task(&req.name, exec_value.as_ref())
                            };
                            if needs_task {
                                // A router-minted task is state a LATER request
                                // redeems, so it is bound by the same table that
                                // governs redemption. On v2's refuse row nothing is
                                // minted and the tool never runs (T-114-37); on v1
                                // this is byte-identical to the previous
                                // `.unwrap_or_else(|| "local")`.
                                let crate::server::task_dispatch::OwnerBinding::Owner(owner_id) =
                                    self.resolve_task_owner(auth_context.as_ref(), call_tool_era)
                                else {
                                    return crate::server::task_dispatch::authentication_required(
                                        id,
                                        crate::types::mrtr::CALL_TOOL_METHOD,
                                    );
                                };
                                let task_params =
                                    req.task.clone().unwrap_or_else(|| serde_json::json!({}));
                                #[allow(clippy::used_underscore_binding)]
                                let progress_token = req
                                    ._meta
                                    .as_ref()
                                    .and_then(|m| m.progress_token.as_ref())
                                    .map(|t| serde_json::to_value(t).unwrap());
                                return match task_router
                                    .handle_task_call(
                                        &req.name,
                                        req.arguments.clone(),
                                        task_params,
                                        &owner_id,
                                        progress_token,
                                    )
                                    .await
                                {
                                    Ok(result) => Self::success_response(id, result),
                                    Err(e) => Self::error_response(
                                        id,
                                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                                        e.to_string(),
                                    ),
                                };
                            }
                        }
                        // Normal tool call path (no task augmentation)
                        // Extract continuation context before the handler call
                        #[cfg(not(target_arch = "wasm32"))]
                        #[allow(clippy::used_underscore_binding)]
                        let continuation_ctx = req
                            ._meta
                            .as_ref()
                            .and_then(|m| m._task_id.clone())
                            .map(|task_id| (task_id, req.name.clone()));

                        match self
                            .handle_call_tool(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(outcome) => match outcome {
                                #[cfg(not(target_arch = "wasm32"))]
                                ToolCallOutcome::TaskCreated { task_value } => {
                                    // The shared unit re-extracts task_id + terminal
                                    // result from task_value (single-source create path).
                                    let (response, claim) = self
                                        .build_task_created_response(
                                            id,
                                            task_value,
                                            auth_context.as_ref(),
                                            call_tool_era,
                                        )
                                        .await;
                                    *dispatch_claim = claim;
                                    response
                                },
                                ToolCallOutcome::Result(result) => {
                                    // Fire-and-forget workflow continuation recording
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let (Some((task_id, tool_name)), Some(ref task_router)) =
                                        (continuation_ctx, &self.task_router)
                                    {
                                        // Recording a continuation WRITES to a task
                                        // record, so it needs a bound owner for the
                                        // same reason minting one does. On v2's
                                        // refuse row there is no owner to write
                                        // under, and inventing one would file the
                                        // continuation into a bucket the caller was
                                        // just refused (T-114-37). Skip and warn
                                        // rather than refuse the whole response:
                                        // this path is ALREADY fire-and-forget, the
                                        // tool has already run, and its result is
                                        // returned unchanged below. Unreachable on
                                        // v1, where the table never refuses.
                                        match self.resolve_task_owner(
                                            auth_context.as_ref(),
                                            call_tool_era,
                                        ) {
                                            crate::server::task_dispatch::OwnerBinding::Owner(
                                                owner_id,
                                            ) => {
                                                let tool_result_value =
                                                    serde_json::to_value(&result)
                                                        .unwrap_or_default();
                                                if let Err(e) = task_router
                                                    .handle_workflow_continuation(
                                                        &task_id,
                                                        &tool_name,
                                                        tool_result_value,
                                                        &owner_id,
                                                    )
                                                    .await
                                                {
                                                    tracing::warn!(
                                                        "Workflow continuation recording failed for task {}: {}",
                                                        task_id,
                                                        e
                                                    );
                                                }
                                            },
                                            crate::server::task_dispatch::OwnerBinding::Refused => {
                                                tracing::warn!(
                                                    target: "mcp.tasks",
                                                    task_id = %task_id,
                                                    tool = %tool_name,
                                                    "workflow continuation NOT recorded: an \
                                                     unauthenticated caller on an auth-configured \
                                                     server binds no v2 task owner"
                                                );
                                            },
                                        }
                                    }
                                    Self::success_response(
                                        id,
                                        serde_json::to_value(result).unwrap(),
                                    )
                                },
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListPrompts(req) => match self.handle_list_prompts(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        ),
                    },
                    ClientRequest::GetPrompt(req) => {
                        match self
                            .handle_get_prompt(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListResources(req) => {
                        match self
                            .handle_list_resources(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ReadResource(req) => {
                        match self
                            .handle_read_resource(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListResourceTemplates(req) => {
                        match self.handle_list_resource_templates(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    // Task endpoint routing (TaskStore preferred, TaskRouter
                    // fallback) — delegated to the shared TaskDispatch unit so the
                    // routing logic lives in exactly one place (HTASK-02).
                    #[cfg(not(target_arch = "wasm32"))]
                    request @ (ClientRequest::TasksGet(_)
                    | ClientRequest::TasksResult(_)
                    | ClientRequest::TasksList(_)
                    | ClientRequest::TasksCancel(_)) => {
                        let (response, claim) = self
                            .task_dispatch()
                            .route_tasks_endpoint(
                                id,
                                request,
                                auth_context.as_ref(),
                                // The ALREADY-RESOLVED context, whole: the era
                                // gates and the v2 extension-declaration gate
                                // both read it, and neither re-reads
                                // `params._meta` (twin-site parity with
                                // `server/mod.rs`).
                                protocol_context.as_ref(),
                            )
                            .await;
                        // The claim travels WITH the response: a v2 `tasks/get`
                        // on an `input_required` task owns the top-level
                        // `inputRequests` the reserved-field registry would
                        // otherwise strip (114-10 row 23).
                        *dispatch_claim = claim;
                        response
                    },
                    // `completion/complete` (CONF-05, G-4). Placed HERE, ahead
                    // of the `_ =>` catch-all below, because that catch-all is
                    // what used to answer `-32601 Method not supported` for
                    // this method while the high-level `Server` answered
                    // `json!({})` — two dispatchers, two different wrong
                    // answers. Both now call the SAME shared unit.
                    //
                    // The THREE other methods the twin catch-all in
                    // `server/mod.rs` still covers — `resources/subscribe`,
                    // `resources/unsubscribe` and `ping` — are deliberately NOT
                    // unified here. See the RESIDUAL note on the
                    // `Subscribe | Unsubscribe | Ping` arm in
                    // `Server::process_client_request`. `logging/setLevel` was
                    // the fourth and LEFT that residual in Phase 118.2-08 under
                    // D-13; its own arm is immediately below.
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::Complete(req) => {
                        match complete_completion(self.completions.as_ref(), req).await {
                            Ok(result) => Self::success_response(
                                id,
                                serde_json::to_value(result)
                                    .unwrap_or_else(|_| serde_json::json!({})),
                            ),
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    // `logging/setLevel` (Phase 118.2-08, D-13). Placed HERE,
                    // ahead of the `_ =>` catch-all, for exactly the reason the
                    // `completion/complete` arm above is: that catch-all
                    // answered `-32601` on BOTH eras while the high-level
                    // `Server` answered `json!({})` on both — two dispatchers,
                    // two answers, and only the OTHER one is on the HTTP path
                    // the official conformance suite measures. Both roots now
                    // call the SAME era-branched shared unit, so a caller
                    // reaching this root off-HTTP (in-process, or a future
                    // transport) can no longer get a different answer from the
                    // one the suite measured (T-118.2-08-01).
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::SetLoggingLevel { .. } => {
                        set_logging_level_response(id, protocol_context.as_ref().map(|ctx| ctx.era))
                    },
                    _ => Self::error_response(
                        id,
                        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                        METHOD_NOT_SUPPORTED_MESSAGE.to_string(),
                    ),
                }
            },
            Request::Server(_) => Self::error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                "Method not supported".to_string(),
            ),
        }
    }

    /// Internal notification handler without middleware processing.
    async fn handle_notification_internal(&self, _notification: Notification) -> Result<()> {
        // Handle notifications if needed
        // Most notifications from client to server don't require action
        Ok(())
    }
}

/// Generate a brief text summary of structured output for widget tools.
///
/// When a tool has widget metadata, `structuredContent` carries the full data
/// for the widget. The `content` text should be a concise summary rather than
/// a JSON dump, since `ChatGPT` displays both and duplication is undesirable.
fn summarize_structured_output(value: &Value) -> String {
    match value {
        Value::Array(arr) => format_record_count(arr.len()),
        Value::Object(map) => {
            // Look for common collection patterns inside the object
            // e.g. { "results": [...], "total": 42 } or { "items": [...] }
            for key in ["results", "items", "data", "records", "rows", "entries"] {
                if let Some(Value::Array(arr)) = map.get(key) {
                    return format_record_count(arr.len());
                }
            }
            let field_count = map.len();
            match field_count {
                0 => "Empty result.".to_string(),
                1 => "Result with 1 field.".to_string(),
                n => format!("Result with {n} fields."),
            }
        },
        Value::String(s) => {
            if s.len() <= 200 {
                s.clone()
            } else {
                let truncated: String = s.chars().take(200).collect();
                format!("{truncated}...")
            }
        },
        Value::Null => "No result.".to_string(),
        other => other.to_string(),
    }
}

fn format_record_count(len: usize) -> String {
    match len {
        0 => "No records returned.".to_string(),
        1 => "1 record returned.".to_string(),
        n => format!("{n} records returned."),
    }
}

/// Compute the serialized JSON byte length without allocating.
fn json_serialized_len(value: &impl serde::Serialize) -> Result<usize> {
    struct CountingWriter(usize);
    impl std::io::Write for CountingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = CountingWriter(0);
    serde_json::to_writer(&mut counter, value)
        .map_err(|e| Error::validation(format!("Cannot measure argument size: {e}")))?;
    Ok(counter.0)
}

/// Convert an optional per-request `_meta` into raw JSON for handler surfacing /
/// ingress era-resolution (Phase 112). Centralizes the `RequestMeta -> Value`
/// conversion that every prompt/resource/tool dispatch site would otherwise
/// hand-roll.
pub(crate) fn request_meta_to_value<T: serde::Serialize>(
    meta: Option<&T>,
) -> Option<serde_json::Value> {
    meta.and_then(|m| serde_json::to_value(m).ok())
}

/// Extract the request's `_meta` object as raw JSON for ingress era-resolution
/// (Phase 112, D-11 — the per-request signal is transport-agnostic).
///
/// # Go-forward policy (Phase 112 Plan 09)
///
/// EVERY [`ClientRequest`] variant that carries a per-request
/// `_meta: Option<RequestMeta>` field MUST be read here so its era/identity/trace
/// signal reaches ingress resolution. That is `CallTool`, `GetPrompt`, and
/// `ReadResource` — the three name/uri-bearing methods. Variants with NO `_meta`
/// field yield `None` and resolve to the v1 fallback by design.
///
/// # This is the TYPED extractor, and it is NOT the HTTP path (Phase 113 D-113-B)
///
/// A stateless v2 server runs no `initialize` handshake, so the per-request
/// `_meta` object is the ONLY era channel — which would make every method that
/// lacks a typed `_meta` field un-v2-able if this were the only extractor.
///
/// It is not. The streamable-HTTP transport resolves the era from the RAW request
/// body's `params._meta` via
/// [`Server::resolve_raw_meta_protocol_context`](crate::server::Server::resolve_raw_meta_protocol_context),
/// which works for EVERY method without any public type carrying a field. That
/// route was chosen over widening these structs because adding a `pub` field to a
/// constructible `pub` struct is a MAJOR semver break (`cargo semver-checks`
/// `constructible_struct_adds_field`), and the v2.5 milestone is scoped additive.
///
/// This typed extractor therefore serves only the dispatch surfaces that have NO
/// raw body at their ingress seam — [`Server::handle_request`] for the stdio /
/// WebSocket transports, and `ServerCore`. Both extractors read the SAME spec
/// spelling `_meta` (Phase 113 D-113-A pinned the three structs with
/// `#[serde(rename = "_meta", alias = "meta")]`), so they cannot disagree about
/// what a `_meta` object IS — they differ only in method coverage, and the HTTP
/// path (the one v2 targets) has full coverage.
///
/// The inner match is EXHAUSTIVE with no wildcard arm: a future `ClientRequest`
/// variant is a `non-exhaustive patterns` COMPILE ERROR here, forcing the author
/// to classify it as `_meta`-bearing or not.
#[allow(clippy::used_underscore_binding)] // _meta is part of the MCP protocol spec
pub(crate) fn extract_request_meta_value(request: &Request) -> Option<serde_json::Value> {
    match request {
        Request::Client(boxed) => match boxed.as_ref() {
            // `_meta`-bearing variants — read the per-request signal.
            ClientRequest::CallTool(req) => request_meta_to_value(req._meta.as_ref()),
            ClientRequest::GetPrompt(req) => request_meta_to_value(req._meta.as_ref()),
            ClientRequest::ReadResource(req) => request_meta_to_value(req._meta.as_ref()),
            // Non-`_meta`-bearing variants — enumerated explicitly (no wildcard)
            // so adding a variant forces a decision above rather than silently
            // dropping its signal. On the HTTP path these still reach v2 via the
            // raw-body reader; see the module note above.
            ClientRequest::Initialize(_)
            | ClientRequest::ListTools(_)
            | ClientRequest::ListPrompts(_)
            | ClientRequest::ListResources(_)
            | ClientRequest::ListResourceTemplates(_)
            | ClientRequest::Subscribe(_)
            | ClientRequest::Unsubscribe(_)
            | ClientRequest::Complete(_)
            | ClientRequest::CreateMessage(_)
            | ClientRequest::TasksGet(_)
            | ClientRequest::TasksResult(_)
            | ClientRequest::TasksList(_)
            | ClientRequest::TasksCancel(_)
            | ClientRequest::SetLoggingLevel { .. }
            | ClientRequest::Ping => None,
        },
        Request::Server(_) => None,
    }
}

/// Resolve the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
/// ONCE at native ingress, shared by BOTH dispatch surfaces (`ServerCore` and the
/// high-level `Server`) so the opt-in gate + resolver sequence lives in exactly
/// one place (Pitfall 3 — twin-wiring drift).
///
/// Returns `Ok(None)` immediately for a non-opted-in server so it runs ZERO
/// era-detection and its v1 path is byte-for-byte unchanged (D-04). For an
/// opted-in server it delegates to the single shared
/// [`resolve_protocol_context`](crate::types::protocol::context::resolve_protocol_context),
/// enforcing the configured accept-list against the request's `_meta`.
pub(crate) fn resolve_ingress_protocol_context(
    accept_list: &[crate::types::ProtocolVersion],
    request: &Request,
) -> std::result::Result<
    Option<crate::types::protocol::ProtocolContext>,
    crate::types::protocol::context::ProtocolNegotiationError,
> {
    if !crate::types::protocol::context::is_v2_opted_in(accept_list) {
        return Ok(None);
    }
    let meta = extract_request_meta_value(request);
    crate::types::protocol::context::resolve_protocol_context(accept_list, meta.as_ref())
}

/// Would the v1 handshake capability fold change anything for this context?
///
/// The first thing [`fold_v1_handshake_capabilities`] asks, kept a named
/// function so the rule reads as a table. It runs BEFORE the fold touches the
/// server-level `client_capabilities` `RwLock` (T-118.1-08-02: an unconditional
/// lock read inside a hot dispatch path is a contention cost the fold does not
/// need to pay on v2 traffic).
///
/// | `context` | result | why |
/// |---|---|---|
/// | `None` | `true` | a non-opted-in server resolves NO context at all (D-04), which is the shape G-9 leaves broken |
/// | `Some(era = V1)`, no capabilities | `true` | the v1 fallback context exists but nothing populated the field |
/// | `Some(era = V1)`, capabilities already set | `false` | already answered; never overwrite |
/// | `Some(era = V2)` | `false` | the v2 `_meta` path owns this field (T-118.1-08-03) |
fn v1_capability_fold_applies(context: Option<&crate::types::protocol::ProtocolContext>) -> bool {
    context.is_none_or(|ctx| {
        matches!(ctx.era, crate::types::protocol::Era::V1) && ctx.client_capabilities.is_none()
    })
}

/// Resolve which peer a handler sees, request-scoped first, then the global one.
///
/// The ONE unit both native dispatch roots call — `ServerCore::attach_peer` and
/// `Server::attach_peer`. It exists for the same reason
/// [`fold_v1_handshake_capabilities`] takes the lock rather than an already-read
/// value: the precedence rule is the kind of thing two roots must never disagree
/// about, and a rule spelled twice is kept in step by a comment instead of by the
/// compiler. Both roots previously carried this body verbatim.
///
/// # Precedence, and why this order
///
/// 1. The `TransportBackchannel`'s peer on THIS request's `ProtocolContext` —
///    session-bound, attached by the transport at the one site that knows which
///    session the request arrived on. It must win: a global handle cannot know
///    which session to route a server-to-client request back to.
/// 2. The server-level `peer_handle`, which is what the in-process path uses —
///    `Server::run` attaches no backchannel, so the fallback is the live path
///    there rather than a safety net.
///
/// Returning `extra` unchanged when neither exists leaves `extra.peer()` as
/// `None`, which handlers already treat as "no back-channel on this transport".
///
/// # Ordering
///
/// Every dispatch site calls this AFTER its `tool_authorizer` check, so an
/// unauthorized caller returns before a handler body ever runs and therefore
/// never sees `extra.peer()` — the invariant stated at `src/shared/peer.rs`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn attach_request_peer(
    extra: crate::server::cancellation::RequestHandlerExtra,
    global: Option<&Arc<dyn crate::shared::peer::PeerHandle>>,
) -> crate::server::cancellation::RequestHandlerExtra {
    // Cloned out of the borrow before `with_peer` consumes `extra`.
    let request_scoped = extra
        .protocol_context
        .as_ref()
        .and_then(crate::types::protocol::ProtocolContext::transport_backchannel)
        .and_then(crate::types::protocol::context::TransportBackchannel::peer)
        .cloned();
    if let Some(peer) = request_scoped {
        return extra.with_peer(peer);
    }
    if let Some(peer) = global {
        return extra.with_peer(peer.clone());
    }
    extra
}

/// Resolve which notification sink a handler's `extra.log(..)` emits through,
/// and apply the log level this request resolved — request-scoped first, then
/// the root's fallback.
///
/// The TWIN of [`attach_request_peer`], and one unit for the same reason: the
/// precedence rule is the kind of thing two roots must never disagree about, and
/// a rule spelled twice is kept in step by a comment instead of by the compiler.
/// Phase 118.1 collapsed exactly this class of divergence for
/// `completion/complete`, where two dispatchers gave two DIFFERENT wrong answers.
///
/// # Precedence, and why this order
///
/// 1. The `TransportBackchannel`'s `notification_sink` on THIS request's
///    `ProtocolContext` — session-bound, attached by the transport at the one
///    site that knows which session the request arrived on. It must win:
///    routing a session's log records onto a server-wide channel would publish
///    one caller's records to whoever is reading that channel
///    (T-118.2-06-02).
/// 2. The root's `fallback`, which on `Server` is derived from its single
///    server-wide `notification_tx` and on [`ServerCore`] is always `None` —
///    `ServerCore` has no notification channel of any kind.
///
/// The fallback is taken as a THUNK, not as a value: arm 1 wins on every
/// HTTP-served request, so an eagerly-built `Server` fallback allocated one
/// `Arc<dyn Fn(..)>` per dispatch that this function then dropped unread. Passing
/// it lazily keeps that allocation off the hot path without moving the
/// precedence rule back out to the two call sites.
///
/// Returning `extra` unchanged when neither exists leaves `extra.log_sink` as
/// `None`, which the emitter already treats as silence rather than as an error
/// (Phase 118.2 D-08).
///
/// # On v2, an ABSENT level is a prohibition (SEP-2575)
///
/// The 2026-07-28 client authorizes logging per request through
/// `params._meta["io.modelcontextprotocol/logLevel"]`, and the schema is
/// explicit: *"If absent, the server MUST NOT send any notifications/message"*.
/// `None` therefore means two different things on the two eras — on v1 it is "the
/// client did not choose, so the server MAY decide", which is what
/// [`DEFAULT_LOG_LEVEL`](crate::server::cancellation::DEFAULT_LOG_LEVEL) (`info`,
/// D-12) implements, and on v2 it is "do not log".
///
/// Expressed by withholding the SINK rather than by inventing a third level
/// value. The emitter already treats a sinkless `extra` as silence (D-08), so
/// this needs no new state on `RequestHandlerExtra`, no tri-state on
/// `ProtocolContext::resolved_log_level`, and no change to the emitter — and it
/// is fail-closed: a handler that logs anyway emits nothing, rather than relying
/// on every handler to check a flag. `examples/s54_v2_dual_conformance.rs`
/// carried exactly such a per-handler guard while this gap was open; it is gone.
///
/// # Why the LEVEL is applied here too
///
/// The sink and the level are two halves of ONE per-request decision. Attaching
/// them at different sites is how one root ends up filtering differently from
/// the other, so the same read of `extra.protocol_context` that resolves the
/// sink also lifts
/// [`ProtocolContext::resolved_log_level`](crate::types::protocol::ProtocolContext::resolved_log_level)
/// onto the `extra`. When the context carries no level the `extra` is left
/// alone, so the emitter's
/// [`DEFAULT_LOG_LEVEL`](crate::server::cancellation::DEFAULT_LOG_LEVEL) applies.
///
/// Doing the lift once, here, also makes the emit-time filter an O(1) read of a
/// plain `Option<LoggingLevel>` on the `extra`, instead of a lock acquisition per
/// record.
///
/// # Ordering
///
/// Every dispatch site calls this AFTER its `tool_authorizer` check, so an
/// unauthorized caller returns before a handler body ever runs and therefore
/// never reaches a handler that could emit (ASVS V4, T-118.2-06-01) — the same
/// invariant [`attach_request_peer`] states for `extra.peer()`.
///
/// # The progress token does NOT gate this
///
/// `Server::progress_reporter_for` still returns `None` for a request with no
/// `params._meta.progressToken`, and that gate is deliberate. The LOG sink is
/// unconditional: a client that never asked for progress still receives
/// `notifications/message` (Phase 118.2 D-07).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn attach_request_log_sink(
    extra: crate::server::cancellation::RequestHandlerExtra,
    fallback: impl FnOnce() -> Option<Arc<dyn Fn(crate::types::Notification) + Send + Sync>>,
) -> crate::server::cancellation::RequestHandlerExtra {
    // Both values are cloned out of the borrow before the `with_*` builders
    // consume `extra` — the same capture-before-move discipline
    // `attach_request_peer` uses, and the reason this reads the sink back OFF
    // the already-constructed `extra` rather than capturing it before
    // `with_protocol_context` moved the context in.
    let request_scoped = extra
        .protocol_context
        .as_ref()
        .and_then(crate::types::protocol::ProtocolContext::transport_backchannel)
        .and_then(crate::types::protocol::context::TransportBackchannel::notification_sink)
        .cloned();
    let resolved_level = extra
        .protocol_context
        .as_ref()
        .and_then(crate::types::protocol::ProtocolContext::resolved_log_level);
    // SEP-2575: on v2, absence is a prohibition rather than a non-answer. See
    // this function's rustdoc for why the sink is withheld instead of a third
    // level value being invented.
    let v2_logging_unauthorized = resolved_level.is_none()
        && extra
            .protocol_context
            .as_ref()
            .is_some_and(|context| context.era == crate::types::protocol::Era::V2);

    let extra = match resolved_level {
        Some(level) => extra.with_log_level(level),
        // No resolved level: leave `extra.log_level` alone so `DEFAULT_LOG_LEVEL`
        // applies at emit time.
        None => extra,
    };

    if v2_logging_unauthorized {
        // No sink at all, so `extra.log(..)` is silent by construction. Returned
        // BEFORE both precedence arms, so neither the request-scoped sink nor the
        // root fallback can reintroduce a vehicle the client did not authorize.
        return extra;
    }
    if let Some(sink) = request_scoped {
        return extra.with_log_sink(sink);
    }
    // Only NOW is the root's fallback built — see the precedence note above.
    if let Some(sink) = fallback() {
        return extra.with_log_sink(sink);
    }
    extra
}

/// Fold the v1 `initialize` handshake's advertised capabilities into the
/// `ProtocolContext` threaded into dispatch (Phase 118.1-08, G-9 / CONF-08).
///
/// ONE shared unit, called from BOTH native dispatch roots — `ServerCore`'s
/// `ProtocolHandler::handle_request` and `Server::handle_request_with_context` —
/// per the twin-site parity rule this file follows everywhere else. Nine copies
/// of a lock read (one per `RequestHandlerExtra` construction site) is exactly
/// the drift that rule exists to prevent, so the fold happens ONCE per dispatch
/// at the root and every downstream site simply receives the already-folded
/// context.
///
/// It takes the `RwLock` rather than an already-read value deliberately: a
/// signature that took `Option<ClientCapabilities>` would leave the guard-then-
/// read half — the part that actually encodes T-118.1-08-02 — copy-pasted at
/// both roots, kept in step by a comment instead of by the compiler. That is
/// the same shape [`MrtrRound`] was introduced to kill for this very pair of
/// sites. Both `ServerCore` and `Server` hold this field as
/// `Arc<RwLock<Option<ClientCapabilities>>>`, so one signature serves both.
///
/// # The THIRD native dispatch root, and why it is exempt
///
/// `Server::handle_tasks_update` is reached directly from the HTTP transport,
/// bypassing `handle_request_with_context` and therefore this fold. That is
/// correct, not an omission: `route_tasks_update` refuses at its first case via
/// [`is_v1_task_era`](crate::server::task_dispatch::is_v1_task_era), which is
/// `true` for exactly the `Some(Era::V1)` / `None` inputs
/// [`v1_capability_fold_applies`] fires on — so a fold there would be dead by
/// construction. A future v1-serving capability read on the tasks surface would
/// need one.
///
/// # The two endpoints this bridges
///
/// The v1 handshake writes `*self.client_capabilities.write().await =
/// Some(init_req.capabilities.clone())` — a SERVER-LEVEL lock. A handler reads
/// [`RequestHandlerExtra::client_capabilities`](crate::RequestHandlerExtra::client_capabilities),
/// which reads only `protocol_context.client_capabilities`, a field
/// `resolve_protocol_context` populates ONLY from the v2 `_meta` reserved key.
/// Without this fold the two never meet and a v1 capability gate reads
/// permanently `None`.
///
/// # The fold is ONE-DIRECTIONAL
///
/// It fires only when [`v1_capability_fold_applies`] holds — that is, on a v1
/// era or no context at all, and only when the field is still empty. A fresh
/// per-request v2 `_meta` declaration therefore always wins over a stale
/// handshake value (T-118.1-08-03).
///
/// # It never fabricates a default
///
/// `handshake_capabilities` of `None` returns the context unchanged. A server
/// that never saw an `initialize` keeps `client_capabilities() == None` — and
/// keeps `era() == None` too, since no context is synthesised either. Inventing
/// `ClientCapabilities::default()` where nothing was advertised would make a
/// capability gate silently permissive, which is worse than the bug
/// (T-118.1-08-04).
///
/// # Why the accessor does NOT read the lock instead
///
/// `client_capabilities()` is a SYNC method on a value that has already been
/// moved into the handler, and the dispatch path holds the lock — that shape
/// deadlocks. The read happens here, once, at the dispatch root, and the guard
/// is dropped at the end of the `let` below — before the caller's
/// `handle_initialize` takes the WRITE lock.
///
/// **Security — self-reported, not for authorization:** like
/// [`client_info`](crate::RequestHandlerExtra::client_info), these capabilities
/// are client-supplied and informational ONLY. They MUST NOT be used as an
/// authorization anchor; real identity binds to the OAuth token (Phase 114 /
/// TASK-05). G-9 widens WHO can read these values — it does not make them
/// trustworthy.
pub(crate) async fn fold_v1_handshake_capabilities(
    context: Option<crate::types::protocol::ProtocolContext>,
    handshake_lock: &RwLock<Option<crate::types::ClientCapabilities>>,
    accept_list: &[crate::types::ProtocolVersion],
) -> Option<crate::types::protocol::ProtocolContext> {
    if !v1_capability_fold_applies(context.as_ref()) {
        return context;
    }
    let Some(capabilities) = handshake_lock.read().await.clone() else {
        return context;
    };
    match context {
        Some(ctx) => Some(ctx.with_client_capabilities(capabilities)),
        // No context resolved (a non-opted-in server, D-04). The handshake DID
        // happen — the lock holds a value — so the era is known to be v1 and the
        // context is synthesised rather than left absent. The version comes from
        // the SHARED absent-signal rule, so it names what the resolver would
        // have named had the server been opted in.
        None => crate::types::protocol::context::first_v1_version(accept_list).map(|version| {
            crate::types::protocol::ProtocolContext::new(crate::types::protocol::Era::V1, version)
                .with_client_capabilities(capabilities)
        }),
    }
}

/// The pure initialize-gate rule: must THIS request have been preceded by an
/// `initialize` handshake?
///
/// | `stateless_mode` | `era`           | result  | why |
/// |------------------|-----------------|---------|-----|
/// | `false`          | `Some(Era::V2)` | `false` | v2 is handshake-free by design (HTTP-01) |
/// | `false`          | `Some(Era::V1)` | `true`  | v1 lifecycle is untouched |
/// | `false`          | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `true`           | anything        | `false` | an explicitly stateless server never had a handshake to demand |
///
/// # Why the era clause exists (Finding 11 / HTTP-01)
///
/// The gate this predicate governs emits
/// [`V1_TASK_PENDING`](crate::types::protocol::error_codes::V1_TASK_PENDING)
/// (`-32002`), which protocol version 2026-07-28 **MUST NOT** emit
/// (`docs/specification/draft/basic/index.mdx` § Error Codes). That site was
/// commented as v1-scoped but had never had its v2 reachability traced;
/// `tests/v2_prohibited_error_codes.rs` traced it BY EXECUTION and found it
/// reachable, because [`ProtocolHandler`] is a PUBLIC trait and
/// `ServerCore::handle_request` is not behind the streamable-HTTP transport whose
/// era gating Phase 113 built.
///
/// The right answer is not a different error code. A v2 request carries no
/// `initialize` handshake at all (HTTP-01), so demanding one before serving it is
/// simply the wrong rule for that era — a different constant would still be
/// refusing a conformant request.
///
/// `stateless_mode` is NOT an era decision: `ServerCoreBuilder::build` resolves it
/// as `self.stateless_mode.unwrap_or_else(Self::detect_stateless_environment)`,
/// i.e. by ENVIRONMENT auto-detection. That is exactly why it could not be relied
/// on to keep `-32002` off the v2 wire, and why the era is a separate clause.
///
/// Split out as a named predicate — the shape
/// [`sessions_active_for`](crate::server::streamable_http_server) established for
/// the session decision — so a third caller cannot re-derive the rule
/// differently.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const fn v1_initialize_gate_applies(
    stateless_mode: bool,
    era: Option<crate::types::protocol::Era>,
) -> bool {
    !stateless_mode && !matches!(era, Some(crate::types::protocol::Era::V2))
}

/// Map a [`ProtocolNegotiationError`](crate::types::protocol::context::ProtocolNegotiationError)
/// to a structured JSON-RPC rejection `(code, message)`.
///
/// Both variants surface as `INVALID_PARAMS` (-32602): a bad/unsupported
/// per-request `protocolVersion` or a malformed reserved `_meta` key is an
/// invalid method parameter. (v2 semantic error-code values are finalized from
/// the 2026-07-28 schema; VERS-06.)
pub(crate) fn negotiation_error_to_rejection(
    error: &crate::types::protocol::context::ProtocolNegotiationError,
) -> (i32, String) {
    use crate::types::protocol::context::ProtocolNegotiationError;
    use crate::types::protocol::error_codes::INVALID_PARAMS;
    match error {
        ProtocolNegotiationError::UnsupportedVersion(v) => {
            (INVALID_PARAMS, format!("Unsupported protocol version: {v}"))
        },
        ProtocolNegotiationError::MalformedMeta(reason) => {
            (INVALID_PARAMS, format!("Malformed _meta: {reason}"))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::tool_middleware::ToolMiddlewareChain;
    use crate::types::ClientCapabilities;

    struct TestTool;

    #[async_trait]
    impl ToolHandler for TestTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }

    /// Build `tool_infos` cache from a tools `HashMap` (mirrors builder logic).
    fn build_tool_infos(
        tools: &HashMap<String, Arc<dyn ToolHandler>>,
    ) -> HashMap<String, ToolInfo> {
        tools
            .iter()
            .map(|(name, handler)| {
                let mut info = handler
                    .metadata()
                    .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
                info.name.clone_from(name);
                (name.clone(), info)
            })
            .collect()
    }

    #[tokio::test]
    async fn test_server_core_initialization() {
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode
            PayloadLimits::default(),
        );

        assert!(!server.is_initialized().await);

        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })));

        let response = server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

        match response.payload {
            ResponsePayload::Result(_) => {
                assert!(server.is_initialized().await);
            },
            ResponsePayload::Error(e) => panic!("Initialization failed: {}", e.message),
        }
    }

    #[tokio::test]
    async fn test_server_core_list_tools() {
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode
            PayloadLimits::default(),
        );

        // Initialize first
        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })));
        server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

        // List tools
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(2i64), list_req, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(e) => panic!("List tools failed: {}", e.message),
        }
    }

    struct EraProbeTool;

    #[async_trait]
    impl ToolHandler for EraProbeTool {
        async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
            // Prove the ingress-resolved context is visible IN the handler.
            let era = extra.era().map(|e| format!("{e:?}"));
            let traceparent = extra.trace_context().map(|tc| tc.traceparent);
            Ok(serde_json::json!({ "era": era, "traceparent": traceparent }))
        }
    }

    /// Extract the probe tool's JSON payload from a wrapped `CallToolResult`
    /// (the dispatcher emits the handler value as text content).
    fn probe_payload(result: &Value) -> Value {
        let text = result["content"][0]["text"]
            .as_str()
            .expect("probe result carries text content");
        serde_json::from_str(text).expect("probe text content is JSON")
    }

    fn probe_call_with_v2_meta() -> Request {
        use crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;
        let meta = crate::types::protocol::RequestMeta::new()
            .with_meta(
                RESERVED_PROTOCOL_VERSION_KEY,
                serde_json::json!("2026-07-28"),
            )
            .with_meta(
                "traceparent",
                serde_json::json!("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
            );
        Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "probe".to_string(),
            arguments: serde_json::json!({}),
            _meta: Some(meta),
            task: None,
        })))
    }

    /// End-to-end: a v2 `_meta` + `traceparent` presented at ingress is resolved
    /// once and visible in the invoked handler via `extra.era()` /
    /// `extra.trace_context()` (Codex MEDIUM — ingress→handler threading proven).
    #[tokio::test]
    async fn test_v2_meta_visible_in_handler_end_to_end() {
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;

        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("probe-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(7i64), probe_call_with_v2_meta(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                let probe = probe_payload(&result);
                assert_eq!(probe["era"], "V2");
                assert_eq!(
                    probe["traceparent"],
                    "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
                );
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    /// A non-opted-in (default v1-only) server runs ZERO era-detection: even a v2
    /// `_meta` signal resolves to no context, so the handler reads `era()==None`
    /// (D-04, byte-for-byte-unchanged v1 path).
    #[tokio::test]
    async fn test_non_opted_in_server_resolves_no_context() {
        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("v1-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(8i64), probe_call_with_v2_meta(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                let probe = probe_payload(&result);
                assert_eq!(probe["era"], serde_json::Value::Null);
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    /// An explicitly-unsupported per-request version is rejected with a structured
    /// error rather than silently served (accept-list enforcement, Codex HIGH #2).
    #[tokio::test]
    async fn test_unsupported_version_rejected_at_ingress() {
        use crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;

        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("probe-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_supported_protocol_versions([ProtocolVersion(
                PROTOCOL_VERSION_2026_07_28.to_string(),
            )])
            .build()
            .unwrap();

        let meta = crate::types::protocol::RequestMeta::new().with_meta(
            RESERVED_PROTOCOL_VERSION_KEY,
            serde_json::json!("1999-01-01"),
        );
        let call = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "probe".to_string(),
            arguments: serde_json::json!({}),
            _meta: Some(meta),
            task: None,
        })));
        let response = server
            .handle_request(RequestId::from(9i64), call, None)
            .await;
        match response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, crate::types::protocol::error_codes::INVALID_PARAMS);
            },
            ResponsePayload::Result(_) => {
                panic!("unsupported version must be rejected, not served")
            },
        }
    }

    // ---- Phase 112 Plan 05: server/discover (VERS-04) ----

    fn v2_ctx() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(
            crate::types::protocol::Era::V2,
            ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
        )
    }

    fn v1_ctx() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(
            crate::types::protocol::Era::V1,
            ProtocolVersion("2025-11-25".to_string()),
        )
    }

    /// Build a v2-opted-in server carrying a `.with_extension`-populated key.
    fn discover_server() -> ServerCore {
        crate::server::builder::ServerCoreBuilder::new()
            .name("discover-server")
            .version("9.9.9")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_extension(
                "io.example/experimental",
                serde_json::json!({ "enabled": true }),
            )
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap()
    }

    /// A v2 `server/discover` projects the already-computed capabilities INCLUDING
    /// the `.with_extension`-populated extensions map, and carries serverInfo.
    #[test]
    fn server_discover_v2_projects_capabilities_with_extensions() {
        let server = discover_server();
        // The wire method still classifies as the internal (non-public-enum) request.
        let internal = crate::types::protocol::classify_internal_method(
            "server/discover",
            &serde_json::json!({}),
        )
        .expect("server/discover classifies as internal");
        assert!(matches!(
            internal,
            crate::types::protocol::InternalClientRequest::ServerDiscover(_)
        ));
        let ctx = v2_ctx();
        // Projection is produced by the ONE shared free fn the production caller uses.
        let response = build_discover_response(
            RequestId::from(1i64),
            DiscoverSource::from(&server.capabilities),
            &server.info,
            Some(&ctx),
        );

        let ResponsePayload::Result(value) = response.payload else {
            panic!("v2 server/discover must return a result");
        };
        // extensions map projected
        assert_eq!(
            value["capabilities"]["extensions"]["io.example/experimental"]["enabled"],
            serde_json::json!(true)
        );
        // serverInfo present
        assert_eq!(value["serverInfo"]["name"], "discover-server");
        assert_eq!(value["serverInfo"]["version"], "9.9.9");
        // negotiated version reflected
        assert_eq!(value["protocolVersion"], "2026-07-28");
    }

    /// A v1 / non-opted-in `server/discover` receives standard -32601 (D-10).
    #[test]
    fn server_discover_v1_returns_method_not_found() {
        let server = discover_server();

        // v1 era context
        let ctx = v1_ctx();
        let resp = build_discover_response(
            RequestId::from(2i64),
            DiscoverSource::from(&server.capabilities),
            &server.info,
            Some(&ctx),
        );
        let ResponsePayload::Error(e) = resp.payload else {
            panic!("v1 server/discover must be an error");
        };
        assert_eq!(
            e.code,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND
        );
        assert_eq!(e.code, -32601);

        // no resolved context at all → also -32601
        let resp_none = build_discover_response(
            RequestId::from(3i64),
            DiscoverSource::from(&server.capabilities),
            &server.info,
            None,
        );
        let ResponsePayload::Error(e2) = resp_none.payload else {
            panic!("context-less server/discover must be an error");
        };
        assert_eq!(e2.code, -32601);
    }

    /// The public `parse_request` maps `server/discover` to -32601 (v1 for free)
    /// — proving the interception seam preserves the v1 wire behavior.
    #[test]
    fn server_discover_public_parse_is_method_not_found() {
        let req = crate::types::JSONRPCRequest::new(
            RequestId::from(1i64),
            "server/discover".to_string(),
            Some(serde_json::json!({})),
        );
        let err = crate::shared::parse_request(req).unwrap_err();
        assert!(err.to_string().contains("Method not found"));
    }

    /// `server/discover` does NOT mutate initialization state (read-only, no
    /// initialize-style side effect).
    #[tokio::test]
    async fn server_discover_does_not_mutate_init_state() {
        // Non-stateless server so `is_initialized()` is meaningful.
        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("discover-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .with_supported_protocol_versions([ProtocolVersion(
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            )])
            .build()
            .unwrap();

        assert!(!server.is_initialized().await);
        let ctx = v2_ctx();
        let _ = build_discover_response(
            RequestId::from(1i64),
            DiscoverSource::from(&server.capabilities),
            &server.info,
            Some(&ctx),
        );
        assert!(
            !server.is_initialized().await,
            "server/discover must not flip initialization state"
        );
    }

    /// Golden fixture: pin the discover wire shape so a change is caught.
    #[test]
    fn server_discover_wire_shape_golden() {
        let caps = ServerCapabilities::tools_only();
        let info = Implementation::new("golden-server", "1.2.3");
        // A REAL two-entry accept-list, so the golden pins that
        // `supportedVersions` reproduces the caller's list in order rather than
        // some derived or defaulted value (G-7).
        let versions = vec![
            ProtocolVersion(crate::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
        ];
        let result = discover_result_from_capabilities(
            DiscoverSource::new(&caps, &versions),
            &info,
            "2026-07-28".to_string(),
        );
        let value = serde_json::to_value(&result).unwrap();
        let expected = serde_json::json!({
            "protocolVersion": "2026-07-28",
            "capabilities": { "tools": { "listChanged": true } },
            "serverInfo": { "name": "golden-server", "version": "1.2.3" },
            "supportedVersions": ["2025-11-25", "2026-07-28"]
        });
        assert_eq!(value, expected, "discover wire shape drifted from golden");
    }

    // ---- Plan 114-05 (TASK-01, D-02): the per-era capability projection ----

    /// A tasks-backed server's capabilities as the build-time rule leaves them:
    /// the v1 `tasks` capability, the v2 extensions entry, an `experimental`
    /// map carrying BOTH a `tasks` flag and an unrelated one.
    fn tasks_backed_capabilities() -> ServerCapabilities {
        let mut caps = ServerCapabilities::default();
        crate::server::task_dispatch::apply_tasks_capability_rule(&mut caps, &HashMap::new(), true)
            .unwrap();
        let mut experimental = HashMap::new();
        experimental.insert("tasks".to_string(), serde_json::json!({ "legacy": true }));
        experimental.insert("io.example/flag".to_string(), serde_json::json!(true));
        caps.experimental = Some(experimental);
        caps
    }

    /// The v2 projection shows the extension and hides BOTH v1 spellings.
    #[test]
    fn server_discover_projects_the_tasks_extension_and_hides_the_v1_tasks_keys() {
        let info = Implementation::new("tasks-server", "1.0.0");
        // Capabilities-only source: this test reads nothing but the projected
        // capabilities, which is exactly what that conversion is for.
        let result = discover_result_from_capabilities(
            DiscoverSource::from(&tasks_backed_capabilities()),
            &info,
            "2026-07-28".to_string(),
        );
        let value = serde_json::to_value(&result).unwrap();
        let caps = &value["capabilities"];

        assert_eq!(
            caps["extensions"][crate::types::capabilities::TASKS_EXTENSION_KEY],
            serde_json::json!({}),
            "v2 discover must advertise the tasks extension as the empty object: {value}"
        );
        // Key ABSENCE, never a falsy value: `skip_serializing_if` keeps `None`
        // off the wire entirely, so accepting `null` here would pass on a change
        // that started emitting an explicit null.
        assert!(
            caps.get("tasks").is_none(),
            "the v1 `tasks` capability must be ABSENT from a v2 discover: {value}"
        );
        assert!(
            caps["experimental"].get("tasks").is_none(),
            "the v1 `experimental.tasks` flag must be ABSENT from a v2 discover: {value}"
        );
    }

    /// A non-tasks `experimental` key SURVIVES the v2 projection.
    ///
    /// D-02 rejected suppressing the whole `experimental` block; this phase owns
    /// exactly one key in it.
    #[test]
    fn server_discover_preserves_unrelated_experimental_keys() {
        let info = Implementation::new("tasks-server", "1.0.0");
        // Capabilities-only source: this test reads nothing but the projected
        // capabilities, which is exactly what that conversion is for.
        let result = discover_result_from_capabilities(
            DiscoverSource::from(&tasks_backed_capabilities()),
            &info,
            "2026-07-28".to_string(),
        );
        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(
            value["capabilities"]["experimental"]["io.example/flag"],
            serde_json::json!(true),
            "an unrelated experimental key must survive the v2 projection: {value}"
        );
    }

    /// The v1 `initialize` view drops the AUTO-ADVERTISED tasks extension.
    ///
    /// The mirror of the v2 row above, and the unit-level guard for the leak
    /// `tests/v2_tasks_negotiation.rs::v1_initialize_stays_byte_identical`
    /// measured on the real wire.
    #[test]
    fn v1_projection_drops_the_auto_advertised_tasks_extension_from_capabilities() {
        let mut caps = ServerCapabilities::default();
        crate::server::task_dispatch::apply_tasks_capability_rule(&mut caps, &HashMap::new(), true)
            .unwrap();
        assert!(
            caps.extensions.is_some(),
            "precondition: the build-time rule must have created the entry"
        );

        let projected = project_capabilities_for_v1(&caps);
        let value = serde_json::to_value(&projected).unwrap();

        assert!(
            value.get("extensions").is_none(),
            "a map holding nothing but the auto-advertised entry must be dropped \
             entirely — `\"extensions\":{{}}` would itself be the v1 byte change: {value}"
        );
        assert!(
            projected.tasks.is_some(),
            "and the v1 negotiation home stays exactly as before: {value}"
        );
        assert!(
            caps.extensions.is_some(),
            "the projection must not mutate the stored capabilities"
        );
    }

    /// An OPERATOR-configured (non-empty) value under the same key survives the
    /// v1 projection.
    ///
    /// Only what pmcp auto-added is removed. Silently deleting an operator's own
    /// configuration from the wire is the mirror-image failure of silently
    /// overwriting it.
    #[test]
    fn v1_projection_preserves_an_operator_configured_tasks_extension_in_capabilities() {
        let configured = serde_json::json!({ "io.example/nonconformant": true });
        let mut caps = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(
            crate::types::capabilities::TASKS_EXTENSION_KEY.to_string(),
            configured.clone(),
        );
        caps.extensions = Some(extensions);

        let projected = project_capabilities_for_v1(&caps);

        assert_eq!(
            projected
                .extensions
                .as_ref()
                .and_then(|map| map.get(crate::types::capabilities::TASKS_EXTENSION_KEY)),
            Some(&configured),
            "an operator-authored value must survive the v1 projection verbatim"
        );
    }

    /// Unrelated extensions keys survive, and the map is kept when they do.
    #[test]
    fn v1_projection_leaves_unrelated_extensions_capabilities_intact() {
        let mut caps = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(
            crate::types::capabilities::TASKS_EXTENSION_KEY.to_string(),
            crate::server::task_dispatch::tasks_extension_value(),
        );
        extensions.insert("io.example/skills".to_string(), serde_json::json!({}));
        caps.extensions = Some(extensions);

        let projected = project_capabilities_for_v1(&caps);
        let extensions = projected.extensions.as_ref().expect("map is kept");

        assert!(
            extensions.contains_key("io.example/skills"),
            "an unrelated extensions key must survive: {extensions:?}"
        );
        assert!(
            !extensions.contains_key(crate::types::capabilities::TASKS_EXTENSION_KEY),
            "and only the auto-advertised tasks entry is removed: {extensions:?}"
        );
    }

    /// Two successive projections of the SAME capabilities yield identical
    /// output, and the source struct is unchanged after both.
    ///
    /// The regression guard for a projection that mutated stored state: under
    /// that bug the first v2 `server/discover` would permanently change what a
    /// subsequent v1 `initialize` client sees.
    #[test]
    fn server_discover_projection_never_mutates_the_stored_capabilities() {
        let caps = tasks_backed_capabilities();
        let before = serde_json::to_value(&caps).unwrap();
        let info = Implementation::new("tasks-server", "1.0.0");

        let first = serde_json::to_value(discover_result_from_capabilities(
            DiscoverSource::from(&caps),
            &info,
            "2026-07-28".to_string(),
        ))
        .unwrap();
        let second = serde_json::to_value(discover_result_from_capabilities(
            DiscoverSource::from(&caps),
            &info,
            "2026-07-28".to_string(),
        ))
        .unwrap();

        assert_eq!(
            first, second,
            "two projections of one server must be identical — no accumulated mutation"
        );
        assert_eq!(
            serde_json::to_value(&caps).unwrap(),
            before,
            "the projection must leave the server's OWN capabilities untouched: \
             they are per-server, the projection is per-request-era"
        );
        assert!(
            caps.tasks.is_some(),
            "specifically, the v1 tasks capability must still be there for the \
             next v1 initialize client"
        );
    }

    // ---- Phase 112 Plan 05: resultType + serverInfo envelope (VERS-07) ----
    //
    // Its OWN module so `cargo test -- inject_v2_result_envelope` selects
    // exactly this suite (including plan 09's reserved-field ownership and the
    // `serverInfo` relocation) rather than matching nothing; the surrounding
    // `ServerCore` fixtures are reached through `use super::*`.
    mod inject_v2_result_envelope {
        use super::*;

        fn result_response(id: i64, result: Value) -> JSONRPCResponse {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::from(id),
                payload: ResponsePayload::Result(result),
            }
        }

        /// Read `serverInfo` from wherever the v2 envelope puts it.
        ///
        /// ONE reader, so the tests that care about OWNERSHIP stay independent of
        /// the tests that pin PLACEMENT — the latter index the nesting directly and
        /// are the ones that must fail if the placement drifts.
        fn server_info_of(result: &Value) -> &Value {
            &result["_meta"][RESERVED_SERVER_INFO_KEY]
        }

        /// A v2 OBJECT success result gains inner-result `resultType:"complete"` and
        /// a `serverInfo` object.
        #[test]
        fn result_type_envelope_v2_object_gets_complete_and_server_info() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(1, serde_json::json!({ "tools": [] }));
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v["resultType"], "complete");
            assert_eq!(server_info_of(&v)["name"], "srv");
            assert_eq!(server_info_of(&v)["version"], "2.0.0");
        }

        /// `resultType` is SERVER-OWNED: a handler-set value is OVERWRITTEN with the
        /// disposition the server computed (Codex Plan-09 HIGH #3).
        ///
        /// Phase 112 preserved the handler's value. That let a handler write
        /// `resultType: "input_required"` on a method the spec forbids it on and
        /// sail straight past the eligibility tripwire, because the tripwire gates
        /// what the SERVER emits, not what a handler smuggles into its own result.
        #[test]
        fn result_type_envelope_overwrites_handler_disposition() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(1, serde_json::json!({ "resultType": "task", "x": 1 }));
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["resultType"], "complete",
                "the server-computed disposition must win"
            );
            assert_eq!(v["x"], 1, "non-reserved handler keys survive untouched");
            // Non-default dispositions round-trip through the wire discriminator.
            assert_eq!(
                ResponseDisposition::InputRequired.as_wire_str(),
                "input_required"
            );
            assert_eq!(ResponseDisposition::Task.as_wire_str(), "task");
        }

        /// The forged-`input_required`-on-`tools/list` scenario, end to end at the
        /// envelope: a handler writing the reserved key itself gets `"complete"`.
        #[test]
        fn handler_forged_input_required_is_overwritten_to_complete() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            // The shape a malicious/confused `tools/list` handler would return.
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "tools": [],
                    "resultType": "input_required",
                    "requestState": "forged-token",
                    "inputRequests": { "x": { "method": "roots/list" } },
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v["resultType"], "complete");
            assert!(
                v.get("requestState").is_none(),
                "a handler-supplied requestState must be removed: {v}"
            );
            assert!(
                v.get("inputRequests").is_none(),
                "a handler-supplied inputRequests must be removed: {v}"
            );
            assert!(v["tools"].is_array(), "the real payload survives");
        }

        /// An `input_required` result KEEPS the MRTR fields, because that egress
        /// minted them — the removal is scoped to results the server did not mint.
        #[test]
        fn input_required_disposition_keeps_the_minted_mrtr_fields() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "content": [],
                    "requestState": "minted-token",
                    "inputRequests": { "x": { "method": "roots/list" } },
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::InputRequired,
                ReservedFieldOwner::Mrtr,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v["resultType"], "input_required");
            assert_eq!(v["requestState"], "minted-token");
            assert!(v["inputRequests"]["x"].is_object());
        }

        /// A handler-set `io.modelcontextprotocol/serverInfo` is OVERWRITTEN with
        /// the server's real `Implementation` — server identity is not a handler
        /// claim (T-113-59).
        #[test]
        fn handler_supplied_server_info_is_overwritten() {
            let info = Implementation::new("real-server", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "_meta": {
                        RESERVED_SERVER_INFO_KEY: { "name": "impersonated", "version": "0.0.0" },
                    },
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(server_info_of(&v)["name"], "real-server");
            assert_eq!(server_info_of(&v)["version"], "2.0.0");
        }

        // ---- Phase 113 Plan 09 Task 3: serverInfo lives inside result._meta ----

        /// The schema places server identity at
        /// `result._meta["io.modelcontextprotocol/serverInfo"]`, and the top-level
        /// key the envelope used to write is GONE (RESEARCH Pitfall 6).
        #[test]
        fn server_info_lives_inside_result_meta_not_at_the_top_level() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(1, serde_json::json!({ "tools": [] }));
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v["_meta"][RESERVED_SERVER_INFO_KEY]["name"], "srv");
            assert_eq!(v["_meta"][RESERVED_SERVER_INFO_KEY]["version"], "2.0.0");
            assert!(
                v.get("serverInfo").is_none(),
                "the envelope must no longer write a top-level serverInfo: {v}"
            );
            assert_eq!(
                RESERVED_SERVER_INFO_KEY,
                "io.modelcontextprotocol/serverInfo"
            );
        }

        /// `_meta` is CREATED when the handler set none, and MERGED into (never
        /// wholesale replaced) when it did.
        #[test]
        fn server_info_merges_into_an_existing_meta() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();

            // Created from nothing.
            let mut created = result_response(1, serde_json::json!({}));
            inject_v2_result_envelope(
                &mut created,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = created.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["_meta"].as_object().expect("an object").len(),
                1,
                "a created _meta carries only the reserved key: {v}"
            );

            // Merged into an existing one.
            let mut merged = result_response(
                2,
                serde_json::json!({ "_meta": { "vendor/key": 1, "io.example/trace": "abc" } }),
            );
            inject_v2_result_envelope(
                &mut merged,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = merged.payload else {
                panic!("expected result");
            };
            assert_eq!(v["_meta"]["vendor/key"], 1);
            assert_eq!(v["_meta"]["io.example/trace"], "abc");
            assert_eq!(v["_meta"][RESERVED_SERVER_INFO_KEY]["name"], "srv");
        }

        /// A v1 / non-opted-in response gains NO `_meta` — the creation is strictly
        /// inside the v2 gate.
        #[test]
        fn v1_gains_no_meta_from_the_envelope() {
            let info = Implementation::new("srv", "2.0.0");
            for ctx in [Some(v1_ctx()), None] {
                let original = serde_json::json!({ "tools": [] });
                let mut resp = result_response(1, original.clone());
                inject_v2_result_envelope(
                    &mut resp,
                    ctx.as_ref(),
                    &info,
                    ResponseDisposition::Complete,
                    ReservedFieldOwner::None,
                    Cacheable::No,
                );
                let ResponsePayload::Result(v) = resp.payload else {
                    panic!("expected result");
                };
                assert_eq!(v, original, "v1 must gain no _meta and stay byte-identical");
            }
        }

        /// The ONE envelope path means `server/discover`, `tools/call`,
        /// `prompts/get`, `resources/read` and an `input_required` result all carry
        /// the reserved key identically. `server/discover` additionally keeps its
        /// OWN top-level `serverInfo` schema field, which the registry deliberately
        /// does not own.
        #[test]
        fn every_v2_result_shape_carries_server_info_identically() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let shapes = [
                ("tools/call", serde_json::json!({ "content": [] })),
                (
                    "prompts/get",
                    serde_json::json!({ "description": "d", "messages": [] }),
                ),
                ("resources/read", serde_json::json!({ "contents": [] })),
            ];
            for (label, shape) in shapes {
                let mut resp = result_response(1, shape);
                inject_v2_result_envelope(
                    &mut resp,
                    Some(&ctx),
                    &info,
                    ResponseDisposition::Complete,
                    ReservedFieldOwner::None,
                    Cacheable::No,
                );
                let ResponsePayload::Result(v) = resp.payload else {
                    panic!("expected result");
                };
                assert_eq!(
                    v["_meta"][RESERVED_SERVER_INFO_KEY]["name"], "srv",
                    "{label}"
                );
                assert!(v.get("serverInfo").is_none(), "{label}: {v}");
            }

            // An input_required result travels the same path.
            let mut input_required = result_response(
                2,
                serde_json::json!({ "content": [], "requestState": "t", "inputRequests": {} }),
            );
            inject_v2_result_envelope(
                &mut input_required,
                Some(&ctx),
                &info,
                ResponseDisposition::InputRequired,
                ReservedFieldOwner::Mrtr,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = input_required.payload else {
                panic!("expected result");
            };
            assert_eq!(v["resultType"], "input_required");
            assert_eq!(v["_meta"][RESERVED_SERVER_INFO_KEY]["name"], "srv");

            // `server/discover` keeps its own schema field AND gains the reserved one.
            let server = discover_server();
            let discover_ctx = v2_ctx();
            let response = build_discover_response(
                RequestId::from(3i64),
                DiscoverSource::from(&server.capabilities),
                &server.info,
                Some(&discover_ctx),
            );
            let ResponsePayload::Result(v) = response.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["serverInfo"]["name"], "discover-server",
                "ServerDiscoverResult's OWN serverInfo field is not server-owned and survives"
            );
            assert_eq!(
                v["_meta"][RESERVED_SERVER_INFO_KEY]["name"],
                "discover-server"
            );
        }

        /// Non-reserved handler `_meta` keys SURVIVE alongside the server's
        /// ownership pass — ownership is scoped to the enumerated set only.
        #[test]
        fn non_reserved_handler_meta_survives() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({ "_meta": { "vendor/key": 1, "io.example/trace": "abc" } }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v["_meta"]["vendor/key"], 1);
            assert_eq!(v["_meta"]["io.example/trace"], "abc");
        }

        /// The pmcp-internal signal key is removed at the ENVELOPE too, not only in
        /// `mrtr_egress` — which is `streamable-http`-only, so on a build without
        /// that feature nothing else would strip it (T-113-60).
        #[test]
        fn envelope_removes_the_internal_signal_key_defense_in_depth() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "_meta": { crate::types::mrtr::MRTR_SIGNAL_META_KEY: { "continuation": 1 } },
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            let rendered = v.to_string();
            assert!(
                !rendered.contains(crate::types::mrtr::MRTR_SIGNAL_META_KEY),
                "the internal signal leaked through the envelope: {rendered}"
            );
            // What remains is exactly the reserved key the envelope owns — the
            // handler's signal left no residue.
            assert_eq!(v["_meta"].as_object().expect("an object").len(), 1);
            assert!(v["_meta"][RESERVED_SERVER_INFO_KEY].is_object());
        }

        /// A handler that set `_meta` to a NON-object gets it replaced with an
        /// object rather than the server dropping its reserved keys.
        #[test]
        fn non_object_handler_meta_is_replaced_with_an_object() {
            let mut result = serde_json::json!({ "_meta": "not-an-object" });
            let meta = result_meta_object_mut(&mut result).expect("an object result");
            meta.insert("vendor/key".to_string(), serde_json::json!(1));
            assert_eq!(result["_meta"]["vendor/key"], 1);
            assert!(result["_meta"].is_object());
        }

        /// `result_meta_object_mut` reports `None` for a non-object RESULT — there
        /// is nowhere to put a `_meta` on a scalar.
        #[test]
        fn result_meta_object_mut_declines_a_non_object_result() {
            let mut scalar = serde_json::json!(42);
            assert!(result_meta_object_mut(&mut scalar).is_none());
            let mut null = Value::Null;
            assert!(result_meta_object_mut(&mut null).is_none());
        }

        /// A v2 scalar/null result is left unchanged (cannot key a non-object), and
        /// error responses get no injection.
        #[test]
        fn result_type_envelope_non_object_and_error_untouched() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();

            // scalar
            let mut scalar = result_response(1, serde_json::json!(42));
            inject_v2_result_envelope(
                &mut scalar,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = scalar.payload else {
                panic!("expected result");
            };
            assert_eq!(v, serde_json::json!(42));

            // null
            let mut null = result_response(2, Value::Null);
            inject_v2_result_envelope(
                &mut null,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = null.payload else {
                panic!("expected result");
            };
            assert_eq!(v, Value::Null);

            // error → no injection
            let mut err =
                ServerCore::error_response(RequestId::from(3i64), -32601, "nope".to_string());
            inject_v2_result_envelope(
                &mut err,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            assert!(matches!(err.payload, ResponsePayload::Error(_)));
        }

        /// Golden byte-identity: a v1 (or non-opted-in) response is UNCHANGED — no
        /// resultType, no serverInfo — for both a success and an error.
        #[test]
        fn result_type_envelope_v1_byte_identical_golden() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v1_ctx();

            // v1 success — byte-identical
            let original = serde_json::json!({ "tools": [], "nextCursor": null });
            let mut resp = result_response(1, original.clone());
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(v, original, "v1 success must stay byte-identical");

            // No context at all — also byte-identical.
            let mut resp_none = result_response(2, original.clone());
            inject_v2_result_envelope(
                &mut resp_none,
                None,
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let ResponsePayload::Result(v2) = resp_none.payload else {
                panic!("expected result");
            };
            assert_eq!(v2, original);

            // v1 error/task-pending — byte-identical (frozen -32002 survives).
            let mut err = ServerCore::error_response(
                RequestId::from(3i64),
                -32002,
                "Task not completed".to_string(),
            );
            let before = serde_json::to_value(&err).unwrap();
            inject_v2_result_envelope(
                &mut err,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::No,
            );
            let after = serde_json::to_value(&err).unwrap();
            assert_eq!(before, after, "v1 error must stay byte-identical");
        }

        /// End-to-end through `handle_request`: a v2 tools/list carries the envelope.
        #[tokio::test]
        async fn result_type_envelope_end_to_end_v2_handle_request() {
            let server = discover_server();
            // `probe_call_with_v2_meta` carries the v2 `_meta` so ingress resolves
            // Era::V2; the tool result is a JSON object → gains the envelope.
            let response = server
                .handle_request(RequestId::from(1i64), probe_call_with_v2_meta(), None)
                .await;
            let ResponsePayload::Result(v) = response.payload else {
                panic!("expected result");
            };
            assert_eq!(v["resultType"], "complete");
            assert_eq!(server_info_of(&v)["name"], "discover-server");
        }

        // ---- Phase 115 Plan 06: the caching-hint projection (SCHM-03) ----
        //
        // These cover the CHOKEPOINT: that it delegates to the projector, on
        // the right eras, for the right cacheability claims. The PROJECTOR's
        // own semantics are covered by `crate::types::caching`'s
        // `projection_tests` module and are deliberately not duplicated here.

        /// A v2 `CacheableResult` with no handler intent gains BOTH hints, at
        /// the safe defaults (D-08).
        #[test]
        fn v2_cacheable_result_gains_both_hints_with_the_safe_defaults() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(1, serde_json::json!({ "tools": [] }));
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["ttlMs"],
                serde_json::json!(crate::types::DEFAULT_TTL_MS),
                "the v2 wire REQUIRES ttlMs; the default 0 means `immediately stale`, \
                 which asserts nothing about cacheability. Got {v}"
            );
            assert_eq!(
                v["cacheScope"],
                serde_json::json!("private"),
                "D-08: an un-considered response must default to `private`. Defaulting to \
                 `public` would be a cross-authorization-context data leak — a shared \
                 gateway would be authorized to serve one caller's response body to \
                 another caller holding a different access token. Got {v}"
            );
            assert_eq!(v["tools"], serde_json::json!([]), "the payload survives");
        }

        /// A handler that DID express intent keeps it, byte for byte. This is
        /// what makes 115-05's `with_ttl_ms` / `with_cache_scope` meaningful.
        #[test]
        fn v2_handler_set_hints_survive_the_projection_unmodified() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({ "tools": [], "ttlMs": 300_000, "cacheScope": "public" }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["ttlMs"],
                serde_json::json!(300_000),
                "a handler-set ttlMs must survive verbatim — the projection ENSURES, it \
                 does not overwrite. Got {v}"
            );
            assert_eq!(
                v["cacheScope"],
                serde_json::json!("public"),
                "a handler-set cacheScope must survive verbatim, got {v}"
            );
        }

        /// `tools/call` and `tasks/update` are not `CacheableResult` extenders,
        /// so a v2 response for them gains NEITHER key (D-07).
        #[test]
        fn v2_non_cacheable_result_gains_neither_hint() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            for shape in [
                serde_json::json!({ "content": [] }), // tools/call
                serde_json::json!({}),                // tasks/update ack
            ] {
                let mut resp = result_response(1, shape);
                inject_v2_result_envelope(
                    &mut resp,
                    Some(&ctx),
                    &info,
                    ResponseDisposition::Complete,
                    ReservedFieldOwner::None,
                    Cacheable::No,
                );
                let ResponsePayload::Result(v) = resp.payload else {
                    panic!("expected result");
                };
                assert!(
                    v.get("ttlMs").is_none() && v.get("cacheScope").is_none(),
                    "D-07: only the six CacheableResult extenders carry these keys, got {v}"
                );
                // The v2 envelope itself still applies — the two are independent.
                assert_eq!(v["resultType"], "complete");
            }
        }

        /// A non-`Complete` disposition suppresses the hints even when the
        /// REQUEST was one of the cacheable methods (D-07).
        ///
        /// `cacheable` is derived from the request METHOD, so it describes that
        /// method's COMPLETE result type. A non-`Complete` disposition means the
        /// body on the wire is a DIFFERENT type: an `InputRequiredResult`
        /// (Phase 113 MRTR) or a task handle (Phase 114), and in the vendored
        /// `2026-07-28` schema both `extends Result`, not `CacheableResult`.
        ///
        /// `resources/read` is what makes this reachable rather than theoretical:
        /// it is the ONLY method that is simultaneously MRTR-eligible
        /// (`client_request_mrtr_eligible`) and `Cacheable::Yes`
        /// (`request_is_cacheable`), and `MRTR_SIGNAL_META_KEY` is `pub`, so any
        /// `ResourceHandler` can put its v2 `resources/read` on this path.
        #[test]
        fn a_non_complete_disposition_suppresses_the_hints_even_when_cacheable() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            for (disposition, owner, wire) in [
                (
                    ResponseDisposition::InputRequired,
                    ReservedFieldOwner::Mrtr,
                    "input_required",
                ),
                (ResponseDisposition::Task, ReservedFieldOwner::None, "task"),
            ] {
                let mut resp = result_response(1, serde_json::json!({ "contents": [] }));
                inject_v2_result_envelope(
                    &mut resp,
                    Some(&ctx),
                    &info,
                    disposition,
                    owner,
                    // The claim `request_is_cacheable(resources/read)` produces.
                    Cacheable::Yes,
                );
                let ResponsePayload::Result(v) = resp.payload else {
                    panic!("expected result");
                };
                assert_eq!(
                    v["resultType"], wire,
                    "the disposition must still reach the wire, got {v}"
                );
                assert!(
                    v.get("ttlMs").is_none() && v.get("cacheScope").is_none(),
                    "a `{wire}` body is NOT a CacheableResult extender, so it must carry \
                     neither hint however the REQUEST was classified. Got {v}"
                );
            }
        }

        /// The plain D-11 case: a v1 response gains neither key even for a
        /// method whose v2 result WOULD carry them.
        #[test]
        fn v1_cacheable_result_gains_neither_hint() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v1_ctx();
            let original = serde_json::json!({ "tools": [], "nextCursor": null });
            let mut resp = result_response(1, original.clone());
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v, original,
                "a v1 response must stay byte-identical: no envelope key AND no caching hint"
            );
        }

        /// The leak an ENSURE-ONLY projection would ship: a handler set the
        /// hints and the client is v1.
        #[test]
        fn v1_strips_a_handler_set_hint() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v1_ctx();
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "resources": [],
                    "nextCursor": null,
                    "ttlMs": 300_000,
                    "cacheScope": "public",
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert!(
                v.get("ttlMs").is_none(),
                "D-11: a v1 wire must NEVER carry a v2 field. An ensure-only projection \
                 would have left this handler-set ttlMs in place. Got {v}"
            );
            assert!(
                v.get("cacheScope").is_none(),
                "D-11: a v1 wire must NEVER carry a v2 field; cacheScope leaked. Got {v}"
            );
            assert_eq!(
                v,
                serde_json::json!({ "resources": [], "nextCursor": null }),
                "the strip must disturb nothing else"
            );
        }

        /// No resolved context is treated as v1 — the conservative unknown⇒V1
        /// fallback, and the EXACT combination `WasmMcpServer` passes.
        #[test]
        fn no_protocol_context_is_treated_as_v1() {
            let info = Implementation::new("srv", "2.0.0");
            let mut resp = result_response(
                1,
                serde_json::json!({
                    "contents": [],
                    "ttlMs": 300_000,
                    "cacheScope": "public",
                }),
            );
            inject_v2_result_envelope(
                &mut resp,
                None,
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v,
                serde_json::json!({ "contents": [] }),
                "an era-less dispatcher (WasmMcpServer passes exactly this) must STRIP \
                 both keys — D-11. Got {v}"
            );
        }

        /// Errors are untouched on BOTH eras even with a cacheable claim.
        #[test]
        fn an_error_payload_is_untouched_on_both_eras() {
            let info = Implementation::new("srv", "2.0.0");
            for ctx in [Some(v2_ctx()), Some(v1_ctx()), None] {
                let mut err = ServerCore::error_response(
                    RequestId::from(3i64),
                    -32002,
                    "Task not completed".to_string(),
                );
                let before = serde_json::to_value(&err).unwrap();
                inject_v2_result_envelope(
                    &mut err,
                    ctx.as_ref(),
                    &info,
                    ResponseDisposition::Complete,
                    ReservedFieldOwner::None,
                    Cacheable::Yes,
                );
                let after = serde_json::to_value(&err).unwrap();
                assert_eq!(
                    before, after,
                    "an error payload carries no result body, so it can carry no hint"
                );
            }
        }

        /// A non-object result body cannot carry a key.
        #[test]
        fn a_non_object_result_is_untouched() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            for shape in [
                serde_json::json!(42),
                Value::Null,
                serde_json::json!([1, 2, 3]),
                serde_json::json!("a string"),
            ] {
                let mut resp = result_response(1, shape.clone());
                inject_v2_result_envelope(
                    &mut resp,
                    Some(&ctx),
                    &info,
                    ResponseDisposition::Complete,
                    ReservedFieldOwner::None,
                    Cacheable::Yes,
                );
                let ResponsePayload::Result(v) = resp.payload else {
                    panic!("expected result");
                };
                assert_eq!(v, shape, "a non-object result body must be left alone");
            }
        }

        /// The injected scope is the SERIALIZATION of the enum default, never a
        /// string literal — so the projection and the enum cannot drift.
        ///
        /// If someone changes `#[default]` to `Public`,
        /// `v2_cacheable_result_gains_both_hints_with_the_safe_defaults` fails
        /// (it names the concrete safe value) and this one keeps the two
        /// consistent, so the pair localizes the change rather than hiding it.
        #[test]
        fn the_injected_scope_is_the_serialization_of_the_enum_default() {
            let info = Implementation::new("srv", "2.0.0");
            let ctx = v2_ctx();
            let mut resp = result_response(1, serde_json::json!({ "prompts": [] }));
            inject_v2_result_envelope(
                &mut resp,
                Some(&ctx),
                &info,
                ResponseDisposition::Complete,
                ReservedFieldOwner::None,
                Cacheable::Yes,
            );
            let ResponsePayload::Result(v) = resp.payload else {
                panic!("expected result");
            };
            assert_eq!(
                v["cacheScope"],
                serde_json::to_value(crate::types::CacheScope::default()).unwrap(),
                "the injected default must BE the enum's serialization, not a parallel \
                 string literal that can drift from it"
            );
            assert_eq!(
                v["ttlMs"],
                serde_json::to_value(crate::types::DEFAULT_TTL_MS).unwrap(),
                "same for the ttl: the constant is the single source"
            );
        }

        /// A response middleware that deletes a projected key WINS.
        ///
        /// # This test asserts a LIMITATION, not an endorsement
        ///
        /// Measured ordering: [`ServerCore::handle_request`] calls
        /// `inject_v2_result_envelope` and THEN
        /// `process_response_with_context(&mut response, &context)`, whose
        /// signature in `src/shared/middleware.rs` takes
        /// `response: &mut JSONRPCResponse`. A registered response middleware
        /// therefore runs AFTER the projection and can add, alter or remove
        /// `ttlMs`, `cacheScope`, `resultType` or `serverInfo`.
        ///
        /// This test asserts the CURRENT behaviour so that a future change of
        /// ordering surfaces as a deliberate decision rather than a silent
        /// alteration of what the SDK puts on the v2 wire.
        ///
        /// The prohibition documented on `inject_v2_result_envelope` stands:
        /// **response middleware MUST NOT mutate `ttlMs`, `cacheScope`,
        /// `resultType` or `serverInfo`.** A middleware that needs to influence
        /// cacheability must set the fields on the result TYPE before dispatch
        /// returns.
        ///
        /// Moving the projection AFTER the middleware chain was considered and
        /// NOT done: it would change what middleware observes about Phase 114's
        /// `resultType` / `serverInfo`, which is a v2 behaviour change outside
        /// SCHM-03's scope. It is fenced by a source tripwire in 115-08 and
        /// booked as a deferred item by 115-10.
        #[tokio::test]
        async fn response_middleware_still_runs_after_the_projection_and_this_is_a_known_limitation(
        ) {
            use crate::shared::middleware::{
                AdvancedMiddleware, EnhancedMiddlewareChain, MiddlewareContext,
            };

            /// Deletes one projected key from every result it sees.
            struct KeyDeletingMiddleware {
                key: &'static str,
            }

            #[async_trait]
            impl AdvancedMiddleware for KeyDeletingMiddleware {
                fn name(&self) -> &'static str {
                    "key-deleting-probe"
                }

                async fn on_response_with_context(
                    &self,
                    response: &mut JSONRPCResponse,
                    _context: &MiddlewareContext,
                ) -> Result<()> {
                    if let ResponsePayload::Result(ref mut value) = response.payload {
                        if let Some(object) = value.as_object_mut() {
                            object.remove(self.key);
                        }
                    }
                    Ok(())
                }
            }

            struct HintingResource;

            #[async_trait]
            impl ResourceHandler for HintingResource {
                async fn read(
                    &self,
                    uri: &str,
                    _extra: RequestHandlerExtra,
                ) -> Result<ReadResourceResult> {
                    Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                        uri,
                        "hi",
                        "text/plain",
                    )]))
                }

                async fn list(
                    &self,
                    _cursor: Option<String>,
                    _extra: RequestHandlerExtra,
                ) -> Result<ListResourcesResult> {
                    Ok(ListResourcesResult::new(vec![]))
                }
            }

            async fn read_with_middleware(deleted_key: &'static str) -> Value {
                let mut chain = EnhancedMiddlewareChain::new();
                chain.add(Arc::new(KeyDeletingMiddleware { key: deleted_key }));
                let server = crate::server::builder::ServerCoreBuilder::new()
                    .name("middleware-ordering-probe")
                    .version("1.0.0")
                    .resources(HintingResource)
                    .stateless_mode(true)
                    .protocol_middleware(Arc::new(RwLock::new(chain)))
                    .with_supported_protocol_versions([ProtocolVersion(
                        crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
                    )])
                    .build()
                    .unwrap();

                let meta = crate::types::protocol::RequestMeta::new().with_meta(
                    crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY,
                    serde_json::json!("2026-07-28"),
                );
                let request =
                    Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
                        uri: "mem://x".to_string(),
                        _meta: Some(meta),
                    })));
                let response = server
                    .handle_request(RequestId::from(1i64), request, None)
                    .await;
                let ResponsePayload::Result(v) = response.payload else {
                    panic!("expected a result");
                };
                v
            }

            // The middleware deleted `ttlMs` AFTER the projection wrote it.
            let v = read_with_middleware("ttlMs").await;
            assert!(
                v.get("ttlMs").is_none(),
                "KNOWN LIMITATION, asserted deliberately: inject_v2_result_envelope runs \
                 BEFORE process_response_with_context, which takes `&mut JSONRPCResponse`, \
                 so response middleware WINS over the projection. Response middleware \
                 MUST NOT mutate ttlMs, cacheScope, resultType or serverInfo — a \
                 middleware that needs to influence cacheability must set the fields on \
                 the result TYPE before dispatch returns. If this assertion now FAILS, \
                 the ordering changed: that is a v2 wire-behaviour change and must be a \
                 deliberate decision (115-08 tripwire, 115-10 deferred item), not a \
                 silent one. Got {v}"
            );
            assert_eq!(
                v["cacheScope"],
                serde_json::json!("private"),
                "the key the middleware did NOT touch still carries the projection, which \
                 is what makes this a measurement of ORDERING rather than of the \
                 projection being absent. Got {v}"
            );

            // The same limitation holds for the OTHER projected key — the
            // measurement is of ordering, not of one specific key.
            let v = read_with_middleware("cacheScope").await;
            assert!(
                v.get("cacheScope").is_none(),
                "the ordering limitation is per-response, not per-key: \
                 process_response_with_context can remove cacheScope just as readily as \
                 ttlMs. Got {v}"
            );
            assert_eq!(
                v["ttlMs"],
                serde_json::json!(crate::types::DEFAULT_TTL_MS),
                "and the untouched key still carries the projection, got {v}"
            );
        }
    }

    #[tokio::test]
    async fn test_stateless_mode_allows_requests_without_init() {
        // Create server in stateless mode
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None, // task_router
            None, // task_store
            true, // stateless_mode enabled
            PayloadLimits::default(),
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), list_req, None)
            .await;

        // Should succeed in stateless mode
        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(e) => panic!(
                "List tools should succeed in stateless mode without init: {}",
                e.message
            ),
        }
    }

    #[tokio::test]
    async fn test_normal_mode_requires_initialization() {
        // Create server in normal mode (stateless_mode = false)
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode disabled (normal mode)
            PayloadLimits::default(),
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), list_req, None)
            .await;

        // Should fail in normal mode
        match response.payload {
            ResponsePayload::Result(_) => {
                panic!("List tools should fail in normal mode without initialization")
            },
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32002);
                assert!(e.message.contains("not initialized"));
            },
        }
    }

    #[test]
    fn test_build_uri_to_tool_meta_indexes_by_standard_key() {
        // Create a tool with openai/* keys (propagation-eligible)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        meta.insert(
            "openai/outputTemplate".to_string(),
            serde_json::json!("ui://chess/board"),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        // Should index by the standard ui.resourceUri key
        assert!(
            index.contains_key("ui://chess/board"),
            "must index by ui.resourceUri value"
        );
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_build_uri_to_tool_meta_includes_openai_when_present() {
        // Create a tool with both standard and openai keys (ChatGpt layer was applied)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        meta.insert(
            "openai/outputTemplate".to_string(),
            serde_json::json!("ui://chess/board"),
        );
        meta.insert(
            "openai/widgetAccessible".to_string(),
            serde_json::json!(true),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        assert!(index.contains_key("ui://chess/board"));
        let entry = &index["ui://chess/board"];
        // Should include the openai keys in the indexed meta
        assert!(
            entry.contains_key("openai/outputTemplate"),
            "must include openai/outputTemplate in index entry"
        );
        assert!(
            entry.contains_key("openai/widgetAccessible"),
            "must include openai/widgetAccessible in index entry"
        );
    }

    #[test]
    fn test_build_uri_to_tool_meta_skips_empty_propagation() {
        // Create a tool with standard-only _meta (no openai/* keys to propagate)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        // Should NOT index when there are no propagation-eligible keys,
        // to avoid producing _meta: {} on resources/list
        assert!(
            !index.contains_key("ui://chess/board"),
            "must not index tools with no propagation-eligible keys"
        );
    }

    #[test]
    fn test_summarize_array() {
        let empty = serde_json::json!([]);
        assert_eq!(summarize_structured_output(&empty), "No records returned.");

        let single = serde_json::json!([{"id": 1}]);
        assert_eq!(summarize_structured_output(&single), "1 record returned.");

        let multi = serde_json::json!([1, 2, 3, 4, 5]);
        assert_eq!(summarize_structured_output(&multi), "5 records returned.");
    }

    #[test]
    fn test_summarize_object_with_collection() {
        let val = serde_json::json!({"results": [1, 2, 3], "total": 3});
        assert_eq!(summarize_structured_output(&val), "3 records returned.");

        let val = serde_json::json!({"items": [], "page": 1});
        assert_eq!(summarize_structured_output(&val), "No records returned.");

        let val = serde_json::json!({"data": [{"name": "a"}]});
        assert_eq!(summarize_structured_output(&val), "1 record returned.");
    }

    #[test]
    fn test_summarize_plain_object() {
        let val = serde_json::json!({"name": "test", "value": 42});
        assert_eq!(summarize_structured_output(&val), "Result with 2 fields.");

        let val = serde_json::json!({});
        assert_eq!(summarize_structured_output(&val), "Empty result.");
    }

    #[test]
    fn test_summarize_primitives() {
        assert_eq!(summarize_structured_output(&Value::Null), "No result.");
        assert_eq!(
            summarize_structured_output(&serde_json::json!("hello")),
            "hello"
        );
        assert_eq!(summarize_structured_output(&serde_json::json!(42)), "42");
    }

    #[test]
    fn test_summarize_string_truncation_multibyte() {
        // Multi-byte chars: each emoji is 4 bytes, 201 of them = 804 bytes
        let long_emoji = "\u{1F600}".repeat(201);
        let result = summarize_structured_output(&Value::String(long_emoji));
        assert!(result.ends_with("..."));
        // Should not panic and should truncate at char boundary
        assert!(result.len() > 3);
    }

    // -----------------------------------------------------------------------
    // Phase 112-09 (Gap B): per-request `_meta`/`ProtocolContext` spine wired for
    // GetPrompt + ReadResource, not only CallTool.
    // -----------------------------------------------------------------------
    mod phase_112_09_context_spine {
        use super::*;
        use crate::types::protocol::{Era, ProtocolContext, RequestMeta};
        use std::sync::Mutex;

        fn get_prompt_request(name: &str, meta: Option<RequestMeta>) -> Request {
            Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
                name: name.to_string(),
                arguments: HashMap::new(),
                _meta: meta,
            })))
        }

        fn read_resource_request(uri: &str, meta: Option<RequestMeta>) -> Request {
            Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
                uri: uri.to_string(),
                _meta: meta,
            })))
        }

        #[test]
        fn extract_request_meta_value_reads_prompt_and_resource_meta() {
            let meta = RequestMeta::new().with_meta("ns/key", serde_json::json!("v"));
            let expected = serde_json::to_value(&meta).unwrap();

            // GetPrompt with _meta → Some(json) equal to to_value(meta).
            let got = extract_request_meta_value(&get_prompt_request("p", Some(meta.clone())));
            assert_eq!(got, Some(expected.clone()));

            // ReadResource with _meta → Some(json) equal to to_value(meta).
            let got = extract_request_meta_value(&read_resource_request("mem://x", Some(meta)));
            assert_eq!(got, Some(expected));

            // _meta == None → None (v1 fallback preserved) for both methods.
            assert_eq!(
                extract_request_meta_value(&get_prompt_request("p", None)),
                None
            );
            assert_eq!(
                extract_request_meta_value(&read_resource_request("mem://x", None)),
                None
            );
        }

        #[test]
        fn all_meta_bearing_client_requests_are_extracted() {
            // Positive coverage for the three `_meta`-bearing variants. The real
            // drift guard is the WILDCARD-FREE exhaustive match in
            // extract_request_meta_value: a new variant is a compile error there,
            // not a silent `None`, so this test need not enumerate the enum.
            let meta = RequestMeta::new().with_meta("io.example/x", serde_json::json!(1));
            let expected = serde_json::to_value(&meta).unwrap();

            let mut call_tool_req = CallToolRequest::new("t", serde_json::json!({}));
            call_tool_req._meta = Some(meta.clone());
            let call_tool = Request::Client(Box::new(ClientRequest::CallTool(call_tool_req)));
            let get_prompt = get_prompt_request("p", Some(meta.clone()));
            let read_resource = read_resource_request("mem://x", Some(meta));

            for req in [&call_tool, &get_prompt, &read_resource] {
                assert_eq!(
                    extract_request_meta_value(req),
                    Some(expected.clone()),
                    "every _meta-bearing ClientRequest variant must extract Some"
                );
            }
        }

        /// The TYPED extractor deliberately covers only the three `_meta`-bearing
        /// methods, and a list-shaped method yields `None` here.
        ///
        /// That is NOT a v2 gap: the streamable-HTTP transport reads the era from
        /// the RAW body's `params._meta` instead (D-113-B resolution — widening
        /// these `pub` structs would have been a MAJOR semver break). This test
        /// pins the boundary so a future reader does not mistake the `None` for a
        /// defect and "fix" it back into a breaking change.
        /// `tests/v2_stateless_http.rs` proves the HTTP path serves these methods
        /// as v2.
        #[test]
        fn typed_extractor_scope_is_the_three_meta_bearing_methods() {
            for method in [
                "tools/list",
                "prompts/list",
                "resources/list",
                "resources/templates/list",
            ] {
                let client: ClientRequest = serde_json::from_value(serde_json::json!({
                    "method": method,
                    "params": { "_meta": { "ns/key": "v" } },
                }))
                .unwrap_or_else(|e| panic!("{method} must deserialize: {e}"));
                let req = Request::Client(Box::new(client));
                assert_eq!(
                    extract_request_meta_value(&req),
                    None,
                    "{method} has no typed _meta field; the HTTP path reads the raw body"
                );
            }
        }

        /// The three name-bearing methods must surface a SPEC-SPELLED `_meta`
        /// arriving on the wire (D-113-A). Before Phase 113 the typed structs
        /// renamed the field to `meta`, so a conformant client was never detected
        /// as v2 at all.
        #[test]
        fn spec_spelled_meta_on_the_wire_reaches_era_resolution() {
            let expected = serde_json::json!({ "ns/key": "v" });
            for (method, params) in [
                (
                    "tools/call",
                    serde_json::json!({ "name": "t", "arguments": {}, "_meta": { "ns/key": "v" } }),
                ),
                (
                    "prompts/get",
                    serde_json::json!({ "name": "p", "arguments": {}, "_meta": { "ns/key": "v" } }),
                ),
                (
                    "resources/read",
                    serde_json::json!({ "uri": "mem://x", "_meta": { "ns/key": "v" } }),
                ),
            ] {
                let client: ClientRequest = serde_json::from_value(serde_json::json!({
                    "method": method,
                    "params": params,
                }))
                .unwrap_or_else(|e| panic!("{method} must deserialize: {e}"));
                let req = Request::Client(Box::new(client));
                assert_eq!(
                    extract_request_meta_value(&req),
                    Some(expected.clone()),
                    "{method} must read the SPEC-spelled `_meta`, not `meta`"
                );
            }
        }

        proptest::proptest! {
            #[test]
            fn extract_request_meta_value_fuzz_never_panics(
                key in "[a-zA-Z0-9._/-]{0,64}",
                strval in ".{0,4096}",
                use_prompt in proptest::prelude::any::<bool>(),
            ) {
                // Arbitrary namespaced key + oversized string value on a RequestMeta
                // set on GetPrompt or ReadResource. extract must round-trip to the
                // SAME serde_json::Value and never panic.
                let meta = RequestMeta::new().with_meta(key, serde_json::json!(strval));
                let expected = serde_json::to_value(&meta).unwrap();
                let req = if use_prompt {
                    get_prompt_request("p", Some(meta))
                } else {
                    read_resource_request("mem://x", Some(meta))
                };
                proptest::prop_assert_eq!(extract_request_meta_value(&req), Some(expected));
            }
        }

        // Capturing handlers record the RequestHandlerExtra signals the REAL
        // dispatch entrypoint threaded into them.
        #[derive(Clone, Debug, Default, PartialEq)]
        struct Captured {
            era: Option<Era>,
            has_client_info: bool,
            traceparent: Option<String>,
        }

        struct CapturingPrompt(Arc<Mutex<Option<Captured>>>);

        #[async_trait]
        impl PromptHandler for CapturingPrompt {
            async fn handle(
                &self,
                _args: HashMap<String, String>,
                extra: RequestHandlerExtra,
            ) -> Result<GetPromptResult> {
                *self.0.lock().unwrap() = Some(Captured {
                    era: extra.era(),
                    has_client_info: extra.client_info().is_some(),
                    traceparent: extra.trace_context().map(|t| t.traceparent),
                });
                Ok(GetPromptResult::new(vec![], None))
            }
        }

        struct CapturingResource(Arc<Mutex<Option<Captured>>>);

        #[async_trait]
        impl ResourceHandler for CapturingResource {
            async fn read(
                &self,
                _uri: &str,
                extra: RequestHandlerExtra,
            ) -> Result<ReadResourceResult> {
                *self.0.lock().unwrap() = Some(Captured {
                    era: extra.era(),
                    has_client_info: extra.client_info().is_some(),
                    traceparent: extra.trace_context().map(|t| t.traceparent),
                });
                Ok(ReadResourceResult::new(vec![Content::text("ok")]))
            }

            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<ListResourcesResult> {
                Ok(ListResourcesResult {
                    resources: vec![],
                    next_cursor: None,
                    ttl_ms: None,
                    cache_scope: None,
                })
            }
        }

        fn build_core(
            prompt_cap: Arc<Mutex<Option<Captured>>>,
            resource_cap: Arc<Mutex<Option<Captured>>>,
        ) -> ServerCore {
            let mut prompts: HashMap<String, Arc<dyn PromptHandler>> = HashMap::new();
            prompts.insert(
                "greeting".to_string(),
                Arc::new(CapturingPrompt(prompt_cap)) as Arc<dyn PromptHandler>,
            );
            let resources: Option<Arc<dyn ResourceHandler>> =
                Some(Arc::new(CapturingResource(resource_cap)));

            ServerCore::new(
                Implementation::new("test-server", "1.0.0"),
                ServerCapabilities::default(),
                HashMap::new(),
                prompts,
                HashMap::new(),
                HashMap::new(),
                resources,
                None,
                None,
                None,
                Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
                Arc::new(RwLock::new(ToolMiddlewareChain::new())),
                None, // task_router
                None, // task_store
                true, // stateless_mode — skip the initialize gate
                PayloadLimits::default(),
            )
            .with_supported_protocol_versions(vec![
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
        }

        fn v2_meta_with_trace() -> RequestMeta {
            RequestMeta::new().with_meta(
                "traceparent",
                serde_json::json!("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
            )
        }

        fn v2_context() -> ProtocolContext {
            ProtocolContext::new(
                Era::V2,
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            )
            .with_client_info(Implementation::new("test-client", "9.9.9"))
        }

        // Enter through the REAL core dispatch entrypoint (handle_request_internal),
        // NOT the leaf handlers — a dropped dispatch-arm thread would regress this.
        #[tokio::test]
        async fn prompt_resource_protocol_context_via_dispatch_core() {
            // --- v2 dispatch: era==V2, client_info==Some, trace_context populated.
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());

            core.handle_request_internal(
                RequestId::from(1i64),
                get_prompt_request("greeting", Some(v2_meta_with_trace())),
                None,
                Some(v2_context()),
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;
            core.handle_request_internal(
                RequestId::from(2i64),
                read_resource_request("mem://greeting", Some(v2_meta_with_trace())),
                None,
                Some(v2_context()),
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;

            for cap in [&pcap, &rcap] {
                let c = cap.lock().unwrap().clone().expect("handler ran");
                assert_eq!(c.era, Some(Era::V2), "era must be V2 on a v2 dispatch");
                assert!(c.has_client_info, "client_info must be visible on v2");
                assert_eq!(
                    c.traceparent.as_deref(),
                    Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
                    "trace_context must reflect the W3C traceparent (proves with_request_meta)"
                );
            }

            // --- opted-in v1 fallback: era==Some(V1) (distinct from None).
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());
            let v1 = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()));
            core.handle_request_internal(
                RequestId::from(3i64),
                get_prompt_request("greeting", None),
                None,
                Some(v1.clone()),
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;
            core.handle_request_internal(
                RequestId::from(4i64),
                read_resource_request("mem://greeting", None),
                None,
                Some(v1),
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;
            assert_eq!(pcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));
            assert_eq!(rcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));

            // --- non-opted-in server (resolver returns None): era==None.
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());
            core.handle_request_internal(
                RequestId::from(5i64),
                get_prompt_request("greeting", None),
                None,
                None,
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;
            core.handle_request_internal(
                RequestId::from(6i64),
                read_resource_request("mem://greeting", None),
                None,
                None,
                &mut DispatchEnvelopeClaim::default(),
            )
            .await;
            assert_eq!(pcap.lock().unwrap().clone().unwrap().era, None);
            assert_eq!(rcap.lock().unwrap().clone().unwrap().era, None);
        }
    }

    /// The D-15 verdict table and the `input_required` egress it feeds
    /// (Plan 113-06, HTTP-02 / HTTP-03).
    ///
    /// Everything here is deterministic: the codec is built with an explicit
    /// fixed key through [`RequestStateCodec::new`], and "expired" is expressed
    /// as a zero-second TTL (`exp == now`, which the codec classifies as
    /// expired) rather than by sleeping.
    #[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
    mod mrtr_ingest_tests {
        use super::super::*;
        use crate::server::request_state::{
            Continuation, RequestBinding, RequestStateCodec, Verdict,
        };
        use crate::types::protocol::{Era, ProtocolContext};
        use crate::types::{CallToolRequest, ListToolsRequest, ProtocolVersion};
        use serde_json::json;
        use std::time::Duration;

        const KEY_A: [u8; 32] = [0x11; 32];
        const KEY_B: [u8; 32] = [0x22; 32];
        const ALICE: &str = "alice";

        fn codec(key: &[u8; 32], ttl_secs: u64) -> RequestStateCodec {
            RequestStateCodec::new(key, Duration::from_secs(ttl_secs)).expect("codec builds")
        }

        /// `arguments` nested `levels` objects deep.
        ///
        /// The salient whitelist wrapper costs one canonical level, so the
        /// `arguments` VALUE sits at depth 1 and its leaf at depth `levels + 1`.
        /// `levels == MAX_CANONICAL_DEPTH - 1` is therefore the deepest
        /// `arguments` the digest still accepts, and `MAX_CANONICAL_DEPTH` the
        /// shallowest it refuses.
        fn nested_arguments(levels: usize) -> Value {
            let mut value = json!("leaf");
            for _ in 0..levels {
                value = json!({ "n": value });
            }
            value
        }

        /// The deepest `arguments` an MRTR request may carry.
        fn arguments_at_the_cap() -> Value {
            nested_arguments(crate::types::mrtr::MAX_CANONICAL_DEPTH - 1)
        }

        /// One level deeper than [`arguments_at_the_cap`] — refused.
        fn arguments_past_the_cap() -> Value {
            nested_arguments(crate::types::mrtr::MAX_CANONICAL_DEPTH)
        }

        /// A `tools/call` for `search` with the given arguments.
        fn call_tool(arguments: Value) -> Request {
            Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
                name: "search".to_string(),
                arguments,
                _meta: None,
                task: None,
            })))
        }

        fn v2_context() -> ProtocolContext {
            ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()))
        }

        /// Mint a token bound to `principal` + the SAME live params dispatch
        /// will derive for `request`.
        fn mint_for(
            codec: &RequestStateCodec,
            principal: &str,
            request: &Request,
            state: &Value,
            round: u8,
        ) -> String {
            let target = mrtr_binding_parts(request).expect("an MRTR-eligible request");
            let binding = RequestBinding::from_request(principal, target.0, &target.1)
                .expect("the fixture params are inside the canonical depth cap");
            codec
                .mint(state, &binding, round, None)
                .expect("mint succeeds")
        }

        fn ingest(
            request: &Request,
            token: Option<&str>,
            subject: Option<&str>,
            has_auth_provider: bool,
            codec: Option<&RequestStateCodec>,
        ) -> MrtrIngest {
            let mut context = v2_context();
            if let Some(token) = token {
                context = context.with_mrtr_params(crate::types::mrtr::MrtrRequestParams {
                    input_responses: None,
                    input_responses_raw: None,
                    request_state: Some(token.to_string()),
                });
            }
            let target = mrtr_binding_parts(request);
            mrtr_ingest(&MrtrIngestInputs {
                target: target.as_ref(),
                protocol_context: Some(&context),
                principal: MrtrPrincipal {
                    authenticated_subject: subject,
                    has_auth_provider,
                },
                codec,
            })
        }

        // -----------------------------------------------------------------
        // The four D-15 verdicts.
        // -----------------------------------------------------------------

        #[test]
        fn valid_token_proceeds_with_state_and_round() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&codec, ALICE, &request, &json!({ "step": 7 }), 2);
            let verdict = ingest(&request, Some(&token), Some(ALICE), false, Some(&codec));
            let MrtrIngest::Proceed {
                continuation,
                round,
                kinds: _,
            } = verdict
            else {
                panic!("a live, authentic token must Proceed, got {verdict:?}");
            };
            assert_eq!(continuation, json!({ "step": 7 }));
            assert_eq!(round, 2);
        }

        /// The conformance mutation: a tampered token is a JSON-RPC ERROR, never
        /// a re-prompt and never a complete result
        /// (`sep-2322-reject-tampered-state`).
        #[test]
        fn tampered_token_rejects_and_never_reelicits() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = format!(
                "{}-TAMPERED",
                mint_for(&codec, ALICE, &request, &json!({}), 0)
            );
            let verdict = ingest(&request, Some(&token), Some(ALICE), false, Some(&codec));
            let MrtrIngest::Reject { code, message } = verdict else {
                panic!("a tampered token must Reject, got {verdict:?}");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
            assert_eq!(message, MRTR_REJECT_MESSAGE);
        }

        /// A token minted for `alice` and presented by `bob` fails the AEAD tag
        /// check — the principal lives in the AAD (T-113-02).
        #[test]
        fn principal_mismatch_rejects() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&codec, ALICE, &request, &json!({}), 0);
            let verdict = ingest(&request, Some(&token), Some("bob"), false, Some(&codec));
            assert!(
                matches!(verdict, MrtrIngest::Reject { .. }),
                "a cross-principal replay must Reject, got {verdict:?}"
            );
        }

        /// A token minted for one set of salient arguments cannot be replayed
        /// onto another, nor onto a different method (T-113-03).
        #[test]
        fn originating_request_mismatch_rejects() {
            let codec = codec(&KEY_A, 300);
            let minted_for = call_tool(json!({ "q": "a" }));
            let token = mint_for(&codec, ALICE, &minted_for, &json!({}), 0);

            let other_args = call_tool(json!({ "q": "b" }));
            assert!(
                matches!(
                    ingest(&other_args, Some(&token), Some(ALICE), false, Some(&codec)),
                    MrtrIngest::Reject { .. }
                ),
                "a token replayed onto different arguments must Reject"
            );

            let other_method = Request::Client(Box::new(ClientRequest::GetPrompt(
                crate::types::GetPromptRequest {
                    name: "search".to_string(),
                    arguments: HashMap::new(),
                    _meta: None,
                },
            )));
            assert!(
                matches!(
                    ingest(
                        &other_method,
                        Some(&token),
                        Some(ALICE),
                        false,
                        Some(&codec)
                    ),
                    MrtrIngest::Reject { .. }
                ),
                "a tools/call token replayed onto prompts/get must Reject"
            );
        }

        /// D-04 degraded path: another instance's per-process key is NOT
        /// tampering — it re-elicits from round 0.
        #[test]
        fn unknown_key_reelicits_from_round_zero() {
            let minting = codec(&KEY_B, 300);
            let serving = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&minting, ALICE, &request, &json!({ "step": 4 }), 3);
            let verdict = ingest(&request, Some(&token), Some(ALICE), false, Some(&serving));
            assert!(
                matches!(verdict, MrtrIngest::Reelicit { round: 0 }),
                "an unknown key id must re-elicit from round 0, got {verdict:?}"
            );
        }

        /// T-113-49: an authentic but expired token re-elicits while PRESERVING
        /// the round, so a hostile server cannot reset the client's D-09 bound
        /// by letting tokens expire.
        #[test]
        fn expired_token_reelicits_preserving_the_round() {
            // A zero-second TTL mints `exp == now`, which the codec classifies
            // as expired — deterministic, no sleeping.
            let minting = codec(&KEY_A, 0);
            let serving = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&minting, ALICE, &request, &json!({ "step": 1 }), 5);
            let verdict = ingest(&request, Some(&token), Some(ALICE), false, Some(&serving));
            assert!(
                matches!(verdict, MrtrIngest::Reelicit { round: 5 }),
                "an expired token must re-elicit at its own round, got {verdict:?}"
            );
        }

        // -----------------------------------------------------------------
        // The canonicalization depth refusal (D-113-M, T-113-122/123/125).
        // -----------------------------------------------------------------

        /// REFUSAL POINT 1: a request that cannot be canonicalized and PRESENTS a
        /// token is refused with `INVALID_PARAMS`, and the handler never runs.
        ///
        /// The token here was minted for a DIFFERENT (shallow) request, so before
        /// the refusal existed this path reached `verify` and produced
        /// [`MRTR_REJECT_MESSAGE`]. Asserting the message is
        /// [`MRTR_UNCANONICALIZABLE_MESSAGE`] is therefore a structural proof that
        /// the refusal fires BEFORE the tag check, not merely alongside it.
        #[test]
        fn an_uncanonicalizable_request_presenting_a_token_is_refused() {
            let codec = codec(&KEY_A, 300);
            let shallow = call_tool(json!({ "q": "a" }));
            let token = mint_for(&codec, ALICE, &shallow, &json!({ "step": 1 }), 0);

            let deep = call_tool(arguments_past_the_cap());
            let verdict = ingest(&deep, Some(&token), Some(ALICE), false, Some(&codec));
            let MrtrIngest::Reject { code, message } = verdict else {
                panic!("an unbindable state-bearing request must Reject, got {verdict:?}");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
            assert_eq!(message, MRTR_UNCANONICALIZABLE_MESSAGE);
            assert_ne!(
                message, MRTR_REJECT_MESSAGE,
                "the refusal must precede `verify` — reaching the tag check would \
                 have produced the generic authentication message instead"
            );

            // `Reject` -> `Err` is what both dispatch sites turn into a JSON-RPC
            // error WITHOUT invoking the handler.
            assert!(
                verdict_is_rejected(&deep, &token, &codec),
                "the handler must never be invoked for an unbindable request"
            );
        }

        /// `MrtrIngest::apply` maps the refusal to `Err`, which is the mechanism by
        /// which the handler is skipped.
        fn verdict_is_rejected(request: &Request, token: &str, codec: &RequestStateCodec) -> bool {
            ingest(request, Some(token), Some(ALICE), false, Some(codec))
                .apply(Some(v2_context()))
                .is_err()
        }

        /// The boundary is EXACT on the ingress side too: `arguments` one level
        /// shallower than the refusal mint, resend and verify end to end.
        #[test]
        fn a_request_exactly_at_the_depth_cap_still_mints_and_verifies() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(arguments_at_the_cap());
            let token = mint_for(&codec, ALICE, &request, &json!({ "step": 3 }), 1);
            let verdict = ingest(&request, Some(&token), Some(ALICE), false, Some(&codec));
            let MrtrIngest::Proceed {
                continuation,
                round,
                kinds: _,
            } = verdict
            else {
                panic!("a request AT the cap must still Proceed, got {verdict:?}");
            };
            assert_eq!(continuation, json!({ "step": 3 }));
            assert_eq!(round, 1);
        }

        /// THE BLAST-RADIUS TEST for the wire-visible behaviour change.
        ///
        /// The refusal is confined to requests that MINT or PRESENT a continuation.
        /// An ordinary deep-`arguments` `tools/call` that never touches MRTR — no
        /// token presented — computes no digest at all and is `Inert`, so it
        /// dispatches byte-for-byte as it did before this change. Explicit rather
        /// than implied: this is the whole claim that bounds the change.
        #[test]
        fn a_deep_request_that_never_touches_mrtr_is_unaffected() {
            let codec = codec(&KEY_A, 300);
            let deep = call_tool(arguments_past_the_cap());
            let verdict = ingest(&deep, None, Some(ALICE), false, Some(&codec));
            assert!(
                matches!(verdict, MrtrIngest::Inert),
                "a deep request with NO requestState must be Inert, got {verdict:?}"
            );
            let (context, round) = verdict
                .apply(Some(v2_context()))
                .expect("Inert is not a rejection");
            assert!(context
                .expect("the context survives")
                .mrtr_continuation()
                .is_none());
            assert_eq!(round, 0);

            // And the same request on a v1 client is equally untouched.
            let v1 = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()));
            let target = mrtr_binding_parts(&deep);
            let v1_verdict = mrtr_ingest(&MrtrIngestInputs {
                target: target.as_ref(),
                protocol_context: Some(&v1),
                principal: MrtrPrincipal {
                    authenticated_subject: Some(ALICE),
                    has_auth_provider: false,
                },
                codec: Some(&codec),
            });
            assert!(matches!(v1_verdict, MrtrIngest::Inert));
        }

        // -----------------------------------------------------------------
        // The server-side round ceiling (D-113-L, T-113-110/111/112/114).
        //
        // These drive `route_mrtr_verdict` DIRECTLY with a constructed
        // `Continuation` rather than minting one. That is deliberate and not a
        // shortcut: the ceiling is a policy over a round that has ALREADY passed
        // the AEAD tag check, so the value under test is exactly the trustworthy
        // one, and a test that had to mint 16 tokens to reach the boundary would
        // be testing the codec instead of the policy.
        // -----------------------------------------------------------------

        /// A decrypted, authenticated continuation carrying `round`.
        fn continuation_at(round: u8) -> Continuation {
            Continuation {
                state: json!({ "step": 1 }),
                exp: 0,
                round,
                kinds: None,
            }
        }

        /// Both verdicts that carry an authentic round are refused AT the
        /// ceiling — `Expired` included, because expiry is within a server's own
        /// gift and must not launder a round past the bound (T-113-112).
        #[test]
        fn round_ceiling_refuses_every_authentic_verdict_at_the_ceiling() {
            for (label, verdict) in [
                ("Ok", Verdict::Ok(continuation_at(MAX_MRTR_ROUNDS))),
                (
                    "Expired",
                    Verdict::Expired(continuation_at(MAX_MRTR_ROUNDS)),
                ),
            ] {
                let routed = route_mrtr_verdict(verdict, "tools/call");
                let MrtrIngest::Reject { code, message } = routed else {
                    panic!("{label} at the ceiling must Reject, got {routed:?}");
                };
                assert_eq!(
                    code,
                    crate::types::protocol::error_codes::INVALID_PARAMS,
                    "{label}: the sibling MRTR reject code, so the v2 HTTP status \
                     mapping is unchanged"
                );
                assert_eq!(message, MRTR_ROUND_CEILING_MESSAGE, "{label}");
                assert_ne!(
                    message, MRTR_REJECT_MESSAGE,
                    "{label}: a ceiling refusal happens AFTER the token verified, so it \
                     is not an authentication oracle and must not hide behind the \
                     generic message"
                );
            }
        }

        /// The boundary is EXACT, asserted on both sides: one below the ceiling
        /// still routes normally.
        #[test]
        fn round_ceiling_admits_exactly_one_below_itself() {
            let below = MAX_MRTR_ROUNDS - 1;
            let proceed = route_mrtr_verdict(Verdict::Ok(continuation_at(below)), "tools/call");
            let MrtrIngest::Proceed {
                continuation,
                round,
                kinds: _,
            } = proceed
            else {
                panic!("ceiling - 1 must still Proceed, got {proceed:?}");
            };
            assert_eq!(round, below);
            assert_eq!(continuation, json!({ "step": 1 }));

            let reelicit =
                route_mrtr_verdict(Verdict::Expired(continuation_at(below)), "tools/call");
            assert!(
                matches!(reelicit, MrtrIngest::Reelicit { round } if round == below),
                "ceiling - 1 must still re-elicit at its own round, got {reelicit:?}"
            );
        }

        /// `UnknownKey` still resets to round 0, ceiling or no ceiling. It is not
        /// a bypass: a client wanting a fresh counter can always just omit the
        /// `requestState` (T-113-113, ACCEPT).
        #[test]
        fn unknown_key_still_resets_to_round_zero_under_the_ceiling() {
            assert!(matches!(
                route_mrtr_verdict(Verdict::UnknownKey, "tools/call"),
                MrtrIngest::Reelicit { round: 0 }
            ));
        }

        /// An authentication failure keeps its OWN generic message — the ceiling
        /// work must not have collapsed the two reject paths into one.
        #[test]
        fn auth_failure_keeps_the_generic_message() {
            let routed = route_mrtr_verdict(Verdict::AuthFailed, "tools/call");
            let MrtrIngest::Reject { message, .. } = routed else {
                panic!("AuthFailed must Reject, got {routed:?}");
            };
            assert_eq!(message, MRTR_REJECT_MESSAGE);
        }

        proptest::proptest! {
            /// For EVERY round an authentic continuation could carry, the round
            /// threaded into egress is strictly below [`MAX_MRTR_ROUNDS`], so the
            /// mint's `saturating_add(1)` can never be observed SATURATING at 255
            /// (T-113-114). Before D-113-L was closed this property was false for
            /// every round from 255 upward — and, worse, unobservable, which is
            /// what made `RequestHandlerExtra::mrtr_round` useless as a
            /// self-limiting input for a handler.
            #[test]
            fn no_authentic_round_can_reach_saturation(round in 0u8..=u8::MAX) {
                for verdict in [
                    Verdict::Ok(continuation_at(round)),
                    Verdict::Expired(continuation_at(round)),
                ] {
                    match route_mrtr_verdict(verdict, "tools/call").apply(Some(v2_context())) {
                        Err((code, message)) => {
                            proptest::prop_assert!(round >= MAX_MRTR_ROUNDS);
                            proptest::prop_assert_eq!(
                                code,
                                crate::types::protocol::error_codes::INVALID_PARAMS
                            );
                            proptest::prop_assert_eq!(message, MRTR_ROUND_CEILING_MESSAGE);
                        },
                        Ok((_, threaded)) => {
                            proptest::prop_assert!(threaded < MAX_MRTR_ROUNDS);
                            // `saturating_add` did not saturate: the widened
                            // arithmetic agrees with it exactly.
                            proptest::prop_assert_eq!(
                                u16::from(threaded.saturating_add(1)),
                                u16::from(threaded) + 1
                            );
                            proptest::prop_assert!(threaded.saturating_add(1) <= MAX_MRTR_ROUNDS);
                        },
                    }
                }
            }
        }

        // -----------------------------------------------------------------
        // Short-circuits: everything MRTR deliberately does not touch.
        // -----------------------------------------------------------------

        /// T-113-23: the spec confines MRTR to three methods. A `requestState`
        /// on `tools/list` is IGNORED — not verified, not errored.
        #[test]
        fn ignores_a_request_state_on_a_non_eligible_method() {
            let codec = codec(&KEY_A, 300);
            let list = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
                cursor: None,
            })));
            assert!(mrtr_binding_parts(&list).is_none());
            let verdict = ingest(&list, Some("anything"), Some(ALICE), false, Some(&codec));
            assert!(
                matches!(verdict, MrtrIngest::Inert),
                "MRTR must be inert outside the three eligible methods, got {verdict:?}"
            );
        }

        #[test]
        fn is_inert_on_v1_and_without_a_token_or_codec() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&codec, ALICE, &request, &json!({}), 0);
            let target = mrtr_binding_parts(&request);

            // v1 era → zero MRTR code (D-04).
            let v1 = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()))
                .with_mrtr_params(crate::types::mrtr::MrtrRequestParams {
                    input_responses: None,
                    input_responses_raw: None,
                    request_state: Some(token.clone()),
                });
            assert!(matches!(
                mrtr_ingest(&MrtrIngestInputs {
                    target: target.as_ref(),
                    protocol_context: Some(&v1),
                    principal: MrtrPrincipal {
                        authenticated_subject: Some(ALICE),
                        has_auth_provider: false,
                    },
                    codec: Some(&codec),
                }),
                MrtrIngest::Inert
            ));

            // No resolved context at all.
            assert!(matches!(
                mrtr_ingest(&MrtrIngestInputs {
                    target: target.as_ref(),
                    protocol_context: None,
                    principal: MrtrPrincipal {
                        authenticated_subject: Some(ALICE),
                        has_auth_provider: false,
                    },
                    codec: Some(&codec),
                }),
                MrtrIngest::Inert
            ));

            // No token presented.
            assert!(matches!(
                ingest(&request, None, Some(ALICE), false, Some(&codec)),
                MrtrIngest::Inert
            ));

            // A v1-only server holds no codec.
            assert!(matches!(
                ingest(&request, Some(&token), Some(ALICE), false, None),
                MrtrIngest::Inert
            ));
        }

        // -----------------------------------------------------------------
        // Principal resolution (T-113-06 / T-113-22).
        // -----------------------------------------------------------------

        /// A server WITH an auth provider refuses MRTR to an unauthenticated
        /// caller: verification is never attempted and a `-32602` is returned.
        #[test]
        fn auth_configured_server_refuses_an_unauthenticated_caller() {
            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&codec, ANONYMOUS_PRINCIPAL, &request, &json!({}), 0);
            let verdict = ingest(&request, Some(&token), None, true, Some(&codec));
            let MrtrIngest::Reject { code, .. } = verdict else {
                panic!("an auth-configured server must refuse MRTR here, got {verdict:?}");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
        }

        /// A server with NO auth provider has no principals to separate, so the
        /// documented anonymous constant is used and MRTR works.
        #[test]
        fn anonymous_principal_is_used_only_without_an_auth_provider() {
            assert_eq!(ANONYMOUS_PRINCIPAL, "");
            assert_eq!(
                resolve_mrtr_principal(MrtrPrincipal {
                    authenticated_subject: None,
                    has_auth_provider: false,
                }),
                Some(ANONYMOUS_PRINCIPAL)
            );
            assert_eq!(
                resolve_mrtr_principal(MrtrPrincipal {
                    authenticated_subject: None,
                    has_auth_provider: true,
                }),
                None,
                "fail closed on an auth-configured server"
            );
            assert_eq!(
                resolve_mrtr_principal(MrtrPrincipal {
                    authenticated_subject: Some(ALICE),
                    has_auth_provider: true,
                }),
                Some(ALICE)
            );

            let codec = codec(&KEY_A, 300);
            let request = call_tool(json!({}));
            let token = mint_for(&codec, ANONYMOUS_PRINCIPAL, &request, &json!({ "a": 1 }), 0);
            assert!(matches!(
                ingest(&request, Some(&token), None, false, Some(&codec)),
                MrtrIngest::Proceed { .. }
            ));
        }

        // -----------------------------------------------------------------
        // `apply`: how each verdict lands on the threaded context.
        // -----------------------------------------------------------------

        #[test]
        fn apply_proceed_surfaces_continuation_and_round() {
            let (context, round) = MrtrIngest::Proceed {
                continuation: json!({ "step": 3 }),
                round: 2,
                kinds: None,
            }
            .apply(Some(v2_context()))
            .expect("Proceed is not a rejection");
            let context = context.expect("context survives");
            assert_eq!(context.mrtr_continuation(), Some(&json!({ "step": 3 })));
            assert_eq!(context.mrtr_round(), Some(2));
            assert_eq!(round, 2, "egress mints the next token at round + 1");
        }

        /// The consensus fix: a re-run handler sees a PRISTINE first call — all
        /// three MRTR accessors `None`.
        #[test]
        fn apply_reelicit_strips_every_signal_and_keeps_the_round() {
            let carried = v2_context()
                .with_mrtr_params(crate::types::mrtr::MrtrRequestParams {
                    input_responses: Some(crate::types::mrtr::InputResponses::new()),
                    input_responses_raw: None,
                    request_state: Some("token".to_string()),
                })
                .with_verified_continuation(json!({ "step": 1 }), 4);
            let (context, round) = MrtrIngest::Reelicit { round: 4 }
                .apply(Some(carried))
                .expect("Reelicit is not a rejection");
            let context = context.expect("context survives");
            assert!(context.input_responses().is_none());
            assert!(context.request_state_token().is_none());
            assert!(context.mrtr_continuation().is_none());
            assert!(context.mrtr_round().is_none());
            assert_eq!(round, 4, "the expired token's round is preserved");
        }

        #[test]
        fn apply_reject_is_an_error_so_the_handler_never_runs() {
            let outcome = MrtrIngest::Reject {
                code: crate::types::protocol::error_codes::INVALID_PARAMS,
                message: MRTR_REJECT_MESSAGE.to_string(),
            }
            .apply(Some(v2_context()));
            let Err((code, message)) = outcome else {
                panic!("Reject must short-circuit dispatch");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
            assert_eq!(message, MRTR_REJECT_MESSAGE);
        }

        #[test]
        fn apply_inert_leaves_the_context_untouched() {
            let (context, round) = MrtrIngest::Inert
                .apply(Some(v2_context()))
                .expect("Inert is not a rejection");
            let context = context.expect("context survives");
            assert!(context.mrtr_continuation().is_none());
            assert_eq!(round, 0);
        }

        // -----------------------------------------------------------------
        // The kind-directed re-decode on the VERIFIED path (D-113-O).
        //
        // These drive `apply` directly with a constructed `Proceed`, for the
        // same reason the round-ceiling tests do: the kinds map is a policy
        // input that has ALREADY passed the AEAD tag check, so the value under
        // test is exactly the trustworthy one. The end-to-end proof that the
        // kinds actually SURVIVE the seal lives in `request_state`'s
        // `mint_then_verify_round_trips_the_requested_kinds` and in
        // `tests/v2_mrtr.rs`, over a real socket.
        // -----------------------------------------------------------------

        /// The literal overlapping answer of D-113-O: `action` AND
        /// `content` + `model`, so it satisfies both `ElicitResult` and
        /// `CreateMessageResult`, and Sampling is tried first.
        fn overlapping_answer() -> Value {
            json!({
                "action": "accept",
                "content": { "type": "text", "text": "hello" },
                "model": "attacker-chosen-model",
            })
        }

        /// A v2 context carrying `answers` as the client sent them, typed the way
        /// transport ingress types them — by untagged guess.
        fn context_answering(answers: &[(&str, Value)]) -> crate::types::protocol::ProtocolContext {
            let raw: serde_json::Map<String, Value> = answers
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect();
            let mut params = json!({ "name": "t", "arguments": {} });
            params["inputResponses"] = Value::Object(raw);
            let mrtr = crate::types::mrtr::extract_mrtr_params(&params)
                .expect("the fixture answers are inside every ingress bound");
            v2_context().with_mrtr_params(mrtr)
        }

        fn kinds_of(
            entries: &[(&str, crate::types::mrtr::InputRequestKind)],
        ) -> crate::types::mrtr::InputRequestKinds {
            entries
                .iter()
                .map(|(key, kind)| ((*key).to_string(), *kind))
                .collect()
        }

        fn proceed_with(
            context: crate::types::protocol::ProtocolContext,
            kinds: Option<crate::types::mrtr::InputRequestKinds>,
        ) -> std::result::Result<(Option<crate::types::protocol::ProtocolContext>, u8), (i32, String)>
        {
            MrtrIngest::Proceed {
                continuation: json!({ "step": 1 }),
                round: 1,
                kinds,
            }
            .apply(Some(context))
        }

        /// D-113-O through `apply`: an elicitation was requested under `"k"` and
        /// the client answers with the overlapping object. The handler now
        /// receives it typed as an ELICITATION — where the untagged guess handed
        /// it over as `Sampling`, the handler's arm fell through, and it
        /// re-elicited.
        ///
        /// The outcome is correct TYPING, not rejection: the overlapping object
        /// is a valid `ElicitResult` (surplus `model` ignored), so the client's
        /// answer was fine and the server's guess was the defect. See the mrtr
        /// unit test of the same name for the full reasoning.
        #[test]
        fn the_literal_d113o_answer_reaches_the_handler_as_an_elicitation() {
            let (context, _) = proceed_with(
                context_answering(&[("k", overlapping_answer())]),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Elicitation,
                )])),
            )
            .expect("a valid ElicitResult answered to an elicitation proceeds");
            assert!(matches!(
                context
                    .expect("context survives")
                    .input_responses()
                    .expect("answers")["k"],
                crate::types::mrtr::InputResponse::Elicitation(_)
            ));
        }

        /// An answer that genuinely cannot be the requested kind is REJECTED with
        /// `INVALID_PARAMS`, and the refusal names the key and the kind.
        #[test]
        fn an_answer_that_cannot_be_the_requested_kind_is_rejected_at_the_verified_path() {
            let sampling_only = json!({
                "content": { "type": "text", "text": "hello" },
                "model": "attacker-chosen-model",
            });
            let outcome = proceed_with(
                context_answering(&[("k", sampling_only)]),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Elicitation,
                )])),
            );
            let Err((code, message)) = outcome else {
                panic!("an answer that is not an ElicitResult must short-circuit dispatch");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
            assert!(
                message.contains("\"k\""),
                "the refusal must NAME the key it is about: {message}"
            );
            assert!(
                message.contains("elicitation/create"),
                "...and the kind the server actually requested there: {message}"
            );
            assert_ne!(
                message, MRTR_REJECT_MESSAGE,
                "it fires only AFTER the tag check passed, so it is not an authentication \
                 oracle and must not hide behind the generic message"
            );
        }

        /// An UNAMBIGUOUS `ElicitResult` proceeds too, and the continuation still
        /// lands. Without this, a fix that rejected everything would pass the
        /// rejection test above.
        #[test]
        fn a_correctly_shaped_answer_reaches_the_handler_typed_by_kind() {
            let (context, round) = proceed_with(
                context_answering(&[("k", json!({ "action": "accept", "content": { "v": 1 } }))]),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Elicitation,
                )])),
            )
            .expect("a well-shaped answer proceeds");
            let context = context.expect("context survives");
            assert_eq!(round, 1);
            assert!(matches!(
                context.input_responses().expect("answers survive")["k"],
                crate::types::mrtr::InputResponse::Elicitation(_)
            ));
            assert_eq!(context.mrtr_continuation(), Some(&json!({ "step": 1 })));
        }

        /// A SAMPLING request answered with the overlapping object is ACCEPTED —
        /// the same bytes the elicitation case rejects. The rejection is a
        /// property of the mismatch, not of the value.
        #[test]
        fn the_same_bytes_are_accepted_when_sampling_is_what_was_requested() {
            let (context, _) = proceed_with(
                context_answering(&[("k", overlapping_answer())]),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Sampling,
                )])),
            )
            .expect("the overlapping object IS a valid CreateMessageResult");
            assert!(matches!(
                context
                    .expect("context survives")
                    .input_responses()
                    .expect("answers")["k"],
                crate::types::mrtr::InputResponse::Sampling(_)
            ));
        }

        /// An answer under a key the continuation never requested is rejected,
        /// and the message does NOT echo that key — it is client-chosen.
        #[test]
        fn an_unsolicited_key_is_rejected_at_the_verified_path() {
            let outcome = proceed_with(
                context_answering(&[("surprise", json!({ "roots": [] }))]),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Elicitation,
                )])),
            );
            let Err((code, message)) = outcome else {
                panic!("an unsolicited key must short-circuit dispatch");
            };
            assert_eq!(code, crate::types::protocol::error_codes::INVALID_PARAMS);
            assert!(
                !message.contains("surprise"),
                "an unsolicited key is CLIENT-chosen, so it must not be echoed: {message}"
            );
        }

        /// A continuation minted before the kinds map existed degrades to the
        /// untagged typing and does NOT reject — the rolling-deploy path. The
        /// answer stays `Sampling`, which is the pre-fix behaviour, preserved
        /// deliberately rather than by omission.
        #[test]
        fn a_pre_kinds_continuation_degrades_to_untagged_instead_of_rejecting() {
            let (context, _) =
                proceed_with(context_answering(&[("k", overlapping_answer())]), None)
                    .expect("a pre-kinds continuation must never reject");
            assert!(matches!(
                context
                    .expect("context survives")
                    .input_responses()
                    .expect("answers")["k"],
                crate::types::mrtr::InputResponse::Sampling(_)
            ));
        }

        /// A verified round carrying NO `inputResponses` is untouched: the client
        /// may resend without answering, and the handler simply asks again.
        #[test]
        fn a_verified_round_with_no_answers_is_not_a_mismatch() {
            let (context, _) = proceed_with(
                v2_context(),
                Some(kinds_of(&[(
                    "k",
                    crate::types::mrtr::InputRequestKind::Elicitation,
                )])),
            )
            .expect("answering nothing is not an error");
            assert!(context
                .expect("context survives")
                .input_responses()
                .is_none());
        }

        // -----------------------------------------------------------------
        // Egress: the signal never reaches the wire, and `input_required` is
        // emitted with a token minted at round + 1.
        //
        // Its OWN module so `cargo test -- mrtr_egress` selects exactly this
        // suite; the ingress helpers above are reached through `use super::*`.
        // -----------------------------------------------------------------
        mod mrtr_egress {
            use super::*;

            /// A form-mode elicitation `inputRequests` map.
            fn form_requests() -> crate::types::mrtr::InputRequests {
                let mut requests = crate::types::mrtr::InputRequests::new();
                requests.insert(
                    "user_name".to_string(),
                    crate::types::mrtr::InputRequest::Elicitation(Box::new(
                        crate::types::elicitation::ElicitRequestParams::Form {
                            message: "Who are you?".to_string(),
                            requested_schema: json!({ "type": "object" }),
                        },
                    )),
                );
                requests
            }

            fn signal_meta() -> Value {
                // Built through the PUBLIC authoring surface, so the doc'd handler
                // path is the one under test.
                let (_, value) = crate::types::mrtr::MrtrSignal {
                    input_requests: form_requests(),
                    continuation: json!({ "step": 1 }),
                }
                .into_meta_entry()
                .expect("signal serializes");
                value
            }

            fn signalling_response() -> JSONRPCResponse {
                signalling_response_for(&signal_meta())
            }

            fn signalling_response_for(signal: &Value) -> JSONRPCResponse {
                ServerCore::success_response(
                    RequestId::from(1i64),
                    json!({
                        "content": [],
                        "_meta": { crate::types::mrtr::MRTR_SIGNAL_META_KEY: signal },
                    }),
                )
            }

            /// A v2 context declaring every MRTR-fulfillable client capability.
            ///
            /// Without this the declared-capability precheck rejects before minting,
            /// which is a DIFFERENT path from the happy one these tests pin.
            fn v2_context_all_caps() -> ProtocolContext {
                v2_context().with_client_capabilities(caps(
                    Some(crate::types::capabilities::ElicitationCapabilities {
                        form: None,
                        url: Some(json!({})),
                    }),
                    Some(crate::types::capabilities::SamplingCapabilities::default()),
                    Some(crate::types::capabilities::RootsCapabilities::default()),
                ))
            }

            fn caps(
                elicitation: Option<crate::types::capabilities::ElicitationCapabilities>,
                sampling: Option<crate::types::capabilities::SamplingCapabilities>,
                roots: Option<crate::types::capabilities::RootsCapabilities>,
            ) -> crate::types::ClientCapabilities {
                crate::types::ClientCapabilities {
                    sampling,
                    elicitation,
                    roots,
                    ..Default::default()
                }
            }

            fn error_of(response: &JSONRPCResponse) -> &crate::types::jsonrpc::JSONRPCError {
                match response.payload {
                    ResponsePayload::Error(ref error) => error,
                    ResponsePayload::Result(_) => panic!("expected an error payload"),
                }
            }

            /// Run egress against a `tools/call` with the given context, and report
            /// how many tokens the codec minted while doing so.
            fn egress_with(
                response: &mut JSONRPCResponse,
                context: Option<&ProtocolContext>,
                codec: Option<&RequestStateCodec>,
                round: u8,
            ) -> (ResponseDisposition, ReservedFieldOwner) {
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request);
                mrtr_egress(
                    response,
                    &MrtrEgressInputs {
                        target: target.as_ref(),
                        protocol_context: context,
                        principal: MrtrPrincipal {
                            authenticated_subject: Some(ALICE),
                            has_auth_provider: false,
                        },
                        codec,
                        round,
                    },
                )
            }

            fn result_of(response: &JSONRPCResponse) -> &Value {
                match response.payload {
                    ResponsePayload::Result(ref value) => value,
                    ResponsePayload::Error(_) => panic!("expected a result payload"),
                }
            }

            #[test]
            fn egress_emits_input_required_with_a_round_plus_one_token() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request);
                let context = v2_context_all_caps();
                let mut response = signalling_response();
                let (disposition, owner) = mrtr_egress(
                    &mut response,
                    &MrtrEgressInputs {
                        target: target.as_ref(),
                        protocol_context: Some(&context),
                        principal: MrtrPrincipal {
                            authenticated_subject: Some(ALICE),
                            has_auth_provider: false,
                        },
                        codec: Some(&codec),
                        round: 4,
                    },
                );
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                );

                // `resultType` is written by the envelope step, NOT by egress —
                // there is exactly one writer of that key. Run the real envelope
                // here so this test pins the END-TO-END contract (egress SELECTS
                // the disposition, `inject_v2_result_envelope` EMITS it) rather
                // than asserting a field without pinning who produced it.
                assert!(
                    result_of(&response).get("resultType").is_none(),
                    "egress must not write resultType — the envelope owns it"
                );
                let server_info = Implementation::new("test", "1.0.0");
                inject_v2_result_envelope(
                    &mut response,
                    Some(&context),
                    &server_info,
                    disposition,
                    owner,
                    Cacheable::No,
                );

                let result = result_of(&response);
                assert_eq!(result["resultType"], "input_required");
                assert!(
                    result["inputRequests"]
                        .as_object()
                        .is_some_and(|m| !m.is_empty()),
                    "the re-elicitation must carry REAL inputRequests, got {result}"
                );
                let token = result["requestState"]
                    .as_str()
                    .expect("a fresh requestState is minted");
                // The internal signal is gone; what `_meta` carries is exactly
                // the server-owned reserved key the envelope wrote.
                assert!(
                    !serde_json::to_string(result)
                        .expect("serializes")
                        .contains(crate::types::mrtr::MRTR_SIGNAL_META_KEY),
                    "got {result}"
                );
                assert_eq!(result["_meta"].as_object().expect("an object").len(), 1);
                assert!(result["_meta"][RESERVED_SERVER_INFO_KEY].is_object());

                // Decrypt in-test: the fresh token carries round + 1.
                let binding = RequestBinding::from_request(
                    ALICE,
                    target.as_ref().expect("eligible").0,
                    &target.as_ref().expect("eligible").1,
                )
                .expect("the fixture params are inside the canonical depth cap");
                let crate::server::request_state::Verdict::Ok(continuation) =
                    codec.verify(token, &binding)
                else {
                    panic!("the freshly minted token must verify");
                };
                assert_eq!(continuation.round, 5);
                assert_eq!(continuation.state, json!({ "step": 1 }));
            }

            /// The pmcp-internal signal key must never reach the wire — not on v1,
            /// and not on a method the spec forbids `input_required` on — and a
            /// signal on either path FAILS LOUDLY rather than shipping a mangled
            /// "complete" result (Codex Plan-09 HIGH #1/#2).
            #[test]
            fn egress_strips_the_internal_signal_on_every_path() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request);
                let v1 = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()));
                let list = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
                    cursor: None,
                })));
                let list_target = mrtr_binding_parts(&list);
                let v2 = v2_context_all_caps();

                for (label, context, target) in [
                    ("v1 era", Some(&v1), target.as_ref()),
                    ("no resolved context", None, target.as_ref()),
                    ("non-eligible method", Some(&v2), list_target.as_ref()),
                ] {
                    let mut response = signalling_response();
                    let (disposition, owner) = mrtr_egress(
                        &mut response,
                        &MrtrEgressInputs {
                            target,
                            protocol_context: context,
                            principal: MrtrPrincipal {
                                authenticated_subject: Some(ALICE),
                                has_auth_provider: false,
                            },
                            codec: Some(&codec),
                            round: 0,
                        },
                    );
                    assert_eq!(
                        (disposition, owner),
                        (ResponseDisposition::Complete, ReservedFieldOwner::None),
                        "{label}"
                    );
                    // The ENTIRE serialized frame — not merely the result object,
                    // which no longer exists on these paths.
                    let rendered =
                        serde_json::to_string(&response).expect("the response serializes");
                    assert!(
                        !rendered.contains(crate::types::mrtr::MRTR_SIGNAL_META_KEY),
                        "{label}: the internal MRTR signal leaked onto the wire: {rendered}"
                    );
                    assert!(
                        !rendered.contains("\"step\""),
                        "{label}: the plaintext continuation leaked onto the wire: {rendered}"
                    );
                    assert!(
                        !rendered.contains("resultType"),
                        "{label}: input_required must not be emitted here"
                    );
                    // Fail LOUDLY: a handler writing the reserved key where MRTR is
                    // impossible is a server bug, and a silently "complete" result
                    // for an unfinished operation is strictly worse than an error.
                    assert_eq!(
                        error_of(&response).code,
                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                        "{label}: a forbidden-path signal must fail loudly"
                    );
                    assert_eq!(error_of(&response).message, MRTR_FORBIDDEN_PATH_MESSAGE);
                }
            }

            /// The reserved key carrying a payload that is not an `MrtrSignal` is a
            /// server bug too — it must not degrade into "no signal", which would
            /// ship an empty success for an operation the handler never completed.
            #[test]
            fn egress_fails_loudly_on_a_malformed_signal() {
                let codec = codec(&KEY_A, 300);
                let context = v2_context_all_caps();
                let mut response = signalling_response_for(&json!("not-a-signal"));
                let (disposition, owner) =
                    egress_with(&mut response, Some(&context), Some(&codec), 0);

                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(
                    error_of(&response).code,
                    crate::types::protocol::error_codes::INTERNAL_ERROR
                );
                assert_eq!(error_of(&response).message, MRTR_MALFORMED_SIGNAL_MESSAGE);
                let rendered = serde_json::to_string(&response).expect("serializes");
                assert!(!rendered.contains(crate::types::mrtr::MRTR_SIGNAL_META_KEY));
            }

            /// All three MRTR-eligible handler kinds reach egress through ONE
            /// authoring surface: `CallToolResult._meta`, `GetPromptResult._meta`
            /// and the newly additive `ReadResourceResult._meta`.
            #[test]
            fn every_eligible_result_type_can_carry_the_signal() {
                let (key, value) = crate::types::mrtr::MrtrSignal {
                    input_requests: form_requests(),
                    continuation: json!({ "step": 1 }),
                }
                .into_meta_entry()
                .expect("signal serializes");

                // resources/read — the leg this plan added.
                let mut resource = crate::types::ReadResourceResult::new(vec![]);
                let mut meta = serde_json::Map::new();
                meta.insert(key.clone(), value.clone());
                resource._meta = Some(Value::Object(meta.clone()));
                let resource = serde_json::to_value(&resource).expect("serializes");
                assert!(resource["_meta"][&key].is_object());

                // prompts/get — the pre-existing `_meta` precedent.
                let mut prompt = crate::types::GetPromptResult {
                    description: None,
                    messages: vec![],
                    _meta: None,
                };
                prompt._meta = Some(meta.clone());
                let prompt = serde_json::to_value(&prompt).expect("serializes");
                assert!(prompt["_meta"][&key].is_object());

                // Each of them survives the round trip THROUGH egress: strip finds
                // the signal wherever the result object came from.
                for shape in [resource, prompt] {
                    let codec = codec(&KEY_A, 300);
                    let context = v2_context_all_caps();
                    let mut response = ServerCore::success_response(RequestId::from(1i64), shape);
                    let (disposition, owner) =
                        egress_with(&mut response, Some(&context), Some(&codec), 0);
                    assert_eq!(
                        (disposition, owner),
                        (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                    );
                    let result = result_of(&response);
                    assert!(result["requestState"].is_string());
                    assert!(result["inputRequests"]["user_name"].is_object());
                    assert!(!serde_json::to_string(result)
                        .expect("serializes")
                        .contains(crate::types::mrtr::MRTR_SIGNAL_META_KEY));
                }
            }

            /// An absent `ReadResourceResult._meta` emits NO key, so the v1
            /// `resources/read` wire shape is byte-identical to pre-Phase-113.
            #[test]
            fn absent_read_resource_meta_emits_no_key() {
                let result = crate::types::ReadResourceResult::new(vec![]);
                let value = serde_json::to_value(&result).expect("serializes");
                assert_eq!(value, json!({ "contents": [] }));
            }

            /// The declared-capability precheck runs BEFORE any minting, proven
            /// structurally rather than with a counter: the codec is ABSENT, so a
            /// mint attempt would fail with `INTERNAL_ERROR`. Getting `-32021`
            /// instead is only possible if the check short-circuited first.
            #[test]
            fn capability_precheck_precedes_minting() {
                // Declares sampling + roots but NOT elicitation.
                let context = v2_context().with_client_capabilities(caps(
                    None,
                    Some(crate::types::capabilities::SamplingCapabilities::default()),
                    Some(crate::types::capabilities::RootsCapabilities::default()),
                ));
                let mut response = signalling_response();
                let (disposition, owner) = egress_with(&mut response, Some(&context), None, 0);

                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                let error = error_of(&response);
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY,
                    "the capability check must precede the mint, which has no codec here"
                );

                // And with a codec present, ZERO tokens reach the wire.
                let codec = codec(&KEY_A, 300);
                let mut with_codec = signalling_response();
                let _ = egress_with(&mut with_codec, Some(&context), Some(&codec), 0);
                let rendered = serde_json::to_string(&with_codec).expect("serializes");
                assert!(
                    !rendered.contains("requestState"),
                    "a rejected result must mint nothing: {rendered}"
                );
            }

            /// A `Reelicit { round: 3 }` mints at 4 — an expired token's round
            /// SURVIVES rather than resetting to 0 (T-113-49).
            #[test]
            fn reelicit_round_three_mints_round_four() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request).expect("eligible");
                let context = v2_context_all_caps();
                let mut response = signalling_response();
                let (disposition, owner) =
                    egress_with(&mut response, Some(&context), Some(&codec), 3);

                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                );
                let token = result_of(&response)["requestState"]
                    .as_str()
                    .expect("a token is minted");
                let binding = RequestBinding::from_request(ALICE, target.0, &target.1)
                    .expect("the fixture params are inside the canonical depth cap");
                let crate::server::request_state::Verdict::Ok(continuation) =
                    codec.verify(token, &binding)
                else {
                    panic!("the freshly minted token must verify");
                };
                assert_eq!(continuation.round, 4);
            }

            /// Two consecutive rounds produce DIFFERENT tokens whose decrypted
            /// rounds differ by one, and each verifies against the same live
            /// request — the retry contract the client loop depends on.
            #[test]
            fn consecutive_rounds_mint_distinct_incrementing_tokens() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request).expect("eligible");
                let binding = RequestBinding::from_request(ALICE, target.0, &target.1)
                    .expect("the fixture params are inside the canonical depth cap");
                let context = v2_context_all_caps();

                let mut first = signalling_response();
                let _ = egress_with(&mut first, Some(&context), Some(&codec), 0);
                let first_token = result_of(&first)["requestState"]
                    .as_str()
                    .expect("token")
                    .to_string();

                let mut second = signalling_response();
                let _ = egress_with(&mut second, Some(&context), Some(&codec), 1);
                let second_token = result_of(&second)["requestState"]
                    .as_str()
                    .expect("token")
                    .to_string();

                assert_ne!(first_token, second_token, "each round mints a fresh token");
                let round_of = |token: &str| match codec.verify(token, &binding) {
                    crate::server::request_state::Verdict::Ok(continuation) => continuation.round,
                    other => panic!("a freshly minted token must verify, got {other:?}"),
                };
                assert_eq!(round_of(&first_token), 1);
                assert_eq!(round_of(&second_token), 2);
            }

            /// ENFORCEMENT POINT B: the mint refuses at the ceiling and admits
            /// exactly one below it, and the refusal reaches the WIRE as
            /// `INTERNAL_ERROR` through `mrtr_egress` (D-113-L, T-113-115).
            ///
            /// `INTERNAL_ERROR` rather than `INVALID_PARAMS` is the point: this
            /// path is unreachable while the ingress bound is intact, so reaching
            /// it means a server invariant broke, not that the client misbehaved.
            #[test]
            fn mint_backstop_refuses_at_the_ceiling_and_admits_one_below() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request).expect("eligible");
                let context = v2_context_all_caps();

                // One below the ceiling: mints normally, at EXACTLY the ceiling.
                let mut admitted = signalling_response();
                let (disposition, owner) = egress_with(
                    &mut admitted,
                    Some(&context),
                    Some(&codec),
                    MAX_MRTR_ROUNDS - 1,
                );
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                );
                let token = result_of(&admitted)["requestState"]
                    .as_str()
                    .expect("a token is minted one below the ceiling");
                let binding = RequestBinding::from_request(ALICE, target.0, &target.1)
                    .expect("the fixture params are inside the canonical depth cap");
                let Verdict::Ok(continuation) = codec.verify(token, &binding) else {
                    panic!("the freshly minted token must verify");
                };
                assert_eq!(
                    continuation.round, MAX_MRTR_ROUNDS,
                    "the last admissible mint lands exactly ON the ceiling, which the \
                     ingress bound then refuses when it is presented"
                );

                // At the ceiling: refused BEFORE the mint.
                let mut refused = signalling_response();
                let (disposition, owner) =
                    egress_with(&mut refused, Some(&context), Some(&codec), MAX_MRTR_ROUNDS);
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(
                    error_of(&refused).code,
                    crate::types::protocol::error_codes::INTERNAL_ERROR
                );
                assert_eq!(
                    error_of(&refused).message,
                    MRTR_ROUND_CEILING_INVARIANT_MESSAGE
                );
                let rendered = serde_json::to_string(&refused).expect("serializes");
                // The FIELD, not the substring: the refusal message itself names
                // `requestState` in prose, which is not a minted token.
                assert!(
                    !rendered.contains(&format!("\"{}\":", crate::types::mrtr::REQUEST_STATE_KEY)),
                    "nothing may be minted past the ceiling: {rendered}"
                );
                assert!(
                    !rendered.contains("\"step\""),
                    "and the plaintext continuation must not leak either: {rendered}"
                );
            }

            // -------------------------------------------------------------
            // The canonicalization depth refusal at the MINT path (D-113-M).
            // -------------------------------------------------------------

            /// Run egress against a `tools/call` carrying the given `arguments`.
            fn egress_for_arguments(
                response: &mut JSONRPCResponse,
                arguments: Value,
                codec: Option<&RequestStateCodec>,
            ) -> (ResponseDisposition, ReservedFieldOwner) {
                let request = call_tool(arguments);
                let target = mrtr_binding_parts(&request);
                let context = v2_context_all_caps();
                mrtr_egress(
                    response,
                    &MrtrEgressInputs {
                        target: target.as_ref(),
                        protocol_context: Some(&context),
                        principal: MrtrPrincipal {
                            authenticated_subject: Some(ALICE),
                            has_auth_provider: false,
                        },
                        codec,
                        round: 0,
                    },
                )
            }

            /// REFUSAL POINT 2: a handler that signals `input_required` on an
            /// unbindable request is refused with `INVALID_PARAMS` and NOTHING is
            /// minted.
            ///
            /// `INVALID_PARAMS` and not `INTERNAL_ERROR` is the assertion that
            /// matters: the sibling `INTERNAL_ERROR` mint-failure channel is for
            /// server bugs, and this condition is caused entirely by the params the
            /// client sent.
            #[test]
            fn egress_refuses_to_mint_for_an_uncanonicalizable_request() {
                let codec = codec(&KEY_A, 300);
                let mut response = signalling_response();
                let (disposition, owner) =
                    egress_for_arguments(&mut response, arguments_past_the_cap(), Some(&codec));

                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(
                    error_of(&response).code,
                    crate::types::protocol::error_codes::INVALID_PARAMS,
                    "the client's params caused this, so it is not an INTERNAL_ERROR"
                );
                assert_eq!(error_of(&response).message, MRTR_UNCANONICALIZABLE_MESSAGE);

                let rendered = serde_json::to_string(&response).expect("serializes");
                assert!(
                    !rendered.contains(&format!("\"{}\":", crate::types::mrtr::REQUEST_STATE_KEY)),
                    "no continuation may be minted for an unidentifiable request: {rendered}"
                );
                assert!(
                    !rendered.contains("\"step\""),
                    "and the plaintext continuation must not leak either: {rendered}"
                );
            }

            /// The egress boundary is exact on both sides: at the cap the mint
            /// still happens and the token verifies against the same live request.
            #[test]
            fn egress_at_the_depth_cap_still_mints_a_verifiable_token() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(arguments_at_the_cap());
                let target = mrtr_binding_parts(&request).expect("eligible");
                let mut response = signalling_response();
                let (disposition, owner) =
                    egress_for_arguments(&mut response, arguments_at_the_cap(), Some(&codec));

                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                );
                let token = result_of(&response)["requestState"]
                    .as_str()
                    .expect("a token is minted at the cap");
                let binding = RequestBinding::from_request(ALICE, target.0, &target.1)
                    .expect("params at the cap still bind");
                let Verdict::Ok(continuation) = codec.verify(token, &binding) else {
                    panic!("the freshly minted token must verify");
                };
                assert_eq!(continuation.round, 1);
            }

            /// The step (3b) refusal precedes the mint's own preconditions, which
            /// is what makes the `seal_input_required` backstop unreachable rather
            /// than merely unlikely: it refuses with NO codec configured, which the
            /// mint itself would need.
            #[test]
            fn the_depth_refusal_precedes_every_mint_precondition() {
                let mut response = signalling_response();
                let (disposition, owner) =
                    egress_for_arguments(&mut response, arguments_past_the_cap(), None);
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(
                    error_of(&response).message,
                    MRTR_UNCANONICALIZABLE_MESSAGE,
                    "the depth check must precede the codec lookup"
                );
                assert_ne!(
                    error_of(&response).message,
                    MRTR_UNCANONICALIZABLE_INVARIANT_MESSAGE,
                    "the mint-site backstop must be UNREACHABLE while step (3b) stands"
                );
            }

            /// The two enforcement points are NOT redundant, pinned structurally:
            /// the mint backstop refuses with NO codec configured, which the
            /// mint itself would need. Getting the ceiling message rather than
            /// "this server has no requestState codec configured" is only
            /// possible if the round check ran FIRST.
            #[test]
            fn mint_backstop_precedes_every_other_mint_precondition() {
                let context = v2_context_all_caps();
                let mut response = signalling_response();
                let (disposition, owner) =
                    egress_with(&mut response, Some(&context), None, MAX_MRTR_ROUNDS);
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(
                    error_of(&response).message,
                    MRTR_ROUND_CEILING_INVARIANT_MESSAGE,
                    "the round check must precede the codec lookup"
                );
            }

            // -------------------------------------------------------------
            // The eligibility tripwire (T-113-23).
            // -------------------------------------------------------------

            /// EXACTLY three `ClientRequest` variants are MRTR-eligible, and
            /// these are they. In the spirit of Phase 112's
            /// `all_meta_bearing_client_requests_are_extracted`: the enum match
            /// is exhaustive, so a NEW variant is a compile error there; this
            /// test pins that no EXISTING variant silently joins the set.
            #[test]
            fn exactly_three_client_request_variants_are_mrtr_eligible() {
                let eligible: Vec<&str> = every_client_request()
                    .iter()
                    .filter(|(_, request)| client_request_mrtr_eligible(request))
                    .map(|(label, _)| *label)
                    .collect();
                assert_eq!(
                    eligible,
                    vec!["tools/call", "prompts/get", "resources/read"],
                    "the spec confines input_required to exactly these three methods"
                );
            }

            /// The enum tripwire and the `MRTR_METHODS` string table cannot
            /// drift: every table row has an eligible enum variant, and every
            /// eligible enum variant resolves back to a table row.
            #[test]
            fn enum_eligibility_agrees_with_the_method_table() {
                for (method, request) in every_client_request() {
                    assert_eq!(
                        client_request_mrtr_eligible(&request),
                        crate::types::mrtr::mrtr_eligible(method),
                        "{method}: the enum tripwire and MRTR_METHODS disagree"
                    );
                }
                // And the table's own rows are all covered by the enum.
                for row in &crate::types::mrtr::MRTR_METHODS {
                    assert!(
                        every_client_request()
                            .iter()
                            .any(|(method, request)| *method == row.method
                                && client_request_mrtr_eligible(request)),
                        "{}: a table row with no eligible enum variant",
                        row.method
                    );
                }
            }

            /// `mrtr_binding_parts` covers EXACTLY the table's rows — driven
            /// from `MRTR_METHODS` rather than a hand-written list, so a new row
            /// automatically widens this test instead of silently escaping it.
            #[test]
            fn binding_parts_cover_exactly_the_method_table() {
                let covered: Vec<&'static str> = every_client_request()
                    .into_iter()
                    .filter_map(|(_, request)| {
                        mrtr_binding_parts(&Request::Client(Box::new(request))).map(|(m, _)| m)
                    })
                    .collect();
                let expected: Vec<&'static str> = crate::types::mrtr::MRTR_METHODS
                    .iter()
                    .map(|row| row.method)
                    .collect();
                assert_eq!(covered, expected);
            }

            /// One instance of EVERY `ClientRequest` variant, paired with its
            /// wire method string.
            ///
            /// Hand-built on purpose: the compile-time tripwire lives in
            /// `client_request_mrtr_eligible`, and a new variant that is missing
            /// here shows up as a mismatch in
            /// `exactly_three_client_request_variants_are_mrtr_eligible` after
            /// the author has already been forced to classify it.
            fn every_client_request() -> Vec<(&'static str, ClientRequest)> {
                use crate::types::prompts::ListPromptsRequest;
                use crate::types::protocol::{
                    CompleteRequest, CompletionArgument, CompletionReference, InitializeRequest,
                };
                use crate::types::resources::{ListResourceTemplatesRequest, ListResourcesRequest};
                vec![
                    (
                        "initialize",
                        ClientRequest::Initialize(InitializeRequest {
                            protocol_version: "2026-07-28".to_string(),
                            capabilities: crate::types::ClientCapabilities::default(),
                            client_info: Implementation::new("c", "1"),
                        }),
                    ),
                    (
                        "tools/call",
                        ClientRequest::CallTool(CallToolRequest {
                            name: "search".to_string(),
                            arguments: json!({}),
                            _meta: None,
                            task: None,
                        }),
                    ),
                    (
                        "prompts/get",
                        ClientRequest::GetPrompt(crate::types::GetPromptRequest {
                            name: "greeting".to_string(),
                            arguments: HashMap::new(),
                            _meta: None,
                        }),
                    ),
                    (
                        "resources/read",
                        ClientRequest::ReadResource(crate::types::ReadResourceRequest {
                            uri: "mem://x".to_string(),
                            _meta: None,
                        }),
                    ),
                    (
                        "tools/list",
                        ClientRequest::ListTools(ListToolsRequest { cursor: None }),
                    ),
                    (
                        "prompts/list",
                        ClientRequest::ListPrompts(ListPromptsRequest { cursor: None }),
                    ),
                    (
                        "resources/list",
                        ClientRequest::ListResources(ListResourcesRequest { cursor: None }),
                    ),
                    (
                        "resources/templates/list",
                        ClientRequest::ListResourceTemplates(ListResourceTemplatesRequest {
                            cursor: None,
                        }),
                    ),
                    (
                        "resources/subscribe",
                        ClientRequest::Subscribe(crate::types::SubscribeRequest {
                            uri: "mem://x".to_string(),
                        }),
                    ),
                    (
                        "resources/unsubscribe",
                        ClientRequest::Unsubscribe(crate::types::UnsubscribeRequest {
                            uri: "mem://x".to_string(),
                        }),
                    ),
                    (
                        "completion/complete",
                        ClientRequest::Complete(CompleteRequest {
                            r#ref: CompletionReference::Prompt {
                                name: "p".to_string(),
                            },
                            argument: CompletionArgument {
                                name: "a".to_string(),
                                value: String::new(),
                            },
                        }),
                    ),
                    (
                        "sampling/createMessage",
                        ClientRequest::CreateMessage(Box::new(
                            crate::types::sampling::CreateMessageParams::new(vec![]),
                        )),
                    ),
                    (
                        "tasks/get",
                        ClientRequest::TasksGet(crate::types::tasks::GetTaskRequest {
                            task_id: "t".to_string(),
                        }),
                    ),
                    (
                        "tasks/result",
                        ClientRequest::TasksResult(crate::types::tasks::GetTaskPayloadRequest {
                            task_id: "t".to_string(),
                        }),
                    ),
                    (
                        "tasks/list",
                        ClientRequest::TasksList(crate::types::tasks::ListTasksRequest {
                            cursor: None,
                        }),
                    ),
                    (
                        "tasks/cancel",
                        ClientRequest::TasksCancel(crate::types::tasks::CancelTaskRequest {
                            task_id: "t".to_string(),
                            result: None,
                        }),
                    ),
                    (
                        "logging/setLevel",
                        ClientRequest::SetLoggingLevel {
                            level: crate::types::notifications::LoggingLevel::Info,
                        },
                    ),
                    ("ping", ClientRequest::Ping),
                ]
            }

            // -------------------------------------------------------------
            // Declared-capability precheck, SUBMODE-aware (T-113-32).
            // -------------------------------------------------------------

            fn requests_of(entries: Vec<(&str, crate::types::mrtr::InputRequest)>) -> Value {
                let mut map = crate::types::mrtr::InputRequests::new();
                for (key, request) in entries {
                    map.insert(key.to_string(), request);
                }
                let (_, value) = crate::types::mrtr::MrtrSignal {
                    input_requests: map,
                    continuation: json!({ "step": 1 }),
                }
                .into_meta_entry()
                .expect("signal serializes");
                value
            }

            fn url_elicitation() -> crate::types::mrtr::InputRequest {
                crate::types::mrtr::InputRequest::Elicitation(Box::new(
                    crate::types::elicitation::ElicitRequestParams::Url {
                        message: "Approve the payment".to_string(),
                        elicitation_id: "e1".to_string(),
                        url: "https://example.test/approve".to_string(),
                    },
                ))
            }

            fn form_elicitation() -> crate::types::mrtr::InputRequest {
                crate::types::mrtr::InputRequest::Elicitation(Box::new(
                    crate::types::elicitation::ElicitRequestParams::Form {
                        message: "Who are you?".to_string(),
                        requested_schema: json!({ "type": "object" }),
                    },
                ))
            }

            /// Drive egress with `signal` against a context declaring `caps`,
            /// returning the resulting JSON-RPC error.
            fn reject_for(
                signal: &Value,
                declared: crate::types::ClientCapabilities,
            ) -> crate::types::jsonrpc::JSONRPCError {
                let codec = codec(&KEY_A, 300);
                let context = v2_context().with_client_capabilities(declared);
                let mut response = signalling_response_for(signal);
                let (disposition, owner) =
                    egress_with(&mut response, Some(&context), Some(&codec), 0);
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                error_of(&response).clone()
            }

            /// A form elicitation against a client that declared NO elicitation
            /// is `-32021`, and `data.requiredCapabilities` is an OBJECT.
            #[test]
            fn undeclared_elicitation_is_minus_32021_with_an_object_payload() {
                let error = reject_for(
                    &requests_of(vec![("who", form_elicitation())]),
                    caps(None, None, None),
                );
                assert_eq!(error.code, -32021);
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert!(
                    required.is_object(),
                    "requiredCapabilities MUST be a ClientCapabilities object, not an array \
                     or a string list: {required}"
                );
                assert!(!required.is_array());
                assert_eq!(required, &json!({ "elicitation": {} }));
            }

            /// A URL-mode elicitation against a client that declared
            /// elicitation WITHOUT url support is `-32021` — the SUBMODE is what
            /// is missing, and the payload names it.
            #[test]
            fn url_elicitation_against_a_form_only_client_is_minus_32021() {
                let form_only = caps(
                    Some(crate::types::capabilities::ElicitationCapabilities {
                        form: Some(
                            crate::types::capabilities::FormElicitationCapability::default(),
                        ),
                        url: None,
                    }),
                    None,
                    None,
                );
                let error = reject_for(&requests_of(vec![("pay", url_elicitation())]), form_only);
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert_eq!(required, &json!({ "elicitation": { "url": {} } }));
            }

            /// A URL elicitation against a client that DID declare url support
            /// passes through unchanged.
            #[test]
            fn url_elicitation_against_a_url_capable_client_passes() {
                let codec = codec(&KEY_A, 300);
                let context = v2_context_all_caps();
                let mut response =
                    signalling_response_for(&requests_of(vec![("pay", url_elicitation())]));
                let (disposition, owner) =
                    egress_with(&mut response, Some(&context), Some(&codec), 0);
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr)
                );
                assert!(result_of(&response)["inputRequests"]["pay"].is_object());
            }

            /// A sampling entry against a client that declared no `sampling`
            /// is `-32021`.
            #[test]
            fn undeclared_sampling_is_minus_32021() {
                let error = reject_for(
                    &requests_of(vec![(
                        "draft",
                        crate::types::mrtr::InputRequest::Sampling(Box::new(
                            crate::types::sampling::CreateMessageParams::new(vec![]),
                        )),
                    )]),
                    caps(
                        Some(crate::types::capabilities::ElicitationCapabilities::default()),
                        None,
                        None,
                    ),
                );
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert_eq!(required, &json!({ "sampling": {} }));
            }

            /// A TOOL-AUGMENTED sampling entry against a client that declared
            /// plain `sampling` needs the `sampling.tools` sub-capability too.
            #[test]
            fn tool_augmented_sampling_needs_the_tools_sub_capability() {
                let mut params = crate::types::sampling::CreateMessageParams::new(vec![]);
                params.tools = Some(vec![]);
                let error = reject_for(
                    &requests_of(vec![(
                        "draft",
                        crate::types::mrtr::InputRequest::Sampling(Box::new(params)),
                    )]),
                    caps(
                        None,
                        Some(crate::types::capabilities::SamplingCapabilities::default()),
                        None,
                    ),
                );
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert_eq!(required, &json!({ "sampling": { "tools": {} } }));
            }

            /// A `roots/list` entry against a client that declared no `roots`
            /// is `-32021`.
            #[test]
            fn undeclared_roots_is_minus_32021() {
                let error = reject_for(
                    &requests_of(vec![("roots", crate::types::mrtr::InputRequest::ListRoots)]),
                    caps(None, None, None),
                );
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert!(required["roots"].is_object());
            }

            /// The rejection is ALL-OR-NOTHING: a map mixing a declared and an
            /// undeclared kind emits NO partial `inputRequests`, and the payload
            /// names only what is missing.
            #[test]
            fn a_mixed_map_is_rejected_wholesale_never_partially_emitted() {
                let error = reject_for(
                    &requests_of(vec![
                        ("who", form_elicitation()),
                        ("roots", crate::types::mrtr::InputRequest::ListRoots),
                    ]),
                    caps(
                        Some(crate::types::capabilities::ElicitationCapabilities::default()),
                        None,
                        None,
                    ),
                );
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::MISSING_REQUIRED_CLIENT_CAPABILITY
                );
                let required = &error.data.as_ref().expect("a payload")["requiredCapabilities"];
                assert!(
                    required.get("elicitation").is_none(),
                    "a DECLARED capability must not appear in the missing set: {required}"
                );
                assert!(required["roots"].is_object());
            }

            /// `inputRequests` keys are unique within one result BY
            /// CONSTRUCTION: `InputRequests` is a `BTreeMap`, so a duplicate key
            /// replaces rather than duplicating. A future change to a
            /// Vec-of-pairs shape fails this test.
            #[test]
            fn input_requests_keys_are_unique_by_construction() {
                let mut map = crate::types::mrtr::InputRequests::new();
                map.insert("dup".to_string(), form_elicitation());
                map.insert(
                    "dup".to_string(),
                    crate::types::mrtr::InputRequest::ListRoots,
                );
                assert_eq!(map.len(), 1, "a BTreeMap cannot hold a duplicate key");

                let serialized = serde_json::to_value(&map).expect("serializes");
                assert_eq!(
                    serialized.as_object().expect("an object").len(),
                    1,
                    "and the wire shape carries the key exactly once"
                );
            }

            /// Fail closed: a server that cannot seal the continuation answers a
            /// JSON-RPC error rather than a bogus "complete" result for an
            /// operation the handler did not complete.
            #[test]
            fn egress_fails_closed_when_it_cannot_mint() {
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request);
                let context = v2_context_all_caps();
                let mut response = signalling_response();
                let (disposition, owner) = mrtr_egress(
                    &mut response,
                    &MrtrEgressInputs {
                        target: target.as_ref(),
                        protocol_context: Some(&context),
                        // Unauthenticated on an auth-configured server (T-113-22).
                        principal: MrtrPrincipal {
                            authenticated_subject: None,
                            has_auth_provider: true,
                        },
                        codec: None,
                        round: 0,
                    },
                );
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                let ResponsePayload::Error(ref error) = response.payload else {
                    panic!("an unmintable continuation must fail closed with an error");
                };
                assert_eq!(
                    error.code,
                    crate::types::protocol::error_codes::INTERNAL_ERROR
                );
            }

            /// A response with no signal is left byte-identical.
            #[test]
            fn egress_is_a_noop_without_a_signal() {
                let codec = codec(&KEY_A, 300);
                let request = call_tool(json!({}));
                let target = mrtr_binding_parts(&request);
                let context = v2_context();
                let original = json!({ "content": [], "_meta": { "vendor/key": 1 } });
                let mut response =
                    ServerCore::success_response(RequestId::from(1i64), original.clone());
                let (disposition, owner) = mrtr_egress(
                    &mut response,
                    &MrtrEgressInputs {
                        target: target.as_ref(),
                        protocol_context: Some(&context),
                        principal: MrtrPrincipal {
                            authenticated_subject: Some(ALICE),
                            has_auth_provider: false,
                        },
                        codec: Some(&codec),
                        round: 0,
                    },
                );
                assert_eq!(
                    (disposition, owner),
                    (ResponseDisposition::Complete, ReservedFieldOwner::None)
                );
                assert_eq!(result_of(&response), &original);
            }
        }

        // -----------------------------------------------------------------
        // The binding is derived from the TYPED request (T-113-03).
        // -----------------------------------------------------------------

        #[test]
        fn binding_parts_cover_exactly_the_eligible_methods() {
            for (request, method) in [
                (call_tool(json!({})), "tools/call"),
                (
                    Request::Client(Box::new(ClientRequest::GetPrompt(
                        crate::types::GetPromptRequest {
                            name: "greeting".to_string(),
                            arguments: HashMap::new(),
                            _meta: None,
                        },
                    ))),
                    "prompts/get",
                ),
                (
                    Request::Client(Box::new(ClientRequest::ReadResource(
                        crate::types::ReadResourceRequest {
                            uri: "mem://greeting".to_string(),
                            _meta: None,
                        },
                    ))),
                    "resources/read",
                ),
            ] {
                let (resolved, params) =
                    mrtr_binding_parts(&request).expect("an MRTR-eligible request");
                assert_eq!(resolved, method);
                assert!(
                    crate::types::mrtr::mrtr_eligible(resolved),
                    "{method} must be in the ONE MRTR method table"
                );
                // The strip half of strip-and-re-run: the params the digest and
                // the re-run are bound to carry no MRTR field.
                assert!(params.get("inputResponses").is_none());
                assert!(params.get("requestState").is_none());
            }

            assert!(
                mrtr_binding_parts(&Request::Client(Box::new(ClientRequest::ListTools(
                    ListToolsRequest { cursor: None }
                ))))
                .is_none()
            );
        }
    }

    // =======================================================================
    // `completion/complete`, per arm (Phase 118.1-04, CONF-05 / G-4).
    //
    // These sit next to the helper rather than in `tests/completion_complete.rs`
    // because `complete_completion` and `bound_completion_values` are
    // `pub(crate)` / private: an integration test can only reach them through a
    // dispatcher, which conflates "the bound is applied" with "the dispatcher
    // applies the bound". Both facts matter, so both are asserted — the
    // dispatcher half lives in `tests/completion_complete.rs`.
    // =======================================================================
    mod completion {
        use super::*;
        use crate::types::completable::{
            CompletionItem, CompletionProviderTrait, CompletionRequest, CompletionResponse,
        };
        use crate::types::protocol::{CompleteRequest, CompletionArgument, CompletionReference};

        /// A provider returning `count` values, reporting `has_more` verbatim.
        struct Fixed {
            count: usize,
            has_more: bool,
        }

        #[async_trait]
        impl CompletionProviderTrait for Fixed {
            async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
                Ok(CompletionResponse {
                    completions: (0..self.count)
                        .map(|index| CompletionItem {
                            value: format!("v{index}"),
                            label: None,
                            description: None,
                            icon: None,
                            metadata: HashMap::new(),
                        })
                        .collect(),
                    has_more: self.has_more,
                    continuation_token: None,
                })
            }
        }

        /// A provider that always fails, to pin the error path.
        struct Failing;

        #[async_trait]
        impl CompletionProviderTrait for Failing {
            async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
                Err(Error::internal("provider exploded"))
            }
        }

        /// The suite's request shape, for the arms that do not care about it.
        fn request() -> CompleteRequest {
            CompleteRequest {
                r#ref: CompletionReference::Prompt {
                    name: "test_prompt_with_arguments".to_string(),
                },
                argument: CompletionArgument {
                    name: "arg1".to_string(),
                    value: String::new(),
                },
            }
        }

        fn provider_arc(
            provider: impl CompletionProviderTrait + 'static,
        ) -> Arc<dyn CompletionProviderTrait> {
            Arc::new(provider)
        }

        #[tokio::test]
        async fn no_provider_answers_an_empty_array_not_an_error() {
            let result = complete_completion(None, &request())
                .await
                .expect("the no-provider path is a SUCCESS, never an error");
            assert!(result.completion.values.is_empty());
            assert!(!result.completion.has_more);
            assert_eq!(result.completion.total, None);
        }

        #[tokio::test]
        async fn a_provider_under_the_bound_is_passed_through_whole() {
            let provider = provider_arc(Fixed {
                count: 7,
                has_more: false,
            });
            let result = complete_completion(Some(&provider), &request())
                .await
                .expect("provider succeeds");
            assert_eq!(result.completion.values.len(), 7);
            assert!(!result.completion.has_more);
            assert_eq!(result.completion.total, Some(7));
        }

        #[tokio::test]
        async fn exactly_the_bound_is_not_treated_as_truncated() {
            let provider = provider_arc(Fixed {
                count: MAX_COMPLETION_VALUES,
                has_more: false,
            });
            let result = complete_completion(Some(&provider), &request())
                .await
                .expect("provider succeeds");
            assert_eq!(result.completion.values.len(), MAX_COMPLETION_VALUES);
            assert!(
                !result.completion.has_more,
                "exactly 100 fits the bound, so nothing was dropped"
            );
            assert_eq!(result.completion.total, Some(MAX_COMPLETION_VALUES));
        }

        #[tokio::test]
        async fn one_over_the_bound_truncates_and_says_so() {
            let provider = provider_arc(Fixed {
                count: MAX_COMPLETION_VALUES + 1,
                has_more: false,
            });
            let result = complete_completion(Some(&provider), &request())
                .await
                .expect("provider succeeds");
            assert_eq!(result.completion.values.len(), MAX_COMPLETION_VALUES);
            assert!(
                result.completion.has_more,
                "an element was dropped, so the list must not read as exhaustive"
            );
            assert_eq!(
                result.completion.total,
                Some(MAX_COMPLETION_VALUES + 1),
                "the provider returned everything it had, so `total` is the TRUE total \
                 including the dropped element"
            );
        }

        #[tokio::test]
        async fn a_provider_claiming_more_suppresses_total() {
            let provider = provider_arc(Fixed {
                count: 3,
                has_more: true,
            });
            let result = complete_completion(Some(&provider), &request())
                .await
                .expect("provider succeeds");
            assert_eq!(result.completion.values.len(), 3);
            assert!(result.completion.has_more);
            assert_eq!(
                result.completion.total, None,
                "the provider says more exist, so the true total is UNKNOWN — inventing \
                 one would be worse than omitting an optional field"
            );
        }

        #[tokio::test]
        async fn a_failing_provider_surfaces_its_error() {
            let provider = provider_arc(Failing);
            let error = complete_completion(Some(&provider), &request())
                .await
                .expect_err("a failing provider must not be swallowed into an empty array");
            assert!(error.to_string().contains("provider exploded"));
        }

        /// A provider that records the [`CompletionRequest`] it was handed.
        struct Recording(std::sync::Mutex<Option<CompletionRequest>>);

        #[async_trait]
        impl CompletionProviderTrait for Recording {
            async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
                *self.0.lock().expect("uncontended") = Some(request);
                Ok(CompletionResponse {
                    completions: Vec::new(),
                    has_more: false,
                    continuation_token: None,
                })
            }
        }

        #[tokio::test]
        async fn the_ref_and_the_argument_reach_the_provider() {
            let recording = Arc::new(Recording(std::sync::Mutex::new(None)));
            let provider: Arc<dyn CompletionProviderTrait> = recording.clone();

            let mut wire = request();
            wire.argument.value = "te".to_string();
            complete_completion(Some(&provider), &wire)
                .await
                .expect("provider succeeds");

            let seen = recording.0.lock().expect("uncontended").clone();
            let seen = seen.expect("the provider was called");
            assert_eq!(seen.argument, "arg1");
            assert_eq!(seen.partial, "te");
            assert_eq!(
                seen.context
                    .get(COMPLETION_REF_PROMPT_KEY)
                    .map(String::as_str),
                Some("test_prompt_with_arguments"),
                "a ref/prompt reference reaches the provider under the spec's own \
                 discriminator spelling"
            );
            assert!(!seen.context.contains_key(COMPLETION_REF_RESOURCE_KEY));
        }

        #[tokio::test]
        async fn a_resource_ref_reaches_the_provider_under_its_own_key() {
            let recording = Arc::new(Recording(std::sync::Mutex::new(None)));
            let provider: Arc<dyn CompletionProviderTrait> = recording.clone();

            let wire = CompleteRequest {
                r#ref: CompletionReference::Resource {
                    uri: "test://static-text".to_string(),
                },
                argument: CompletionArgument {
                    name: "path".to_string(),
                    value: String::new(),
                },
            };
            complete_completion(Some(&provider), &wire)
                .await
                .expect("provider succeeds");

            let seen = recording.0.lock().expect("uncontended").clone();
            let seen = seen.expect("the provider was called");
            assert_eq!(
                seen.context
                    .get(COMPLETION_REF_RESOURCE_KEY)
                    .map(String::as_str),
                Some("test://static-text")
            );
            assert!(!seen.context.contains_key(COMPLETION_REF_PROMPT_KEY));
        }
    }
}

// ===========================================================================
// `ServerCore::attach_peer` precedence — the TWIN of the `Server` suite in
// `src/server/mod.rs` (`peer_precedence_tests`).
//
// Phase 118.1 plan 11. Both dispatch roots read the request-scoped
// `TransportBackchannel` before the global `peer_handle`, and the two must never
// disagree about which peer a handler sees. The `Server` side is measured over
// there; this is the same measurement on this side, so a future edit to one
// impl cannot silently diverge from the other.
// ===========================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
mod core_peer_precedence_tests {
    use super::*;
    use crate::shared::peer::PeerHandle;
    use crate::types::protocol::context::TransportBackchannel;
    use crate::types::protocol::{Era, ProtocolContext, ProtocolVersion};
    use crate::types::roots::{ListRootsResult, Root};
    use crate::types::sampling::{CreateMessageParams, CreateMessageResult};
    use crate::types::ProgressToken;

    /// A peer that reports WHICH source supplied it.
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

    async fn attached_peer_name(extra: &RequestHandlerExtra) -> Option<String> {
        let peer = extra.peer()?;
        let roots = peer.list_roots().await.expect("the fixture peer answers");
        roots.roots.first().and_then(|r| r.name.clone())
    }

    fn context_with_peer(name: &'static str) -> ProtocolContext {
        let peer: Arc<dyn PeerHandle> = Arc::new(NamedPeer(name));
        ProtocolContext::new(
            Era::V1,
            ProtocolVersion(crate::types::protocol::LATEST_PROTOCOL_VERSION.to_string()),
        )
        .with_transport_backchannel(TransportBackchannel::new().with_peer(peer))
    }

    fn extra_with_context(context: Option<ProtocolContext>) -> RequestHandlerExtra {
        RequestHandlerExtra::new(
            "req-attach-peer".to_string(),
            RequestHandlerExtra::default().cancellation_token,
        )
        .with_protocol_context(context)
    }

    fn bare_core() -> ServerCore {
        crate::server::builder::ServerCoreBuilder::new()
            .name("attach-peer-precedence-core")
            .version("1.0.0")
            .build()
            .expect("core builds")
    }

    /// THE precedence claim (T-118.1-11-04), measured on this dispatch root.
    #[tokio::test]
    async fn the_request_scoped_peer_wins_over_the_global_peer_handle() {
        let mut core = bare_core();
        core.peer_handle = Some(Arc::new(NamedPeer("global")));

        let extra = core.attach_peer(extra_with_context(Some(context_with_peer(
            "request-scoped",
        ))));

        assert_eq!(
            attached_peer_name(&extra).await.as_deref(),
            Some("request-scoped"),
            "ServerCore must resolve the peer exactly as `Server` does: the request-scoped \
             transport handle wins over the global one"
        );
    }

    /// The in-process fallback is untouched here too.
    #[tokio::test]
    async fn the_global_handle_still_applies_when_no_backchannel_rides_the_context() {
        let mut core = bare_core();
        core.peer_handle = Some(Arc::new(NamedPeer("global")));

        let extra = core.attach_peer(extra_with_context(None));
        assert_eq!(
            attached_peer_name(&extra).await.as_deref(),
            Some("global"),
            "with no backchannel the global handle must still apply"
        );
    }

    /// Neither source configured: still a no-op.
    #[tokio::test]
    async fn attach_peer_is_a_no_op_when_neither_source_is_configured() {
        let core = bare_core();
        let extra = core.attach_peer(extra_with_context(None));
        assert!(
            extra.peer().is_none(),
            "with no global handle and no backchannel, `extra.peer()` stays None"
        );
    }
}

// ===========================================================================
// `attach_request_log_sink` — the log-sink precedence rule, the log-level
// carrier, and the `ServerCore` root that calls them (Phase 118.2 plan 06,
// CONF-10 / D-07).
//
// These live HERE rather than in `tests/log_emitter.rs` for the same reason the
// peer suite above does, stated verbatim in `src/server/mod.rs`'s
// `peer_precedence_tests`: they need crate-internal access. All three of
// `attach_request_log_sink`, `TransportBackchannel` and
// `ProtocolContext::with_resolved_log_level` are `pub(crate)`, so an integration
// test cannot construct the request-scoped half of the precedence rule at all.
// `tests/log_emitter.rs` carries the STRUCTURAL both-roots fence that guards
// these from silent deletion.
//
// The `Server` twin of this module is `log_sink_precedence_tests` in
// `src/server/mod.rs` — same claims, other root, so a future edit to one impl
// cannot silently diverge from the other.
// ===========================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
mod core_log_sink_tests {
    use super::*;
    use crate::types::protocol::context::TransportBackchannel;
    use crate::types::protocol::{Era, ProtocolContext, ProtocolVersion};
    use crate::types::{LoggingLevel, Notification};
    use std::sync::Mutex;

    /// A sink that records every notification handed to it, so a fence can tell
    /// WHICH of two sinks a record reached rather than only that one did.
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

    /// A 2026-07-28 context carrying a live back-channel — the shape SEP-2575
    /// governs.
    fn v2_context_with_sink(capture: &Capture) -> ProtocolContext {
        ProtocolContext::new(
            Era::V2,
            ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
        )
        .with_transport_backchannel(
            TransportBackchannel::new().with_notification_sink(capture.sink()),
        )
    }

    /// THE SEP-2575 claim: on v2, an ABSENT `_meta` log level is a PROHIBITION.
    ///
    /// The vendored schema is explicit — "If absent, the server MUST NOT send any
    /// notifications/message" — and the conformance scenario
    /// `sep-2575-server-no-log-without-loglevel` reds the server on a single
    /// frame. Before this rule, `resolve_request_log_level` answered `None`, the
    /// emitter fell back to `DEFAULT_LOG_LEVEL` (`info`, D-12) and emitted, so
    /// every pmcp server violated it; the only thing standing between the suite
    /// and a red was a hand-written `if extra.log_level.is_some()` guard inside
    /// one example's handler.
    #[test]
    fn a_v2_request_with_no_resolved_level_gets_no_log_sink_at_all() {
        let capture = Capture::default();
        let extra = attach_request_log_sink(
            extra_with_context(Some(v2_context_with_sink(&capture))),
            || None,
        );

        assert!(
            extra.log_sink.is_none(),
            "a v2 request that carried no `io.modelcontextprotocol/logLevel` must be given NO \
             vehicle: withholding the sink is what makes the rule hold for every handler rather \
             than only the ones that remember to check"
        );
        extra
            .log(LoggingLevel::Error, "this must not reach the wire")
            .expect("a sinkless emit is silence, not an error (D-08)");
        assert_eq!(
            capture.len(),
            0,
            "not one `notifications/message` frame — the scenario fails the server on the first"
        );
    }

    /// The mirror: a v2 request that DID authorize logging still gets its sink,
    /// so the rule is a gate and not a blanket refusal.
    #[test]
    fn a_v2_request_that_authorized_logging_still_gets_its_sink() {
        let capture = Capture::default();
        let context = v2_context_with_sink(&capture).with_resolved_log_level(LoggingLevel::Debug);
        let extra = attach_request_log_sink(extra_with_context(Some(context)), || None);

        assert_eq!(
            extra.log_level,
            Some(LoggingLevel::Debug),
            "the authorized level still reaches the emitter"
        );
        extra
            .log(LoggingLevel::Debug, "authorized")
            .expect("the emitter always returns Ok");
        assert_eq!(
            capture.len(),
            1,
            "and the record is delivered — v2 logging is GATED, not disabled"
        );
    }

    /// v1 is untouched: there, `None` means "the client did not choose, so the
    /// server MAY decide", which is exactly what `DEFAULT_LOG_LEVEL` implements.
    /// Reading the two eras the same way is what the fix would break if it were
    /// applied unconditionally.
    #[test]
    fn a_v1_request_with_no_resolved_level_still_logs_at_the_default() {
        let capture = Capture::default();
        let extra = attach_request_log_sink(
            extra_with_context(Some(context_with_sink(&capture))),
            || None,
        );

        assert!(
            extra.log_sink.is_some(),
            "v1 absence is a non-answer, not a prohibition"
        );
        extra
            .log(LoggingLevel::Info, "info is at the default bar")
            .expect("the emitter always returns Ok");
        assert_eq!(
            capture.len(),
            1,
            "MCP 2025-11-25 says the server MAY decide which messages to send automatically"
        );
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

    fn bare_core() -> ServerCore {
        crate::server::builder::ServerCoreBuilder::new()
            .name("attach-log-sink-precedence-core")
            .version("1.0.0")
            .build()
            .expect("core builds")
    }

    /// THE precedence claim (T-118.2-06-02). Both sources configured at once.
    ///
    /// Two DISTINGUISHABLE captures, not one: a fence that only proved "a record
    /// arrived somewhere" could not tell the correct routing from the inverted
    /// one, and the inverted one publishes a session's records onto the
    /// server-wide channel.
    #[test]
    fn a_request_scoped_sink_wins_over_the_root_fallback() {
        let request_scoped = Capture::default();
        let fallback = Capture::default();

        let extra = attach_request_log_sink(
            extra_with_context(Some(context_with_sink(&request_scoped))),
            || Some(fallback.sink()),
        );
        extra
            .log(LoggingLevel::Warning, "which sink received me?")
            .expect("the emitter always returns Ok");

        assert_eq!(
            request_scoped.len(),
            1,
            "the session-bound transport sink must win: the root fallback is a SINGLE server-wide \
             channel and cannot express which session issued this request (T-118.2-06-02)"
        );
        assert_eq!(
            fallback.len(),
            0,
            "the root fallback must NOT also receive the record — that would publish one \
             session's log records onto the server-wide channel"
        );
    }

    /// The mirror: with no backchannel, the root's fallback is what runs.
    #[test]
    fn the_root_fallback_is_used_when_no_request_scoped_sink_exists() {
        let fallback = Capture::default();

        let extra = attach_request_log_sink(extra_with_context(Some(bare_context())), || {
            Some(fallback.sink())
        });
        extra
            .log(LoggingLevel::Warning, "no backchannel on this request")
            .expect("the emitter always returns Ok");

        assert_eq!(
            fallback.len(),
            1,
            "with no request-scoped sink the root fallback must apply — that is the live path on \
             the in-process `Server::run` transport, not a safety net"
        );
    }

    /// Neither source configured: still a no-op, and `extra.log(..)` is silent
    /// rather than an error (D-08).
    #[test]
    fn attach_request_log_sink_is_a_no_op_when_neither_source_exists() {
        let extra = attach_request_log_sink(extra_with_context(Some(bare_context())), || None);
        assert!(
            extra.log_sink.is_none(),
            "with no fallback and no backchannel, `extra.log_sink` stays None"
        );
        assert!(
            extra.log(LoggingLevel::Error, "into the void").is_ok(),
            "a sinkless emit is silence, not an error (D-08)"
        );
    }

    /// The CARRIER fence: a level resolved onto the `ProtocolContext` reaches
    /// the emitter, and its absence leaves `DEFAULT_LOG_LEVEL` in force.
    ///
    /// This runs BEFORE any writer of `with_resolved_log_level` exists (the HTTP
    /// ingress lands in plan 07), which is the point: a broken carrier cannot
    /// hide behind a missing writer.
    #[test]
    fn a_resolved_log_level_on_the_context_reaches_the_extra() {
        let with_level = Capture::default();
        let context = context_with_sink(&with_level).with_resolved_log_level(LoggingLevel::Debug);
        let extra = attach_request_log_sink(extra_with_context(Some(context)), || None);

        assert_eq!(
            extra.log_level,
            Some(LoggingLevel::Debug),
            "the resolved level must be lifted off the context by the SAME unit that resolves the \
             sink — attaching them at different sites is how two roots end up filtering differently"
        );
        extra
            .log(LoggingLevel::Debug, "debug is at the bar now")
            .expect("the emitter always returns Ok");
        assert_eq!(
            with_level.len(),
            1,
            "with `debug` resolved, a debug record must pass the filter"
        );

        // The mirror: no resolved level, so D-12's `info` default applies and the
        // same debug record is dropped.
        let defaulted = Capture::default();
        let extra = attach_request_log_sink(
            extra_with_context(Some(context_with_sink(&defaulted))),
            || None,
        );
        assert!(
            extra.log_level.is_none(),
            "with nothing resolved the unit must leave `log_level` alone so DEFAULT_LOG_LEVEL \
             applies at emit time"
        );
        extra
            .log(LoggingLevel::Debug, "below the default bar")
            .expect("the emitter always returns Ok");
        assert_eq!(
            defaulted.len(),
            0,
            "an unconfigured request filters at `info` (D-12), so a debug record is dropped"
        );
    }

    /// The ROOT claim on this side: `ServerCore::attach_peer` — the one
    /// post-authorization site every `ServerCore` dispatcher calls — wires the
    /// log sink, not just the peer.
    #[test]
    fn the_server_core_root_attaches_the_request_scoped_log_sink() {
        let capture = Capture::default();
        let core = bare_core();

        let extra = core.attach_peer(extra_with_context(Some(context_with_sink(&capture))));
        extra
            .log(LoggingLevel::Info, "through the ServerCore root")
            .expect("the emitter always returns Ok");

        assert_eq!(
            capture.len(),
            1,
            "`ServerCore::attach_peer` must attach the log sink too — it is the single site every \
             dispatcher on this root already calls, AFTER its `tool_authorizer` check"
        );
    }

    /// The documented asymmetry, pinned: `ServerCore` has no `notification_tx`
    /// of any kind, so its fallback is a literal `None` and a request with no
    /// backchannel gets no sink at all. A future reader who "fixes" the literal
    /// `None` by inventing a channel here breaks this.
    #[test]
    fn the_server_core_root_has_no_fallback_sink_of_its_own() {
        let core = bare_core();
        let extra = core.attach_peer(extra_with_context(Some(bare_context())));
        assert!(
            extra.log_sink.is_none(),
            "`ServerCore` owns no notification channel; the request-scoped `TransportBackchannel` \
             is its ONLY possible sink"
        );
    }
}

// ===========================================================================
// `logging/setLevel` — ONE era-branched answer, BOTH native dispatch roots
// (Phase 118.2 plan 08, CONF-10 / D-13).
//
// # Why these live HERE and not in `tests/log_emitter.rs`
//
// The plan puts all of this phase's fences in the integration file. Two of them
// CANNOT be written there, for a reason that is itself part of the finding:
//
//   * `ClientRequest::SetLoggingLevel` carries no `_meta` — it is one of the
//     variants `request_meta_value` enumerates as non-`_meta`-bearing — so
//     `ProtocolHandler::handle_request`, the only PUBLIC dispatch entry, always
//     resolves `era == None` for it and can exercise the v1 branch ONLY. An
//     integration test literally cannot present a v2 `logging/setLevel` to a
//     native root.
//   * The era-bearing seams are `Server::handle_request_with_context`
//     (`pub(crate)`) and `ServerCore::handle_request_internal` (private to this
//     file). This file is the only place that can reach BOTH, which makes it the
//     only place the D-13 twin-root claim can be stated as a test at all.
//
// Same placement, and the same stated reason, as `core_log_sink_tests` above and
// 118.1's `attach_peer` suite. `tests/log_emitter.rs` carries the WIRE fence for
// the v1 answer, the source-level scope fence over the untouched residual, and a
// name-existence guard so these two cannot be deleted silently.
// ===========================================================================
#[cfg(all(test, not(target_arch = "wasm32")))]
mod set_logging_level_tests {
    use super::*;
    use crate::types::protocol::{Era, ProtocolContext, ProtocolVersion};

    /// A comparable projection of a dispatch answer.
    ///
    /// `Ok(result)` or `Err((code, message))` — enough to state "the two roots
    /// gave the SAME answer" as an equality rather than as two separate
    /// assertions that could both drift in the same direction.
    fn answer_of(response: JSONRPCResponse) -> std::result::Result<Value, (i32, String)> {
        match response.payload {
            ResponsePayload::Result(value) => Ok(value),
            ResponsePayload::Error(error) => Err((error.code, error.message)),
        }
    }

    fn set_level_request() -> Request {
        Request::Client(Box::new(ClientRequest::SetLoggingLevel {
            level: crate::types::notifications::LoggingLevel::Info,
        }))
    }

    fn context_for(era: Era) -> ProtocolContext {
        let version = match era {
            Era::V1 => "2025-11-25",
            Era::V2 => "2026-07-28",
        };
        ProtocolContext::new(era, ProtocolVersion(version.to_string()))
    }

    /// A core in STATELESS mode.
    ///
    /// Not decoration: `v1_initialize_gate_applies` refuses every v1 request on a
    /// stateful core that has not seen `initialize`, and that `-32002` would be
    /// answered BEFORE the arm under test. Stateless mode removes the gate so the
    /// fence measures the dispatch arm rather than the handshake.
    fn bare_core() -> ServerCore {
        crate::server::builder::ServerCoreBuilder::new()
            .name("set-logging-level-core")
            .version("1.0.0")
            .stateless_mode(true)
            .build()
            .expect("core builds")
    }

    fn bare_server() -> crate::server::Server {
        crate::server::Server::builder()
            .name("set-logging-level-server")
            .version("1.0.0")
            .build()
            .expect("server builds")
    }

    /// The `ServerCore` root's answer for `era`.
    async fn core_answer(era: Option<Era>) -> std::result::Result<Value, (i32, String)> {
        let core = bare_core();
        answer_of(
            core.handle_request_internal(
                RequestId::from(1i64),
                set_level_request(),
                None,
                era.map(context_for),
                &mut DispatchEnvelopeClaim::default(),
            )
            .await,
        )
    }

    /// The high-level `Server` root's answer for `era`.
    async fn server_answer(era: Option<Era>) -> std::result::Result<Value, (i32, String)> {
        let server = bare_server();
        answer_of(
            server
                .handle_request_with_context(
                    RequestId::from(1i64),
                    set_level_request(),
                    None,
                    era.map(context_for),
                )
                .await,
        )
    }

    /// T-118.2-08-01 — the v2 retirement is enforced at the DISPATCH root, not
    /// only at the HTTP ingress.
    ///
    /// # Which route this fence takes, and why
    ///
    /// IN-PROCESS, through each root's own era-bearing dispatch entry — NOT over
    /// HTTP. `retire_v2_method` refuses a v2 `logging/setLevel` at the header
    /// gate, long before any dispatcher sees it, so a wire fence would prove the
    /// PRE-EXISTING transport retirement and say nothing at all about the arm
    /// this plan added. The whole of D-13 is that the dispatch root was the layer
    /// that had it wrong: before this plan `Server` answered `json!({})` here on
    /// BOTH eras, so a caller reaching it by any path other than the HTTP ingress
    /// — in-process, or a future transport — got a success for a method the
    /// 2026-07-28 schema removed.
    #[tokio::test]
    async fn v2_set_logging_level_is_retired_on_the_dispatch_root() {
        for (root, answer) in [
            ("Server", server_answer(Some(Era::V2)).await),
            ("ServerCore", core_answer(Some(Era::V2)).await),
        ] {
            let Err((code, message)) = answer else {
                panic!(
                    "{root} SERVED a v2 `logging/setLevel` instead of refusing it — the retirement \
                     is enforced only at the HTTP layer again, which is exactly the defect D-13 \
                     closed"
                );
            };
            assert_eq!(
                code,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                "{root} refused with {code} ({message}), not -32601. The HTTP ingress answers \
                 `METHOD_NOT_FOUND` for the same method through `V2_RETIRED_METHODS`; a refusal \
                 that is not a retirement would not be credited by the suite and could be a \
                 capability or params rejection wearing a retirement's clothes"
            );
        }
    }

    /// The D-13 claim itself, as an equality: both roots, every era, one answer.
    ///
    /// The `None` row is load-bearing. A server that never opted in to protocol
    /// negotiation resolves no era at all, and it has always been on v1; retiring
    /// the method for it would break every default-configuration server in the
    /// SDK. `None` must therefore answer exactly as `Some(V1)` does.
    ///
    /// The sibling precedent is the paired
    /// `attach_peer_is_a_no_op_when_neither_source_is_configured`, which exists in
    /// BOTH `src/server/core.rs` and `src/server/mod.rs`. This phase states the
    /// pairing as one cross-root equality instead, because "the two answers are
    /// EQUAL" is the property, and two mirrored tests can drift together while an
    /// equality cannot.
    #[tokio::test]
    async fn both_dispatch_roots_agree_about_set_logging_level() {
        for era in [None, Some(Era::V1), Some(Era::V2)] {
            let server = server_answer(era).await;
            let core = core_answer(era).await;
            assert_eq!(
                server, core,
                "the two native dispatch roots disagree about `logging/setLevel` on era {era:?} — \
                 `Server` said {server:?}, `ServerCore` said {core:?}. That divergence is the \
                 whole defect D-13 closed: only one of these roots is on the HTTP path, so only \
                 one of them is measured by the official conformance suite, and the other one is \
                 what an in-process or future-transport caller gets"
            );
        }
    }

    /// Pitfall 8, at the dispatch root: the v1 answer is a LITERAL empty object.
    ///
    /// The pinned suite's `2025-11-25:logging-set-level` scenario fails on
    /// `Object.keys(r).length > 0`, so an acknowledgement object — or an echo of
    /// the level just set, which would additionally be a READ of session state
    /// through a WRITE endpoint (T-118.2-08-02) — reds a currently-green blocking
    /// scenario. `tests/log_emitter.rs` pins the same constraint over the wire.
    #[tokio::test]
    async fn the_v1_answer_is_an_object_with_zero_keys_on_both_roots() {
        for era in [None, Some(Era::V1)] {
            for (root, answer) in [
                ("Server", server_answer(era).await),
                ("ServerCore", core_answer(era).await),
            ] {
                let Ok(result) = answer else {
                    panic!("{root} refused a v1 `logging/setLevel` on era {era:?}: {answer:?}");
                };
                let object = result.as_object().unwrap_or_else(|| {
                    panic!("{root}: the v1 answer must be an OBJECT, got {result}")
                });
                assert!(
                    object.is_empty(),
                    "{root}: the v1 answer must have ZERO keys, got {result} — any non-empty \
                     object fails the pinned suite's `logging-set-level` scenario"
                );
            }
        }
    }
}
