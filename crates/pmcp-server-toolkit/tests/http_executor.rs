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

// =============================================================================
// Phase 128 — CR-01 refusal probes, driven through `PlanExecutor`
//
// These drive `pmcp_code_mode::PlanExecutor` over a real `HttpCodeExecutor`
// rather than calling `execute_request` directly, because `PlanExecutor` is the
// layer where placeholder resolution now happens (D-09). A probe that called the
// executor directly would bypass the very guard it is meant to prove.
//
// Every refusal probe asserts, via `received_requests()`, that the wiremock
// upstream recorded ZERO requests — a refusal that still dispatches is not a
// refusal — and asserts the surfaced error contains neither the placeholder
// value nor the resolved path (SC-7).
// =============================================================================

use pmcp_code_mode::{ExecutionConfig, PlanCompiler, PlanExecutor};

/// Run `script` against a wiremock server that answers EVERY request with `200
/// {}`, returning the execution result plus the number of requests the upstream
/// actually observed.
///
/// The catch-all mock is deliberate: it means a zero request count can only be
/// because the request was never made, not because it failed to match.
async fn run_script(script: &str) -> (Result<serde_json::Value, String>, usize) {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth);

    let mut compiler = PlanCompiler::new();
    let plan = compiler
        .compile_code(script)
        .expect("the probe script compiles");
    let mut executor = PlanExecutor::new(exec, ExecutionConfig::default());
    let outcome = executor
        .execute(&plan)
        .await
        .map(|r| r.value)
        .map_err(|e| e.to_string());

    let observed = server
        .received_requests()
        .await
        .expect("wiremock records requests")
        .len();
    (outcome, observed)
}

/// Assert a refused probe: `Err`, zero upstream requests, and a message carrying
/// neither the value fragments nor the path.
fn assert_refused(
    outcome: &Result<serde_json::Value, String>,
    observed: usize,
    forbidden: &[&str],
) {
    let rendered = outcome
        .as_ref()
        .expect_err("the probe must be refused, not dispatched");
    assert_eq!(
        observed, 0,
        "a refusal must reach ZERO upstream requests; got {observed}. Error was: {rendered}"
    );
    for needle in forbidden {
        assert!(
            !rendered.contains(needle),
            "the refusal must carry neither the value nor the path; found {needle:?} in {rendered:?}"
        );
    }
}

/// CR-01 row "Query via placeholder": `api.get('/search/{v}', {v:'2026AA?string=x'})`.
#[tokio::test]
async fn http_executor_refuses_query_separator_in_a_placeholder_value() {
    let tail = "x".repeat(60);
    let script = format!(
        "const r = await api.get('/search/{{v}}', {{ v: '2026AA?string={tail}' }});\nreturn r;"
    );
    let (outcome, observed) = run_script(&script).await;
    assert_refused(&outcome, observed, &["2026AA", "?string=", "/search/"]);
}

/// CR-01 row "Length via two placeholders": `api.get('/search/{a}{b}', …)` where
/// 180 + 200 code points compose over the 256 cap.
///
/// The test asserts BOTH halves. Without the first half it would still pass with
/// the composed check deleted, because some other rule could be doing the
/// refusing — and this row is the one all three review lanes identified as having
/// no implementing mechanism before this phase.
#[tokio::test]
async fn http_executor_refuses_length_composed_from_two_placeholders() {
    let first = "a".repeat(180);
    let second = "b".repeat(200);

    // HALF ONE — each value passes the PER-VALUE check in isolation.
    let rules = pmcp_code_mode::PlaceholderRules::default();
    pmcp_code_mode::validate_path_placeholder("a", &first, &rules)
        .expect("180 code points is under the cap and must pass alone");
    pmcp_code_mode::validate_path_placeholder("b", &second, &rules)
        .expect("200 code points is under the cap and must pass alone");

    // HALF TWO — composed, the same two values are refused before dispatch.
    let script = format!(
        "const r = await api.get('/search/{{a}}{{b}}', {{ a: '{first}', b: '{second}' }});\nreturn r;"
    );
    let (outcome, observed) = run_script(&script).await;
    assert_refused(&outcome, observed, &["aaaaa", "bbbbb", "/search/"]);
}

