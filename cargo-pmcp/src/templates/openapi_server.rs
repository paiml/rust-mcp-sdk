//! Single-crate `openapi-server` template emitter (Shape B/C/D, OAPI-07 / CF-3).
//!
//! The OpenAPI sibling of [`crate::templates::sql_server`]: one `generate`
//! orchestrator that calls one private `generate_<file>` fn per output file, each
//! a single raw `fs::write(dir.join("X"), <literal>).context(...)`. There is NO
//! template engine — emission is raw string literals (`format!` escapes literal
//! braces as `{{`/`}}`).
//!
//! It emits a SINGLE runnable crate (CF-3):
//! - `Cargo.toml` — pins `pmcp-server-toolkit` with the `openapi-code-mode`
//!   UMBRELLA feature (composes `http` + `code-mode` + `pmcp-code-mode/js-runtime`
//!   so the script-tool / Code-Mode engine compiles in — matches the Plan 90-06
//!   binary dep, RESEARCH Pitfall 4). `pmcp` carries `streamable-http`. It also
//!   depends on `pmcp-openapi-server` (the Plan 06 lib) for the `dispatch` +
//!   `build_server` orchestrators: unlike the SQL path there is NO
//!   `ServerBuilderExt::*_with_http_connector` (Plan 06 SUMMARY decision), so the
//!   `(HttpConnector, HttpCodeExecutor)` pair seam + the assemble step live in the
//!   `pmcp-openapi-server` library, exactly as its own `examples/openapi_server_min.rs`
//!   imports them.
//! - `src/main.rs` — the ≤15-line wiring (CF-5): load config[+optional spec] →
//!   `dispatch` → `build_server` → serve HTTP, with the `StreamableHttpServer`
//!   boilerplate inlined in a private `serve()` helper so `main` stays ≤15
//!   statement lines (mirror Plan 86-02 Pitfall §2). This is the same wiring shape
//!   as the Plan 06 `examples/openapi_server_min.rs`, but actually serves.
//! - `config.toml` — `[backend]` + ONE single-call AND ONE script `[[tools]]`
//!   (engine-accurate JS subset, Plan 90-05), `[code_mode] enabled = true` with an
//!   inline DEV `token_secret` + `allow_inline_token_secret_for_dev = true` and a
//!   LOUD "replace for production" note (CF-4).
//! - `api.yaml` — a minimal OpenAPI spec (the scaffold-discovery story, D-03).
//! - `deploy.toml` + `.pmcp/deploy.toml` — `[target] type = "pmcp-run"` (CF-6;
//!   `get_target_id` has no shape inference, so the target MUST be declared here).

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Emit the files of a single runnable `openapi-server` crate into `dir`.
///
/// Beyond the runnable-crate files (`Cargo.toml`, `src/main.rs`, `config.toml`,
/// `api.yaml`) this also emits the deploy descriptor (`deploy.toml` +
/// `.pmcp/deploy.toml`, `target_type = "pmcp-run"`) so `cargo pmcp deploy`
/// selects pmcp.run with the Phase 77 target enum UNCHANGED (CF-6).
pub fn generate(dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(dir, name)?;
    generate_main_rs(dir)?;
    generate_config_toml(dir, name)?;
    generate_api_yaml(dir)?;
    generate_deploy_toml(dir, name)?;

    if std::env::var("PMCP_QUIET").is_err() {
        println!("  {} Generated OpenAPI server crate files", "✓".green());
    }
    Ok(())
}

fn generate_cargo_toml(dir: &Path, name: &str) -> Result<()> {
    // A6 published-version coupling: these versions track the to-be-published
    // toolkit. Until `pmcp-server-toolkit 0.1.0` lands on crates.io a local
    // `[patch.crates-io]` is needed for `cargo run` against an unpublished build.
    //
    // The `openapi-code-mode` umbrella feature composes `http` + `code-mode` +
    // `pmcp-code-mode/js-runtime`; the bare `["code-mode","http"]` pair does NOT
    // forward `pmcp-code-mode/js-runtime`, so the script-tool / Code-Mode path
    // would compile out (RESEARCH Pitfall 4). Mirrors the Plan 90-06 binary dep.
    let content = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "2.8.1", features = ["streamable-http"] }}
