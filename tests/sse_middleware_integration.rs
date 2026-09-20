//! Integration tests for SSE with HTTP middleware.
//!
//! Tests that HTTP middleware correctly processes:
//! - Initial SSE GET requests
//! - SSE reconnections with Last-Event-ID
//! - Middleware state across reconnections

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
use pmcp::client::http_middleware::{
    HttpMiddleware, HttpMiddlewareChain, HttpMiddlewareContext, HttpRequest, HttpResponse,
};
use pmcp::server::streamable_http_server::{
    InMemoryEventStore, StreamableHttpServer, StreamableHttpServerConfig,
};
use pmcp::server::Server;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::ClientBuilder;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

/// Middleware that tracks how many times it's called and what headers it sees
#[derive(Debug)]
struct RequestTrackingMiddleware {
    request_count: Arc<AtomicUsize>,
    last_event_ids: Arc<Mutex<Vec<Option<String>>>>,
}

impl RequestTrackingMiddleware {
    fn new() -> Self {
        Self {
            request_count: Arc::new(AtomicUsize::new(0)),
            last_event_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }

    async fn get_last_event_ids(&self) -> Vec<Option<String>> {
        self.last_event_ids.lock().await.clone()
    }
}

#[async_trait]
impl HttpMiddleware for RequestTrackingMiddleware {
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        _context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        // Increment counter
        self.request_count.fetch_add(1, Ordering::SeqCst);

        // Track Last-Event-ID header if present
        let last_event_id = request.get_header("Last-Event-ID").map(|s| s.to_string());
        self.last_event_ids.lock().await.push(last_event_id);

        tracing::debug!(
            "RequestTrackingMiddleware: request #{}, Last-Event-ID: {:?}",
            self.request_count.load(Ordering::SeqCst),
            request.get_header("Last-Event-ID")
        );

        Ok(())
    }

    fn priority(&self) -> i32 {
        50 // Normal priority
    }
}

