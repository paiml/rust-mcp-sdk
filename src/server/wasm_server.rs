//! Environment-agnostic MCP server for WASM/WASI deployment.
//!
//! This module provides a properly typed MCP server that maintains type safety
//! while being deployable to any WASI environment.

use crate::error::{Error, Result};
use crate::types::{
    CallToolRequest, CallToolResult, ClientRequest, Content, GetPromptRequest, GetPromptResult,
    Implementation, InitializeRequest, InitializeResult, JSONRPCError, JSONRPCResponse,
    ListPromptsRequest, ListPromptsResult, ListResourcesRequest, ListResourcesResult,
    ListToolsRequest, ListToolsResult, PromptInfo, ReadResourceRequest, ReadResourceResult,
    Request, RequestId, ResourceInfo, ServerCapabilities, ToolInfo,
};
use crate::ErrorCode;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;

/// Serialize a result that extends `CacheableResult`, stripping the v2-only
/// caching hints on the way out (115-06, SCHM-03 / D-11).
///
/// # Why this exists at all
///
/// `WasmMcpServer` is the THIRD dispatcher, and it is not on the native
/// chokepoint. It carries no `ProtocolContext`, has no accept-list and performs
/// no era resolution of any kind, so it can never legitimately serve a v2
/// client — yet its `WasmResource` handlers construct `ReadResourceResult` /
/// `ListResourcesResult` values that this file serializes DIRECTLY. Once
/// 115-05 gave those types `with_ttl_ms` / `with_cache_scope` builders, a
/// handler could put `ttlMs` / `cacheScope` straight onto this server's v1
/// wire. D-11 forbids exactly that: a v1 wire never carries a v2 field.
///
/// `None` is therefore not a placeholder era — it is the CORRECT era for this
/// dispatcher, and it selects `project_caching_hints`'s STRIP arm.
/// `crate::types::caching` is deliberately `cfg`-free so this
/// `cfg(target_arch = "wasm32")` file can reach the very same projector that
/// `src/server/core.rs` (which is shaped for `not(wasm32)`) calls; a projector
/// living in either server module would be unreachable from the other.
///
/// # How this is proven
///
/// Three ways, none of which alone is sufficient:
///
/// 1. `make wasm-build` — COMPILE-time only. It does not catch removal of the
///    call: deleting it still builds clean (measured in 115-06 Task 2).
/// 2. `crate::types::caching`'s native unit test
///    `no_context_strips_both_keys_which_is_the_wasm_path` — the BEHAVIOUR of
///    the `None` arm. No native build or test compiles THIS file
///    (`src/server/mod.rs` gates it on `target_arch = "wasm32"`, and its
///    `cfg(all(test, target_arch = "wasm32"))` test module does not compile at
///    all), so that native test is the only runnable proof of the arm.
/// 3. 115-08's SOURCE tripwire — that these call sites still EXIST.
fn cacheable_result_to_value<T: Serialize>(result: T) -> Result<Value> {
    let mut value = serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))?;
    crate::types::caching::project_caching_hints(
        &mut value,
        None,
        crate::types::caching::Cacheable::Yes,
    );
    Ok(value)
}

/// A tool that can be executed in WASM environments.
pub trait WasmTool: Send + Sync {
    /// Execute the tool with given arguments.
    fn execute(&self, args: Value) -> Result<Value>;

    /// Get the tool's schema/description.
    fn info(&self) -> ToolInfo;
}

/// A resource that can be accessed in WASM environments.
pub trait WasmResource: Send + Sync {
    /// Read the resource at the given URI.
    fn read(&self, uri: &str) -> Result<ReadResourceResult>;

    /// List available resources.
    fn list(&self, cursor: Option<String>) -> Result<ListResourcesResult>;

    /// Get resource templates if any.
    fn templates(&self) -> Vec<ResourceInfo> {
        Vec::new()
    }
}

/// A prompt that can be generated in WASM environments.
pub trait WasmPrompt: Send + Sync {
    /// Generate a prompt with the given arguments.
    fn generate(&self, args: HashMap<String, String>) -> Result<GetPromptResult>;

