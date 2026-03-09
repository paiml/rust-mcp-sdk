//! MCP server implementation.

#[cfg(not(target_arch = "wasm32"))]
use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::shared::{Protocol, ProtocolOptions, TransportMessage};
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
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use crate::runtime::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;

// Core modules (currently native-only due to dependencies)
#[cfg(not(target_arch = "wasm32"))]
pub mod adapters;
#[cfg(not(target_arch = "wasm32"))]
pub mod builder;
#[cfg(not(target_arch = "wasm32"))]
pub mod core;

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
#[cfg(not(target_arch = "wasm32"))]
pub mod preset;
/// Progress reporting support for long-running operations.
#[cfg(not(target_arch = "wasm32"))]
pub mod progress;
/// Simple prompt implementations with metadata support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_prompt;
/// Simple resource implementations with builder pattern support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_resources;
/// Simple tool implementations with schema support.
#[cfg(not(target_arch = "wasm32"))]
pub mod simple_tool;
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

/// Typed tool implementations with automatic schema generation.
#[cfg(not(target_arch = "wasm32"))]
pub mod typed_tool;

/// UI resource implementations for MCP Apps Extension (SEP-1865).
#[cfg(not(target_arch = "wasm32"))]
pub mod ui;

/// MCP Apps Extension - Interactive UI support for multiple MCP hosts.
///
/// Provides adapters for `ChatGPT` Apps, MCP Apps (SEP-1865), and MCP-UI.
#[cfg(all(not(target_arch = "wasm32"), feature = "mcp-apps"))]
#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]
pub mod mcp_apps;

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

// For WASM, provide a simple stub for RequestHandlerExtra
#[cfg(target_arch = "wasm32")]
pub mod cancellation {
    /// Stub for WASM - no cancellation support
    #[derive(Debug, Clone, Default)]
    pub struct RequestHandlerExtra;
}
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
pub mod subscriptions;
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
    sampling: Option<Arc<dyn SamplingHandler>>,
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,
    initialized: Arc<RwLock<bool>>,
    /// Channel for sending notifications
    notification_tx: Option<mpsc::Sender<Notification>>,
    /// Cancellation manager for request cancellation
    cancellation_manager: cancellation::CancellationManager,
    /// Roots manager for directory/URI registration
    roots_manager: Arc<RwLock<roots::RootsManager>>,
    /// Subscription manager for resource subscriptions
    subscription_manager: Arc<RwLock<subscriptions::SubscriptionManager>>,
    /// Elicitation manager for user input requests
    elicitation_manager: Option<Arc<elicitation::ElicitationManager>>,
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

#[cfg(not(target_arch = "wasm32"))]
impl Server {
    /// Check if a tool exists
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Check if a prompt exists
    pub fn has_prompt(&self, name: &str) -> bool {
        self.prompts.contains_key(name)
    }

