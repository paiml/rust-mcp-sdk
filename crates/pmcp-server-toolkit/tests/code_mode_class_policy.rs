//! `[code_mode]` operation classes for an OpenAPI server, end to end.
//!
//! A config-only OpenAPI server states its own security posture: a mode per
//! class (`read_mode`, `write_mode`, `delete_mode`, `admin_mode`), allow and
//! block lists, and an operation catalog. These tests drive the declared policy
//! through the boundaries a deployment uses — config load, `validate_code`,
//! `execute_code`'s executor and curated `[[tools]]` — with no policy evaluator.
//!
//! The UMLS team's four probes are here: a write refused at `validate_code`, an
//! operation outside the allowlist refused, a misspelled key refused at boot,
//! and a reclassified operation changing the verdict.
//!
//! Run with `cargo test -p pmcp-server-toolkit --features openapi-code-mode
//! --test code_mode_class_policy`.

#![cfg(feature = "openapi-code-mode")]

use std::sync::Arc;

use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use pmcp_server_toolkit::code_mode::{
    code_mode_http_tools_from_executor, ExecutionConfig, ExecutionError, HttpCodeExecutor,
    HttpExecutor, ResolvedPath, ValidationFlavor,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::error::{ConfigValidationError, ToolkitError};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector_and_scripts;
use serde_json::{json, Value};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

const HEADER: &str = r#"
[server]
name = "umls"
version = "0.1.0"

[backend]
base_url = "http://127.0.0.1:9"
"#;

const CODE_MODE: &str = r#"
[code_mode]
enabled = true
server_id = "umls"
token_secret = "class-policy-test-secret-16+"
allow_inline_token_secret_for_dev = true
"#;

/// The UMLS declared posture: reads allowed, everything else denied.
const READ_ONLY: &str = r#"
read_mode = "allow_all"
write_mode = "deny_all"
delete_mode = "deny_all"
admin_mode = "deny_all"
"#;

fn toml(code_mode_keys: &str, rest: &str) -> String {
    format!("{HEADER}{CODE_MODE}{code_mode_keys}\n{rest}")
}

fn load(code_mode_keys: &str, rest: &str) -> pmcp_server_toolkit::error::Result<ServerConfig> {
    ServerConfig::from_toml_strict_validated(&toml(code_mode_keys, rest))
}

fn config(code_mode_keys: &str, rest: &str) -> ServerConfig {
    load(code_mode_keys, rest).expect("config loads")
}

fn validation_error(code_mode_keys: &str, rest: &str) -> ConfigValidationError {
    match load(code_mode_keys, rest) {
        Err(ToolkitError::Validation(e)) => e,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn executor(base_url: &str) -> HttpCodeExecutor {
    let auth = create_auth_provider(&AuthConfig::None).expect("no-auth provider");
    HttpCodeExecutor::new(reqwest::Client::new(), base_url.to_string(), auth)
}

fn validate_tool(cfg: &ServerConfig) -> Arc<dyn ToolHandler> {
    let server = code_mode_http_tools_from_executor(
        Server::builder().name("t").version("0.1.0"),
        cfg,
        executor("http://127.0.0.1:9"),
        ExecutionConfig::default(),
        ValidationFlavor::OpenApi,
    )
    .expect("code mode wires")
    .build()
    .expect("server builds");
    Arc::clone(server.get_tool("validate_code").expect("validate_code"))
}

async fn validate(cfg: &ServerConfig, code: &str) -> pmcp::Result<Value> {
    validate_tool(cfg)
        .handle(json!({ "code": code }), RequestHandlerExtra::default())
        .await
}

async fn refusal(cfg: &ServerConfig, code: &str) -> String {
    let err = validate(cfg, code).await.expect_err("refused");
    assert!(
        matches!(err, pmcp::Error::ToolRejected { .. }),
        "a policy refusal is a tool-level rejection: {err:?}"
    );
    err.to_string()
}

// --- config load -----------------------------------------------------------

#[test]
fn the_declared_read_only_posture_loads() {
    let cfg = config(READ_ONLY, "");
    let section = cfg.code_mode.expect("code_mode");
    assert_eq!(
        pmcp_server_toolkit::code_mode::openapi_class_policy(&section).to_string(),
        "read=allow_all write=deny_all delete=deny_all admin=deny_all \
         blocked_operations=0 blocked_paths=0"
    );
}

/// UMLS probe 3.
#[test]
fn a_misspelled_key_fails_the_boot() {
    let err = load("write_mod = \"deny_all\"", "").expect_err("refused");
    assert!(matches!(err, ToolkitError::Parse(_)), "{err:?}");
}

#[test]
fn a_misspelled_mode_or_category_fails_the_boot() {
    assert!(matches!(
        load("write_mode = \"deny-all\"", ""),
        Err(ToolkitError::Parse(_))
    ));
    let catalog = r#"
[[code_mode.operations]]
id = "search"
category = "reed"
path = "GET /search/{version}"
"#;
    assert!(matches!(load("", catalog), Err(ToolkitError::Parse(_))));
}

#[test]
fn a_sql_key_on_an_openapi_server_fails_the_boot() {
    let err = validation_error("allow_writes = true", "");
    assert!(
        matches!(
            err,
            ConfigValidationError::CodeModeKeyWrongBackend {
                key: "allow_writes",
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("write_mode"), "{err}");
}

#[test]
fn a_class_key_on_a_server_without_a_backend_fails_the_boot() {
    let no_backend = format!(
        "[server]\nname = \"s\"\nversion = \"0.1.0\"\n{CODE_MODE}write_mode = \"deny_all\"\n"
    );
    let err = ServerConfig::from_toml_strict_validated(&no_backend).expect_err("refused");
    assert!(
        matches!(
            err,
            ToolkitError::Validation(ConfigValidationError::CodeModeKeyWrongBackend {
                key: "write_mode",
                ..
            })
        ),
        "{err:?}"
    );
}

#[test]
fn an_allowlist_mode_needs_its_list() {
    let err = validation_error("read_mode = \"allowlist\"", "");
    assert!(
        matches!(
            err,
            ConfigValidationError::ClassModeNeedsList {
                class: "read",
                list: "allowed_operations",
                ..
            }
        ),
        "{err:?}"
    );
    let err = validation_error("write_mode = \"blocklist\"", "");
    assert!(
        matches!(
            err,
            ConfigValidationError::ClassModeNeedsList {
                list: "blocked_operations",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn catalog_ids_are_unique_and_entries_complete() {
    let dup = r#"
[[code_mode.operations]]
id = "search"
category = "read"
path = "GET /search"
[[code_mode.operations]]
id = "search"
category = "read"
path = "GET /other"
"#;
    assert!(matches!(
        validation_error("", dup),
        ConfigValidationError::DuplicateOperationId(id) if id == "search"
    ));
    let empty = "[[code_mode.operations]]\nid = \"x\"\ncategory = \"read\"\npath = \"\"\n";
    assert!(matches!(
        validation_error("", empty),
        ConfigValidationError::EmptyOperationField(0)
    ));
}

#[test]
fn an_unknown_auto_approve_level_fails_the_boot() {
    assert!(matches!(
        validation_error("auto_approve_levels = [\"lowe\"]", ""),
        ConfigValidationError::UnknownAutoApproveLevel(l) if l == "lowe"
    ));
}

// --- validate_code ---------------------------------------------------------

/// UMLS probe 1.
#[tokio::test]
async fn a_write_is_refused_at_validate_code() {
    let cfg = config(READ_ONLY, "");
    let message = refusal(
        &cfg,
        "const r = await api.post('/crosswalk/SECRET', {}); return r;",
    )
    .await;
    assert!(
        message.contains("write operations are deny_all"),
        "{message}"
    );
    assert!(
        !message.contains("SECRET"),
        "never echoes the path: {message}"
    );

    let ok = validate(
        &cfg,
        "const r = await api.get('/search/current'); return r;",
    )
    .await
    .expect("a read validates");
    assert!(ok["approval_token"].is_string());
}

/// UMLS probe 2.
#[tokio::test]
async fn an_operation_outside_the_allowlist_is_refused() {
    let keys = r#"
read_mode = "allowlist"
allowed_operations = ["searchConcepts", "GET /content/current/CUI/{cui}"]
"#;
    let catalog = r#"
[[code_mode.operations]]
id = "searchConcepts"
category = "read"
path = "GET /search/{version}"
"#;
    let cfg = config(keys, catalog);
    for allowed in [
        "const r = await api.get('/search/current'); return r;",
        "const r = await api.get('/content/current/CUI/C0004057'); return r;",
    ] {
        validate(&cfg, allowed).await.expect(allowed);
    }
    let message = refusal(
        &cfg,
        "const r = await api.get('/semantic-network/current'); return r;",
    )
    .await;
    assert!(
        message.contains("not in this server's read allowlist"),
        "{message}"
    );
}

/// UMLS probe 4: the catalog, not the HTTP method, decides the class.
#[tokio::test]
async fn reclassifying_an_operation_changes_the_verdict() {
    let search = "const r = await api.post('/search', { string: 'aspirin' }); return r;";
    let catalog = |category: &str| {
        format!(
            "[[code_mode.operations]]\nid = \"search\"\ncategory = \"{category}\"\npath = \"POST /search\"\n"
        )
    };

    refusal(&config(READ_ONLY, ""), search).await;
    validate(&config(READ_ONLY, &catalog("read")), search)
        .await
        .expect("a POST declared read is a read");
    let message = refusal(&config(READ_ONLY, &catalog("admin")), search).await;
    assert!(message.contains("operation 'search'"), "{message}");
    assert!(
        message.contains("admin operations are deny_all"),
        "{message}"
    );

    let admin_allowed = format!("{READ_ONLY}\nadmin_mode = \"allow_all\"")
        .replace("admin_mode = \"deny_all\"\n", "");
    validate(&config(&admin_allowed, &catalog("admin")), search)
        .await
        .expect("admin allowed");
}

#[tokio::test]
async fn blocked_paths_and_operations_apply_in_every_mode() {
    let keys = r#"
write_mode = "allow_all"
blocked_paths = ["/admin"]
blocked_operations = ["PATCH", "POST /items/{id}/purge"]
"#;
    let cfg = config(keys, "");
    refusal(&cfg, "const r = await api.get('/admin/users'); return r;").await;
    refusal(&cfg, "const r = await api.patch('/items/1', {}); return r;").await;
    refusal(
        &cfg,
        "const r = await api.post('/items/9/purge', {}); return r;",
    )
    .await;
    validate(&cfg, "const r = await api.post('/items/9', {}); return r;")
        .await
        .expect("an unblocked write");
}

/// With no class key set, Code Mode keeps the 0.3 defaults.
#[tokio::test]
async fn defaults_without_class_keys_allow_reads_and_deny_writes() {
    let cfg = config("", "");
    validate(&cfg, "const r = await api.get('/x'); return r;")
        .await
        .expect("read");
    refusal(&cfg, "const r = await api.delete('/x/1'); return r;").await;
}

// --- execute_code's executor -----------------------------------------------

/// Validation sees `/items/${id}` as a template; the executor sees where the
/// request goes, and refuses before anything is sent.
#[tokio::test]
async fn the_executor_rechecks_the_resolved_path() {
    let upstream = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&upstream)
        .await;
    let keys = r#"
write_mode = "allowlist"
allowed_operations = ["PUT /items/{id}"]
"#;
    let section = config(keys, "").code_mode.expect("code_mode");
    let exec = executor(&upstream.uri()).with_class_policy(&section);
    assert!(exec.has_class_policy());

    exec.execute_request("PUT", ResolvedPath::from_checked("/items/7").unwrap(), None)
        .await
        .expect("allowlisted");
    match exec
        .execute_request(
            "PUT",
            ResolvedPath::from_checked("/items/7/owner").unwrap(),
            None,
        )
        .await
    {
        Err(ExecutionError::RequestRefused { message }) => {
            assert!(message.contains("write allowlist"), "{message}");
            assert!(
                !message.contains("owner"),
                "never echoes the path: {message}"
            );
        },
        other => panic!("expected RequestRefused, got {other:?}"),
    }
    let sent = upstream.received_requests().await.unwrap_or_default();
    assert_eq!(sent.len(), 1, "the refused request was never sent");
}

// --- curated tools ---------------------------------------------------------

fn synthesize(cfg: &ServerConfig) -> pmcp_server_toolkit::error::Result<usize> {
    let auth = create_auth_provider(&AuthConfig::None).expect("no-auth provider");
    let connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(reqwest::Client::new(), "http://127.0.0.1:9".into(), auth)
            .expect("connector"),
    );
    synthesize_from_config_with_http_connector_and_scripts(
        cfg,
        connector,
        executor("http://127.0.0.1:9"),
        ExecutionConfig::default(),
    )
    .map(|tools| tools.len())
}

const WRITE_TOOL: &str = r#"
[[tools]]
name = "create_item"
method = "POST"
path = "/items"
"#;

const WRITE_SCRIPT_TOOL: &str = r#"
[[tools]]
name = "touch_item"
script = "const r = await api.post('/items', {}); return r;"
"#;

#[test]
fn a_curated_tool_of_a_denied_class_fails_the_boot() {
    for tool in [WRITE_TOOL, WRITE_SCRIPT_TOOL] {
        let err = synthesize(&config(READ_ONLY, tool)).expect_err("refused");
        let ToolkitError::Validation(ConfigValidationError::CuratedToolRefusedByPolicy {
            reason,
            ..
        }) = err
        else {
            panic!("expected CuratedToolRefusedByPolicy, got {err:?}");
        };
        assert!(reason.contains("write operations are deny_all"), "{reason}");
    }
}

#[test]
fn a_curated_tool_the_policy_allows_synthesizes() {
    let keys = "write_mode = \"allowlist\"\nallowed_operations = [\"POST /items\"]\n";
    assert_eq!(synthesize(&config(keys, WRITE_TOOL)).expect("allowed"), 1);
}

/// Back-compat: with no class key set, curated tools are not classified.
#[test]
fn curated_tools_are_unclassified_without_class_keys() {
    assert_eq!(synthesize(&config("", WRITE_TOOL)).expect("unchanged"), 1);
    assert_eq!(
        synthesize(&config("", WRITE_SCRIPT_TOOL)).expect("unchanged"),
        1
    );
}
