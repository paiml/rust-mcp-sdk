//! Environment-agnostic MCP server for WASM/WASI deployment.
//!
//! This module provides a properly typed MCP server that maintains type safety
//! while being deployable to any WASI environment.

use crate::error::{Error, Result};
use crate::types::{
    CallToolParams, CallToolResult, ClientRequest, Content, GetPromptParams, GetPromptResult,
    Implementation, InitializeParams, InitializeResult, JSONRPCError, JSONRPCResponse,
    ListPromptsParams, ListPromptsResult, ListResourcesParams, ListResourcesResult,
    ListToolsParams, ListToolsResult, PromptInfo, ReadResourceParams, ReadResourceResult, Request,
    RequestId, ResourceInfo, ServerCapabilities, ToolInfo,
};
use crate::{ErrorCode, SUPPORTED_PROTOCOL_VERSIONS};
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;

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

    fn handle_initialize(&self, params: InitializeParams) -> Result<Value> {
        // Negotiate protocol version
        let client_version = params.protocol_version.clone();
        let negotiated_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&client_version.as_str()) {
            client_version
        } else {
            // Fall back to the latest supported version
            SUPPORTED_PROTOCOL_VERSIONS[0].to_string()
        };

        let result = InitializeResult {
            protocol_version: crate::types::ProtocolVersion(negotiated_version),
            capabilities: self.capabilities.clone(),
            server_info: self.info.clone(),
            instructions: None,
        };
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }

    fn handle_list_tools(&self, _params: ListToolsParams) -> Result<Value> {
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        let result = ListToolsResult {
            tools,
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }

    fn handle_call_tool(&self, params: CallToolParams) -> Result<Value> {
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
                    vec![Content::Text {
                        text: text.to_string(),
                    }]
                } else if result_value.is_object() {
                    // For structured data, wrap in a content type that preserves structure
                    vec![Content::Text {
                        text: serde_json::to_string_pretty(&result_value)
                            .unwrap_or_else(|_| "{}".to_string()),
                    }]
                } else {
                    vec![Content::Text {
                        text: result_value.to_string(),
                    }]
                };

                let result = CallToolResult {
                    content,
                    is_error: false,
                    ..Default::default()
                };
                serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
            },
            Err(e) => {
                let result = CallToolResult {
                    content: vec![Content::Text {
                        text: format!("Error: {}", e),
                    }],
                    is_error: true,
                    ..Default::default()
                };
                serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
            },
        }
    }

    fn handle_list_resources(&self, params: ListResourcesParams) -> Result<Value> {
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

        let result = ListResourcesResult {
            resources: all_resources,
            next_cursor,
        };
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }

    fn handle_read_resource(&self, params: ReadResourceParams) -> Result<Value> {
        // Find the first resource that can handle this URI
        for resource in self.resources.values() {
            if let Ok(result) = resource.read(&params.uri) {
                return serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()));
            }
        }
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            &format!("No resource handler for URI: {}", params.uri),
        ))
    }

    fn handle_list_prompts(&self, _params: ListPromptsParams) -> Result<Value> {
        let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();

        let result = ListPromptsResult {
            prompts,
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
    }

    fn handle_get_prompt(&self, params: GetPromptParams) -> Result<Value> {
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
            info: Implementation {
                name: self.name,
                version: self.version,
            },
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
            description: Some(self.description.clone()),
            input_schema: self.input_schema.clone(),
            output_schema: None,
            annotations: None,
            _meta: None,
            execution: None,
        }
    }
}
