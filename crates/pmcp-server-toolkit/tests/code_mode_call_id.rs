//! One `execute_code` run makes many requests; they share ONE `call_id`.
//!
//! Refs #386 (breaking window #389). A per-request cap alone lets a caller split
//! free text across several requests that each fit under it, and a policy that sees
//! each request in isolation cannot total them. The id is what lets it: every
//! request one `tools/call` makes carries the same id, and the next call gets a
//! fresh one.
//!
//! Run with `cargo test -p pmcp-server-toolkit --features openapi-code-mode
//! --test code_mode_call_id -- --test-threads=1`.

#![cfg(feature = "openapi-code-mode")]

use std::sync::{Arc, Mutex};

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

/// `(path, call_id)` for every request the policy was shown. Allows everything.
type Log = Arc<Mutex<Vec<(String, String)>>>;

struct CallIdRecorder(Log);

#[async_trait]
impl RequestPolicy for CallIdRecorder {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
        self.0
            .lock()
            .expect("lock")
            .push((req.path.to_string(), req.call_id.to_string()));
        Ok(())
    }
}

fn code_mode_config() -> ServerConfig {
    let toml = r#"
[server]
name = "call-id-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "call-id-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "call-id-secret-16-or-more-chars"
allow_inline_token_secret_for_dev = true
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

/// A script making TWO requests, so a shared id is observable.
const TWO_CALLS: &str = "const a = await api.get(`/one/${args.id}`); \
                         const b = await api.get(`/two/${args.id}`); return b;";

async fn run_once(
    validate: &Arc<dyn ToolHandler>,
    execute: &Arc<dyn ToolHandler>,
) -> pmcp::Result<Value> {
    let validated = validate
        .handle(json!({ "code": TWO_CALLS }), RequestHandlerExtra::default())
        .await
        .expect("the code validates");
    let token = validated["approval_token"]
        .as_str()
        .unwrap_or_else(|| panic!("no approval token: {validated}"))
        .to_string();
    execute
        .handle(
            json!({
                "code": TWO_CALLS,
                "approval_token": token,
                "variables": { "id": "1" }
            }),
            RequestHandlerExtra::default(),
        )
        .await
}

async fn arrange(log: Log) -> (MockServer, Arc<dyn ToolHandler>, Arc<dyn ToolHandler>) {
    let upstream = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&upstream)
        .await;
    let auth = create_auth_provider(&AuthConfig::None).expect("no-auth provider");
    let http = HttpCodeExecutor::new(reqwest::Client::new(), upstream.uri(), auth)
        .with_request_policy(Arc::new(CallIdRecorder(log)));
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
    let validate = Arc::clone(server.get_tool("validate_code").expect("validate_code"));
    let execute = Arc::clone(server.get_tool("execute_code").expect("execute_code"));
    (upstream, validate, execute)
}

/// Both requests of one run carry the SAME non-empty id.
#[tokio::test]
async fn code_mode_call_id_is_shared_by_every_request_one_run_makes() {
    let log: Log = Arc::default();
    let (_upstream, validate, execute) = arrange(Arc::clone(&log)).await;

    run_once(&validate, &execute)
        .await
        .expect("the run succeeds");

    let seen = log.lock().expect("lock").clone();
    assert_eq!(seen.len(), 2, "the script makes two requests: {seen:?}");
    assert!(!seen[0].1.is_empty(), "the id must be set: {seen:?}");
    assert_eq!(
        seen[0].1, seen[1].1,
        "one execute_code run must stamp ONE id on all its requests: {seen:?}"
    );
}

/// A second run gets a DIFFERENT id, or a budget keyed on it would accumulate across
/// unrelated calls and starve every caller after the first.
#[tokio::test]
async fn code_mode_call_id_differs_between_two_runs() {
    let log: Log = Arc::default();
    let (_upstream, validate, execute) = arrange(Arc::clone(&log)).await;

    run_once(&validate, &execute).await.expect("first run");
    run_once(&validate, &execute).await.expect("second run");

    let seen = log.lock().expect("lock").clone();
    assert_eq!(seen.len(), 4, "two runs of two requests: {seen:?}");
    assert_ne!(
        seen[0].1, seen[2].1,
        "two tools/call invocations must not share a call id: {seen:?}"
    );
}
