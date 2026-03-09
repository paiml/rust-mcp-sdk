//! Transport-independent MCP server core implementation.
//!
//! This module provides the core server functionality that is decoupled from
//! transport mechanisms, enabling deployment to various environments including
//! WASM/WASI targets.

use crate::error::{Error, Result};
use crate::shared::middleware::{EnhancedMiddlewareChain, MiddlewareContext};
use crate::shared::protocol_helpers::{create_notification, create_request};
use crate::types::jsonrpc::ResponsePayload;
use crate::types::{
    CallToolParams, CallToolResult, ClientCapabilities, ClientRequest, Content, GetPromptParams,
    GetPromptResult, Implementation, InitializeParams, InitializeResult, JSONRPCError,
    JSONRPCResponse, ListPromptsParams, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesParams, ListResourcesResult, ListToolsParams,
    ListToolsResult, Notification, PromptInfo, ProtocolVersion, ReadResourceParams,
    ReadResourceResult, Request, RequestId, ServerCapabilities, ToolInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
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

/// Build a URI-to-tool-meta index from registered tool metadata.
///
/// Maps resource URIs (from `openai/outputTemplate`) to the linked tool's
/// `openai/*` `_meta` keys. Used to auto-propagate widget descriptor keys
/// onto `ResourceInfo` during `resources/list` and `resources/read`.
/// When multiple tools share the same URI, first tool registered wins.
pub(crate) fn build_uri_to_tool_meta(
    tool_infos: &HashMap<String, ToolInfo>,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for info in tool_infos.values() {
        if let Some(meta) = info.widget_meta() {
            if let Some(serde_json::Value::String(uri)) = meta.get("openai/outputTemplate") {
                // First tool registered wins (per user decision)
                map.entry(uri.clone())
                    .or_insert_with(|| crate::types::ui::filter_to_descriptor_keys(meta));
            }
        }
    }
    map
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
    /// Maps resource URIs to their linked tool's `openai/*` `_meta` keys.
    uri_to_tool_meta: HashMap<String, serde_json::Map<String, serde_json::Value>>,

    /// Cached prompt metadata (populated at registration, immutable)
    prompt_infos: HashMap<String, PromptInfo>,

    /// Resource handler (optional)
    resources: Option<Arc<dyn ResourceHandler>>,

    /// Sampling handler (optional)
    sampling: Option<Arc<dyn SamplingHandler>>,

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
        stateless_mode: bool,
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
            stateless_mode,
        }
    }

    /// Check if the server is initialized.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get client capabilities if available.
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Handle initialization request.
    async fn handle_initialize(&self, init_req: &InitializeParams) -> Result<InitializeResult> {
        // Store client capabilities
        *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
        *self.initialized.write().await = true;

        // Negotiate protocol version
        let negotiated_version =
            if crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&init_req.protocol_version.as_str()) {
                init_req.protocol_version.clone()
            } else {
                crate::DEFAULT_PROTOCOL_VERSION.to_string()
            };

        Ok(InitializeResult {
            protocol_version: ProtocolVersion(negotiated_version),
            capabilities: self.capabilities.clone(),
            server_info: self.info.clone(),
            instructions: None,
        })
    }

    /// Handle list tools request.
    async fn handle_list_tools(&self, _req: &ListToolsParams) -> Result<ListToolsResult> {
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    /// Handle call tool request.
    async fn handle_call_tool(
        &self,
        req: &CallToolParams,
        auth_context: Option<AuthContext>,
    ) -> Result<CallToolResult> {
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

        // Create request handler extra data with auth_context
        let request_id = format!("tool_{}", req.name);
        let mut extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

        // Execute tool with or without middleware depending on platform
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            // Create tool context for middleware
            let context = ToolContext::new(&req.name, &request_id);

            // Clone arguments for middleware processing
            let mut args = req.arguments.clone();

            // Process request through tool middleware chain
            // Middleware rejection short-circuits tool execution (on_error already called by chain)
            self.tool_middleware
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Execute the tool with potentially modified args and extra
            let mut result = handler.handle(args, extra).await;

            // Process response through tool middleware chain
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
        };

        #[cfg(target_arch = "wasm32")]
        let result = {
            // On WASM, execute tool directly without middleware
            let args = req.arguments.clone();
            handler.handle(args, extra).await
        };

        // Convert result to CallToolResult
        let value = result?;
        let text = serde_json::to_string_pretty(&value)?;
        let mut call_result = CallToolResult::new(vec![Content::Text { text }]);

        // Enrich with widget metadata so widgets can access data via toolOutput.
        if let Some(info) = self.tool_infos.get(&req.name) {
            call_result = call_result.with_widget_enrichment(info, value);
        }

        Ok(call_result)
    }

    /// Handle list prompts request.
    async fn handle_list_prompts(&self, _req: &ListPromptsParams) -> Result<ListPromptsResult> {
        let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();

        tracing::debug!(
            target: "mcp.prompts",
            count = prompts.len(),
            "Returning prompts"
        );

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    /// Handle get prompt request.
    async fn handle_get_prompt(
        &self,
        req: &GetPromptParams,
        auth_context: Option<AuthContext>,
    ) -> Result<GetPromptResult> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Prompt '{}' not found", req.name)))?;

        // Create request handler extra data with auth_context
        let request_id = format!("prompt_{}", req.name);
        let extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

        handler.handle(req.arguments.clone(), extra).await
    }

    /// Handle list resources request.
    async fn handle_list_resources(
        &self,
        req: &ListResourcesParams,
        auth_context: Option<AuthContext>,
    ) -> Result<ListResourcesResult> {
        let mut result = match &self.resources {
            Some(handler) => {
                let request_id = "list_resources".to_string();
                let extra = RequestHandlerExtra::new(
                    request_id.clone(),
                    self.cancellation_manager
                        .create_token(request_id.clone())
                        .await,
                )
                .with_auth_context(auth_context);
                handler.list(req.cursor.clone(), extra).await?
            },
            None => ListResourcesResult {
                resources: vec![],
                next_cursor: None,
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
        req: &ReadResourceParams,
        auth_context: Option<AuthContext>,
    ) -> Result<ReadResourceResult> {
        let handler = self.resources.as_ref().ok_or_else(|| {
            Error::internal(format!("Resource handler not available for '{}'", req.uri))
        })?;

        let request_id = format!("read_{}", req.uri);
        let extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

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
        })
    }

    /// Create an error response.
    fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
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

    /// Create a success response.
    fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
        JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            payload: ResponsePayload::Result(result),
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
            return Self::error_response(id, -32603, e.to_string());
        }

        // Execute the actual request handling with auth_context
        let mut response = self
            .handle_request_internal(id.clone(), request, auth_context)
            .await;

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
    /// Resolve the owner ID from the authentication context using the task router.
    ///
    /// Returns `None` if no task router is configured. When a task router is
    /// available, it delegates to [`TaskRouter::resolve_owner`] which uses
    /// the priority chain: OAuth subject > client ID > session ID > "local".
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_task_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
        let router = self.task_router.as_ref()?;
        Some(match auth_context {
            Some(ctx) => router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None),
            None => router.resolve_owner(None, None, None),
        })
    }

    /// Internal request handler without middleware processing.
    async fn handle_request_internal(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse {
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };

                match self.handle_initialize(init_req).await {
                    Ok(result) => Self::success_response(id, serde_json::to_value(result).unwrap()),
                    Err(e) => Self::error_response(id, -32603, e.to_string()),
                }
            },
            Request::Client(ref boxed_req) => {
                // Check if server is initialized for server requests (skip in stateless mode)
                // Stateless mode is for serverless deployments where each request may create
                // a fresh server instance (AWS Lambda, Cloudflare Workers, etc.)
                if !self.stateless_mode && !self.is_initialized().await {
                    return Self::error_response(
                        id,
                        -32002,
                        "Server not initialized. Call initialize first.".to_string(),
                    );
                }

                match boxed_req.as_ref() {
                    ClientRequest::ListTools(req) => match self.handle_list_tools(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::CallTool(req) => {
                        // Check for task-augmented call: explicit task field or tool requires task
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(ref task_router) = self.task_router {
                            // Determine if this tool requires task augmentation
                            let tool_execution = self
                                .tool_infos
                                .get(&req.name)
                                .and_then(|m| m.execution.as_ref());
                            let needs_task = req.task.is_some()
                                || task_router.tool_requires_task(&req.name, tool_execution);
                            if needs_task {
                                let owner_id = self
                                    .resolve_task_owner(auth_context.as_ref())
                                    .unwrap_or_else(|| "local".to_string());
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
                                    Err(e) => Self::error_response(id, -32603, e.to_string()),
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

                        match self.handle_call_tool(req, auth_context.clone()).await {
                            Ok(result) => {
                                // Fire-and-forget workflow continuation recording
                                #[cfg(not(target_arch = "wasm32"))]
                                if let (Some((task_id, tool_name)), Some(ref task_router)) =
                                    (continuation_ctx, &self.task_router)
                                {
                                    let owner_id = self
                                        .resolve_task_owner(auth_context.as_ref())
                                        .unwrap_or_else(|| "local".to_string());
                                    let tool_result_value =
                                        serde_json::to_value(&result).unwrap_or_default();
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
                                }
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListPrompts(req) => match self.handle_list_prompts(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::GetPrompt(req) => {
                        match self.handle_get_prompt(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListResources(req) => {
                        match self.handle_list_resources(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ReadResource(req) => {
                        match self.handle_read_resource(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListResourceTemplates(req) => {
                        match self.handle_list_resource_templates(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    // Task endpoint routing (experimental MCP Tasks)
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::TasksGet(params) => {
                        if let Some(ref task_router) = self.task_router {
                            let owner_id = self
                                .resolve_task_owner(auth_context.as_ref())
                                .unwrap_or_else(|| "local".to_string());
                            match task_router
                                .handle_tasks_get(params.clone(), &owner_id)
                                .await
                            {
                                Ok(result) => Self::success_response(id, result),
                                Err(e) => Self::error_response(id, -32603, e.to_string()),
                            }
                        } else {
                            Self::error_response(id, -32601, "Tasks not enabled".to_string())
                        }
                    },
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::TasksResult(params) => {
                        if let Some(ref task_router) = self.task_router {
                            let owner_id = self
                                .resolve_task_owner(auth_context.as_ref())
                                .unwrap_or_else(|| "local".to_string());
                            match task_router
                                .handle_tasks_result(params.clone(), &owner_id)
                                .await
                            {
                                Ok(result) => Self::success_response(id, result),
                                Err(e) => Self::error_response(id, -32603, e.to_string()),
                            }
                        } else {
                            Self::error_response(id, -32601, "Tasks not enabled".to_string())
                        }
                    },
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::TasksList(params) => {
                        if let Some(ref task_router) = self.task_router {
                            let owner_id = self
                                .resolve_task_owner(auth_context.as_ref())
                                .unwrap_or_else(|| "local".to_string());
                            match task_router
                                .handle_tasks_list(params.clone(), &owner_id)
                                .await
                            {
                                Ok(result) => Self::success_response(id, result),
                                Err(e) => Self::error_response(id, -32603, e.to_string()),
                            }
                        } else {
                            Self::error_response(id, -32601, "Tasks not enabled".to_string())
                        }
                    },
                    #[cfg(not(target_arch = "wasm32"))]
                    ClientRequest::TasksCancel(params) => {
                        if let Some(ref task_router) = self.task_router {
                            let owner_id = self
                                .resolve_task_owner(auth_context.as_ref())
                                .unwrap_or_else(|| "local".to_string());
                            match task_router
                                .handle_tasks_cancel(params.clone(), &owner_id)
                                .await
                            {
                                Ok(result) => Self::success_response(id, result),
                                Err(e) => Self::error_response(id, -32603, e.to_string()),
                            }
                        } else {
                            Self::error_response(id, -32601, "Tasks not enabled".to_string())
                        }
                    },
                    _ => Self::error_response(id, -32601, "Method not supported".to_string()),
                }
            },
            Request::Server(_) => {
                Self::error_response(id, -32601, "Method not supported".to_string())
            },
        }
    }

    /// Internal notification handler without middleware processing.
    async fn handle_notification_internal(&self, _notification: Notification) -> Result<()> {
        // Handle notifications if needed
        // Most notifications from client to server don't require action
        Ok(())
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
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
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
            false, // stateless_mode
        );

        assert!(!server.is_initialized().await);

        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
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
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
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
            false, // stateless_mode
        );

        // Initialize first
        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));
        server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

        // List tools
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
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
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
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
            true, // stateless_mode enabled
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
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
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
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
            false, // stateless_mode disabled (normal mode)
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
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
}
