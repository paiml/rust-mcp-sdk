// Net-new code for Phase 83 PATTERNS §13 (builder extension surface).
// Hosts the `ServerBuilderExt` trait + `try_*` fallible variants per review R7.

//! Builder extension trait for [`pmcp::ServerBuilder`] — connects config-driven
//! synthesis (Plans 04, 05, 06) to the public Phase 82 builder API.
//!
//! Per CONTEXT.md D-10 + D-11, this is the "common path" surface — power users
//! call [`crate::tools::synthesize_from_config`] +
//! [`crate::code_mode::register_code_mode_tools`] directly. Shape C ≤15-line
//! `main.rs` users compose this trait.
//!
//! Per review R7, each method has a panicking convenience form
//! ([`ServerBuilderExt::tools_from_config`],
//! [`ServerBuilderExt::code_mode_from_config`]) AND a fallible companion
//! ([`ServerBuilderExt::try_tools_from_config`],
//! [`ServerBuilderExt::try_code_mode_from_config`]). The panicking forms
//! delegate to the `try_*` variants with documented panic messages — production
//! servers should prefer the `try_*` shape so misconfiguration surfaces as a
//! `Result`, not a crash.

use std::sync::Arc;

use pmcp::ServerBuilder;

use crate::config::ServerConfig;
use crate::error::Result;
use crate::policy::ToolkitHooks;
use crate::sql::SqlConnector;

/// Composable builder extensions for config-driven `pmcp` servers.
///
/// Implemented for [`pmcp::ServerBuilder`] (Phase 82's public, `Arc`-aware
/// builder) so config-driven wiring composes with the standard chained-method
/// builder DSL.
pub trait ServerBuilderExt: Sized {
    /// Register every `[[tools]]` entry from `config` as a `tool_arc` handler
    /// (TKIT-07). Panicking convenience wrapping
    /// [`ServerBuilderExt::try_tools_from_config`].
    ///
    /// # Panics
    ///
    /// Panics with `"tools_from_config: ..."` if
    /// [`crate::tools::synthesize_from_config`] returns `Err`. Prefer
    /// [`ServerBuilderExt::try_tools_from_config`] for production servers
    /// where misconfiguration must surface as a `Result`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .tools_from_config(&cfg);
    /// ```
    fn tools_from_config(self, config: &ServerConfig) -> Self;

    /// Fallible companion to [`ServerBuilderExt::tools_from_config`]
    /// (review R7).
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if synthesis fails — typically
    /// [`crate::ToolkitError::Synth`] or [`crate::ToolkitError::Validation`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_tools_from_config(&cfg)?;
    /// # Ok(()) }
    /// ```
    fn try_tools_from_config(self, config: &ServerConfig) -> Result<Self>;

    /// Register every `[[tools]]` entry from `config` as a `tool_arc` handler,
    /// threading `connector` into each handler so `tools/call` executes SQL and
    /// emits `structuredContent` (Phase 84 CONN-01 / D-06). Panicking
    /// convenience wrapping [`ServerBuilderExt::try_tools_from_config_with_connector`].
    ///
    /// This is the Shape A wiring point: production servers with a live
    /// connector use this entry point; the connector-less
    /// [`ServerBuilderExt::tools_from_config`] remains for callers that only
    /// need the synthesized tool schemas (handlers error at runtime if invoked).
    ///
    /// # Panics
    ///
    /// Panics with `"tools_from_config_with_connector: ..."` if
    /// [`crate::tools::synthesize_from_config_with_connector`] returns `Err`.
    /// Prefer [`ServerBuilderExt::try_tools_from_config_with_connector`] for
    /// production servers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    /// use pmcp_server_toolkit::sql::SqlConnector;
    ///
    /// fn build(connector: Arc<dyn SqlConnector>) {
    ///     let cfg = ServerConfig::default();
    ///     let _builder = Server::builder()
    ///         .name("demo")
    ///         .version("0.1.0")
    ///         .tools_from_config_with_connector(&cfg, connector);
    /// }
    /// ```
    fn tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Self;

