use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::deployment::post_deploy_tests::PostDeployTestsConfig;
use crate::deployment::widgets::WidgetsConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub target: TargetConfig,
    /// AWS configuration (required for `aws-lambda` and `pmcp-run` targets;
    /// absent for Cloud Run and other non-AWS targets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws: Option<AwsConfig>,
    /// GCP configuration (required for the `google-cloud-run` target;
    /// absent for AWS-only targets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gcp: Option<GcpConfig>,
    pub server: ServerConfig,
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub secrets: HashMap<String, String>,
    /// OAuth / identity configuration.
    ///
    /// Defaults to `AuthConfig::default()` (`enabled = false, provider = "none"`)
    /// when the `[auth]` block is omitted from `deploy.toml`. This lets minimum-
    /// schema deploy.toml files (per upstream issue #260) load without boilerplate
    /// `[auth] enabled=false, provider="none"` stanzas. Servers that own their
    /// own auth_provider() in-binary should leave `[auth]` omitted.
    #[serde(default)]
    pub auth: AuthConfig,
    /// Observability configuration.
    ///
    /// Defaults to `ObservabilityConfig::default()` (everything zero/disabled)
    /// when the `[observability]` block is omitted from `deploy.toml`. Per-field
    /// `#[serde(default)]` also makes partial blocks loadable — most fields are
    /// AWS-Lambda-specific and unused by Cloud Run, so omitting them is normal.
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_gateway: Option<ApiGatewayConfig>,
    /// Assets configuration for bundling files with deployment
    #[serde(default)]
    pub assets: AssetsConfig,

    /// Composition configuration for server-to-server communication
    #[serde(default)]
    pub composition: CompositionConfig,

    /// IAM declarations for the Lambda execution role (tables, buckets, raw statements).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the no-`[iam]`
    /// path so pre-existing `.pmcp/deploy.toml` files round-trip unchanged.
    #[serde(default, skip_serializing_if = "IamConfig::is_empty")]
    pub iam: IamConfig,

    /// Widget pre-build declarations (Phase 79).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the
    /// no-`[[widgets]]` path so pre-existing `.pmcp/deploy.toml` files
    /// round-trip unchanged (mirrors the Phase 76 IamConfig D-05 contract).
    #[serde(default, skip_serializing_if = "WidgetsConfig::is_empty")]
    pub widgets: WidgetsConfig,

    /// Post-deploy verification config (Phase 79).
    ///
    /// `Option::is_none` skip preserves byte-identity on the
    /// no-`[post_deploy_tests]` path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_deploy_tests: Option<PostDeployTestsConfig>,

    /// Opt-in layout descriptor for multi-crate isolated Cargo layouts.
    ///
    /// When present, drives surgical per-crate `COPY` lines in the generated
    /// Dockerfile and a `cargo build --manifest-path` build step. Used only
    /// by the `google-cloud-run` target today; ignored by AWS Lambda.
    /// See upstream issue #258 for the multi-crate isolated pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,

    /// Opt-out runtime-stage knob for the generated Dockerfile.
    ///
    /// When `base` is `None`, the scaffolder emits the target-appropriate
    /// secure default (currently `gcr.io/distroless/cc-debian12` for
    /// Cloud Run). When `base` is `Some`, the scaffolder uses that value
    /// verbatim. `apt_packages` is honored only when `base` resolves to a
    /// debian-family image. See upstream issue #259.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeConfig>,

    /// Typed `[azure]` section for the Azure Container Apps deploy target
    /// (Phase 80, REQ-80-04).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the
    /// no-`[azure]` path so pre-existing `.pmcp/deploy.toml` files round-trip
    /// unchanged (mirrors the Phase 76 `IamConfig` D-05 contract). See
    /// [`AzureConfig`].
    #[serde(default, skip_serializing_if = "AzureConfig::is_empty")]
    pub azure: AzureConfig,

    /// Curated template metadata (`server_type`, `snapshot_baked`) threaded
    /// into the generated CDK `stack.ts` (Phase 98, DSTK-02).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the
    /// no-`[metadata]` path so pre-existing `.pmcp/deploy.toml` files
    /// round-trip unchanged (mirrors the Phase 76 `IamConfig` D-05 contract).
    /// See [`MetadataConfig`].
    #[serde(default, skip_serializing_if = "MetadataConfig::is_empty")]
    pub metadata: MetadataConfig,

    /// Runtime opt-out carrier for the `stack.ts` regeneration guard
    /// (Phase 98, DSTK-01). When `true`, the deploy path overwrites an
    /// existing `deploy/lib/stack.ts`; when `false` (the default) an
    /// existing curated file is preserved.
    ///
    /// Set at runtime by the `--regenerate-stack`/`--force` flag (Plan 98-02)
    /// and never persisted — `#[serde(skip)]` mirrors the existing
    /// non-persisted `project_root` field, so toggling it produces no
    /// `regenerate_stack` key in the serialized TOML.
    #[serde(skip)]
    pub regenerate_stack: bool,

    /// Project root directory (not serialized)
    #[serde(skip)]
    pub project_root: PathBuf,
}

/// Curated template metadata for the generated CDK `stack.ts` (Phase 98).
///
/// Maps to an optional `[metadata]` block in `.pmcp/deploy.toml`:
///
/// ```toml
/// [metadata]
/// server_type = "graph-rag"
/// snapshot_baked = true
/// ```
///
/// Both fields are `Option` and elided when `None` via
/// `skip_serializing_if = "Option::is_none"`, so the enclosing
/// `#[serde(skip_serializing_if = "MetadataConfig::is_empty")]` guard on
/// `DeployConfig::metadata` keeps configs that do not opt in byte-identical
/// (the DSTK-02 backward-compat contract, mirroring `IamConfig`'s D-05).
///
/// `server_type` overrides the `mcp:serverType` template literal (otherwise
/// hardcoded `'custom'` for pmcp.toml/custom servers); `snapshot_baked`
/// drives the new `mcp:snapshotBaked` literal (no representation today). The
/// render-path plumbing for both lands in Plan 98-03.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataConfig {
    /// Override for the `mcp:serverType` template metadata literal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,

    /// Drives the `mcp:snapshotBaked` template metadata literal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_baked: Option<bool>,
}

impl MetadataConfig {
    /// Returns `true` when no metadata is declared (both fields `None`).
    ///
    /// Used by `DeployConfig`'s
    /// `#[serde(skip_serializing_if = "MetadataConfig::is_empty")]` to preserve
    /// byte-identity for configs without a `[metadata]` section (DSTK-02).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.server_type.is_none() && self.snapshot_baked.is_none()
    }
}

/// One-line notice printed by both deploy targets when an existing
/// `deploy/lib/stack.ts` is preserved (the regeneration guard skipped the
/// write). Factored as a `const` so both write sites emit an identical
/// string (Phase 98, DSTK-01).
//
// Why allow(dead_code): both consumers (commands::deploy::deploy and
// targets::pmcp_run::deploy) live in the bin-only tree. config.rs is ALSO
// mounted into the lib via `#[path]` (see lib.rs), where this const has no
// caller — so the lib build alone reports it unused. It is exercised by the
// in-crate test `preserved_notice_names_the_regenerate_flag` and used by the
// bin build.
#[allow(dead_code)]
pub(crate) const STACK_TS_PRESERVED_NOTICE: &str =
    "preserved existing deploy/lib/stack.ts (pass --regenerate-stack to overwrite)";

/// Write `stack.ts` into `lib_dir`, guarded by an existence check so a
/// pre-existing operator-curated file is never silently overwritten
/// (Phase 98, DSTK-01).
///
/// Behavior:
/// - The `lib_dir` is always created (`create_dir_all`).
/// - When `deploy/lib/stack.ts` does NOT exist, it is written unconditionally
///   (first-deploy scaffold) regardless of `regenerate`.
/// - When it EXISTS and `regenerate == false`, the write is SKIPPED and the
///   file is left byte-for-byte unchanged.
/// - When it EXISTS and `regenerate == true`, it is overwritten.
///
/// Returns `Ok(true)` when the file was written, `Ok(false)` when an existing
/// file was preserved. Callers print [`STACK_TS_PRESERVED_NOTICE`] on `false`.
///
/// This helper is deliberately decoupled from IAM validation: callers run
/// validation BEFORE invoking it, so the validation step is never skipped on
/// the preserve path.
//
// Why allow(dead_code): the production callers are bin-only (see
// STACK_TS_PRESERVED_NOTICE above); config.rs is also mounted into the lib
// via `#[path]`, where the non-test build sees no caller. Exercised by the
// in-crate `stack_ts_guard_tests` and used by the bin build.
#[allow(dead_code)]
pub(crate) fn write_stack_ts_guarded(
    lib_dir: &Path,
    stack_ts: &str,
    regenerate: bool,
) -> Result<bool> {
    std::fs::create_dir_all(lib_dir).context("Failed to create deploy/lib directory")?;
    let path = lib_dir.join("stack.ts");
    if path.exists() && !regenerate {
        return Ok(false);
    }
    std::fs::write(&path, stack_ts).context("Failed to write deploy/lib/stack.ts")?;
    Ok(true)
}