    /// Get the prompt's information.
    fn info(&self) -> PromptInfo;
}

/// Environment-agnostic MCP server for WASM deployment.
///
/// This server maintains full type safety while being deployable to any
/// WASI environment (Cloudflare Workers, Fermyon Spin, Wasmtime, etc).
pub struct WasmMcpServer {
    info: Implementation,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Box<dyn WasmTool>>,
    resources: HashMap<String, Box<dyn WasmResource>>,
    prompts: HashMap<String, Box<dyn WasmPrompt>>,
    /// Cached tool metadata (populated at registration, avoids per-request cloning)
    tool_infos: HashMap<String, ToolInfo>,
    /// Cached prompt metadata (populated at registration, avoids per-request cloning)
    prompt_infos: HashMap<String, PromptInfo>,
}

impl WasmMcpServer {
    /// Create a new WASM MCP server.
    pub fn builder() -> WasmMcpServerBuilder {
        WasmMcpServerBuilder::new()
    }

    /// Map error types to appropriate JSON-RPC error codes.
    fn map_error_code(error: &Error) -> ErrorCode {
        // Check if it's a Protocol error with a specific code
        match error {
            Error::Protocol { code, .. } => *code,
            _ => ErrorCode::INTERNAL_ERROR,
        }
    }

    /// Handle an MCP request with full type safety.
    pub async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse {
        let result = match request {
            Request::Client(client_req) => self.handle_client_request(*client_req).await,
            Request::Server(_) => Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Server requests not supported in WASM",
            )),
        };

        match result {
            Ok(value) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Result(value),
            },
            Err(error) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(JSONRPCError {
                    code: Self::map_error_code(&error).0,
                    message: error.to_string(),
                    data: None,
                }),
            },
        }
    }

    async fn handle_client_request(&self, request: ClientRequest) -> Result<Value> {
        match request {
            ClientRequest::Initialize(params) => self.handle_initialize(params),
            ClientRequest::ListTools(params) => self.handle_list_tools(params),
            ClientRequest::CallTool(params) => self.handle_call_tool(params),
            ClientRequest::ListResources(params) => self.handle_list_resources(params),
            ClientRequest::ReadResource(params) => self.handle_read_resource(params),
            ClientRequest::ListPrompts(params) => self.handle_list_prompts(params),
            ClientRequest::GetPrompt(params) => self.handle_get_prompt(params),
            _ => Err(Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                "Method not supported in WASM",
            )),
        }
    }

    fn handle_initialize(&self, params: InitializeRequest) -> Result<Value> {
        let negotiated_version = crate::negotiate_protocol_version(&params.protocol_version);

        let result = InitializeResult {
            protocol_version: crate::types::ProtocolVersion(negotiated_version.to_string()),
            capabilities: self.capabilities.clone(),
            server_info: self.info.clone(),
            instructions: None,
        };
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }

    fn handle_list_tools(&self, _params: ListToolsRequest) -> Result<Value> {
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        // `ListToolsResult` extends `CacheableResult` — route it through the
        // stripping serializer (D-11). Dispatcher-BUILT rather than
        // handler-returned, so nothing can be set here today; it goes through
        // the same helper anyway so the set of cacheable sites is uniform and
        // a future `WasmTool`-driven list cannot quietly become a leak.
        let result = ListToolsResult {
            tools,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        };
        cacheable_result_to_value(result)
    }

    fn handle_call_tool(&self, params: CallToolRequest) -> Result<Value> {
        let tool = self.tools.get(&params.name).ok_or_else(|| {
            Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                &format!("Tool '{}' not found", params.name),
            )
        })?;

        let args = params.arguments.clone();
        match tool.execute(args) {
            Ok(result_value) => {
                // Determine content type based on the result structure
                let content = if let Some(text) = result_value.as_str() {
                    vec![Content::text(text)]
                } else if result_value.is_object() {
                    // For structured data, wrap in a content type that preserves structure
                    vec![Content::text(
                        serde_json::to_string_pretty(&result_value)
                            .unwrap_or_else(|_| "{}".to_string()),
                    )]
                } else {
                    vec![Content::text(result_value.to_string())]
                };

                let result = CallToolResult::new(content);
                serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
            },
            Err(e) => {
                // An application-level rejection (e.g. Code Mode policy: a
                // SELECT missing its LIMIT) carries a model-actionable message
                // and structured detail — route it through the shared
                // `rejected` envelope so the wasm transport matches the native
                // paths (message → content, details → structuredContent),
                // rather than flattening it to a prefixed `Error: …` string.
                let result = match e {
                    Error::ToolRejected { message, details } => {
                        CallToolResult::rejected(message, details)
                    },
                    other => CallToolResult::error(vec![Content::text(format!("Error: {other}"))]),
                };
                serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
            },
        }
    }

    fn handle_list_resources(&self, params: ListResourcesRequest) -> Result<Value> {
        // Aggregate resources from all providers with cursor support
        let mut all_resources = Vec::new();
        let mut next_cursor = None;

        // Parse cursor to determine which provider to query
        let (provider_name, provider_cursor) = if let Some(cursor) = params.cursor {
            // Format: "provider:cursor" or just "cursor" for first provider
            if let Some((name, cur)) = cursor.split_once(':') {
                (Some(name.to_string()), Some(cur.to_string()))
            } else {
                (None, Some(cursor))
            }
        } else {
            (None, None)
        };

        // Query the appropriate provider(s)
        let mut found_provider = provider_name.is_none();
        for (name, resource) in &self.resources {
            if let Some(ref pname) = provider_name {
                if name != pname {
                    continue;
                }
            }

            if found_provider {
                match resource.list(provider_cursor.clone()) {
                    Ok(result) => {
                        all_resources.extend(result.resources);
                        if let Some(cursor) = result.next_cursor {
                            next_cursor = Some(format!("{}:{}", name, cursor));
                        }
                        break; // Only query one provider at a time for pagination
                    },
                    Err(_) => continue,
                }
            }

            if provider_name.is_none() {
                found_provider = true;
            }
        }

        // `ListResourcesResult` extends `CacheableResult` — route it through the
        // stripping serializer (D-11). This aggregation re-BUILDS the result
        // from each provider's `resources` / `next_cursor`, so a hint a
        // `WasmResource::list` implementation set is already dropped by the
        // rebuild; the strip is the belt-and-braces that survives someone later
        // deciding to forward the provider's result wholesale.
        let result = ListResourcesResult {
            resources: all_resources,
            next_cursor,
            ttl_ms: None,
            cache_scope: None,
        };
        cacheable_result_to_value(result)
    }

    fn handle_read_resource(&self, params: ReadResourceRequest) -> Result<Value> {
        // Find the first resource that can handle this URI
        for resource in self.resources.values() {
            if let Ok(result) = resource.read(&params.uri) {
                // THE leak site: `ReadResourceResult` extends `CacheableResult`
                // and this value is HANDLER-RETURNED, serialized verbatim with
                // no rebuild in between. A `WasmResource::read` that called
                // `with_cache_scope(CacheScope::Public)` would otherwise put a
                // v2-only key on this era-less dispatcher's v1 wire (D-11,
                // T-115-36). The stripping serializer is what closes it.
                return cacheable_result_to_value(result);
            }
        }
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            &format!("No resource handler for URI: {}", params.uri),
        ))
    }

    fn handle_list_prompts(&self, _params: ListPromptsRequest) -> Result<Value> {
        let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();

        // `ListPromptsResult` extends `CacheableResult` — route it through the
        // stripping serializer (D-11). Dispatcher-built, like `tools/list`.
        let result = ListPromptsResult {
            prompts,
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
        };
        cacheable_result_to_value(result)
    }

    fn handle_get_prompt(&self, params: GetPromptRequest) -> Result<Value> {
        let prompt = self.prompts.get(&params.name).ok_or_else(|| {
            Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                &format!("Prompt '{}' not found", params.name),
            )
        })?;

        let result = prompt.generate(params.arguments.clone())?;
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }
}

