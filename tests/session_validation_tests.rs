// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#[cfg(all(feature = "streamable-http", feature = "v1-compat"))]
mod session_validation_tests {
    use pmcp::server::streamable_http_server::StreamableHttpServer;
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

    async fn create_test_server() -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        http_server
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    #[tokio::test]
    async fn test_double_initialization_rejected() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // Setup first client and initialize successfully
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
        let mut client1 = StreamableHttpTransport::new(client_config);

        // First initialization - should succeed
        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        client1
            .send(init_message.clone())
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response1 = client1
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let session_id = client1
            .session_id()
            .expect("Session should be set after first init");

        // Setup second client with same session ID and attempt re-initialization
        let client_config2 = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: Some(session_id),
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client2 = StreamableHttpTransport::new(client_config2);

        // Second initialization with same session - should fail
        let result = client2.send(init_message).await;
        assert!(result.is_err(), "Double initialization should fail");

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_unknown_session_id_returns_404() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // Setup client with invalid session ID
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: Some("invalid-session-id".to_string()),
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send non-init request with invalid session ID - should fail
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result = client.send(ping_message).await;
        assert!(result.is_err(), "Request with unknown session should fail");

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_non_init_without_session_returns_400() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // Setup client without session ID
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

        // Send non-init request without session ID - should fail
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result = client.send(ping_message).await;
        assert!(result.is_err(), "Request without session should fail");

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_session_id_in_all_responses() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // Setup client transport
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

        // Initialize
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
        let _init_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let session_id = client
            .session_id()
            .expect("Session ID should be set after init");

        // Send another request - session ID should still be present
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client
            .send(ping_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _ping_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Session ID should be preserved
        assert_eq!(client.session_id().unwrap(), session_id);

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_sse_endpoint_with_unknown_session() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;
        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Try GET SSE with unknown session ID
        let response = client
            .get(&url)
            .header("accept", "text/event-stream")
            .header("mcp-session-id", "unknown-session")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 404);

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_unknown_session() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;
        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Try DELETE with unknown session ID
        let response = client
            .delete(&url)
            .header("mcp-session-id", "unknown-session")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 404);

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_protocol_version_validation() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // Setup client and initialize
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

        // Initialize to get session and negotiated version
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
        let _init_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Verify protocol version is set
        assert!(
            client.protocol_version().is_some(),
            "Protocol version should be negotiated"
        );
        let negotiated_version = client.protocol_version().unwrap();

        // Verify it's a supported version
        assert!(
            pmcp::SUPPORTED_PROTOCOL_VERSIONS.contains(&negotiated_version.as_str()),
            "Negotiated version should be supported"
        );

        // Send another request to verify version consistency
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client
            .send(ping_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _ping_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Protocol version should remain the same
        assert_eq!(
            client.protocol_version().unwrap(),
            negotiated_version,
            "Protocol version should remain consistent"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_protocol_version_in_responses() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

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

        // Initialize
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
        let _init_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let negotiated_version = client
            .protocol_version()
            .expect("Protocol version should be set");

        // Send another request
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client
            .send(ping_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _ping_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Protocol version should be preserved
        assert_eq!(
            client.protocol_version().unwrap(),
            negotiated_version,
            "Protocol version should remain consistent"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_error_response_bodies() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;
        let client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);

        // Test 1: Missing Content-Type header - should return JSON error
        let response = client
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 415);
        let error_body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error_body["jsonrpc"], "2.0");
        assert_eq!(error_body["error"]["code"], -32700);
        assert!(error_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Content-Type"));
        assert_eq!(error_body["id"], serde_json::Value::Null);

        // Test 2: Missing Accept header - should return JSON error
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 406);
        let error_body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(error_body["jsonrpc"], "2.0");
        assert_eq!(error_body["error"]["code"], -32700);
        assert!(error_body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Accept"));

        // Test 3: Missing session ID for non-init request
        // First initialize to establish that we're in stateful mode
        let init_config = StreamableHttpTransportConfig {
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
        let mut init_client = StreamableHttpTransport::new(init_config);

        let init_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        };

        init_client
            .send(init_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _init_response = init_client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Now try a non-init request without session ID - should fail with JSON error
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

        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        // This should fail with an error containing JSON error body
        let result = client.send(ping_message).await;
        assert!(result.is_err());

        // Test 4: Unknown session ID - should return JSON error with 404
        let client_config2 = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: Some("non-existent-session".to_string()),
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client2 = StreamableHttpTransport::new(client_config2);

        let ping_message2 = TransportMessage::Request {
            id: 3i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result2 = client2.send(ping_message2).await;
        assert!(result2.is_err());

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_successful_session_lifecycle() -> Result<()> {
        let (server_addr, server_task) = create_test_server().await?;

        // 1. Initialize - should succeed and return session ID
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
        let _init_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let session_id = client
            .session_id()
            .expect("Session should be set after init");

        // 2. Send regular request - should succeed
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client
            .send(ping_message)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _ping_response = client
            .receive()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Session ID should be preserved
        assert_eq!(client.session_id().unwrap(), session_id);

        // 3. Delete session using raw HTTP since transport doesn't support delete
        let reqwest_client = reqwest::Client::new();
        let url = format!("http://{}", server_addr);
        let delete_response = reqwest_client
            .delete(&url)
            .header("mcp-session-id", &session_id)
            .send()
            .await
            .unwrap();

        assert_eq!(delete_response.status().as_u16(), 200);

        // 4. Try to use deleted session - should fail
        let client_config2 = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr)).map_err(|e| {
                Box::new(pmcp::Error::Internal(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: Some(session_id),
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let mut client2 = StreamableHttpTransport::new(client_config2);

        let ping_message2 = TransportMessage::Request {
            id: 3i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result = client2.send(ping_message2).await;
        assert!(result.is_err(), "Request with deleted session should fail");

        server_task.abort();
        Ok(())
    }
}
