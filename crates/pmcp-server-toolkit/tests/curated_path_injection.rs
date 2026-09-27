//! CR-01 on the CURATED single-call HTTP surface — the JS-engine-free build.
//!
//! # What this binary covers
//!
//! The two change-request acceptance rows that need no JS engine, driven end to
//! end through a synthesized `[[tools]]` HTTP tool against a wiremock backend:
//!
//! - a placeholder value carrying a query separator
//!   (`version = "current?string=x"`), and
//! - a placeholder value carrying parent-directory traversal
//!   (`version = "current/../../search/current"`),
//!
//! each asserting ZERO recorded upstream requests and a refusal containing neither
//! the value nor the resolved path. Plus three rows the two refusals cannot carry
//! on their own:
//!
//! - `curated_path_injection_cap_independent_of_d3` — D-08: the placeholder cap is
//!   a module constant in core and `[server.validation] default_max_length = 0`
//!   cannot reach it. Without this row, switching the free-text policy off would
//!   silently switch the length half of the CR-01 fix off too.
//! - the D-11 row — a spec document declaring `allowReserved: true` on a path
//!   parameter still cannot widen the floor. It lives HERE rather than beside the
//!   parser's other unit tests because `src/http/schema.rs` carries a verification
//!   gate requiring that keyword's name to appear in NO non-comment line of that
//!   file, and a test document declaring it is a non-comment line.
//! - two ACCEPT controls: a fully compliant call (exactly ONE recorded upstream
//!   request) and an operator-written query string in the `[[tools]]` `path`. A
//!   refusal suite with no passing control cannot distinguish "refused correctly"
//!   from "handler broken", and a narrowing with no accept row is
//!   indistinguishable from a deleted check.
//!
//! # Why it is named and gated this way
//!
//! Named `curated_path_injection` so a positional
//! `--test curated_path_injection` verify filter resolves to a dedicated binary,
//! and every test fn carries the `curated_path_injection_` prefix so a positional
//! name filter resolves too.
//!
//! The file gate is `#![cfg(all(feature = "http", feature = "input-validation"))]`
//! and deliberately NOT `openapi-code-mode`. Requiring `openapi-code-mode` here
//! would pull in `pmcp-code-mode` and its JS engine, which would test the WRONG
//! build: the whole point of these rows is that CR-01 is closed on the light,
//! curated, JS-engine-free configuration that is the toolkit's common case. A
//! future contributor "fixing" the gate to `openapi-code-mode` would silently move
//! the two SC-1 rows onto a build they are not about — hence the two deliberately
//! opposed grep gates in this plan's verification: the string must appear in this
//! comment and must appear nowhere else in the file.
//!
//! The per-value and composed-path unit tests live in `src/http/client.rs`
//! (`http::client::placeholder_floor` and `http::client::query_separator`); this
//! binary proves the same floor through the public synthesis + handler path.
//!
//! Offline (wiremock only).

#![cfg(all(feature = "http", feature = "input-validation"))]

use std::sync::Arc;

use pmcp::RequestHandlerExtra;
use pmcp_server_toolkit::config::{
    ParamDecl, ServerConfig, ServerSection, ToolDecl, ValidationSection,
};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{
    HttpClient, HttpConnector, OpenApiSchema, Parameter, ParameterLocation,
};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector;
use serde_json::json;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The static bearer the connector adds SERVER-SIDE; the compliant control asserts
/// it reached the upstream request, proving the accepted path is fully wired.
const SERVER_SIDE_TOKEN: &str = "server-side-bearer";

/// The change request's curated tool path: two whole-segment placeholders.
const TOOL_PATH: &str = "/content/{version}/CUI/{cui}";

/// A conforming `cui` value, used by every row so only `version` varies.
const GOOD_CUI: &str = "C0018787";

