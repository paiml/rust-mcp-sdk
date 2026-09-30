//! A Code Mode refusal is a TOOL-LEVEL rejection; a real fault is not.
//!
//! Drives the registered `validate_code` then `execute_code` handlers end to end
//! (pmcp-code-mode 0.7 / toolkit 0.3, breaking window #389, refs #385).
//!
//! Before 0.7 every failure of `execute_code` was `pmcp::Error::Internal`, so a
//! policy refusal reached the model as `Internal error: Execution error: Runtime
//! error: ...` and read as a server crash. A refusal is the caller's to fix, so it
//! is now `Error::ToolRejected` (an `isError: true` result); a genuine fault stays
//! `Internal`. These two rows are the two halves of that contract.
//!
//! The refused call builds its path from a run-time variable on purpose: that is a
//! request `validate_code` can never see, so the row stays valid whatever static
//! checking `validate_code` later performs.
//!
//! Run with `cargo test -p pmcp-server-toolkit --features openapi-code-mode
//! --test code_mode_refusal_channel -- --test-threads=1`.

#![cfg(feature = "openapi-code-mode")]

use std::sync::Arc;

use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use pmcp_server_toolkit::code_mode::{
    code_mode_http_tools_from_executor, ExecutionConfig, HttpCodeExecutor, ValidationFlavor,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::{async_trait, OutboundRequest, PolicyRefusal, RequestPolicy};
use serde_json::{json, Value};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The policy's own fixed message, so a test can assert it is what the model reads.
const POLICY_MESSAGE: &str = "test policy: this endpoint is not allowed";

/// Refuses every request whose path contains `/blocked/`.
struct RefuseBlocked;

#[async_trait]
impl RequestPolicy for RefuseBlocked {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
        if req.path.contains("/blocked/") {
            Err(PolicyRefusal::new(POLICY_MESSAGE))
        } else {
            Ok(())
        }
    }
}

fn code_mode_config() -> ServerConfig {
    let toml = r#"
[server]
name = "refusal-channel-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "refusal-channel-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "refusal-channel-secret-16-or-more"
allow_inline_token_secret_for_dev = true
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

/// The registered `validate_code` and `execute_code` handlers over an executor
/// pointed at `upstream`, with `policy` attached when given.
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

/// `validate_code` then `execute_code` for `code` with `variables`, returning the
/// `execute_code` result.
async fn validate_then_execute(
    validate: &Arc<dyn ToolHandler>,
    execute: &Arc<dyn ToolHandler>,
    code: &str,
    variables: Value,
) -> pmcp::Result<Value> {
    let validated = validate
        .handle(json!({ "code": code }), RequestHandlerExtra::default())
        .await
        .expect("the code validates");
    let token = validated["approval_token"]
        .as_str()
        .unwrap_or_else(|| panic!("validate_code issued no approval token: {validated}"))
        .to_string();
    execute
        .handle(
            json!({ "code": code, "approval_token": token, "variables": variables }),
            RequestHandlerExtra::default(),
        )
        .await
}

/// A request the policy refuses is a TOOL-LEVEL rejection carrying the policy's own
/// message, and nothing reaches the upstream.
#[tokio::test]
async fn code_mode_a_policy_refusal_is_a_tool_level_rejection() {
    let upstream = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&upstream)
        .await;
    let (validate, execute) = handlers(&upstream, Some(Arc::new(RefuseBlocked)));

    let err = validate_then_execute(
        &validate,
        &execute,
        "const r = await api.get(`/blocked/${args.id}`); return r;",
        json!({ "id": "1" }),
    )
    .await
    .expect_err("the policy refuses the call");

    assert!(
        matches!(err, pmcp::Error::ToolRejected { .. }),
        "a refusal is the caller's to fix, so it must be a tool-level rejection, got: {err:?}"
    );
    assert!(
        err.to_string().contains(POLICY_MESSAGE),
        "the model must read the policy's own message, got: {err}"
    );
    assert!(
        !err.to_string().contains("Internal"),
        "a refusal must not read as an internal error: {err}"
    );
    assert!(
        upstream
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "a refused call must record ZERO upstream requests"
    );
}

/// The other half of the contract: a genuine FAULT is still `Internal`. If every
/// failure became a tool-level rejection, a backend outage would read to a model as
/// its own mistake and it would retry a call that cannot succeed.
#[tokio::test]
async fn code_mode_an_upstream_fault_is_still_internal() {
    let upstream = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500))
        .mount(&upstream)
        .await;
    let (validate, execute) = handlers(&upstream, None);

    let err = validate_then_execute(
        &validate,
        &execute,
        "const r = await api.get(`/anything/${args.id}`); return r;",
        json!({ "id": "1" }),
    )
    .await
    .expect_err("the upstream returns 500");

    assert!(
        matches!(err, pmcp::Error::Internal(_)),
        "a real fault must stay Internal, got: {err:?}"
    );
}
