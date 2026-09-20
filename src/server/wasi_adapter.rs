//! WASI HTTP adapter for serverless MCP deployments.
//!
//! This module provides a WASI-compatible HTTP adapter that enables deployment
//! of MCP servers to WebAssembly System Interface (WASI) environments such as
//! Cloudflare Workers, Vercel Edge Functions, and other WASI-compliant runtimes.

#[cfg(target_arch = "wasm32")]
use crate::error::Result;
#[cfg(target_arch = "wasm32")]
use crate::server::ProtocolHandler;
#[cfg(target_arch = "wasm32")]
use crate::shared::TransportMessage;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

/// WASI HTTP adapter for handling MCP requests in serverless environments.
///
/// This adapter is designed to work with WASI HTTP interfaces, processing
/// individual HTTP requests containing MCP messages and returning appropriate
/// responses. It operates in a stateless manner suitable for serverless
/// deployment.
///
/// # Examples
///
/// ```rust,no_run
/// #[cfg(target_arch = "wasm32")]
/// use pmcp::server::wasi_adapter::WasiHttpAdapter;
/// use pmcp::server::builder::ServerCoreBuilder;
/// use std::sync::Arc;
///
/// #[cfg(target_arch = "wasm32")]
/// async fn handle_request(body: String) -> Result<String, Box<dyn std::error::Error>> {
///     // Build the server
///     let server = ServerCoreBuilder::new()
///         .name("wasi-server")
///         .version("1.0.0")
///         .build()?;
///     
///     let handler = Arc::new(server);
///     let adapter = WasiHttpAdapter::new();
///     
///     // Process the request
///     adapter.handle_request(handler, body).await
/// }
/// ```
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct WasiHttpAdapter {
    /// Optional session management configuration
    session_enabled: bool,
}

#[cfg(target_arch = "wasm32")]
impl WasiHttpAdapter {
    /// Create a new WASI HTTP adapter with default settings.
    pub fn new() -> Self {
        Self {
            session_enabled: false,
        }
    }

    /// Create a new WASI HTTP adapter with session support.
    ///
    /// When session support is enabled, the adapter can maintain state
    /// across requests using session identifiers in HTTP headers.
    pub fn with_sessions() -> Self {
        Self {
            session_enabled: true,
        }
    }

    /// Handle a single HTTP request containing an MCP message.
    ///
    /// This method processes the incoming HTTP request body as an MCP message,
    /// forwards it to the protocol handler, and returns the response as a
    /// JSON string suitable for the HTTP response body.
    ///
    /// # Arguments
    ///
    /// * `handler` - The protocol handler to process the MCP request
    /// * `body` - The HTTP request body containing the MCP message as JSON
    ///
    /// # Returns
    ///
    /// A JSON string containing the MCP response, or an error.
    pub async fn handle_request(
        &self,
        handler: Arc<dyn ProtocolHandler>,
        body: String,
    ) -> Result<String> {
        // Parse the incoming request
        let message: TransportMessage = serde_json::from_str(&body)?;

        match message {
            TransportMessage::Request { id, request } => {
                // Handle the request
                let response = handler.handle_request(id, request).await;

                // Serialize the response
                Ok(serde_json::to_string(&TransportMessage::Response(
                    response,
                ))?)
            },
            TransportMessage::Notification(notification) => {
                // Handle the notification (no response expected)
                handler.handle_notification(notification).await?;

                // Return empty response for notifications
                Ok("{}".to_string())
            },
            _ => {
                // WASI HTTP adapter only accepts requests and notifications
                Err(crate::error::Error::protocol(
                    crate::error::ErrorCode::INVALID_REQUEST,
                    "WASI HTTP adapter only accepts requests and notifications",
                ))
            },
        }
    }

    /// Handle a request with HTTP headers for session management.
    ///
    /// This method extends `handle_request` with support for session management
    /// through HTTP headers, enabling stateful operations in serverless environments
    /// that support it.
    ///
    /// # Arguments
    ///
    /// * `handler` - The protocol handler to process the MCP request
    /// * `body` - The HTTP request body containing the MCP message as JSON
    /// * `headers` - HTTP headers that may contain session information
    ///
    /// # Returns
    ///
    /// A tuple containing the response body and optional session ID for the response headers.
    pub async fn handle_request_with_session(
        &self,
        handler: Arc<dyn ProtocolHandler>,
        body: String,
        headers: Vec<(String, String)>,
    ) -> Result<(String, Option<String>)> {
        // Extract session ID from headers if session management is enabled
        let session_id = if self.session_enabled {
            headers
                .iter()
                .find(|(name, _)| name.to_lowercase() == "x-mcp-session-id")
                .map(|(_, value)| value.clone())
        } else {
            None
        };

        // Process the request
        let response = self.handle_request(handler, body).await?;

        // Return response with session ID if applicable
        Ok((response, session_id))
    }