/// FORK-2 sibling of the row above: the same structure, a different rule. `"."`
/// plus `"."` composes to a parent-directory segment rather than an over-cap one,
/// which is the clearest demonstration that per-value checking is insufficient BY
/// CONSTRUCTION.
#[tokio::test]
async fn http_executor_refuses_traversal_composed_from_two_placeholders() {
    let script = "const r = await api.get('/search/{a}{b}/x', { a: '.', b: '.' });\nreturn r;";
    let (outcome, observed) = run_script(script).await;
    assert_refused(&outcome, observed, &["/search/"]);
}

/// CR-01 row "Traversal through a spec path": `/content/{version}/CUI/{cui}` with
/// `version` carrying traversal plus a query separator.
#[tokio::test]
async fn http_executor_refuses_traversal_through_a_spec_declared_path() {
    let script = "const r = await api.get('/content/{version}/CUI/{cui}', \
                  { version: 'current/../../search/current?string=abc', cui: 'C0018787' });\nreturn r;";
    let (outcome, observed) = run_script(script).await;
    assert_refused(
        &outcome,
        observed,
        &["../", "?string=", "/content/", "C0018787"],
    );
}

/// FORK 2 — the LAYER-1 route, which uses no `{key}` placeholder at all.
///
/// This probe is ADDED beyond the change request's matrix, deliberately. All
/// three of the CR's Code Mode rows use `{key}` placeholders, which is precisely
/// why the `${var}` template-literal route survived review of the CR itself: a
/// script writing `` api.get(`/search/${v}`) `` never enters layer 2, so before
/// this phase it was never floored. Do not prune this as extraneous — it is the
/// probe that would have caught the gap.
#[tokio::test]
async fn http_executor_refuses_reserved_char_in_template_literal_interpolation() {
    let tail = "x".repeat(60);
    let script = format!(
        "const v = '2026AA?string={tail}';\nconst r = await api.get(`/search/${{v}}`);\nreturn r;"
    );
    let (outcome, observed) = run_script(&script).await;
    assert_refused(&outcome, observed, &["2026AA", "?string=", "/search/"]);
}

/// The CONTROL. A refusal suite with no passing control cannot distinguish
/// "refused correctly" from "broken": asserts exactly ONE recorded upstream
/// request, on the substituted path, with auth added server-side.
#[tokio::test]
async fn http_executor_compliant_placeholder_call_produces_exactly_one_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/content/current/CUI/C0018787"))
        .and(header("authorization", "Bearer server-side-tok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "Headache"})))
        .mount(&server)
        .await;

    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: "server-side-tok".to_string(),
        required: true,
    })
    .expect("bearer");
    let exec = HttpCodeExecutor::new(reqwest::Client::new(), server.uri(), auth);

    let script = "const r = await api.get('/content/{version}/CUI/{cui}', \
                  { version: 'current', cui: 'C0018787' });\nreturn r;";
    let mut compiler = PlanCompiler::new();
    let plan = compiler
        .compile_code(script)
        .expect("the control script compiles");
    let mut executor = PlanExecutor::new(exec, ExecutionConfig::default());
    let result = executor
        .execute(&plan)
        .await
        .expect("a compliant placeholder call must succeed");
    assert_eq!(result.value["name"], "Headache");

    let observed = server
        .received_requests()
        .await
        .expect("wiremock records requests");
    assert_eq!(observed.len(), 1, "exactly one upstream request");
    assert_eq!(observed[0].url.path(), "/content/current/CUI/C0018787");
    assert_eq!(
        observed[0]
            .headers
            .get("authorization")
            .map(|v| v.to_str().unwrap_or_default()),
        Some("Bearer server-side-tok"),
        "auth is added server-side, never by the script"
    );
}