/// Build the prominent, multi-line warning emitted by both deploy targets when
/// an operator-curated `deploy/lib/stack.ts` was PRESERVED (the regeneration
/// guard skipped the write) yet `.pmcp/deploy.toml` still declares `[iam]`
/// and/or `[environment]` sections whose delivery depends on the stack.ts.
///
/// Returns `None` when both sections are empty (nothing surprising to warn
/// about). Callers only invoke this on the preserve path, so a `Some` result
/// always corresponds to a preserved custom stack.ts.
///
/// Why this exists (Phase 98 follow-up — `deploy-toml-inert-for-preserved-stack`):
/// the `#279`/DSTK-01 exists-guard preserves a curated stack.ts by existence
/// alone, which silently severs the `{iam_block}` splice (IAM is only injected
/// when stack.ts is (re)generated). Previously `[environment]` had the same
/// hazard, but the pmcp-run target now merges `[environment]` construct-
/// agnostically into the synthesized template post-synth
/// (`environment-inert-for-shared-cdk-constructs`), so a preserved stack.ts no
/// longer blocks it there; the aws-lambda `cdk deploy` path still relies on the
/// curated stack.ts reading `process.env`. This warning makes the remaining gap
/// loud at deploy time without blocking the deploy (the operator may have
/// deliberately inlined the equivalents directly in stack.ts).
//
// Why allow(dead_code): identical rationale to STACK_TS_PRESERVED_NOTICE — the
// production callers live in the bin-only tree, while config.rs is also mounted
// into the lib via `#[path]` (see lib.rs) where no caller exists. Exercised by
// the in-crate `stack_ts_guard_tests` and used by the bin build.
#[allow(dead_code)]
pub(crate) fn stack_ts_preserved_inert_warning(
    iam_is_empty: bool,
    environment_is_empty: bool,
) -> Option<String> {
    if iam_is_empty && environment_is_empty {
        return None;
    }

    let mut lines = vec![
        "⚠️  stack.ts PRESERVED — some .pmcp/deploy.toml sections are NOT auto-applied:"
            .to_string(),
    ];
    if !iam_is_empty {
        lines.push(
            "   • [iam] / [[iam.statements]]: spliced into stack.ts ONLY when it is (re)generated."
                .to_string(),
        );
        lines.push(
            "     A preserved custom stack.ts keeps its own role policy. To apply the deploy.toml"
                .to_string(),
        );
        lines.push(
            "     IAM, run `cargo pmcp deploy --regenerate-stack`, or add the addToRolePolicy(...)"
                .to_string(),
        );
        lines.push("     statements directly in stack.ts.".to_string());
    }
    if !environment_is_empty {
        lines.push(
            "   • [environment]: on the pmcp-run target it is merged construct-agnostically into"
                .to_string(),
        );
        lines.push(
            "     every AWS::Lambda::Function's Environment.Variables in the synthesized template,"
                .to_string(),
        );
        lines.push(
            "     so a preserved stack.ts no longer blocks it (declared keys override construct"
                .to_string(),
        );
        lines.push(
            "     defaults). On the aws-lambda target it still reaches the Lambda only if stack.ts"
                .to_string(),
        );
        lines.push("     reads it via process.env.<KEY> in its environment:{} block.".to_string());
    }
    Some(lines.join("\n"))
}

/// Composition configuration for MCP server-to-server communication.
///
/// Enables servers to be composed in a tiered architecture:
/// - `foundation` tier: Core data connectors (databases, CRMs, APIs)
/// - `domain` tier: Business logic servers that call foundation servers
///
/// # Example Configuration
///
/// ```toml
/// [composition]
/// tier = "foundation"
/// allow_composition = true
/// internal_only = false
/// description = "Database explorer providing SQL query execution"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionConfig {
    /// Server tier: "foundation" or "domain"
    #[serde(default = "default_tier")]
    pub tier: String,

    /// Whether this server can be called by other servers
    #[serde(default = "default_true")]
    pub allow_composition: bool,

    /// Whether this server is only available internally (not exposed via API)
    #[serde(default)]
    pub internal_only: bool,

    /// Description for composition discovery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_tier() -> String {
    "foundation".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(rename = "type")]
    pub target_type: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

/// Google Cloud configuration for the `google-cloud-run` target.
///
/// Mirrors [`AwsConfig`] in shape (project + region) and is required when
/// `target.type = "google-cloud-run"`. See upstream issue #260.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpConfig {
    /// GCP project id (e.g. `"my-project-123"`). Populated as a placeholder
    /// on `cargo pmcp deploy init` and edited by the operator before deploy.
    pub project_id: String,
    /// Cloud Run region (e.g. `"us-central1"`).
    pub region: String,
}

/// Cargo layout descriptor for the multi-crate isolated pattern.
///
/// Non-workspace sibling crates with path-dep relationships — for example, a
/// `gcp-cloud-run` binary crate that declares `auth-echo-core = { path = "../auth-echo-core" }`
/// — cannot be handled by either of the two default Dockerfile templates
/// (workspace `COPY . .` over-bundles; simple-crate `COPY src` under-bundles).
/// Setting `kind = "multi-crate-isolated"` plus `primary` + `path_deps`
/// produces a surgical Dockerfile with per-crate COPY lines and a
/// `cargo build --manifest-path <primary>/Cargo.toml` build step.
///
/// See upstream issue #258.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Layout kind. Currently only `"multi-crate-isolated"` is recognized;
    /// any other value is treated as the default template (workspace or
    /// simple binary, depending on `Cargo.toml` detection).
    pub kind: String,
    /// Sibling directory containing the binary crate to deploy (e.g.
    /// `"gcp-cloud-run"`). The build runs `cargo build --manifest-path
    /// <primary>/Cargo.toml`.
    pub primary: String,
    /// Sibling crate directories declared as path dependencies by the
    /// primary crate (e.g. `["auth-echo-core"]`). Each entry is copied into
    /// the Docker build context as a `Cargo.toml` + `src/` pair so the
    /// build can resolve them.
    #[serde(default)]
    pub path_deps: Vec<String>,
}

impl LayoutConfig {
    /// Returns `true` when this layout uses the multi-crate isolated pattern,
    /// which drives surgical per-crate `COPY` lines in the generated Dockerfile.
    #[must_use]
    pub fn is_multi_crate_isolated(&self) -> bool {
        self.kind == "multi-crate-isolated"
    }
}

/// Runtime-stage Docker base-image knob.
///
/// When `base` is `None`, the scaffolder emits the target-appropriate
/// secure default: `gcr.io/distroless/cc-debian12` for Cloud Run (no shell,
/// no package manager, ~4× smaller than `debian:bookworm-slim`). When
/// `base` is set, the scaffolder uses that image verbatim.
///
/// `apt_packages` is honored only when `base` resolves to a debian-family
/// image (image ref starting with `debian:` or `ubuntu:`). It triggers a
/// `RUN apt-get update && apt-get install -y <packages>` layer; an empty
/// list (the default) produces no apt layer.
///
/// See upstream issue #259.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    /// Runtime-stage `FROM` image. `None` selects the target default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Apt packages to install on debian/ubuntu bases. Ignored otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apt_packages: Vec<String>,
}

/// Typed `[azure]` deploy.toml section for the Azure Container Apps target
/// (Phase 80, REQ-80-04).
///
/// Mirrors the [`IamConfig`] / [`WidgetsConfig`] backward-compat contract:
/// the `skip_serializing_if = "AzureConfig::is_empty"` guard on
/// `DeployConfig::azure` preserves byte-identity on the no-`[azure]` path so
/// pre-Phase-80 `.pmcp/deploy.toml` files round-trip unchanged (D-05).
///
/// `resource_group`/`environment` stay `None` when unset so the consumer
/// (`AcaSettings::from_config`, landing in 80-02) can keep its
/// `<server-name>-rg` / `<server-name>-env` fallbacks. Precedence is
/// ENV (`AZURE_*`) > `[azure]` section > these built-in defaults.
///
/// # Example
///
/// ```toml
/// [azure]
/// location = "westus2"
/// resource_group = "my-rg"
/// target_port = 9090
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    /// Azure resource group. Falls back to `<server-name>-rg` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_group: Option<String>,

    /// Container Apps environment. Falls back to `<server-name>-env` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,

    /// Azure region. Defaults to `eastus`.
    #[serde(default = "default_azure_location")]
    pub location: String,

    /// Ingress target port. Defaults to `8080`.
    #[serde(default = "default_azure_target_port")]
    pub target_port: u16,

    /// Minimum replica count. Defaults to `1`.
    #[serde(default = "default_azure_min_replicas")]
    pub min_replicas: u32,
}

fn default_azure_location() -> String {
    "eastus".to_string()
}

fn default_azure_target_port() -> u16 {
    8080
}

fn default_azure_min_replicas() -> u32 {
    1
}

impl Default for AzureConfig {
    fn default() -> Self {
        Self {
            resource_group: None,
            environment: None,
            location: default_azure_location(),
            target_port: default_azure_target_port(),
            min_replicas: default_azure_min_replicas(),
        }
    }
}

impl AzureConfig {
    /// Returns `true` when every field equals its built-in default
    /// (`resource_group`/`environment` `None`, `location == "eastus"`,
    /// `target_port == 8080`, `min_replicas == 1`).
    ///
    /// Used by `DeployConfig`'s
    /// `#[serde(skip_serializing_if = "AzureConfig::is_empty")]` to preserve
    /// byte-identity for configs without an `[azure]` section (D-05).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resource_group.is_none()
            && self.environment.is_none()
            && self.location == default_azure_location()
            && self.target_port == default_azure_target_port()
            && self.min_replicas == default_azure_min_replicas()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    /// Lambda memory in MB. Used by AWS targets; ignored by Cloud Run
    /// (which uses [`Self::memory`]). Defaults to 512 to keep the schema
    /// loadable for Cloud Run deploy.toml files that omit it.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    /// Lambda timeout in seconds. Used by AWS targets; ignored by Cloud Run.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved_concurrency: Option<u32>,
    /// Cloud Run memory limit in `Mi`/`Gi` form (e.g. `"256Mi"`, `"1Gi"`).
    /// `None` → use Cloud Run default (`"512Mi"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// Cloud Run CPU allocation (e.g. `"1"`, `"2"`). `None` → use `"1"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// Cloud Run ingress (`"all"` | `"internal"` |
    /// `"internal-and-cloud-load-balancing"`). `None` → use Cloud Run default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<String>,
    /// Cloud Run public-access flag. `None` → fall back to env var
    /// `CLOUD_RUN_ALLOW_UNAUTHENTICATED` then to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_unauthenticated: Option<bool>,
    /// Cloud Run autoscaler max instances. `None` → `10`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_instances: Option<u32>,
    /// Cloud Run autoscaler min instances. `None` → `0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_instances: Option<u32>,
    /// Binary name to build (passed as `cargo build --bin <binary>`).
    /// Used by the multi-crate isolated layout; falls back to
    /// [`Self::name`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
}

