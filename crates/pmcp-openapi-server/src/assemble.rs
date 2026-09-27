//! Server assembly: `ServerConfig` + `(connector, http_exec)` + optional spec →
//! built [`pmcp::Server`].
//!
//! This module is the heart of the Shape A OpenAPI binary's "no Rust required"
//! promise. It takes the parsed config, the dispatched
//! `(HttpConnector, HttpCodeExecutor)` pair, and the OPTIONAL parsed
//! [`OpenApiSchema`] and assembles a complete [`pmcp::Server`] — curated
//! single-call `[[tools]]`, admin-authored script tools, the Code-Mode
//! `validate_code` / `execute_code` pair, the configured resources/prompts, and
//! (when a spec is supplied) the `api_schema` resource — through the
//! `pmcp-server-toolkit` builder chain alone.
//!
//! # Pitfall 6: toolkit assemble, NOT the reference server builder
//!
//! This deliberately does NOT lift the reference pmcp-run server builder. It
//! assembles via the toolkit's
//! [`synthesize_from_config_with_http_connector_and_scripts`] (Plans 03/05) +
//! [`code_mode_tools_from_executor`] (Plan 04), registering the synthesized
//! handlers via `tool_arc` (the same builder-chain pattern `pmcp-sql-server`'s
//! `assemble.rs` uses through its `ServerBuilderExt` connector methods).
//!
//! # H1 — inbound token capture for `oauth_passthrough`
//!
//! A [`TokenCaptureAuthProvider`] (lifted CONCEPT from the reference
//! `pmcp_server.rs:37-64`, NOT the whole file — Pitfall 6) is installed on the
//! builder so the inbound MCP client `Authorization` header lands in the
//! [`AuthContext`] threaded through `pmcp::RequestHandlerExtra`. The toolkit's
//! Code-Mode / script-tool handlers then read that captured token from `extra`
//! and re-derive a per-request executor via
//! `request_executor_from_extra` → [`HttpCodeExecutor::with_inbound_token`], so
//! the outbound `oauth_passthrough` provider (Plan 90-10) forwards it to the
//! backend. The per-request derivation now lives in the TOOLKIT (the handlers
//! live there; the binary cannot be a dependency of the toolkit), so the binary
//! wires the OpenAPI Code-Mode path via
//! [`code_mode_http_tools_from_executor`](pmcp_server_toolkit::code_mode::code_mode_http_tools_from_executor)
//! — the dead binary `request_executor` (WR-01) was removed.
//!
//! # No-spec + Code-Mode behavior (D-03)
//!
//! When `[code_mode] enabled = true` and NO spec is supplied, Code Mode still
//! RUNS but WITHOUT the `api_schema` resource — a `tracing::warn!` is emitted and
//! assembly proceeds (it does NOT fail and does NOT silently drop Code Mode).
//! When a spec IS supplied it is merged as the `api_schema` resource.

use std::sync::Arc;

// SINGLE crate-root toolkit import (the binding witness of the DX promise): the
// builder-chain symbols resolve from `pmcp_server_toolkit::*`. The HTTP/OpenAPI
// assemble path uses the free `synthesize_*` + `code_mode_tools_from_executor`
// functions (the http analog of the SQL `ServerBuilderExt` connector methods —
// there is no `ServerBuilderExt::*_with_http_connector` method), registering the
// synthesized handlers via `tool_arc` exactly as the SQL ext methods do.
use pmcp_server_toolkit::{ServerConfig, StaticPromptHandler, StaticResourceHandler};

use pmcp_server_toolkit::code_mode::{
    code_mode_http_tools_from_executor, ExecutionConfig, HttpCodeExecutor, ValidationFlavor,
};
use pmcp_server_toolkit::http::{HttpConnector, OpenApiSchema};
use pmcp_server_toolkit::prompts::PromptConfig;
use pmcp_server_toolkit::resources::ResourceConfig;
use pmcp_server_toolkit::synthesize_from_config_with_http_connector_and_scripts;

use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::Server;

/// URI of the OpenAPI contract resource served from `--spec` (D-03 surface (a)).
const API_SCHEMA_URI: &str = "api_schema";

/// Suffix matched to override an existing schema resource with the `--spec` body.
const SCHEMA_URI_SUFFIX: &str = "/schema";

