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
        source: tool_handlers::ExecSource::PerRequestHttp {
            // Phase 128 E1: label the executor with the tool it serves, so a
            // registered `RequestPolicy` can attribute an outbound request. One
            // `execute_code` call may issue many requests; they all carry this
            // label.
            base: base.with_tool_label("execute_code"),
            exec_config,
        },
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
    /// The operator's parsed OpenAPI document, when one was supplied
    /// (Phase 128 D4(b)).
    ///
    /// `None` on every executor built by [`HttpCodeExecutor::new`], which is what
    /// keeps that constructor's signature unchanged. An `Arc` rather than an owned
    /// document because the same parse is also served verbatim as the `api_schema`
    /// resource, and the spec can be large — one allocation is shared, never
    /// cloned. Read ONLY by
    /// [`placeholder_rules`](pmcp_code_mode::HttpExecutor::placeholder_rules).
    schema: Option<Arc<crate::http::OpenApiSchema>>,
    /// The E1 outbound-request policy, when one was registered (Phase 128).
    ///
    /// `None` on every executor built by [`HttpCodeExecutor::new`], which keeps
    /// that constructor's signature unchanged and keeps the no-policy request
    /// path allocation-free.
    policy: Option<Arc<dyn crate::policy::RequestPolicy>>,
    /// The MCP tool this executor serves, for [`crate::policy::OutboundRequest`]'s
    /// `tool` field (Phase 128 E1).
    ///
    /// `HttpExecutor::execute_request` is a `pmcp-code-mode` trait method and
    /// carries no tool name, so the label is attached where a PER-TOOL executor is
    /// minted: a script tool's own `[[tools]]` `name` at synthesis, and
    /// `execute_code` for the generic Code Mode tool. `Arc<str>` because the
    /// executor is cloned per request.
    tool_label: Option<Arc<str>>,
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
            schema: None,
            policy: None,
            tool_label: None,
        }
    }

    /// Label this executor with the MCP tool it serves, so an E1 policy is told
    /// which `tools/call` an outbound request came from (Phase 128).
    ///
    /// Attach it where a PER-TOOL executor is minted — `ScriptToolHandler::new`
    /// for a script tool, `code_mode_http_tools_from_executor` for `execute_code`.
    /// On the Code Mode surface one `tools/call` may issue many outbound requests
    /// and they all carry this same label.
    #[must_use]
    pub fn with_tool_label(mut self, tool: impl AsRef<str>) -> Self {
        self.tool_label = Some(Arc::from(tool.as_ref()));
        self
    }

    /// The MCP tool this executor serves, or `""` when it carries no label (a
    /// caller driving the executor directly, with no tool to name).
    #[must_use]
    pub fn tool_label(&self) -> &str {
        self.tool_label.as_deref().unwrap_or("")
    }

    /// Consult the registered E1 policy, if any, for one already-assembled
    /// outbound request (Phase 128).
    ///
    /// The mirror of `http::HttpClient`'s helper of the same name. Its own
    /// function so `execute_request` keeps ONE added statement and stays under
    /// the cognitive-complexity 25 gate, and returns immediately when no policy
    /// is registered.
    async fn run_request_policy(
        &self,
        method: &str,
        path: &str,
        query: &[(String, String)],
        body: Option<&serde_json::Value>,
    ) -> std::result::Result<(), ExecutionError> {
        let Some(policy) = self.policy.as_ref() else {
            return Ok(());
        };
        let req = crate::policy::OutboundRequest::new(self.tool_label(), method, path, query, body);
        policy
            .check(&req)
            .await
            .map_err(|refusal| ExecutionError::RuntimeError {
                message: format!("outbound request refused by policy: {refusal}"),
            })
    }

    /// Attach the E1 [`crate::policy::RequestPolicy`] consulted before every
    /// outbound request this executor makes (Phase 128).
    ///
    /// Cheap clone-with-builder, the same shape as
    /// [`with_inbound_token`](Self::with_inbound_token).
    ///
    /// # Call it BEFORE the executor fans out
    ///
    /// Both HTTP surfaces run on ONE executor (D-02) — script tools take a clone
    /// and Code Mode takes the original — so a clone taken before this builder
    /// runs is permanently ungoverned. The same constraint
    /// [`with_schema`](Self::with_schema) documents, for the same reason.
    #[must_use]
    pub fn with_request_policy(mut self, policy: Arc<dyn crate::policy::RequestPolicy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Whether this executor consults an E1 policy before sending.
    ///
    /// Public for the same reason [`has_schema`](Self::has_schema) is: the wiring
    /// lives in a different crate, so a registered-but-unreached policy must be
    /// observable from outside rather than only from a `#[cfg(test)]` accessor.
    #[must_use]
    pub fn has_request_policy(&self) -> bool {
        self.policy.is_some()
    }

    /// Attach the operator's parsed OpenAPI document, so a path placeholder can be
    /// narrowed by what the spec DECLARES for it (Phase 128 D4(b)).
    ///
    /// Cheap clone-with-builder, the same shape as
    /// [`with_inbound_token`](Self::with_inbound_token): the `Arc` is shared with
    /// the `api_schema` resource rather than the document being duplicated.
    ///
    /// # Call it BEFORE the executor fans out
    ///
    /// Both HTTP surfaces run on ONE executor (D-02) — script tools take a clone
    /// and Code Mode takes the original. A clone taken before this builder runs is
    /// permanently unnarrowed, so the call has to precede both fan-out sites. The
    /// production wiring is `pmcp-openapi-server`'s `build_server`, the only place
    /// the executor and the parsed spec are both in scope.
    #[must_use]
    pub fn with_schema(mut self, schema: Arc<crate::http::OpenApiSchema>) -> Self {
        warn_if_narrowing_unavailable();
        self.schema = Some(schema);
        self
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

    /// Whether this executor carries an OpenAPI document, and therefore whether a
    /// path placeholder can be narrowed by a spec DECLARATION (Phase 128 D4(b)).
    ///
    /// `false` never means "unchecked": a spec-less executor still applies the
    /// unconditional character floor and the always-on length cap to every
    /// placeholder value. It means only that no ADDITIONAL declared narrowing is
    /// available.
    ///
    /// Public, and deliberately so. T-128-36c is the risk that `with_schema` gets
    /// wired to a `#[cfg(test)]` helper — or applied after the executor has already
    /// fanned out — leaving the production binary unnarrowed while every test
    /// passes. The wiring lives in a DIFFERENT crate (`pmcp-openapi-server`'s
    /// `build_server`), so a `#[cfg(test)]` accessor could not prove it from there.
    /// This is a read-only boolean over a private field; it exposes nothing about
    /// the document.
    #[must_use]
    pub fn has_schema(&self) -> bool {
        self.schema.is_some()
    }

    /// Test-only accessor for the per-request captured token, so unit tests can
    /// assert [`request_executor_from_extra`] threads the inbound token (the
    /// field is otherwise private — Plan 90-10).
    #[cfg(test)]
    pub(crate) fn inbound_token_for_test(&self) -> Option<&str> {
        self.inbound_token.as_deref()
    }

    /// Render a JSON scalar for GET-query substitution (strings unquoted),
    /// REJECTING non-scalar values (WR-03 / GAP 4).
    ///
    /// This is the `code_mode` counterpart of [`crate::http::client`]'s
    /// `render_scalar`; both HTTP surfaces apply the SAME decided rule. Because
    /// the `Parameter` model carries no OpenAPI `style`/`explode`/`type` hint,
    /// the rule is uniform: a scalar (`String`, `Number`, `Bool`, `Null`)
    /// renders to a bare string (`Null` → `"null"`, preserving prior behavior);
    /// an `Object` or `Array` in a GET-query field is rejected rather than
    /// silently JSON-stringified into the URL.
    ///
    /// # Scope after Phase 128 D-09
    ///
    /// This is now reached ONLY from step (4) — the remaining-body-as-query-params
    /// step. The `{path}` substitution half moved up to
    /// `pmcp_code_mode::PlanExecutor`, which applies the identical rule
    /// (`render_path_scalar`) and then additionally floors the rendered value
    /// through `validate_path_placeholder`. The sibling `resolve_path` helper that
    /// used to live here was deleted with step (1) rather than left as a
    /// caller-less function.
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

/// The spec-declared narrowing for ONE layer-2 `{param}` value (Phase 128 D4(b)).
///
/// A free helper, not an inline block, for two reasons: it keeps the trait method
/// trivially under the cog-25 gate (SP-4), and it lets the `input-validation`-off
/// build be a SIBLING FUNCTION with its own doc rather than a `cfg` arm buried in
/// the impl.
///
/// The lookup is an O(1) index hit. [`OpenApiSchema`](crate::http::OpenApiSchema)
/// indexes operations by `(path, METHOD)`, which is why `method` is part of the
/// trait signature: `GET /things/{id}` and `DELETE /things/{id}` are two
/// operations that may declare different constraints for the same `id`, and a
/// `(path_template, param)` signature could only scan linearly or narrow from the
/// wrong operation.
///
/// Only PATH-position parameters are consulted. A query-position namesake
/// describes a different part of the request and must not narrow a path
/// placeholder.
///
/// # What a MISS costs
///
/// No schema, no matching operation, or no matching PATH parameter returns
/// [`PlaceholderRules::default()`](pmcp_code_mode::PlaceholderRules) — which
/// RETAINS the unconditional character floor and the always-on
/// 256-code-point cap, and LOSES the spec's additional narrowing. That is a real
/// reduction, not a no-op: a Code Mode script writing `/users/{alias}` against a
/// spec that declares `/users/{id}` reaches the SAME endpoint while the declared
/// `pattern` silently disappears, because the lookup is by exact template text.
///
/// Three things bound that, and they are all the bound there is:
///
/// 1. the sentence above, so the cost is stated rather than described as harmless;
/// 2. [`log_spec_lookup_miss`], a `tracing::debug!` fired once per
///    `(method, template)` pair, naming the method and the template and never a
///    value — so a drifted template produces a signal instead of silence;
/// 3. `ServerConfig::lint_against_spec`, which refuses the CONFIGURED case before
///    deploy. Its bound, stated: it covers a template written in the config. A
///    template a Code Mode script COMPOSES at runtime is not visible at config
///    time, which is why (2) exists as well.
///
/// Template canonicalization is deliberately NOT attempted. Normalizing `{alias}`
/// to `{id}` requires knowing the two denote the same parameter, which only the
/// spec's own path can establish — so a canonicalizer either re-derives the exact
/// match it was meant to replace, or guesses, and a wrong guess narrows from
/// ANOTHER parameter's declared rules. That can refuse a legitimate value under a
/// rule the caller's endpoint does not carry, which is strictly worse than not
/// narrowing.
///
/// This function builds [`PlaceholderRules`](pmcp_code_mode::PlaceholderRules) and
/// nothing else. It evaluates no pattern of its own: there is exactly one regex
/// path in this phase and it lives in core, which is what makes a placeholder
/// `pattern` and an `inputSchema` `pattern` resolve the whitespace shorthand
/// identically.
#[cfg(all(feature = "openapi-code-mode", feature = "input-validation"))]
fn spec_placeholder_rules<'a>(
    schema: Option<&'a crate::http::OpenApiSchema>,
    method: &str,
    path_template: &str,
    param: &str,
) -> pmcp_code_mode::PlaceholderRules<'a> {
    let default = pmcp_code_mode::PlaceholderRules::default();
    let Some(schema) = schema else {
        return default;
    };
    let Some(operation) = schema.operation_for(path_template, method) else {
        log_spec_lookup_miss(method, path_template);
        return default;
    };
    operation
        .path_parameters()
        .into_iter()
        .find(|p| p.name == param)
        .map_or(default, |p| p.placeholder_rules())
}

