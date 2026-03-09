//! # MCP SDK for Rust
//!
//! A high-quality Rust implementation of the Model Context Protocol (MCP) SDK.
//!
//! This crate provides both client and server implementations of MCP with:
//! - Full protocol compatibility with the TypeScript SDK
//! - Zero-copy parsing where possible
//! - Comprehensive type safety
//! - Multiple transport options (stdio, HTTP/SSE, WebSocket)
//! - Built-in authentication support
//!
//! ## Quick Start
//!
//! ### Client Example
//!
//! ```rust
//! use pmcp::{Client, StdioTransport, ClientCapabilities};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client with stdio transport
//! let transport = StdioTransport::new();
//! let mut client = Client::new(transport);
//!
//! // Initialize the connection
//! let server_info = client.initialize(ClientCapabilities::default()).await?;
//!
//! // List available tools
//! let tools = client.list_tools(None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Server Example
//!
//! ```rust
//! use pmcp::{Server, ServerCapabilities, ToolHandler};
//! use async_trait::async_trait;
//! use serde_json::Value;
//!
//! struct MyTool;
//!
//! #[async_trait]
//! impl ToolHandler for MyTool {
//!     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
//!         Ok(serde_json::json!({"result": "success"}))
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Server::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .capabilities(ServerCapabilities::default())
//!     .tool("my-tool", MyTool)
//!     .build()?;
//!
//! // Run with stdio transport
//! server.run_stdio().await?;
//! # Ok(())
//! # }
//! ```

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Allow certain clippy lints that are too pedantic for this codebase
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::result_large_err)]

pub mod assets;
pub mod client;
#[cfg(feature = "composition")]
#[cfg_attr(docsrs, doc(cfg(feature = "composition")))]
pub mod composition;
pub mod error;
pub mod runtime;
pub mod server;
pub mod shared;
pub mod types;
pub mod utils;

#[cfg(feature = "simd")]
pub mod simd;

// Re-export commonly used types
pub use client::{Client, ClientBuilder};
pub use error::{Error, ErrorCode, Result};
#[cfg(not(target_arch = "wasm32"))]
pub use server::cancellation::RequestHandlerExtra;
#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    auth,
    simple_prompt::{SimplePrompt, SyncPrompt},
    simple_resources::{DynamicResourceHandler, ResourceCollection, StaticResource},
    simple_tool::{SimpleTool, SyncTool},
    typed_tool::{SimpleToolExt, SyncToolExt, TypedSyncTool, TypedTool, TypedToolWithOutput},
    ui::UIResourceBuilder,
    PromptHandler, ResourceHandler, SamplingHandler, Server, ServerBuilder, ToolHandler,
};
#[cfg(target_arch = "wasm32")]
pub use server::{
    wasm_server::{
        SimpleTool, WasmMcpServer, WasmMcpServerBuilder, WasmPrompt, WasmResource, WasmTool,
    },
    wasm_typed_tool::WasmTypedTool,
};
// Re-export WASM server types under their native names for compatibility
#[cfg(target_arch = "wasm32")]
pub use server::wasm_server::{WasmMcpServer as Server, WasmMcpServerBuilder as ServerBuilder};
#[cfg(target_arch = "wasm32")]
pub use server::wasm_typed_tool::WasmTypedTool as TypedTool;
#[cfg(not(target_arch = "wasm32"))]
pub use shared::StdioTransport;
pub use shared::{
    batch::{BatchRequest, BatchResponse},
    uri_template::UriTemplate,
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, RetryMiddleware, Transport,
};

#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub use shared::{WebSocketConfig, WebSocketTransport};

#[cfg(all(feature = "websocket-wasm", target_arch = "wasm32"))]
pub use shared::{WasmWebSocketConfig, WasmWebSocketTransport};

#[cfg(target_arch = "wasm32")]
pub use shared::{WasmHttpClient, WasmHttpConfig, WasmHttpTransport};

#[cfg(all(feature = "http", not(target_arch = "wasm32")))]
pub use shared::{HttpConfig, HttpTransport};
pub use types::{
    AuthInfo, AuthScheme, CallToolRequest, CallToolResult, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, CompletionArgument, CompletionReference,
    Content, CreateMessageParams, CreateMessageRequest, CreateMessageResult, GetPromptResult,
    Implementation, IncludeContext, ListResourcesResult, ListToolsResult, LoggingLevel,
    MessageContent, ModelPreferences, ProgressNotification, ProgressToken, PromptMessage,
    ProtocolVersion, ReadResourceResult, RequestId, ResourceInfo, Role, RootsCapabilities,
    SamplingCapabilities, SamplingMessage, ServerCapabilities, ServerNotification, ServerRequest,
    TokenUsage, ToolCapabilities, ToolInfo, UIMimeType, UIResource, UIResourceContents,
};