/// Error assembling a [`pmcp::Server`] from config + pair + spec.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssembleError {
    /// A toolkit builder step (tool synthesis, code-mode wiring, resource
    /// loading) failed. Carries the toolkit's own error for diagnostics.
    #[error("toolkit assembly step failed: {0}")]
    Toolkit(#[from] pmcp_server_toolkit::ToolkitError),

    /// The final `pmcp::Server::builder().build()` failed. Carries the pmcp error.
    #[error("pmcp server build failed: {0}")]
    Build(#[from] pmcp::Error),
}

/// Inbound auth provider that captures the MCP client `Authorization` header into
/// [`AuthContext::token`] so the per-request `oauth_passthrough` forward (H1) can
/// read it from [`RequestHandlerExtra`].
///
/// Concept lifted from the reference `pmcp_server.rs:37-64` (NOT the whole file —
/// Pitfall 6). `is_required()` is `false`: capture is best-effort so a curated /
/// static-auth server still serves unauthenticated clients.
struct TokenCaptureAuthProvider;

#[pmcp::async_trait]
impl AuthProvider for TokenCaptureAuthProvider {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> pmcp::Result<Option<AuthContext>> {
        Ok(Some(AuthContext {
            subject: "proxy-authenticated".to_string(),
            scopes: vec![],
            claims: std::collections::HashMap::new(),
            token: authorization_header.map(str::to_string),
            client_id: None,
            expires_at: None,
            authenticated: authorization_header.is_some(),
        }))
    }

    fn is_required(&self) -> bool {
        false
    }
}

/// Build the [`ExecutionConfig`] bounds for the shared engine from
/// `[code_mode.limits]`, falling back to the defaults (`max_api_calls=50`,
/// `max_loop_iterations=100`, `timeout_seconds=30`).
fn execution_config(cfg: &ServerConfig) -> ExecutionConfig {
    let mut exec = ExecutionConfig::default();
    if let Some(limits) = cfg.code_mode.as_ref().and_then(|cm| cm.limits.as_ref()) {
        if let Some(n) = limits.max_tables_per_query {
            // `max_tables_per_query` is the closest SQL-shaped bound; for OpenAPI
            // we map the configured complexity ceiling onto max_api_calls so an
            // operator who tightens limits also tightens the per-script API
            // budget. Defaults stay when unset.
            exec.max_api_calls = n as usize;
        }
        if let Some(n) = limits.max_join_depth {
            exec.max_loop_iterations = n as usize;
        }
    }
    exec
}

/// Clone the configured `[[resources]]`, optionally appending/overriding the
/// `api_schema` resource with the `--spec` body (D-03 surface (a)).
///
/// When `spec` is `Some`, its text becomes the `api_schema` resource: an existing
/// resource whose URI is exactly `api_schema` or ends with `/schema` is
/// overridden; otherwise one is appended. When `spec` is `None`, the configured
/// resources pass through unchanged (no `api_schema` is synthesized).
fn merge_spec_resource(cfg: &ServerConfig, spec: Option<&OpenApiSchema>) -> Vec<ResourceConfig> {
    let mut found = false;
    let mut configs: Vec<ResourceConfig> = cfg
        .resources
        .iter()
        .map(|r| {
            let is_schema = spec.is_some()
                && !found
                && (r.uri == API_SCHEMA_URI || r.uri.ends_with(SCHEMA_URI_SUFFIX));
            if is_schema {
                found = true;
            }
            ResourceConfig {
                uri: r.uri.clone(),
                name: r.name.clone().unwrap_or_else(|| r.uri.clone()),
                description: r.description.clone(),
                mime_type: r
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "text/markdown".to_string()),
                content: if is_schema {
                    spec.map(|s| s.spec_text().to_string())
                } else {
                    Some(r.content.clone().unwrap_or_default())
                },
                content_file: None,
                meta: None,
            }
        })
        .collect();

    if let (Some(s), false) = (spec, found) {
        configs.push(ResourceConfig {
            uri: API_SCHEMA_URI.to_string(),
            name: "OpenAPI Schema".to_string(),
            description: Some("The OpenAPI contract for code-mode script generation".to_string()),
            mime_type: "application/yaml".to_string(),
            content: Some(s.spec_text().to_string()),
            content_file: None,
            meta: None,
        });
    }

    configs
}

/// Map the parsed `[[prompts]]` declarations onto the toolkit's [`PromptConfig`].
fn prompt_configs(cfg: &ServerConfig) -> Vec<PromptConfig> {
    cfg.prompts
        .iter()
        .map(|p| PromptConfig {
            name: p.name.clone(),
            description: p.description.clone().unwrap_or_default(),
            include_resources: p.include_resources.clone(),
        })
        .collect()
}