/// Report a `(method, path_template)` pair the spec does not carry, ONCE.
///
/// At `debug!` rather than `warn!` because a Code Mode script may legitimately
/// address a long-tail endpoint the operator's spec omits, so this is diagnostic
/// signal and not an error. It names only author-written text — the method and the
/// template — and never a placeholder value.
#[cfg(all(feature = "openapi-code-mode", feature = "input-validation"))]
fn log_spec_lookup_miss(method: &str, path_template: &str) {
    /// Bound on the distinct pairs remembered.
    ///
    /// A Code Mode script composes its template at RUNTIME, so an unbounded memo
    /// is an unbounded allocation driven by caller-influenced input. Past the
    /// bound the LOG goes quiet rather than the process growing: a server that has
    /// already produced this many distinct misses has a configuration problem the
    /// first entries already named. Enforcement is unaffected either way — the
    /// floor and the cap never depend on this memo.
    const MAX_REMEMBERED: usize = 64;

    static SEEN: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashSet<(String, String)>>,
    > = std::sync::OnceLock::new();

    let Ok(mut seen) = SEEN
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        .lock()
    else {
        return;
    };
    if seen.len() >= MAX_REMEMBERED || !seen.insert((method.to_string(), path_template.to_string()))
    {
        return;
    }
    drop(seen);
    tracing::debug!(
        target: "pmcp_server_toolkit::code_mode",
        method = method,
        path_template = path_template,
        "no OpenAPI operation matches this (method, path template) pair: every placeholder \
         value still faces the unconditional character floor and the always-on length cap, \
         and the spec's ADDITIONAL narrowing is NOT applied. A template written in the \
         config is reported before deploy by ServerConfig::lint_against_spec; a template a \
         Code Mode script composes at runtime can only be reported here."
    );
}

