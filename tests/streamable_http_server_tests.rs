// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#[cfg(all(feature = "streamable-http", feature = "v1-compat"))]
mod streamable_http_server_tests {
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
    use pmcp::server::Server;
    use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::{
        ClientCapabilities, ClientRequest, Implementation, InitializeRequest, Request,
    };
    // Use boxed error for tests to satisfy clippy
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use url::Url;

    #[tokio::test]
    async fn test_initialization_generates_session_id() -> Result<()> {
        // Setup server with stateful mode (default)
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization without session ID
        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client
            .send(init_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Verify session ID was assigned
        assert!(
            client.session_id().is_some(),
            "Session ID should be generated on initialization"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_non_init_requires_session_in_stateful_mode() -> Result<()> {
        // Setup server with stateful mode
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client without session
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None, // No session ID
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send non-init request without session ID - should fail
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result = client.send(ping_message).await;
        assert!(
            result.is_err(),
            "Non-init request without session should fail"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_mode_no_session_required() -> Result<()> {
        // Setup server in stateless mode
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
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
        let (server_addr, server_task) = http_server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization
        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client
            .send(init_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // No session ID should be generated in stateless mode
        assert!(
            client.session_id().is_none(),
            "No session ID in stateless mode"
        );

        // Non-init requests should still work without session
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client
            .send(ping_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_protocol_version_header_included() -> Result<()> {
        // Setup server
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization
        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client
            .send(init_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Protocol version should be set after initialization
        assert!(
            client.protocol_version().is_some(),
            "Protocol version should be set"
        );

        server_task.abort();
        Ok(())
    }
}