#[tokio::test]
async fn test_middleware_runs_on_sse_get() {
    // Create server in SSE mode (stateful with event store)
    let server = Server::builder()
        .name("sse-middleware-test")
        .version("1.0.0")
        .capabilities(ServerCapabilities::minimal())
        .build()
        .unwrap();

    let event_store = InMemoryEventStore::default();
    let config = StreamableHttpServerConfig {
        session_id_generator: Some(Box::new(|| uuid::Uuid::new_v4().to_string())),
        enable_json_response: false, // Force SSE mode
        event_store: Some(Arc::new(event_store)),
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance = StreamableHttpServer::with_config(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Mutex::new(server)),
        config,
    );

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create HTTP middleware chain with tracking middleware
    let tracking_middleware = Arc::new(RequestTrackingMiddleware::new());
    let mut http_chain = HttpMiddlewareChain::new();
    http_chain.add(tracking_middleware.clone());

    // Create client with SSE transport and middleware
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false, // Use SSE
        on_resumption_token: None,
        http_middleware_chain: Some(Arc::new(http_chain)),
    };

    let transport = StreamableHttpTransport::new(client_config);
    let mut client = ClientBuilder::new(transport).build();

    // Initialize - this should trigger SSE GET request
    let _init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    // Give SSE a moment to establish
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify middleware was called for the SSE GET request
    let request_count = tracking_middleware.get_request_count();
    assert!(
        request_count >= 1,
        "Middleware should have been called at least once for SSE GET, got {}",
        request_count
    );

    // Verify initial request had no Last-Event-ID
    let last_event_ids = tracking_middleware.get_last_event_ids().await;
    assert!(
        !last_event_ids.is_empty(),
        "Should have at least one request tracked"
    );
    assert_eq!(
        last_event_ids[0], None,
        "Initial SSE request should not have Last-Event-ID"
    );

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_middleware_with_multiple_http_methods() {
    // Middleware that tracks HTTP methods
    #[derive(Debug)]
    struct MethodTrackingMiddleware {
        methods: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl HttpMiddleware for MethodTrackingMiddleware {
        async fn on_request(
            &self,
            request: &mut HttpRequest,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            self.methods.lock().await.push(request.method.clone());
            Ok(())
        }

        fn priority(&self) -> i32 {
            50
        }
    }

    // Create server
    let server = Server::builder()
        .name("multi-method-test")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .build()
        .unwrap();

    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true, // JSON mode for simpler testing
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance = StreamableHttpServer::with_config(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Mutex::new(server)),
        config,
    );

    let (addr, handle) = server_instance.start().await.unwrap();

    let method_tracker = Arc::new(MethodTrackingMiddleware {
        methods: Arc::new(Mutex::new(Vec::new())),
    });

    let mut http_chain = HttpMiddlewareChain::new();
    http_chain.add(method_tracker.clone());

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

    // Initialize (POST)
    let _init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    // List tools (POST)
    let _tools = client.list_tools(None).await.unwrap();

    // Verify the middleware saw every request the client made.
    //
    // This assertion used to read "all requests should be POST in JSON mode",
    // which was only true because of Phase 118.2's Defect A: `start_sse` sat
    // inside `if !response.status().is_success()`, and `202 Accepted` IS a
    // success status, so the client never opened its session stream and no GET
    // was ever issued. It does now, exactly as the reference SDK does
    // (`isInitializedNotification` -> `_startOrAuthSse`), and
    // `enable_json_response` is a hint about POST response FORMAT, not a reason
    // to skip the server-to-client channel. The old expectation encoded the
    // defect; the new one records the corrected behaviour.
    let methods = method_tracker.methods.lock().await;
    let posts = methods.iter().filter(|m| *m == "POST").count();
    let gets = methods.iter().filter(|m| *m == "GET").count();
    assert!(
        posts >= 2,
        "Should have at least 2 POST requests: {methods:?}"
    );
    assert_eq!(
        gets, 1,
        "exactly ONE GET — the session stream opened after notifications/initialized, and not \
         re-opened by any other notification: {methods:?}"
    );
    assert_eq!(
        posts + gets,
        methods.len(),
        "only POST and GET are expected on this transport: {methods:?}"
    );

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_middleware_modifies_request_headers() {
    // Middleware that adds a custom header
    #[derive(Debug)]
    struct HeaderInjectionMiddleware {
        header_value: String,
    }

    #[async_trait]
    impl HttpMiddleware for HeaderInjectionMiddleware {
        async fn on_request(
            &self,
            request: &mut HttpRequest,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            request.add_header("X-Custom-Test", &self.header_value);
            Ok(())
        }

        fn priority(&self) -> i32 {
            10
        }
    }

    // Create server
    let server = Server::builder()
        .name("header-test")
        .version("1.0.0")
        .capabilities(ServerCapabilities::minimal())
        .build()
        .unwrap();

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

    let server_instance = StreamableHttpServer::with_config(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Mutex::new(server)),
        config,
    );

    let (addr, handle) = server_instance.start().await.unwrap();

    let mut http_chain = HttpMiddlewareChain::new();
    http_chain.add(Arc::new(HeaderInjectionMiddleware {
        header_value: "test-value-123".to_string(),
    }));

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

    // Make request - middleware should inject header
    let _init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    // If we got here without error, the server received the request with custom header
    // (In a real test, we'd verify the server saw the header, but that requires server-side tracking)

    // Cleanup
    drop(client);
    handle.abort();
}

#[tokio::test]
async fn test_middleware_response_processing() {
    // Middleware that tracks response status codes
    #[derive(Debug)]
    struct ResponseTrackingMiddleware {
        status_codes: Arc<Mutex<Vec<u16>>>,
    }

    #[async_trait]
    impl HttpMiddleware for ResponseTrackingMiddleware {
        async fn on_response(
            &self,
            response: &mut HttpResponse,
            _context: &HttpMiddlewareContext,
        ) -> pmcp::Result<()> {
            self.status_codes.lock().await.push(response.status);
            tracing::debug!("Response status: {}", response.status);
            Ok(())
        }

        fn priority(&self) -> i32 {
            50
        }
    }

    let response_tracker = Arc::new(ResponseTrackingMiddleware {
        status_codes: Arc::new(Mutex::new(Vec::new())),
    });

    // Create server with tools capability
    let server = Server::builder()
        .name("response-test")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .build()
        .unwrap();

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

    let server_instance = StreamableHttpServer::with_config(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Mutex::new(server)),
        config,
    );

    let (addr, handle) = server_instance.start().await.unwrap();

    let mut http_chain = HttpMiddlewareChain::new();
    http_chain.add(response_tracker.clone());

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

    // Make requests
    let _init_result = client
        .initialize(pmcp::ClientCapabilities::minimal())
        .await
        .unwrap();

    let _tools = client.list_tools(None).await.unwrap();

    // Verify middleware tracked response status codes
    let status_codes = response_tracker.status_codes.lock().await;
    assert!(
        status_codes.len() >= 2,
        "Should have tracked at least 2 responses"
    );
    for status in status_codes.iter() {
        assert!(
            *status == 200 || *status == 202,
            "Successful requests should have 200 or 202 (Accepted) status, got {}",
            status
        );
    }

    // Cleanup
    drop(client);
    handle.abort();
}