/// The `input-validation`-off half of the spec narrowing.
///
/// `Parameter::placeholder_rules` — the accessor that names the core
/// `PlaceholderRules` type — is gated on the toolkit's `input-validation` feature,
/// so on a build without it there is no declared-rules accessor to read and the
/// spec contributes no narrowing.
///
/// **This is not the floor being switched off.** The floor and the cap run inside
/// `pmcp_code_mode::PlanExecutor`, which depends on `pmcp/schema-validation`
/// unconditionally; they are not behind this feature and this feature cannot turn
/// them off. What IS off is the spec's additional narrowing — and
/// [`warn_if_narrowing_unavailable`] says so once, at the moment an operator
/// supplies a spec and would otherwise believe it was being enforced.
#[cfg(all(feature = "openapi-code-mode", not(feature = "input-validation")))]
fn spec_placeholder_rules<'a>(
    schema: Option<&'a crate::http::OpenApiSchema>,
    method: &str,
    path_template: &str,
    param: &str,
) -> pmcp_code_mode::PlaceholderRules<'a> {
    let _ = (schema, method, path_template, param);
    pmcp_code_mode::PlaceholderRules::default()
}

/// No-op on a build that HAS `input-validation`: the narrowing is available, so
/// there is no opt-out to report. The sibling below is the half that speaks.
#[cfg(all(feature = "openapi-code-mode", feature = "input-validation"))]
fn warn_if_narrowing_unavailable() {}

