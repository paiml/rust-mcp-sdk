//! MCP client implementation.

use crate::error::{Error, Result};
use crate::shared::{
    EnhancedMiddlewareChain, MiddlewareContext, Protocol, ProtocolOptions, Transport,
};
use crate::types::{
    CallToolRequest, CallToolResult, CancelledNotification, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, CreateMessageRequest, CreateMessageResult,
    GetPromptRequest, GetPromptResult, Implementation, InitializeRequest, InitializeResult,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest,
    ListToolsResult, LoggingLevel, Notification, ProgressNotification, ReadResourceRequest,
    ReadResourceResult, Request, RequestId, ServerCapabilities, SubscribeRequest,
    UnsubscribeRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, oneshot, RwLock};

#[cfg(target_arch = "wasm32")]
use futures::SinkExt;
#[cfg(target_arch = "wasm32")]
use futures_channel::{mpsc, oneshot};
#[cfg(target_arch = "wasm32")]
use futures_locks::RwLock;

#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
pub mod http_logging_middleware;
pub mod http_middleware;
#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
pub mod oauth;
pub mod oauth_middleware;
pub mod transport;

/// MCP client for connecting to servers.
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Arc<RwLock<Protocol>>,
    middleware_chain: Arc<RwLock<EnhancedMiddlewareChain>>,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    server_version: Option<Implementation>,
    instructions: Option<String>,
    initialized: bool,
    info: Implementation,
    notification_tx: Option<mpsc::Sender<Notification>>,
    active_requests: Arc<RwLock<HashMap<RequestId, oneshot::Sender<()>>>>,
}

impl<T: Transport> std::fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("transport", &"<Arc<RwLock<Transport>>>")
            .field("protocol", &"<Arc<RwLock<Protocol>>>")
            .field("capabilities", &self.capabilities)
            .field("server_capabilities", &self.server_capabilities)
            .field("initialized", &self.initialized)
            .field("info", &self.info)
            .finish()
    }
}

