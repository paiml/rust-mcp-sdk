// Net-new code for Phase 83 TKIT-06 / TKIT-09 (code-mode wiring surface).
//
// Bridges `[code_mode]` config blocks into pmcp-code-mode's `ValidationPipeline`
// + HMAC token machinery, with every public type RE-EXPORTED from pmcp-code-mode
// per D-16 (NO duplicate HMAC / token code per PATTERNS §"Anti-Patterns" #2).
//
// Per Phase 83 review R1, the preflight at
// `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md`
// determined the wiring strategy: **R1 split** —
// `validation_pipeline_from_config(&ServerConfig) -> Result<ValidationPipeline>`
// + `code_mode_tools_from_executor(executor, config) -> Result<...>` — because
// `pmcp-code-mode`'s `CodeExecutor` trait requires backend injection
// (`HttpExecutor`, `SdkExecutor`, `McpExecutor`) and no config-only constructor
// exists.

//! Code-mode wiring: bridges `[code_mode]` config blocks into pmcp-code-mode's
//! validation pipeline + HMAC token machinery, with policy / executor /
//! validation types re-exported verbatim (NO duplicate impl per RESEARCH
//! §"Anti-Patterns" #2).
//!
//! # R1 split (per `CODE_MODE_API_NOTES.md` Section 6)
//!
//! - [`validation_pipeline_from_config`] builds a [`ValidationPipeline`] from a
//!   parsed [`crate::config::ServerConfig`]. This is the entry point Shape A /
//!   Shape C consumers reach for — no per-server Rust glue needed.
//! - [`code_mode_tools_from_executor`] composes a caller-supplied
//!   [`CodeExecutor`] (Plan 08 wires this into `pmcp::ServerBuilder` via
//!   `code_mode_from_config`).
//! - [`register_code_mode_tools`] is the tolerant builder-extension entry
//!   point: a no-op when `[code_mode]` is absent, an R9 enforcement gate when
//!   present.
//!
//! # Security invariants (R6 + R9)
//!
//! - **R6 — toolkit-owned secret type.** `token_secret` resolution flows
//!   through [`crate::secrets::SecretValue`] (feature-independent) and
//!   converts to [`TokenSecret`] via `From` only at the HMAC boundary. This
//!   keeps `--no-default-features` stable.
//! - **R9 — inline-secret rejection.** A `[code_mode] token_secret = "raw"`
//!   literal is REJECTED at validation/resolve time unless the operator
//!   explicitly sets `allow_inline_token_secret_for_dev = true`. Default-deny;
//!   warnings are not protection.

#![cfg(feature = "code-mode")]

// === Re-exports (TKIT-06 + D-16) ===
//
// Every symbol below is a pure re-export of `pmcp_code_mode::*`. Plan 06 ships
// NO duplicate HMAC / token / policy / pipeline code (PATTERNS §"Anti-Patterns"
// #2 — duplicating these would create two copies of a security-critical
// invariant set).
//
// Symbols verified against `crates/pmcp-code-mode/src/lib.rs` per
// CODE_MODE_API_NOTES.md Section 7.

pub use pmcp_code_mode::{
    canonicalize_code, compute_context_hash, hash_code, ApprovalToken, AuthorizationDecision,
    CodeExecutor, CodeModeConfig, ExecutionError, HmacTokenGenerator, NoopPolicyEvaluator,
    PolicyEvaluator, TokenGenerator, TokenSecret, ValidationContext, ValidationPipeline,
};

#[cfg(feature = "avp")]
pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};

// OpenAPI / Code-Mode engine surface (Plan 90-04 / OAPI-05). Gated under the
// `openapi-code-mode` umbrella (which forwards `pmcp-code-mode/js-runtime`), so
// the bare `code-mode` (SQL-only) build does NOT pull the SWC JS engine
// (RESEARCH Pitfall 4). Re-exported here so the binary (Plan 06) + Plan 05
// reference ONE stable path for the engine types the OpenAPI flavor needs.
#[cfg(feature = "openapi-code-mode")]
pub use pmcp_code_mode::{ExecutionConfig, HttpExecutor, JsCodeExecutor};

use std::sync::Arc;

use crate::config::{CodeModeSection, ServerConfig};
use crate::error::{ConfigValidationError, Result, ToolkitError};
use crate::secrets::SecretValue;
use crate::sql::{Dialect, SqlConnector};

/// Which validation surface the generalized code-mode wiring drives (OAPI-10 /
/// D-02 / Gemini review: a compile-time enum, NOT a stringly-typed `&str`, so a
/// flavor typo is impossible).
///
/// Selects BOTH the `CodeModeToolBuilder` format string (the `validate_code` /
/// `execute_code` tool schema `format` enum, via the private `code_format`
/// accessor) AND which `ValidationPipeline` method `validate_code` calls:
/// - [`ValidationFlavor::Sql`] → the `sql` format + `validate_sql_query` (the
///   Shape A SQL path; unchanged behavior).
/// - [`ValidationFlavor::OpenApi`] → the `openapi` format +
///   `validate_javascript_code` (the OpenAPI JS path; really runs SWC-backed JS
///   validation, not a stub).
#[cfg(feature = "code-mode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationFlavor {
    /// SQL Code Mode — validate via `validate_sql_query`, `"sql"` tool format.
    Sql,
    /// OpenAPI Code Mode — validate via `validate_javascript_code`, `"openapi"`
    /// tool format. Available regardless of the JS engine feature at the type
    /// level; the OpenAPI `validate_code` path requires `openapi-code-mode`.
    OpenApi,
}

#[cfg(feature = "code-mode")]
impl ValidationFlavor {
    /// The `CodeModeToolBuilder` format string for this flavor.
    fn code_format(self) -> &'static str {
        match self {
            Self::Sql => "sql",
            Self::OpenApi => "openapi",
        }
    }
}

/// Derive a per-request [`HttpCodeExecutor`] from a [`pmcp::RequestHandlerExtra`]
/// by threading the captured inbound MCP client token (Plan 90-10 / OAPI-03 /
/// OAPI-05).
///
/// This is the **toolkit-resident** replacement for the binary's dead
/// `assemble.rs::request_executor` (WR-01 — the binary helper had NO runtime
/// callers because the handlers, which live in THIS crate, could not reach it
/// across the crate boundary). Both [`crate::tools`]'s `ScriptToolHandler` and
/// the OpenAPI [`tool_handlers::ExecuteCodeHandler`] call this from inside their
/// `handle` methods so the per-request `oauth_passthrough` token actually reaches
/// the outbound request at runtime.
///
/// Reads `extra.auth_context().and_then(|ctx| ctx.token.clone())` (the raw
/// inbound `Authorization` header captured by the binary's
/// `TokenCaptureAuthProvider`) and returns a cheap clone of `base` carrying that
/// token via [`HttpCodeExecutor::with_inbound_token`]. For an `oauth_passthrough`
/// backend the cloned executor forwards the captured token to `target_header`;
/// for static-auth backends the token is ignored (harmless).
#[cfg(feature = "openapi-code-mode")]
#[must_use]
pub fn request_executor_from_extra(
    base: &HttpCodeExecutor,
    extra: &pmcp::RequestHandlerExtra,
) -> HttpCodeExecutor {
    let token = extra.auth_context().and_then(|ctx| ctx.token.clone());
    base.clone().with_inbound_token(token)
}

// =============================================================================
// R1 split — validation_pipeline_from_config + code_mode_tools_from_executor
// =============================================================================

/// Build a [`ValidationPipeline`] from a [`ServerConfig`]'s `[code_mode]` block.
///
/// Maps every reference-server [`CodeModeSection`] field onto
/// [`CodeModeConfig`] per the verified construction surface in
/// `CODE_MODE_API_NOTES.md` Section 2. The pipeline's HMAC token machinery is
/// keyed by the resolved [`TokenSecret`] (derived from a toolkit-owned
/// [`SecretValue`] per review R6).
///
/// Per Phase 83 review R1 — the preflight selected the R1 split because
/// `pmcp-code-mode`'s [`CodeExecutor`] requires backend injection
/// (`HttpExecutor` / `SdkExecutor` / `McpExecutor`); no config-only executor
/// constructor exists. This function delivers the validation surface; the
/// caller supplies the executor (see [`code_mode_tools_from_executor`]).
///
/// # Errors
///
/// - [`ToolkitError::CodeMode`] if `config.code_mode` is `None`.
/// - [`ToolkitError::Validation`] wrapping
///   [`ConfigValidationError::InlineSecretRejected`] when `token_secret` is an
///   inline literal without `allow_inline_token_secret_for_dev` (review R9).
/// - [`ToolkitError::CodeMode`] if the env var referenced by `env:VAR_NAME` is
///   unset, or if the resolved secret is shorter than
///   [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::validation_pipeline_from_config;
/// use pmcp_server_toolkit::config::ServerConfig;
///
/// // ServerConfig with a [code_mode] block + env:-style token_secret
/// // resolves into a ValidationPipeline ready to validate SQL / GraphQL.
/// let toml = r#"
/// [server]
/// name = "demo"
/// version = "0.1.0"
/// [code_mode]
/// enabled = true
/// token_secret = "env:DEMO_HMAC_SECRET"
/// "#;
/// std::env::set_var("DEMO_HMAC_SECRET", "demo-secret-that-is-long-enough");
/// let cfg = ServerConfig::from_toml_strict_validated(toml).unwrap();
/// let _pipeline = validation_pipeline_from_config(&cfg).unwrap();
/// ```
pub fn validation_pipeline_from_config(config: &ServerConfig) -> Result<ValidationPipeline> {
    let section = config.code_mode.as_ref().ok_or_else(|| {
        ToolkitError::CodeMode("ServerConfig has no [code_mode] block".to_string())
    })?;
    let cm_config = build_cm_config(section);
    let secret_value = resolve_token_secret(section)?;
    let token_secret: TokenSecret = secret_value.into(); // R6 conversion
    ValidationPipeline::from_token_secret(cm_config, &token_secret)
        .map_err(|e| ToolkitError::CodeMode(format!("ValidationPipeline construction failed: {e}")))
}