    /// Fallible companion to
    /// [`ServerBuilderExt::tools_from_config_with_connector`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if synthesis fails — typically
    /// [`crate::ToolkitError::Synth`] or [`crate::ToolkitError::Validation`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    /// use pmcp_server_toolkit::sql::SqlConnector;
    ///
    /// # fn run(connector: Arc<dyn SqlConnector>) -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_tools_from_config_with_connector(&cfg, connector)?;
    /// # Ok(()) }
    /// ```
    fn try_tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self>;

    /// Wire the `[code_mode]` block. Panicking convenience wrapping
    /// [`ServerBuilderExt::try_code_mode_from_config`].
    ///
    /// When the `code-mode` feature is disabled, this is a no-op that emits
    /// a `tracing::warn!` so operators auditing logs can spot the feature gap
    /// (threat T-83-08-02 mitigation).
    ///
    /// # Panics
    ///
    /// Panics if [`ServerBuilderExt::try_code_mode_from_config`] errors —
    /// commonly because `token_secret`'s referenced env var is unset, or an
    /// inline literal `token_secret` was supplied without the dev-only escape
    /// hatch (review R9). Prefer
    /// [`ServerBuilderExt::try_code_mode_from_config`] for production servers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .code_mode_from_config(&cfg);
    /// ```
    fn code_mode_from_config(self, config: &ServerConfig) -> Self;

    /// Fallible companion to [`ServerBuilderExt::code_mode_from_config`]
    /// (review R7) — the CONNECTORLESS, **validation-only / no-tool** path.
    ///
    /// Tolerant of `config.code_mode = None` (returns the builder unchanged).
    /// When `[code_mode]` IS present this builds + validates the pipeline (so
    /// R9 / secret-resolution errors fire) but registers NO tools, because no
    /// executor is available to bind `execute_code` to. For the path that
    /// actually registers `validate_code` + `execute_code`, use the LOCKED
    /// connector-aware
    /// [`ServerBuilderExt::try_code_mode_from_config_with_connector`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if code-mode wiring fails — commonly
    /// [`crate::ToolkitError::CodeMode`] (env var missing) or
    /// [`crate::ToolkitError::Validation`] (inline `token_secret` rejected
    /// per review R9).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_code_mode_from_config(&cfg)?;
    /// # Ok(()) }
    /// ```
    fn try_code_mode_from_config(self, config: &ServerConfig) -> Result<Self>;

    /// Wire the `[code_mode]` block, registering BOTH `validate_code` and
    /// `execute_code` over `connector` (the LOCKED connector-aware API — the
    /// pure-config binary's path; SHAP-A-01 / SC-3).
    ///
    /// When `[code_mode]` is present this constructs a
    /// [`crate::code_mode::SqlCodeExecutor`] from `connector` and delegates to
    /// [`crate::code_mode::code_mode_tools_from_executor`], which registers the
    /// two tools with the static `[code_mode]` policy baked into the validation
    /// pipeline (allow_writes / allow_deletes / allow_ddl enforced; DELETE/DDL
    /// on a read-only config are rejected). When `[code_mode]` is absent this is
    /// a no-op (registers neither tool). Unlike the connectorless
    /// [`ServerBuilderExt::try_code_mode_from_config`], this is the tool-
    /// registering path because it has an executor to bind `execute_code` to.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if code-mode wiring fails — commonly
    /// [`crate::ToolkitError::CodeMode`] (env var missing / secret too short)
    /// or [`crate::ToolkitError::Validation`] (inline `token_secret` rejected
    /// per review R9).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    /// use pmcp_server_toolkit::sql::SqlConnector;
    ///
    /// # fn run(connector: Arc<dyn SqlConnector>) -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_code_mode_from_config_with_connector(&cfg, connector)?;
    /// # Ok(()) }
    /// ```
    fn try_code_mode_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self>;

