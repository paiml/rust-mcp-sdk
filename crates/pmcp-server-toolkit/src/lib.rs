// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as a public SDK crate for Phase 83.

//! Runtime library for config-driven MCP servers.
//!
//! `pmcp-server-toolkit` lifts the operational glue that pmcp-run servers
//! share — auth providers, secrets resolution, static resources/prompts,
//! a `[[tools]]` synthesizer, and code-mode wiring — into the public SDK.
//!
//! Phase 83 ships the empty crate skeleton; subsequent plans land the
//! functionality across these modules:
//!
//! - [`auth`] — `AuthProvider` implementations (`StaticAuthProvider`, `BearerAuthProvider`).
//! - [`secrets`] — `SecretsProvider` trait + env/AWS implementations and the
//!   `SecretValue` newtype that never leaks via `Debug`/`Display`/`Serialize`.
//! - [`config`] — `ServerConfig` types with `#[serde(deny_unknown_fields)]` strictness.
//! - [`prompts`] — `StaticPromptHandler` adapter for static prompt templates.
//! - [`resources`] — `StaticResourceHandler` adapter for shipped resources.
//! - [`tools`] — `synthesize_from_config` builder turning `[[tools]]` into runtime handlers.
//! - [`sql`] — `SqlConnector` trait + dialect enum for backend-agnostic SQL toolkits.
//! - [`builder_ext`] — `ServerBuilderExt` extension methods on `pmcp::ServerBuilder`.
//! - [`code_mode`] *(feature `code-mode`)* — re-exports from `pmcp-code-mode` plus toolkit glue.
//! - [`error`] — `ToolkitError` enum and the crate-level `Result<T>` alias.
//!
//! The public module set is locked by Phase 83 decision D-15. See the
//! `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/` design log for
//! the architectural responsibility map and review notes.

pub mod auth;
pub mod builder_ext;
pub mod config;

/// The `${VAR}` / `env:VAR` reference grammar — the ONE parse chokepoint every
/// env-reference path in the toolkit shares (credentials, `token_secret`, and
/// `[backend].base_url`).
///
/// Deliberately carries NO `#[cfg(feature = ...)]` gate: the function used to
/// live inside the feature-gated `http` module, which made "the toolkit's
/// universal chokepoint" true only in `http` builds.
///
/// `pub` rather than `pub(crate)` (plan 120-05): the grammar is duplicated by
/// necessity in `pmcp-package` — the workspace-excluded leaf crate, which
/// neither may depend on this one nor be depended on by it — and the two
/// implementations are held to a shared accept/reject table asserted from an
/// INTEGRATION test in each crate. An integration test is an external consumer,
/// so the reference implementation has to be reachable from outside the crate
/// for that parity claim to be checkable at all.
pub mod env_ref;

pub mod error;
pub mod prompts;
pub mod resources;
pub mod secrets;
pub mod sql;
pub mod tools;

/// HTTP backend primitives for config-driven OpenAPI MCP servers (Phase 90).
///
/// Gated behind the opt-in `http` feature so the curated / no-`http` toolkit
/// build stays light (RESEARCH Pitfall 4).
#[cfg(feature = "http")]
pub mod http;

/// Governed-Excel workbook served-tool module (Phase 92).
///
/// Gated behind the opt-in `workbook` feature so the no-`workbook` toolkit
/// build never links the `pmcp-workbook-runtime` BundleSource/BundleLoader
/// surface. The `workbook-embedded` feature additionally enables the runtime's
/// `embedded` (include_dir) EmbeddedSource for binary-baked bundles.
#[cfg(feature = "workbook")]
pub mod workbook;

#[cfg(feature = "code-mode")]
pub mod code_mode;

pub use error::{Result, ToolkitError};

// === Crate-root re-exports per D-15 (headline DX promise — reviewed R3) ===
//
// A Shape C consumer writes a single one-line crate-root import:
//   use pmcp_server_toolkit::{AuthProvider, StaticAuthProvider,
//                             SecretsProvider, SecretValue, EnvSecrets};
//
// NO `as _` no-name imports (those break the DX promise — review R3).

// Auth — re-export pmcp's trait at the toolkit crate root so consumers don't
// have to write `pmcp::server::auth::AuthProvider`. The toolkit's static impl
// is also re-exported at crate root.
pub use crate::auth::StaticAuthProvider;
pub use pmcp::server::auth::AuthProvider;

// Secrets — toolkit-owned trait + value type + concrete impls. Per review R6
// the secret type `SecretValue` is toolkit-owned (NOT pmcp_code_mode::TokenSecret),
// so it's stable under `--no-default-features`.
pub use crate::secrets::{EnvSecrets, SecretValue, SecretsProvider, SecretsProviderChain};

