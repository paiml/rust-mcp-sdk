// Crate-level rustdoc is sourced from CRATE-README.md via include_str! so that
// docs.rs and GitHub render from a single authoritative source. Every
// `rust,no_run` code block inside CRATE-README.md is compiled as a doctest
// under `cargo test --doc`, which catches API drift automatically.
#![doc = include_str!("../CRATE-README.md")]
//!
//! ## The `v1-compat` feature
//!
//! `v1-compat` is **default-on** and gates the MCP 2025-11-25 compatibility
//! layer: the `initialize`/session lifecycle and SSE resumability
//! (`Last-Event-ID` plus its event store). SSE framing and parsing themselves
//! are shared with the 2026-07-28 (v2) path and are deliberately *not* gated.
//!
//! Because it is in `default`, consumers need to do nothing. Building
//! **without** it —
//! `cargo build -p pmcp --no-default-features --features full-v2` — is the
//! severability proof: `full-v2` is the `full` feature list minus exactly
//! `v1-compat`, so the crate still compiles the real transport while the v1
//! layer is absent.
//!
//! Removal is condition-gated on public client adoption of v2 and carries no
//! date; the normative policy lives at `docs/v1-sunset-policy.md`
//! ([online copy](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/v1-sunset-policy.md)).
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
// _meta is a protocol field name mandated by the MCP spec; suppress underscore lint
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::result_large_err)]

#[macro_use]
#[allow(unused_macros)]
mod generated_contracts;

pub mod assets;
pub mod client;
#[cfg(feature = "composition")]
pub mod composition;
pub mod error;
pub mod runtime;
pub mod secrets;
pub mod server;
pub mod shared;
pub mod types;
pub mod utils;

/// Conformance helpers for proving the `tasks/*` wire surface round-trips
/// through the real client deserialization types.
///
/// Feature-gated behind `testing` (folded into `full`) so it is available to
/// integration tests, examples, and the quality gate, but omitted from lean
/// default release builds. See [`testing::assert_roundtrips_through_client`].
#[cfg(any(test, feature = "testing"))]
pub mod testing;

#[cfg(feature = "simd")]
pub mod simd;

/// Axum Router convenience API for secure MCP server hosting.
///
/// Re-exports `router`, `router_with_config`,
/// `RouterConfig`, and `AllowedOrigins`
/// for ergonomic usage: `pmcp::axum::router(server)`.
#[cfg(feature = "streamable-http")]
pub mod axum {
    pub use crate::server::axum_router::{
        router, router_with_config, AllowedOrigins, RouterConfig,
    };
}

// Re-export commonly used types
pub use client::{Client, ClientBuilder, ClientOptions, ToolCallResponse, WaitForTaskOptions};
pub use error::{Error, ErrorCode, Result};
#[cfg(not(target_arch = "wasm32"))]
pub use server::cancellation::RequestHandlerExtra;
#[cfg(not(target_arch = "wasm32"))]
pub use server::task_store::{InMemoryTaskStore, StoreConfig, TaskStore, TaskStoreError};
#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    auth,
    limits::PayloadLimits,
    simple_prompt::{SimplePrompt, SyncPrompt},
    simple_resources::{DynamicResourceHandler, ResourceCollection, StaticResource},
    simple_tool::{SimpleTool, SyncTool},
    state::State,
    typed_prompt::TypedPrompt,
    typed_tool::{SimpleToolExt, SyncToolExt, TypedSyncTool, TypedTool, TypedToolWithOutput},
    ui::UIResourceBuilder,
    McpServer, PromptHandler, ResourceHandler, SamplingHandler, Server, ServerBuilder, ToolHandler,
    ToolOutput,
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
// Re-export proc macros from pmcp-macros so users can write `use pmcp::{mcp_tool, mcp_server}`
// instead of adding pmcp-macros as a separate dependency.
#[cfg(feature = "macros")]
pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};

#[cfg(not(target_arch = "wasm32"))]
pub use shared::StdioTransport;

/// Target-agnostic PKCE (RFC 7636) crypto helper — re-exported UNGATED so the
/// path resolves on both host and wasm32 (model: `StdioTransport` above, NOT the
/// wasm-gated transport re-exports below).
pub use shared::pkce::{code_challenge_s256, generate_code_verifier, generate_state};

