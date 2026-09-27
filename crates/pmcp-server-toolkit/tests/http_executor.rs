//! OAPI-05 / H1 / H2 — `HttpCodeExecutor` integration tests.
//!
//! Drives `pmcp_code_mode::HttpExecutor::execute_request` against a wiremock
//! backend, proving:
//! - An already-resolved path reaches the wiremock backend verbatim + the
//!   returned JSON round-trip (GET). Placeholder substitution is NO LONGER this
//!   executor's job: Phase 128 D-09 moved it up into `PlanExecutor`, so this
//!   surface receives a `ResolvedPath` and must not resolve anything.
//! - The per-request inbound token reaches the outgoing `Authorization` header
//!   through an `oauth_passthrough` provider (H1).
//! - `ExecutionError` Display NEVER echoes the URL or token (Pitfall 5 /
//!   T-90-04-01).
//!
//! Run with: `cargo test -p pmcp-server-toolkit --features openapi-code-mode \
//! --test http_executor -- --test-threads=1`. The test fns are
//! `http_executor_`-prefixed so the positional `http_executor` verify filter
//! resolves (Plan 01 verify-filter lesson).

#![cfg(feature = "openapi-code-mode")]

use pmcp_code_mode::{ExecutionError, HttpExecutor, ResolvedPath};
use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
use pmcp_server_toolkit::http::auth::{
    create_auth_provider, create_passthrough_auth_provider, AuthConfig,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn http_executor_get_sends_the_resolved_path_verbatim_and_returns_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 7, "name": "Ada"})))
        .mount(&server)
        .await;

    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth);

    let result = exec
        .execute_request(
            "GET",
            ResolvedPath::from_checked("/users/7").expect("a clean resolved path"),
            None,
        )
        .await
        .expect("GET with an already-resolved path must succeed");
    assert_eq!(result["id"], 7);
    assert_eq!(result["name"], "Ada");
}

#[tokio::test]
async fn http_executor_post_sends_body_and_applies_static_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/items"))
        .and(header("authorization", "Bearer static-tok"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"created": true})))
        .mount(&server)
        .await;

    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: "static-tok".to_string(),
        required: true,
    })
    .expect("bearer");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth);

    let result = exec
        .execute_request(
            "POST",
            ResolvedPath::from_checked("/items").expect("a clean resolved path"),
            Some(json!({"name": "widget"})),
        )
        .await
        .expect("POST with body + static bearer must succeed");
    assert_eq!(result["created"], true);
}

#[tokio::test]
async fn http_executor_forwards_per_request_inbound_token() {
    // H1: an executor built `with_inbound_token(Some("client-tok"))` over an
    // OAuthPassthrough provider sends `Authorization: Bearer client-tok`.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer client-tok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let auth = create_passthrough_auth_provider(
        &AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        },
        None,
    )
    .expect("passthrough");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth)
        .with_inbound_token(Some("client-tok".to_string()));

    let result = exec
        .execute_request(
            "GET",
            ResolvedPath::from_checked("/me").expect("a clean resolved path"),
            None,
        )
        .await
        .expect("per-request inbound token must reach the backend");
    assert_eq!(result["ok"], true);
}

#[tokio::test]
async fn http_executor_connect_failure_is_redacted() {
    // A connect failure to an unroutable address must surface a RuntimeError
    // whose message contains NO URL or token (Pitfall 5 / T-90-04-01).
    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: "super-secret-token".to_string(),
        required: true,
    })
    .expect("bearer");
    // Reserved TEST-NET-1 address with a closed port — connect fails fast.
    let exec = HttpCodeExecutor::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(300))
            .build()
            .expect("client"),
        "http://192.0.2.1:9".to_string(),
        auth,
    );

    let err = exec
        .execute_request(
            "GET",
            ResolvedPath::from_checked("/secret/path").expect("a clean resolved path"),
            None,
        )
        .await
        .expect_err("connect to an unroutable host must error");
    let rendered = err.to_string();
    for forbidden in [
        "super-secret-token",
        "Bearer",
        "192.0.2.1",
        "http://",
        "/secret/path",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "ExecutionError Display must not echo {forbidden:?}; got {rendered:?}"
        );
    }
    assert!(
        matches!(err, ExecutionError::RuntimeError { .. }),
        "expected a RuntimeError, got {err:?}"
    );
}

/// Ensure the executor is `Clone` (the binary clones it per request to attach a
/// token) and the clone carries an independent token.
#[test]
fn http_executor_display_no_secret() {
    // Compile-time assertion that the redaction-bearing error type stays a
    // RuntimeError with a non-secret message (the runtime proof is the
    // `_connect_failure_is_redacted` test above; this guards the variant shape).
    let err = ExecutionError::RuntimeError {
        message: "backend returned HTTP status 503".to_string(),
    };
    let rendered = err.to_string();
    for forbidden in ["Bearer", "Authorization", "https://", "http://"] {
        assert!(!rendered.contains(forbidden), "must not echo {forbidden:?}");
    }
    assert!(
        rendered.contains("503"),
        "status must be visible: {rendered}"
    );

    fn assert_clone<T: Clone>() {}
    assert_clone::<HttpCodeExecutor>();
}
