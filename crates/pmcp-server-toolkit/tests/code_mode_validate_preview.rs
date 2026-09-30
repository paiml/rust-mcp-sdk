//! `validate_code` asks the outbound policy about the calls it can already see.
//!
//! Before this, a script whose call an embedder's `RequestPolicy` would refuse
//! validated cleanly, was handed an approval token, and only failed once
//! `execute_code` ran it (#384, breaking window #389). Now a call whose whole
//! request is known before the script runs is previewed at validation: a refusal
//! comes back as a rejected validation with no token.
//!
//! Only FULLY LITERAL calls are previewed. A call whose path or body depends on a
//! variable, a loop item or a template is not knowable until it runs, so it is
//! skipped and execution stays the authority for it.
//!
//! Run with `cargo test -p pmcp-server-toolkit --features openapi-code-mode
//! --test code_mode_validate_preview -- --test-threads=1`.

#![cfg(feature = "openapi-code-mode")]

use std::sync::{Arc, Mutex};

use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use pmcp_server_toolkit::code_mode::{
    code_mode_http_tools_from_executor, ExecutionConfig, HttpCodeExecutor, ValidationFlavor,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::{
    async_trait, OutboundRequest, PolicyRefusal, RequestPhase, RequestPolicy,
};
use serde_json::{json, Value};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

const POLICY_MESSAGE: &str = "test policy: this endpoint is not allowed";

/// One request as the policy saw it.
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    body: Option<Value>,
    call_id: String,
    phase: RequestPhase,
}

/// Records every request and refuses any whose path contains `/blocked/` or whose
/// body or query mentions `forbidden`.
#[derive(Default)]
struct Recording {
    seen: Mutex<Vec<Seen>>,
}

impl Recording {
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("lock").clone()
    }
}

#[async_trait]
impl RequestPolicy for Recording {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
        self.seen.lock().expect("lock").push(Seen {
            method: req.method.to_string(),
            path: req.path.to_string(),
            query: req.query.to_vec(),
            body: req.body.cloned(),
            call_id: req.call_id.to_string(),
            phase: req.phase,
        });
        let body_forbidden = req
            .body
            .is_some_and(|b| b.to_string().contains("forbidden"));
        let query_forbidden = req.query.iter().any(|(_, v)| v.contains("forbidden"));
        if req.path.contains("/blocked/") || body_forbidden || query_forbidden {
            Err(PolicyRefusal::new(POLICY_MESSAGE))
        } else {
            Ok(())
        }
    }
}

fn code_mode_config() -> ServerConfig {
    let toml = r#"
[server]
name = "validate-preview-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "validate-preview-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "validate-preview-secret-16-or-more"
allow_inline_token_secret_for_dev = true
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

fn handlers(
    upstream: &MockServer,
    policy: Option<Arc<dyn RequestPolicy>>,
) -> (Arc<dyn ToolHandler>, Arc<dyn ToolHandler>) {
    let auth = create_auth_provider(&AuthConfig::None).expect("no-auth provider");
    let mut http = HttpCodeExecutor::new(reqwest::Client::new(), upstream.uri(), auth);
    if let Some(policy) = policy {
        http = http.with_request_policy(policy);
    }
    let server = code_mode_http_tools_from_executor(
        Server::builder().name("t").version("0.1.0"),
        &code_mode_config(),
        http,
        ExecutionConfig::default(),
        ValidationFlavor::OpenApi,
    )
    .expect("code mode wires")
    .build()
    .expect("server builds");
    (
        Arc::clone(server.get_tool("validate_code").expect("validate_code")),
        Arc::clone(server.get_tool("execute_code").expect("execute_code")),
    )
}

async fn validate(v: &Arc<dyn ToolHandler>, code: &str) -> pmcp::Result<Value> {
    v.handle(json!({ "code": code }), RequestHandlerExtra::default())
        .await
}

async fn upstream() -> MockServer {
    let upstream = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&upstream)
        .await;
    upstream
}

/// A literal call the policy refuses is refused AT VALIDATION: a tool-level
/// rejection carrying the policy's message, and no approval token is issued.
#[tokio::test]
async fn validate_code_refuses_a_literal_call_the_policy_refuses() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, _) = handlers(&upstream, Some(policy.clone()));

    let err = validate(
        &validate_tool,
        "const r = await api.get('/blocked/1'); return r;",
    )
    .await
    .expect_err("a refused literal call must not validate");

    assert!(
        matches!(err, pmcp::Error::ToolRejected { .. }),
        "a validation refusal is a tool-level rejection, got: {err:?}"
    );
    assert!(
        err.to_string().contains(POLICY_MESSAGE),
        "the model must read the policy's own message: {err}"
    );
    // The refusal is value-free: it names the call by position and never echoes the
    // path the caller wrote, which may itself carry a sensitive literal.
    assert!(
        !err.to_string().contains("/blocked/1"),
        "a validation refusal must not echo the caller's path: {err}"
    );
    assert!(
        upstream
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "a validation preview must never send anything"
    );
}