/// Report, ONCE, that a supplied spec cannot narrow on this build.
///
/// An enforcement that is off must never read as on. An operator who passes
/// `--spec` has asked for the spec's declarations to be applied; on a build
/// without `input-validation` they are not, and this is the only moment at which
/// that intent is observable.
#[cfg(all(feature = "openapi-code-mode", not(feature = "input-validation")))]
fn warn_if_narrowing_unavailable() {
    static WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if WARNED.set(()).is_ok() {
        tracing::warn!(
            target: "pmcp_server_toolkit::code_mode",
            "an OpenAPI spec was supplied but this build has the toolkit's \
             `input-validation` feature OFF, so a path placeholder gets the unconditional \
             character floor and the always-on length cap and NOT the spec's declared \
             pattern/maxLength narrowing. Rebuild with `input-validation` (it is in the \
             toolkit's default feature set) to apply the declarations."
        );
    }
}

#[cfg(feature = "openapi-code-mode")]
#[pmcp_code_mode::async_trait]
impl pmcp_code_mode::HttpExecutor for HttpCodeExecutor {
    /// Narrow a layer-2 `{param}` value by what the carried OpenAPI document
    /// DECLARES for it (Phase 128 D4(b)).
    ///
    /// Delegates to the private `spec_placeholder_rules` helper in this module,
    /// whose rustdoc states exactly what a schema/operation/parameter MISS costs —
    /// the floor and the cap are retained, the spec's narrowing is lost — and what
    /// bounds that loss. Named in plain backticks rather than as an intra-doc link
    /// because this method is public and the helper is private, which rustdoc
    /// (correctly) warns about.
    fn placeholder_rules(
        &self,
        method: &str,
        path_template: &str,
        param: &str,
    ) -> pmcp_code_mode::PlaceholderRules<'_> {
        spec_placeholder_rules(self.schema.as_deref(), method, path_template, param)
    }

    async fn execute_request(
        &self,
        method: &str,
        path: pmcp_code_mode::ResolvedPath<'_>,
        body: Option<serde_json::Value>,
    ) -> std::result::Result<serde_json::Value, ExecutionError> {
        let path = path.as_str();
        let upper = method.to_uppercase();
        let is_get_like = matches!(upper.as_str(), "GET" | "HEAD" | "OPTIONS");

        // (1) REMOVED in Phase 128 (D-09). Placeholder resolution used to happen
        //     here, and that is exactly what made this executor — and every other
        //     `HttpExecutor` implementor, in this repo and out of it — blind BY
        //     CONSTRUCTION to what it was about to send: a decorator wrapping the
        //     public trait saw only the template the script wrote, never the
        //     substituted values, so a placeholder carrying a query separator
        //     became a different endpoint with nothing in a position to notice.
        //
        //     `pmcp_code_mode::PlanExecutor` now resolves BOTH layers and checks
        //     the composed result before dispatch, so `path` arrives as a
        //     `ResolvedPath` with every placeholder already substituted and
        //     checked, and `body` already has the path-consumed keys removed.
        //     Resolving again here would be a double-resolution bug.
        let resolved_path = path;
        let remaining_body = body;

        // (2) Shared join_url helper (Pitfall 2 — preserves an API-Gateway
        //     stage prefix; it does NOT use the RFC-3986 path-replacing join).
        //     join_url does the base+path CONCAT; we still parse the result to
        //     append query pairs because reqwest 0.13 gates
        //     RequestBuilder::query behind a `query` feature the toolkit
        //     deliberately does not enable (Plan 01 Rule 1).
        let url = crate::http::join_url(&self.base_url, resolved_path);

        // (2a) Phase 128 — the NON-AUTH half of step (4) moved ABOVE the E1 hook.
        //      Step (4) used to run entirely after `auth.apply` at (3), which put
        //      the remaining-body-to-query conversion after any hook placed before
        //      auth. A policy documented to inspect the query pairs would then have
        //      inspected an EMPTY slice while the pairs about to be sent still sat
        //      in `body` — a security hook that is present, documented and blind,
        //      which is worse than an absent one. D-12 is preserved exactly: only
        //      the AUTH-supplied query additions stay behind the hook, so the
        //      credential's contribution is still invisible to the policy.
        //
        //      Nothing is renumbered; the auth-supplied pairs are appended at (3a).
        let mut query_params: Vec<(String, String)> = Vec::new();
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

        // (2b) Phase 128 E1 / D-12 — the outbound-policy hook. AFTER `join_url` so
        //      the policy sees the URL as it will be sent, and BEFORE `auth.apply`
        //      so no credential exists yet in `headers` / `query`. A refusal returns
        //      before auth and before the send. The mirror of the curated surface's
        //      hook in `http/client.rs::execute_inner`.
        self.run_request_policy(&upper, &url, &query_params, request_body.as_ref())
            .await?;

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

        // (3a) The auth-supplied query additions — an API-key-in-query credential —
        //      join the pairs AFTER the hook, which is what keeps them invisible to
        //      the policy.
        query_params.extend(auth_query);

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

// =============================================================================
// Phase 128 D4(b) — the `placeholder_rules` override on `HttpCodeExecutor`
// =============================================================================

/// The spec-narrowing half of D4(b) on the Code Mode surface.
///
/// Plan 05 moved placeholder resolution ahead of dispatch and left
/// `HttpExecutor::placeholder_rules` default-implemented, because `PlanExecutor`
/// has no access to an OpenAPI document. These rows prove the executor that DOES
/// own the document supplies the narrowing, and — the load-bearing half — that an
/// executor without one is still floored and capped.
///
/// Nine of the rows assert what a MISS does, because a miss is the shape a reader
/// most easily mistakes for "no checks".
#[cfg(all(test, feature = "openapi-code-mode", feature = "input-validation"))]
mod placeholder_rules_override {
    use super::HttpCodeExecutor;
    use crate::http::auth::{create_auth_provider, AuthConfig};
    use crate::http::OpenApiSchema;
    use pmcp_code_mode::HttpExecutor;
    use std::sync::Arc;

    /// A spec declaring a NARROW pattern on `GET /things/{id}`, a DIFFERENT
    /// pattern on `DELETE /things/{id}` (so the method is provably load-bearing),
    /// a `maxLength`, and `allowReserved: true` on a third path parameter (D-11).
    const SPEC: &str = r#"{
      "openapi": "3.0.0",
      "info": { "title": "t", "version": "1" },
      "paths": {
        "/things/{id}": {
          "get": {
            "operationId": "getThing",
            "parameters": [
              { "name": "id", "in": "path", "required": true,
                "schema": { "type": "string", "pattern": "^G[0-9]+$", "maxLength": 12 } }
            ],
            "responses": { "200": { "description": "ok" } }
          },
          "delete": {
            "operationId": "deleteThing",
            "parameters": [
              { "name": "id", "in": "path", "required": true,
                "schema": { "type": "string", "pattern": "^D[0-9]+$" } }
            ],
            "responses": { "200": { "description": "ok" } }
          }
        },
        "/reserved/{seg}": {
          "get": {
            "operationId": "getReserved",
            "parameters": [
              { "name": "seg", "in": "path", "required": true,
                "allowReserved": true,
                "schema": { "type": "string" } }
            ],
            "responses": { "200": { "description": "ok" } }
          }
        }
      }
    }"#;

    fn bare() -> HttpCodeExecutor {
        let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
        HttpCodeExecutor::new(
            reqwest::Client::new(),
            "https://api.example".to_string(),
            auth,
        )
    }

    fn with_spec() -> HttpCodeExecutor {
        bare().with_schema(Arc::new(
            OpenApiSchema::parse(SPEC).expect("the fixture spec parses"),
        ))
    }

    /// `new`'s signature is unchanged, so the schema starts absent and every
    /// pre-existing construction site keeps compiling. A default-returning
    /// executor is FLOORED AND CAPPED — the assertion below is about the absence
    /// of NARROWING, not about the absence of checks.
    #[test]
    fn an_executor_with_no_schema_returns_the_default() {
        let exec = bare();
        let rules = exec.placeholder_rules("GET", "/things/{id}", "id");
        assert_eq!(rules.declared_pattern, None);
        assert_eq!(rules.declared_max_length, None);
        assert!(!rules.allow_slash);
    }

    #[test]
    fn a_declared_pattern_reaches_the_rules() {
        let exec = with_spec();
        let rules = exec.placeholder_rules("GET", "/things/{id}", "id");
        assert_eq!(rules.declared_pattern, Some("^G[0-9]+$"));
        assert_eq!(rules.declared_max_length, Some(12));
    }

    /// The `method` parameter is load-bearing, not decoration: two operations on
    /// one path declare different patterns and each must get its own.
    #[test]
    fn the_method_selects_the_operation() {
        let exec = with_spec();
        assert_eq!(
            exec.placeholder_rules("DELETE", "/things/{id}", "id")
                .declared_pattern,
            Some("^D[0-9]+$"),
            "a DELETE must never be narrowed by the GET's declared pattern"
        );
        assert_eq!(
            exec.placeholder_rules("get", "/things/{id}", "id")
                .declared_pattern,
            Some("^G[0-9]+$"),
            "`operation_for` upper-cases the method, so a lowercase verb still hits"
        );
    }

    #[test]
    fn an_unknown_path_template_returns_the_default() {
        let exec = with_spec();
        // The `/users/{alias}` versus `/users/{id}` spelling-drift case: a template
        // the spec does not carry loses the NARROWING and keeps the floor + cap.
        let rules = exec.placeholder_rules("GET", "/things/{alias}", "alias");
        assert_eq!(rules.declared_pattern, None);
        assert_eq!(rules.declared_max_length, None);
        assert!(!rules.allow_slash);
    }

    #[test]
    fn an_unknown_method_on_a_known_path_returns_the_default() {
        let exec = with_spec();
        assert_eq!(
            exec.placeholder_rules("PUT", "/things/{id}", "id")
                .declared_pattern,
            None
        );
    }

    #[test]
    fn an_unknown_parameter_name_returns_the_default() {
        let exec = with_spec();
        assert_eq!(
            exec.placeholder_rules("GET", "/things/{id}", "nope")
                .declared_pattern,
            None
        );
    }

    /// A QUERY-position parameter of the same name must not narrow a PATH
    /// placeholder: `placeholder_rules` answers a question about the path.
    #[test]
    fn a_non_path_parameter_is_not_consulted() {
        let spec = r#"{
          "openapi": "3.0.0",
          "info": { "title": "t", "version": "1" },
          "paths": {
            "/q": {
              "get": {
                "operationId": "q",
                "parameters": [
                  { "name": "id", "in": "query", "required": false,
                    "schema": { "type": "string", "pattern": "^Q[0-9]+$" } }
                ],
                "responses": { "200": { "description": "ok" } }
              }
            }
          }
        }"#;
        let exec = bare().with_schema(Arc::new(OpenApiSchema::parse(spec).expect("parses")));
        assert_eq!(
            exec.placeholder_rules("GET", "/q", "id").declared_pattern,
            None
        );
    }

    /// D-11 / T-128-37 — a spec's reserved-expansion keyword must never reach
    /// `allow_slash`. Asserted for EVERY spec-derived result this fixture can
    /// produce, not only the one that declares the keyword.
    #[test]
    fn allow_slash_is_false_for_every_spec_derived_result() {
        let exec = with_spec();
        for (method, template, param) in [
            ("GET", "/things/{id}", "id"),
            ("DELETE", "/things/{id}", "id"),
            ("GET", "/reserved/{seg}", "seg"),
            ("GET", "/things/{alias}", "alias"),
            ("PUT", "/things/{id}", "id"),
        ] {
            assert!(
                !exec.placeholder_rules(method, template, param).allow_slash,
                "{method} {template} {param}: allow_slash is config-only (D-11)"
            );
        }
    }

    /// A miss is not a hole: the value a floored-and-capped default refuses is
    /// still refused. This is T-128-36 stated as a test rather than as a doc
    /// sentence.
    #[test]
    fn a_schema_miss_still_refuses_a_floor_denied_value() {
        let exec = with_spec();
        let rules = exec.placeholder_rules("GET", "/things/{alias}", "alias");
        assert!(
            pmcp_code_mode::validate_path_placeholder("alias", "current/../../etc", &rules)
                .is_err(),
            "the floor survives a schema miss"
        );
        assert!(
            pmcp_code_mode::validate_path_placeholder("alias", &"x".repeat(257), &rules).is_err(),
            "the always-on cap survives a schema miss"
        );
    }

    /// The narrowing actually narrows: a value that CLEARS the floor is refused by
    /// the spec's declared pattern, and accepted without the schema.
    #[test]
    fn the_narrowing_refuses_a_floor_clean_value_the_spec_forbids() {
        let clean = "NOTMATCHING";
        let spec_exec = with_spec();
        let narrowed = spec_exec.placeholder_rules("GET", "/things/{id}", "id");
        let err = pmcp_code_mode::validate_path_placeholder("id", clean, &narrowed)
            .expect_err("the declared pattern must refuse it");
        assert_eq!(err.rule, "pattern");
        assert!(
            !err.to_string().contains(clean),
            "the refusal must not echo the value: {err}"
        );

        let bare_exec = bare();
        let bare_rules = bare_exec.placeholder_rules("GET", "/things/{id}", "id");
        assert!(
            pmcp_code_mode::validate_path_placeholder("id", clean, &bare_rules).is_ok(),
            "without the schema the same value passes — so the NARROWING refused it, not the floor"
        );
    }
}