pmcp-server-toolkit = {{ version = "0.1.0", features = ["openapi-code-mode"] }}
# The OpenAPI assemble orchestrators (`dispatch` builds the (HttpConnector,
# HttpCodeExecutor) pair; `build_server` assembles the pmcp::Server). Unlike the
# SQL path there is no ServerBuilderExt http method, so this lib owns the seam.
pmcp-openapi-server = "0.1.0"
clap = {{ version = "4", features = ["derive", "env"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
"#,
    );

    fs::write(dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;
    Ok(())
}

/// The emitted `src/main.rs`. This is the ≤15-line Shape C wiring (CF-5): the
/// same shape as the Plan 06 `examples/openapi_server_min.rs`, but with a private
/// `serve()` helper that inlines the `StreamableHttpServer` boilerplate so `main`
/// stays ≤15 statement lines (Plan 86-02 Pitfall §2) AND actually serves. The
/// golden-drift test `emitted_main_is_le_15_statement_lines` enforces the line
/// budget.
fn emitted_main_rs() -> &'static str {
    r#"//! Config-driven OpenAPI MCP server (streamable HTTP).
//!
//! Generated by `cargo pmcp new --kind openapi-server`. Reads `config.toml` (the
//! `[backend]` / `[[tools]]` / `[code_mode]` declarations) and an OPTIONAL
//! `api.yaml` OpenAPI spec, then assembles + serves a `pmcp::Server` over
//! streamable HTTP — no Rust required to change behaviour, just edit the config.

use std::net::SocketAddr;
use std::sync::Arc;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use pmcp_openapi_server::{build_server, dispatch};
use pmcp_server_toolkit::http::OpenApiSchema;
use pmcp_server_toolkit::ServerConfig;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Inline streamable-HTTP serve helper: collapses `with_config` + `start` into
/// one call while inlining the `StreamableHttpServer` body (so `main` stays
/// ≤15 statement lines).
async fn serve(server: Server) -> Result<(SocketAddr, JoinHandle<()>), Box<dyn std::error::Error>> {
    let shared = Arc::new(Mutex::new(server));
    let cfg = StreamableHttpServerConfig::default();
    Ok(
        StreamableHttpServer::with_config("127.0.0.1:8080".parse()?, shared, cfg)
            .start()
            .await?,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(&std::fs::read_to_string("config.toml")?)?;
    let spec = std::fs::read_to_string("api.yaml")
        .ok()
        .map(|s| OpenApiSchema::parse(&s))
        .transpose()?; // api.yaml is OPTIONAL (D-03): curated-only boots without it
    let (connector, http_exec) = dispatch(&cfg).await?;
    let server = build_server(&cfg, connector, http_exec, spec)?;
    let (addr, handle) = serve(server).await?;
    println!("PMCP_OPENAPI_SERVER_ADDR=http://{addr}"); // machine-readable bound addr
    handle.await?;
    Ok(())
}
"#
}

fn generate_main_rs(dir: &Path) -> Result<()> {
    fs::write(dir.join("src").join("main.rs"), emitted_main_rs())
        .context("Failed to create src/main.rs")?;
    Ok(())
}

fn generate_config_toml(dir: &Path, name: &str) -> Result<()> {
    // Emit ONLY `deny_unknown_fields`-known keys (any typo is a hard parse error
    // at the consumer). ONE single-call tool + ONE script tool (engine-accurate JS
    // subset, Plan 90-05: `api.get` paths are string/template literals, the result
    // binds to a const before `return`).
    let content = format!(
        r#"# Config-driven OpenAPI MCP server. Parses through `from_toml_strict_validated`
# (`#[serde(deny_unknown_fields)]`); every key below is a known toolkit field.
[server]
name = "{name}"
version = "0.1.0"

# The backend the tools call. `base_url` is the REST API root; `api.yaml` (the
# OPTIONAL OpenAPI spec) is exposed as the `api_schema` resource for Code Mode.
[backend]
base_url = "https://api.example.com"

[backend.http]
timeout_seconds = 30
retries = 3
retry_backoff_ms = 1000

# A single-call tool: maps 1:1 onto one HTTP request (GET /widgets).
[[tools]]
name = "list_widgets"
description = "List widgets from the backend API"
path = "/widgets"
method = "GET"

[tools.annotations]
read_only_hint = true
idempotent_hint = true

# A script tool: a multi-call orchestration in the engine-accurate JS subset
# (Plan 90-05). `api.get` paths are string/template literals and the result binds
# to a const before `return`.
[[tools]]
name = "widget_with_detail"
description = "Fetch a widget and its detail in one orchestrated call"
script = """
const widget = await api.get(`/widgets/${{args.id}}`);
const detail = await api.get(`/widgets/${{args.id}}/detail`);
const out = {{ widget: widget, detail: detail }};
return out;
"""

[[tools.parameters]]
name = "id"
type = "string"
description = "The widget id to fetch"
required = true

[tools.annotations]
read_only_hint = true

[code_mode]
# code-mode (validate_code / execute_code) is visible on first run; it runs JS
# against the backend through the SAME HTTP engine the script tool uses.
enabled = true
# DEV ONLY — replace with a secrets ref (token_secret = "env:CODE_MODE_SECRET").
# When you do, ALSO declare it, or `cargo pmcp package save` refuses the config:
#   [[config_slots]]
#   key = "code_mode.token_secret"
#   kind = "secret"
#   name = "CODE_MODE_SECRET"
# for production. The deploy path (cargo pmcp deploy) substitutes a secrets ref
# automatically. The inline literal below is rejected unless the dev flag is set.
token_secret = "dev-only-insecure-secret-min-16-bytes"
allow_inline_token_secret_for_dev = true
"#,
    );

    fs::write(dir.join("config.toml"), content).context("Failed to create config.toml")?;
    Ok(())
}

fn generate_api_yaml(dir: &Path) -> Result<()> {
    // A minimal OpenAPI 3 spec (D-03: the spec is OPTIONAL at runtime, but ship one
    // for the scaffold-discovery story — it becomes the `api_schema` resource Code
    // Mode reads). The two paths mirror the script tool's `api.get` calls.
    let content = r#"openapi: 3.0.3
info:
  title: Example Widget API
  version: 0.1.0
  description: >
    Minimal scaffold OpenAPI spec. Replace with your real API description; it is
    exposed as the `api_schema` resource so Code Mode can discover endpoints.
servers:
  - url: https://api.example.com
paths:
  /widgets:
    get:
      operationId: listWidgets
      summary: List widgets
      responses:
        "200":
          description: A list of widgets
  /widgets/{id}:
    get:
      operationId: getWidget
      summary: Get one widget by id
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: A single widget
  /widgets/{id}/detail:
    get:
      operationId: getWidgetDetail
      summary: Get a widget's detail by id
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: A widget's detail
"#;

    fs::write(dir.join("api.yaml"), content).context("Failed to create api.yaml")?;
    Ok(())
}

/// Emit the deploy descriptor that drives `cargo pmcp deploy` (CF-6).
///
/// The file is written to BOTH `deploy.toml` (project root — human-visible) and
/// `.pmcp/deploy.toml` (the path [`crate::deployment::config::DeployConfig::load`]
/// reads). `target_type` resolves to pmcp.run via `[target] type = "pmcp-run"`
/// (CF-6) — `get_target_id` does NOT infer pmcp.run from project shape, so the
/// target MUST be declared here. The Phase 77 target enum is UNCHANGED.
fn generate_deploy_toml(dir: &Path, name: &str) -> Result<()> {
    let content = format!(
        r#"# Deploy descriptor for `cargo pmcp deploy` (config-driven single-crate OpenAPI
# server).
#
# Deploy posture:
# - Assets (config.toml + api.yaml) are bundled and read at runtime.
# - SECRET: the local `config.toml` carries an inline DEV `token_secret` for
#   out-of-box `cargo run`. The deploy path substitutes `${{CODE_MODE_SECRET}}`
#   into the BUNDLED config so the deployed artifact NEVER ships the dev literal —
#   supply `CODE_MODE_SECRET` as a deploy secret/env on pmcp.run.

[target]
# CF-6: pmcp.run is selected by THIS value (target_type → get_target_id reads
# target.type); it is NOT inferred from project shape, and the Phase 77 target
# enum is UNCHANGED.
type = "pmcp-run"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "{name}"
memory_mb = 512
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[auth]
enabled = false
provider = "none"

[observability]
log_retention_days = 30
enable_xray = false
create_dashboard = false

[assets]
# Bundle the config + spec so the deployed server reads the same inputs.
include = ["config.toml", "api.yaml"]
"#,
    );

    fs::write(dir.join("deploy.toml"), &content).context("Failed to create deploy.toml")?;

    // DeployConfig::load reads `<project_root>/.pmcp/deploy.toml`; emit a copy there
    // so `cargo pmcp deploy` (and the Task 2 scaffold test) load the same descriptor.
    let pmcp_dir = dir.join(".pmcp");
    fs::create_dir_all(&pmcp_dir).context("Failed to create .pmcp directory")?;
    fs::write(pmcp_dir.join("deploy.toml"), &content)
        .context("Failed to create .pmcp/deploy.toml")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count the load-bearing STATEMENT lines of `main` (CF-5 ≤15). Mirrors the
    /// Plan 86-02 counting rule: skip blank lines, comments, and rustfmt
    /// method-chain / closing-delimiter continuations (a wrapped statement counts
    /// once). The `serve()` helper is hoisted OUT of `main`, so only the `main`
    /// body is counted.
    fn main_statement_lines(src: &str) -> usize {
        // Isolate the `#[tokio::main] async fn main` body.
        let main_start = src
            .find("async fn main(")
            .expect("emitted main.rs must define `async fn main`");
        let body = &src[main_start..];
        body.lines()
            .skip(1) // the `async fn main(...)` signature line
            .take_while(|l| l.trim() != "}") // stop at main's closing brace
            .map(|l| match l.find("//") {
                Some(idx) => l[..idx].trim(),
                None => l.trim(),
            })
            .filter(|t| {
                if t.is_empty() {
                    return false;
                }
                if t.starts_with("//") {
                    return false;
                }
                // rustfmt continuations: a method-chain line ('.foo()') or a lone
                // closing delimiter counts with its owning statement, not separately.
                if t.starts_with('.') {
                    return false;
                }
                if matches!(*t, ")" | ");" | ")?;" | "}" | "};" | "]" | "],") {
                    return false;
                }
                true
            })
            .count()
    }

    #[test]
    fn emitted_main_is_le_15_statement_lines() {
        let n = main_statement_lines(emitted_main_rs());
        assert!(
            n <= 15,
            "emitted src/main.rs `main` body is {n} statement lines; CF-5 budget is ≤15"
        );
    }

    #[test]
    fn emitted_main_has_cf5_wiring_tokens() {
        // The ≤15-line wiring: load config[+optional spec] → dispatch → build →
        // serve. These exact tokens prove the Shape C shape (CF-5) and the D-03
        // optional-spec posture.
        let m = emitted_main_rs();
        let tokens = [
            "ServerConfig::from_toml_strict_validated",
            "read_to_string(\"config.toml\")",
            "read_to_string(\"api.yaml\")",
            "OpenApiSchema::parse",
            "dispatch(&cfg)",
            "build_server(",
            "serve(server)",
            "PMCP_OPENAPI_SERVER_ADDR",
        ];
        for tok in tokens {
            assert!(
                m.contains(tok),
                "emitted main.rs missing wiring token: {tok}"
            );
        }
        // D-03: api.yaml is loaded with `.ok()` so a missing spec boots curated-only.
        assert!(
            m.contains("read_to_string(\"api.yaml\")\n        .ok()"),
            "emitted main.rs must load api.yaml optionally (.ok()) — D-03 curated-only boot"
        );
    }

    #[test]
    fn emitted_config_is_code_mode_enabled_with_dev_secret_note() {
        // CF-4: code_mode enabled + inline DEV secret guarded by the dev flag + a
        // LOUD replace-for-production note. Render into a tempdir and read back.
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        generate(tmp.path(), "scaffold_openapi_demo").expect("generate scaffold");

        let config = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
        assert!(
            config.contains("enabled = true"),
            "code_mode must be enabled"
        );
        assert!(
            config.contains("allow_inline_token_secret_for_dev = true"),
            "config must set the dev inline-secret flag (CF-4)"
        );
        assert!(
            config.to_lowercase().contains("replace")
                && config.to_lowercase().contains("production"),
            "config must carry a LOUD replace-for-production note (CF-4)"
        );
    }

    #[test]
    fn emitted_deploy_toml_targets_pmcp_run() {
        // CF-6: scaffolded deploy.toml selects pmcp.run; emitted to both root and
        // .pmcp/. The Phase 77 target enum is unchanged (detection-based).
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        generate(tmp.path(), "scaffold_openapi_demo").expect("generate scaffold");

        for p in ["deploy.toml", ".pmcp/deploy.toml"] {
            let deploy = fs::read_to_string(tmp.path().join(p)).unwrap();
            assert!(
                deploy.contains(r#"type = "pmcp-run""#),
                "{p} must declare target type = \"pmcp-run\" (CF-6)"
            );
        }
    }

    #[test]
    fn emitted_cargo_toml_uses_openapi_code_mode_umbrella() {
        // The toolkit dep MUST use the `openapi-code-mode` umbrella so the
        // script-tool / Code-Mode engine compiles in (RESEARCH Pitfall 4).
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        generate(tmp.path(), "scaffold_openapi_demo").expect("generate scaffold");

        let cargo = fs::read_to_string(tmp.path().join("Cargo.toml")).unwrap();
        assert!(
            cargo.contains("openapi-code-mode"),
            "Cargo.toml must enable pmcp-server-toolkit with the openapi-code-mode umbrella"
        );
    }
}