/// Builder for WasmMcpServer.
pub struct WasmMcpServerBuilder {
    name: String,
    version: String,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Box<dyn WasmTool>>,
    resources: HashMap<String, Box<dyn WasmResource>>,
    prompts: HashMap<String, Box<dyn WasmPrompt>>,
    /// Cached tool metadata (populated at registration, avoids per-request cloning)
    tool_infos: HashMap<String, ToolInfo>,
    /// Cached prompt metadata (populated at registration, avoids per-request cloning)
    prompt_infos: HashMap<String, PromptInfo>,
}

impl WasmMcpServerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            name: "wasm-mcp-server".to_string(),
            version: "1.0.0".to_string(),
            capabilities: ServerCapabilities::default(),
            tools: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
            tool_infos: HashMap::new(),
            prompt_infos: HashMap::new(),
        }
    }

    /// Set the server name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the server version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set server capabilities.
    pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Add a tool to the server.
    pub fn tool<T: WasmTool + 'static>(mut self, name: impl Into<String>, tool: T) -> Self {
        let name = name.into();
        // Cache metadata at registration time before moving ownership
        let info = tool.info();
        self.tool_infos.insert(name.clone(), info);
        self.tools.insert(name, Box::new(tool));
        self.capabilities.tools = Some(Default::default());
        self
    }

    /// Add a resource provider to the server.
    pub fn resource<R: WasmResource + 'static>(
        mut self,
        name: impl Into<String>,
        resource: R,
    ) -> Self {
        self.resources.insert(name.into(), Box::new(resource));
        self.capabilities.resources = Some(Default::default());
        self
    }

    /// Add a prompt to the server.
    pub fn prompt<P: WasmPrompt + 'static>(mut self, name: impl Into<String>, prompt: P) -> Self {
        let name = name.into();
        // Cache metadata at registration time before moving ownership
        let info = prompt.info();
        self.prompt_infos.insert(name.clone(), info);
        self.prompts.insert(name, Box::new(prompt));
        self.capabilities.prompts = Some(Default::default());
        self
    }

    /// Build the server.
    pub fn build(self) -> WasmMcpServer {
        WasmMcpServer {
            info: Implementation::new(self.name, self.version),
            capabilities: self.capabilities,
            tools: self.tools,
            resources: self.resources,
            prompts: self.prompts,
            tool_infos: self.tool_infos,
            prompt_infos: self.prompt_infos,
        }
    }
}