/// Register `validate_code` + `execute_code` on `builder`, driven by the
/// `[code_mode]` block, a caller-supplied [`CodeExecutor`], and a
/// [`ValidationFlavor`] (OAPI-10 / D-02).
///
/// This is the ONE backend-agnostic wiring function serving BOTH the SQL path
/// (`flavor = ValidationFlavor::Sql`, executor = [`SqlCodeExecutor`]) and the
/// OpenAPI path (`flavor = ValidationFlavor::OpenApi`, executor =
/// `JsCodeExecutor<HttpCodeExecutor>`). The `executor` is type-erased to
/// `Arc<dyn CodeExecutor>` so the same function — and the same `execute_code`
/// handler body, which already dispatches through the trait — works for any
/// backend; only the `flavor` selects the validation surface + tool format.
///
/// This is the actual two-tool registration the LOCKED
/// [`crate::builder_ext::ServerBuilderExt::try_code_mode_from_config_with_connector`]
/// delegates to (the Phase 83-06 R1 split precedent: the connector-aware
/// builder method constructs the executor, this helper wires the tools).
///
/// - When `config.code_mode.is_none()` the builder is returned UNCHANGED
///   (no-op) — code-mode is opt-in at the config level.
/// - When `[code_mode]` IS present, the R9 inline-secret gate and the
///   secret-resolution / HMAC machinery run via [`validation_pipeline_from_config`]
///   (errors surface BEFORE `.build()`), then both tools are registered with
///   the static `[code_mode]` policy baked into the pipeline (SC-3 / D-13).
///   A [`NoopPolicyEvaluator`] is wired so authorization is purely the static
///   config policy (allow_writes / allow_deletes / allow_ddl), not an external
///   Cedar/AVP engine.
///
/// # Errors
///
/// Surfaces every error from [`validation_pipeline_from_config`] when
/// `config.code_mode.is_some()` — most notably
/// [`ConfigValidationError::InlineSecretRejected`] (review R9) and the
/// [`ToolkitError::CodeMode`] secret-resolution / 16-byte-minimum failures.
pub fn code_mode_tools_from_executor(
    builder: pmcp::ServerBuilder,
    config: &ServerConfig,
    executor: Arc<dyn CodeExecutor>,
    flavor: ValidationFlavor,
) -> Result<pmcp::ServerBuilder> {
    let Some(section) = config.code_mode.as_ref() else {
        return Ok(builder); // no-op when block absent
    };
    // Build the policy-bearing pipeline. This is also the R9 enforcement gate +
    // secret resolution — must run BEFORE the builder is returned so a
    // misconfigured token_secret is caught at builder-time, not first request.
    let cm_config = build_cm_config(section);
    let secret_value = resolve_token_secret(section)?;
    let token_secret: TokenSecret = secret_value.into();
    let evaluator: Arc<dyn PolicyEvaluator> = Arc::new(NoopPolicyEvaluator::new());
    let pipeline = ValidationPipeline::from_token_secret_with_policy(
        cm_config.clone(),
        &token_secret,
        evaluator,
    )
    .map_err(|e| ToolkitError::CodeMode(format!("ValidationPipeline construction failed: {e}")))?;
    let pipeline = Arc::new(pipeline);

    let validate_handler = tool_handlers::ValidateCodeHandler {
        pipeline: Arc::clone(&pipeline),
        config: cm_config,
        flavor,
    };
    let execute_handler = tool_handlers::ExecuteCodeHandler {
        pipeline,
        source: tool_handlers::ExecSource::Static(executor),
        flavor,
    };

    Ok(builder
        .tool_arc("validate_code", Arc::new(validate_handler))
        .tool_arc("execute_code", Arc::new(execute_handler)))
}

/// Register `validate_code` + `execute_code` on `builder` for the **OpenAPI
/// per-request** Code-Mode path (Plan 90-10 / OAPI-03 / OAPI-05).
///
/// This is the per-request analog of [`code_mode_tools_from_executor`]. Where
/// that helper takes a FIXED type-erased `Arc<dyn CodeExecutor>` (the SQL path,
/// whose `SqlCodeExecutor` carries no per-request state), this helper takes the
/// concrete [`HttpCodeExecutor`] `base` + [`ExecutionConfig`] so the
/// [`tool_handlers::ExecuteCodeHandler`] can RE-DERIVE a request-scoped
/// `JsCodeExecutor` per call via [`request_executor_from_extra`] — threading the
/// captured inbound MCP token so an `oauth_passthrough` backend forwards it to
/// the real backend.
///
/// The reason a per-request entry point is required: a `JsCodeExecutor`'s inner
/// `http` field is private with no accessor, so a type-erased
/// `Arc<dyn CodeExecutor>` cannot be re-derived per request. Holding the base
/// [`HttpCodeExecutor`] (which IS `Clone` + has the `with_inbound_token` builder)
/// makes the per-request rederivation possible WITHOUT changing the SQL path.
///
/// The `validate_code` handler is identical to the
/// [`code_mode_tools_from_executor`] one; only `execute_code` differs (it carries
/// the [`tool_handlers::ExecSource::PerRequestHttp`] source instead of
/// [`tool_handlers::ExecSource::Static`]).
///
/// # Errors
///
/// Surfaces every error from [`validation_pipeline_from_config`] when
/// `config.code_mode.is_some()` (R9 inline-secret rejection, secret-resolution /
/// 16-byte-minimum failures). No-op (returns the builder unchanged) when
/// `config.code_mode.is_none()`.
#[cfg(feature = "openapi-code-mode")]
pub fn code_mode_http_tools_from_executor(
    builder: pmcp::ServerBuilder,
    config: &ServerConfig,
    base: HttpCodeExecutor,
    exec_config: ExecutionConfig,
    flavor: ValidationFlavor,
) -> Result<pmcp::ServerBuilder> {
    let Some(section) = config.code_mode.as_ref() else {
        return Ok(builder); // no-op when block absent
    };
    // R9 enforcement gate + secret resolution — must run BEFORE the builder is
    // returned so a misconfigured token_secret is caught at builder-time.
    let cm_config = build_cm_config(section);
    let secret_value = resolve_token_secret(section)?;
    let token_secret: TokenSecret = secret_value.into();
    let evaluator: Arc<dyn PolicyEvaluator> = Arc::new(NoopPolicyEvaluator::new());
    let pipeline = ValidationPipeline::from_token_secret_with_policy(
        cm_config.clone(),
        &token_secret,
        evaluator,
    )
    .map_err(|e| ToolkitError::CodeMode(format!("ValidationPipeline construction failed: {e}")))?;
    let pipeline = Arc::new(pipeline);

    let validate_handler = tool_handlers::ValidateCodeHandler {
        pipeline: Arc::clone(&pipeline),
        config: cm_config,
        flavor,
    };
    let execute_handler = tool_handlers::ExecuteCodeHandler {
        pipeline,
        source: tool_handlers::ExecSource::PerRequestHttp { base, exec_config },
        flavor,
    };

    Ok(builder
        .tool_arc("validate_code", Arc::new(validate_handler))
        .tool_arc("execute_code", Arc::new(execute_handler)))
}

/// Tolerant builder-extension entry point for `[code_mode]` config — the
/// CONNECTORLESS, **validation-only / no-tool** path.
///
/// Used by [`crate::builder_ext::ServerBuilderExt::try_code_mode_from_config`]
/// (the connectorless companion). It is deliberately tolerant of
/// `config.code_mode = None` (returns the builder unchanged) so callers can
/// invoke it unconditionally — code-mode is opt-in at the config level.
///
/// When `[code_mode]` IS present, this helper drives
/// [`validation_pipeline_from_config`] to surface R9 enforcement errors
/// (inline `token_secret` rejection) before the builder reaches `.build()`,
/// but registers NO tools because there is no executor to bind to. The
/// tool-registering path is
/// [`crate::builder_ext::ServerBuilderExt::try_code_mode_from_config_with_connector`]
/// (which delegates to [`code_mode_tools_from_executor`]).
///
/// # Errors
///
/// Returns every error from [`validation_pipeline_from_config`] when
/// `config.code_mode.is_some()`. No errors when `config.code_mode.is_none()`.
pub fn register_code_mode_tools(
    builder: pmcp::ServerBuilder,
    config: &ServerConfig,
) -> Result<pmcp::ServerBuilder> {
    if config.code_mode.is_none() {
        return Ok(builder); // no-op when block absent
    }
    // R9 enforcement gate — must run BEFORE the builder is returned so that a
    // misconfigured `[code_mode] token_secret = "inline-string"` is caught at
    // builder-time, not at first request. NO tools registered (no executor) —
    // this is the documented connectorless validation-only path.
    let _pipeline = validation_pipeline_from_config(config)?;
    Ok(builder)
}

// =============================================================================
// Hand-built validate_code / execute_code ToolHandlers (Plan 85-02 Task 2)
//
// Mirrors the `#[derive(CodeMode)]` macro output in pmcp-code-mode-derive but
// hand-written here so the toolkit does NOT take a proc-macro dependency. Only
// the PUBLIC API (`code_mode_tools_from_executor` +
// `try_code_mode_from_config_with_connector`) is LOCKED; this internal
// mechanism is the implementer's discretion (Plan 85-02 Task 2).
// =============================================================================
mod tool_handlers {
    use std::sync::Arc;

    use super::ValidationFlavor;
    use pmcp_code_mode::TokenGenerator as _;

    /// Run the flavor-appropriate validation surface (OAPI-10 / D-02).
    ///
    /// - [`ValidationFlavor::Sql`] → `validate_sql_query` (Shape A SQL path).
    /// - [`ValidationFlavor::OpenApi`] → `validate_javascript_code` (the OpenAPI
    ///   JS path; really runs SWC-backed JS validation). Only reachable when the
    ///   `openapi-code-mode` feature is enabled — the binary that wires the
    ///   OpenApi flavor enables that umbrella, so the arm is feature-gated.
    fn run_flavored_validation(
        pipeline: &pmcp_code_mode::ValidationPipeline,
        flavor: ValidationFlavor,
        code: &str,
        context: &pmcp_code_mode::ValidationContext,
    ) -> std::result::Result<pmcp_code_mode::ValidationResult, String> {
        match flavor {
            ValidationFlavor::Sql => pipeline
                .validate_sql_query(code, context)
                .map_err(|e| format!("Validation error: {e}")),
            #[cfg(feature = "openapi-code-mode")]
            ValidationFlavor::OpenApi => pipeline
                .validate_javascript_code(code, context)
                .map_err(|e| format!("Validation error: {e}")),
            #[cfg(not(feature = "openapi-code-mode"))]
            ValidationFlavor::OpenApi => Err(
                "OpenAPI Code Mode validation requires the `openapi-code-mode` feature".to_string(),
            ),
        }
    }

    /// `validate_code` tool handler: runs the code through the policy-bearing
    /// [`ValidationPipeline`](pmcp_code_mode::ValidationPipeline) (SQL or JS per
    /// [`ValidationFlavor`]) and returns the explanation + (on success) an HMAC
    /// approval token.
    pub(super) struct ValidateCodeHandler {
        pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
        pub(super) config: pmcp_code_mode::CodeModeConfig,
        pub(super) flavor: ValidationFlavor,
    }

    #[pmcp_code_mode::async_trait]
    impl pmcp::ToolHandler for ValidateCodeHandler {
        async fn handle(
            &self,
            args: serde_json::Value,
            _extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<serde_json::Value> {
            let input: pmcp_code_mode::ValidateCodeInput = serde_json::from_value(args)
                .map_err(|e| pmcp::Error::Internal(format!("Invalid arguments: {e}")))?;
            let code = input.code.trim();
            let dry_run = input.dry_run.unwrap_or(false);

            // Static-policy ValidationContext — the toolkit binds approval
            // tokens to a fixed config-derived context (no live user/session
            // surface in the pure-config binary). Static `[code_mode]` policy
            // (allow_writes/deletes/ddl for SQL; openapi_blocked_paths /
            // disallowed ops for OpenApi) is enforced inside the validation
            // surface selected by `flavor`.
            let context = pmcp_code_mode::ValidationContext::new(
                "code-mode-config",
                "code-mode-session",
                "schema-hash",
                "perms-hash",
            );

            let result = run_flavored_validation(&self.pipeline, self.flavor, code, &context)
                .map_err(pmcp::Error::Internal)?;

            let mut response = pmcp_code_mode::ValidationResponse::from_result(result);
            if response.result.is_valid {
                if dry_run {
                    response.result.approval_token = None;
                }
                let risk = response.result.risk_level;
                response = response.with_auto_approved(self.config.should_auto_approve(risk));
            }
            let (json, is_error) = response.to_json_response();
            // A policy rejection (allow_writes/deletes/ddl off, require_limit, …)
            // is reported by `to_json_response` with `is_error == true`. Surface it
            // as a TOOL-level rejection via `Error::tool_rejected` so the MCP
            // `tools/call` result is `CallToolResult { isError: true }` carrying a
            // model-actionable `message` plus the full violation JSON in
            // `structuredContent` — NOT a `-32603` protocol error (which reads as a
            // server fault and gives the model nothing to correct). This is the
            // production-reference observable the generated.yaml `failure`
            // assertions (DELETE/DDL/no-LIMIT) verify: mcp-tester treats
            // `isError: true` as a failed step (SC-3 policy-enforcement proof,
            // threat T-85-02-02).
            if is_error {
                let message = response
                    .result
                    .violations
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        "Code Mode rejected the query (policy validation failed)".to_string()
                    });
                return Err(pmcp::Error::tool_rejected(message, Some(json)));
            }
            Ok(json)
        }

        fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
            Some(
                pmcp_code_mode::CodeModeToolBuilder::new(self.flavor.code_format())
                    .build_validate_tool(),
            )
        }
    }

    /// How `execute_code` obtains the [`CodeExecutor`](pmcp_code_mode::CodeExecutor)
    /// for a request (Plan 90-10 / OAPI-03 / OAPI-05).
    ///
    /// - [`ExecSource::Static`] — a FIXED type-erased executor (the SQL path's
    ///   `SqlCodeExecutor`, which carries no per-request state). Unchanged
    ///   behavior; available under bare `code-mode`.
    /// - [`ExecSource::PerRequestHttp`] — the OpenAPI path: a base
    ///   [`HttpCodeExecutor`](super::HttpCodeExecutor) + [`ExecutionConfig`](super::ExecutionConfig)
    ///   from which a request-scoped `JsCodeExecutor` is RE-DERIVED per call (via
    ///   [`request_executor_from_extra`](super::request_executor_from_extra)) so
    ///   the captured inbound `oauth_passthrough` token is threaded to the
    ///   backend. Feature-gated `openapi-code-mode` (the engine types are only in
    ///   scope there); the SQL build is unaffected.
    pub(super) enum ExecSource {
        /// SQL path — a fixed type-erased executor, no per-request derivation.
        Static(Arc<dyn pmcp_code_mode::CodeExecutor>),
        /// OpenAPI path — re-derive a request-scoped executor per call so the
        /// captured inbound token reaches the backend (OAPI-03 / OAPI-05).
        #[cfg(feature = "openapi-code-mode")]
        PerRequestHttp {
            /// The base executor (cloned + token-threaded per request).
            base: super::HttpCodeExecutor,
            /// The execution bounds for the per-request `JsCodeExecutor`.
            exec_config: super::ExecutionConfig,
        },
    }

    /// `execute_code` tool handler: verifies the approval token + code hash,
    /// then runs the code through the backend-agnostic
    /// [`CodeExecutor`](pmcp_code_mode::CodeExecutor) (SQL re-validates before
    /// the connector; OpenAPI runs the validated JS through a request-scoped
    /// `JsCodeExecutor`). The `flavor` only selects the tool `format` metadata —
    /// the `handle` body dispatches through the trait regardless of backend.
    pub(super) struct ExecuteCodeHandler {
        pub(super) pipeline: Arc<pmcp_code_mode::ValidationPipeline>,
        pub(super) source: ExecSource,
        pub(super) flavor: ValidationFlavor,
    }

    impl ExecuteCodeHandler {
        /// Run the validated `code` through the source-appropriate executor
        /// (Plan 90-10). The `Static` arm dispatches through the fixed
        /// type-erased executor (SQL); the `PerRequestHttp` arm RE-DERIVES a
        /// request-scoped `JsCodeExecutor` carrying the captured inbound token
        /// via [`request_executor_from_extra`](super::request_executor_from_extra)
        /// so an `oauth_passthrough` backend forwards it (OAPI-03 / OAPI-05).
        ///
        /// Extracted from `handle` to keep both bodies under the cog ≤25 budget.
        async fn run_code(
            &self,
            code: &str,
            variables: Option<&serde_json::Value>,
            #[cfg_attr(not(feature = "openapi-code-mode"), allow(unused_variables))]
            extra: &pmcp::RequestHandlerExtra,
        ) -> std::result::Result<serde_json::Value, pmcp_code_mode::ExecutionError> {
            // Gated to match the ONE arm that needs it. Under `openapi-code-mode`
            // the `PerRequestHttp` arm calls `.execute()` on a freshly built
            // `JsCodeExecutor`, which resolves only through this trait. Without
            // that feature the arm is cfg'd out and the `Static` arm resolves
            // `.execute()` without the trait, leaving the import unused — a
            // warning that becomes a hard error wherever `-D warnings` reaches
            // this crate. Measured both ways: `--features http` warns,
            // `--features http,openapi-code-mode` does not.
            #[cfg(feature = "openapi-code-mode")]
            use pmcp_code_mode::CodeExecutor as _;
            match &self.source {
                ExecSource::Static(executor) => executor.execute(code, variables).await,
                #[cfg(feature = "openapi-code-mode")]
                ExecSource::PerRequestHttp { base, exec_config } => {
                    let http_exec = super::request_executor_from_extra(base, extra);
                    super::JsCodeExecutor::new(http_exec, exec_config.clone())
                        .execute(code, variables)
                        .await
                },
            }
        }
    }

    #[pmcp_code_mode::async_trait]
    impl pmcp::ToolHandler for ExecuteCodeHandler {
        async fn handle(
            &self,
            args: serde_json::Value,
            extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<serde_json::Value> {
            let input: pmcp_code_mode::ExecuteCodeInput = serde_json::from_value(args)
                .map_err(|e| pmcp::Error::Internal(format!("Invalid arguments: {e}")))?;
            let code = input.code.trim();

            // Token / code-hash verification failures are model-actionable
            // rejections (the model must re-run validate_code to obtain a fresh
            // token, or resend the exact validated code), so surface them as
            // `CallToolResult { isError: true }` via `Error::tool_rejected` —
            // not `-32603`. A genuine execution fault (connector/SQL runtime,
            // below) stays an `Internal` protocol error: the caller cannot fix
            // it by changing input.
            let token_gen = self.pipeline.token_generator();
            let token =
                pmcp_code_mode::ApprovalToken::decode(&input.approval_token).map_err(|e| {
                    pmcp::Error::tool_rejected(
                        format!(
                        "Invalid approval_token: {e}. Call validate_code to obtain a valid token."
                    ),
                        None,
                    )
                })?;
            token_gen.verify(&token).map_err(|e| {
                pmcp::Error::tool_rejected(
                    format!(
                        "Approval token is invalid or expired: {e}. \
                         Call validate_code again to obtain a fresh token."
                    ),
                    None,
                )
            })?;
            token_gen.verify_code(code, &token).map_err(|e| {
                pmcp::Error::tool_rejected(
                    format!(
                        "Code does not match the validated code: {e}. execute_code must use the \
                         exact code string that was passed to validate_code."
                    ),
                    None,
                )
            })?;

            let result = self
                .run_code(code, input.variables.as_ref(), &extra)
                .await
                .map_err(|e| pmcp::Error::Internal(format!("Execution error: {e}")))?;
            Ok(result)
        }

        fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
            Some(
                pmcp_code_mode::CodeModeToolBuilder::new(self.flavor.code_format())
                    .build_execute_tool(),
            )
        }
    }
}

// =============================================================================
// SHAP-A-01 — SqlCodeExecutor (Plan 85-02 Task 1)
// =============================================================================

/// [`CodeExecutor`] adapter bridging the toolkit's single-method
/// [`SqlConnector`] to the code-mode `validate_code` / `execute_code` flow.
///
/// # Re-derived for the single-method trait
///
/// The production reference (`mcp-sql-server-core::SqlCodeModeHandler`) is
/// written over a 2-method `DatabaseConnector` (`execute_query` /
/// `execute_statement`) and dispatches by [`crate::sql`]'s
/// `QueryType`. The toolkit's [`SqlConnector`] exposes a SINGLE
/// [`SqlConnector::execute`] entry point, so this adapter collapses that
/// 2-method dispatch into one `connector.execute(sql, &params)` call regardless
/// of statement type — re-validating the SQL FIRST for defense-in-depth. The
/// `execute_code` `variables` input IS bound as named params (85-10 WR-02);
/// it is never silently dropped.
///
/// # Defense-in-depth re-validation (threat T-85-02-01)
///
/// Before touching the connector, [`SqlCodeExecutor::execute`] re-runs the
/// `[code_mode]` policy against the supplied SQL via the same
/// [`ValidationPipeline`] the `validate_code` tool used. The code-mode
/// framework already verified the approval token + code hash before calling
/// this method, but re-validation guards against a token issued for an
/// allowed statement being replayed with a different (e.g. mutating)
/// statement. A policy violation returns `Err(ExecutionError::BackendError)`
/// BEFORE the connector is reached — a config-driven server cannot bypass the
/// write/DDL guards (SC-3, threat T-85-02-02).
///
/// # Observable result shape (REVIEW FIX Codex MEDIUM #6b)
///
/// The production handler returns
/// `{"columns": [...], "rows": [...], "rows_affected": N}` because its
/// 2-method connector surfaces columns + affected-row counts separately. The
/// toolkit's [`SqlConnector::execute`] returns `Vec<Value>` (one JSON object
/// per row, keyed by column name) with no separate columns/rows_affected
/// channel, so this adapter mirrors production's OBSERVABLE `"rows"` key:
/// `{"rows": <values>}`. The parity replay (Plan 06) only exercises
/// `execute_code` with an INVALID token (asserts `failure`), so this success
/// shape is not asserted by `generated.yaml`; mirroring production keeps the
/// executor correct for any future success-path scenario and for the direct
/// unit assertions in this crate.
pub struct SqlCodeExecutor {
    connector: Arc<dyn SqlConnector>,
    /// The re-validation pipeline, built ONCE at construction (85-10 IN-01).
    ///
    /// Previously [`SqlCodeExecutor::revalidate`] rebuilt the pipeline AND
    /// re-resolved the `token_secret` env var on EVERY `execute` call. Caching
    /// it here means the secret is resolved a single time (at construction /
    /// builder time) — a removed/rotated env var after startup no longer breaks
    /// in-flight requests, and a bad secret still fails fast at builder time.
    pipeline: Arc<ValidationPipeline>,
}

impl SqlCodeExecutor {
    /// Construct an executor over `connector`, enforcing the `[code_mode]`
    /// policy carried by `config` on every [`SqlCodeExecutor::execute`] call.
    ///
    /// The [`ValidationPipeline`] is built ONCE here (85-10 IN-01) via
    /// [`validation_pipeline_from_config`], so the `token_secret` env var is
    /// resolved a single time at construction rather than on every request.
    ///
    /// # Errors
    ///
    /// Returns every error from [`validation_pipeline_from_config`] — most
    /// notably the R9 inline-secret rejection and the secret-resolution /
    /// 16-byte-minimum failures — so a misconfigured `token_secret` fails at
    /// builder time, not first request.
    pub fn new(connector: Arc<dyn SqlConnector>, config: ServerConfig) -> Result<Self> {
        let pipeline = Arc::new(validation_pipeline_from_config(&config)?);
        Ok(Self {
            connector,
            pipeline,
        })
    }

    /// Defense-in-depth re-validation of `code` against the `[code_mode]`
    /// policy (threat T-85-02-01). Returns `Err` BEFORE any connector call when
    /// the statement violates the static policy (e.g. a DELETE under
    /// `allow_deletes = false`) or fails to parse.
    ///
    /// Reuses the cached [`SqlCodeExecutor::pipeline`] (85-10 IN-01) — it does
    /// NOT rebuild the pipeline or re-read the `token_secret` env var per call.
    fn revalidate(&self, code: &str) -> std::result::Result<(), ExecutionError> {
        let ctx = ValidationContext::new(
            "code-mode-executor",
            "code-mode-session",
            "schema-hash",
            "perms-hash",
        );
        let result = self
            .pipeline
            .validate_sql_query(code, &ctx)
            .map_err(|e| ExecutionError::BackendError(format!("SQL validation failed: {e}")))?;
        if !result.is_valid {
            return Err(ExecutionError::BackendError(
                "SQL rejected by [code_mode] policy on re-validation".to_string(),
            ));
        }
        Ok(())
    }
}

