//! Transport adapter patterns for connecting protocol handlers to various transports.
//!
//! This module provides the adapter interface and implementations that bridge
//! the transport-independent `ServerCore` with specific transport mechanisms.

use crate::error::Result;
use crate::shared::{Transport as TransportTrait, TransportMessage};
// Types are re-exported through TransportMessage
#[cfg(test)]
use crate::types::{JSONRPCResponse, Request, RequestId};
use async_trait::async_trait;
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use futures::lock::RwLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;

use super::core::ProtocolHandler;

/// Transport adapter trait for binding protocol handlers to specific transports.
///
/// This trait defines how a protocol handler (like `ServerCore`) can be connected
/// to different transport mechanisms (stdio, HTTP, WebSocket, WASI HTTP, etc.).
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait TransportAdapter: Send + Sync {
    /// Serve the protocol handler using this transport.
    ///
    /// This method starts serving requests using the specific transport
    /// implementation, forwarding them to the protocol handler.
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()>;

    /// Get the transport type name.
    fn transport_type(&self) -> &'static str;
}

/// Transport adapter trait for WASM environments (single-threaded).
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait TransportAdapter {
    /// Serve the protocol handler using this transport.
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()>;

    /// Get the transport type name.
    fn transport_type(&self) -> &'static str;
}

/// Generic transport adapter that works with any Transport implementation.
///
/// This adapter provides a common implementation for transports that implement
/// the Transport trait, reducing code duplication.
#[derive(Debug)]
pub struct GenericTransportAdapter<T: TransportTrait> {
    transport: Arc<RwLock<T>>,
}

impl<T: TransportTrait> GenericTransportAdapter<T> {
    /// Create a new generic transport adapter.
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
        }
    }

    /// Process messages from the transport.
    async fn process_messages(
        transport: Arc<RwLock<T>>,
        handler: Arc<dyn ProtocolHandler>,
    ) -> Result<()> {
        loop {
            // Receive message from transport
            let message = {
                let mut t = transport.write().await;
                if !t.is_connected() {
                    break;
                }
                if let Ok(msg) = t.receive().await {
                    msg
                } else {
                    // Connection likely closed, check and break if so
                    if !t.is_connected() {
                        break;
                    }
                    return Err(crate::error::Error::internal("Transport receive failed"));
                }
            };

            // Process the message
            match message {
                TransportMessage::Request { id, request } => {
                    let response = handler.handle_request(id, request, None).await;
                    let mut t = transport.write().await;
                    t.send(TransportMessage::Response(response)).await?;
                },
                TransportMessage::Notification(notification) => {
                    handler.handle_notification(notification).await?;
                },
                TransportMessage::Response(_) => {
                    // Servers don't typically receive responses
                    tracing::warn!("Server received unexpected response message");
                },
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<T: TransportTrait + 'static> TransportAdapter for GenericTransportAdapter<T> {
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        // Process messages
        let result = Self::process_messages(self.transport.clone(), handler).await;

        // Close the transport when done
        {
            let mut t = self.transport.write().await;
            let _ = t.close().await;
        }

        result
    }

    fn transport_type(&self) -> &'static str {
        "generic"
    }
}

/// STDIO transport adapter.
///
/// This adapter connects a protocol handler to standard input/output streams,
/// commonly used for CLI-based MCP servers.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct StdioAdapter {
    inner: GenericTransportAdapter<crate::shared::stdio::StdioTransport>,
}