fn default_memory_mb() -> u32 {
    512
}

fn default_timeout_seconds() -> u32 {
    30
}

/// OAuth authentication configuration for MCP servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Whether OAuth is enabled
    pub enabled: bool,

    /// OAuth provider type (cognito, oidc, none)
    #[serde(default = "default_oauth_provider")]
    pub provider: String,

    /// Cognito-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognito: Option<CognitoConfig>,

    /// External OIDC provider configuration (future)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc: Option<OidcConfig>,

    /// Dynamic Client Registration settings
    #[serde(default)]
    pub dcr: DcrConfig,

    /// OAuth scopes configuration
    #[serde(default)]
    pub scopes: ScopesConfig,

    /// Legacy fields for backwards compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default)]
    pub callback_urls: Vec<String>,
}

fn default_oauth_provider() -> String {
    "none".to_string()
}

/// Cognito-specific OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoConfig {
    /// Cognito User Pool ID (created or existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,

    /// User Pool name (used when creating new)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_name: Option<String>,

    /// Resource server identifier for scopes
    #[serde(default = "default_resource_server_id")]
    pub resource_server_id: String,

    /// Social identity providers
    #[serde(default)]
    pub social_providers: Vec<String>,

    /// MFA setting (optional, required, off)
    #[serde(default = "default_mfa")]
    pub mfa: String,

    /// Access token TTL
    #[serde(default = "default_access_token_ttl")]
    pub access_token_ttl: String,

    /// Refresh token TTL
    #[serde(default = "default_refresh_token_ttl")]
    pub refresh_token_ttl: String,

    /// Hosted UI domain prefix (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

fn default_resource_server_id() -> String {
    "mcp".to_string()
}

fn default_mfa() -> String {
    "optional".to_string()
}

fn default_access_token_ttl() -> String {
    "1h".to_string()
}

fn default_refresh_token_ttl() -> String {
    "30d".to_string()
}

/// External OIDC provider configuration.
///
/// Supports multiple OIDC providers via the `provider_type` field:
/// - `google` - Google Identity Platform
/// - `auth0` - Auth0
/// - `okta` - Okta
/// - `entra` - Microsoft Entra ID (Azure AD)
/// - `generic` - Any OIDC-compliant provider
///
/// The SDK's `GenericOidcProvider` is used for token validation at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    /// OIDC provider type (google, auth0, okta, entra, generic)
    #[serde(default = "default_oidc_provider_type")]
    pub provider_type: String,

    /// OIDC issuer URL
    pub issuer: String,

    /// Client ID for this application
    pub client_id: String,

    /// Client secret (optional for public clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Expected audience (defaults to client_id if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,

    /// JWKS URL (auto-discovered if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_url: Option<String>,

    /// Custom scopes to request (in addition to openid)
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
}

fn default_oidc_provider_type() -> String {
    "generic".to_string()
}

fn default_oidc_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

/// Dynamic Client Registration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrConfig {
    /// Enable DCR endpoint
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Patterns to auto-detect public clients (no secret required)
    #[serde(default = "default_public_client_patterns")]
    pub public_client_patterns: Vec<String>,

    /// Default scopes for new clients
    #[serde(default = "default_scopes")]
    pub default_scopes: Vec<String>,

    /// Allowed scopes clients can request
    #[serde(default = "default_allowed_scopes")]
    pub allowed_scopes: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_public_client_patterns() -> Vec<String> {
    vec![
        "claude".to_string(),
        "desktop".to_string(),
        "cursor".to_string(),
        "mcp-inspector".to_string(),
        "chatgpt".to_string(),
    ]
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "mcp/read".to_string(),
    ]
}

fn default_allowed_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
        "mcp/read".to_string(),
        "mcp/write".to_string(),
    ]
}

/// OAuth scopes configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScopesConfig {
    /// Custom scope definitions
    #[serde(default)]
    pub custom: std::collections::HashMap<String, String>,
}

impl Default for CognitoConfig {
    fn default() -> Self {
        Self {
            user_pool_id: None,
            user_pool_name: None,
            resource_server_id: default_resource_server_id(),
            social_providers: vec![],
            mfa: default_mfa(),
            access_token_ttl: default_access_token_ttl(),
            refresh_token_ttl: default_refresh_token_ttl(),
            domain: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub log_retention_days: u32,
    #[serde(default)]
    pub enable_xray: bool,
    #[serde(default)]
    pub create_dashboard: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarms: Option<AlarmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    pub error_threshold: u32,
    pub latency_threshold_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGatewayConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_limit: Option<u32>,
}

/// Assets configuration for bundling files with deployment.
///
/// Assets are files bundled with your MCP server (databases, markdown files, configs).
/// They are accessible at runtime via `pmcp::assets` module.
///
/// # Example Configuration
///
/// ```toml
/// [assets]
/// include = [
///     "chinook.db",
///     "resources/**/*.md",
///     "config/*.toml",
/// ]
/// exclude = ["**/*.tmp", "**/.DS_Store"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsConfig {
    /// Glob patterns for files to include as assets.
    ///
    /// Patterns are relative to the workspace root.
    /// Examples: `"chinook.db"`, `"resources/**/*.md"`, `"config/*.toml"`
    #[serde(default)]
    pub include: Vec<String>,

    /// Glob patterns for files to exclude from assets.
    ///
    /// Applied after include patterns.
    /// Examples: `"**/*.tmp"`, `"**/.DS_Store"`
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Base directory for assets (relative to workspace root).
    ///
    /// If not specified, assets are resolved from workspace root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<String>,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![
                "**/*.tmp".to_string(),
                "**/.DS_Store".to_string(),
                "**/Thumbs.db".to_string(),
            ],
            base_dir: None,
        }
    }
}

impl AssetsConfig {
    /// Create a new assets config with the given include patterns.
    #[allow(dead_code)]
    pub fn new(include: Vec<String>) -> Self {
        Self {
            include,
            ..Default::default()
        }
    }

    /// Check if any assets are configured.
    pub fn has_assets(&self) -> bool {
        !self.include.is_empty()
    }

    /// Resolve asset patterns to actual file paths.
    pub fn resolve_files(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let base = match &self.base_dir {
            Some(dir) => project_root.join(dir),
            None => project_root.to_path_buf(),
        };

        let mut files = Vec::new();

        for pattern in &self.include {
            let full_pattern = base.join(pattern);
            let glob_pattern = full_pattern.to_string_lossy();

            let paths =
                glob::glob(&glob_pattern).context(format!("Invalid glob pattern: {}", pattern))?;

            for entry in paths.flatten() {
                if entry.is_file() && !self.is_excluded(&entry) {
                    files.push(entry);
                }
            }
        }

        Ok(files)
    }

    /// Check if a path matches any exclude pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.exclude {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                if glob_pattern.matches(&path_str) {
                    return true;
                }
            }
        }

        false
    }
}