// Example implementations for common patterns

/// Simple function-based tool implementation.
pub struct SimpleTool<F> {
    name: String,
    description: String,
    input_schema: Value,
    handler: F,
}

impl<F> SimpleTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    /// Build a tool from a name, a description and a handler closure.
    ///
    /// The input schema defaults to a permissive open object; use
    /// [`Self::with_schema`] to constrain it.
    pub fn new(name: impl Into<String>, description: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            }),
            handler,
        }
    }

    /// Replace the default permissive input schema.
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }
}

impl<F> WasmTool for SimpleTool<F>
where
    F: Fn(Value) -> Result<Value> + Send + Sync,
{
    fn execute(&self, args: Value) -> Result<Value> {
        (self.handler)(args)
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.name.clone(),
            title: None,
            description: Some(self.description.clone()),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: None,
            icons: None,
            _meta: None,
            execution: None,
        }
    }
}

impl std::fmt::Debug for WasmMcpServer {
    /// Hand-written because the registries hold `dyn WasmTool`/`WasmResource`/
    /// `WasmPrompt`, which are not `Debug` and must not be forced to be.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmMcpServer")
            .field("info", &self.info)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for WasmMcpServerBuilder {
    /// Hand-written for the same reason as [`WasmMcpServer`]: the pending
    /// registries hold non-`Debug` trait objects.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmMcpServerBuilder")
            .field("name", &self.name)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl<F> std::fmt::Debug for SimpleTool<F> {
    /// Hand-written: `F` is a handler closure and is deliberately not bounded by
    /// `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}