/// Register every configured prompt on `builder`, resolving each prompt's
/// `include_resources` against the merged resource handler.
fn register_prompts(
    mut builder: pmcp::ServerBuilder,
    cfg: &ServerConfig,
    resources: &StaticResourceHandler,
) -> pmcp::ServerBuilder {
    for (name, handler) in StaticPromptHandler::from_configs(&prompt_configs(cfg), resources) {
        builder = builder.prompt_arc(name, Arc::new(handler));
    }
    builder
}

/// Assemble a [`pmcp::Server`] from config + the dispatched pair + the optional
/// parsed spec.
///
/// Wires, in order:
/// 1. Single-call `[[tools]]` + admin-authored script tools via
///    [`synthesize_from_config_with_http_connector_and_scripts`] (Plans 03/05) —
///    the synthesizer routes `is_script_tool()` to a `ScriptToolHandler` over the
///    SAME `http_exec`.
/// 2. Code Mode `validate_code` + `execute_code` via
///    [`code_mode_http_tools_from_executor`](pmcp_server_toolkit::code_mode::code_mode_http_tools_from_executor)
///    (Plan 90-10) over the SAME `http_exec` (D-02: one engine), with
///    [`ValidationFlavor::OpenApi`] (real SWC-backed JS validation). The handler
///    re-derives a request-scoped `JsCodeExecutor` per call so the captured
///    inbound `oauth_passthrough` token reaches the backend (OAPI-03/OAPI-05).
/// 3. The configured resources, with the `api_schema` resource merged from
///    `spec` when supplied (D-03).
/// 4. The configured prompts, resolved against the merged resources.
/// 5. The inbound [`TokenCaptureAuthProvider`] (H1).
///
/// # No-spec + Code-Mode behavior (D-03)
///
/// When `[code_mode] enabled = true` and `spec.is_none()`, Code Mode RUNS but
/// without the `api_schema` resource — a `tracing::warn!` is emitted and assembly
/// proceeds.
///
/// # Errors
///
/// [`AssembleError::Toolkit`] if a toolkit step fails (e.g. tool synthesis or a
/// `token_secret` resolution error) or [`AssembleError::Build`] if the final
/// `pmcp::Server` build fails.
/// STUB — the RED half of Phase 128 D4(b)'s wiring. Returns the executor
/// unchanged, ignoring the spec, so `narrowed_executor_attaches_a_supplied_spec`
/// fails on an assertion rather than on a missing function.
fn narrowed_executor(
    http_exec: HttpCodeExecutor,
    spec: Option<&Arc<OpenApiSchema>>,
) -> HttpCodeExecutor {
    let _ = spec;
    http_exec
}