impl DeployConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".pmcp/deploy.toml");

        if !config_path.exists() {
            anyhow::bail!("Deployment not initialized. Run: cargo pmcp deploy init");
        }

        let config_str =
            std::fs::read_to_string(&config_path).context("Failed to read .pmcp/deploy.toml")?;

        let mut config: Self =
            toml::from_str(&config_str).context("Failed to parse .pmcp/deploy.toml")?;

        // Set the project root
        config.project_root = project_root.to_path_buf();

        // Auto-detect and merge template-required assets
        config.auto_configure_template_assets(project_root);

        Ok(config)
    }

    /// Auto-configure assets based on detected templates in the workspace.
    ///
    /// This scans the workspace config for templates that require specific assets
    /// (e.g., sqlite-explorer requires chinook.db) and adds them to the assets config
    /// if they exist in the workspace and aren't already configured.
    fn auto_configure_template_assets(&mut self, project_root: &Path) {
        // Load workspace config to detect templates
        let workspace_config = match crate::utils::config::WorkspaceConfig::load() {
            Ok(config) => config,
            Err(_) => return, // No workspace config, skip auto-detection
        };

        // Collect template-required assets
        let mut required_assets: Vec<String> = Vec::new();

        for server in workspace_config.servers.values() {
            // sqlite-explorer / db-explorer templates require chinook.db.
            // (Add other templates with asset requirements here.)
            if matches!(server.template.as_str(), "sqlite-explorer" | "db-explorer")
                && !required_assets.contains(&"chinook.db".to_string())
            {
                required_assets.push("chinook.db".to_string());
            }
        }

        // Add required assets that exist and aren't already configured
        for asset in required_assets {
            let asset_path = project_root.join(&asset);
            if asset_path.exists() && !self.assets.include.contains(&asset) {
                self.assets.include.push(asset);
            }
        }
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&config_dir).context("Failed to create .pmcp directory")?;

        let config_path = config_dir.join("deploy.toml");
        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, config_str).context("Failed to write .pmcp/deploy.toml")?;

        Ok(())
    }

    /// Persist this config to `.pmcp/deploy.toml` only when the file
    /// does not already exist. Returns `Ok(true)` if a file was written
    /// and `Ok(false)` if an existing deploy.toml was preserved.
    ///
    /// Why: `cargo pmcp deploy init` is expected to be idempotent —
    /// re-running it on a project where the operator has already filled
    /// in env vars, IAM bindings, or other tuning must not clobber that
    /// work. (Scaffolder-immunity invariant from upstream issue #260.)
    pub fn save_if_missing(&self, project_root: &Path) -> Result<bool> {
        let config_path = project_root.join(".pmcp/deploy.toml");
        if config_path.exists() {
            return Ok(false);
        }
        self.save(project_root)?;
        Ok(true)
    }

    pub fn default_for_server(server_name: String, region: String, project_root: PathBuf) -> Self {
        let mut environment = HashMap::new();
        environment.insert("RUST_LOG".to_string(), "info".to_string());

        Self {
            target: TargetConfig {
                target_type: "aws-lambda".to_string(),
                version: "1.0.0".to_string(),
            },
            aws: Some(AwsConfig {
                region,
                account_id: None,
            }),
            gcp: None,
            server: ServerConfig {
                name: server_name,
                memory_mb: 512,
                timeout_seconds: 30,
                reserved_concurrency: None,
                memory: None,
                cpu: None,
                ingress: None,
                allow_unauthenticated: None,
                max_instances: None,
                min_instances: None,
                binary: None,
            },
            environment,
            secrets: HashMap::new(),
            auth: AuthConfig::default(),
            observability: ObservabilityConfig {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: Some(AlarmConfig {
                    error_threshold: 10,
                    latency_threshold_ms: 5000,
                }),
            },
            api_gateway: None,
            assets: AssetsConfig::default(),
            composition: CompositionConfig::default(),
            iam: IamConfig::default(),
            widgets: WidgetsConfig::default(),
            post_deploy_tests: None,
            layout: None,
            runtime: None,
            azure: AzureConfig::default(),
            metadata: MetadataConfig::default(),
            regenerate_stack: false,
            project_root,
        }
    }

    /// Construct a default [`DeployConfig`] for the `google-cloud-run` target.
    ///
    /// Mirrors [`Self::default_for_server`] but populates the schema shape
    /// required for Cloud Run deployments (per upstream issue #260): the
    /// `[gcp]` block replaces `[aws]`, and the `[server]` block uses Cloud
    /// Run-flavored fields (`memory` as a string like `"256Mi"`, `cpu`,
    /// `ingress`, `allow_unauthenticated`).
    pub fn default_for_cloud_run_server(
        server_name: String,
        project_id: String,
        region: String,
        project_root: PathBuf,
    ) -> Self {
        let mut environment = HashMap::new();
        environment.insert("RUST_LOG".to_string(), "info".to_string());

        Self {
            target: TargetConfig {
                target_type: "google-cloud-run".to_string(),
                version: "1.0.0".to_string(),
            },
            aws: None,
            gcp: Some(GcpConfig { project_id, region }),
            server: ServerConfig {
                name: server_name,
                // memory_mb / timeout_seconds are AWS-specific; defaults are
                // retained for serde-load symmetry but never read by the
                // Cloud Run target.
                memory_mb: default_memory_mb(),
                timeout_seconds: default_timeout_seconds(),
                reserved_concurrency: None,
                memory: Some("256Mi".to_string()),
                cpu: Some("1".to_string()),
                ingress: Some("all".to_string()),
                allow_unauthenticated: Some(true),
                max_instances: Some(10),
                min_instances: Some(0),
                binary: None,
            },
            environment,
            secrets: HashMap::new(),
            auth: AuthConfig::default(),
            observability: ObservabilityConfig {
                // AWS-specific observability fields; Cloud Run observability
                // (Cloud Logging / Cloud Trace) is deferred per #260 §5.
                log_retention_days: 30,
                enable_xray: false,
                create_dashboard: false,
                alarms: None,
            },
            api_gateway: None,
            assets: AssetsConfig::default(),
            composition: CompositionConfig::default(),
            iam: IamConfig::default(),
            widgets: WidgetsConfig::default(),
            post_deploy_tests: None,
            layout: None,
            runtime: None,
            azure: AzureConfig::default(),
            metadata: MetadataConfig::default(),
            regenerate_stack: false,
            project_root,
        }
    }

    /// Return a reference to the AWS configuration block.
    ///
    /// Panics with a clear message when called on a non-AWS deploy config.
    /// Call sites that only run for AWS targets (`aws-lambda`, `pmcp-run`)
    /// use this to avoid an `Option::unwrap` everywhere.
    ///
    /// # Panics
    ///
    /// Panics if the `[aws]` block is absent (e.g. a `google-cloud-run`
    /// config). Non-AWS code paths must read [`Self::aws`] (the `Option`)
    /// directly rather than calling this accessor.
    #[must_use]
    pub fn aws(&self) -> &AwsConfig {
        self.aws.as_ref().expect(
            "deploy.toml is missing the required [aws] block — \
             this code path requires an AWS target (aws-lambda or pmcp-run)",
        )
    }

    /// Build the transient env-var map handed to the CDK child process
    /// (`DeployExecutor.extra_env` on the aws-lambda path).
    ///
    /// Combines developer-declared `[environment]` values with the deploy-time
    /// resolved `[secrets]`. Both flow identically as CDK process env vars; the
    /// generated/curated `stack.ts` decides which keys it consumes via
    /// `process.env`, so a hardcoded literal in the stack.ts `environment:{}`
    /// block still takes final precedence — `[environment]` is additive-fill,
    /// exactly like `[secrets]` behave today (fix for
    /// `deploy-toml-inert-for-preserved-stack`, FIX #2).
    ///
    /// # Precedence
    ///
    /// On a key collision between `[environment]` and `[secrets]`, the
    /// **secret wins**: a resolved `[secrets]` value is the authoritative,
    /// sensitive value and must not be shadowed by a plain `[environment]`
    /// default. (This mirrors the pre-fix behaviour where `[secrets]` were the
    /// sole contents of `extra_env`.)
    ///
    /// This map is transient (per D-05/D-06): never written back to
    /// `deploy.toml`.
    #[must_use]
    pub fn deploy_env_vars(&self) -> HashMap<String, String> {
        let mut env = self.environment.clone();
        // Secrets overlay environment: a resolved secret is authoritative and
        // must win on key collision (see # Precedence above).
        for (key, value) in &self.secrets {
            env.insert(key.clone(), value.clone());
        }
        env
    }

    /// Create config with OAuth enabled using Cognito
    pub fn with_cognito_oauth(
        server_name: String,
        region: String,
        project_root: PathBuf,
        cognito_config: CognitoConfig,
    ) -> Self {
        let mut config = Self::default_for_server(server_name, region, project_root);
        config.auth = AuthConfig {
            enabled: true,
            provider: "cognito".to_string(),
            cognito: Some(cognito_config),
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: None,
            callback_urls: vec![],
        };
        config
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "none".to_string(),
            cognito: None,
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: None,
            callback_urls: vec![],
        }
    }
}

impl AuthConfig {
    /// Convert auth config to environment variables for the server.
    ///
    /// These environment variables are read by the server_common template
    /// to initialize the appropriate `IdentityProvider` at runtime.
    #[allow(dead_code)]
    pub fn to_env_vars(&self, region: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if !self.enabled {
            vars.insert("AUTH_PROVIDER".to_string(), "none".to_string());
            return vars;
        }

        match self.provider.as_str() {
            "cognito" => {
                vars.insert("AUTH_PROVIDER".to_string(), "cognito".to_string());
                vars.insert("AUTH_REGION".to_string(), region.to_string());

                // Use cognito config if available, fall back to legacy fields
                if let Some(cognito) = &self.cognito {
                    if let Some(user_pool_id) = &cognito.user_pool_id {
                        vars.insert("AUTH_USER_POOL_ID".to_string(), user_pool_id.clone());
                    }
                } else if let Some(user_pool_id) = &self.user_pool_id {
                    vars.insert("AUTH_USER_POOL_ID".to_string(), user_pool_id.clone());
                }

                // Client ID from cognito config or legacy field
                if let Some(client_id) = &self.client_id {
                    vars.insert("AUTH_CLIENT_ID".to_string(), client_id.clone());
                }
            },
            "oidc" | "google" | "auth0" | "okta" | "entra" => {
                if let Some(oidc) = &self.oidc {
                    vars.insert("AUTH_PROVIDER".to_string(), oidc.provider_type.clone());
                    vars.insert("AUTH_ISSUER".to_string(), oidc.issuer.clone());
                    vars.insert("AUTH_CLIENT_ID".to_string(), oidc.client_id.clone());

                    if let Some(secret) = &oidc.client_secret {
                        vars.insert("AUTH_CLIENT_SECRET".to_string(), secret.clone());
                    }
                } else {
                    // Fallback to generic OIDC with provider as type
                    vars.insert("AUTH_PROVIDER".to_string(), self.provider.clone());
                }
            },
            _ => {
                vars.insert("AUTH_PROVIDER".to_string(), "none".to_string());
            },
        }

        vars
    }

    /// Create auth config for Cognito.
    #[allow(dead_code)]
    pub fn cognito(_region: &str, user_pool_id: &str, client_id: &str) -> Self {
        Self {
            enabled: true,
            provider: "cognito".to_string(),
            cognito: Some(CognitoConfig {
                user_pool_id: Some(user_pool_id.to_string()),
                user_pool_name: None,
                resource_server_id: default_resource_server_id(),
                social_providers: vec![],
                mfa: default_mfa(),
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
                domain: None,
            }),
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: Some(user_pool_id.to_string()),
            client_id: Some(client_id.to_string()),
            callback_urls: vec![],
        }
    }