// AWS-feature-gated secrets impls.
#[cfg(feature = "aws")]
pub use crate::secrets::{OrgSecretsManagerProvider, SecretsManagerSecrets, SsmSecrets};

// Resources (TKIT-04) — Plan 03 headline re-export per D-15 + review R3.
pub use crate::resources::StaticResourceHandler;

// Prompts (TKIT-05) — Plan 03 headline re-export per D-15 + review R3.
pub use crate::prompts::StaticPromptHandler;

// Plan 08 (TKIT-05 completion): the multi-prompt construction helper. The
// `impl From<&ServerConfig>` on `StaticPromptHandler` covers single-prompt
// servers; this function covers the common multi-prompt path. Lifted to the
// crate root per review R3 so the backend-core smoke test and downstream
// shape-C consumers don't need `pmcp_server_toolkit::prompts::*` paths.
pub use crate::prompts::prompt_handlers_from_config;

// Config (TKIT-01) — Plan 04 headline re-export per D-15 + review R3.
// ServerConfig is THE single top-level config type a Shape C consumer touches.
pub use crate::config::ServerConfig;

// Validation error type also surfaces at the crate root so consumers can
// pattern-match on it without importing from `error` (review R3 headline DX).
pub use crate::error::ConfigValidationError;

// Tools (TKIT-07) — Plan 05 headline re-export per D-15 + review R3.
// `synthesize_from_config` is the one-call entry point Shape A/C consumers
// reach for; lifting it to the crate root keeps the import surface flat.
pub use crate::tools::synthesize_from_config;

// Phase 84 (CONN-01 / D-06) — additive connector-threaded variant alongside the
// existing `synthesize_from_config`. The no-connector entry point above is
// unchanged; this one wires `Arc<dyn SqlConnector>` into each handler so
// `tools/call` can execute SQL and emit `structuredContent`.
pub use crate::tools::synthesize_from_config_with_connector;

// Phase 90 (OAPI-02a) — single-call HTTP synthesizer, mirroring the SQL
// connector-threaded variant above. Feature-gated on `http`. Wires
// `Arc<dyn HttpConnector>` into each single-call `[[tools]]` handler so
// `tools/call` executes the REST operation and returns JSON.
#[cfg(feature = "http")]
pub use crate::tools::synthesize_from_config_with_http_connector;

// Phase 90 (OAPI-02b / D-01 / D-02) — single-call + SCRIPT HTTP synthesizer.
// Gated `openapi-code-mode` (the umbrella that forwards
// `pmcp-code-mode/js-runtime`). Adds the shared `HttpCodeExecutor` + bounds so a
// `script` `[[tools]]` synthesizes a `ScriptToolHandler` that runs admin-authored
// JS over the SAME engine Code Mode uses (one engine, two surfaces).
#[cfg(feature = "openapi-code-mode")]
pub use crate::tools::synthesize_from_config_with_http_connector_and_scripts;

// Builder extensions (TKIT-08) — Plan 08 headline re-export per D-15 + review R3.
// The trait method set is the Shape C ≤15-line `main.rs` surface; lifting it
// to the crate root is the binding witness of D-15 (the runnable example
// imports SOLELY from `pmcp_server_toolkit::*` — never from module paths).
pub use crate::builder_ext::ServerBuilderExt;

// SQL connector trait stub (TKIT-10) — Plan 07 headline re-export per D-15 +
// review R3. MINIMIZED Phase 83 surface per review R2: ONLY `Dialect`,
// `SqlConnector`, and `ConnectorError` are re-exported. `execute()` and
// `translate_placeholders` are intentionally absent — they land in Phase 84
// (pmcp-server-toolkit 0.2.0) once the first real connector validates the
// contract. `MockSqlConnector` stays `pub(crate)` — it's test-only.
pub use crate::sql::{ConnectorError, Dialect, SqlConnector};

// HTTP connector (Phase 90 OAPI-01) — crate-root re-export of the headline
// types, mirroring the SQL connector re-export. Feature-gated on `http`.
#[cfg(feature = "http")]
pub use crate::http::{HttpConnector, HttpConnectorError, Operation};

// Workbook served-tool boot surface (Phase 92, WBSV-01/08/09 / D-11) — the
// FULL consumer-side contract at the crate root so Shape A/B servers register a
// governed workbook in ONE call WITHOUT ever naming `pmcp-workbook-runtime`:
// the builder-ext trait, the `BundleSource` trait + its on-disk impl, the
// fail-closed loader entry point, and both error types. The `EmbeddedSource`
// impl is gated on `workbook-embedded` (it needs the runtime's `embedded`
// include_dir support). Gated on `workbook` because the module is feature-gated.
#[cfg(feature = "workbook")]
pub use crate::workbook::{
    load_bundle, BundleLoadError, BundleSource, BundleSourceError, LocalDirSource,
    WorkbookBuilderExt,
};