/// The preview marks itself `Validate`; execution marks itself `Execute`. A
/// stateful budget policy depends on that difference to avoid charging a dry run.
#[tokio::test]
async fn the_preview_is_marked_validate_and_execution_is_marked_execute() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, execute_tool) = handlers(&upstream, Some(policy.clone()));
    let code = "const r = await api.get('/items/1'); return r;";

    let validated = validate(&validate_tool, code).await.expect("validates");
    let token = validated["approval_token"]
        .as_str()
        .expect("token")
        .to_string();
    let after_validate = policy.seen();
    assert_eq!(after_validate.len(), 1, "one literal call, one preview");
    assert_eq!(after_validate[0].phase, RequestPhase::Validate);

    execute_tool
        .handle(
            json!({ "code": code, "approval_token": token }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("executes");
    let all = policy.seen();
    assert_eq!(all.len(), 2, "the execution consults the policy once more");
    assert_eq!(all[1].phase, RequestPhase::Execute);
}

/// The preview and the send are assembled by the SAME code, so a policy sees the
/// same method, path, query pairs and body in both phases. This is the row that
/// fails if the preview grows its own copy of the request-assembly rules.
#[tokio::test]
async fn the_preview_and_the_send_present_the_same_request() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, execute_tool) = handlers(&upstream, Some(policy.clone()));

    for code in [
        "const r = await api.get('/items', { q: 'x', n: 2 }); return r;",
        "const r = await api.get('/items/1', { name: 'a' }); return r;",
    ] {
        let before = policy.seen().len();
        let validated = validate(&validate_tool, code).await.expect("validates");
        let token = validated["approval_token"]
            .as_str()
            .expect("token")
            .to_string();
        execute_tool
            .handle(
                json!({ "code": code, "approval_token": token }),
                RequestHandlerExtra::default(),
            )
            .await
            .expect("executes");
        let seen = policy.seen();
        let (preview, sent) = (&seen[before], &seen[before + 1]);
        assert_eq!(preview.phase, RequestPhase::Validate);
        assert_eq!(sent.phase, RequestPhase::Execute);
        assert_eq!(
            (
                &preview.method,
                &preview.path,
                &preview.query,
                &preview.body
            ),
            (&sent.method, &sent.path, &sent.query, &sent.body),
            "preview and send must present the same request for: {code}"
        );
    }
}

/// A call whose request depends on run-time data cannot be previewed; it validates,
/// and execution stays the authority for it.
#[tokio::test]
async fn a_call_built_from_runtime_data_is_not_previewed() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, _) = handlers(&upstream, Some(policy.clone()));

    validate(
        &validate_tool,
        "const r = await api.get(`/blocked/${args.id}`); return r;",
    )
    .await
    .expect("a templated path is not previewed, so it validates");
    assert!(
        policy.seen().is_empty(),
        "the policy must not be asked about a request that is not yet known"
    );
}

/// A literal call inside a branch, a loop or a try block is still a call the script
/// can make, so it is previewed.
#[tokio::test]
async fn a_literal_call_in_a_nested_block_is_previewed() {
    let upstream = upstream().await;
    for code in [
        "if (args.go) { const r = await api.get('/blocked/1'); return r; } return 1;",
        "try { const r = await api.get('/blocked/1'); return r; } catch (e) { return 0; }",
        "const r = await Promise.all([api.get('/items/1'), api.get('/blocked/2')]); return r;",
    ] {
        let policy = Arc::new(Recording::default());
        let (validate_tool, _) = handlers(&upstream, Some(policy));
        let err = validate(&validate_tool, code)
            .await
            .expect_err(&format!("must be refused at validation: {code}"));
        assert!(err.to_string().contains(POLICY_MESSAGE), "{code}: {err}");
    }
}

/// A literal argument object is part of the request the policy sees. On a GET it
/// becomes query pairs, through the same conversion the send uses.
///
/// (The toolkit's `[code_mode]` config cannot enable write methods for the OpenAPI
/// flavor, so a literal POST body is not reachable through `validate_code` here; the
/// shared `prepare_request` carries it identically.)
#[tokio::test]
async fn a_literal_argument_object_is_previewed_as_query_pairs() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, _) = handlers(&upstream, Some(policy.clone()));

    let err = validate(
        &validate_tool,
        "const r = await api.get('/items', { note: 'forbidden text' }); return r;",
    )
    .await
    .expect_err("the argument is refused by the policy");
    assert!(err.to_string().contains(POLICY_MESSAGE), "{err}");
    assert_eq!(
        policy.seen()[0].query,
        vec![("note".to_string(), "forbidden text".to_string())]
    );
}