/// Convert the `execute_code` `variables` input (a JSON object of name→value)
/// into the `(name, value)` pairs [`SqlConnector::execute`] binds (85-10
/// WR-02). A leading `:` on a key is stripped so callers may send either
/// `{":name": ...}` or `{"name": ...}` — the connector's
/// `translate_placeholders` keys params WITHOUT the `:` (matching
/// [`extract_named_params`](crate::tools)). `None` or a non-object value yields
/// an empty slice, so the parity `execute_code` scenario (passes `None`) is
/// unaffected.
fn variables_to_params(variables: Option<&serde_json::Value>) -> Vec<(String, serde_json::Value)> {
    let Some(serde_json::Value::Object(map)) = variables else {
        return Vec::new();
    };
    map.iter()
        .map(|(k, v)| {
            let key = k.strip_prefix(':').unwrap_or(k).to_string();
            (key, v.clone())
        })
        .collect()
}

#[pmcp_code_mode::async_trait]
impl CodeExecutor for SqlCodeExecutor {
    /// Re-validate the SQL against the `[code_mode]` policy, then execute it via
    /// the single-method [`SqlConnector::execute`].
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::BackendError`] when re-validation rejects the
    /// statement (policy violation or parse failure) or when the connector
    /// surfaces a [`crate::sql::ConnectorError`]. Connector error messages are
    /// surfaced verbatim from the toolkit's already-sanitized
    /// `ConnectorError` Display (T-84-01-01 / threat T-85-02-04) — no raw
    /// backend credentials are echoed.
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> std::result::Result<serde_json::Value, ExecutionError> {
        // (1) Defense-in-depth re-validation BEFORE the connector is reached.
        self.revalidate(code)?;
        // (2) Honor the schema-advertised `variables` input by BINDING it as
        //     named params (85-10 WR-02 / threat T-85-10-01) — never a silent
        //     drop. A `None` / absent map yields `&[]`, so the parity scenario
        //     (passes None) is unaffected. Binding (not string interpolation)
        //     preserves parameterized-query safety.
        let params = variables_to_params(variables);
        let rows =
            self.connector.execute(code, &params).await.map_err(|e| {
                ExecutionError::BackendError(format!("connector execute failed: {e}"))
            })?;
        // (3) Mirror production's observable `"rows"` key (REVIEW FIX #6b).
        Ok(serde_json::json!({ "rows": rows }))
    }
}

// =============================================================================
// OAPI-05 — HttpCodeExecutor (Plan 90-04 Task 1 / H1 / H2)
// =============================================================================

/// Low-level HTTP executor bridging the toolkit's outbound
/// [`HttpAuthProvider`](crate::http::auth::HttpAuthProvider) to pmcp-code-mode's
/// [`HttpExecutor`](pmcp_code_mode::HttpExecutor) trait.
///
/// This is the OpenAPI analog of [`SqlCodeExecutor`], but at a DIFFERENT layer:
/// it impls the LOW-LEVEL `pmcp_code_mode::HttpExecutor`
/// (`execute_request(method, path, body)`), NOT the high-level
/// [`CodeExecutor`]. It is wrapped by a
/// [`JsCodeExecutor`](pmcp_code_mode::JsCodeExecutor) for the Code Mode path
/// (the `JsCodeExecutor<HttpCodeExecutor>: CodeExecutor` blanket impl) and is
/// called directly by script tools (Plan 05). The single-call synthesizer
/// (Plan 03) does NOT use this path — it calls `HttpConnector::execute`
/// directly.
///
/// # Per-request passthrough token (H1)
///
/// The `inbound_token` field carries the per-request MCP client token captured
/// by the binary (Plan 06) into [`AuthContext`]. It is passed to
/// [`HttpAuthProvider::apply`](crate::http::auth::HttpAuthProvider::apply) so an
/// [`OAuthPassthroughAuth`](crate::http::auth::OAuthPassthroughAuth) provider
/// forwards it to the backend; static providers ignore it (proven in Plan 01).
/// Because Code Mode reuses ONE executor instance across requests, the binary
/// produces a per-request clone carrying the captured token via
/// [`HttpCodeExecutor::with_inbound_token`].
///
/// # Redaction (Pitfall 5 / T-90-04-01)
///
/// Auth/transport failures are mapped to
/// [`ExecutionError::RuntimeError`](pmcp_code_mode::ExecutionError::RuntimeError)
/// whose message names the operation / status only — it NEVER echoes the
/// request URL or the `Authorization` token.
///
/// # Feature gate (H2)
///
/// Gated under `openapi-code-mode` (the Plan 90-01 umbrella that forwards
/// `pmcp-code-mode/js-runtime`). The bare `code-mode` feature does NOT bring
/// `HttpExecutor` into scope, so this type cannot be gated on
/// `all(feature = "http", feature = "code-mode")`.
#[cfg(feature = "openapi-code-mode")]
#[derive(Clone)]
pub struct HttpCodeExecutor {
    client: reqwest::Client,
    base_url: String,
    auth: Arc<dyn crate::http::auth::HttpAuthProvider>,
    /// Per-request captured MCP client token for `oauth_passthrough` (H1).
    /// `None` for the static-auth path; set per request via
    /// [`HttpCodeExecutor::with_inbound_token`].
    inbound_token: Option<String>,
}

#[cfg(feature = "openapi-code-mode")]
impl HttpCodeExecutor {
    /// Construct an executor over `client` + `base_url`, authenticating outgoing
    /// requests via `auth`. The per-request `inbound_token` starts `None`;
    /// the binary attaches it per request with
    /// [`HttpCodeExecutor::with_inbound_token`].
    #[must_use]
    pub fn new(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn crate::http::auth::HttpAuthProvider>,
    ) -> Self {
        Self {
            client,
            base_url,
            auth,
            inbound_token: None,
        }
    }

    /// Cheap clone-with-token builder (H1): the binary calls this PER REQUEST to
    /// attach the captured inbound MCP token so an `oauth_passthrough` provider
    /// forwards it. Static providers ignore the token, so calling this on a
    /// static-auth executor is harmless.
    ///
    /// Single-call tools (Plan 03) don't use this path; the per-request token
    /// flows through Code Mode + script tools only.
    #[must_use]
    pub fn with_inbound_token(mut self, token: Option<String>) -> Self {
        self.inbound_token = token;
        self
    }

    /// Test-only accessor for the per-request captured token, so unit tests can
    /// assert [`request_executor_from_extra`] threads the inbound token (the
    /// field is otherwise private — Plan 90-10).
    #[cfg(test)]
    pub(crate) fn inbound_token_for_test(&self) -> Option<&str> {
        self.inbound_token.as_deref()
    }

    /// Substitute `{key}` path-template segments from `body` keys, returning the
    /// resolved path and the remaining (non-path) body fields.
    ///
    /// Lifted from the pmcp-run reference `execute_request` (kept a free helper
    /// so the trait method stays under the cog ≤25 budget).
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::RuntimeError`] naming the offending key when a
    /// `{key}` path value is a non-scalar (`Object`/`Array`) — see
    /// [`HttpCodeExecutor::scalar_str`] for the decided rule (WR-03 / GAP 4).
    fn resolve_path(
        path: &str,
        body: &Option<serde_json::Value>,
    ) -> std::result::Result<(String, Option<serde_json::Value>), ExecutionError> {
        let mut resolved_path = path.to_string();
        let remaining = if let Some(serde_json::Value::Object(obj)) = body {
            let mut remaining = serde_json::Map::new();
            for (key, value) in obj {
                let placeholder = format!("{{{key}}}");
                if resolved_path.contains(&placeholder) {
                    resolved_path =
                        resolved_path.replace(&placeholder, &Self::scalar_str(key, value)?);
                } else {
                    remaining.insert(key.clone(), value.clone());
                }
            }
            if remaining.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(remaining))
            }
        } else {
            body.clone()
        };
        Ok((resolved_path, remaining))
    }

    /// Render a JSON scalar for path / query substitution (strings unquoted),
    /// REJECTING non-scalar values (WR-03 / GAP 4).
    ///
    /// This is the `code_mode` counterpart of [`crate::http::client`]'s
    /// `render_scalar`; both HTTP surfaces apply the SAME decided rule. Because
    /// the `Parameter` model carries no OpenAPI `style`/`explode`/`type` hint,
    /// the rule is uniform: a scalar (`String`, `Number`, `Bool`, `Null`)
    /// renders to a bare string (`Null` → `"null"`, preserving prior behavior);
    /// an `Object` or `Array` in a `{path}` substitution or a GET-query field is
    /// rejected rather than silently JSON-stringified into the URL.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::RuntimeError`] naming `key` when `value` is a
    /// non-scalar. Per Pitfall 5 the message names the KEY only — never the value.
    fn scalar_str(
        key: &str,
        value: &serde_json::Value,
    ) -> std::result::Result<String, ExecutionError> {
        match value {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Null => Ok("null".to_string()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                Err(ExecutionError::RuntimeError {
                    message: format!("path/query param '{key}' must be a scalar"),
                })
            },
        }
    }
}

#[cfg(feature = "openapi-code-mode")]
#[pmcp_code_mode::async_trait]
impl pmcp_code_mode::HttpExecutor for HttpCodeExecutor {
    async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> std::result::Result<serde_json::Value, ExecutionError> {
        let upper = method.to_uppercase();
        let is_get_like = matches!(upper.as_str(), "GET" | "HEAD" | "OPTIONS");

        // (1) Path-param substitution from the body object. A non-scalar `{key}`
        //     value is rejected (WR-03) rather than JSON-stringified into the URL.
        let (resolved_path, remaining_body) = Self::resolve_path(path, &body)?;

        // (2) Shared join_url helper (Pitfall 2 — preserves an API-Gateway
        //     stage prefix; it does NOT use the RFC-3986 path-replacing join).
        //     join_url does the base+path CONCAT; we still parse the result to
        //     append query pairs because reqwest 0.13 gates
        //     RequestBuilder::query behind a `query` feature the toolkit
        //     deliberately does not enable (Plan 01 Rule 1).
        let url = crate::http::join_url(&self.base_url, &resolved_path);

        // (3) Apply auth, threading the per-request inbound token (H1). Auth
        //     failures map to a RuntimeError WITHOUT echoing URL/token
        //     (Pitfall 5 / T-90-04-01).
        let mut headers = reqwest::header::HeaderMap::new();
        let mut auth_query: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        self.auth
            .apply(&mut headers, &mut auth_query, self.inbound_token.as_deref())
            .await
            .map_err(|_| ExecutionError::RuntimeError {
                message: "authentication failed for outgoing request".to_string(),
            })?;

        let mut query_params: Vec<(String, String)> = auth_query.into_iter().collect();

        // (4) For GET-like requests, serialize remaining body fields as query
        //     params; otherwise keep them as the JSON body.
        let request_body = if is_get_like {
            if let Some(serde_json::Value::Object(obj)) = &remaining_body {
                for (key, value) in obj {
                    // A non-scalar GET-query value is rejected (WR-03) rather than
                    // silently JSON-stringified into the URL.
                    query_params.push((key.clone(), Self::scalar_str(key, value)?));
                }
            }
            None
        } else {
            remaining_body
        };

        // Append query params via url::Url (reqwest 0.13's RequestBuilder::query
        // is behind the off-by-default `query` feature; Plan 01 Rule 1).
        let final_url = if query_params.is_empty() {
            url
        } else {
            let mut parsed = url::Url::parse(&url).map_err(|_| ExecutionError::RuntimeError {
                message: "could not construct the request URL".to_string(),
            })?;
            {
                let mut pairs = parsed.query_pairs_mut();
                for (k, v) in &query_params {
                    pairs.append_pair(k, v);
                }
            }
            parsed.to_string()
        };

        let mut request = match upper.as_str() {
            "GET" => self.client.get(&final_url),
            "POST" => self.client.post(&final_url),
            "PUT" => self.client.put(&final_url),
            "DELETE" => self.client.delete(&final_url),
            "PATCH" => self.client.patch(&final_url),
            "HEAD" => self.client.head(&final_url),
            _ => {
                return Err(ExecutionError::RuntimeError {
                    message: "unsupported HTTP method".to_string(),
                })
            },
        };
        request = request.headers(headers);
        if let Some(b) = request_body {
            request = request.header("Content-Type", "application/json").json(&b);
        }

        // (5) Send + read. Transport / status / parse errors NEVER echo the URL
        //     or token (Pitfall 5).
        let response = request
            .send()
            .await
            .map_err(|_| ExecutionError::RuntimeError {
                message: "outgoing HTTP request failed".to_string(),
            })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|_| ExecutionError::RuntimeError {
                message: "failed to read response body".to_string(),
            })?;
        if !status.is_success() {
            return Err(ExecutionError::RuntimeError {
                message: format!("backend returned HTTP status {}", status.as_u16()),
            });
        }
        if text.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&text).map_err(|_| ExecutionError::RuntimeError {
            message: "failed to parse response body as JSON".to_string(),
        })
    }
}