    /// Create auth config for generic OIDC.
    #[allow(dead_code)]
    pub fn oidc(provider_type: &str, issuer: &str, client_id: &str) -> Self {
        Self {
            enabled: true,
            provider: "oidc".to_string(),
            cognito: None,
            oidc: Some(OidcConfig {
                provider_type: provider_type.to_string(),
                issuer: issuer.to_string(),
                client_id: client_id.to_string(),
                client_secret: None,
                audience: None,
                jwks_url: None,
                scopes: default_oidc_scopes(),
            }),
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: Some(client_id.to_string()),
            callback_urls: vec![],
        }
    }
}

impl Default for DcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            public_client_patterns: default_public_client_patterns(),
            default_scopes: default_scopes(),
            allowed_scopes: default_allowed_scopes(),
        }
    }
}

impl Default for CompositionConfig {
    fn default() -> Self {
        Self {
            tier: default_tier(),
            allow_composition: true,
            internal_only: false,
            description: None,
        }
    }
}

/// IAM declarations for the deployed Lambda's execution role.
///
/// Translated by [`crate::deployment::iam::render_iam_block`] into
/// `mcpFunction.addToRolePolicy(...)` calls in the generated CDK `stack.ts`.
/// An empty `IamConfig` is elided from serialised TOML via the
/// `skip_serializing_if = "IamConfig::is_empty"` guard on `DeployConfig::iam`,
/// so files without an `[iam]` section round-trip byte-identically.
///
/// See [`TablePermission`], [`BucketPermission`], and [`IamStatement`] for
/// the per-kind details.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IamConfig {
    /// DynamoDB table permissions (sugar form — see [`TablePermission`]).
    #[serde(default)]
    pub tables: Vec<TablePermission>,

    /// S3 bucket permissions (sugar form — object-level ARNs only — see
    /// [`BucketPermission`]).
    #[serde(default)]
    pub buckets: Vec<BucketPermission>,

    /// Raw IAM policy statements (passthrough after the validator).
    #[serde(default)]
    pub statements: Vec<IamStatement>,
}

impl IamConfig {
    /// Returns `true` when the IAM config contains no declarations
    /// (all three vectors are empty).
    ///
    /// Used by `DeployConfig`'s
    /// `#[serde(skip_serializing_if = "IamConfig::is_empty")]` to preserve
    /// byte-identity for configs without an `[iam]` section (D-05).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.buckets.is_empty() && self.statements.is_empty()
    }
}

/// DynamoDB table permission (sugar form).
///
/// `actions` is a subset of `{"read", "write", "readwrite"}`. Wave 3's
/// translation (`cargo_pmcp::deployment::iam::render_table`) expands each
/// sugar keyword into the 4-action DynamoDB list per 76-CONTEXT.md D-02:
/// - `read` → `GetItem`, `Query`, `Scan`, `BatchGetItem`
/// - `write` → `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem`
/// - `readwrite` → union (8 actions)
///
/// When `include_indexes = true`, the resource list additionally grants
/// `arn:aws:dynamodb:${this.region}:${this.account}:table/NAME/index/*`
/// (GSI/LSI access).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    /// Table name (AWS region + account inherited from the deploy context).
    pub name: String,

    /// Sugar keywords — subset of `{"read", "write", "readwrite"}`. Unknown
    /// values are rejected by Wave 4's validator.
    pub actions: Vec<String>,

    /// When `true`, grants access to `table/NAME/index/*` (GSI/LSI).
    #[serde(default)]
    pub include_indexes: bool,
}

/// S3 bucket permission (sugar form — object-level ARN only).
///
/// Wave 3's translation (`cargo_pmcp::deployment::iam::render_bucket`)
/// expands sugar keywords per 76-CONTEXT.md D-02:
/// - `read` → `GetObject`
/// - `write` → `PutObject`, `DeleteObject`
/// - `readwrite` → union (3 actions)
///
/// The resource ARN is `arn:aws:s3:::NAME/*` (object-level only).
/// Bucket-level operations (e.g. `s3:ListBucket`) must be declared via
/// `[[iam.statements]]` per CR scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketPermission {
    /// Bucket name.
    pub name: String,

    /// Sugar keywords — subset of `{"read", "write", "readwrite"}`.
    pub actions: Vec<String>,
}

/// Raw IAM `PolicyStatement` (passthrough after Wave 4 validation).
///
/// Emitted verbatim as
/// `new iam.PolicyStatement({ effect, actions, resources })` in the generated
/// `stack.ts`. Wave 4's validator enforces:
/// - `effect` is `"Allow"` or `"Deny"`
/// - `actions` is non-empty, each matching `^[a-z0-9-]+:[A-Za-z0-9*]+$`
/// - `resources` is non-empty (ARN or `*`)
/// - Rejects Allow `*:*` with resources `*`
/// - Warns on cross-account ARNs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamStatement {
    /// `"Allow"` or `"Deny"` (validated in Wave 4).
    pub effect: String,

    /// Non-empty list of `service:action` strings; `*` permitted as an
    /// action-name wildcard.
    pub actions: Vec<String>,

    /// Non-empty list of ARNs (or `"*"`). Cross-account ARNs emit a validator
    /// warning in Wave 4.
    pub resources: Vec<String>,
}

#[cfg(test)]
mod iam_wave1_tests {
    use super::*;

    #[test]
    fn iam_config_default_is_empty() {
        assert!(
            IamConfig::default().is_empty(),
            "IamConfig::default() must report empty in Wave 1"
        );
    }

    #[test]
    fn deploy_config_without_iam_block_parses() {
        // Round-trip: serialise a default DeployConfig (no [iam]) and re-parse it.
        // The `skip_serializing_if` guard must elide the [iam] table from the
        // emitted TOML so that subsequent parses do not require the section.
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave1"),
        );
        let serialised = toml::to_string(&cfg).expect("DeployConfig serialises");
        let parsed: DeployConfig =
            toml::from_str(&serialised).expect("round-trip DeployConfig parses");
        assert!(
            parsed.iam.is_empty(),
            "round-tripped DeployConfig must have empty IamConfig"
        );
    }

    #[test]
    fn deploy_config_serializes_without_iam_when_empty() {
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave1"),
        );
        let out = toml::to_string(&cfg).expect("DeployConfig serialises");
        assert!(
            !out.contains("[iam]"),
            "empty IamConfig must not emit [iam] table (D-05 backward-compat) — got:\n{out}"
        );
    }
}

#[cfg(test)]
mod iam_wave2_tests {
    //! Wave 2 (phase 76 Plan 02) — full `IamConfig` schema coverage.
    //!
    //! These tests drive the replacement of the Wave 1 zero-sized stub with the
    //! three-vector schema described in CONTEXT.md (§Scope) and
    //! `CLI_IAM_CHANGE_REQUEST.md`. They co-exist with `iam_wave1_tests` — the
    //! Wave 1 invariants (default-empty, skip-serialize-empty, round-trip) are
    //! still enforced there.
    //!
    //! Integration-level coverage (serde roundtrip through
    //! `cargo_pmcp::deployment::config::*`) lives in
    //! `cargo-pmcp/tests/iam_config.rs`. Because `cargo_pmcp`'s library surface
    //! does not re-export `deployment::config` (same lib-boundary constraint
    //! documented in Wave 1's summary, Rule 3 deviation #1), the struct-level
    //! assertions live in-crate here where `super::*` makes the private types
    //! directly accessible.

    use super::*;

    /// Builds a cost-coach-shaped TOML fixture by serialising a valid
    /// `default_for_server` baseline and appending the three `[[iam.*]]`
    /// blocks from 76-CONTEXT.md (§Scope). This sidesteps the fragility of
    /// hand-crafting every required field (several structs — `ServerConfig`,
    /// `ObservabilityConfig`, `TargetConfig` — have no `#[serde(default)]`)
    /// and keeps the fixture locked to whatever non-IAM defaults the crate
    /// currently ships.
    fn cost_coach_deploy_toml() -> String {
        let baseline = DeployConfig::default_for_server(
            "cost-coach".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-wave2-fixture"),
        );
        let mut out = toml::to_string(&baseline).expect("baseline serialises");
        out.push_str(
            r#"
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
"#,
        );
        out
    }

    #[test]
    fn iam_config_default_has_three_empty_vectors() {
        let iam = IamConfig::default();
        assert!(iam.tables.is_empty());
        assert!(iam.buckets.is_empty());
        assert!(iam.statements.is_empty());
        assert!(iam.is_empty());
    }