/// One-tool `ServerConfig` for a curated single-call `GET` tool on `path`.
fn cfg_on(path: &str, validation: ValidationSection) -> ServerConfig {
    ServerConfig {
        server: ServerSection {
            name: "umls-demo".to_string(),
            version: "0.1.0".to_string(),
            validation,
            ..Default::default()
        },
        tools: vec![ToolDecl {
            name: "get_content".to_string(),
            description: Some("Fetch content for a CUI".to_string()),
            path: Some(path.to_string()),
            method: Some("GET".to_string()),
            parameters: vec![string_param("version"), string_param("cui")],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A required string parameter with NO declared narrowing, so the unconditional
/// floor is the only thing that can refuse.
fn string_param(name: &str) -> ParamDecl {
    ParamDecl {
        name: name.to_string(),
        param_type: Some("string".to_string()),
        required: true,
        ..Default::default()
    }
}

/// Mount a catch-all mock and synthesize the tool's handler against `server`.
///
/// The mock answers ANY request, deliberately: with a catch-all mounted, a
/// `received_requests()` of zero can only mean the request was never made, never
/// that it was made and unmatched.
async fn arrange(
    server: &MockServer,
    path: &str,
    validation: ValidationSection,
) -> Arc<dyn pmcp::server::ToolHandler> {
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(server)
        .await;

    let cfg = cfg_on(path, validation);
    let mut out = synthesize_from_config_with_http_connector(&cfg, connector_for(server))
        .expect("synthesize one tool");
    assert_eq!(out.len(), 1, "exactly one synthesized tool expected");
    out.remove(0).2
}

/// An `HttpClient` pointed at `server` that adds the static bearer server-side.
fn connector_for(server: &MockServer) -> Arc<dyn HttpConnector> {
    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: SERVER_SIDE_TOKEN.to_string(),
        required: true,
    })
    .expect("bearer auth provider");
    Arc::new(
        HttpClient::new(reqwest::Client::new(), server.uri(), auth).expect("build http client"),
    )
}

/// Every request the mock actually observed.
async fn observed(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .expect("wiremock records requests")
}

/// Drive `version` through the tool and return the refusal text, asserting zero
/// upstream requests and that the message carries neither the value nor the path.
async fn refuse_version(server: &MockServer, version: &str, validation: ValidationSection) {
    let handler = arrange(server, TOOL_PATH, validation).await;
    let err = handler
        .handle(
            json!({ "version": version, "cui": GOOD_CUI }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("an injected path-placeholder value must be refused");
    let msg = err.to_string();

    assert!(
        observed(server).await.is_empty(),
        "a refused call must record ZERO upstream requests; got {:?}",
        observed(server).await.len()
    );
    assert!(
        msg.contains("version"),
        "the refusal must name the declared parameter: {msg}"
    );
    assert!(
        !msg.contains(version),
        "the refusal must carry no byte of the value: {msg}"
    );
    for fragment in ["/content", "/CUI", GOOD_CUI] {
        assert!(
            !msg.contains(fragment),
            "the refusal must never contain the resolved path ({fragment}): {msg}"
        );
    }
}

/// CR-01, curated row 1 — a query separator arriving through a placeholder value.
#[tokio::test]
async fn curated_path_injection_refuses_a_query_via_a_placeholder_value() {
    let server = MockServer::start().await;
    refuse_version(&server, "current?string=x", ValidationSection::default()).await;
}

/// CR-01, curated row 2 — parent-directory traversal through a placeholder value.
#[tokio::test]
async fn curated_path_injection_refuses_traversal_via_a_placeholder_value() {
    let server = MockServer::start().await;
    refuse_version(
        &server,
        "current/../../search/current",
        ValidationSection::default(),
    )
    .await;
}

/// D-08 — the placeholder cap survives `[server.validation] default_max_length = 0`.
///
/// With the D3 knob at zero no `maxLength` is emitted into `inputSchema` at all, so
/// D1 accepts a 300-code-point value and it reaches `substitute_path`. The refusal
/// therefore proves the cap is a module constant in core that configuration cannot
/// reach — which is the whole reason D-08 exists as a separate decision.
#[tokio::test]
async fn curated_path_injection_cap_independent_of_d3() {
    let server = MockServer::start().await;
    let over_cap = "v".repeat(300);
    refuse_version(
        &server,
        &over_cap,
        ValidationSection {
            default_max_length: 0,
            ..Default::default()
        },
    )
    .await;
}

/// D-11 — a spec document declaring `allowReserved: true` on a path parameter
/// still cannot widen the floor.
///
/// End-to-end rather than a field assertion: the parsed `Parameter` is driven
/// through the real connector with a `/`-carrying value, so the row fails if
/// `allow_slash` is ever wired from the document AND if the floor stops reading it.
/// The ACCEPT half is in the same test — the identical operation with the identical
/// value succeeds once `allow_slash` is set the one legitimate way, from config —
/// so the row cannot be satisfied by refusing everything.
#[tokio::test]
async fn curated_path_injection_spec_allow_reserved_does_not_widen_the_floor() {
    // `r##"…"##`: an OpenAPI document may carry `"#/components/…` refs, and `"#`
    // would close a single-hash raw string.
    const SPEC: &str = r##"
openapi: 3.0.0
info:
  title: Reserved
  version: 1.0.0
paths:
  /files/{subpath}:
    get:
      operationId: getFile
      parameters:
        - name: subpath
          in: path
          required: true
          allowReserved: true
          schema:
            type: string
      responses:
        '200':
          description: OK
"##;

    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&server)
        .await;

    let schema = OpenApiSchema::parse(SPEC).expect("parse spec");
    let spec_op = schema
        .operation_for("/files/{subpath}", "GET")
        .expect("getFile present")
        .clone();
    let subpath = spec_op
        .parameters
        .iter()
        .find(|p| p.name == "subpath")
        .expect("subpath parameter present");
    assert!(
        !subpath.allow_slash,
        "D-11: a spec keyword must never set allow_slash"
    );

    let client = connector_for(&server);
    let args = json!({ "subpath": "a/b/c" });
    let err = client
        .execute(&spec_op, &args)
        .await
        .expect_err("a spec must not lift the path-separator refusal");
    assert!(err.to_string().contains("subpath"), "{err}");
    assert!(
        observed(&server).await.is_empty(),
        "a refused call must record ZERO upstream requests"
    );

    // ACCEPT half: the SAME operation and the SAME value succeed once the
    // permission arrives the one legitimate way — from the server's own config.
    let mut config_op = spec_op.clone();
    config_op.parameters =
        vec![Parameter::new("subpath", ParameterLocation::Path, true).with_rules(None, None, true)];
    client
        .execute(&config_op, &args)
        .await
        .expect("a config-declared allow_slash must permit the value");
    assert_eq!(
        observed(&server).await.len(),
        1,
        "the accepted call must produce exactly ONE upstream request"
    );
}

/// The compliant CONTROL — a conforming call produces exactly ONE upstream request
/// carrying the server-side auth header.
#[tokio::test]
async fn curated_path_injection_accepts_a_compliant_call_with_one_upstream_request() {
    let server = MockServer::start().await;
    let handler = arrange(&server, TOOL_PATH, ValidationSection::default()).await;

    handler
        .handle(
            json!({ "version": "2026AA", "cui": GOOD_CUI }),
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
    assert_eq!(requests[0].url.path(), "/content/2026AA/CUI/C0018787");
    let auth = requests[0]
        .headers
        .get("authorization")
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    assert_eq!(auth, format!("Bearer {SERVER_SIDE_TOKEN}"));
}

/// The narrowing's ACCEPT control on the curated surface: an operator-written query
/// string in the `[[tools]]` `path` reaches the wire instead of being refused.
///
/// A `[[tools]]` `path` is operator-authored configuration, exactly as a Code Mode
/// script's literal path text is operator-authored script text, so the same
/// asymmetry applies — refusing `..` from it catches a traversal bug, refusing `?`
/// from it rejects legitimate configuration.
#[tokio::test]
async fn curated_path_injection_accepts_an_author_written_query_string_in_the_path() {
    let server = MockServer::start().await;
    let handler = arrange(
        &server,
        "/content/{version}/CUI?string=x",
        ValidationSection::default(),
    )
    .await;

    handler
        .handle(
            json!({ "version": "2026AA", "cui": GOOD_CUI }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("an operator-written query string in the config path must be accepted");

    let requests = observed(&server).await;
    assert_eq!(requests.len(), 1, "exactly ONE upstream request expected");
    assert_eq!(requests[0].url.path(), "/content/2026AA/CUI");
    let query = requests[0].url.query().unwrap_or_default();
    assert!(
        query.contains("string=x"),
        "the operator-written query must survive to the wire: {query}"
    );
}