/// The binary-baked workbook [`BundleSource`] (WBSV-09), re-exported at the
/// crate root only when the `workbook-embedded` feature is active.
#[cfg(feature = "workbook-embedded")]
pub use crate::workbook::EmbeddedSource;

// Code-mode prompt assembler (TKIT-10 / D-12) — Plan 07 headline re-export.
// Feature-gated on `code-mode` because it lives in the code_mode module which
// is itself feature-gated (D-16: code-mode is opt-in).
#[cfg(feature = "code-mode")]
pub use crate::code_mode::assemble_code_mode_prompt;

// File-based prompt seam (Plan 85-02 Task 3 / D-04 / D-05) — the sync,
// connectorless counterpart that seeds the prompt from a `--schema` file
// without live introspection (SC-1 prerequisite).
#[cfg(feature = "code-mode")]
pub use crate::code_mode::assemble_code_mode_prompt_with_schema;

// === Asset-aware path resolution (Phase 86 Review H1 — decided ONCE) ===
//
// Shapes B/C/D (the example, the scaffold emitter, and the deploy path) all need
// the SAME answer to "where do I read config.toml / schema.sql, and where do I
// write the demo SQLite DB?" so that a generated `main.rs` runs unchanged locally
// AND on AWS Lambda. The resolution is fixed here and re-used everywhere; callers
// MUST NOT hand-roll path logic.
//
// Config + schema are loaded via `pmcp::assets::load_string("config.toml")` and
// `pmcp::assets::load_string("schema.sql")`. The pmcp asset loader already
// resolves the correct base on each platform (verified in `src/assets/loader.rs`):
//   - Lambda: `$LAMBDA_TASK_ROOT/assets` (default `/var/task/assets`) — the
//     deploy bundler places `[assets] include` files under `assets/` in the zip.
//   - Local: `$PMCP_ASSETS_DIR` or the current working directory.
// The `assets` module is NOT feature-gated, so it is reachable from the toolkit's
// `default-features = false` `pmcp` dependency without enabling extra features.

/// Resolve the writable filesystem path for the demo SQLite database.
///
/// On AWS Lambda the deployment root (`/var/task`) is read-only, so a SQLite
/// database that must be created/seeded at startup has to live under the
/// writable `/tmp`. Locally a relative `demo.db` in the working directory is
/// fine. Lambda is detected by the presence of the `LAMBDA_TASK_ROOT`
/// environment variable, which the Lambda runtime always sets.
///
/// This pairs with `pmcp::assets::load_string("config.toml")` /
/// `pmcp::assets::load_string("schema.sql")` for read-only assets — config and
/// schema are bundled (and resolved) via the pmcp asset loader, while the
/// mutable database goes wherever this resolver points. Both halves are decided
/// once here so the example, the scaffold emitter, and the deploy path share one
/// shape (Phase 86 Review H1).
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::demo_db_path;
///
/// // Locally (no LAMBDA_TASK_ROOT) the demo DB is a relative file.
/// std::env::remove_var("LAMBDA_TASK_ROOT");
/// assert_eq!(demo_db_path(), std::path::PathBuf::from("demo.db"));
/// ```
#[must_use]
pub fn demo_db_path() -> std::path::PathBuf {
    if std::env::var("LAMBDA_TASK_ROOT").is_ok() {
        // Lambda: /var/task is read-only; SQLite must bootstrap into /tmp.
        std::path::PathBuf::from("/tmp/demo.db")
    } else {
        std::path::PathBuf::from("demo.db")
    }
}

// Why: compile-only assertion proving the headline D-15 / review-R3 crate-root
// DX promise. If any of these paths fails to resolve, the crate fails to
// build — no test runtime required.
#[allow(dead_code)]
const _ROOT_REEXPORT_SMOKE: fn() = || {
    let _: Option<&dyn AuthProvider> = None;
    let _: Option<&dyn SecretsProvider> = None;
    let _: Option<StaticAuthProvider> = None;
    let _: Option<EnvSecrets> = None;
    let _: Option<SecretValue> = None;
    let _: Option<SecretsProviderChain> = None;
    let _: Option<StaticResourceHandler> = None;
    let _: Option<StaticPromptHandler> = None;
    let _: Option<ServerConfig> = None;
    let _: Option<ConfigValidationError> = None;
    // Plan 05 (TKIT-07): synthesize_from_config is fn-typed; reference the
    // function pointer to assert the re-exported path resolves at the crate root.
    let _: fn(&ServerConfig) -> Result<Vec<crate::tools::SynthesizedTool>> = synthesize_from_config;
    // Plan 07 (TKIT-10): SqlConnector trait stub + Dialect enum re-exports.
    let _: Option<Dialect> = None;
    let _: Option<ConnectorError> = None;
    let _: Option<&dyn SqlConnector> = None;
    // Plan 08 (TKIT-08): ServerBuilderExt trait — the headline Shape C
    // surface. The trait is `Sized` (can't be `dyn`) — reference its method
    // pointer instead to assert the crate-root path resolves.
    let _: fn(pmcp::ServerBuilder, &ServerConfig) -> Result<pmcp::ServerBuilder> =
        <pmcp::ServerBuilder as ServerBuilderExt>::try_tools_from_config;
};

