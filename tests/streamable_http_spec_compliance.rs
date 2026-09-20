//! Comprehensive spec compliance tests for streamable HTTP transport
// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#[cfg(all(feature = "streamable-http", feature = "v1-compat"))]
mod spec_compliance_tests {
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
    use pmcp::server::Server;
    use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::{
        ClientCapabilities, ClientRequest, Implementation, InitializeRequest, Request,
    };
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use url::Url;

    // Use boxed error for tests to satisfy clippy's large_enum_variant warning
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    // Helper to convert pmcp::Error to boxed error
    fn box_err(e: pmcp::Error) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(e)
    }

    // ==================== BASELINE TESTS (BOTH MODES) ====================

    #[tokio::test]
    async fn test_baseline_accept_header_validation() -> Result<()> {
        // Test in stateful mode (but applies to both)
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Test: POST missing Accept header → 406
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            406,
            "Missing Accept should return 406"
        );
        let error_body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error_body["jsonrpc"], "2.0");
        assert!(error_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Accept"));

        // Test: POST with wrong Accept header → 406
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "text/html")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            406,
            "Wrong Accept should return 406"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_baseline_content_type_validation() -> Result<()> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Test: POST with wrong Content-Type → 415
        let response = client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "text/plain")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            415,
            "Wrong Content-Type should return 415"
        );
        let error_body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error_body["jsonrpc"], "2.0");
        assert!(error_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Content-Type"));

        // Test: POST missing Content-Type → 415
        let response = client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            415,
            "Missing Content-Type should return 415"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_baseline_protocol_version_required_non_init() -> Result<()> {
        // Test that non-init requests MUST include protocol version header
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        // First initialize to establish a session (in stateful mode)
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;
        let session_id = client.session_id();

        // Now send a non-init request WITHOUT protocol version header
        let reqwest_client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        let ping_body = serde_json::json!({
            "id": 2,
            "request": {
                "method": "ping"
            }
        });

        let mut request = reqwest_client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json");
        // Deliberately NOT including mcp-protocol-version header

        if let Some(sid) = &session_id {
            request = request.header("mcp-session-id", sid);
        }

        let response = request.body(ping_body.to_string()).send().await.unwrap();

        // According to spec: "Non-initialize requests MUST include `mcp-protocol-version` header"
        // So this should return 400
        let status = response.status().as_u16();
        println!(
            "Response status without protocol version header: {}",
            status
        );

        // Note: Current implementation may not enforce this requirement
        // If it doesn't return 400, we need to fix the implementation

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_baseline_protocol_version_requirement() -> Result<()> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        // First initialize to establish we're testing non-init requests
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;
        let session_id = client.session_id();

        // Test: Non-init request without protocol version header → should it be 400?
        // According to spec: "Non-initialize requests MUST include `mcp-protocol-version` header"
        // Let's test with raw HTTP
        let reqwest_client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        let ping_body = serde_json::json!({
            "id": 2,
            "request": {
                "method": "ping"
            }
        });

        let mut request = reqwest_client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json");

        if let Some(sid) = &session_id {
            request = request.header("mcp-session-id", sid);
        }

        let response = request.body(ping_body.to_string()).send().await.unwrap();

        // Check if this returns 400 when missing protocol version
        // Current implementation might not enforce this
        println!(
            "Response status without protocol version: {}",
            response.status()
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_baseline_notifications_only_returns_202() -> Result<()> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        // Initialize first
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;

        // Get the negotiated protocol version from the client
        let protocol_version = client
            .protocol_version()
            .unwrap_or_else(|| pmcp::LATEST_PROTOCOL_VERSION.to_string());

        // Send a notification
        let _notification_message = TransportMessage::Notification(
            pmcp::types::Notification::Client(pmcp::types::ClientNotification::Initialized),
        );

        // This should work but StreamableHttpTransport might not support sending notifications
        // Let's use raw HTTP
        let reqwest_client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Since TransportMessage is untagged, a notification serializes as just the notification itself
        // And Notification::Client(ClientNotification::Initialized) should serialize properly
        let notification_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let mut request = reqwest_client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .header("mcp-protocol-version", &protocol_version);

        if let Some(sid) = client.session_id() {
            request = request.header("mcp-session-id", sid);
        }

        let response = request
            .body(notification_body.to_string())
            .send()
            .await
            .unwrap();

        let status = response.status().as_u16();
        if status != 202 {
            let body = response.text().await.unwrap();
            println!("Response status: {}, body: {}", status, body);
            panic!("Notification should return 202 Accepted, got {}", status);
        }

        server_task.abort();
        Ok(())
    }

    // ==================== STATEFUL MODE TESTS ====================

    #[tokio::test]
    async fn test_stateful_initialize_creates_session() -> Result<()> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        // Explicitly use stateful mode (default)
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None, // No session ID initially
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;

        // Session ID should be created
        assert!(
            client.session_id().is_some(),
            "Session ID should be created after initialization"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateful_concurrent_sse_conflict() -> Result<()> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(box_err)?;

        // Initialize to get session
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;
        let session_id = client.session_id().expect("Should have session ID");

        let reqwest_client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // First GET SSE - should succeed
        let response1 = reqwest_client
            .get(&url)
            .header("accept", "text/event-stream")
            .header("mcp-session-id", &session_id)
            .send()
            .await
            .unwrap();

        assert_eq!(response1.status().as_u16(), 200, "First SSE should succeed");

        // Second concurrent GET SSE - should return 409 Conflict
        let response2 = reqwest_client
            .get(&url)
            .header("accept", "text/event-stream")
            .header("mcp-session-id", &session_id)
            .send()
            .await
            .unwrap();

        assert_eq!(
            response2.status().as_u16(),
            409,
            "Second concurrent SSE should return 409 Conflict"
        );

        server_task.abort();
        Ok(())
    }

    // ==================== STATELESS MODE TESTS ====================

    async fn create_stateless_server() -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(box_err)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let config = StreamableHttpServerConfig {
            session_id_generator: None, // Stateless mode
            enable_json_response: false,
            event_store: None,
            on_session_initialized: None,
            on_session_closed: None,
            http_middleware: None,
            allowed_origins: None,
            max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
        };
        let http_server = StreamableHttpServer::with_config(addr, server, config);
        http_server.start().await.map_err(box_err)
    }

    #[tokio::test]
    async fn test_stateless_no_session_id_in_response() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;

        // No session ID should be created in stateless mode
        assert!(
            client.session_id().is_none(),
            "No session ID should be created in stateless mode"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_reinitialize_allowed() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        // First initialization
        client.send(init_message.clone()).await.map_err(box_err)?;
        let _response1 = client.receive().await.map_err(box_err)?;

        // Second initialization - should also succeed in stateless mode
        let init_message2 = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client.send(init_message2).await.map_err(box_err)?;
        let _response2 = client.receive().await.map_err(box_err)?;

        // Both should succeed in stateless mode

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_no_session_required() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None, // No session ID
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send non-init request without session ID - should work in stateless mode
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client.send(ping_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;

        // Should succeed without session ID in stateless mode

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_ignores_arbitrary_session_id() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| pmcp::Error::Internal(e.to_string()))
                .map_err(box_err)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: Some("arbitrary-session-id".to_string()), // Arbitrary session ID
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send request with arbitrary session ID - should be ignored in stateless mode
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client.send(ping_message).await.map_err(box_err)?;
        let _response = client.receive().await.map_err(box_err)?;

        // Should succeed, ignoring the arbitrary session ID

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_sse_returns_405() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Try GET SSE in stateless mode
        let response = client
            .get(&url)
            .header("accept", "text/event-stream")
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            405,
            "GET SSE should return 405 in stateless mode"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_delete_returns_404_or_405() -> Result<()> {
        let (server_addr, server_task) = create_stateless_server().await?;

        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Try DELETE without session in stateless mode
        let response = client.delete(&url).send().await.unwrap();

        let status = response.status().as_u16();
        assert!(
            status == 404 || status == 405,
            "DELETE without session should return 404 or 405 in stateless mode, got {}",
            status
        );

        server_task.abort();
        Ok(())
    }
}