// =============================================================================
// Helpers (Pattern G — cog ≤25 each, kept small + explicit)
// =============================================================================

/// Translate unprefixed toolkit [`CodeModeSection`] fields into pmcp-code-mode's
/// `sql_`-prefixed [`CodeModeConfig`].
///
/// Mapping is **explicit field-by-field** (PATTERNS §10 + D-13). Silent serde
/// aliasing would couple the toolkit's stable surface to pmcp-code-mode's
/// internal field names — undesirable. Fields on `CodeModeSection` without a
/// `CodeModeConfig` counterpart are noted in inline comments rather than
/// silently dropped (review R1 + threat T-83-06-04).
fn build_cm_config(section: &CodeModeSection) -> CodeModeConfig {
    let mut cfg = CodeModeConfig {
        enabled: section.enabled,
        // SQL policy bits — toolkit's unprefixed names → pmcp_code_mode's sql_-prefixed.
        sql_allow_writes: section.allow_writes,
        sql_allow_deletes: section.allow_deletes,
        sql_allow_ddl: section.allow_ddl,
        sql_blocked_tables: section.blocked_tables.iter().cloned().collect(),
        sql_blocked_columns: section.sensitive_columns.iter().cloned().collect(),
        ..CodeModeConfig::default()
    };
    if let Some(ref sid) = section.server_id {
        cfg.server_id = Some(sid.clone());
    }
    // Token TTL — both sides use seconds, but pmcp_code_mode uses i64 and the
    // toolkit uses Option<u64>. Saturate to i64::MAX rather than wrap.
    if let Some(ttl) = section.token_ttl_seconds {
        cfg.token_ttl_seconds = i64::try_from(ttl).unwrap_or(i64::MAX);
    }
    // Auto-approval — toolkit ships risk-level names as strings; the
    // pmcp_code_mode side wants RiskLevel enums. Best-effort parse; unrecognised
    // entries are silently skipped (operator typos surface as "nothing auto-
    // approved" rather than a parse error — by design, since the registry is
    // open-ended).
    map_auto_approve_levels(&section.auto_approve_levels, &mut cfg);
    // `max_limit` (toolkit) corresponds to `sql_max_rows` (pmcp_code_mode).
    if let Some(max) = section.max_limit {
        cfg.sql_max_rows = max;
    }
    // `require_limit` (toolkit) → `sql_require_limit` (pmcp_code_mode). Enforced
    // in check_sql_config_authorization: a read-only statement without a LIMIT
    // is rejected when this is set (closes VERIFICATION Gap 1 — previously this
    // field was parsed but discarded, so a low-row no-LIMIT SELECT was accepted
    // despite require_limit=true).
    cfg.sql_require_limit = section.require_limit;
    // [code_mode.limits] — pmcp_code_mode's CodeModeConfig has `max_depth` and
    // `max_field_count` (GraphQL-flavoured) but no direct counterparts for
    // `max_tables_per_query` / `max_join_depth` / `max_subquery_depth`. These
    // toolkit fields are exposed for forward compatibility with Phase 84's
    // SQL connector enforcement; they are NOT silently mapped here.
    if let Some(ref limits) = section.limits {
        let _gap_max_tables = limits.max_tables_per_query;
        let _gap_max_join = limits.max_join_depth;
        let _gap_max_subquery = limits.max_subquery_depth;
    }
    cfg
}

/// Decompose auto-approve-level parsing to keep [`build_cm_config`] under
/// Pattern G's cog ≤25 budget.
fn map_auto_approve_levels(levels: &[String], cfg: &mut CodeModeConfig) {
    use pmcp_code_mode::RiskLevel;
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        match level.to_ascii_lowercase().as_str() {
            "low" => out.push(RiskLevel::Low),
            "medium" => out.push(RiskLevel::Medium),
            "high" => out.push(RiskLevel::High),
            "critical" => out.push(RiskLevel::Critical),
            _ => {
                tracing::debug!(
                    target: "pmcp_server_toolkit::code_mode",
                    "[code_mode] auto_approve_levels: unrecognised level '{}' — skipping",
                    level
                );
            },
        }
    }
    if !out.is_empty() {
        cfg.auto_approve_levels = out;
    }
}

/// Per review R9: `token_secret` is `env:`- or `${VAR}`-only by default. Inline
/// literals are REJECTED at config-validation time unless
/// `allow_inline_token_secret_for_dev` is set. Returns the resolved bytes
/// wrapped in the toolkit-owned [`SecretValue`] (per review R6).
///
/// Accepted forms:
/// - `token_secret = "env:VAR_NAME"` — reads `VAR_NAME` from the process env.
/// - `token_secret = "${VAR_NAME}"` — reads `VAR_NAME` from the process env
///   (the form every reference SQL-API config emits, Plan 85-01 Gap #3).
/// - `token_secret = "raw-string"` — REJECTED unless
///   `allow_inline_token_secret_for_dev = true`.
///
/// A missing/unset env var (either form) returns
/// [`ToolkitError::CodeMode`] — never a panic, never a fall-back to a weak or
/// empty secret (threat-model item T-85-01-01).
/// Read `var` from the process env for `token_secret`, treating a missing OR
/// set-but-empty/whitespace value as UNSET (85-10 secondary fix, threat
/// T-85-10-03).
///
/// `HmacTokenGenerator` enforces a 16-byte minimum downstream, but an empty
/// (or all-whitespace) env value should surface as a clear "set but empty"
/// configuration error at startup — never flow to the HMAC layer as a
/// degenerate secret. Both the `env:VAR` and `${VAR}` forms route through here.
fn resolve_secret_env_var(var: &str) -> Result<SecretValue> {
    let value = std::env::var(var)
        .map_err(|_| ToolkitError::CodeMode(format!("env var '{var}' not set for token_secret")))?;
    if value.trim().is_empty() {
        return Err(ToolkitError::CodeMode(format!(
            "env var '{var}' is set but empty for token_secret"
        )));
    }
    Ok(SecretValue::new(value.into_bytes()))
}

fn resolve_token_secret(section: &CodeModeSection) -> Result<SecretValue> {
    let raw = section.token_secret.as_ref().ok_or_else(|| {
        ToolkitError::CodeMode(
            "[code_mode] token_secret is required when code-mode is enabled".to_string(),
        )
    })?;
    // Both reference forms (`env:VAR` and `${VAR}`) are parsed by the ONE
    // toolkit-wide grammar chokepoint (Phase 120 Plan 04 Task 2). This module
    // previously carried its own `expand_braced_var`; a second `${}` parser with
    // slightly different edge cases is a latent security bug, so the grammar is
    // now single-sourced and only the RESOLUTION policy stays local (error on
    // unset, never a fall-back to a weak or empty secret — T-85-01-01).
    //
    // A string that merely *contains* `${` (e.g. an Athena `output_location`
    // substring) is still NOT a reference — `parse_env_ref` requires the exact
    // `${...}` shape — so it falls through to the inline-secret handling below
    // and stays rejected unless the dev flag is set (R9 / REVIEW FIX #6).
    match crate::env_ref::parse_env_ref(raw) {
        // A MALFORMED reference: the empty `${}`, or a `${NAME}` whose NAME is
        // not portably settable (`${MY-SECRET}`, `${a.b}`), or a
        // multi-placeholder composition. The grammar maps all of them to the
        // empty name, and `resolve_secret_env_var("")` would report
        // `env var '' not set for token_secret` — a message that names neither
        // what the operator wrote nor what to do about it. Say the actual thing
        // instead, and point at the `env:` escape hatch, which keeps its
        // any-non-empty-remainder rule precisely for exotic names.
        Some("") => {
            return Err(ToolkitError::CodeMode(
                "[code_mode] token_secret is a malformed environment reference; a `${VAR}` \
                 reference must name exactly ONE variable matching [A-Za-z0-9_]+ and nothing \
                 else. For a name outside that set, use the `env:NAME` form, which accepts any \
                 non-empty name. (The value is not echoed here.)"
                    .to_string(),
            ))
        },
        Some(var) => return resolve_secret_env_var(var),
        None => {},
    }
    if section.allow_inline_token_secret_for_dev {
        tracing::warn!(
            target: "pmcp_server_toolkit::code_mode",
            "[code_mode] token_secret is inline AND allow_inline_token_secret_for_dev=true; \
             accepting under dev/test exception — NEVER set this flag in a committed \
             production config"
        );
        return Ok(SecretValue::new(raw.as_bytes().to_vec()));
    }
    Err(ToolkitError::Validation(
        ConfigValidationError::InlineSecretRejected,
    ))
}

// =============================================================================
// TKIT-10 — assemble_code_mode_prompt (D-12 / review R2)
// =============================================================================

/// TKIT-10: assemble the code-mode bootstrap prompt body from a connector's
/// [`SqlConnector::schema_text`] + curated `[[database.tables]]` descriptions.
///
/// Per Phase 83 review R2 (BOTH reviewers HIGH severity), this function calls
/// ONLY [`SqlConnector::schema_text`] — never `execute()`, which is deferred
/// to Phase 84. Dialect-aware placeholder GUIDANCE is included even though
/// `translate_placeholders` is deferred, because the LLM still benefits from
/// knowing the eventual binding shape.
///
/// # Output structure
///
/// ```text
/// # Code Mode — {dialect.name()}
///
/// {dialect.placeholder_guidance()}
///
/// ## Schema
///
/// {connector.schema_text()}
///
/// ## Curated Tables
///
/// - `table_a`: description A
/// - `table_b`: description B
/// ```
///
/// The "Curated Tables" section is omitted entirely when
/// `config.database.tables` is empty OR every entry has no `description`.
/// Entries with `description = None` are skipped individually.
///
/// # Errors
///
/// Returns [`ToolkitError::CodeMode`] if `connector.schema_text()` fails.
/// The toolkit does not retry; callers should ensure the connector is ready
/// before assembling.
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::assemble_code_mode_prompt;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
///
/// async fn assemble<C: SqlConnector>(connector: &C, config: &ServerConfig) {
///     let prompt = assemble_code_mode_prompt(connector, config).await.unwrap();
///     assert!(prompt.contains("# Code Mode"));
/// }
/// ```
pub async fn assemble_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String> {
    let dialect = connector.dialect();
    let schema_text = connector
        .schema_text()
        .await
        .map_err(|e| ToolkitError::CodeMode(format!("schema_text failed: {e}")))?;

    let curated = format_curated_tables(config);

    let mut out = String::with_capacity(schema_text.len() + curated.len() + 256);
    out.push_str("# Code Mode — ");
    out.push_str(dialect.name());
    out.push_str("\n\n");
    out.push_str(dialect.placeholder_guidance());
    out.push_str("\n\n## Schema\n\n");
    out.push_str(&schema_text);
    if !curated.is_empty() {
        out.push_str("\n\n## Curated Tables\n\n");
        out.push_str(&curated);
    }
    out.push('\n');
    Ok(out)
}