impl<T: Transport> Client<T> {
    /// Create a new client with the given transport.
    ///
    /// Uses default client information with the name "pmcp-client" and the
    /// current crate version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport};
    ///
    /// let transport = StdioTransport::new();
    /// let client = Client::new(transport);
    /// ```
    pub fn new(transport: T) -> Self {
        Self::with_info(
            transport,
            Implementation {
                name: "pmcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        )
    }

    /// Create a new client with custom info.
    ///
    /// Allows specifying custom client name and version information that will
    /// be sent to the server during initialization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation {
    ///     name: "my-custom-client".to_string(),
    ///     version: "2.1.0".to_string(),
    /// };
    /// let client = Client::with_info(transport, client_info);
    /// ```
    pub fn with_info(transport: T, client_info: Implementation) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new client with custom protocol options.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    /// use pmcp::shared::ProtocolOptions;
    ///
    /// // Custom options for high-throughput scenarios
    /// let options = ProtocolOptions {
    ///     enforce_strict_capabilities: false,
    ///     debounced_notification_methods: vec![
    ///         "notifications/progress".to_string(),
    ///         "notifications/message".to_string(),
    ///     ],
    /// };
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation {
    ///     name: "high-throughput-client".to_string(),
    ///     version: "1.0.0".to_string(),
    /// };
    ///
    /// let client = Client::with_options(transport, client_info, options);
    /// ```
    pub fn with_options(
        transport: T,
        client_info: Implementation,
        options: ProtocolOptions,
    ) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(options))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize the connection with the server.
    ///
    /// Performs the MCP initialization handshake, negotiating capabilities and
    /// receiving server information. This must be called before using other
    /// client methods.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// let capabilities = ClientCapabilities::default();
    /// let server_info = client.initialize(capabilities).await?;
    ///
    /// println!("Server: {} v{}",
    ///          server_info.server_info.name,
    ///          server_info.server_info.version);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is already initialized
    /// - The server rejects the initialization
    /// - Communication with the server fails
    pub async fn initialize(
        &mut self,
        capabilities: ClientCapabilities,
    ) -> Result<InitializeResult> {
        if self.initialized {
            return Err(Error::InvalidState("Client already initialized".into()));
        }

        self.capabilities = Some(capabilities.clone());

        // Send initialize request
        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::types::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info: self.info.clone(),
        })));

        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        // Parse initialize result
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                if let Ok(init_result) = serde_json::from_value::<InitializeResult>(result) {
                    // Validate protocol version
                    if !crate::types::SUPPORTED_PROTOCOL_VERSIONS
                        .contains(&init_result.protocol_version.as_str())
                    {
                        return Err(Error::protocol_msg(format!(
                            "Server protocol version {} not supported",
                            init_result.protocol_version
                        )));
                    }

                    self.server_capabilities = Some(init_result.capabilities.clone());
                    self.server_version = Some(init_result.server_info.clone());
                    self.instructions.clone_from(&init_result.instructions);
                    self.initialized = true;

                    // Send initialized notification
                    self.send_notification(Notification::Client(ClientNotification::Initialized))
                        .await?;

                    Ok(init_result)
                } else {
                    Err(Error::parse("Invalid initialize result format"))
                }
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get server capabilities after initialization.
    pub fn get_server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Get server version information after initialization.
    pub fn get_server_version(&self) -> Option<&Implementation> {
        self.server_version.as_ref()
    }

    /// Get server instructions after initialization.
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Send a ping to the server.
    pub async fn ping(&self) -> Result<()> {
        self.ensure_initialized()?;
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Set the logging level on the server.
    pub async fn set_logging_level(&self, level: LoggingLevel) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("logging", "logging/setLevel")?;

        let request = Request::Client(Box::new(ClientRequest::SetLoggingLevel { level }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available tools.
    ///
    /// Retrieves information about all tools available on the server, including
    /// their names, descriptions, and input schemas.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all tools
    /// let tools = client.list_tools(None).await?;
    /// for tool in tools.tools {
    ///     println!("Tool: {} - {}",
    ///              tool.name,
    ///              tool.description.unwrap_or_else(|| "No description".to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional pagination cursor for retrieving additional results
    pub async fn list_tools(&self, cursor: Option<String>) -> Result<ListToolsResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool.
    ///
    /// Invokes a server-provided tool with the specified name and arguments.
    /// The server must have declared the tool via the tools capability during initialization.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `arguments` - JSON value containing the tool's arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Call a simple tool with no arguments
    /// let result = client.call_tool(
    ///     "list_files".to_string(),
    ///     json!({})
    /// ).await?;
    ///
    /// // Call a tool with specific arguments
    /// let search_result = client.call_tool(
    ///     "search".to_string(),
    ///     json!({
    ///         "query": "rust programming",
    ///         "limit": 10
    ///     })
    /// ).await?;
    ///
    /// // Tools can return structured data
    /// if let Some(content) = result.content.first() {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Tool result: {}", text);
    ///         }
    ///         _ => println!("Non-text tool result"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support tools
    /// - The tool name doesn't exist
    /// - The arguments are invalid for the tool
    /// - Network or protocol errors occur
    pub async fn call_tool(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available prompts.
    ///
    /// Retrieves information about all prompts available on the server, including
    /// their names, descriptions, and required arguments.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large prompt lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all prompts
    /// let prompts = client.list_prompts(None).await?;
    /// for prompt in prompts.prompts {
    ///     println!("Prompt: {} - {}",
    ///              prompt.name,
    ///              prompt.description.unwrap_or_else(|| "No description".to_string()));
    ///     
    ///     // Show required arguments
    ///     if let Some(args) = prompt.arguments {
    ///         for arg in args {
    ///             println!("  - {}: {} (required: {})",
    ///                      arg.name,
    ///                      arg.description.unwrap_or_else(|| "No description".to_string()),
    ///                      arg.required);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - Network or protocol errors occur
    pub async fn list_prompts(&self, cursor: Option<String>) -> Result<ListPromptsResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get a prompt.
    ///
    /// Retrieves a specific prompt from the server with the provided arguments.
    /// The prompt is processed by the server and returned with filled-in content.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to retrieve
    /// * `arguments` - Key-value pairs for prompt arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use std::collections::HashMap;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Get a prompt with arguments
    /// let mut args = HashMap::new();
    /// args.insert("language".to_string(), "Rust".to_string());
    /// args.insert("topic".to_string(), "async programming".to_string());
    ///
    /// let prompt_result = client.get_prompt(
    ///     "code_review".to_string(),
    ///     args
    /// ).await?;
    ///
    /// println!("Prompt description: {}",
    ///          prompt_result.description.unwrap_or_else(|| "No description".to_string()));
    ///
    /// // Process the prompt messages
    /// for message in prompt_result.messages {
    ///     println!("Role: {}", message.role);
    ///     match &message.content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Content: {}", text);
    ///         }
    ///         _ => println!("Non-text content"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - The prompt name doesn't exist
    /// - Required arguments are missing
    /// - Network or protocol errors occur
    pub async fn get_prompt(
        &self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<GetPromptResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/get")?;

        let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name,
            arguments,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available resources.
    ///
    /// Retrieves information about all resources available on the server, including
    /// their names, descriptions, URIs, and MIME types.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large resource lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resources
    /// let resources = client.list_resources(None).await?;
    /// for resource in resources.resources {
    ///     println!("Resource: {} ({})", resource.name, resource.uri);
    ///     if let Some(description) = resource.description {
    ///         println!("  Description: {}", description);
    ///     }
    ///     if let Some(mime_type) = resource.mime_type {
    ///         println!("  MIME Type: {}", mime_type);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - Network or protocol errors occur
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ListResourcesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List resource templates.
    ///
    /// Retrieves information about all resource templates available on the server.
    /// Resource templates define patterns for dynamically generated resources.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large template lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resource templates
    /// let templates = client.list_resource_templates(None).await?;
    /// for template in templates.resource_templates {
    ///     println!("Template: {} ({})", template.name, template.uri_template);
    ///     if let Some(description) = template.description {
    ///         println!("  Description: {}", description);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource templates
    /// - Network or protocol errors occur
    pub async fn list_resource_templates(
        &self,
        cursor: Option<String>,
    ) -> Result<ListResourceTemplatesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/templates/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResourceTemplates(
            ListResourceTemplatesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Read a resource.
    ///
    /// Retrieves the content of a specific resource from the server by its URI.
    /// Resources can contain text, binary data, or structured content.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to read
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Read a text resource
    /// let resource = client.read_resource("file://readme.txt".to_string()).await?;
    /// for content in resource.contents {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Text content: {}", text);
    ///         }
    ///         pmcp::Content::Resource { uri, .. } => {
    ///             println!("Resource reference: {}", uri);
    ///         }
    ///         _ => println!("Other content type"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - The resource URI doesn't exist
    /// - Access to the resource is denied
    /// - Network or protocol errors occur
    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/read")?;

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Subscribe to resource updates.
    ///
    /// Subscribes to receive notifications when a resource changes.
    /// The server will send notifications when the subscribed resource is modified.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to subscribe to
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a configuration file
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Now the client will receive notifications when settings.json changes
    /// // Handle notifications in your event loop
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI doesn't exist
    /// - Network or protocol errors occur
    pub async fn subscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/subscribe")?;

        // Check if server supports subscriptions
        if let Some(resources) = &self
            .server_capabilities
            .as_ref()
            .and_then(|c| c.resources.as_ref())
        {
            if !resources.subscribe.unwrap_or(false) {
                return Err(Error::capability(
                    "Server does not support resource subscriptions",
                ));
            }
        }

        let request = Request::Client(Box::new(ClientRequest::Subscribe(SubscribeRequest { uri })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Unsubscribe from resource updates.
    ///
    /// Unsubscribes from notifications for a previously subscribed resource.
    /// After unsubscribing, the client will no longer receive change notifications.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to unsubscribe from
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a resource
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Later, unsubscribe when no longer needed
    /// client.unsubscribe_resource("file://config/settings.json".to_string()).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI was not previously subscribed to
    /// - Network or protocol errors occur
    pub async fn unsubscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/unsubscribe")?;

        let request = Request::Client(Box::new(ClientRequest::Unsubscribe(UnsubscribeRequest {
            uri,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Request completion from the server.
    ///
    /// Requests auto-completion suggestions from the server for a given context.
    /// This is useful for implementing IDE-like features with contextual suggestions.
    ///
    /// # Arguments
    ///
    /// * `params` - The completion request parameters
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CompleteRequest};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Request completion for partial text
    /// let completion_request = CompleteRequest {
    ///     r#ref: pmcp::CompletionReference::Resource {
    ///         uri: "file://code.rs".to_string(),
    ///     },
    ///     argument: pmcp::CompletionArgument {
    ///         name: "function_name".to_string(),
    ///         value: "calc_".to_string(),
    ///     },
    /// };
    ///
    /// let completions = client.complete(completion_request).await?;
    /// for completion in completions.completion.values {
    ///     println!("Suggestion: {}", completion);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support completions
    /// - The completion context is invalid
    /// - Network or protocol errors occur
    pub async fn complete(&self, params: CompleteRequest) -> Result<CompleteResult> {
        self.ensure_initialized()?;
        self.assert_capability("completions", "completion/complete")?;

        let request = Request::Client(Box::new(ClientRequest::Complete(params)));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Create a message using sampling (for LLM providers).
    ///
    /// Requests the server to generate a message using its language model capabilities.
    /// This is typically used by servers that provide LLM functionality.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CreateMessageRequest, SamplingMessage};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let mut capabilities = ClientCapabilities::default();
    /// capabilities.sampling = Some(Default::default());
    ///
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(capabilities).await?;
    ///
    /// // Create a message with the LLM
    /// let request = CreateMessageRequest {
    ///     messages: vec![
    ///         SamplingMessage {
    ///             role: pmcp::types::Role::User,
    ///             content: pmcp::types::Content::Text {
    ///                 text: "Explain how to implement a binary search tree".to_string(),
    ///             },
    ///         },
    ///     ],
    ///     model_preferences: Some(pmcp::types::ModelPreferences {
    ///         hints: Some(vec![
    ///             pmcp::types::ModelHint {
    ///                 name: Some("gpt-4".to_string()),
    ///             },
    ///         ]),
    ///         cost_priority: Some(0.5),
    ///         speed_priority: Some(0.3),
    ///         intelligence_priority: Some(0.2),
    ///     }),
    ///     system_prompt: Some("You are a helpful programming assistant".to_string()),
    ///     include_context: pmcp::types::IncludeContext::ThisServerOnly,
    ///     temperature: Some(0.7),
    ///     max_tokens: Some(1000),
    ///     stop_sequences: None,
    ///     metadata: Default::default(),
    /// };
    ///
    /// let result = client.create_message(request).await?;
    /// println!("Model: {}", result.model);
    /// println!("Response: {:?}", result.content);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support sampling
    /// - The request parameters are invalid
    /// - Network or protocol errors occur
    pub async fn create_message(
        &self,
        params: CreateMessageRequest,
    ) -> Result<CreateMessageResult> {
        self.ensure_initialized()?;
        self.assert_capability("sampling", "sampling/createMessage")?;

        let request = Request::Client(Box::new(ClientRequest::CreateMessage(params)));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Send roots list changed notification.
    ///
    /// Notifies the server that the client's root list has changed.
    /// This is typically sent when the workspace or project roots are modified.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let mut capabilities = ClientCapabilities::default();
    /// // Enable roots list changed capability
    /// capabilities.roots = Some(pmcp::RootsCapabilities {
    ///     list_changed: true,
    /// });
    ///
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(capabilities).await?;
    ///
    /// // Notify server when project roots change
    /// client.send_roots_list_changed().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The client doesn't support roots list changed notifications
    /// - Network or protocol errors occur
    pub async fn send_roots_list_changed(&self) -> Result<()> {
        self.ensure_initialized()?;
        if let Some(roots) = &self.capabilities.as_ref().and_then(|c| c.roots.as_ref()) {
            if roots.list_changed {
                // OK, we support it
            } else {
                return Err(Error::capability(
                    "Client does not support roots list changed notifications",
                ));
            }
        }

        self.send_notification(Notification::Client(ClientNotification::RootsListChanged))
            .await
    }

    /// Authenticate with the server.
    ///
    /// Performs authentication using the provided authentication information.
    /// This should be called after initialization if the server requires authentication.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, AuthInfo, AuthScheme};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// // Initialize first
    /// client.initialize(pmcp::ClientCapabilities::default()).await?;
    ///
    /// // Authenticate with bearer token
    /// let auth = AuthInfo {
    ///     scheme: AuthScheme::Bearer,
    ///     token: Some("your-api-token".to_string()),
    ///     oauth: None,
    ///     params: Default::default(),
    /// };
    ///
    /// client.authenticate(&auth)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - Authentication fails
    /// - The server doesn't support authentication
    pub fn authenticate(&self, auth_info: &crate::types::AuthInfo) -> Result<()> {
        self.ensure_initialized()?;

        // In a real implementation, this would send an authentication request
        // For now, we'll just validate that we can authenticate
        match auth_info.scheme {
            crate::types::AuthScheme::None => Ok(()),
            crate::types::AuthScheme::Bearer => {
                if auth_info.token.is_none() {
                    return Err(Error::validation("Bearer token required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::OAuth2 => {
                if auth_info.oauth.is_none() {
                    return Err(Error::validation("OAuth information required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::Custom(_) => {
                // Custom auth schemes would be handled here
                Ok(())
            },
        }
    }

    /// Cancel a request.
    ///
    /// Sends a cancellation notification for an active request.
    /// This allows graceful termination of long-running operations.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to cancel
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, RequestId};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Start a long-running operation
    /// let request_id = RequestId::String("long-operation-123".to_string());
    ///
    /// // Later, cancel the request if needed
    /// client.cancel_request(&request_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the cancellation
    pub async fn cancel_request(&self, request_id: &RequestId) -> Result<()> {
        // Send cancellation notification
        self.send_notification(Notification::Cancelled(CancelledNotification {
            request_id: request_id.clone(),
            reason: Some("User requested cancellation".to_string()),
        }))
        .await?;

        // Cancel any local tracking
        let sender = self.active_requests.write().await.remove(request_id);
        if let Some(sender) = sender {
            let _ = sender.send(());
        }

        Ok(())
    }

    /// Send a progress notification.
    ///
    /// Sends a progress update for a long-running operation.
    /// This allows the server or client to track operation progress.
    ///
    /// # Arguments
    ///
    /// * `progress` - The progress notification to send
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, ProgressNotification, RequestId};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Send progress update for a file processing operation
    /// let progress = ProgressNotification {
    ///     progress_token: pmcp::ProgressToken::String("file-processing".to_string()),
    ///     progress: 75.0,
    ///     total: None,
    ///     message: Some("Processing files...".to_string()),
    /// };
    ///
    /// client.send_progress(progress).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the notification
    pub async fn send_progress(&self, progress: ProgressNotification) -> Result<()> {
        self.send_notification(Notification::Progress(progress))
            .await
    }

    /// Check if client is initialized.
    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Error::InvalidState("Client not initialized".into()))
        }
    }

    /// Assert that the server has a specific capability.
    fn assert_capability(&self, capability: &str, method: &str) -> Result<()> {
        let has_capability = match capability {
            "tools" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.tools.is_some()),
            "prompts" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.prompts.is_some()),
            "resources" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.resources.is_some()),
            "logging" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.logging.is_some()),
            "completions" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.completions.is_some()),
            _ => false,
        };

        if has_capability {
            Ok(())
        } else {
            Err(Error::capability(format!(
                "Server does not support {} (required for {})",
                capability, method
            )))
        }
    }

    /// Send a request and wait for response.
    #[allow(clippy::cognitive_complexity)]
    async fn send_request(
        &self,
        request_id: RequestId,
        request: Request,
    ) -> Result<crate::types::JSONRPCResponse> {
        use crate::shared::protocol_helpers::create_request;

        // Track request for cancellation
        let (cancel_tx, _cancel_rx) = oneshot::channel();
        self.active_requests
            .write()
            .await
            .insert(request_id.clone(), cancel_tx);

        // Create middleware context
        let context = MiddlewareContext::with_request_id(request_id.to_string());

        // Convert to JSONRPC request
        let mut jsonrpc_request = create_request(request_id.clone(), request.clone());

        // Process request through middleware chain (read-only access)
        self.middleware_chain
            .read()
            .await
            .process_request_with_context(&mut jsonrpc_request, &context)
            .await?;

        // Send request through transport
        let message = crate::types::TransportMessage::Request {
            id: request_id.clone(),
            request,
        };

        self.transport.write().await.send(message).await?;

        // Wait for response, dispatching any unsolicited notifications along the way
        loop {
            let response_message = self.transport.write().await.receive().await?;

            match response_message {
                crate::types::TransportMessage::Response(mut response) => {
                    // Remove from active requests
                    self.active_requests.write().await.remove(&request_id);

                    // Process response through middleware chain (read-only access)
                    self.middleware_chain
                        .read()
                        .await
                        .process_response_with_context(&mut response, &context)
                        .await?;
                    return Ok(response);
                },
                crate::types::TransportMessage::Notification(notification) => {
                    // Unsolicited notification (e.g., progress, resource changes, SSE events)
                    // Convert to JSONRPC notification for middleware processing
                    use crate::shared::protocol_helpers::create_notification;
                    let mut jsonrpc_notification = create_notification(notification.clone());

                    // Process through protocol middleware chain
                    let notif_context = MiddlewareContext::default();

                    if let Err(e) = self
                        .middleware_chain
                        .write()
                        .await
                        .process_notification_with_context(
                            &mut jsonrpc_notification,
                            &notif_context,
                        )
                        .await
                    {
                        // Log error but don't terminate dispatcher - continue processing
                        tracing::warn!(
                            "Notification middleware processing failed for {}: {}",
                            jsonrpc_notification.method,
                            e
                        );
                    }

                    // Forward to notification handler if registered
                    if let Some(tx) = &self.notification_tx {
                        // Clone the sender because send() requires &mut self
                        #[allow(unused_mut)]
                        let mut tx_clone = tx.clone();
                        if let Err(e) = tx_clone.send(notification).await {
                            tracing::debug!("Notification channel closed: {}", e);
                        }
                    }

                    // Continue loop to wait for the actual response
                },
                crate::types::TransportMessage::Request { .. } => {
                    // Unexpected message type
                    self.active_requests.write().await.remove(&request_id);
                    return Err(Error::protocol_msg(
                        "Unexpected message type while waiting for response",
                    ));
                },
            }
        }
    }

    /// Send a notification.
    async fn send_notification(&self, notification: Notification) -> Result<()> {
        let message = crate::types::TransportMessage::Notification(notification);
        self.transport.write().await.send(message).await
    }
}

/// Builder for creating clients with custom configuration.
///
/// # Examples
///
/// ```rust
/// use pmcp::{ClientBuilder, StdioTransport};
///
/// # async fn example() -> Result<(), pmcp::Error> {
/// // Basic client builder
/// let transport = StdioTransport::new();
/// let client = ClientBuilder::new(transport)
///     .enforce_strict_capabilities(true)
///     .build();
///
/// // Client with debounced notifications
/// let transport2 = StdioTransport::new();
/// let debounced_client = ClientBuilder::new(transport2)
///     .debounced_notifications(vec![
///         "notifications/progress".to_string(),
///         "notifications/log".to_string(),
///     ])
///     .enforce_strict_capabilities(false)
///     .build();
///
/// // Chain multiple configurations
/// let transport3 = StdioTransport::new();
/// let configured_client = ClientBuilder::new(transport3)
///     .enforce_strict_capabilities(true)
///     .debounced_notifications(vec!["notifications/resources/changed".to_string()])
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder<T: Transport> {
    transport: T,
    options: ProtocolOptions,
    middleware_chain: EnhancedMiddlewareChain,
}

impl<T: Transport> std::fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("transport", &"<Transport>")
            .field("options", &self.options)
            .finish()
    }
}

impl<T: Transport> ClientBuilder<T> {
    /// Create a new client builder.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            options: ProtocolOptions::default(),
            middleware_chain: EnhancedMiddlewareChain::new(),
        }
    }

    /// Set whether to enforce strict capabilities.
    pub fn enforce_strict_capabilities(mut self, enforce: bool) -> Self {
        self.options.enforce_strict_capabilities = enforce;
        self
    }

    /// Set debounced notification methods.
    pub fn debounced_notifications(mut self, methods: Vec<String>) -> Self {
        self.options.debounced_notification_methods = methods;
        self
    }

    /// Add middleware to the client.
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_middleware(
        mut self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.middleware_chain.add(middleware);
        self
    }

    /// Add protocol-level middleware to the client.
    ///
    /// This is an alias for `with_middleware()` that provides explicit naming to distinguish
    /// protocol middleware (operates on JSON-RPC messages) from HTTP middleware
    /// (operates on HTTP requests/responses via `StreamableHttpTransportConfigBuilder`).
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_protocol_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_protocol_middleware(
        self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.with_middleware(middleware)
    }

    /// Set the entire middleware chain.
    ///
    /// This replaces any previously configured middleware.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::EnhancedMiddlewareChain;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let mut chain = EnhancedMiddlewareChain::new();
    /// // Add middleware to chain...
    ///
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .middleware_chain(chain)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn middleware_chain(mut self, chain: EnhancedMiddlewareChain) -> Self {
        self.middleware_chain = chain;
        self
    }

    /// Build the client.
    pub fn build(self) -> Client<T> {
        let mut client = Client::with_options(
            self.transport,
            Implementation {
                name: "pmcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            self.options,
        );
        // Replace the default middleware chain with the configured one
        client.middleware_chain = Arc::new(RwLock::new(self.middleware_chain));
        client
    }
}

impl<T: Transport> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            protocol: self.protocol.clone(),
            middleware_chain: self.middleware_chain.clone(),
            capabilities: self.capabilities.clone(),
            server_capabilities: self.server_capabilities.clone(),
            server_version: self.server_version.clone(),
            instructions: self.instructions.clone(),
            initialized: self.initialized,
            info: self.info.clone(),
            notification_tx: self.notification_tx.clone(),
            active_requests: self.active_requests.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::{JSONRPCError, ResponsePayload},
        JSONRPCResponse, ProgressNotification, ProgressToken, TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_responses(responses: Vec<TransportMessage>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, response: TransportMessage) {
            self.responses.lock().unwrap().push(response);
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| Error::protocol_msg("No more responses"))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_client_creation() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert!(!client.initialized);
        assert_eq!(client.info.name, "pmcp-client");
    }

    #[test]
    fn test_client_with_info() {
        let transport = MockTransport::new();
        let info = Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client = Client::with_info(transport, info);
        assert_eq!(client.info.name, "test-client");
        assert_eq!(client.info.version, "1.0.0");
    }

    #[test]
    fn test_client_builder() {
        let transport = MockTransport::new();
        let client = ClientBuilder::new(transport)
            .enforce_strict_capabilities(true)
            .debounced_notifications(vec!["test".to_string()])
            .build();
        assert!(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(client.protocol.read())
                .options()
                .enforce_strict_capabilities
        );
    }

    #[tokio::test]
    async fn test_client_initialization() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        let caps = ClientCapabilities::minimal();

        let result = client.initialize(caps).await;
        assert!(result.is_ok());
        assert!(client.initialized);
        assert_eq!(client.server_version.as_ref().unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_ping() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let ping_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({})),
        });

        let transport = MockTransport::with_responses(vec![ping_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize first
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Ping
        let result = client.ping().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let tools_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "tools": [{
                    "name": "test-tool",
                    "description": "Test tool",
                    "inputSchema": {}
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![tools_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize with tools capability
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // List tools
        let result = client.list_tools(None).await;
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_error_response() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let error_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Error(JSONRPCError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        });

        let transport = MockTransport::with_responses(vec![error_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should get error
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[tokio::test]
    async fn test_uninitialized_error() {
        let transport = MockTransport::new();
        let client = Client::new(transport);

        // Try to call method without initialization
        let result = client.ping().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_capability_enforcement() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    // No tools capability
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize without tools capability
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should fail
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_send_progress() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Send progress
        let progress = ProgressNotification {
            progress_token: ProgressToken::String("test".to_string()),
            progress: 50.0,
            total: None,
            message: Some("Halfway done".to_string()),
        };

        let result = client.send_progress(progress).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "completions": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let complete_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "completion": {
                    "values": ["test1", "test2"]
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![complete_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Complete
        let result = client
            .complete(CompleteRequest {
                r#ref: crate::types::CompletionReference::Resource {
                    uri: "test://test".to_string(),
                },
                argument: crate::types::CompletionArgument {
                    name: "test".to_string(),
                    value: "t".to_string(),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_resource() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "resources": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let read_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "contents": [{
                    "type": "text",
                    "text": "Hello, world!"
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![read_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // Read resource
        let result = client.read_resource("test://test".to_string()).await;
        if let Err(e) = &result {
            tracing::error!("Read resource error: {:?}", e);
        }
        assert!(result.is_ok());
        let contents = result.unwrap();
        assert_eq!(contents.contents.len(), 1);
    }
}
