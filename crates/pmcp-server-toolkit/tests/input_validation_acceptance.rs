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

/// Build a one-tool `ServerConfig` for a `POST /comments` single-call tool
/// carrying `parameters` (Phase 128 CR-02).
fn cfg_post_with_params(parameters: Vec<ParamDecl>) -> ServerConfig {
    ServerConfig {
        server: ServerSection {
            name: "umls-demo".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
        tools: vec![ToolDecl {
            name: "add_comment".to_string(),
            description: Some("Post a comment".to_string()),
            path: Some("/comments".to_string()),
            method: Some("POST".to_string()),
            parameters,
            ..Default::default()
        }],
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

/// Mount `POST /comments` on `server` and synthesize the single tool's handler,
/// wired through the SAME `synthesize_from_config_with_http_connector` seam
/// `arrange` uses — so the returned handler is the `ValidatingToolHandler`
/// decorator, not the bare connector.
///
/// That distinction is the whole point of this arrangement (Phase 128 CR-02): the
/// tree's only pre-existing `POST` coverage built an `Operation` by hand and called
/// `HttpConnector::execute` directly, i.e. BELOW the decorator, so it could not see
/// that `additionalProperties: false` had closed the only route a synthesized
/// mutating tool had to a request body.
async fn arrange_post(
    server: &MockServer,
    parameters: Vec<ParamDecl>,
) -> Arc<dyn pmcp::server::ToolHandler> {
    Mock::given(method("POST"))
        .and(path("/comments"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "ok": true })))
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

    let cfg = cfg_post_with_params(parameters);
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

/// A refused argument reaches the client as a TOOL-LEVEL rejection, not a JSON-RPC
/// protocol error.
///
/// The caller can fix a refused argument, and `Error::ToolRejected` is the SDK's
/// variant for that: the server maps it to a result with `isError: true` carrying
/// the message, where `Error::Validation` becomes a protocol error that reads to a
/// model as a server fault. Measured on 2.21.0, 14 of 15 SDK refusals in a probe
/// matrix took the protocol channel and a downstream server wrote an adapter to
/// remap them. This fails if the refusal goes back to `Error::Validation`.
#[tokio::test]
async fn a_schema_refusal_is_a_tool_level_rejection_not_a_protocol_error() {
    let server = MockServer::start().await;
    let handler = arrange(&server, vec![required_string("cui")]).await;

    let err = handler
        .handle(
            json!({ "cui": "C0018787", "undeclared": "x" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("an undeclared argument key must be refused");

    assert!(
        matches!(err, pmcp::Error::ToolRejected { .. }),
        "a schema refusal must be a tool-level rejection the model can act on, got: {err:?}"
    );
    assert!(
        !err.to_string().starts_with("Validation error"),
        "the message must not carry the protocol-error prefix: {err}"
    );
    assert!(
        observed(&server).await.is_empty(),
        "a refused call must record ZERO upstream requests"
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

/// Phase 128 CR-02 — a curated `POST` tool's DECLARED parameters reach the JSON
/// request body, driven THROUGH the `ValidatingToolHandler` decorator.
///
/// This is the row that was missing. With `additionalProperties: false` enforced,
/// an undeclared argument is refused, so a declared one is the ONLY route to a
/// payload; before the fix `build_operation` marked it `ParameterLocation::Query`
/// and `build_body` — which collected only args absent from
/// `operation.parameters` — produced nothing. The tool accepted the call, answered
/// `2xx`, and the backend received an empty body with the values in the URL.
///
/// Fails on removal: revert `build_operation`'s `ParameterLocation::Body`
/// assignment and the body assertion sees `{}` while the query assertion sees
/// `body_text` and `title`.
#[tokio::test]
async fn input_validation_post_tool_sends_declared_parameters_in_the_json_body() {
    let server = MockServer::start().await;
    let handler = arrange_post(
        &server,
        vec![required_string("body_text"), optional_string("title")],
    )
    .await;

    handler
        .handle(
            json!({ "body_text": "the comment text", "title": "a title" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("a compliant POST call must succeed");

    let requests = observed(&server).await;
    assert_eq!(
        requests.len(),
        1,
        "a compliant call must produce exactly ONE upstream request"
    );
    let body: Value =
        serde_json::from_slice(&requests[0].body).expect("the upstream body is JSON, not empty");
    assert_eq!(
        body,
        json!({ "body_text": "the comment text", "title": "a title" }),
        "every declared non-path parameter must reach the JSON payload"
    );
    assert_eq!(
        requests[0].url.query(),
        None,
        "a payload field must NOT also travel in the request line: {}",
        requests[0].url
    );
}

/// Phase 128 WR-10 — the reserved `body` argument becomes the whole payload and is
/// no longer ALSO appended to the query string.
///
/// Fails on removal: revert `build_operation`'s `Body` assignment and the declared
/// `body` parameter is `Query`-located again, so `build_query` appends `body=…` to
/// the URL while `build_body`'s short-circuit sends the same value as the payload —
/// the same value in two encodings, one of them in the access log's request line.
#[tokio::test]
async fn input_validation_post_tool_reserved_body_key_travels_once() {
    let server = MockServer::start().await;
    let handler = arrange_post(&server, vec![required_string("body")]).await;

    handler
        .handle(
            json!({ "body": "raw-payload-text" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("a compliant POST call must succeed");

    let requests = observed(&server).await;
    assert_eq!(requests.len(), 1, "exactly ONE upstream request");
    let body: Value = serde_json::from_slice(&requests[0].body).expect("the upstream body is JSON");
    assert_eq!(
        body,
        json!("raw-payload-text"),
        "`body` is the reserved whole-payload key"
    );
    assert_eq!(
        requests[0].url.query(),
        None,
        "the reserved `body` value must travel exactly ONCE: {}",
        requests[0].url
    );
}

/// The refusal control for the POST surface: an UNDECLARED argument is still
/// refused before any backend call. Without this row the two rows above are
/// satisfiable by re-opening `additionalProperties`, which would undo D1 rather
/// than fix CR-02.
#[tokio::test]
async fn input_validation_post_tool_still_refuses_an_undeclared_argument() {
    let server = MockServer::start().await;
    let handler = arrange_post(&server, vec![required_string("body_text")]).await;

    let err = handler
        .handle(
            json!({ "body_text": "ok", "apiKey": "secret" }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("an undeclared argument key must still be refused on a POST tool");
    let msg = err.to_string();

    assert!(
        observed(&server).await.is_empty(),
        "a refused call must record ZERO upstream requests"
    );
    assert!(
        !msg.contains("apiKey") && !msg.contains("secret"),
        "the refusal must echo neither the rejected key nor its value: {msg}"
    );
}