/// Alias for [`assemble_code_mode_prompt`] satisfying CONN-04's literal naming.
///
/// Identical behavior; both names are valid public surface. Per Phase 84 D-12 +
/// RESEARCH §"Open Questions" Q2 / Landmine #15 the recommendation is an
/// alias-next-to (no deprecation attribute on either name), matching the P83
/// dual-naming precedent (`register_code_mode_tools` vs
/// `code_mode_tools_from_executor`).
///
/// # Errors
///
/// Returns [`ToolkitError::CodeMode`] if `connector.schema_text()` fails —
/// surfaced verbatim from [`assemble_code_mode_prompt`].
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::build_code_mode_prompt;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
///
/// async fn assemble<C: SqlConnector>(connector: &C, config: &ServerConfig) {
///     let prompt = build_code_mode_prompt(connector, config).await.unwrap();
///     assert!(prompt.contains("# Code Mode"));
/// }
/// ```
pub async fn build_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String> {
    assemble_code_mode_prompt(connector, config).await
}

/// File-based counterpart to [`assemble_code_mode_prompt`] — assemble the
/// code-mode prompt body from a `--schema` file's text WITHOUT any live
/// connector introspection (Plan 85-02 Task 3 / D-04 / D-05).
///
/// This is a SYNC fn taking the [`Dialect`] + the already-loaded `schema_text`
/// directly, so it can NEVER trigger a [`SqlConnector::schema_text`] round-trip.
/// For lazy / network-backed non-SQLite connectors that matters: the
/// connector-based [`assemble_code_mode_prompt`] would hit the network at prompt
/// time (breaking SC-1), and it would surface the LIVE schema rather than the
/// admin-redacted `--schema` file. Routing the `--schema` file content through
/// THIS helper makes the file the single source of truth — what's in the file
/// is exactly what the client sees (the D-05 redaction guarantee).
///
/// # Output structure
///
/// Mirrors [`assemble_code_mode_prompt`] except the schema block is preceded by
/// a `# Database Schema` header (REVIEW FIX — Gemini LOW, folded here per D-05;
/// the header text is kept identical to the resource-surface
/// `merge_schema_resource` helper Plan 05 uses, so prompt + resource parity
/// holds):
///
/// ```text
/// # Code Mode — {dialect.name()}
///
/// {dialect.placeholder_guidance()}
///
/// ## Schema
///
/// # Database Schema
///
/// {schema_text}
///
/// ## Curated Tables
///
/// - `table_a`: description A
/// ```
///
/// An empty `schema_text` still produces a valid (non-panicking) prompt with
/// the `# Code Mode` header present.
#[must_use]
pub fn assemble_code_mode_prompt_with_schema(
    schema_text: &str,
    dialect: Dialect,
    config: &ServerConfig,
) -> String {
    const SCHEMA_HEADER: &str = "# Database Schema\n\n";

    let curated = format_curated_tables(config);

    let mut out = String::with_capacity(schema_text.len() + curated.len() + 256);
    out.push_str("# Code Mode — ");
    out.push_str(dialect.name());
    out.push_str("\n\n");
    out.push_str(dialect.placeholder_guidance());
    out.push_str("\n\n## Schema\n\n");
    out.push_str(SCHEMA_HEADER);
    out.push_str(schema_text);
    if !curated.is_empty() {
        out.push_str("\n\n## Curated Tables\n\n");
        out.push_str(&curated);
    }
    out.push('\n');
    out
}

/// Format the `[[database.tables]]` curated descriptions as a Markdown list.
///
/// Entries with no `description` are skipped. Returns an empty string when no
/// described entries exist; callers use that as the signal to omit the whole
/// "Curated Tables" section (keeping the prompt body tight).
fn format_curated_tables(config: &ServerConfig) -> String {
    config
        .database
        .tables
        .iter()
        .filter_map(|t| {
            t.description
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| format!("- `{}`: {}", t.name, d))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// =============================================================================
// Unit tests
// =============================================================================

/// Process-global lock serializing every test that reads or mutates the shared
/// process environment via `std::env::{set_var, remove_var}`.
///
/// Those calls are process-global and not thread-safe, so under the default
/// multi-threaded test runner the env-touching tests in this file's `tests` and
/// `sql_code_executor_tests` modules otherwise interleave and corrupt each
/// other's variables (e.g. an executor build fails to read the `TEST_SECRET_VAR`
/// it just set). Acquire the guard around each synchronous env-op group; NEVER
/// hold it across an `.await` (the `std` `MutexGuard` is `!Send`, and tokio's
/// multi-thread runtime requires the test future to be `Send`).
#[cfg(test)]
mod test_env_guard {
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Lock the process-env mutex, recovering from poisoning so a panicking
    /// test does not cascade-fail its siblings.
    pub(super) fn lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CodeModeLimits, CodeModeSection};

    /// Compile-only assertion that the headline re-exports resolve at the
    /// `code_mode::*` path (TKIT-06 + D-16 + R3).
    #[allow(dead_code)]
    const _RE_EXPORTS_COMPILE: fn() = || {
        let _: Option<Box<dyn CodeExecutor>> = None;
        let _: Option<Box<dyn PolicyEvaluator>> = None;
        let _: Option<ApprovalToken> = None;
        let _: Option<HmacTokenGenerator> = None;
        let _: Option<TokenSecret> = None;
        let _: Option<NoopPolicyEvaluator> = None;
        let _: Option<ValidationPipeline> = None;
        let _: Option<ValidationContext> = None;
        let _: Option<CodeModeConfig> = None;
        let _: Option<AuthorizationDecision> = None;
        let _hash = canonicalize_code;
        let _ctx = compute_context_hash;
        let _h = hash_code;
    };

    /// Lightweight test fixture: a `CodeModeSection` with all required fields
    /// populated for env-style secret resolution.
    fn env_section(var: &str) -> CodeModeSection {
        CodeModeSection {
            enabled: true,
            server_id: Some("test-server".to_string()),
            allow_writes: false,
            allow_deletes: false,
            allow_ddl: false,
            require_limit: false,
            max_limit: Some(1000),
            blocked_tables: vec![],
            sensitive_columns: vec![],
            auto_approve_levels: vec!["low".to_string()],
            token_ttl_seconds: Some(300),
            token_secret: Some(format!("env:{var}")),
            allow_inline_token_secret_for_dev: false,
            limits: Some(CodeModeLimits {
                max_tables_per_query: Some(5),
                max_join_depth: Some(3),
                max_subquery_depth: Some(2),
            }),
        }
    }

    #[test]
    fn build_cm_config_maps_allow_writes() {
        let mut section = env_section("UNUSED");
        section.allow_writes = true;
        let cfg = build_cm_config(&section);
        assert!(
            cfg.sql_allow_writes,
            "unprefixed allow_writes=true must map to sql_allow_writes=true"
        );
        assert!(cfg.enabled);
        assert_eq!(cfg.server_id.as_deref(), Some("test-server"));
        // max_limit → sql_max_rows
        assert_eq!(cfg.sql_max_rows, 1000);
        // token_ttl_seconds → i64
        assert_eq!(cfg.token_ttl_seconds, 300);
    }

    #[test]
    fn build_cm_config_maps_require_limit_true() {
        // VERIFICATION Gap 1: toolkit `require_limit` must flow to the enforced
        // pmcp-code-mode `sql_require_limit` (previously discarded).
        let mut section = env_section("UNUSED");
        section.require_limit = true;
        let cfg = build_cm_config(&section);
        assert!(
            cfg.sql_require_limit,
            "require_limit=true must map to sql_require_limit=true"
        );
    }

    #[test]
    fn build_cm_config_maps_require_limit_false() {
        let mut section = env_section("UNUSED");
        section.require_limit = false;
        let cfg = build_cm_config(&section);
        assert!(
            !cfg.sql_require_limit,
            "require_limit=false must map to sql_require_limit=false"
        );
    }

    #[test]
    fn build_cm_config_propagates_blocked_tables() {
        let mut section = env_section("UNUSED");
        section.blocked_tables = vec!["users".into(), "secrets".into()];
        section.sensitive_columns = vec!["users.password".into()];
        let cfg = build_cm_config(&section);
        assert!(cfg.sql_blocked_tables.contains("users"));
        assert!(cfg.sql_blocked_tables.contains("secrets"));
        assert!(cfg.sql_blocked_columns.contains("users.password"));
    }

    #[test]
    fn resolve_token_secret_env_reference_succeeds() {
        let _env = super::test_env_guard::lock();
        const VAR: &str = "PMCP_TOOLKIT_CODE_MODE_TEST_RESOLVE_ENV";
        // Long enough to satisfy HmacTokenGenerator::MIN_SECRET_LEN (16 bytes).
        std::env::set_var(VAR, "a-test-secret-bytes-16-or-more");
        let section = env_section(VAR);
        let resolved = resolve_token_secret(&section).expect("env resolution must succeed");
        assert_eq!(resolved.expose_secret(), b"a-test-secret-bytes-16-or-more");
        std::env::remove_var(VAR);
    }

    #[test]
    fn resolve_token_secret_inline_without_dev_flag_rejected() {
        // R9 — inline literal + flag absent → InlineSecretRejected.
        let mut section = env_section("UNUSED");
        section.token_secret = Some("raw-string-that-should-be-rejected".to_string());
        section.allow_inline_token_secret_for_dev = false;
        // SecretValue intentionally does not implement Debug (R5 invariant),
        // so we cannot use `expect_err` directly on Result<SecretValue, _>.
        match resolve_token_secret(&section) {
            Ok(_) => panic!("must reject inline literal"),
            Err(ToolkitError::Validation(ConfigValidationError::InlineSecretRejected)) => {},
            Err(other) => panic!("expected InlineSecretRejected, got {other:?}"),
        }
    }

    #[test]
    fn resolve_token_secret_inline_with_dev_flag_accepted() {
        // R9 — inline literal + dev flag → accepted (with tracing::warn).
        let mut section = env_section("UNUSED");
        section.token_secret = Some("a-test-secret-bytes-16-or-more".to_string());
        section.allow_inline_token_secret_for_dev = true;
        let resolved = resolve_token_secret(&section).expect("dev flag must permit inline literal");
        assert_eq!(resolved.expose_secret(), b"a-test-secret-bytes-16-or-more");
    }

    #[test]
    fn resolve_token_secret_empty_env_var_is_set_but_empty_error() {
        let _env = super::test_env_guard::lock();
        // 85-10 / T-85-10-03: a set-but-EMPTY env value must NOT flow to the
        // HMAC layer as a degenerate secret — it surfaces as a clear
        // "set but empty" CodeMode error (env: form).
        const VAR: &str = "PMCP_TOOLKIT_CODE_MODE_TEST_EMPTY_ENV";
        std::env::set_var(VAR, "");
        let section = env_section(VAR);
        let outcome = resolve_token_secret(&section);
        std::env::remove_var(VAR);
        match outcome {
            Ok(_) => panic!("empty env var must error, not yield an empty secret"),
            Err(ToolkitError::CodeMode(msg)) => {
                assert!(
                    msg.contains(VAR) && msg.contains("set but empty"),
                    "error must name the var as set-but-empty, got: {msg}"
                );
            },
            Err(other) => panic!("expected CodeMode 'set but empty', got {other:?}"),
        }
    }

    #[test]
    fn resolve_token_secret_whitespace_env_var_is_set_but_empty_error() {
        let _env = super::test_env_guard::lock();
        // All-whitespace is treated the same as empty (${VAR} form).
        const VAR: &str = "PMCP_TOOLKIT_CODE_MODE_TEST_WS_ENV";
        std::env::set_var(VAR, "   ");
        let mut section = env_section("UNUSED");
        section.token_secret = Some(format!("${{{VAR}}}"));
        let outcome = resolve_token_secret(&section);
        std::env::remove_var(VAR);
        match outcome {
            Ok(_) => panic!("whitespace-only env var must error"),
            Err(ToolkitError::CodeMode(msg)) => {
                assert!(
                    msg.contains(VAR) && msg.contains("set but empty"),
                    "error must name the var as set-but-empty, got: {msg}"
                );
            },
            Err(other) => panic!("expected CodeMode 'set but empty', got {other:?}"),
        }
    }

    #[test]
    fn variables_to_params_maps_object_stripping_colon_prefix() {
        // 85-10 WR-02: a JSON object of name→value becomes (name, value) pairs,
        // with a leading `:` stripped to match the connector's keying.
        let vars = serde_json::json!({ ":name": "Rock", "limit": 5 });
        let mut params = variables_to_params(Some(&vars));
        params.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            params,
            vec![
                ("limit".to_string(), serde_json::json!(5)),
                ("name".to_string(), serde_json::json!("Rock")),
            ]
        );
    }

    #[test]
    fn variables_to_params_none_or_non_object_is_empty() {
        // None / non-object yields an empty slice — the parity execute_code
        // scenario (passes None) is unaffected.
        assert!(variables_to_params(None).is_empty());
        assert!(variables_to_params(Some(&serde_json::json!("not-an-object"))).is_empty());
        assert!(variables_to_params(Some(&serde_json::json!([1, 2, 3]))).is_empty());
    }

    #[test]
    fn resolve_token_secret_missing_env_var_surfaces_error() {
        // Use a var name that is overwhelmingly unlikely to be set in CI.
        let section = env_section("PMCP_TOOLKIT_DEFINITELY_NOT_SET_FOR_TEST");
        // SecretValue has no Debug — pattern-match instead of expect_err.
        match resolve_token_secret(&section) {
            Ok(_) => panic!("missing env var must error"),
            Err(ToolkitError::CodeMode(msg)) => {
                assert!(
                    msg.contains("PMCP_TOOLKIT_DEFINITELY_NOT_SET_FOR_TEST"),
                    "error message must name the missing env var, got: {msg}"
                );
            },
            Err(other) => panic!("expected CodeMode error, got {other:?}"),
        }
    }
}