    /// [`Self::try_tools_from_config`] with registered E1/E2 hooks, and the
    /// once-at-startup enforcement log (Phase 128).
    ///
    /// # Why hooks are a PARAMETER rather than accumulated on the builder
    ///
    /// This trait is implemented for CORE's [`pmcp::ServerBuilder`], whose fields
    /// are private. A Rust extension trait cannot add a field to a foreign type, so
    /// a `with_request_policy(self) -> Self` on this trait would have nowhere to
    /// store anything. [`ToolkitHooks`] carries the registrations instead and
    /// [`Self::try_tools_from_config`] became a thin wrapper passing a default — so
    /// no pre-existing signature changed and no existing caller broke.
    ///
    /// # What E1 does NOT govern on this path
    ///
    /// This entry point synthesizes SQL / connectorless tools and owns no HTTP
    /// egress surface, so a [`crate::RequestPolicy`] registered here has nothing to
    /// govern. That is reported as a startup WARNING rather than accepted in
    /// silence: a policy the operator registered and nothing consults is exactly
    /// the present-but-inert defect this phase exists to close. E1 reaches the two
    /// HTTP surfaces through `pmcp-openapi-server`'s `build_server`.
    ///
    /// # Errors
    ///
    /// As [`Self::try_tools_from_config`].
    fn try_tools_from_config_with(
        self,
        config: &ServerConfig,
        hooks: &ToolkitHooks,
    ) -> Result<Self>;

    /// [`Self::try_tools_from_config_with_connector`] with registered E1/E2 hooks
    /// (Phase 128). See [`Self::try_tools_from_config_with`] for the design and for
    /// what E1 does and does not govern on this path.
    ///
    /// # Errors
    ///
    /// As [`Self::try_tools_from_config_with_connector`].
    fn try_tools_from_config_with_connector_and_hooks(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
        hooks: &ToolkitHooks,
    ) -> Result<Self>;
}

/// Warn when a [`ToolkitHooks`] carries a [`crate::RequestPolicy`] on an assembly
/// path with no HTTP egress surface to apply it to (Phase 128).
///
/// An E1 policy governs the two HTTP surfaces. The `ServerBuilderExt` tool paths
/// synthesize SQL / connectorless handlers, so a policy registered here is
/// unreachable — and an unreachable policy that stays silent is a rule the
/// operator believes is enforced and is not.
fn warn_if_policy_has_no_surface(hooks: &ToolkitHooks, entry_point: &str) {
    if hooks.request_policy().is_some() {
        tracing::warn!(
            target: "pmcp_server_toolkit::builder_ext",
            entry_point = %entry_point,
            "a RequestPolicy is registered but THIS assembly path has no HTTP egress \
             surface to apply it to, so it will never run. E1 governs the curated HTTP \
             connector and the Code Mode executor; SQL connector traffic is not \
             intercepted. Register the policy on the path that builds those (the \
             OpenAPI binary's build_server)."
        );
    }
}

impl ServerBuilderExt for ServerBuilder {
    fn tools_from_config(self, config: &ServerConfig) -> Self {
        self.try_tools_from_config(config).expect(
            "tools_from_config: synthesize_from_config returned an error — \
             prefer try_tools_from_config to handle this as a Result",
        )
    }

    fn try_tools_from_config(self, config: &ServerConfig) -> Result<Self> {
        self.try_tools_from_config_with(config, &ToolkitHooks::default())
    }