// -----------------------------------------------------------------------------
// Phase 128 E1 — the Code Mode surface's outbound-policy seam.
//
// The exact mirror of `http::client`'s `request_policy_seam`: same contract, same
// assertions, different surface. Selected by the `--lib code_mode::` filter.
// -----------------------------------------------------------------------------

/// The E1 hook on the Code Mode / script-tool surface.
#[cfg(all(test, feature = "openapi-code-mode"))]
mod request_policy_seam {
    use super::HttpCodeExecutor;
    use crate::http::auth::HttpAuthProvider;
    use crate::http::HttpConnectorError;
    use crate::policy::{OutboundRequest, PolicyRefusal, RequestPolicy};
    use async_trait::async_trait;
    use pmcp_code_mode::{HttpExecutor, ResolvedPath};
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Records whether auth ran, so a refusal test proves the hook is BEFORE auth.
    struct RecordingAuth {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl HttpAuthProvider for RecordingAuth {
        async fn apply(
            &self,
            headers: &mut HeaderMap,
            query: &mut HashMap<String, String>,
            _inbound_token: Option<&str>,
        ) -> Result<(), HttpConnectorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            headers.insert("authorization", HeaderValue::from_static("Bearer tok"));
            // An API-key-in-query credential: the policy must NEVER see this pair.
            query.insert("app_key".to_string(), "super-secret".to_string());
            Ok(())
        }
    }