// =============================================================================
// SHAP-A-01 — SqlCodeExecutor unit tests (Plan 85-02 Task 1)
// =============================================================================

#[cfg(all(test, feature = "sqlite"))]
mod sql_code_executor_tests {
    use super::*;
    use crate::config::{CodeModeSection, ServerConfig, ServerSection};
    use crate::sql::SqliteConnector;

    const TEST_SECRET_VAR: &str = "PMCP_TOOLKIT_SQL_EXECUTOR_TEST_SECRET";

    fn ensure_secret() {
        std::env::set_var(TEST_SECRET_VAR, "executor-test-secret-16-or-more");
    }

    /// A read-only `[code_mode]` config (no writes/deletes/DDL) plus an
    /// in-memory SQLite connector seeded with a single `Artist` row.
    async fn read_only_executor() -> SqlCodeExecutor {
        let connector = SqliteConnector::open_in_memory().expect("open in-memory sqlite");
        connector
            .execute(
                "CREATE TABLE Artist (ArtistId INTEGER PRIMARY KEY, Name TEXT)",
                &[],
            )
            .await
            .expect("create table");
        connector
            .execute(
                "INSERT INTO Artist (ArtistId, Name) VALUES (1, 'AC/DC')",
                &[],
            )
            .await
            .expect("seed row");

        let config = ServerConfig {
            server: ServerSection {
                name: "executor-test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            code_mode: Some(CodeModeSection {
                enabled: true,
                server_id: Some("executor-test".to_string()),
                allow_writes: false,
                allow_deletes: false,
                allow_ddl: false,
                token_secret: Some(format!("env:{TEST_SECRET_VAR}")),
                ..Default::default()
            }),
            ..Default::default()
        };
        // Serialize set-secret + env-read (build) so a concurrent test cannot
        // corrupt the process environment between them. Synchronous — no
        // `.await` inside the locked section (the `std` guard is `!Send`).
        let _env = super::test_env_guard::lock();
        ensure_secret();
        SqlCodeExecutor::new(Arc::new(connector), config).expect("build executor")
    }

    /// Same in-memory connector as [`read_only_executor`], but the `[code_mode]`
    /// config sets `require_limit = true` so a bare SELECT must reject on policy.
    async fn read_only_executor_with_require_limit() -> SqlCodeExecutor {
        let connector = SqliteConnector::open_in_memory().expect("open in-memory sqlite");
        connector
            .execute(
                "CREATE TABLE Artist (ArtistId INTEGER PRIMARY KEY, Name TEXT)",
                &[],
            )
            .await
            .expect("create table");
        connector
            .execute(
                "INSERT INTO Artist (ArtistId, Name) VALUES (1, 'AC/DC')",
                &[],
            )
            .await
            .expect("seed row");

        let config = ServerConfig {
            server: ServerSection {
                name: "executor-test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            code_mode: Some(CodeModeSection {
                enabled: true,
                server_id: Some("executor-test".to_string()),
                allow_writes: false,
                allow_deletes: false,
                allow_ddl: false,
                require_limit: true,
                token_secret: Some(format!("env:{TEST_SECRET_VAR}")),
                ..Default::default()
            }),
            ..Default::default()
        };
        // Serialize set-secret + env-read (build) so a concurrent test cannot
        // corrupt the process environment between them. Synchronous — no
        // `.await` inside the locked section (the `std` guard is `!Send`).
        let _env = super::test_env_guard::lock();
        ensure_secret();
        SqlCodeExecutor::new(Arc::new(connector), config).expect("build executor")
    }

    #[tokio::test]
    async fn read_only_select_returns_rows() {
        let executor = read_only_executor().await;
        let result = executor
            .execute("SELECT ArtistId, Name FROM Artist", None)
            .await
            .expect("read-only SELECT must succeed under a read-only policy");
        // Mirrors production's observable `"rows"` key (REVIEW FIX #6b).
        let rows = result.get("rows").expect("payload has a `rows` key");
        let arr = rows.as_array().expect("`rows` is an array");
        assert_eq!(arr.len(), 1, "one seeded row expected, got {arr:?}");
        assert_eq!(arr[0]["Name"], "AC/DC");
    }

    #[tokio::test]
    async fn require_limit_rejects_bare_select_before_connector() {
        // VERIFICATION Gap 1: with require_limit=true, a no-LIMIT SELECT is
        // rejected on re-validation BEFORE the connector — even though the
        // single seeded row never exceeds any row-count limit.
        let executor = read_only_executor_with_require_limit().await;
        let err = executor
            .execute("SELECT * FROM Artist", None)
            .await
            .expect_err("bare SELECT must be rejected when require_limit=true");
        assert!(
            matches!(err, ExecutionError::BackendError(_)),
            "expected a policy-rejection BackendError, got {err:?}"
        );
        // The table is untouched — proving the rejection is the require_limit
        // policy, not a row-count failure.
        let count = executor
            .connector
            .execute("SELECT COUNT(*) AS n FROM Artist", &[])
            .await
            .expect("count query");
        assert_eq!(count[0]["n"], 1, "row count must be unchanged");
    }

    #[tokio::test]
    async fn require_limit_allows_limited_select() {
        let executor = read_only_executor_with_require_limit().await;
        let result = executor
            .execute("SELECT ArtistId, Name FROM Artist LIMIT 5", None)
            .await
            .expect("a LIMITed SELECT must succeed under require_limit=true");
        let rows = result.get("rows").expect("payload has a `rows` key");
        let arr = rows.as_array().expect("`rows` is an array");
        assert_eq!(arr.len(), 1, "one seeded row expected, got {arr:?}");
    }

    #[tokio::test]
    async fn delete_rejected_before_connector_under_read_only_policy() {
        // allow_deletes=false → re-validation rejects DELETE BEFORE the
        // connector is reached (threat T-85-02-01 / SC-3).
        let executor = read_only_executor().await;
        let err = executor
            .execute("DELETE FROM Artist WHERE ArtistId = 1", None)
            .await
            .expect_err("DELETE must be rejected when allow_deletes=false");
        assert!(
            matches!(err, ExecutionError::BackendError(_)),
            "expected a policy-rejection BackendError, got {err:?}"
        );
        // The row must still be present — proving the connector was never reached.
        let still_there = executor
            .connector
            .execute("SELECT COUNT(*) AS n FROM Artist", &[])
            .await
            .expect("count query");
        assert_eq!(still_there[0]["n"], 1, "DELETE must not have run");
    }

    #[tokio::test]
    async fn ddl_rejected_under_read_only_policy() {
        // allow_ddl=false → re-validation rejects DROP TABLE.
        let executor = read_only_executor().await;
        let err = executor
            .execute("DROP TABLE Artist", None)
            .await
            .expect_err("DROP must be rejected when allow_ddl=false");
        assert!(matches!(err, ExecutionError::BackendError(_)));
    }

    #[tokio::test]
    async fn malformed_sql_returns_err_never_panics() {
        let executor = read_only_executor().await;
        let result = executor.execute("SELEC nonsense FRM", None).await;
        assert!(
            result.is_err(),
            "malformed SQL must surface an Err, never panic"
        );
    }

    #[tokio::test]
    async fn execute_binds_variables_input() {
        // 85-10 WR-02 / T-85-10-01: the schema-advertised `variables` input is
        // BOUND as named params (not silently dropped), so a `WHERE Name = :name`
        // resolves against the seeded row.
        let executor = read_only_executor().await;
        let vars = serde_json::json!({ ":name": "AC/DC" });
        let result = executor
            .execute(
                "SELECT ArtistId FROM Artist WHERE Name = :name",
                Some(&vars),
            )
            .await
            .expect("bound variable must resolve the WHERE clause");
        let rows = result.get("rows").expect("payload has a `rows` key");
        let arr = rows.as_array().expect("`rows` is an array");
        assert_eq!(arr.len(), 1, "the bound :name must match the seeded row");
        assert_eq!(arr[0]["ArtistId"], 1);
    }

    #[tokio::test]
    async fn execute_empty_variables_is_unaffected() {
        // An empty variables map binds nothing — identical to today's None path.
        let executor = read_only_executor().await;
        let empty = serde_json::json!({});
        let result = executor
            .execute("SELECT ArtistId, Name FROM Artist", Some(&empty))
            .await
            .expect("empty variables must behave exactly like None");
        let arr = result["rows"].as_array().expect("`rows` array");
        assert_eq!(arr.len(), 1);
    }

    #[tokio::test]
    async fn pipeline_cached_at_construction_not_reread_per_execute() {
        // 85-10 IN-01 / T-85-10-03: the pipeline is built ONCE in `new`, so a
        // SECOND execute does NOT re-resolve the token_secret env var. Remove the
        // env var after construction — the executor must STILL succeed (proving
        // it did not re-read the now-missing secret).
        let executor = read_only_executor().await;
        // First execute (baseline) succeeds.
        executor
            .execute("SELECT ArtistId FROM Artist LIMIT 1", None)
            .await
            .expect("first execute succeeds");
        // Remove the secret the pipeline was built from. Each discrete env
        // mutation is serialized under the shared lock (held only across the
        // synchronous call, never across the `.await`s above/below).
        {
            let _env = super::test_env_guard::lock();
            std::env::remove_var(TEST_SECRET_VAR);
        }
        // Second execute STILL succeeds — the cached pipeline never re-reads env.
        let result = executor
            .execute("SELECT ArtistId FROM Artist LIMIT 1", None)
            .await
            .expect("second execute must succeed from the cached pipeline");
        // Restore for any sibling tests sharing the process env.
        {
            let _env = super::test_env_guard::lock();
            ensure_secret();
        }
        assert!(result.get("rows").is_some());
    }
}