    #[test]
    fn iam_config_is_empty_flips_when_any_vector_is_populated() {
        // Populating any one vector MUST flip is_empty to false — this is the
        // exact condition that toggles the D-05 `skip_serializing_if` guard.
        let mut iam = IamConfig::default();
        iam.tables.push(TablePermission {
            name: "t1".into(),
            actions: vec!["read".into()],
            include_indexes: false,
        });
        assert!(
            !iam.is_empty(),
            "populated tables vector must flip is_empty"
        );

        let mut iam = IamConfig::default();
        iam.buckets.push(BucketPermission {
            name: "b1".into(),
            actions: vec!["read".into()],
        });
        assert!(
            !iam.is_empty(),
            "populated buckets vector must flip is_empty"
        );

        let mut iam = IamConfig::default();
        iam.statements.push(IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["arn:aws:s3:::b/*".into()],
        });
        assert!(
            !iam.is_empty(),
            "populated statements vector must flip is_empty"
        );
    }

    #[test]
    fn cost_coach_shaped_toml_parses_into_populated_iam_config() {
        let fixture = cost_coach_deploy_toml();
        let cfg: DeployConfig = toml::from_str(&fixture).expect("cost-coach TOML parses");

        assert_eq!(cfg.iam.tables.len(), 1);
        assert_eq!(cfg.iam.tables[0].name, "cost-coach-tenants");
        assert_eq!(cfg.iam.tables[0].actions, vec!["readwrite".to_string()]);
        assert!(cfg.iam.tables[0].include_indexes);

        assert_eq!(cfg.iam.buckets.len(), 1);
        assert_eq!(cfg.iam.buckets[0].name, "cost-coach-snapshots");
        assert_eq!(cfg.iam.buckets[0].actions, vec!["readwrite".to_string()]);

        assert_eq!(cfg.iam.statements.len(), 1);
        assert_eq!(cfg.iam.statements[0].effect, "Allow");
        assert_eq!(
            cfg.iam.statements[0].actions,
            vec!["secretsmanager:GetSecretValue".to_string()]
        );
        assert_eq!(
            cfg.iam.statements[0].resources,
            vec!["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*".to_string()]
        );

        assert!(!cfg.iam.is_empty());
    }

    #[test]
    fn include_indexes_defaults_false_when_omitted() {
        let baseline = DeployConfig::default_for_server(
            "demo".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-wave2-include-indexes"),
        );
        let mut toml_str = toml::to_string(&baseline).expect("baseline serialises");
        // Append a table entry WITHOUT `include_indexes` — default must be false.
        toml_str.push_str(
            r#"
[[iam.tables]]
name = "t1"
actions = ["read"]
"#,
        );
        let cfg: DeployConfig = toml::from_str(&toml_str).expect("parses");
        assert_eq!(cfg.iam.tables.len(), 1);
        assert!(
            !cfg.iam.tables[0].include_indexes,
            "include_indexes must default to false when TOML omits it"
        );
    }

    #[test]
    fn populated_iam_roundtrips_losslessly_through_toml() {
        // Round-trip: parse → serialise → re-parse → structural equality
        // (IamConfig intentionally derives no PartialEq per PATTERNS.md §S1, so
        // compare each field individually).
        let fixture = cost_coach_deploy_toml();
        let orig: DeployConfig = toml::from_str(&fixture).expect("cost-coach TOML parses");
        let serialised = toml::to_string(&orig).expect("DeployConfig serialises");
        let reparsed: DeployConfig = toml::from_str(&serialised)
            .unwrap_or_else(|e| panic!("reparse failed — serialised:\n{serialised}\nerror: {e}"));

        assert_eq!(orig.iam.tables.len(), reparsed.iam.tables.len());
        assert_eq!(orig.iam.tables[0].name, reparsed.iam.tables[0].name);
        assert_eq!(orig.iam.tables[0].actions, reparsed.iam.tables[0].actions);
        assert_eq!(
            orig.iam.tables[0].include_indexes,
            reparsed.iam.tables[0].include_indexes
        );

        assert_eq!(orig.iam.buckets.len(), reparsed.iam.buckets.len());
        assert_eq!(orig.iam.buckets[0].name, reparsed.iam.buckets[0].name);
        assert_eq!(orig.iam.buckets[0].actions, reparsed.iam.buckets[0].actions);

        assert_eq!(orig.iam.statements.len(), reparsed.iam.statements.len());
        assert_eq!(
            orig.iam.statements[0].effect,
            reparsed.iam.statements[0].effect
        );
        assert_eq!(
            orig.iam.statements[0].actions,
            reparsed.iam.statements[0].actions
        );
        assert_eq!(
            orig.iam.statements[0].resources,
            reparsed.iam.statements[0].resources
        );
    }

    #[test]
    fn d05_empty_iam_still_elides_every_iam_header() {
        // D-05 backward-compat guard, refined: ensure that the post-Wave-2
        // struct still elides ALL iam-related table headers when every vector
        // is empty (not just the bare `[iam]` header that Wave 1 tested).
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave2"),
        );
        let out = toml::to_string(&cfg).expect("DeployConfig serialises");
        for header in [
            "[iam]",
            "[[iam.tables]]",
            "[[iam.buckets]]",
            "[[iam.statements]]",
        ] {
            assert!(
                !out.contains(header),
                "empty IamConfig must not emit {header} header (D-05) — got:\n{out}"
            );
        }
    }

    #[test]
    fn sub_struct_types_are_constructable() {
        // Sanity: the sub-structs are `pub` and their fields are `pub` — Wave 3
        // (render_iam_block) and Wave 4 (validator) will need to build these by
        // hand for test fixtures.
        let _tp = TablePermission {
            name: "t".into(),
            actions: vec!["read".into()],
            include_indexes: false,
        };
        let _bp = BucketPermission {
            name: "b".into(),
            actions: vec!["write".into()],
        };
        let _st = IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["arn:aws:s3:::bucket/*".into()],
        };
        let _iam = IamConfig::default();
    }
}

#[cfg(test)]
mod gcp_layout_runtime_tests {
    //! Ported from the 0.6.x reference (`config.rs` test module) for the
    //! `google-cloud-run` `[gcp]` + `[layout]` + `[runtime]` schema port.
    //! These are the parse/roundtrip contract: a GCR deploy.toml carrying
    //! `[gcp]`/`[layout]`/`[runtime]` and no `[aws]`/`memory_mb`/`timeout_seconds`
    //! must load, while every existing aws-lambda shape must still load.
    use super::*;

    fn aws_lambda_toml() -> &'static str {
        r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "auth-echo-aws-lambda"
memory_mb = 256
timeout_seconds = 30

[environment]
RUST_LOG = "auth_echo_aws_lambda=info,pmcp=warn"

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true
"#
    }

    fn cloud_run_toml() -> &'static str {
        r#"
[target]
type = "google-cloud-run"
version = "1.0.0"

[gcp]
project_id = "my-gcp-project"
region = "us-central1"

[server]
name = "auth-echo-cloud-run"
memory = "256Mi"
cpu = "1"
ingress = "all"
allow_unauthenticated = true

[environment]
EXPECTED_AUDIENCE = "abc.apps.googleusercontent.com"
RUST_LOG = "info"

[auth]
enabled = false

[observability]
log_retention_days = 30
enable_xray = false
create_dashboard = false

[layout]
kind = "multi-crate-isolated"
primary = "gcp-cloud-run"
path_deps = ["auth-echo-core"]