#[cfg(not(target_arch = "wasm32"))]
impl StdioAdapter {
    /// Create a new STDIO adapter.
    pub fn new() -> Self {
        use crate::shared::stdio::StdioTransport;
        Self {
            inner: GenericTransportAdapter::new(StdioTransport::new()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for StdioAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl TransportAdapter for StdioAdapter {
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        self.inner.serve(handler).await
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }
}

/// HTTP transport adapter for stateless HTTP-based communication.
///
/// This adapter is designed for serverless environments where each HTTP request
/// contains a complete MCP request and expects a complete response.
#[cfg(feature = "http")]
#[derive(Debug)]
pub struct HttpAdapter {
    // HTTP-specific configuration could go here
}

#[cfg(feature = "http")]
impl HttpAdapter {
    /// Create a new HTTP adapter.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a single HTTP request containing an MCP message.
    ///
    /// This method is designed to be called from HTTP request handlers in
    /// serverless environments or web servers.
    pub async fn handle_http_request(
        &self,
        handler: Arc<dyn ProtocolHandler>,
        body: String,
    ) -> Result<String> {
        // Parse the incoming request
        let message: TransportMessage = serde_json::from_str(&body)?;

        match message {
            TransportMessage::Request { id, request } => {
                let response = handler.handle_request(id, request, None).await;
                Ok(serde_json::to_string(&TransportMessage::Response(
                    response,
                ))?)
            },
            TransportMessage::Notification(notification) => {
                handler.handle_notification(notification).await?;
                Ok("".to_string()) // No response for notifications
            },
            TransportMessage::Response(_) => Err(crate::error::Error::protocol(
                crate::error::ErrorCode::INVALID_REQUEST,
                "HTTP adapter only accepts requests and notifications",
            )),
        }
    }
}

#[cfg(feature = "http")]
impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "http")]
#[async_trait]
impl TransportAdapter for HttpAdapter {
    async fn serve(&self, _handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        // HTTP adapter doesn't run a continuous serve loop
        // Instead, it handles individual requests via handle_http_request
        Err(crate::error::Error::internal(
            "HTTP adapter should be used with handle_http_request method",
        ))
    }

    fn transport_type(&self) -> &'static str {
        "http"
    }
}

/// WebSocket transport adapter.
///
/// This adapter connects a protocol handler to WebSocket connections,
/// enabling real-time bidirectional communication.
#[cfg(feature = "websocket")]
#[derive(Debug)]
pub struct WebSocketAdapter<T: TransportTrait> {
    inner: GenericTransportAdapter<T>,
}

#[cfg(feature = "websocket")]
impl<T: TransportTrait + 'static> WebSocketAdapter<T> {
    /// Create a new WebSocket adapter with the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            inner: GenericTransportAdapter::new(transport),
        }
    }
}

#[cfg(feature = "websocket")]
#[async_trait]
impl<T: TransportTrait + 'static> TransportAdapter for WebSocketAdapter<T> {
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        self.inner.serve(handler).await
    }

    fn transport_type(&self) -> &'static str {
        "websocket"
    }
}

/// Mock transport adapter for testing.
#[cfg(test)]
#[derive(Debug)]
pub struct MockAdapter {
    requests: Arc<RwLock<Vec<(RequestId, Request)>>>,
    responses: Arc<RwLock<Vec<JSONRPCResponse>>>,
}

#[cfg(test)]
impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl MockAdapter {
    /// Create a new mock adapter.
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(Vec::new())),
            responses: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a request to be processed.
    pub async fn add_request(&self, id: RequestId, request: Request) {
        self.requests.write().await.push((id, request));
    }

    /// Get all responses that were generated.
    pub async fn get_responses(&self) -> Vec<JSONRPCResponse> {
        self.responses.read().await.clone()
    }
}

#[cfg(test)]
#[async_trait]
impl TransportAdapter for MockAdapter {
    async fn serve(&self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        let requests = self.requests.read().await.clone();
        for (id, request) in requests {
            let response = handler.handle_request(id, request, None).await;
            self.responses.write().await.push(response);
        }
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::core::ServerCore;
    use crate::types::{ClientRequest, Implementation, InitializeRequest, ServerCapabilities};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_mock_adapter() {
        use crate::runtime::RwLock;
        use crate::shared::middleware::EnhancedMiddlewareChain;

        let server = ServerCore::new(
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
            ServerCapabilities::tools_only(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(
                crate::server::tool_middleware::ToolMiddlewareChain::new(),
            )),
            None, // task_router
            false,
        );

        let handler = Arc::new(server);
        let adapter = MockAdapter::new();

        // Add an initialization request
        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: crate::types::ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));

        adapter.add_request(RequestId::from(1i64), init_req).await;

        // Serve the requests
        adapter.serve(handler).await.unwrap();

        // Check responses
        let responses = adapter.get_responses().await;
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].id, RequestId::from(1i64));
    }
}