/// Every call in one script shares one call id, so a per-run budget sees one run.
#[tokio::test]
async fn one_validation_uses_one_call_id() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, _) = handlers(&upstream, Some(policy.clone()));

    validate(
        &validate_tool,
        "const a = await api.get('/items/1'); const b = await api.get('/items/2'); return [a, b];",
    )
    .await
    .expect("validates");
    let seen = policy.seen();
    assert_eq!(seen.len(), 2);
    assert!(!seen[0].call_id.is_empty());
    assert_eq!(seen[0].call_id, seen[1].call_id);
}

/// With no policy registered, validation is exactly what it was.
#[tokio::test]
async fn without_a_policy_validation_is_unchanged() {
    let upstream = upstream().await;
    let (validate_tool, _) = handlers(&upstream, None);
    let validated = validate(
        &validate_tool,
        "const r = await api.get('/blocked/1'); return r;",
    )
    .await
    .expect("nothing can refuse it");
    assert!(validated["approval_token"].is_string());
}

/// A rejected validation's STRUCTURED detail is value-free too. The message was, but
/// `explanation` and `metadata.accessed_types` repeated the literal path the script
/// wrote (found by the UMLS team on 2.22.0). Covers both refusal sources: the
/// policy preview and the pre-existing static rules.
#[tokio::test]
async fn a_rejected_validation_does_not_echo_the_path_in_its_structured_detail() {
    let upstream = upstream().await;
    let policy = Arc::new(Recording::default());
    let (validate_tool, _) = handlers(&upstream, Some(policy));

    for code in [
        // refused by the outbound policy preview
        "const r = await api.get('/blocked/SECRETPATH?string=synthetic'); return r;",
        // refused by the static allow_mutations rule
        "const r = await api.post('/items/SECRETPATH', { a: 1 }); return r;",
    ] {
        let err = validate(&validate_tool, code).await.expect_err("refused");
        let pmcp::Error::ToolRejected { message, details } = err else {
            panic!("expected a tool-level rejection for: {code}");
        };
        let details = details.expect("a rejection carries structured detail");
        assert!(
            !message.contains("SECRETPATH") && !details.to_string().contains("SECRETPATH"),
            "the refusal must not echo the caller's path anywhere: {message} / {details}"
        );
        assert!(
            !details.to_string().contains("synthetic"),
            "nor its query string: {details}"
        );
        assert!(
            !details["violations"]
                .as_array()
                .expect("violations")
                .is_empty(),
            "the violations must survive redaction: {details}"
        );
    }
}

/// The redaction is for REFUSALS only: an accepted validation keeps its explanation.
#[tokio::test]
async fn an_accepted_validation_keeps_its_explanation() {
    let upstream = upstream().await;
    let (validate_tool, _) = handlers(&upstream, None);
    let validated = validate(
        &validate_tool,
        "const r = await api.get('/items/1'); return r;",
    )
    .await
    .expect("validates");
    assert!(
        validated["explanation"]
            .as_str()
            .is_some_and(|e| e.contains("/items/1")),
        "an accepted validation explains what it will do: {validated}"
    );
}

// -- F-31 (2.22.2): a script that does not compile is the caller's mistake --------

/// `validate_code` must refuse a script the plan compiler cannot compile, with NO
/// policy registered and NO approval token. The validator alone accepts
/// `api.get('/search/' + q)`, so before this the model was handed a token for a
/// script that could only fail at `execute_code`, as an internal error.
#[tokio::test]
async fn validate_code_refuses_a_script_that_does_not_compile_even_without_a_policy() {
    let upstream = upstream().await;
    let (validate_tool, _) = handlers(&upstream, None);

    let err = validate(
        &validate_tool,
        "const r = await api.get('/search/' + args.q); return r;",
    )
    .await
    .expect_err("a concatenated path does not compile, so it must not validate");

    let pmcp::Error::ToolRejected { message, details } = err else {
        panic!("a compile error is the caller's, so a tool-level rejection");
    };
    assert!(
        message.contains("template literal"),
        "the model must read what to change: {message}"
    );
    let details = details.expect("structured detail");
    assert_eq!(details["valid"], false);
    assert!(details["approval_token"].is_null(), "no token: {details}");
    assert_eq!(details["violations"][0]["rule"], "invalid_script");
}

/// A syntax error is refused at validation without quoting the code.
#[tokio::test]
async fn a_syntax_error_is_refused_at_validation_without_echoing_the_code() {
    let upstream = upstream().await;
    let (validate_tool, _) = handlers(&upstream, None);

    let err = validate(&validate_tool, "const SECRETTOKEN = = 1;")
        .await
        .expect_err("a syntax error must not validate");
    let pmcp::Error::ToolRejected { message, details } = err else {
        panic!("expected a tool-level rejection");
    };
    let everything = format!("{message} {}", details.unwrap_or_default());
    assert!(
        !everything.contains("SECRETTOKEN"),
        "the refusal must not echo the code: {everything}"
    );
    assert!(everything.contains("syntax error"), "{everything}");
}
