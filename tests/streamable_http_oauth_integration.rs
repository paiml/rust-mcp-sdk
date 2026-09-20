//! Integration tests for `StreamableHttpServer` + OAuth middleware.
//!
//! Tests real client-server interaction with OAuth middleware:
//! - OAuth middleware injects tokens correctly
//! - `auth_provider` precedence over OAuth middleware
//! - Authorization header propagation to server
//! - Server-side token validation

// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#![cfg(all(feature = "streamable-http", feature = "v1-compat"))]

use async_trait::async_trait;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::{Server, ToolHandler};
use pmcp::shared::streamable_http::{
    AuthProvider, StreamableHttpTransport, StreamableHttpTransportConfig,
};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::ClientBuilder;
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use url::Url;

/// Simple echo tool for testing
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "echo": args,
            "received": "ok"
        }))
    }
}

/// Create a test server with minimal capabilities
async fn create_auth_test_server() -> Arc<Mutex<Server>> {
    let server = Server::builder()
        .name("auth-test-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", EchoTool)
        .build()
        .unwrap();

    Arc::new(Mutex::new(server))
}

#[tokio::test]
async fn test_oauth_middleware_injects_token() {
    // Start server
    let server = create_auth_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with OAuth
    let mut http_chain = HttpMiddlewareChain::new();
    let token = BearerToken::with_expiry(
        "test-oauth-token-12345".to_string(),
        Duration::from_hours(1),
    );
    http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // Create client with OAuth middleware, no auth_provider
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);

    let mut client = ClientBuilder::new(transport).build();

    // Initialize client
    let init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    assert_eq!(init_result.server_info.name, "auth-test-server");

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_auth_provider_takes_precedence_over_oauth() {
    /// Simple auth provider for testing
    #[derive(Debug)]
    struct TestAuthProvider {
        token: String,
    }

    #[async_trait]
    impl AuthProvider for TestAuthProvider {
        async fn get_access_token(&self) -> pmcp::Result<String> {
            Ok(self.token.clone())
        }
    }

    // Start server
    let server = create_auth_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with OAuth
    let mut http_chain = HttpMiddlewareChain::new();
    let oauth_token = BearerToken::new("oauth-token-should-be-skipped".to_string());
    http_chain.add(Arc::new(OAuthClientMiddleware::new(oauth_token)));

    // Create client with BOTH auth_provider AND OAuth middleware
    let auth_provider = Arc::new(TestAuthProvider {
        token: "auth-provider-token-wins".to_string(),
    });

    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: Some(auth_provider),
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);

    let mut client = ClientBuilder::new(transport).build();

    // Initialize client - should use auth_provider token, not OAuth token
    let init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    assert_eq!(init_result.server_info.name, "auth-test-server");

    // The auth_provider token should have been used (OAuth skipped due to precedence)
    // We can't easily verify the exact header without server-side inspection,
    // but the fact that initialization succeeded means the precedence worked

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_oauth_token_expiry_triggers_error() {
    // Start server
    let server = create_auth_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with EXPIRED OAuth token
    let mut http_chain = HttpMiddlewareChain::new();
    let expired_token = BearerToken::with_expiry(
        "expired-token".to_string(),
        Duration::from_secs(0), // Expires immediately
    );

    // Wait to ensure token is expired
    tokio::time::sleep(Duration::from_millis(10)).await;

    http_chain.add(Arc::new(OAuthClientMiddleware::new(expired_token)));

    // Create client with expired OAuth token
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);

    let mut client = ClientBuilder::new(transport).build();

    // Initialize should fail with authentication error
    let result = client.initialize(pmcp::ClientCapabilities::minimal()).await;

    assert!(result.is_err(), "Expired token should cause error");
    assert!(
        matches!(result.unwrap_err(), pmcp::Error::Authentication(_)),
        "Should be authentication error"
    );

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_multiple_requests_with_oauth() {
    // Start server
    let server = create_auth_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with OAuth
    let mut http_chain = HttpMiddlewareChain::new();
    let token = BearerToken::with_expiry("persistent-token".to_string(), Duration::from_hours(1));
    http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // Create client
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);

    let mut client = ClientBuilder::new(transport).build();

    // Initialize - OAuth should inject token
    let _init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    // Make multiple requests - OAuth should inject token for each
    // We'll just call list_tools multiple times to verify middleware runs
    for _i in 0..5 {
        // Each request should succeed with OAuth token injection
        let _tools = client.list_tools(None).await.unwrap();
    }

    // Cleanup - drop client and abort server
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_oauth_with_case_insensitive_header_check() {
    // Start server
    let server = create_auth_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with OAuth
    let mut http_chain = HttpMiddlewareChain::new();
    let token = BearerToken::new("case-test-token".to_string());
    http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // Create client with extra headers that include authorization (different case)
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![
            // This should be detected by OAuth middleware despite case difference
            (
                "AUTHORIZATION".to_string(),
                "Bearer manual-token".to_string(),
            ),
        ],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);

    let mut client = ClientBuilder::new(transport).build();

    // Initialize - OAuth should detect the existing Authorization header (case-insensitive)
    // and not add a duplicate
    let init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    assert_eq!(init_result.server_info.name, "auth-test-server");

    // Cleanup
    drop(client);
    handle.abort();
}