    /// Check if session management is enabled.
    pub fn is_session_enabled(&self) -> bool {
        self.session_enabled
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WasiHttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper module for WASI HTTP world integration using wit-bindgen.
///
/// This module provides the glue code for integrating with WASI HTTP
/// interfaces through the wit-bindgen tool.
#[cfg(all(target_arch = "wasm32", feature = "wasi-http"))]
pub mod wasi_http_world {
    use super::*;
    use wit_bindgen::generate;

    // Generate WASI HTTP bindings
    generate!({
        world: "wasi:http/proxy@0.2.0",
    });

    use exports::wasi::http::incoming_handler::Guest;
    use wasi::http::types::{
        Headers, IncomingBody, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
    };

    /// WASI HTTP handler implementation.
    ///
    /// This struct implements the WASI HTTP Guest trait to handle
    /// incoming HTTP requests in WASI environments.
    pub struct WasiHttpHandler {
        adapter: WasiHttpAdapter,
        handler: Arc<dyn ProtocolHandler>,
    }

    impl WasiHttpHandler {
        /// Create a new WASI HTTP handler.
        pub fn new(handler: Arc<dyn ProtocolHandler>) -> Self {
            Self {
                adapter: WasiHttpAdapter::new(),
                handler,
            }
        }

        /// Create a new WASI HTTP handler with session support.
        pub fn with_sessions(handler: Arc<dyn ProtocolHandler>) -> Self {
            Self {
                adapter: WasiHttpAdapter::with_sessions(),
                handler,
            }
        }

        /// Process an incoming HTTP request.
        async fn process_request(&self, request: IncomingRequest) -> Result<OutgoingResponse> {
            // Get the request body
            let body = request
                .consume()
                .map_err(|_| crate::error::Error::parse("Failed to consume request body"))?;

            let body_stream = body
                .stream()
                .map_err(|_| crate::error::Error::parse("Failed to get body stream"))?;

            // Read the body content
            let mut body_bytes = Vec::new();
            loop {
                match body_stream.blocking_read(1024 * 64) {
                    Ok(chunk) => {
                        if chunk.is_empty() {
                            break;
                        }
                        body_bytes.extend_from_slice(&chunk);
                    },
                    Err(_) => break,
                }
            }

            let body_string = String::from_utf8(body_bytes)
                .map_err(|_| crate::error::Error::parse("Invalid UTF-8 in request body"))?;

            // Process the MCP request
            let response_body = self
                .adapter
                .handle_request(self.handler.clone(), body_string)
                .await?;

            // Create the response
            let response = OutgoingResponse::new(Headers::new());
            response.set_status_code(200).ok();

            let response_body_bytes = response_body.into_bytes();
            let body = response
                .body()
                .map_err(|_| crate::error::Error::internal("Failed to get response body"))?;

            let stream = body
                .write()
                .map_err(|_| crate::error::Error::internal("Failed to get body stream"))?;

            stream
                .blocking_write_and_flush(&response_body_bytes)
                .map_err(|_| crate::error::Error::internal("Failed to write response"))?;

            OutgoingBody::finish(body, None)
                .map_err(|_| crate::error::Error::internal("Failed to finish response body"))?;

            Ok(response)
        }
    }

    impl Guest for WasiHttpHandler {
        fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
            // Create a runtime for async processing
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime");

            // Process the request
            let result = runtime.block_on(async { self.process_request(request).await });

            // Set the response
            match result {
                Ok(response) => {
                    ResponseOutparam::set(response_out, Ok(response));
                },
                Err(e) => {
                    // Create error response
                    let error_response = OutgoingResponse::new(Headers::new());
                    error_response.set_status_code(500).ok();

                    let error_body = format!("{{\"error\": \"{}\"}}", e);
                    if let Ok(body) = error_response.body() {
                        if let Ok(stream) = body.write() {
                            let _ = stream.blocking_write_and_flush(error_body.as_bytes());
                        }
                        let _ = OutgoingBody::finish(body, None);
                    }

                    ResponseOutparam::set(response_out, Ok(error_response));
                },
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod tests {
    use super::*;
    use crate::server::builder::ServerCoreBuilder;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::ToolHandler;
    use crate::types::{
        CallToolRequest, ClientRequest, Implementation, InitializeRequest, Request, RequestId,
        ServerRequest,
    };
    use async_trait::async_trait;
    use serde_json::Value;

    struct TestTool;

    #[async_trait]
    impl ToolHandler for TestTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(serde_json::json!({"result": "wasi-test"}))
        }
    }

    #[tokio::test]
    async fn test_wasi_adapter_request() {
        let server = ServerCoreBuilder::new()
            .name("wasi-test-server")
            .version("1.0.0")
            .tool("test-tool", TestTool)
            .build()
            .unwrap();

        let handler = Arc::new(server);
        let adapter = WasiHttpAdapter::new();

        // Create an initialization request
        let init_request = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
                protocol_version: "2024-11-05".to_string(),
                capabilities: crate::types::ClientCapabilities::default(),
                client_info: Implementation::new("test-client", "1.0.0"),
            }))),
        };

        let request_body = serde_json::to_string(&init_request).unwrap();
        let response = adapter.handle_request(handler, request_body).await.unwrap();

        // Parse and verify response
        let response_message: TransportMessage = serde_json::from_str(&response).unwrap();
        match response_message {
            TransportMessage::Response(resp) => {
                assert_eq!(resp.id, RequestId::from(1i64));
            },
            _ => panic!("Expected response message"),
        }
    }

    #[tokio::test]
    async fn test_wasi_adapter_notification() {
        let server = ServerCoreBuilder::new()
            .name("wasi-test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let handler = Arc::new(server);
        let adapter = WasiHttpAdapter::new();

        // Create a notification
        let notification = TransportMessage::Notification(crate::types::Notification::Server(
            crate::types::ServerNotification::Cancelled(
                crate::types::CancelledNotification::new(RequestId::from(1i64)).with_reason("test"),
            ),
        ));

        let request_body = serde_json::to_string(&notification).unwrap();
        let response = adapter.handle_request(handler, request_body).await.unwrap();

        // Notifications should return empty response
        assert_eq!(response, "{}");
    }

    #[tokio::test]
    async fn test_wasi_adapter_with_sessions() {
        let adapter = WasiHttpAdapter::with_sessions();
        assert!(adapter.is_session_enabled());

        let adapter_no_session = WasiHttpAdapter::new();
        assert!(!adapter_no_session.is_session_enabled());
    }
}
