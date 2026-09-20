//! Unit tests for streamable HTTP modules to improve coverage.

// Phase 117 gated the v1 session-lifecycle and SSE-resumability public API
// behind `v1-compat`, so this file — whose subject IS that API — does not
// compile on `--no-default-features --features full-v2`. Gating it here is what
// makes the aggregate `cargo test -p pmcp --no-default-features --features
// full-v2` a green run rather than a hard build failure. On the severed build it
// contributes zero tests, which is correct: there is no v1 to test. The severed
// build's own coverage is the `tests/v2_*_on_severed_build.rs` family plus the
// 1867 lib tests that now run there.
#![cfg(all(feature = "streamable-http", feature = "v1-compat"))]

use pmcp::shared::streamable_http::{
    AuthProvider, SendOptions, StreamableHttpTransport, StreamableHttpTransportConfig,
};
use pmcp::shared::Transport;
use pmcp::types::{ClientRequest, Request, RequestId};
use std::sync::Arc;
use url::Url;

// Mock auth provider for testing
#[derive(Debug)]
struct MockAuthProvider {
    token: String,
}

#[async_trait::async_trait]
impl AuthProvider for MockAuthProvider {
    async fn get_access_token(&self) -> pmcp::error::Result<String> {
        Ok(self.token.clone())
    }
}

#[test]
fn test_send_options_default() {
    let opts = SendOptions::default();
    assert!(opts.related_request_id.is_none());
    assert!(opts.resumption_token.is_none());
}

#[test]
fn test_send_options_with_values() {
    let opts = SendOptions {
        related_request_id: Some("req-123".to_string()),
        resumption_token: Some("token-456".to_string()),
    };
    assert_eq!(opts.related_request_id, Some("req-123".to_string()));
    assert_eq!(opts.resumption_token, Some("token-456".to_string()));
}

#[test]
fn test_streamable_http_config_creation() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![("X-Custom".to_string(), "value".to_string())],
        auth_provider: Some(Arc::new(MockAuthProvider {
            token: "test-token".to_string(),
        })),
        session_id: Some("session-123".to_string()),
        enable_json_response: true,
        on_resumption_token: Some(Arc::new(|token| {
            println!("Resumption token: {}", token);
        })),
        http_middleware_chain: None,
    };

    assert_eq!(config.url.as_str(), "http://localhost:8080/");
    assert_eq!(config.extra_headers.len(), 1);
    assert!(config.auth_provider.is_some());
    assert_eq!(config.session_id, Some("session-123".to_string()));
    assert!(config.enable_json_response);
    assert!(config.on_resumption_token.is_some());
}

#[test]
fn test_streamable_http_transport_creation() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let transport = StreamableHttpTransport::new(config);
    assert!(transport.session_id().is_none());
    assert!(transport.protocol_version().is_none());
    assert!(transport.last_event_id().is_none());
    assert!(transport.is_connected());
}

#[test]
fn test_streamable_http_transport_session_management() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: Some("initial-session".to_string()),
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let transport = StreamableHttpTransport::new(config);
    assert_eq!(transport.session_id(), Some("initial-session".to_string()));

    // Update session ID
    transport.set_session_id(Some("new-session".to_string()));
    assert_eq!(transport.session_id(), Some("new-session".to_string()));

    // Clear session ID
    transport.set_session_id(None);
    assert!(transport.session_id().is_none());
}

#[test]
fn test_streamable_http_transport_protocol_version() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let transport = StreamableHttpTransport::new(config);
    assert!(transport.protocol_version().is_none());

    // Set protocol version
    transport.set_protocol_version(Some("2025-06-18".to_string()));
    assert_eq!(transport.protocol_version(), Some("2025-06-18".to_string()));
}

#[test]
fn test_streamable_http_config_debug() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("StreamableHttpTransportConfig"));
    assert!(debug_str.contains("url"));
    assert!(debug_str.contains("extra_headers"));
}

#[test]
fn test_streamable_http_transport_debug() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let transport = StreamableHttpTransport::new(config);
    let debug_str = format!("{:?}", transport);
    assert!(debug_str.contains("StreamableHttpTransport"));
    assert!(debug_str.contains("config"));
}

#[test]
fn test_streamable_http_config_clone() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![("X-Test".to_string(), "value".to_string())],
        auth_provider: None,
        session_id: Some("session-123".to_string()),
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let cloned = config.clone();
    assert_eq!(config.url, cloned.url);
    assert_eq!(config.extra_headers, cloned.extra_headers);
    assert_eq!(config.session_id, cloned.session_id);
    assert_eq!(config.enable_json_response, cloned.enable_json_response);
}

#[test]
fn test_streamable_http_transport_clone() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: Some("session-123".to_string()),
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let transport = StreamableHttpTransport::new(config);
    let cloned = transport.clone();
    assert_eq!(transport.session_id(), cloned.session_id());
}

#[tokio::test]
async fn test_streamable_http_transport_close() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(config);

    // Close should succeed even without a real connection
    let result = transport.close().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_streamable_http_send_with_auth_provider() {
    let config = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080").unwrap(),
        extra_headers: vec![],
        auth_provider: Some(Arc::new(MockAuthProvider {
            token: "bearer-token-123".to_string(),
        })),
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
        http_middleware_chain: None,
    };

    let mut transport = StreamableHttpTransport::new(config);

    let message = pmcp::shared::TransportMessage::Request {
        id: RequestId::from(1i64),
        request: Request::Client(Box::new(ClientRequest::Ping)),
    };

    // This will fail because we're not connected to a real server,
    // but it will exercise the auth provider code path
    let _ = transport.send(message).await;
}

#[test]
fn test_send_options_clone() {
    let opts = SendOptions {
        related_request_id: Some("req-123".to_string()),
        resumption_token: Some("token-456".to_string()),
    };

    let cloned = opts.clone();
    assert_eq!(opts.related_request_id, cloned.related_request_id);
    assert_eq!(opts.resumption_token, cloned.resumption_token);
}

#[test]
fn test_send_options_debug() {
    let opts = SendOptions {
        related_request_id: Some("req-123".to_string()),
        resumption_token: Some("token-456".to_string()),
    };

    let debug_str = format!("{:?}", opts);
    assert!(debug_str.contains("SendOptions"));
    assert!(debug_str.contains("related_request_id"));
    assert!(debug_str.contains("resumption_token"));
}