[runtime]
base = "gcr.io/distroless/cc-debian12"
"#
    }

    #[test]
    fn aws_lambda_deploy_toml_still_loads() {
        let config: DeployConfig = toml::from_str(aws_lambda_toml())
            .expect("AWS Lambda deploy.toml shape must remain loadable");
        assert_eq!(config.target.target_type, "aws-lambda");
        let aws = config.aws.as_ref().expect("aws block present");
        assert_eq!(aws.region, "us-east-1");
        assert!(config.gcp.is_none(), "gcp absent in AWS Lambda config");
        assert_eq!(config.server.memory_mb, 256);
        assert_eq!(config.server.timeout_seconds, 30);
        assert!(config.layout.is_none());
        assert!(config.runtime.is_none());
    }

    #[test]
    fn cloud_run_deploy_toml_loads() {
        let config: DeployConfig =
            toml::from_str(cloud_run_toml()).expect("Cloud Run deploy.toml shape must load");
        assert_eq!(config.target.target_type, "google-cloud-run");
        assert!(config.aws.is_none(), "aws absent in Cloud Run config");
        let gcp = config.gcp.as_ref().expect("gcp block present");
        assert_eq!(gcp.project_id, "my-gcp-project");
        assert_eq!(gcp.region, "us-central1");
        assert_eq!(config.server.memory.as_deref(), Some("256Mi"));
        assert_eq!(config.server.cpu.as_deref(), Some("1"));
        assert_eq!(config.server.allow_unauthenticated, Some(true));
        // memory_mb / timeout_seconds default cleanly when omitted
        assert_eq!(config.server.memory_mb, 512);
        assert_eq!(config.server.timeout_seconds, 30);
        let layout = config.layout.as_ref().expect("layout block present");
        assert_eq!(layout.kind, "multi-crate-isolated");
        assert_eq!(layout.primary, "gcp-cloud-run");
        assert_eq!(layout.path_deps, vec!["auth-echo-core".to_string()]);
        assert!(layout.is_multi_crate_isolated());
        let runtime = config.runtime.as_ref().expect("runtime block present");
        assert_eq!(
            runtime.base.as_deref(),
            Some("gcr.io/distroless/cc-debian12")
        );
        assert!(runtime.apt_packages.is_empty());
    }

    #[test]
    fn default_for_cloud_run_server_roundtrips() {
        let config = DeployConfig::default_for_cloud_run_server(
            "my-server".to_string(),
            "my-project".to_string(),
            "us-central1".to_string(),
            PathBuf::from("/tmp"),
        );
        let serialized = toml::to_string_pretty(&config).expect("serialize");
        let reloaded: DeployConfig = toml::from_str(&serialized).expect("reload");
        assert_eq!(reloaded.target.target_type, "google-cloud-run");
        assert!(reloaded.aws.is_none());
        let gcp = reloaded.gcp.as_ref().expect("gcp present after roundtrip");
        assert_eq!(gcp.project_id, "my-project");
        assert_eq!(reloaded.server.memory.as_deref(), Some("256Mi"));
        assert_eq!(reloaded.server.allow_unauthenticated, Some(true));
        // Defaults: layout and runtime are None — operator opts in.
        assert!(reloaded.layout.is_none());
        assert!(reloaded.runtime.is_none());
    }

    #[test]
    fn default_for_server_roundtrips_as_aws_lambda() {
        let config = DeployConfig::default_for_server(
            "lambda-server".to_string(),
            "us-east-1".to_string(),
            PathBuf::from("/tmp"),
        );
        let serialized = toml::to_string_pretty(&config).expect("serialize");
        let reloaded: DeployConfig = toml::from_str(&serialized).expect("reload");
        assert_eq!(reloaded.target.target_type, "aws-lambda");
        let aws = reloaded.aws.as_ref().expect("aws present after roundtrip");
        assert_eq!(aws.region, "us-east-1");
        assert!(reloaded.gcp.is_none());
    }

    /// Consolidated discriminator guard for ALL deploy targets in one place.
    ///
    /// Every target must serialize the sub-block that matches its
    /// `target.type` — and ONLY that block — then load back with the
    /// discriminator intact and never carrying both `[aws]` and `[gcp]`.
    /// This is the regression guard for the bug class where `deploy init`
    /// wrote an AWS-shaped `deploy.toml` for a Google Cloud Run target
    /// (fixed in 260527-ttn), plus the symmetric risk for `pmcp-run`: it is
    /// Lambda-shaped (see `deployment::naming`, which treats
    /// `"aws-lambda" | "pmcp-run"` identically) and must therefore round-trip
    /// with an `[aws]` block and NO `[gcp]` block. Round-trips through the
    /// same `toml::{to_string_pretty, from_str}` calls that
    /// `DeployConfig::{save, load}` use.
    #[test]
    fn every_target_roundtrips_with_its_own_subblock_only() {
        let aws_lambda = DeployConfig::default_for_server(
            "lambda-srv".to_string(),
            "us-east-1".to_string(),
            PathBuf::from("/tmp"),
        );

        // pmcp-run is Lambda-shaped: `deploy init` builds the AWS default and
        // overrides only the discriminator (see commands/deploy/init.rs).
        let mut pmcp_run = DeployConfig::default_for_server(
            "pmcprun-srv".to_string(),
            "us-east-1".to_string(),
            PathBuf::from("/tmp"),
        );
        pmcp_run.target.target_type = "pmcp-run".to_string();

        let cloud_run = DeployConfig::default_for_cloud_run_server(
            "gcr-srv".to_string(),
            "my-project".to_string(),
            "us-central1".to_string(),
            PathBuf::from("/tmp"),
        );

        // (config, expected target.type, expects [aws] block, expects [gcp] block)
        let cases = [
            (aws_lambda, "aws-lambda", true, false),
            (pmcp_run, "pmcp-run", true, false),
            (cloud_run, "google-cloud-run", false, true),
        ];

        for (config, want_type, expects_aws, expects_gcp) in cases {
            let toml_str = toml::to_string_pretty(&config).expect("serialize");

            // The discriminator must be written verbatim into [target].
            assert!(
                toml_str.contains(&format!("type = \"{want_type}\"")),
                "[target].type `{want_type}` missing from serialized deploy.toml:\n{toml_str}"
            );

            let reloaded: DeployConfig = toml::from_str(&toml_str).expect("reload");

            assert_eq!(
                reloaded.target.target_type, want_type,
                "discriminator must survive the round-trip"
            );
            assert_eq!(
                reloaded.aws.is_some(),
                expects_aws,
                "target `{want_type}`: [aws] block presence mismatch after round-trip"
            );
            assert_eq!(
                reloaded.gcp.is_some(),
                expects_gcp,
                "target `{want_type}`: [gcp] block presence mismatch after round-trip"
            );
            // Mutual exclusivity: the original bug wrote BOTH shapes. A target
            // must never carry both sub-blocks simultaneously.
            assert!(
                !(reloaded.aws.is_some() && reloaded.gcp.is_some()),
                "target `{want_type}` must not carry both [aws] and [gcp] blocks"
            );
        }
    }

    #[test]
    #[should_panic(expected = "deploy.toml is missing the required [aws] block")]
    fn aws_helper_panics_when_aws_absent() {
        let config = DeployConfig::default_for_cloud_run_server(
            "s".to_string(),
            "p".to_string(),
            "r".to_string(),
            PathBuf::from("/tmp"),
        );
        let _ = config.aws();
    }

    /// Minimum-viable Cloud Run deploy.toml per upstream issue #260 must load
    /// without `[auth]` and `[observability]` blocks. Both fields now carry
    /// `#[serde(default)]` so the documented "schema is `[target] + [gcp] +
    /// [server] + [environment]`" promise actually holds.
    #[test]
    fn minimum_schema_cloud_run_loads_without_auth_or_observability() {
        let toml_src = r#"
[target]
type = "google-cloud-run"
version = "1.0.0"

[gcp]
project_id = "my-gcp-project"
region = "us-central1"

[server]
name = "auth-echo-cloud-run"
memory = "256Mi"

[environment]
RUST_LOG = "info"
"#;
        let config: DeployConfig = toml::from_str(toml_src).expect(
            "minimum-schema Cloud Run deploy.toml must load without [auth]/[observability]",
        );
        assert_eq!(config.target.target_type, "google-cloud-run");
        let gcp = config.gcp.as_ref().expect("gcp present");
        assert_eq!(gcp.project_id, "my-gcp-project");
        // Defaults applied when [auth] is omitted.
        assert!(!config.auth.enabled, "auth.enabled defaults to false");
        assert_eq!(config.auth.provider, "none");
        assert!(config.auth.callback_urls.is_empty());
        // Defaults applied when [observability] is omitted.
        assert_eq!(config.observability.log_retention_days, 0);
        assert!(!config.observability.enable_xray);
        assert!(!config.observability.create_dashboard);
        assert!(config.observability.alarms.is_none());
    }

    /// Partial `[observability]` blocks must also load — per-field
    /// `#[serde(default)]` lets operators specify only the knobs they care
    /// about (e.g., AWS Lambda users who want xray on but accept default
    /// retention).
    #[test]
    fn partial_observability_block_loads() {
        let toml_src = r#"
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "s"

[environment]

[observability]
enable_xray = true
"#;
        let config: DeployConfig =
            toml::from_str(toml_src).expect("partial [observability] block must load");
        assert!(config.observability.enable_xray, "explicitly set");
        assert_eq!(config.observability.log_retention_days, 0, "field default");
        assert!(!config.observability.create_dashboard, "field default");
        assert!(config.observability.alarms.is_none(), "field default");
    }

    #[test]
    fn runtime_apt_packages_defaults_empty() {
        let toml_src = r#"
[target]
type = "google-cloud-run"
version = "1.0.0"

[gcp]
project_id = "p"
region = "r"

[server]
name = "s"

[environment]

[auth]
enabled = false

[observability]
log_retention_days = 0
enable_xray = false
create_dashboard = false

[runtime]
base = "debian:bookworm-slim"
"#;
        let config: DeployConfig = toml::from_str(toml_src).expect("load");
        let runtime = config.runtime.as_ref().expect("runtime present");
        assert_eq!(runtime.base.as_deref(), Some("debian:bookworm-slim"));
        assert!(runtime.apt_packages.is_empty(), "default is empty");
    }
}

#[cfg(test)]
mod azure_wave1_tests {
    //! Phase 80 Plan 01 — typed `[azure]` section (REQ-80-04).
    //!
    //! Struct-level assertions live in-crate (`super::*` exposes the private
    //! `AzureConfig`/`DeployConfig` types) mirroring the Phase 76 IamConfig
    //! lib-boundary note. The only external artifact is the committed golden
    //! `tests/golden/deploy-toml-no-azure.golden.toml`, regenerated by the
    //! `#[ignore]`d `emit_golden` helper below.

    use super::*;

    /// Committed pre-azure baseline (byte-identity / D-05 proof, Test A).
    const GOLDEN: &str = include_str!("../../tests/golden/deploy-toml-no-azure.golden.toml");

