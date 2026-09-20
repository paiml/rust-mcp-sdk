//! Integration tests for streamable HTTP transport.

// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#![cfg(all(feature = "streamable-http", feature = "v1-compat"))]

use pmcp::server::streamable_http_server::{
    InMemoryEventStore, StreamableHttpServer, StreamableHttpServerConfig,
};
use pmcp::server::Server;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::{
    ClientNotification, ClientRequest, Implementation, InitializeRequest, Notification, Request,
    RequestId,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

/// Create a test server with minimal capabilities
async fn create_test_server() -> Arc<Mutex<Server>> {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::minimal())
        .build()
        .unwrap();

    Arc::new(Mutex::new(server))
}

#[tokio::test]
async fn test_streamable_http_stateless_mode() {
    // Start server in stateless mode
    let server = create_test_server().await;
    let config = StreamableHttpServerConfig {
        session_id_generator: None, // Stateless mode
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

    // Create client transport
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(client_config);

    // Send initialization request
    let init_request = TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
            Implementation::new("test-client", "1.0.0"),
            Default::default(),
        )))),
    };

    transport.send(init_request).await.unwrap();

    // Receive response
    let response = transport.receive().await.unwrap();
    match response {
        TransportMessage::Response(json_response) => {
            assert_eq!(json_response.id, RequestId::from(1i64));
        },
        _ => panic!("Expected response"),
    }

    // Cleanup
    transport.close().await.unwrap();
    handle.abort();
}

#[tokio::test]
async fn test_streamable_http_stateful_mode() {
    // Start server in stateful mode
    let server = create_test_server().await;
    let session_initialized = Arc::new(Mutex::new(false));
    let session_closed = Arc::new(Mutex::new(false));

    let init_clone = session_initialized.clone();
    let closed_clone = session_closed.clone();

    let config = StreamableHttpServerConfig {
        session_id_generator: Some(Box::new(|| {
            format!("test-session-{}", uuid::Uuid::new_v4())
        })),
        enable_json_response: false,
        event_store: Some(Arc::new(InMemoryEventStore::default())),
        on_session_initialized: Some(Box::new(move |_session_id| {
            let init = init_clone.clone();
            tokio::spawn(async move {
                *init.lock().await = true;
            });
        })),
        on_session_closed: Some(Box::new(move |_session_id| {
            let closed = closed_clone.clone();
            tokio::spawn(async move {
                *closed.lock().await = true;
            });
        })),
        http_middleware: None,
        allowed_origins: None,
        max_request_bytes: pmcp::server::limits::DEFAULT_MAX_REQUEST_BYTES,
    };

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create client transport
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(client_config);

    // Send initialization request
    let init_request = TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
            Implementation::new("test-client", "1.0.0"),
            Default::default(),
        )))),
    };

    transport.send(init_request).await.unwrap();

    // Receive response
    let response = transport.receive().await.unwrap();
    match response {
        TransportMessage::Response(json_response) => {
            assert_eq!(json_response.id, RequestId::from(1i64));
            // Check that we got a session ID
            assert!(transport.session_id().is_some());
        },
        _ => panic!("Expected response"),
    }

    // Wait for session initialization callback
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(*session_initialized.lock().await);

    // Close transport
    transport.close().await.unwrap();

    // Cleanup
    handle.abort();
}

#[tokio::test]
async fn test_sse_parser_integration() {
    use pmcp::shared::sse_parser::SseParser;

    let mut parser = SseParser::new();

    // Test basic event
    let events = parser.feed("data: hello\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello");

    // Test event with ID
    let events = parser.feed("id: 123\ndata: world\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, Some("123".to_string()));
    assert_eq!(events[0].data, "world");

    // Test multi-line data
    let events = parser.feed("data: line1\ndata: line2\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line1\nline2");

    // Test incremental parsing
    let mut parser = SseParser::new();
    assert_eq!(parser.feed("data: par").len(), 0);
    assert_eq!(parser.feed("tial\n").len(), 0);
    let events = parser.feed("\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "partial");
}

#[tokio::test]
async fn test_transport_send_receive_multiple() {
    let server = create_test_server().await;
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

    // Create client transport
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(client_config);

    // Send multiple requests
    for i in 1..=3 {
        let request = TransportMessage::Request {
            id: RequestId::from(i as i64),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("test-client", "1.0.0"),
                Default::default(),
            )))),
        };

        transport.send(request).await.unwrap();

        // Receive response
        let response = transport.receive().await.unwrap();
        match response {
            TransportMessage::Response(json_response) => {
                assert_eq!(json_response.id, RequestId::from(i as i64));
            },
            _ => panic!("Expected response"),
        }
    }

    // Cleanup
    transport.close().await.unwrap();
    handle.abort();
}

#[tokio::test]
async fn test_event_store_persistence() {
    use pmcp::server::streamable_http_server::EventStore;

    let store = InMemoryEventStore::default();

    // Store some events
    let msg1 =
        TransportMessage::Notification(Notification::Client(ClientNotification::Initialized));
    let msg2 =
        TransportMessage::Notification(Notification::Client(ClientNotification::RootsListChanged));

    store.store_event("stream1", "event1", &msg1).await.unwrap();
    store.store_event("stream1", "event2", &msg2).await.unwrap();

    // Replay events
    let replayed = store.replay_events_after("event1").await.unwrap();
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].0, "event2");

    // Get stream for event
    let stream = store.get_stream_for_event("event1").await.unwrap();
    assert_eq!(stream, Some("stream1".to_string()));
}

#[tokio::test]
async fn test_transport_with_headers() {
    let server = create_test_server().await;
    let config = StreamableHttpServerConfig::default();

    let server_instance =
        StreamableHttpServer::with_config("127.0.0.1:0".parse().unwrap(), server.clone(), config);

    let (addr, handle) = server_instance.start().await.unwrap();

    // Create client transport with extra headers
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", addr)).unwrap(),
        extra_headers: vec![
            ("X-Custom-Header".to_string(), "custom-value".to_string()),
            ("X-API-Key".to_string(), "test-key".to_string()),
        ],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(client_config);

    // Send request
    let request = TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
            Implementation::new("test-client", "1.0.0"),
            Default::default(),
        )))),
    };

    transport.send(request).await.unwrap();

    // Should receive response successfully
    let response = transport.receive().await.unwrap();
    assert!(matches!(response, TransportMessage::Response(_)));

    // Cleanup
    transport.close().await.unwrap();
    handle.abort();
}