/// Target-agnostic OAuth authorization-RESPONSE validation (RFC 9207 `iss` +
/// CSRF `state`) — re-exported UNGATED for the same reason as the PKCE helper
/// above: a Workers/Lambda redirect handler must be able to reach it without
/// the `oauth` feature. `iss_presence_from` and `parse_iss_env_value` stay on
/// the module path, since only a client builder resolves precedence.
pub use shared::oauth_validation::{
    validate_authorization_response, AuthorizationRequestRecord, IssPresence,
};

/// Target-agnostic OAuth credential storage (SEP-2352's `(issuer, account,
/// server)` key, the document format, the schema migration and the platform
/// seam) — re-exported UNGATED for the same reason as the two helpers above: a
/// hosting platform must be able to implement the store without the `oauth`
/// feature. `normalize_server_key`, `DroppedEntry` and
/// `CREDENTIAL_SCHEMA_VERSION` stay on the module path.
pub use shared::credential_store::{
    parse_credential_snapshot, CredentialKey, CredentialSnapshot, CredentialStore,
    CredentialStoreAdmin, InMemoryCredentialStore, MigrationReport, StoredCredentials,
};

/// The DEFAULT on-disk credential store — gated, unlike the tier above, because
/// a file under the user's home directory is exactly what a hosting platform
/// cannot use. `CREDENTIAL_LOCK_SUFFIX` and `CREDENTIAL_LOCK_STALE_SECS` stay on
/// the module path, since only an operator diagnosing a stray lock needs them.
#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
pub use shared::credential_file::{default_credential_path, FileCredentialStore};

/// Peer back-channel trait for server-to-client RPCs from inside request handlers.
#[cfg(not(target_arch = "wasm32"))]
pub use shared::peer::PeerHandle;

/// Unstable test-support re-exports for internal integration tests.
///
/// **Not part of the stable API surface.** This module is hidden from docs
/// and may change or be removed without notice; it exists solely so the
/// integration tests under `tests/server_request_dispatcher_integration.rs`
/// can exercise the otherwise-`pub(crate)` dispatcher.
#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod __test_support {
    pub use crate::server::peer_impl::DispatchPeerHandle;
    pub use crate::server::server_request_dispatcher::{
        spawn_server_request_drain, ServerRequestDispatcher, DEFAULT_DISPATCH_TIMEOUT,
    };
    // TOUT-02 double-wrap tripwire seam: the `looks_like_call_tool_result`
    // marker fn + `double_wrap_tripwire` decision fn live in the crate-private
    // `task_dispatch` module, so the `tests/double_wrap_tripwire.rs` integration
    // binary reaches them here for the helper-level precision + debug-panic
    // tests without spinning up a full dispatch.
    pub use crate::server::task_dispatch::{
        double_wrap_tripwire, looks_like_call_tool_result, DoubleWrapMarker,
    };
    pub use crate::types::ServerRequest;
}

/// Tower middleware layers for MCP HTTP security.
#[cfg(feature = "streamable-http")]
pub use server::tower_layers::{AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer};

pub use shared::{
    batch::{BatchRequest, BatchResponse},
    uri_template::UriTemplate,
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, RetryMiddleware, SharedSender,
    Transport,
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
    Content, CreateMessageParams, CreateMessageResult, GetPromptResult, Implementation,
    IncludeContext, ListResourcesResult, ListToolsResult, LoggingLevel, ModelPreferences,
    ProgressNotification, ProgressToken, PromptMessage, ProtocolVersion, ReadResourceResult,
    RequestId, ResourceInfo, Role, RootsCapabilities, SamplingCapabilities, SamplingMessage,
    ServerCapabilities, ServerNotification, ServerRequest, TokenUsage, ToolCapabilities, ToolInfo,
    UIMimeType, UIResource, UIResourceContents,
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
/// let result = ToolResult::new(vec![Content::text("Operation completed successfully")]);
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
/// let error_result = ToolResult::error(vec![
///     Content::text("Tool execution failed: Invalid input parameter"),
/// ]);
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
/// let resource_result = ToolResult::new(vec![
///     Content::resource_with_text("file:///tmp/output.txt", "File contents here...", "text/plain"),
/// ]);
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
/// let result = ToolResult::new(vec![Content::text("Hello, MCP!")]);
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
/// assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25");
/// ```
///
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
///
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
/// // 4 supported versions (2025 + backward-compat 2024)
/// assert_eq!(SUPPORTED_PROTOCOL_VERSIONS.len(), 4);
///
/// // 2024-11-05 accepted for backward compatibility with existing clients
/// assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&"2024-11-05"));
/// ```
pub use types::protocol::version::{
    negotiate_protocol_version, DEFAULT_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION,
    SUPPORTED_PROTOCOL_VERSIONS,
};

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