pub fn build_server(
    cfg: &ServerConfig,
    connector: Arc<dyn HttpConnector>,
    http_exec: HttpCodeExecutor,
    spec: Option<OpenApiSchema>,
) -> Result<Server, AssembleError> {
    let exec_config = execution_config(cfg);

    // D-03: define the no-spec + code-mode behavior — warn and proceed (Code Mode
    // runs without the api_schema contract resource).
    let code_mode_on = cfg.code_mode.as_ref().is_some_and(|cm| cm.enabled);
    if code_mode_on && spec.is_none() {
        tracing::warn!(
            target: "pmcp_openapi_server",
            "code_mode enabled without --spec: the api_schema resource is unavailable; \
             the LLM will generate long-tail scripts without the OpenAPI contract"
        );
    }

    // ONE `Arc<OpenApiSchema>`, shared between the `api_schema` resource below and
    // the executor's D4(b) narrowing. The parsed document is retained verbatim and
    // can be large; cloning it to give the executor its own copy would double that
    // allocation for no benefit.
    let spec = spec.map(Arc::new);

    let resources =
        StaticResourceHandler::from_configs(&merge_spec_resource(cfg, spec.as_deref()))?;

    // Phase 128 D4(b) — the executor gets the spec HERE, and the position is
    // load-bearing rather than incidental.
    //
    // Both HTTP surfaces run on ONE executor (D-02): the synthesizer at (1) takes a
    // CLONE and Code Mode at (2) takes the original. `HttpCodeExecutor` is a
    // cheap-clone value type, so a clone taken before `with_schema` would be
    // permanently unnarrowed — the narrowing has to be attached before EITHER
    // fan-out, which is why this statement precedes both. `build_server` is also the
    // only place the executor and the parsed spec are both in scope: `dispatch.rs`
    // constructs the executor with no spec available, and the two `#[cfg(test)]`
    // helpers that look like candidates would leave the production binary
    // unnarrowed while every test passed (T-128-36c).
    let http_exec = narrowed_executor(http_exec, spec.as_ref());

    // The CONFIGURED half of the template-spelling guard (T-128-36a). A `[[tools]]`
    // `(method, path)` the spec does not declare keeps the unconditional floor and
    // the always-on cap but loses the spec's narrowing, and that is an author error
    // the operator can fix before deploy — so it is reported once, at startup, in
    // the deploy log. Its bound is stated on `lint_against_spec`: a template a Code
    // Mode script composes at RUNTIME is invisible at config time and is reported
    // instead by the toolkit's once-per-pair debug log.
    if let Some(schema) = spec.as_deref() {
        for finding in cfg.lint_against_spec(schema) {
            tracing::warn!(target: "pmcp_openapi_server", "{finding}");
        }
    }

    // 1. Single-call + script tools over the shared connector + http_exec.
    let synthesized = synthesize_from_config_with_http_connector_and_scripts(
        cfg,
        connector,
        http_exec.clone(),
        exec_config.clone(),
    )?;
    let mut builder = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version);
    for (name, _info, handler) in synthesized {
        builder = builder.tool_arc(name, handler);
    }

    // 2. Code Mode over the SAME http_exec (D-02). The per-request entry point
    //    (Plan 90-10) hands the handler the base HttpCodeExecutor + exec_config
    //    so it re-derives a request-scoped JsCodeExecutor carrying the captured
    //    inbound oauth_passthrough token (OAPI-03/OAPI-05). The synthesizer above
    //    took its own clone; code-mode takes the original http_exec.
    builder = code_mode_http_tools_from_executor(
        builder,
        cfg,
        http_exec,
        exec_config,
        ValidationFlavor::OpenApi,
    )?;

    // 3 + 4. Prompts resolved against the merged resources, then the resources.
    let builder = register_prompts(builder, cfg, &resources);
    let builder = builder.resources_arc(Arc::new(resources));

    // 5. Inbound token capture (H1).
    let server = builder.auth_provider(TokenCaptureAuthProvider).build()?;
    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::{build_server, merge_spec_resource, AssembleError, API_SCHEMA_URI};
    use pmcp_server_toolkit::config::ServerConfig;
    use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
    use pmcp_server_toolkit::http::OpenApiSchema;

    fn curated_only_cfg() -> ServerConfig {
        let toml = r#"
[server]
name = "tube"
version = "0.1.0"

[backend]
base_url = "https://api.tfl.gov.uk"

[code_mode]
enabled = true
token_secret = "${OPENAPI_ASSEMBLE_SECRET}"

[[tools]]
name = "get_line_status"
description = "Status for a tube line"
path = "/Line/{id}/Status"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
required = true
"#;
        ServerConfig::from_toml_strict_validated(toml).expect("parse")
    }

    fn http_exec() -> pmcp_server_toolkit::code_mode::HttpCodeExecutor {
        let auth = create_auth_provider(&AuthConfig::None).expect("auth");
        pmcp_server_toolkit::code_mode::HttpCodeExecutor::new(
            reqwest::Client::new(),
            "https://api.tfl.gov.uk".to_string(),
            auth,
        )
    }

    fn connector() -> std::sync::Arc<dyn pmcp_server_toolkit::http::HttpConnector> {
        let auth = create_auth_provider(&AuthConfig::None).expect("auth");
        std::sync::Arc::new(
            pmcp_server_toolkit::http::HttpClient::new(
                reqwest::Client::new(),
                "https://api.tfl.gov.uk".to_string(),
                auth,
            )
            .expect("connector"),
        )
    }

    #[test]
    fn no_spec_code_mode_warns_and_still_builds() -> Result<(), AssembleError> {
        // D-03: code_mode enabled + no spec must warn-and-proceed (server builds,
        // Code Mode tools register, no api_schema resource).
        std::env::set_var("OPENAPI_ASSEMBLE_SECRET", "assemble-secret-min-16-bytes");
        let cfg = curated_only_cfg();
        let server = build_server(&cfg, connector(), http_exec(), None)?;
        // The server builds without a spec — D-03 curated/no-spec boot proof.
        drop(server);
        Ok(())
    }

    #[test]
    fn spec_is_merged_as_api_schema_resource() {
        let cfg = curated_only_cfg();
        let spec = OpenApiSchema::parse(
            r#"{"openapi":"3.0.0","info":{"title":"t","version":"1"},"paths":{}}"#,
        )
        .expect("parse spec");
        let merged = merge_spec_resource(&cfg, Some(&spec));
        let api = merged
            .iter()
            .find(|r| r.uri == API_SCHEMA_URI)
            .expect("api_schema resource present when spec supplied");
        assert!(
            api.content.as_deref().unwrap().contains("openapi"),
            "api_schema carries the spec text"
        );
    }

    #[test]
    fn no_spec_does_not_synthesize_api_schema() {
        let cfg = curated_only_cfg();
        let merged = merge_spec_resource(&cfg, None);
        assert!(
            !merged.iter().any(|r| r.uri == API_SCHEMA_URI),
            "no api_schema resource without a spec (D-03)"
        );
    }

    // NOTE: the per-request executor derivation that was tested here as the
    // binary's `request_executor` moved into the toolkit as
    // `request_executor_from_extra` (Plan 90-10 / WR-01 — the binary helper was
    // dead). Its unit coverage now lives in the toolkit
    // (`code_mode::per_request_executor_tests`), and the end-to-end
    // handler-path forwarding proof is `tests/oauth_passthrough_e2e.rs`.

    /// A spec declaring a NARROW pattern on the curated tool's own path, so the
    /// wiring's effect is observable rather than merely present.
    const NARROWING_SPEC: &str = r#"{
      "openapi": "3.0.0",
      "info": { "title": "tube", "version": "1" },
      "paths": {
        "/Line/{id}/Status": {
          "get": {
            "operationId": "getLineStatus",
            "parameters": [
              { "name": "id", "in": "path", "required": true,
                "schema": { "type": "string", "pattern": "^[a-z]+$" } }
            ],
            "responses": { "200": { "description": "ok" } }
          }
        }
      }
    }"#;

    /// T-128-36c — the wiring must attach a supplied spec to the executor the
    /// binary actually serves with. Asserted through the toolkit's public
    /// `has_schema()` because the wiring lives in THIS crate while the field lives
    /// in the toolkit, so a `#[cfg(test)]` accessor could not reach it.
    #[test]
    fn narrowed_executor_attaches_a_supplied_spec() {
        let spec = std::sync::Arc::new(OpenApiSchema::parse(NARROWING_SPEC).expect("parses"));
        assert!(
            !http_exec().has_schema(),
            "the constructor must leave the schema absent, or this row proves nothing"
        );
        assert!(
            super::narrowed_executor(http_exec(), Some(&spec)).has_schema(),
            "a supplied spec must reach the executor"
        );
    }

    /// The spec-less deployment stays supported: no spec, no schema, and the floor
    /// plus the cap are what still protect it.
    #[test]
    fn narrowed_executor_leaves_a_spec_less_executor_alone() {
        assert!(!super::narrowed_executor(http_exec(), None).has_schema());
    }

    /// ONE `Arc` is shared rather than the document being cloned: the executor's
    /// copy and the caller's are the same allocation.
    #[test]
    fn narrowed_executor_shares_one_arc() {
        let spec = std::sync::Arc::new(OpenApiSchema::parse(NARROWING_SPEC).expect("parses"));
        let before = std::sync::Arc::strong_count(&spec);
        let exec = super::narrowed_executor(http_exec(), Some(&spec));
        assert_eq!(
            std::sync::Arc::strong_count(&spec),
            before + 1,
            "the executor must hold a clone of the Arc, not a clone of the document"
        );
        drop(exec);
        assert_eq!(std::sync::Arc::strong_count(&spec), before);
    }

    /// `build_server` with a spec still builds and registers both tool families —
    /// the narrowing is additive, not a new failure mode.
    #[test]
    fn build_server_with_a_spec_builds_and_registers_tools() -> Result<(), AssembleError> {
        std::env::set_var("OPENAPI_ASSEMBLE_SECRET", "s3cr3t-assemble-narrowing");
        let cfg = curated_only_cfg();
        let spec = OpenApiSchema::parse(NARROWING_SPEC).expect("parses");
        let server = build_server(&cfg, connector(), http_exec(), Some(spec))?;
        assert!(server.get_tool("get_line_status").is_some());
        assert!(server.get_tool("execute_code").is_some());
        std::env::remove_var("OPENAPI_ASSEMBLE_SECRET");
        Ok(())
    }
}