    /// Deterministic baseline used for the committed-golden byte-identity proof.
    /// The project_root path is fixed so the golden is stable across machines.
    fn azure_golden_baseline() -> DeployConfig {
        DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase80-azure-golden"),
        )
    }

    /// Manual: regenerates the committed golden when the non-azure baseline shape
    /// legitimately changes. Run with `--ignored`. Not part of the normal suite.
    #[test]
    #[ignore = "manual: regenerates the committed golden TOML"]
    fn emit_golden() {
        let out = toml::to_string(&azure_golden_baseline()).expect("baseline serialises");
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/golden/deploy-toml-no-azure.golden.toml"
            ),
            out,
        )
        .expect("write golden");
    }

    /// Test A — byte-identity: the serialised empty-default baseline (azure
    /// elided) is BYTE-IDENTICAL to the committed golden, proving D-05.
    #[test]
    fn empty_azure_serialises_byte_identical_to_golden() {
        let out = toml::to_string(&azure_golden_baseline()).expect("baseline serialises");
        assert_eq!(
            out, GOLDEN,
            "empty-default DeployConfig must serialise byte-identically to the committed golden (D-05)"
        );
        // Belt-and-suspenders: the committed baseline really is the pre-azure shape.
        assert!(
            !GOLDEN.contains("[azure]"),
            "committed golden must carry no [azure] header"
        );
    }

    /// Test B — is_empty flips false for each of the five fields, one per field.
    #[test]
    fn is_empty_flips_when_any_field_differs_from_default() {
        assert!(
            AzureConfig::default().is_empty(),
            "AzureConfig::default() must report empty"
        );

        let mut c = AzureConfig::default();
        c.resource_group = Some("custom-rg".to_string());
        assert!(!c.is_empty(), "resource_group=Some must flip is_empty");

        let mut c = AzureConfig::default();
        c.environment = Some("custom-env".to_string());
        assert!(!c.is_empty(), "environment=Some must flip is_empty");

        let mut c = AzureConfig::default();
        c.location = "westus2".to_string();
        assert!(!c.is_empty(), "non-default location must flip is_empty");

        let mut c = AzureConfig::default();
        c.target_port = 9090;
        assert!(!c.is_empty(), "non-default target_port must flip is_empty");

        let mut c = AzureConfig::default();
        c.min_replicas = 2;
        assert!(!c.is_empty(), "non-default min_replicas must flip is_empty");
    }

    /// Test C — a non-empty `[azure]` roundtrips (serialise → parse) preserving
    /// resource_group/location/target_port; is_empty stays false.
    #[test]
    fn non_empty_azure_roundtrips_through_toml() {
        let mut cfg = azure_golden_baseline();
        cfg.azure.location = "westus2".to_string();
        cfg.azure.resource_group = Some("custom-rg".to_string());
        cfg.azure.target_port = 9090;

        let serialised = toml::to_string(&cfg).expect("config with [azure] serialises");
        assert!(
            serialised.contains("[azure]"),
            "non-default azure must emit an [azure] table"
        );

        let parsed: DeployConfig =
            toml::from_str(&serialised).expect("round-trip DeployConfig parses");
        assert_eq!(parsed.azure.location, "westus2");
        assert_eq!(parsed.azure.resource_group.as_deref(), Some("custom-rg"));
        assert_eq!(parsed.azure.target_port, 9090);
        assert!(
            !parsed.azure.is_empty(),
            "round-tripped non-default azure must not be empty"
        );
    }

    /// Test D — an `[azure]` fragment omitting target_port/min_replicas parses
    /// them to their built-in defaults (8080 / 1).
    #[test]
    fn azure_fragment_applies_field_defaults() {
        let mut toml_str = toml::to_string(&azure_golden_baseline()).expect("baseline serialises");
        toml_str.push_str("\n[azure]\nlocation = \"westus2\"\n");

        let parsed: DeployConfig =
            toml::from_str(&toml_str).expect("config with partial [azure] parses");
        assert_eq!(parsed.azure.location, "westus2");
        assert_eq!(
            parsed.azure.target_port, 8080,
            "omitted target_port must default to 8080"
        );
        assert_eq!(
            parsed.azure.min_replicas, 1,
            "omitted min_replicas must default to 1"
        );
        assert!(
            parsed.azure.resource_group.is_none(),
            "omitted resource_group must stay None"
        );
    }

    // ── Property: arbitrary-input [azure] TOML roundtrip (Phase 80 Plan 04) ──────
    //
    // The fixed-case roundtrip above (Test C) proves a handful of hand-picked
    // values survive serialise → parse. This property generalises it to
    // arbitrary valid AzureConfig values, proving all five fields are preserved.
    // It lives in-crate because AzureConfig + DeployConfig are reached via
    // `super::*` (mirrors the Phase 76 struct-level-tests-in-crate decision).
    use proptest::prelude::*;

    /// Strategy for an optional name string. Restricted to a printable,
    /// TOML-serialisable charset (basic-string escaping covers it) while still
    /// exercising unicode, spaces, quotes and backslashes. `None` is included so
    /// the `resource_group`/`environment` fallbacks are covered too.
    fn arb_opt_name() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            Just(None),
            "[\\PC]{0,40}".prop_map(Some), // any printable char, 0..=40 long
        ]
    }

    fn arb_azure_config() -> impl Strategy<Value = AzureConfig> {
        (
            arb_opt_name(),
            arb_opt_name(),
            "[\\PC]{1,40}",
            any::<u16>(),
            any::<u32>(),
        )
            .prop_map(
                |(resource_group, environment, location, target_port, min_replicas)| AzureConfig {
                    resource_group,
                    environment,
                    location,
                    target_port,
                    min_replicas,
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// For arbitrary valid AzureConfig values, `toml::to_string` of a
        /// DeployConfig carrying that `[azure]` section then `toml::from_str`
        /// back roundtrips and preserves all five fields.
        #[test]
        fn azure_config_toml_roundtrips_all_fields(azure in arb_azure_config()) {
            let mut cfg = azure_golden_baseline();
            cfg.azure = azure.clone();

            let serialised = toml::to_string(&cfg).expect("DeployConfig with [azure] serialises");
            let parsed: DeployConfig =
                toml::from_str(&serialised).expect("round-trip DeployConfig parses");

            prop_assert_eq!(parsed.azure.resource_group, azure.resource_group);
            prop_assert_eq!(parsed.azure.environment, azure.environment);
            prop_assert_eq!(parsed.azure.location, azure.location);
            prop_assert_eq!(parsed.azure.target_port, azure.target_port);
            prop_assert_eq!(parsed.azure.min_replicas, azure.min_replicas);
        }
    }
}

#[cfg(test)]
mod stack_ts_guard_tests {
    //! Phase 98 Plan 02 — unit coverage for the shared exists-guard helper
    //! [`write_stack_ts_guarded`] (DSTK-01). Both deploy targets call this
    //! helper, so its three cases (absent→write, exists+no-flag→preserve,
    //! exists+flag→overwrite) are proven once here.

    use super::*;

    #[test]
    fn writes_when_stack_ts_absent_regardless_of_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib_dir = tmp.path().join("deploy").join("lib");

        // No file yet: helper writes the first-deploy scaffold even with the
        // regenerate flag OFF.
        let wrote = write_stack_ts_guarded(&lib_dir, "scaffold contents", false)
            .expect("write should succeed");
        assert!(wrote, "absent stack.ts must be written (returns Ok(true))");
        let on_disk =
            std::fs::read_to_string(lib_dir.join("stack.ts")).expect("stack.ts written to disk");
        assert_eq!(on_disk, "scaffold contents");
    }

    #[test]
    fn preserves_existing_stack_ts_without_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib_dir = tmp.path().join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create lib dir");
        let curated = "// curated by operator — do not clobber\n";
        std::fs::write(lib_dir.join("stack.ts"), curated).expect("seed curated stack.ts");

        let wrote = write_stack_ts_guarded(&lib_dir, "REGENERATED TEMPLATE", false)
            .expect("guard should succeed");
        assert!(
            !wrote,
            "existing stack.ts must be preserved (returns Ok(false))"
        );
        let on_disk =
            std::fs::read_to_string(lib_dir.join("stack.ts")).expect("read stack.ts back");
        assert_eq!(
            on_disk, curated,
            "curated stack.ts must be byte-identical when regenerate=false"
        );
    }

    #[test]
    fn preserved_notice_names_the_regenerate_flag() {
        // The notice is consumed by the bin-only write sites; assert its text
        // here so the lib build exercises the constant and operators get an
        // actionable hint (mentions the opt-in flag).
        assert!(STACK_TS_PRESERVED_NOTICE.contains("--regenerate-stack"));
        assert!(STACK_TS_PRESERVED_NOTICE.contains("preserved existing deploy/lib/stack.ts"));
    }

    #[test]
    fn overwrites_existing_stack_ts_with_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib_dir = tmp.path().join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create lib dir");
        std::fs::write(lib_dir.join("stack.ts"), "// curated\n").expect("seed curated stack.ts");

        let wrote = write_stack_ts_guarded(&lib_dir, "REGENERATED TEMPLATE", true)
            .expect("guard should succeed");
        assert!(wrote, "regenerate=true must overwrite (returns Ok(true))");
        let on_disk =
            std::fs::read_to_string(lib_dir.join("stack.ts")).expect("read stack.ts back");
        assert_eq!(on_disk, "REGENERATED TEMPLATE");
    }

    // ── FIX #1: preserved-stack inert-section warning ───────────────────────
    // (deploy-toml-inert-for-preserved-stack)

    #[test]
    fn inert_warning_silent_when_both_sections_empty() {
        assert!(
            stack_ts_preserved_inert_warning(true, true).is_none(),
            "no warning when both [iam] and [environment] are empty"
        );
    }

    #[test]
    fn inert_warning_fires_for_non_empty_iam() {
        let w = stack_ts_preserved_inert_warning(false, true)
            .expect("warning must fire when [iam] is non-empty on a preserved stack");
        assert!(w.contains("PRESERVED"), "warning names the preserve cause");
        assert!(
            w.contains("--regenerate-stack"),
            "IAM remedy names the opt-in flag"
        );
        assert!(w.contains("[iam]"), "warning calls out the [iam] section");
    }

    #[test]
    fn inert_warning_fires_for_non_empty_environment() {
        let w = stack_ts_preserved_inert_warning(true, false)
            .expect("warning must fire when [environment] is non-empty on a preserved stack");
        assert!(
            w.contains("[environment]"),
            "warning calls out the [environment] section"
        );
        assert!(
            w.contains("process.env"),
            "warning explains the process.env delivery caveat"
        );
    }

    #[test]
    fn inert_warning_fires_for_both_sections() {
        let w = stack_ts_preserved_inert_warning(false, false)
            .expect("warning must fire when both sections are non-empty");
        assert!(w.contains("[iam]"));
        assert!(w.contains("[environment]"));
    }

    // ── FIX #2: [environment] threaded into the CDK env-injection path ───────

    #[test]
    fn deploy_env_vars_includes_environment_entries() {
        let mut cfg = DeployConfig::default_for_server(
            "demo".to_string(),
            "us-east-1".to_string(),
            std::path::PathBuf::from("/tmp/deploy-env-vars"),
        );
        cfg.environment
            .insert("MY_FLAG".to_string(), "on".to_string());

        let env = cfg.deploy_env_vars();
        assert_eq!(
            env.get("MY_FLAG").map(String::as_str),
            Some("on"),
            "[environment] entries must reach the deploy env-var map (FIX #2)"
        );
        // Built-in RUST_LOG from default_for_server survives the merge.
        assert_eq!(env.get("RUST_LOG").map(String::as_str), Some("info"));
    }

    #[test]
    fn deploy_env_vars_secret_wins_over_environment_on_collision() {
        let mut cfg = DeployConfig::default_for_server(
            "demo".to_string(),
            "us-east-1".to_string(),
            std::path::PathBuf::from("/tmp/deploy-env-vars-precedence"),
        );
        cfg.environment
            .insert("SHARED".to_string(), "env-default".to_string());
        cfg.secrets
            .insert("SHARED".to_string(), "resolved-secret".to_string());

        let env = cfg.deploy_env_vars();
        assert_eq!(
            env.get("SHARED").map(String::as_str),
            Some("resolved-secret"),
            "resolved [secrets] must win over [environment] on key collision"
        );
    }
}