// =============================================================================
// TKIT-10 — assemble_code_mode_prompt integration tests
// =============================================================================

#[cfg(test)]
mod tkit10_tests {
    use super::*;
    use crate::config::{DatabaseSection, DatabaseTableDecl, ServerConfig, ServerSection};
    use crate::sql::{Dialect, MockSqlConnector};

    fn make_cfg(tables: Vec<DatabaseTableDecl>) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            database: DatabaseSection {
                tables,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn assemble_includes_schema_text_and_dialect_name() {
        let connector = MockSqlConnector {
            dialect: Dialect::Postgres,
            schema: "CREATE TABLE users (id SERIAL PRIMARY KEY);".to_string(),
        };
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            prompt.contains("# Code Mode — PostgreSQL"),
            "prompt missing dialect header: {prompt}"
        );
        assert!(
            prompt.contains("CREATE TABLE users"),
            "prompt missing schema body: {prompt}"
        );
        assert!(
            prompt.contains("$1"),
            "Postgres guidance should mention $1: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_includes_curated_descriptions() {
        let connector = MockSqlConnector {
            dialect: Dialect::Athena,
            schema: "(see Glue catalog)".to_string(),
        };
        let cfg = make_cfg(vec![
            DatabaseTableDecl {
                name: "users".to_string(),
                description: Some("App users".to_string()),
            },
            DatabaseTableDecl {
                name: "orders".to_string(),
                description: Some("Customer orders".to_string()),
            },
        ]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            prompt.contains("## Curated Tables"),
            "prompt missing curated header: {prompt}"
        );
        assert!(
            prompt.contains("`users`: App users"),
            "prompt missing users description: {prompt}"
        );
        assert!(
            prompt.contains("`orders`: Customer orders"),
            "prompt missing orders description: {prompt}"
        );
        // Athena uses ? placeholders, not $1
        assert!(
            prompt.contains("Amazon Athena"),
            "prompt missing Athena dialect name: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_omits_curated_section_when_tables_empty() {
        let connector = MockSqlConnector {
            dialect: Dialect::Sqlite,
            schema: "CREATE TABLE t (id INTEGER PRIMARY KEY);".to_string(),
        };
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            !prompt.contains("## Curated Tables"),
            "empty [[database.tables]] must omit curated section: {prompt}"
        );
        assert!(
            prompt.contains("SQLite"),
            "prompt missing SQLite dialect name: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_skips_tables_without_descriptions() {
        // A described entry mixed with an undescribed one — only the described
        // row should render. Curated section still emits because at least one
        // row qualifies.
        let connector = MockSqlConnector {
            dialect: Dialect::MySql,
            schema: "CREATE TABLE t (id INT);".to_string(),
        };
        let cfg = make_cfg(vec![
            DatabaseTableDecl {
                name: "with_desc".to_string(),
                description: Some("has description".to_string()),
            },
            DatabaseTableDecl {
                name: "no_desc".to_string(),
                description: None,
            },
        ]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(prompt.contains("`with_desc`: has description"));
        assert!(
            !prompt.contains("`no_desc`"),
            "undescribed table must not appear in curated section: {prompt}"
        );
    }

    // =========================================================================
    // assemble_code_mode_prompt_with_schema — file-based prompt seam (Task 3)
    // =========================================================================

    #[test]
    fn with_schema_includes_header_dialect_schema_and_curated() {
        let cfg = make_cfg(vec![DatabaseTableDecl {
            name: "Artist".to_string(),
            description: Some("Musical artists".to_string()),
        }]);
        let schema = "CREATE TABLE Artist (ArtistId INTEGER PRIMARY KEY, Name TEXT);";
        let prompt = assemble_code_mode_prompt_with_schema(schema, Dialect::Sqlite, &cfg);

        assert!(
            prompt.contains("# Code Mode"),
            "missing code-mode header: {prompt}"
        );
        assert!(prompt.contains("SQLite"), "missing dialect name: {prompt}");
        assert!(
            prompt.contains("# Database Schema"),
            "missing schema-resource header: {prompt}"
        );
        assert!(
            prompt.contains(schema),
            "schema text must appear verbatim: {prompt}"
        );
        assert!(
            prompt.contains("`Artist`: Musical artists"),
            "curated table description must appear: {prompt}"
        );
    }

    /// The helper is a SYNC fn — this test calls it from a non-async context,
    /// which only compiles because it never awaits a connector (proving it
    /// cannot trigger a live `schema_text()`).
    #[test]
    fn with_schema_is_sync_and_uses_passed_dialect() {
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt_with_schema(
            "CREATE TABLE t (id INT);",
            Dialect::Postgres,
            &cfg,
        );
        assert!(
            prompt.contains("# Code Mode — PostgreSQL"),
            "passed dialect must drive the header: {prompt}"
        );
        // Postgres placeholder guidance mentions $1 — proves dialect param is used.
        assert!(prompt.contains("$1"), "Postgres guidance missing: {prompt}");
        // No curated section when [[database.tables]] is empty.
        assert!(
            !prompt.contains("## Curated Tables"),
            "empty tables must omit curated section: {prompt}"
        );
    }

    #[test]
    fn with_schema_empty_text_still_has_header() {
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt_with_schema("", Dialect::MySql, &cfg);
        assert!(
            prompt.contains("# Code Mode — MySQL"),
            "empty schema must still produce a valid prompt with the header: {prompt}"
        );
        assert!(
            prompt.contains("# Database Schema"),
            "schema-resource header present even for empty schema: {prompt}"
        );
    }
}

// =============================================================================
// Plan 90-10 — per-request executor seam + OpenAPI per-request wiring tests
// =============================================================================

#[cfg(all(test, feature = "openapi-code-mode"))]
mod per_request_executor_tests {
    use super::*;
    use crate::config::{CodeModeSection, ServerConfig, ServerSection};
    use crate::http::auth::{create_passthrough_auth_provider, AuthConfig};
    use pmcp::server::auth::AuthContext;

    /// A passthrough-configured `HttpCodeExecutor` over a fixed base_url.
    fn passthrough_base() -> HttpCodeExecutor {
        let auth = create_passthrough_auth_provider(
            &AuthConfig::OAuthPassthrough {
                target_header: "Authorization".to_string(),
                required: true,
            },
            None,
        )
        .expect("passthrough auth provider");
        HttpCodeExecutor::new(
            reqwest::Client::new(),
            "https://api.example".to_string(),
            auth,
        )
    }

    fn extra_with_token(token: Option<&str>) -> pmcp::RequestHandlerExtra {
        let ctx = AuthContext {
            subject: "s".to_string(),
            scopes: vec![],
            claims: std::collections::HashMap::new(),
            token: token.map(str::to_string),
            client_id: None,
            expires_at: None,
            authenticated: token.is_some(),
        };
        pmcp::RequestHandlerExtra::default().with_auth_context(Some(ctx))
    }

    #[test]
    fn request_executor_from_extra_threads_present_token() {
        // Plan 90-10 / OAPI-03 / OAPI-05: the captured inbound token reaches the
        // per-request executor's inbound_token field.
        let base = passthrough_base();
        assert_eq!(
            base.inbound_token_for_test(),
            None,
            "base executor starts with no inbound token"
        );
        let extra = extra_with_token(Some("Bearer client-tok"));
        let scoped = request_executor_from_extra(&base, &extra);
        assert_eq!(
            scoped.inbound_token_for_test(),
            Some("Bearer client-tok"),
            "the captured inbound token must be threaded into the per-request executor"
        );
    }

    #[test]
    fn request_executor_from_extra_no_token_yields_none() {
        let base = passthrough_base();
        let extra = extra_with_token(None);
        let scoped = request_executor_from_extra(&base, &extra);
        assert_eq!(
            scoped.inbound_token_for_test(),
            None,
            "an extra carrying no token must yield an executor with inbound_token None"
        );
        // No auth context at all also yields None (never panics).
        let bare = request_executor_from_extra(&base, &pmcp::RequestHandlerExtra::default());
        assert_eq!(bare.inbound_token_for_test(), None);
    }

    fn cfg_with_code_mode() -> ServerConfig {
        std::env::set_var(
            "PMCP_TOOLKIT_90_10_HTTP_SECRET",
            "per-request-test-secret-16-or-more",
        );
        ServerConfig {
            server: ServerSection {
                name: "http-cm".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            code_mode: Some(CodeModeSection {
                enabled: true,
                server_id: Some("http-cm".to_string()),
                token_secret: Some("env:PMCP_TOOLKIT_90_10_HTTP_SECRET".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn http_tools_register_validate_and_execute_with_per_request_source() {
        // code_mode_http_tools_from_executor builds the ExecuteCodeHandler over
        // the PerRequestHttp source (constructed without panic over a passthrough
        // executor) and registers both Code-Mode tools.
        let _env = super::test_env_guard::lock();
        let cfg = cfg_with_code_mode();
        let builder = pmcp::Server::builder().name("http-cm").version("0.1.0");
        let builder = code_mode_http_tools_from_executor(
            builder,
            &cfg,
            passthrough_base(),
            ExecutionConfig::default(),
            ValidationFlavor::OpenApi,
        )
        .expect("OpenAPI per-request code-mode wiring must build");
        let server = builder.build().expect("server builds");
        assert!(
            server.get_tool("validate_code").is_some(),
            "validate_code registered"
        );
        assert!(
            server.get_tool("execute_code").is_some(),
            "execute_code registered"
        );
        std::env::remove_var("PMCP_TOOLKIT_90_10_HTTP_SECRET");
    }

    #[test]
    fn http_tools_no_op_when_code_mode_absent() {
        let cfg = ServerConfig {
            server: ServerSection {
                name: "no-cm".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let builder = pmcp::Server::builder().name("no-cm").version("0.1.0");
        let builder = code_mode_http_tools_from_executor(
            builder,
            &cfg,
            passthrough_base(),
            ExecutionConfig::default(),
            ValidationFlavor::OpenApi,
        )
        .expect("no-op when [code_mode] absent");
        let server = builder.build().expect("server builds");
        assert!(
            server.get_tool("execute_code").is_none(),
            "no tools without [code_mode]"
        );
    }
}

#[cfg(all(test, feature = "sqlite", feature = "openapi-code-mode"))]
mod sql_static_source_tests {
    use super::*;
    use crate::config::{CodeModeSection, ServerConfig, ServerSection};
    use crate::sql::SqliteConnector;

    #[test]
    fn sql_path_registers_static_source_unchanged() {
        // The SQL path via code_mode_tools_from_executor still builds the
        // ExecuteCodeHandler with the Static source (SqlCodeExecutor) — Plan
        // 90-10 must not change the SQL wiring.
        let _env = super::test_env_guard::lock();
        std::env::set_var(
            "PMCP_TOOLKIT_90_10_SQL_SECRET",
            "sql-static-test-secret-16-or-more",
        );
        let connector = SqliteConnector::open_in_memory().expect("sqlite");
        let cfg = ServerConfig {
            server: ServerSection {
                name: "sql-cm".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            code_mode: Some(CodeModeSection {
                enabled: true,
                server_id: Some("sql-cm".to_string()),
                token_secret: Some("env:PMCP_TOOLKIT_90_10_SQL_SECRET".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let executor: Arc<dyn CodeExecutor> =
            Arc::new(SqlCodeExecutor::new(Arc::new(connector), cfg.clone()).expect("executor"));
        let builder = pmcp::Server::builder().name("sql-cm").version("0.1.0");
        let builder = code_mode_tools_from_executor(builder, &cfg, executor, ValidationFlavor::Sql)
            .expect("SQL code-mode wiring must build");
        let server = builder.build().expect("server builds");
        assert!(server.get_tool("validate_code").is_some());
        assert!(server.get_tool("execute_code").is_some());
        std::env::remove_var("PMCP_TOOLKIT_90_10_SQL_SECRET");
    }
}