    /// Get a prompt handler by name
    pub fn get_prompt(&self, name: &str) -> Option<&Arc<dyn PromptHandler>> {
        self.prompts.get(name)
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
    /// let progress = ProgressNotification {
    ///     progress_token: ProgressToken::String("task-123".to_string()),
    ///     progress: 50.0,
    ///     total: None,
    ///     message: Some("Processing...".to_string()),
    /// };
    ///
    /// server.send_notification(ServerNotification::Progress(progress)).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_notification(&self, notification: ServerNotification) {
        if let Some(tx) = &self.notification_tx {
            let _ = tx.send(Notification::Server(notification)).await;
        }
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
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transport fails to initialize or operate
    /// - Communication with the client fails
    /// - The server encounters an unrecoverable error
    pub async fn run<T: crate::shared::Transport + 'static>(mut self, transport: T) -> Result<()> {
        let (notification_tx, notification_rx) = mpsc::channel(100);
        self.notification_tx = Some(notification_tx);

        // Hook cancellation manager to send notifications via the same channel
        if let Some(tx) = &self.notification_tx {
            let tx = tx.clone();
            self.cancellation_manager
                .set_notification_sender(Arc::new(move |notification| {
                    let _ = tx.try_send(notification);
                }));
        }

        let server = Arc::new(self);
        let transport = Arc::new(RwLock::new(transport));
        let protocol = Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default())));

        Self::spawn_notification_handler(transport.clone(), notification_rx);
        Self::spawn_message_handler(server.clone(), transport.clone(), protocol);

        // Keep the main task alive
        Self::run_main_loop().await
    }

    /// Spawn task to handle outgoing notifications.
    fn spawn_notification_handler(
        transport: Arc<RwLock<impl crate::shared::Transport + 'static>>,
        mut notification_rx: mpsc::Receiver<Notification>,
    ) {
        tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                if let Err(e) =
                    Self::send_notification_through_transport(&transport, notification).await
                {
                    Self::log_error(&format!("Failed to send notification: {}", e)).await;
                }
            }
        });
    }

    /// Spawn task to handle incoming messages.
    fn spawn_message_handler(
        server: Arc<Self>,
        transport: Arc<RwLock<impl crate::shared::Transport + 'static>>,
        _protocol: Arc<RwLock<Protocol>>,
    ) {
        tokio::spawn(async move {
            loop {
                let message = match Self::receive_message_from_transport(&transport).await {
                    Ok(msg) => msg,
                    Err(e) => {
                        Self::log_error(&format!("Transport receive error: {}", e)).await;
                        break;
                    },
                };

                if let Err(e) = Self::handle_transport_message(&server, &transport, message).await {
                    Self::log_error(&format!("Message handling error: {}", e)).await;
                    break;
                }
            }
        });
    }

    /// Send a notification through the transport.
    async fn send_notification_through_transport(
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        notification: Notification,
    ) -> Result<()> {
        let mut t = transport.write().await;
        t.send(TransportMessage::Notification(notification)).await
    }

    /// Receive a message from the transport.
    async fn receive_message_from_transport(
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
    ) -> Result<TransportMessage> {
        let mut t = transport.write().await;
        t.receive().await
    }

    /// Handle a transport message.
    async fn handle_transport_message(
        server: &Arc<Self>,
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        message: TransportMessage,
    ) -> Result<()> {
        match message {
            TransportMessage::Request { id, request } => {
                Self::handle_request_message(server, transport, id, request).await
            },
            TransportMessage::Response(_) => {
                Self::log_warning("Server received unexpected response message").await;
                Ok(())
            },
            TransportMessage::Notification(notification) => {
                // Handle client cancellation notifications
                if let Notification::Client(crate::types::ClientNotification::Cancelled(params)) =
                    &notification
                {
                    let request_id = params.request_id.to_string();
                    server
                        .cancellation_manager
                        .cancel_request_silent(request_id)
                        .await?;
                }

                Self::log_debug("Server received notification").await;
                Ok(())
            },
        }
    }

    /// Handle a request message.
    async fn handle_request_message(
        server: &Arc<Self>,
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        id: RequestId,
        request: Request,
    ) -> Result<()> {
        let response = server.handle_request(id, request, None).await;
        let mut t = transport.write().await;
        t.send(TransportMessage::Response(response)).await
    }

    /// Log an error message.
    async fn log_error(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Error, message, None).await;
    }

    /// Log a warning message.
    async fn log_warning(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Warning, message, None).await;
    }

    /// Log a debug message.
    async fn log_debug(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Debug, message, None).await;
    }

    /// Run the main event loop.
    async fn run_main_loop() -> Result<()> {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<auth::AuthContext>,
    ) -> JSONRPCResponse {
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };
                // Store client capabilities
                *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
                *self.initialized.write().await = true;

                let result = InitializeResult {
                    protocol_version: ProtocolVersion("2024-11-05".to_string()),
                    capabilities: self.capabilities.clone(),
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
            Request::Client(boxed_req) => {
                self.handle_client_request(id, *boxed_req, auth_context)
                    .await
            },
            Request::Server(_) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::jsonrpc::JSONRPCError {
                        code: -32601,
                        message: "Server requests not supported by server".to_string(),
                        data: None,
                    },
                ),
            },
        }
    }

    async fn handle_client_request(
        &self,
        id: RequestId,
        request: ClientRequest,
        auth_context: Option<auth::AuthContext>,
    ) -> JSONRPCResponse {
        let result = self
            .process_client_request(id.clone(), request, auth_context)
            .await;
        Self::create_response(id, result)
    }

    /// Process a client request and return the result.
    async fn process_client_request(
        &self,
        request_id: RequestId,
        request: ClientRequest,
        auth_context: Option<auth::AuthContext>,
    ) -> Result<serde_json::Value> {
        match request {
            ClientRequest::Initialize(_) => {
                // Already handled above
                unreachable!("Initialize should be handled separately")
            },
            ClientRequest::ListTools(req) => self.handle_list_tools(req),
            ClientRequest::CallTool(req) => {
                self.handle_call_tool(request_id, req, auth_context).await
            },
            ClientRequest::ListPrompts(req) => self.handle_list_prompts(req),
            ClientRequest::GetPrompt(req) => {
                self.handle_get_prompt(request_id, req, auth_context).await
            },
            ClientRequest::ListResources(req) => {
                self.handle_list_resources(request_id, req, auth_context)
                    .await
            },
            ClientRequest::ReadResource(req) => {
                self.handle_read_resource(request_id, req, auth_context)
                    .await
            },
            ClientRequest::ListResourceTemplates(req) => {
                Self::handle_list_resource_templates(self, req)
            },
            ClientRequest::Subscribe(_)
            | ClientRequest::Unsubscribe(_)
            | ClientRequest::Complete(_)
            | ClientRequest::SetLoggingLevel { level: _ }
            | ClientRequest::Ping => Ok(serde_json::json!({})),
            ClientRequest::CreateMessage(req) => self.handle_create_message(request_id, req).await,
            ClientRequest::ElicitInputResponse(response) => {
                // Handle elicitation response if we have a manager
                if let Some(elicitation_manager) = &self.elicitation_manager {
                    elicitation_manager.handle_response(response).await?;
                }
                Ok(serde_json::json!({}))
            },
            // Task requests (experimental MCP Tasks) -- routing handled in Plan 02
            ClientRequest::TasksGet(_)
            | ClientRequest::TasksResult(_)
            | ClientRequest::TasksList(_)
            | ClientRequest::TasksCancel(_) => Err(crate::Error::protocol(
                crate::ErrorCode::METHOD_NOT_FOUND,
                "Tasks not supported: no task router configured",
            )),
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
                        code: -32603,
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
        })?)
    }

    #[allow(clippy::cognitive_complexity)]
    async fn handle_call_tool(
        &self,
        request_id: RequestId,
        req: CallToolRequest,
        auth_context: Option<auth::AuthContext>,
    ) -> Result<Value> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Tool '{}' not found", req.name)))?;

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

        // Create progress reporter if progress token is provided
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter = req
            ._meta
            .as_ref()
            .and_then(|meta| meta.progress_token.as_ref())
            .and_then(|token| {
                self.notification_tx.as_ref().map(|tx| {
                    let tx = tx.clone();
                    let reporter = crate::server::progress::ServerProgressReporter::new(
                        token.clone(),
                        Arc::new(move |notification| {
                            let _ = tx.try_send(notification);
                        }),
                    );
                    Arc::new(reporter) as Arc<dyn crate::server::progress::ProgressReporter>
                })
            });

        let mut extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id.to_string(),
            cancellation_token,
        )
        .with_auth_context(validated_auth_context)
        .with_progress_reporter(progress_reporter);

        // Execute tool with middleware (native-only)
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            // Create tool context for middleware
            let context = tool_middleware::ToolContext::new(&req.name, &request_id_str);

            // Clone arguments for middleware processing
            let mut args = req.arguments;

            // Process request through tool middleware chain
            // Middleware rejection short-circuits tool execution
            self.tool_middleware_chain
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Execute the tool with potentially modified args and extra
            let mut result = handler.handle(args, extra).await;

            // Process response through tool middleware chain
            if let Err(e) = self
                .tool_middleware_chain
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
                self.tool_middleware_chain
                    .read()
                    .await
                    .handle_tool_error(&req.name, e, &context)
                    .await;
            }

            result
        };

        // On WASM, execute tool directly without middleware
        #[cfg(target_arch = "wasm32")]
        let result = handler.handle(req.arguments, extra).await;

        let result = match result {
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
        // Build CallToolResult, adding structured_content for widget tools
        let text = result.to_string();
        let mut call_result = CallToolResult::new(vec![crate::types::Content::Text { text }]);

        if let Some(info) = self.tool_infos.get(&req.name) {
            call_result = call_result.with_widget_enrichment(info, result);
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
                    crate::types::PromptInfo {
                        name: name.clone(),
                        description: None,
                        arguments: None,
                    }
                }
            })
            .collect::<Vec<_>>();

        Ok(serde_json::to_value(ListPromptsResult {
            prompts,
            next_cursor: None,
        })?)
    }

    async fn handle_get_prompt(
        &self,
        request_id: RequestId,
        req: GetPromptRequest,
        auth_context: Option<auth::AuthContext>,
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

        // Create progress reporter if progress token is provided
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter = req
            ._meta
            .as_ref()
            .and_then(|meta| meta.progress_token.as_ref())
            .and_then(|token| {
                self.notification_tx.as_ref().map(|tx| {
                    let tx = tx.clone();
                    let reporter = crate::server::progress::ServerProgressReporter::new(
                        token.clone(),
                        Arc::new(move |notification| {
                            let _ = tx.try_send(notification);
                        }),
                    );
                    Arc::new(reporter) as Arc<dyn crate::server::progress::ProgressReporter>
                })
            });

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id_str.clone(),
            cancellation_token,
        )
        .with_auth_context(auth_context)
        .with_progress_reporter(progress_reporter);
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
    ) -> Result<Value> {
        if let Some(handler) = &self.resources {
            let request_id_str = request_id.to_string();
            let cancellation_token = self
                .cancellation_manager
                .create_token(request_id_str.clone())
                .await;
            let extra = crate::server::cancellation::RequestHandlerExtra::new(
                request_id_str.clone(),
                cancellation_token,
            )
            .with_auth_context(auth_context);
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
            })?)
        }
    }

    async fn handle_read_resource(
        &self,
        request_id: RequestId,
        req: ReadResourceRequest,
        auth_context: Option<auth::AuthContext>,
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

        // Create progress reporter if progress token is provided
        #[allow(clippy::used_underscore_binding)] // _meta is part of MCP protocol spec
        let progress_reporter = req
            ._meta
            .as_ref()
            .and_then(|meta| meta.progress_token.as_ref())
            .and_then(|token| {
                self.notification_tx.as_ref().map(|tx| {
                    let tx = tx.clone();
                    let reporter = crate::server::progress::ServerProgressReporter::new(
                        token.clone(),
                        Arc::new(move |notification| {
                            let _ = tx.try_send(notification);
                        }),
                    );
                    Arc::new(reporter) as Arc<dyn crate::server::progress::ProgressReporter>
                })
            });

        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id_str.clone(),
            cancellation_token,
        )
        .with_auth_context(auth_context)
        .with_progress_reporter(progress_reporter);
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
                if let crate::types::protocol::Content::Resource { uri, meta, .. } = content {
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
        })?)
    }

    async fn handle_create_message(
        &self,
        request_id: RequestId,
        req: crate::types::CreateMessageRequest,
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
        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id_str.clone(),
            cancellation_token,
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

/// Builder for creating servers.
#[cfg(not(target_arch = "wasm32"))]
pub struct ServerBuilder {
    name: Option<String>,
    version: Option<String>,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    resources: Option<Arc<dyn ResourceHandler>>,
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
        }
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
    /// let capabilities = ServerCapabilities {
    ///     tools: Some(ToolCapabilities {
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
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
    /// use pmcp::{Server, PromptHandler, GetPromptResult, PromptMessage, MessageContent};
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
    ///             vec![PromptMessage {
    ///                 role: pmcp::Role::User,
    ///                 content: pmcp::Content::Text {
    ///                     text: format!("Please review this {} code:", language),
    ///                 },
    ///             }],
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
    ///         Ok(ReadResourceResult::new(vec![pmcp::Content::Text {
    ///             text: "File content here".to_string(),
    ///         }]))
    ///     }
    ///
    ///     async fn list(&self, _cursor: Option<String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<ListResourcesResult> {
    ///         Ok(ListResourcesResult::new(vec![pmcp::ResourceInfo {
    ///             uri: "file://example.txt".to_string(),
    ///             name: "example.txt".to_string(),
    ///             description: Some("Example file".to_string()),
    ///             mime_type: Some("text/plain".to_string()),
    ///             meta: None,
    ///         }]))
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
    ///         Ok(CreateMessageResult {
    ///             content: pmcp::MessageContent::Text {
    ///                 text: "Generated response".to_string(),
    ///             },
    ///             model: "mock-llm-v1".to_string(),
    ///             usage: Some(pmcp::TokenUsage {
    ///                 input_tokens: 10,
    ///                 output_tokens: 5,
    ///                 total_tokens: 15,
    ///             }),
    ///             stop_reason: Some("end_of_text".to_string()),
    ///         })
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

    /// Build the server.
    ///
    /// Constructs the final Server instance from the configured builder.
    /// This validates that required fields (name and version) are set.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The server name is not set
    /// - The server version is not set
    pub fn build(self) -> Result<Server> {
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

        // Build URI-to-tool-meta index for widget resource _meta propagation
        let uri_to_tool_meta = core::build_uri_to_tool_meta(&tool_infos);

        Ok(Server {
            info: Implementation { name, version },
            capabilities: self.capabilities,
            tools: self.tools,
            tool_infos,
            uri_to_tool_meta,
            prompts: self.prompts,
            resources: self.resources,
            sampling: self.sampling,
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            notification_tx: None,
            cancellation_manager: self.cancellation_manager,
            roots_manager: Arc::new(RwLock::new(self.roots_manager)),
            subscription_manager: Arc::new(RwLock::new(subscriptions::SubscriptionManager::new())),
            elicitation_manager: None,
            auth_provider: self.auth_provider,
            tool_authorizer,
            #[cfg(not(target_arch = "wasm32"))]
            tool_middleware_chain,
            #[cfg(feature = "streamable-http")]
            http_middleware: self.http_middleware,
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
                client_info: Implementation {
                    name: "test-client".to_string(),
                    version: "1.0.0".to_string(),
                },
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

        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

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
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
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

    #[tokio::test]
    async fn test_handle_list_resources() {
        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

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
        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

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
            Box::new(crate::types::protocol::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::protocol::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
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
}