/// Type alias for [`CallToolResult`] - provides convenient access to tool execution results
///
/// This alias was added to resolve the common expectation that `ToolResult` should be
/// importable directly from the crate root. It provides the same functionality as
/// [`CallToolResult`] but with a more intuitive name for users implementing MCP tools.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use pmcp::{ToolResult, Content};
///
/// // Create a successful tool result
/// let result = ToolResult {
///     content: vec![Content::Text {
///         text: "Operation completed successfully".to_string(),
///     }],
///     is_error: false,
///     ..Default::default()
/// };
///
/// assert_eq!(result.content.len(), 1);
/// assert!(!result.is_error);
/// ```
///
/// Error handling:
///
/// ```rust
/// use pmcp::{ToolResult, Content};
///
/// // Create an error result
/// let error_result = ToolResult {
///     content: vec![Content::Text {
///         text: "Tool execution failed: Invalid input parameter".to_string(),
///     }],
///     is_error: true,
///     ..Default::default()
/// };
///
/// assert!(error_result.is_error);
/// ```
///
/// Using with different content types:
///
/// ```rust
/// use pmcp::{ToolResult, Content};
///
/// // Tool result with resource content
/// let resource_result = ToolResult {
///     content: vec![Content::Resource {
///         uri: "file:///tmp/output.txt".to_string(),
///         text: Some("File contents here...".to_string()),
///         mime_type: Some("text/plain".to_string()),
///         meta: None,
///     }],
///     is_error: false,
///     ..Default::default()
/// };
///
/// match &resource_result.content[0] {
///     Content::Resource { uri, mime_type, .. } => {
///         assert_eq!(uri, "file:///tmp/output.txt");
///         assert_eq!(mime_type, &Some("text/plain".to_string()));
///     }
///     _ => panic!("Expected resource content"),
/// }
/// ```
///
/// Serialization and JSON compatibility:
///
/// ```rust
/// use pmcp::{ToolResult, Content};
/// use serde_json;
///
/// let result = ToolResult {
///     content: vec![Content::Text {
///         text: "Hello, MCP!".to_string(),
///     }],
///     is_error: false,
///     ..Default::default()
/// };
///
/// // Serialize to JSON
/// let json_str = serde_json::to_string(&result).unwrap();
/// println!("Serialized: {}", json_str);
///
/// // Deserialize back
/// let deserialized: ToolResult = serde_json::from_str(&json_str).unwrap();
/// assert_eq!(result.content.len(), deserialized.content.len());
/// ```
pub use types::CallToolResult as ToolResult;
#[cfg(not(target_arch = "wasm32"))]
pub use utils::{BatchingConfig, DebouncingConfig, MessageBatcher, MessageDebouncer};

// Re-export async_trait for convenience
pub use async_trait::async_trait;

/// Protocol version constants
///
/// # Examples
///
/// ```rust
/// use pmcp::LATEST_PROTOCOL_VERSION;
///
/// // Use in client initialization
/// let protocol_version = LATEST_PROTOCOL_VERSION;
/// println!("Using MCP protocol version: {}", protocol_version);
///
/// // Check if a version is the latest
/// assert_eq!(LATEST_PROTOCOL_VERSION, "2025-06-18");
/// ```
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// Default protocol version to use for negotiation
///
/// # Examples
///
/// ```rust
/// use pmcp::DEFAULT_PROTOCOL_VERSION;
///
/// // Use as fallback when negotiating protocol version
/// let negotiated_version = DEFAULT_PROTOCOL_VERSION;
/// println!("Negotiating with protocol version: {}", negotiated_version);
///
/// // This is typically used internally by the SDK
/// assert_eq!(DEFAULT_PROTOCOL_VERSION, "2025-03-26");
/// ```
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

/// List of all protocol versions supported by this SDK
///
/// # Examples
///
/// ```rust
/// use pmcp::SUPPORTED_PROTOCOL_VERSIONS;
///
/// // Check if a version is supported
/// let version_to_check = "2025-03-26";
/// let is_supported = SUPPORTED_PROTOCOL_VERSIONS.contains(&version_to_check);
/// assert!(is_supported);
///
/// // List all supported versions
/// println!("Supported MCP protocol versions:");
/// for version in SUPPORTED_PROTOCOL_VERSIONS {
///     println!("  - {}", version);
/// }
///
/// // Use in version negotiation
/// fn negotiate_version(client_version: &str) -> Option<&'static str> {
///     SUPPORTED_PROTOCOL_VERSIONS.iter()
///         .find(|&&v| v == client_version)
///         .copied()
/// }
/// ```
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

/// Default request timeout in milliseconds
///
/// # Examples
///
/// ```rust
/// use pmcp::DEFAULT_REQUEST_TIMEOUT_MS;
/// use std::time::Duration;
///
/// // Convert to Duration for use with timeouts
/// let timeout = Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS);
/// println!("Default timeout: {:?}", timeout);
///
/// // Use in custom transport configuration
/// struct TransportConfig {
///     timeout_ms: u64,
/// }
///
/// impl Default for TransportConfig {
///     fn default() -> Self {
///         Self {
///             timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
///         }
///     }
/// }
///
/// // Verify default value
/// assert_eq!(DEFAULT_REQUEST_TIMEOUT_MS, 60_000); // 60 seconds
/// ```
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 60_000;

/// Server-side logging function (placeholder for examples).
///
/// In a real server context, this would send a `LogMessage` notification.
/// For examples, this is a no-op.
#[allow(clippy::unused_async)]
pub async fn log(
    _level: types::protocol::LogLevel,
    _message: &str,
    _data: Option<serde_json::Value>,
) {
    // In a real implementation, this would:
    // 1. Get the current server context
    // 2. Send a LogMessage notification through the transport
    // For now, this is a placeholder for the examples
}
