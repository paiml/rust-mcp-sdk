//! D1 / SC-1 / SC-7 — config-declared input-schema enforcement acceptance rows.
//!
//! Owns rows **8, 9, 10 and 11** of the `128-CHANGE-REQUEST.md` acceptance matrix,
//! driving a synthesized single-call HTTP tool against a wiremock backend and
//! proving, through the real `pmcp::server::schema_validation::validate_input` seam:
//!
//! - row 8  — absent `arguments` on a ZERO-parameter tool is ACCEPTED as `{}`
//!            (exactly one upstream request).
//! - row 9  — absent `arguments` on a tool declaring a `required` parameter is
//!            REFUSED by the `required` keyword, never by `type` (zero upstream
//!            requests, and the refusal never contains the string `null`).
//! - row 10 — an UNDECLARED argument key is refused before any backend call, and
//!            the refusal names only DECLARED parameters: not the rejected key, not
//!            the rejected value.
//! - row 11 — the passing control: a compliant call produces exactly ONE upstream
//!            request carrying the server-side auth header. Without it a refusal
//!            suite cannot distinguish "refused correctly" from "handler broken".
//!
//! Rows 8 and 9 are the pair that is easy to conflate; both are kept, with their
//! opposite expectations.
//!
//! Run with: `cargo test -p pmcp-server-toolkit --features http,input-validation \
//! --test input_validation_acceptance -- --test-threads=1`. The test fns are
//! `input_validation_`-prefixed so a positional `input_validation_acceptance`
//! verify filter resolves. Offline (wiremock only).

#![cfg(all(feature = "http", feature = "input-validation"))]

use std::sync::Arc;

use pmcp::RequestHandlerExtra;
use pmcp_server_toolkit::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The static bearer the connector adds SERVER-SIDE; row 11 asserts it reached the
/// upstream request, proving the compliant path is fully wired.
const SERVER_SIDE_TOKEN: &str = "server-side-bearer";

/// Build a one-tool `ServerConfig` for a `GET /lookup` single-call tool carrying
/// `parameters`.
fn cfg_with_params(parameters: Vec<ParamDecl>) -> ServerConfig {
    ServerConfig {
        server: ServerSection {
            name: "umls-demo".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
        tools: vec![ToolDecl {
            name: "lookup".to_string(),
            description: Some("Look a concept up by CUI".to_string()),
            path: Some("/lookup".to_string()),
            method: Some("GET".to_string()),
            parameters,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A required string parameter.
fn required_string(name: &str) -> ParamDecl {
    ParamDecl {
        name: name.to_string(),
        param_type: Some("string".to_string()),
        required: true,
        ..Default::default()
    }
}

/// An optional string parameter.
fn optional_string(name: &str) -> ParamDecl {
    ParamDecl {
        name: name.to_string(),
        param_type: Some("string".to_string()),
        required: false,
        ..Default::default()
    }
}

/// Mount `GET /lookup` on `server` and synthesize the single tool's handler,
/// wired through an `HttpClient` that adds the static bearer server-side.
async fn arrange(
    server: &MockServer,
    parameters: Vec<ParamDecl>,
) -> Arc<dyn pmcp::server::ToolHandler> {
    Mock::given(method("GET"))
        .and(path("/lookup"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(server)
        .await;

    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: SERVER_SIDE_TOKEN.to_string(),
        required: true,
    })
    .expect("bearer auth provider");
    let connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(reqwest::Client::new(), server.uri(), auth).expect("build http client"),
    );

    let cfg = cfg_with_params(parameters);
    let mut out =
        synthesize_from_config_with_http_connector(&cfg, connector).expect("synthesize one tool");
    assert_eq!(out.len(), 1, "exactly one synthesized tool expected");
    out.remove(0).2
}

/// Every request the mock actually observed. Uses `received_requests()` rather
/// than `.expect(0)`, so the assertion reads the recorded set directly instead of
/// relying on mock-expectation teardown.
async fn observed(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .expect("wiremock records requests")
}

/// Row 10 — an undeclared argument key is refused before any backend call, and the
/// refusal names only DECLARED parameters.
#[tokio::test]
async fn input_validation_refuses_undeclared_argument_without_contacting_upstream() {
    let server = MockServer::start().await;
    let handler = arrange(
        &server,
        vec![required_string("cui"), optional_string("version")],
    )
    .await;

    let err = handler
        .handle(
            json!({ "cui": "C0018787", "apiKey": "secret" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("an undeclared argument key must be refused");
    let msg = err.to_string();

    assert!(
        observed(&server).await.is_empty(),
        "a refused call must record ZERO upstream requests"
    );
    assert!(
        !msg.contains("apiKey"),
        "the refusal must not echo the rejected KEY: {msg}"
    );
    assert!(
        !msg.contains("secret"),
        "the refusal must not echo the rejected VALUE: {msg}"
    );
    assert!(
        msg.contains("cui") && msg.contains("version"),
        "the refusal must name both DECLARED parameters: {msg}"
    );
}

/// Row 8 — absent `arguments` on a zero-parameter tool is ACCEPTED as `{}`.
#[tokio::test]
async fn input_validation_accepts_absent_arguments_on_zero_param_tool() {
    let server = MockServer::start().await;
    let handler = arrange(&server, vec![]).await;

    handler
        .handle(Value::Null, RequestHandlerExtra::default())
        .await
        .expect("absent arguments on a zero-parameter tool must validate as `{}`");

    assert_eq!(
        observed(&server).await.len(),
        1,
        "an accepted call must produce exactly ONE upstream request"
    );
}

/// Row 9 — absent `arguments` on a tool declaring a `required` parameter is REFUSED
/// by `required`, never by `type` (which would echo `null`).
#[tokio::test]
async fn input_validation_refuses_absent_arguments_when_required_declared() {
    let server = MockServer::start().await;
    let handler = arrange(&server, vec![required_string("cui")]).await;

    let err = handler
        .handle(Value::Null, RequestHandlerExtra::default())
        .await
        .expect_err("absent arguments must be refused when a parameter is required");
    let msg = err.to_string();

    assert!(
        observed(&server).await.is_empty(),
        "a refused call must record ZERO upstream requests"
    );
    assert!(
        msg.contains("cui"),
        "the refusal must name the `required` keyword's declared property: {msg}"
    );
    assert!(
        !msg.contains("null"),
        "a `type` violation on `null` is the measured-WRONG path and echoes `null`: {msg}"
    );
}

/// Row 11 — the passing control: a compliant call produces exactly ONE upstream
/// request, carrying the auth header the server added.
#[tokio::test]
async fn input_validation_accepts_compliant_call_with_one_upstream_request() {
    let server = MockServer::start().await;
    let handler = arrange(
        &server,
        vec![required_string("cui"), optional_string("version")],
    )
    .await;

    handler
        .handle(
            json!({ "cui": "C0018787", "version": "2026AA" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("a fully compliant call must succeed");

    let requests = observed(&server).await;
    assert_eq!(
        requests.len(),
        1,
        "a compliant call must produce exactly ONE upstream request"
    );
    let auth = requests[0]
        .headers
        .get("authorization")
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    assert_eq!(
        auth,
        format!("Bearer {SERVER_SIDE_TOKEN}"),
        "the upstream request must carry the SERVER-SIDE auth header"
    );
}