// Plan 06 (TKIT-06 + TKIT-09): compile-only assertion that the code_mode
// submodule's re-exports + wiring helpers resolve at `code_mode::*`. Gated on
// `code-mode` because the module itself is feature-gated (D-15 + D-16: the
// headline submodule, not a flattened crate-root surface).
#[cfg(feature = "code-mode")]
#[allow(dead_code)]
const _CODE_MODE_REEXPORT_SMOKE: fn() = || {
    let _: Option<Box<dyn crate::code_mode::CodeExecutor>> = None;
    let _: Option<crate::code_mode::ValidationPipeline> = None;
    let _: Option<crate::code_mode::TokenSecret> = None;
    let _: Option<crate::code_mode::HmacTokenGenerator> = None;
    let _: Option<crate::code_mode::ApprovalToken> = None;
    let _: Option<crate::code_mode::NoopPolicyEvaluator> = None;
    let _: fn(&ServerConfig) -> Result<crate::code_mode::ValidationPipeline> =
        crate::code_mode::validation_pipeline_from_config;
    // Plan 07 (TKIT-10 / D-12): assemble_code_mode_prompt re-exports at crate
    // root under the code-mode feature. The fn returns a `BoxFuture`-ish async
    // surface; reference the function pointer to assert the path resolves.
    let _ = assemble_code_mode_prompt;
};

// Phase 92 (WBSV-01/08/09 / D-11): compile-only assertion that the FULL workbook
// boot surface resolves at the crate root — the binding witness that a Shape A/B
// consumer registers a governed workbook WITHOUT naming `pmcp-workbook-runtime`.
// Gated on `workbook` because the module is feature-gated.
#[cfg(feature = "workbook")]
#[allow(dead_code)]
const _WORKBOOK_REEXPORT_SMOKE: fn() = || {
    // BundleSource (trait) + its on-disk impl + both error types — the loader
    // inputs/outputs consumers need from the crate root.
    let _: Option<&dyn BundleSource> = None;
    let _: Option<LocalDirSource> = None;
    let _: Option<BundleSourceError> = None;
    let _: Option<BundleLoadError> = None;
    // load_bundle (the fail-closed boot entry point) is fn-typed; reference its
    // function pointer to assert the re-exported path resolves at the crate root.
    let _: fn(
        &dyn BundleSource,
    ) -> std::result::Result<crate::workbook::WorkbookBundle, BundleLoadError> = load_bundle;
    // WorkbookBuilderExt (the headline one-call registration) is `Sized` (can't
    // be `dyn`) — reference its `try_` method pointer instead.
    let _: fn(pmcp::ServerBuilder, &dyn BundleSource) -> Result<pmcp::ServerBuilder> =
        <pmcp::ServerBuilder as WorkbookBuilderExt>::try_with_workbook_bundle;
};

// Phase 92 (WBSV-09): the embedded-source re-export resolves at the crate root
// when the `workbook-embedded` feature layers include_dir support on top.
#[cfg(feature = "workbook-embedded")]
#[allow(dead_code)]
const _WORKBOOK_EMBEDDED_REEXPORT_SMOKE: fn() = || {
    let _: Option<EmbeddedSource> = None;
};

#[cfg(test)]
mod demo_db_path_tests {
    use super::demo_db_path;
    use std::path::PathBuf;

    // Why: these tests mutate the process-global LAMBDA_TASK_ROOT env var. The
    // project runs `cargo test -- --test-threads=1` (CLAUDE.md), so they execute
    // serially and cannot race. Each test restores the prior state.
    #[test]
    fn returns_tmp_path_under_lambda() {
        let prev = std::env::var("LAMBDA_TASK_ROOT").ok();
        std::env::set_var("LAMBDA_TASK_ROOT", "/var/task");
        assert_eq!(demo_db_path(), PathBuf::from("/tmp/demo.db"));
        match prev {
            Some(v) => std::env::set_var("LAMBDA_TASK_ROOT", v),
            None => std::env::remove_var("LAMBDA_TASK_ROOT"),
        }
    }

    #[test]
    fn returns_relative_path_locally() {
        let prev = std::env::var("LAMBDA_TASK_ROOT").ok();
        std::env::remove_var("LAMBDA_TASK_ROOT");
        assert_eq!(demo_db_path(), PathBuf::from("demo.db"));
        if let Some(v) = prev {
            std::env::set_var("LAMBDA_TASK_ROOT", v);
        }
    }
}
