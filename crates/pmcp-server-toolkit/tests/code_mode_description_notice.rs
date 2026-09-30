//! `[code_mode] description_notice` reaches both Code Mode tool descriptions.
//!
//! The model reads a tool's description before it calls the tool, so a rule the
//! operator states there is followed instead of discovered through a refusal
//! (#387 item 2, breaking window #389).
//!
//! Run with `cargo test -p pmcp-server-toolkit --features openapi-code-mode,sqlite
//! --test code_mode_description_notice -- --test-threads=1`.

#![cfg(all(feature = "openapi-code-mode", feature = "sqlite"))]

use std::sync::Arc;

use pmcp::{Server, ToolHandler};
use pmcp_server_toolkit::code_mode::{
    code_mode_http_tools_from_executor, code_mode_tools_from_executor, ExecutionConfig,
    HttpCodeExecutor, SqlCodeExecutor, ValidationFlavor,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::sql::SqliteConnector;

const NOTICE: &str = "Responses are de-identified; free-text fields are redacted.";

fn config(notice: Option<&str>) -> ServerConfig {
    let notice_line = notice
        .map(|n| format!("description_notice = \"{n}\"\n"))
        .unwrap_or_default();
    let toml = format!(
        r#"
[server]
name = "description-notice-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "description-notice-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "description-notice-secret-16-or-more"
allow_inline_token_secret_for_dev = true
{notice_line}"#
    );
    ServerConfig::from_toml_strict_validated(&toml).expect("config parses + validates")
}

fn descriptions(server: &pmcp::Server) -> Vec<(String, String)> {
    ["validate_code", "execute_code"]
        .iter()
        .map(|name| {
            let tool: &Arc<dyn ToolHandler> = server.get_tool(name).expect("tool registered");
            let info = tool.metadata().expect("metadata");
            (
                (*name).to_string(),
                info.description.expect("a description"),
            )
        })
        .collect()
}

fn http_server(notice: Option<&str>) -> Server {
    let auth = create_auth_provider(&AuthConfig::None).expect("no-auth provider");
    let http = HttpCodeExecutor::new(reqwest::Client::new(), "http://127.0.0.1:1".into(), auth);
    code_mode_http_tools_from_executor(
        Server::builder().name("t").version("0.1.0"),
        &config(notice),
        http,
        ExecutionConfig::default(),
        ValidationFlavor::OpenApi,
    )
    .expect("wires")
    .build()
    .expect("builds")
}

fn sql_server(notice: Option<&str>) -> Server {
    let cfg = config(notice);
    let connector = SqliteConnector::open_in_memory().expect("in-memory sqlite");
    let executor = SqlCodeExecutor::new(Arc::new(connector), cfg.clone()).expect("executor");
    code_mode_tools_from_executor(
        Server::builder().name("t").version("0.1.0"),
        &cfg,
        Arc::new(executor),
        ValidationFlavor::Sql,
    )
    .expect("wires")
    .build()
    .expect("builds")
}

/// Both tools, on both registration paths, carry the notice AFTER the SDK's own
/// description.
#[test]
fn the_notice_is_appended_to_both_tool_descriptions_on_both_paths() {
    for (path, server, plain) in [
        ("http", http_server(Some(NOTICE)), http_server(None)),
        ("sql", sql_server(Some(NOTICE)), sql_server(None)),
    ] {
        let with = descriptions(&server);
        let without = descriptions(&plain);
        for ((name, with), (_, without)) in with.iter().zip(&without) {
            assert!(
                with.ends_with(NOTICE),
                "{path} {name}: the notice must be the last text: {with}"
            );
            assert!(
                with.starts_with(without.as_str()),
                "{path} {name}: the SDK description must be kept intact before the notice"
            );
            assert_eq!(
                with,
                &format!("{without}\n\n{NOTICE}"),
                "{path} {name}: exactly one blank line between them"
            );
        }
    }
}

/// With no notice the descriptions are exactly the SDK's: the key is opt-in.
#[test]
fn without_a_notice_the_descriptions_are_unchanged() {
    for server in [http_server(None), sql_server(None)] {
        for (name, description) in descriptions(&server) {
            assert!(
                !description.contains(NOTICE) && !description.ends_with("\n\n"),
                "{name}: an unset notice must leave the description untouched: {description}"
            );
        }
    }
}

/// A notice that is only whitespace is no notice.
#[test]
fn a_blank_notice_is_ignored() {
    let blank = descriptions(&http_server(Some("   ")));
    let unset = descriptions(&http_server(None));
    assert_eq!(blank, unset);
}