    type Seen = Arc<Mutex<Vec<(String, String, Vec<(String, String)>, Option<String>)>>>;

    struct Recorder {
        seen: Seen,
        refuse: Option<&'static str>,
    }

    #[async_trait]
    impl RequestPolicy for Recorder {
        async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
            self.seen.lock().expect("lock").push((
                req.tool.to_string(),
                req.path.to_string(),
                req.query.to_vec(),
                req.body.map(ToString::to_string),
            ));
            match self.refuse {
                Some(msg) => Err(PolicyRefusal::new(msg)),
                None => Ok(()),
            }
        }
    }

    fn recorder(refuse: Option<&'static str>) -> (Arc<Recorder>, Seen) {
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Recorder {
                seen: Arc::clone(&seen),
                refuse,
            }),
            seen,
        )
    }

    fn exec(
        base_url: String,
        policy: Option<Arc<dyn RequestPolicy>>,
    ) -> (HttpCodeExecutor, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let auth = Arc::new(RecordingAuth {
            calls: Arc::clone(&calls),
        });
        let e = HttpCodeExecutor::new(reqwest::Client::new(), base_url, auth);
        let e = match policy {
            Some(p) => e.with_request_policy(p),
            None => e,
        };
        (e, calls)
    }

    #[tokio::test]
    async fn a_refusing_policy_stops_the_request_before_auth_and_before_the_send() {
        use wiremock::MockServer;
        let server = MockServer::start().await;
        let (policy, _seen) = recorder(Some("refused by test policy"));
        let (executor, auth_calls) = exec(server.uri(), Some(policy));

        let err = executor
            .execute_request(
                "GET",
                ResolvedPath::from_checked("/users/42").expect("checked"),
                None,
            )
            .await
            .expect_err("the policy refuses");
        assert!(
            err.to_string().contains("refused by test policy"),
            "the refusal must carry the policy's own message, got: {err}"
        );
        assert_eq!(
            auth_calls.load(Ordering::SeqCst),
            0,
            "the auth provider must NOT have been invoked — the hook is before auth"
        );
        assert!(server
            .received_requests()
            .await
            .expect("recorded")
            .is_empty());
    }

    #[tokio::test]
    async fn an_allowing_policy_lets_the_request_through_and_auth_is_applied() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let (policy, _seen) = recorder(None);
        let (executor, auth_calls) = exec(server.uri(), Some(policy));
        let out = executor
            .execute_request(
                "GET",
                ResolvedPath::from_checked("/users/42").expect("checked"),
                None,
            )
            .await
            .expect("allowed");
        assert_eq!(out["ok"], true);
        assert_eq!(auth_calls.load(Ordering::SeqCst), 1);
    }

    /// The assertion that fails if the non-auth half of step (4) is moved back
    /// BELOW the hook: a policy written to inspect query pairs would then inspect
    /// an empty slice while the pairs about to be sent still sat in the body.
    #[tokio::test]
    async fn the_policy_sees_the_remaining_body_query_pairs_on_a_get() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let (policy, seen) = recorder(None);
        let (executor, _auth) = exec(server.uri(), Some(policy));
        executor
            .execute_request(
                "GET",
                ResolvedPath::from_checked("/search").expect("checked"),
                Some(serde_json::json!({ "term": "aspirin", "limit": 5 })),
            )
            .await
            .expect("allowed");

        let seen = seen.lock().expect("lock");
        assert_eq!(seen.len(), 1);
        let (_tool, observed_path, query, body) = &seen[0];
        assert!(observed_path.ends_with("/search"));
        assert!(
            !query.is_empty(),
            "OutboundRequest.query must carry the remaining-body pairs a GET will send"
        );
        let keys: Vec<&str> = query.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            keys.contains(&"term") && keys.contains(&"limit"),
            "got {keys:?}"
        );
        assert!(
            body.is_none(),
            "a GET's remaining body became query pairs before the hook ran"
        );
    }

    /// The credential must be absent from every field, in-crate as well as in the
    /// integration binary: the auth provider above contributes BOTH a header and
    /// an `app_key` query pair, and neither may be visible.
    #[tokio::test]
    async fn the_policy_never_sees_the_credential() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let (policy, seen) = recorder(None);
        let (executor, _auth) = exec(server.uri(), Some(policy));
        executor
            .execute_request(
                "POST",
                ResolvedPath::from_checked("/items").expect("checked"),
                Some(serde_json::json!({ "name": "widget" })),
            )
            .await
            .expect("allowed");

        let seen = seen.lock().expect("lock");
        let (tool, observed_path, query, body) = &seen[0];
        for field in [tool.as_str(), observed_path.as_str()] {
            assert!(
                !field.contains("super-secret"),
                "credential leaked: {field}"
            );
        }
        assert!(
            !query
                .iter()
                .any(|(k, v)| k == "app_key" || v.contains("super-secret")),
            "the auth provider's query credential must be invisible to the policy"
        );
        assert!(!body.as_deref().unwrap_or("").contains("super-secret"));
    }

    #[tokio::test]
    async fn no_policy_behaves_exactly_as_before() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let (executor, auth_calls) = exec(server.uri(), None);
        assert!(!executor.has_request_policy());
        let out = executor
            .execute_request(
                "GET",
                ResolvedPath::from_checked("/x").expect("checked"),
                None,
            )
            .await
            .expect("succeeds");
        assert_eq!(out["ok"], true);
        assert_eq!(auth_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_policy_is_told_which_tool_the_executor_serves() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let (policy, seen) = recorder(None);
        let (executor, _auth) = exec(server.uri(), Some(policy));
        let executor = executor.with_tool_label("lookup_code");
        executor
            .execute_request(
                "GET",
                ResolvedPath::from_checked("/x").expect("checked"),
                None,
            )
            .await
            .expect("allowed");
        assert_eq!(seen.lock().expect("lock")[0].0, "lookup_code");
    }
}