    fn try_tools_from_config_with(
        mut self,
        config: &ServerConfig,
        hooks: &ToolkitHooks,
    ) -> Result<Self> {
        crate::policy::emit_validation_report(config, hooks);
        warn_if_policy_has_no_surface(hooks, "try_tools_from_config_with");
        let synthesized = crate::tools::synthesize_from_config_and_hooks(config, hooks)?;
        // T-83-08-02 mitigation: emit a visible signal when the [[tools]]
        // block is empty so an operator notices the gap rather than seeing a
        // silently-empty server.
        if synthesized.is_empty() {
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_tools_from_config: config declared zero [[tools]] entries — \
                 server will expose no tools (set RUST_LOG=warn to surface this)"
            );
        }
        for (name, _info, handler) in synthesized {
            self = self.tool_arc(name, handler);
        }
        Ok(self)
    }

    fn tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Self {
        self.try_tools_from_config_with_connector(config, connector)
            .expect(
                "tools_from_config_with_connector: synthesize_from_config_with_connector \
                 returned an error — prefer try_tools_from_config_with_connector to handle \
                 this as a Result",
            )
    }

    fn try_tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self> {
        self.try_tools_from_config_with_connector_and_hooks(
            config,
            connector,
            &ToolkitHooks::default(),
        )
    }

    fn try_tools_from_config_with_connector_and_hooks(
        mut self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
        hooks: &ToolkitHooks,
    ) -> Result<Self> {
        crate::policy::emit_validation_report(config, hooks);
        warn_if_policy_has_no_surface(hooks, "try_tools_from_config_with_connector_and_hooks");
        let synthesized = crate::tools::synthesize_from_config_with_connector_and_hooks(
            config, connector, hooks,
        )?;
        // T-83-08-02 mitigation: visible signal when the [[tools]] block is
        // empty so an operator notices the gap rather than a silently-empty server.
        if synthesized.is_empty() {
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_tools_from_config_with_connector: config declared zero [[tools]] entries — \
                 server will expose no tools (set RUST_LOG=warn to surface this)"
            );
        }
        for (name, _info, handler) in synthesized {
            self = self.tool_arc(name, handler);
        }
        Ok(self)
    }

    fn code_mode_from_config(self, config: &ServerConfig) -> Self {
        self.try_code_mode_from_config(config).expect(
            "code_mode_from_config: register_code_mode_tools errored — \
             prefer try_code_mode_from_config to handle (e.g. missing env var)",
        )
    }

    fn try_code_mode_from_config(self, config: &ServerConfig) -> Result<Self> {
        #[cfg(feature = "code-mode")]
        {
            crate::code_mode::register_code_mode_tools(self, config)
        }
        #[cfg(not(feature = "code-mode"))]
        {
            let _ = config;
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_code_mode_from_config called but `code-mode` feature is \
                 disabled at compile-time — skipping (T-83-08-02 visibility)"
            );
            Ok(self)
        }
    }

    fn try_code_mode_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self> {
        #[cfg(feature = "code-mode")]
        {
            if config.code_mode.is_none() {
                return Ok(self); // no-op when block absent (mirrors connectorless path)
            }
            // Coerce the SQL executor to the backend-agnostic `Arc<dyn
            // CodeExecutor>` the generalized wiring fn takes (OAPI-10 / D-02).
            // `CodeExecutor` is `#[async_trait]` and object-safe, so this is a
            // plain unsize coercion. The SQL path passes `ValidationFlavor::Sql`.
            let executor: Arc<dyn crate::code_mode::CodeExecutor> = Arc::new(
                crate::code_mode::SqlCodeExecutor::new(connector, config.clone())?,
            );
            crate::code_mode::code_mode_tools_from_executor(
                self,
                config,
                executor,
                crate::code_mode::ValidationFlavor::Sql,
            )
        }
        #[cfg(not(feature = "code-mode"))]
        {
            let _ = (config, connector);
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_code_mode_from_config_with_connector called but `code-mode` \
                 feature is disabled at compile-time — skipping (T-83-08-02 visibility)"
            );
            Ok(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ServerConfig, ServerSection, ToolDecl};
    use pmcp::Server;

    fn min_cfg() -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools: vec![ToolDecl {
                name: "ping".to_string(),
                description: Some("ping".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn tools_from_config_registers_synthesized_handlers() {
        let cfg = min_cfg();
        let server = Server::builder()
            .name("test")
            .version("0.1.0")
            .tools_from_config(&cfg)
            .build()
            .expect("build");
        assert!(
            server.get_tool("ping").is_some(),
            "tools_from_config must wire each [[tools]] entry via tool_arc (Phase 82)"
        );
    }

    /// Phase 128: the hooks-taking entry point registers the same handlers, and a
    /// registered E2 validator actually reaches the tool it names.
    #[test]
    fn try_tools_from_config_with_registers_handlers_and_reaches_the_validator() {
        use crate::policy::{ArgumentRefusal, ArgumentValidator, ToolkitHooks};
        use serde_json::Value;
        use std::sync::Arc;

        struct RefuseAll;
        impl ArgumentValidator for RefuseAll {
            fn validate(&self, _args: &Value) -> std::result::Result<(), ArgumentRefusal> {
                Err(ArgumentRefusal::new(
                    "this tool is administratively disabled",
                ))
            }
        }

        let cfg = min_cfg();
        let hooks = ToolkitHooks::default().with_argument_validator("ping", Arc::new(RefuseAll));
        let server = Server::builder()
            .name("test")
            .version("0.1.0")
            .try_tools_from_config_with(&cfg, &hooks)
            .expect("ok")
            .build()
            .expect("build");
        assert!(
            server.get_tool("ping").is_some(),
            "the hooks-taking entry point must register handlers exactly as the wrapper does"
        );
    }

    /// The pre-existing method must behave identically to the wrapper it became —
    /// an empty `ToolkitHooks` changes nothing.
    #[test]
    fn try_tools_from_config_is_a_thin_wrapper_over_the_hooks_variant() {
        use crate::policy::ToolkitHooks;

        let cfg = min_cfg();
        let via_wrapper = Server::builder()
            .name("t")
            .version("0.1.0")
            .try_tools_from_config(&cfg)
            .expect("ok")
            .build()
            .expect("build");
        let via_hooks = Server::builder()
            .name("t")
            .version("0.1.0")
            .try_tools_from_config_with(&cfg, &ToolkitHooks::default())
            .expect("ok")
            .build()
            .expect("build");
        assert!(via_wrapper.get_tool("ping").is_some());
        assert!(via_hooks.get_tool("ping").is_some());
    }

    /// A `RequestPolicy` registered on this path has no HTTP egress surface to
    /// govern. It must not be an ERROR (a config edit may add one later) and it must
    /// not be silent either — `warn_if_policy_has_no_surface` is the report. This
    /// asserts the non-error half and that the helper is reached at all.
    #[test]
    fn a_policy_on_the_sql_path_is_reported_and_not_an_error() {
        use crate::policy::{OutboundRequest, PolicyRefusal, RequestPolicy, ToolkitHooks};
        use std::sync::Arc;

        struct RefuseAll;
        #[async_trait::async_trait]
        impl RequestPolicy for RefuseAll {
            async fn check(
                &self,
                _req: &OutboundRequest<'_>,
            ) -> std::result::Result<(), PolicyRefusal> {
                Err(PolicyRefusal::new("refused"))
            }
        }

        let hooks = ToolkitHooks::default().with_request_policy(Arc::new(RefuseAll));
        super::warn_if_policy_has_no_surface(&hooks, "unit-test");
        let cfg = min_cfg();
        let builder = Server::builder()
            .name("t")
            .version("0.1.0")
            .try_tools_from_config_with(&cfg, &hooks);
        assert!(
            builder.is_ok(),
            "an unreachable policy warns; it must never fail the build"
        );
    }

    #[test]
    fn try_tools_from_config_returns_ok_on_valid_config() {
        let cfg = min_cfg();
        let builder = Server::builder().name("t").version("0.1.0");
        let result = builder.try_tools_from_config(&cfg);
        assert!(result.is_ok(), "valid config must return Ok");
    }

    #[test]
    fn code_mode_from_config_is_noop_when_block_absent() {
        // Plan 06 Task 2 ensures register_code_mode_tools tolerates
        // config.code_mode = None.
        let cfg = min_cfg();
        let _builder = Server::builder()
            .name("t")
            .version("0.1.0")
            .code_mode_from_config(&cfg);
        // No panic means tolerance works.
    }

    #[test]
    fn try_code_mode_from_config_is_ok_when_block_absent() {
        let cfg = min_cfg();
        let builder = Server::builder().name("t").version("0.1.0");
        let result = builder.try_code_mode_from_config(&cfg);
        assert!(
            result.is_ok(),
            "code_mode = None must produce Ok (no-op) so callers can invoke unconditionally"
        );
    }
}
