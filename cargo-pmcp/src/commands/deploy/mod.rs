use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};

use crate::commands::configure::workspace::find_deploy_root;
use crate::commands::flags::FormatValue;

/// Detect server name from Cargo.toml in the project root or core workspace
fn detect_server_name(project_root: &Path) -> Result<String> {
    if let Some(name) = try_detect_name_from_cargo(project_root)? {
        return Ok(name);
    }

    // Fallback to directory name
    Ok(project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mcp-server")
        .to_string())
}

/// Parse the root Cargo.toml and return a server name either from a core-workspace
/// package (workspace mode) or the root package directly.
fn try_detect_name_from_cargo(project_root: &Path) -> Result<Option<String>> {
    let cargo_toml_path = project_root.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Ok(None);
    }

    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)?;
    let cargo_toml = match toml::from_str::<toml::Value>(&cargo_toml_content) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if cargo_toml.get("workspace").is_some() {
        if let Some(name) = find_name_in_core_workspace(project_root) {
            return Ok(Some(name));
        }
    }

    Ok(read_root_package_name(&cargo_toml)
        .map(|package| crate::deployment::server_name::default_server_name(&package)))
}

/// The `[server] name` a `deploy init` that does not go through
/// `InitCommand` directly (pmcp-run, cloudflare-workers) works with (debug
/// session `cargo-pmcp-deploy-targets`, finding #3).
///
/// An existing `.pmcp/deploy.toml` keeps its name (re-init never resets it);
/// `--name` renames it there, as a verified one-line edit. Without a
/// deploy.toml: `--name`, else the package default.
fn init_server_name(project_root: &Path, explicit: Option<&str>) -> Result<String> {
    if project_root.join(".pmcp/deploy.toml").exists() {
        let kept = crate::deployment::DeployConfig::load(project_root)?;
        return match explicit.filter(|name| *name != kept.server.name) {
            Some(name) => Ok(
                rename_server_in_deploy_toml(project_root, &kept.server.name, name)?
                    .server
                    .name,
            ),
            None => Ok(kept.server.name),
        };
    }
    explicit.map_or_else(
        || detect_server_name(project_root),
        |name| Ok(name.to_string()),
    )
}

/// The `[gcp] region` a new Cloud Run deploy.toml gets without `--region`.
const CLOUD_RUN_DEFAULT_REGION: &str = "us-central1";

/// `deploy init`'s region for `target_id` (debug session
/// `cargo-pmcp-deploy-targets`, finding #10).
///
/// - `google-cloud-run`: `--region`, else `us-central1`. `AWS_REGION` is never
///   consulted: an AWS region is not a GCP region. (clap used to fill
///   `--region` from `AWS_REGION` or `us-east-1` for every target, so the
///   Cloud Run default was unreachable and `[gcp] region = "us-east-1"` was
///   scaffolded.)
/// - every other target: unchanged — `--region`, else `$AWS_REGION` (even
///   when empty, as clap's `env` did), else `us-east-1`.
fn default_init_region(target_id: &str, explicit: Option<&str>) -> String {
    if target_id == "google-cloud-run" {
        return explicit
            .filter(|region| !region.is_empty())
            .unwrap_or(CLOUD_RUN_DEFAULT_REGION)
            .to_string();
    }
    explicit.map_or_else(
        || std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        str::to_string,
    )
}

/// The Cloud Run arm of the `deploy init` dispatch: the config
/// `init_google_cloud_run` scaffolds from, for the parsed `--region` and
/// `--name` flags.
fn cloud_run_init_from_flags(
    project_root: &Path,
    region: Option<&str>,
    explicit_name: Option<&str>,
) -> Result<crate::deployment::DeployConfig> {
    let region = default_init_region("google-cloud-run", region);
    cloud_run_init_config(project_root, &region, explicit_name)
}

/// The config `deploy init --target-type google-cloud-run` scaffolds from.
///
/// An existing `.pmcp/deploy.toml` is kept (Cloud Run init never rewrites
/// it); `--name` renames its server in place, as a verified one-line edit.
/// A new one is [`default_config_for_target`]'s Cloud Run shape, with the
/// name defaulting to the package name with `-lambda` stripped and `region`
/// (already resolved by [`default_init_region`]) as its `[gcp] region`.
fn cloud_run_init_config(
    project_root: &Path,
    region: &str,
    explicit_name: Option<&str>,
) -> Result<crate::deployment::DeployConfig> {
    let deploy_toml_path = project_root.join(".pmcp/deploy.toml");
    if deploy_toml_path.exists() {
        let kept = crate::deployment::DeployConfig::load(project_root)?;
        return match explicit_name.filter(|name| *name != kept.server.name) {
            Some(name) => rename_server_in_deploy_toml(project_root, &kept.server.name, name),
            None => Ok(kept),
        };
    }
    let server_name = match explicit_name {
        Some(name) => name.to_string(),
        None => detect_server_name(project_root)?,
    };
    Ok(default_config_for_target(
        "google-cloud-run",
        server_name,
        region.to_string(),
        project_root.to_path_buf(),
    ))
}

/// Rename `[server] name` in an existing `.pmcp/deploy.toml`, editing only
/// that line, and return the reloaded config.
fn rename_server_in_deploy_toml(
    project_root: &Path,
    old: &str,
    new: &str,
) -> Result<crate::deployment::DeployConfig> {
    let path = project_root.join(".pmcp/deploy.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let edited =
        crate::deployment::deploy_toml_edit::set_string_in_table(&text, "server", "name", new)
            .with_context(|| {
                format!(
                    "could not set `name` in the [server] table of {} automatically (it is not a \
                     plain `name = \"...\"` line). Set `name = \"{new}\"` under [server] by hand, \
                     then re-run. Nothing was changed.",
                    path.display()
                )
            })?;
    std::fs::write(&path, edited).with_context(|| format!("Failed to write {}", path.display()))?;
    println!("📝 .pmcp/deploy.toml: [server] name \"{old}\" -> \"{new}\"");
    crate::deployment::DeployConfig::load(project_root)
}

/// Scan `<project_root>/core-workspace/*/Cargo.toml` for a package name,
/// stripping "mcp-" prefix and "-core" suffix.
fn find_name_in_core_workspace(project_root: &Path) -> Option<String> {
    let core_workspace_dir = project_root.join("core-workspace");
    if !core_workspace_dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(&core_workspace_dir).ok()?;
    for entry in entries.flatten() {
        let cargo_path = entry.path().join("Cargo.toml");
        if let Some(name) = read_core_package_name(&cargo_path) {
            let clean_name = name
                .strip_prefix("mcp-")
                .unwrap_or(&name)
                .strip_suffix("-core")
                .unwrap_or(&name)
                .to_string();
            return Some(clean_name);
        }
    }
    None
}

/// Read `package.name` from a single core-crate Cargo.toml.
fn read_core_package_name(cargo_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(cargo_path).ok()?;
    let core_cargo = toml::from_str::<toml::Value>(&content).ok()?;
    core_cargo
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
}

/// Return `package.name` from the root Cargo.toml, if present.
fn read_root_package_name(cargo_toml: &toml::Value) -> Option<String> {
    cargo_toml
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
}

/// Detect a config-driven single-crate project (Shape B/D, SHAP-D-01).
///
/// True when the project ROOT carries the three markers a `cargo pmcp new --kind
/// sql-server` scaffold emits (D-09 heuristic):
/// 1. `config.toml` at the root,
/// 2. `schema.sql` at the root, and
/// 3. a root `Cargo.toml` whose `[dependencies]` reference `pmcp-server-toolkit`.
///
/// This is the detection seam used by the deploy `None` arm (M3) to emit an
/// informational note before the existing `[assets]`-driven bundling + the
/// single-crate `find_lambda_package_dir` fallback (H3) run for this layout. It
/// adds NO `TargetEntry` variant and changes NO enum (D-10) — pmcp.run is selected
/// purely by the scaffold's `target_type = "pmcp-run"` (M3).
///
/// Lives as a `pub(crate)` free fn so it is unit-testable in-module (a bin-only
/// helper cannot be reached from an integration test — M3).
pub(crate) fn is_config_driven_project(project_root: &Path) -> bool {
    let has_config = project_root.join("config.toml").exists();
    let has_schema = project_root.join("schema.sql").exists();
    let references_toolkit = std::fs::read_to_string(project_root.join("Cargo.toml"))
        .map(|c| c.contains("pmcp-server-toolkit"))
        .unwrap_or(false);
    has_config && has_schema && references_toolkit
}

/// Build a default `DeployConfig` whose SHAPE matches the requested `target_id`.
///
/// The cargo-pmcp init dispatch (`execute_async` / `DeployAction::Init`) calls this
/// to pick the right constructor — `default_for_cloud_run_server` for
/// `"google-cloud-run"`, `default_for_server` for everything else (pmcp-run,
/// cloudflare-workers, …). Before this branched, every non-aws-lambda target
/// got the AWS-shaped default (required `[aws]`, no `[gcp]`, AWS-flavored
/// `memory_mb`), which save_if_missing then cemented into the operator's
/// `.pmcp/deploy.toml` (n51 follow-up #1).
///
/// The google-cloud-run shape is what `deploy init --target-type
/// google-cloud-run` writes for a new deploy.toml ([`cloud_run_init_config`]
/// builds it here; debug session `cargo-pmcp-deploy-targets`, finding A3 —
/// this arm used to be unreachable and differed from what init wrote).
/// `project_id` is the `your-gcp-project-id` placeholder for the operator to
/// replace via `[gcp].project_id`; `deploy::resolve_params` treats it like an
/// empty value and falls back to `gcloud config get-value project`.
pub(crate) fn default_config_for_target(
    target_id: &str,
    server_name: String,
    region: String,
    project_root: std::path::PathBuf,
) -> crate::deployment::DeployConfig {
    if target_id == "google-cloud-run" {
        crate::deployment::DeployConfig::default_for_cloud_run_server(
            server_name,
            "your-gcp-project-id".to_string(),
            region,
            project_root,
        )
    } else {
        crate::deployment::DeployConfig::default_for_server(server_name, region, project_root)
    }
}

pub mod deploy;
pub mod init;

use init::InitCommand;

/// Phase 77: resolve target and emit D-13 banner. Idempotent (OnceLock-guarded).
///
/// Safe to call from every AWS-touching code path; the OnceLock prevents duplicate output.
/// Errors are logged but not propagated (banner is informational, not load-bearing).
///
/// `project_root` is the workspace root (typically `find_workspace_root()`); `deploy_config`
/// is the loaded `DeployConfig` (when available — pass `None` for early-init paths that don't
/// yet have one). The resolver re-runs precedence resolution; the OnceLock guard makes
/// duplicate emissions across the dispatch tree no-ops.
fn emit_target_banner_if_resolved(
    global_flags: &crate::commands::GlobalFlags,
    project_root: &std::path::Path,
    deploy_config: Option<&crate::deployment::config::DeployConfig>,
) {
    match crate::commands::configure::resolver::resolve_target(
        None,
        None,
        project_root,
        deploy_config,
    ) {
        Ok(Some(resolved)) => {
            let _ = crate::commands::configure::banner::emit_resolved_banner_once(
                &resolved,
                global_flags.quiet,
            );
        },
        Ok(None) => { /* D-11 zero-touch: no banner */ },
        Err(_) => { /* swallow — error already surfaced at dispatch time in main.rs */ },
    }
}

#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target TYPE (aws-lambda, cloudflare-workers, pmcp-run, google-cloud-run).
    /// Selects which deployment backend to use. NOT to be confused with the global `--target`
    /// flag which selects a NAMED target from `~/.pmcp/config.toml` (Phase 77).
    #[arg(long = "target-type", alias = "target", global = true)]
    target_type: Option<String>,

    /// Use shared OAuth pool for SSO (pmcp-run only).
    ///
    /// Enables OAuth authentication using an existing shared Cognito User Pool.
    /// This enables Single Sign-On (SSO) with other MCP servers using the same pool.
    ///
    /// Example: --shared-pool agent-framework
    ///
    /// Equivalent to running:
    ///   cargo pmcp deploy
    ///   cargo pmcp deploy oauth enable --server <server> --shared-pool <name>
    #[arg(long, value_name = "POOL_NAME")]
    shared_pool: Option<String>,

    /// Skip OAuth configuration during deployment.
    ///
    /// Deploy without OAuth first, then add it later with:
    ///   cargo pmcp deploy oauth enable --server <server> [--shared-pool <name>]
    ///
    /// This is useful for testing the server before adding authentication.
    #[arg(long)]
    no_oauth: bool,

    /// Skip the widget pre-build step entirely (Phase 79).
    ///
    /// Use this when your CI pipeline runs `npm run build` (or equivalent)
    /// in a separate step. Cargo's incremental cache may still serve a stale
    /// binary if `include_str!` resolves to widget files — see
    /// `cargo pmcp doctor` for the build.rs scaffold check (Phase 79 v2).
    #[arg(long)]
    no_widget_build: bool,

    /// Build widgets only; skip cargo build, upload, and Lambda hot-swap (Phase 79).
    ///
    /// Useful for the fast inner-loop iteration on widget code without
    /// re-deploying the Rust binary. The PMCP_WIDGET_DIRS env var is still
    /// set so a follow-up `cargo build` picks up the rebuilt widgets.
    #[arg(long)]
    widgets_only: bool,

    /// Skip the post-deploy verification suite entirely (warmup + check +
    /// conformance + apps).
    ///
    /// Use when CI runs verification in a separate step or when you
    /// intentionally want a deploy that doesn't probe the live endpoint.
    /// NOTE: this also skips the warmup grace, since the warmup serves no
    /// purpose without subsequent tests.
    #[arg(long)]
    no_post_deploy_test: bool,

    /// Run only a subset of post-deploy tests (comma-separated).
    ///
    /// Valid values: connectivity, conformance, apps. Default: all three
    /// (when the corresponding subsystem is present).
    /// Example: --post-deploy-tests=conformance,apps
    #[arg(long, value_delimiter = ',', value_name = "CHECKS")]
    post_deploy_tests: Option<Vec<String>>,

    /// Override [post_deploy_tests].on_failure from deploy.toml.
    ///
    /// Values:
    ///   warn      Print failure banner; CLI exits 0; pipeline continues
    ///   fail      Print failure banner with IS-LIVE warning; CLI exits 3
    ///             (REVISION 3 HIGH-2)
    ///   rollback  REJECTED. Auto-rollback support will land in a future
    ///             phase that verifies the existing DeployTarget::rollback()
    ///             trait implementations. Use 'fail' (default) or 'warn'.
    ///
    /// Default (from config or "fail"): the deployed broken Lambda revision
    /// STAYS LIVE on `fail`; CI/CD pipelines that interpret nonzero exit as
    /// auto-rollback will misread this — the deploy DID succeed at the
    /// infrastructure level. Exit code 3 is unique-per-failure-mode for
    /// CI/CD detection (REVISION 3 HIGH-2). To roll back manually, run:
    ///     cargo pmcp deploy rollback --target <target>
    #[arg(long, value_name = "MODE", value_parser = parse_on_test_failure_flag)]
    on_test_failure: Option<crate::deployment::post_deploy_tests::OnFailure>,

    /// Override [post_deploy_tests].apps_mode from deploy.toml.
    ///
    /// Values: standard, chatgpt, claude-desktop. Default (from config or
    /// "claude-desktop"): runs the strict Phase 78 claude-desktop validator
    /// on every widget.
    #[arg(long, value_name = "MODE")]
    apps_mode: Option<String>,

    /// Overwrite an existing `deploy/lib/stack.ts` (Phase 98, DSTK-01).
    ///
    /// By default `cargo pmcp deploy` PRESERVES a pre-existing, operator-curated
    /// `deploy/lib/stack.ts` instead of silently regenerating it from the
    /// template (a first deploy still scaffolds a missing file without this
    /// flag). Pass `--regenerate-stack` (alias `--force`) to re-render and
    /// overwrite the existing file from `.pmcp/deploy.toml`.
    #[arg(long = "regenerate-stack", alias = "force")]
    regenerate_stack: bool,

    /// Override deploy-root resolution. Accepts either a directory containing
    /// `.pmcp/deploy.toml`, OR a path to a `deploy.toml` file directly. Highest
    /// precedence — applies to deploy, init, and all read subcommands.
    #[arg(long = "manifest-path", value_name = "PATH", global = true)]
    manifest_path: Option<PathBuf>,

    #[command(subcommand)]
    action: Option<DeployAction>,
}

/// REVISION 3 HIGH-G2 — clap value parser that delegates to
/// `OnFailure::FromStr` which carries the verbatim `ROLLBACK_REJECT_MESSAGE`
/// for the string `"rollback"`. Wired on the `--on-test-failure` arg above.
fn parse_on_test_failure_flag(
    s: &str,
) -> Result<crate::deployment::post_deploy_tests::OnFailure, String> {
    s.parse()
}

#[derive(Debug, Parser)]
pub enum DeployAction {
    /// Initialize deployment configuration
    Init {
        /// Region. AWS targets: the AWS region (default: the `AWS_REGION`
        /// environment variable, else `us-east-1`). google-cloud-run: the
        /// `[gcp] region` of a new deploy.toml (default: `us-central1`;
        /// `AWS_REGION` is not consulted).
        #[arg(long)]
        region: Option<String>,

        /// Skip credentials check
        #[arg(long)]
        skip_credentials_check: bool,

        /// OAuth provider (cognito, oidc, none)
        #[arg(long, value_name = "PROVIDER")]
        oauth: Option<String>,

        /// Use shared OAuth infrastructure (format: shared:<name>)
        #[arg(long, value_name = "NAME")]
        oauth_shared: Option<String>,

        /// Existing Cognito User Pool ID (skip creation)
        #[arg(long, value_name = "POOL_ID")]
        cognito_user_pool_id: Option<String>,

        /// Cognito User Pool name (when creating new)
        #[arg(long, value_name = "NAME")]
        cognito_pool_name: Option<String>,

        /// Enable social login providers (comma-separated: github,google,apple)
        #[arg(long, value_name = "PROVIDERS", value_delimiter = ',')]
        social_providers: Option<Vec<String>>,

        /// Server name: `[server] name` in .pmcp/deploy.toml. It names the
        /// deployment (on aws-lambda the stack is `<NAME>-stack` and the
        /// function `<NAME>`). On an existing deploy.toml it renames the server
        /// and keeps every other setting. Default for a new deploy.toml: the
        /// Cargo package name with one trailing `-lambda` removed. Without
        /// this flag, an existing deploy.toml's name is kept.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
    },

    /// View deployment logs
    Logs {
        /// Follow logs in real-time
        #[arg(long)]
        tail: bool,

        /// Number of lines to show
        #[arg(long, default_value = "100")]
        lines: usize,
    },

    /// View deployment metrics
    Metrics {
        /// Time period (1h, 24h, 7d, 30d)
        #[arg(long, default_value = "24h")]
        period: String,
    },

    /// Test the deployment
    Test {},

    /// Rollback to previous version
    Rollback {
        /// Version to rollback to (default: previous)
        version: Option<String>,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },

    /// Destroy the deployment
    Destroy {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,

        /// Remove all deployment files (CDK project, Lambda wrapper, config)
        #[arg(long)]
        clean: bool,

        /// Don't wait for async operations to complete (pmcp-run only)
        #[arg(long)]
        no_wait: bool,
    },

    /// Manage secrets
    Secrets {
        #[command(subcommand)]
        action: SecretsAction,
    },

    /// Show deployment outputs
    Outputs {
        /// Output format (text, json)
        #[arg(long, value_enum, default_value = "text")]
        format: FormatValue,
    },

    /// Login to deployment target (pmcp-run, cloudflare, etc.)
    Login,

    /// Logout from deployment target
    Logout,

    /// Manage OAuth configuration for pmcp.run servers
    Oauth {
        #[command(subcommand)]
        action: OAuthAction,
    },

    /// Check status of an async operation (pmcp-run only)
    Status {
        /// Operation ID to check (deployment ID for destroy operations)
        operation_id: String,
    },
}

#[derive(Debug, Parser)]
pub enum SecretsAction {
    /// Set a secret value
    Set {
        /// Secret key
        key: String,

        /// Get value from environment variable
        #[arg(long)]
        from_env: Option<String>,
    },

    /// List all secrets
    List,

    /// Delete a secret
    Delete {
        /// Secret key
        key: String,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
}

/// Default OAuth scopes for MCP servers
const DEFAULT_OAUTH_SCOPES: &[&str] = &["openid", "email", "mcp/read"];

/// Default public client patterns for MCP OAuth
const DEFAULT_PUBLIC_CLIENT_PATTERNS: &[&str] =
    &["claude", "cursor", "desktop", "mcp-inspector", "chatgpt"];

#[derive(Debug, Parser)]
pub enum OAuthAction {
    /// Enable OAuth for an MCP server on pmcp.run
    ///
    /// Configures OAuth authentication using AWS Cognito. When using --shared-pool
    /// or --copy-from, multiple MCP servers can share the same user pool, enabling
    /// Single Sign-On (SSO) across servers.
    Enable {
        /// Server ID (deployment ID) to enable OAuth for
        #[arg(long)]
        server: String,

        /// Copy OAuth configuration from an existing server.
        ///
        /// Fetches OAuth settings (scopes, DCR, public clients, user pool) from
        /// another server and applies them to this server. This is the easiest
        /// way to enable SSO across multiple MCP servers.
        ///
        /// Example: --copy-from advanced-mcp-course
        #[arg(long, value_name = "SERVER_ID")]
        copy_from: Option<String>,

        /// OAuth scopes for this server (comma-separated).
        ///
        /// Defines what permissions clients can request:
        ///   - openid:    Required for OIDC (always include)
        ///   - email:     Access to user's email address
        ///   - mcp/read:  Read-only MCP operations
        ///   - mcp/write: Read-write MCP operations
        ///
        /// Default: "openid,email,mcp/read"
        ///
        /// Note: When using --shared-pool, scopes are server-specific and
        /// do NOT affect other servers sharing the same pool.
        #[arg(long, value_delimiter = ',', value_name = "SCOPES")]
        scopes: Option<Vec<String>>,

        /// Enable Dynamic Client Registration (RFC 7591).
        ///
        /// When enabled, MCP clients (Claude, Cursor, ChatGPT) can automatically
        /// register themselves when users add your server URL.
        ///
        /// Default: true (recommended for MCP servers)
        #[arg(long, default_value = "true")]
        dcr: bool,

        /// Public client patterns (comma-separated).
        ///
        /// Client names matching these patterns are treated as public OAuth
        /// clients (no client_secret required). This is correct for desktop
        /// and native apps that cannot securely store secrets.
        ///
        /// Default: "claude,cursor,desktop,mcp-inspector,chatgpt"
        ///
        /// Example: --public-clients "claude,cursor,my-app"
        #[arg(long, value_delimiter = ',', value_name = "PATTERNS")]
        public_clients: Option<Vec<String>>,

        /// Use an existing Cognito User Pool instead of creating a new one.
        ///
        /// This enables Single Sign-On (SSO) across multiple MCP servers.
        /// Users with accounts on other servers sharing this pool can
        /// automatically access this server.
        ///
        /// Value can be:
        ///   - Cognito User Pool ID (e.g., "us-east-1_TSTigvdHH")
        ///   - Shared pool name from organization setup
        ///
        /// TIP: To find an existing pool ID, run:
        ///   cargo pmcp deploy oauth status --server <existing-server>
        ///
        /// Note: Other parameters (--scopes, --dcr, --public-clients) configure
        /// THIS server's OAuth behavior, not the shared pool itself.
        #[arg(long, value_name = "POOL_ID_OR_NAME")]
        shared_pool: Option<String>,
    },

    /// Disable OAuth for an MCP server
    ///
    /// Disables OAuth authentication. The Cognito User Pool is NOT deleted,
    /// so you can re-enable OAuth at any time.
    Disable {
        /// Server ID (deployment ID)
        #[arg(long)]
        server: String,
    },

    /// Show OAuth status and endpoints for an MCP server
    ///
    /// Displays current OAuth configuration including endpoints, scopes,
    /// and Cognito User Pool details. Use this to find pool IDs for
    /// sharing with other servers.
    Status {
        /// Server ID (deployment ID)
        #[arg(long)]
        server: String,
    },
}

impl DeployCommand {
    pub fn execute(&self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        // Run async code in tokio runtime
        tokio::runtime::Runtime::new()?.block_on(self.execute_async(global_flags))
    }

    /// Phase 79 Wave 2 — Step 2.5 helper.
    ///
    /// Iterates over all `[[widgets]]` entries (or convention-detected
    /// `widget/`/`widgets/`), runs the orchestrator on each, collects every
    /// resolved `absolute_output_dir`, and joins them into a single
    /// `PMCP_WIDGET_DIRS` env var (colon-separated, Unix `PATH` convention)
    /// BEFORE returning.
    ///
    /// Stops on first widget failure (mirrors `79-CONTEXT.md` "Multiple
    /// `[[widgets]]` blocks supported. Stop on first failure"). Skips the
    /// env var when no widgets are detected so 79-04's build.rs
    /// local-discovery fallback (HIGH-G1) can take over for direct
    /// Merge CLI flag overrides into the loaded `[post_deploy_tests]` config.
    /// REVISION 3 HIGH-G2: `OnFailure::Rollback` is impossible here because
    /// clap rejects `--on-test-failure=rollback` at parse time AND the custom
    /// `Deserialize` impl rejects `on_failure="rollback"` at config load time.
    /// Cog ≤8.
    fn materialize_post_deploy_config(
        &self,
        config: &crate::deployment::DeployConfig,
    ) -> crate::deployment::post_deploy_tests::PostDeployTestsConfig {
        let mut pdt = config.post_deploy_tests.clone().unwrap_or_default();
        if let Some(checks) = self.post_deploy_tests.as_ref() {
            pdt.checks = checks.clone();
        }
        if let Some(of) = self.on_test_failure {
            // REVISION 3: typed OnFailure (Fail|Warn) — clap rejected rollback
            // at parse, so this assignment cannot widen the variant set.
            pdt.on_failure = of;
        }
        if let Some(s) = self.apps_mode.as_deref() {
            pdt.apps_mode = match s {
                "standard" => crate::deployment::post_deploy_tests::AppsMode::Standard,
                "chatgpt" => crate::deployment::post_deploy_tests::AppsMode::Chatgpt,
                "claude-desktop" => crate::deployment::post_deploy_tests::AppsMode::ClaudeDesktop,
                _ => pdt.apps_mode,
            };
        }
        pdt
    }

    /// `cargo run` scenarios.
    ///
    /// Cog ≤8.
    async fn pre_build_widgets_and_set_env(
        widgets: &[crate::deployment::widgets::WidgetConfig],
        project_root: &std::path::Path,
        global_flags: &crate::commands::GlobalFlags,
    ) -> Result<()> {
        if widgets.is_empty() {
            return Ok(());
        }
        let quiet = !global_flags.should_output();
        let mut all_output_dirs: Vec<String> = Vec::with_capacity(widgets.len());
        for widget in widgets {
            let resolved =
                crate::deployment::widgets::run_widget_build(widget, project_root, quiet).await?;
            all_output_dirs.push(resolved.absolute_output_dir.to_string_lossy().to_string());
        }
        // REVISION 3 HIGH-C1: colon-join all widget output dirs (Unix PATH
        // convention). Empty case was early-returned above so 79-04's
        // build.rs local-discovery fallback (HIGH-G1) takes over for
        // direct-cargo-run scenarios.
        let joined = all_output_dirs.join(":");
        // SAFETY: env var mutation is the explicit purpose of this fn —
        // downstream `target.build()` invokes `cargo build` which reads
        // PMCP_WIDGET_DIRS via the build.rs scaffold (Phase 79 Wave 4).
        std::env::set_var("PMCP_WIDGET_DIRS", &joined);
        Ok(())
    }

    async fn execute_async(&self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        // Anchor project_root on `.pmcp/deploy.toml` (or an explicit
        // --manifest-path), NOT on the first ancestor Cargo.toml. init falls
        // back to current_dir(); reads error "Deployment not initialized".
        let for_init = matches!(&self.action, Some(DeployAction::Init { .. }));
        let project_root = self.resolve_project_root(for_init)?;
        // Jidoka: fire the footgun guard before ANY scaffolding for init.
        if for_init {
            Self::guard_init_root(&project_root)?;
        }

        // Get target from flag or config
        let target_id = self.get_target_id(&project_root)?;

        // Get target registry and resolve target
        let registry = crate::deployment::TargetRegistry::new();
        let target = registry.get(&target_id)?;

        match &self.action {
            Some(action) => {
                match action {
                    DeployAction::Init {
                        region,
                        skip_credentials_check,
                        oauth,
                        oauth_shared,
                        cognito_user_pool_id,
                        cognito_pool_name,
                        social_providers,
                        name,
                    } => {
                        if let Some(name) = name {
                            crate::deployment::server_name::validate_server_name(name)?;
                        }
                        let init_region = default_init_region(&target_id, region.as_deref());
                        // For init, route through InitCommand for aws-lambda and
                        // container targets (azure-container-apps). InitCommand
                        // dispatches on target_type at the top of execute():
                        // the container arm writes a deploy.toml stub + invokes
                        // the target's init() (Dockerfile) and skips AWS creds + CDK.
                        if target_id == "aws-lambda" || target_id == "azure-container-apps" {
                            emit_target_banner_if_resolved(global_flags, &project_root, None);
                            let mut cmd = InitCommand::new(project_root)
                                .with_region(&init_region)
                                .with_credentials_check(!skip_credentials_check)
                                .with_target_type(&target_id);
                            if let Some(name) = name {
                                cmd = cmd.with_server_name(name);
                            }

                            // Configure OAuth if specified
                            if let Some(provider) = oauth {
                                cmd = cmd.with_oauth_provider(provider);
                            }
                            if let Some(shared_name) = oauth_shared {
                                cmd = cmd.with_oauth_shared(&shared_name);
                            }
                            if let Some(pool_id) = cognito_user_pool_id {
                                cmd = cmd.with_cognito_user_pool_id(&pool_id);
                            }
                            if let Some(pool_name) = cognito_pool_name {
                                cmd = cmd.with_cognito_pool_name(&pool_name);
                            }
                            if let Some(providers) = social_providers {
                                cmd = cmd.with_social_providers(providers.clone());
                            }

                            cmd.execute()
                        } else if target_id == "google-cloud-run" {
                            // Cloud Run init writes a Cloud Run-shaped
                            // .pmcp/deploy.toml ([target] + [gcp] + [server] +
                            // [environment]) — see upstream issue #260.
                            //
                            // Prefer loading the existing deploy.toml so that
                            // operator-authored [layout] / [server].binary /
                            // [environment] blocks flow into the Dockerfile +
                            // cloudbuild.yaml generators. Without this, a
                            // pre-existing deploy.toml is preserved on disk
                            // (save_if_missing) but the in-memory default is
                            // still what generate_dockerfile() sees, so
                            // [layout] kind = "multi-crate-isolated" is
                            // silently ignored on re-init.
                            //
                            // Gate the load on file existence so parse and
                            // permission errors propagate as deploy failures
                            // instead of silently falling back to a default
                            // scaffold (which would mask a malformed
                            // deploy.toml as "no file present").
                            let config = cloud_run_init_from_flags(
                                &project_root,
                                region.as_deref(),
                                name.as_deref(),
                            )?;
                            target.init(&config).await
                        } else {
                            // For other targets (pmcp-run, cloudflare-workers, …), use the
                            // new modular approach. default_config_for_target picks the
                            // SHAPE for the target (google-cloud-run reaches it through
                            // cloud_run_init_config above) — without the branch, a
                            // non-AWS init wrote an AWS-shape deploy.toml (no [gcp],
                            // required [aws]/memory_mb), then save_if_missing cemented
                            // it on every re-init (n51 follow-up #1).
                            let server_name = init_server_name(&project_root, name.as_deref())?;
                            let mut config = default_config_for_target(
                                &target_id,
                                server_name,
                                init_region,
                                project_root.clone(),
                            );

                            // Update target type to match the actual target. (For
                            // google-cloud-run the constructor already sets it; the
                            // explicit reassignment keeps the post-condition uniform.)
                            config.target.target_type = target_id.clone();

                            // Configure OAuth if specified (for pmcp-run target)
                            if let Some(provider) = oauth {
                                if provider == "cognito" || provider == "oidc" {
                                    config.auth.enabled = true;
                                    config.auth.provider = provider.clone();

                                    // Set default scopes if not specified
                                    if config.auth.dcr.default_scopes.is_empty() {
                                        config.auth.dcr.default_scopes = vec![
                                            "openid".to_string(),
                                            "email".to_string(),
                                            "mcp/read".to_string(),
                                            "mcp/write".to_string(),
                                        ];
                                    }
                                }
                            }

                            emit_target_banner_if_resolved(
                                global_flags,
                                &project_root,
                                Some(&config),
                            );
                            target.init(&config).await
                        }
                    },
                    DeployAction::Logs { tail, lines } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        target.logs(&config, *tail, *lines).await
                    },
                    DeployAction::Metrics { period } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        let metrics = target.metrics(&config, period).await?;
                        // Requested data -- always show
                        println!("Metrics for {}: {}", target.name(), metrics.period);
                        Ok(())
                    },
                    DeployAction::Test {} => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        let results = target.test(&config, global_flags.verbose).await?;
                        // Test results are requested output
                        if results.success {
                            println!(
                                "All tests passed ({}/{})",
                                results.tests_passed, results.tests_run
                            );
                        } else {
                            println!(
                                "Some tests failed ({}/{})",
                                results.tests_passed, results.tests_run
                            );
                        }
                        Ok(())
                    },
                    DeployAction::Rollback { version, yes: _ } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        target.rollback(&config, version.as_deref()).await
                    },
                    DeployAction::Destroy {
                        yes,
                        clean,
                        no_wait,
                    } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));

                        if !yes {
                            println!("WARNING: This will destroy deployment on {}", target.name());
                            print!("Type '{}' to confirm: ", config.server.name);
                            use std::io::{self, Write};
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if input.trim() != config.server.name {
                                println!("Confirmation failed. Aborting.");
                                return Ok(());
                            }
                        }

                        // Use async destroy if --no-wait is specified and target supports it
                        if *no_wait && target.supports_async_operations() {
                            let result = target.destroy_async(&config, *clean).await?;
                            if let Some(op) = result.async_operation {
                                if global_flags.should_output() {
                                    println!();
                                    println!("{}", op.message);
                                    println!();
                                    println!("Destruction initiated. Use the following to check progress:");
                                    println!("   cargo pmcp deploy status {}", op.operation_id);
                                }
                            } else if global_flags.should_output() {
                                println!("{}", result.message);
                            }
                            Ok(())
                        } else {
                            // Default behavior: wait for completion
                            target.destroy(&config, *clean).await
                        }
                    },
                    DeployAction::Secrets { action } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        let secrets_action = match action {
                            SecretsAction::Set { key, from_env } => {
                                crate::deployment::SecretsAction::Set {
                                    key: key.clone(),
                                    from_env: from_env.clone(),
                                }
                            },
                            SecretsAction::List => crate::deployment::SecretsAction::List,
                            SecretsAction::Delete { key, yes } => {
                                crate::deployment::SecretsAction::Delete {
                                    key: key.clone(),
                                    yes: *yes,
                                }
                            },
                        };
                        target.secrets(&config, secrets_action).await
                    },
                    DeployAction::Outputs { format } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));
                        let outputs = target.outputs(&config).await?;

                        match format {
                            FormatValue::Json => {
                                println!("{}", serde_json::to_string_pretty(&outputs)?);
                            },
                            FormatValue::Text => {
                                outputs.display();
                            },
                        }
                        Ok(())
                    },
                    DeployAction::Login => {
                        // Login is target-specific
                        match target_id.as_str() {
                            "pmcp-run" => {
                                emit_target_banner_if_resolved(global_flags, &project_root, None);
                                crate::deployment::targets::pmcp_run::login().await
                            },
                            _ => {
                                bail!("Login is not supported for target: {}", target_id);
                            },
                        }
                    },
                    DeployAction::Logout => {
                        // Logout is target-specific (local-only — no AWS/pmcp.run call,
                        // banner intentionally NOT emitted per RESEARCH §7).
                        match target_id.as_str() {
                            "pmcp-run" => crate::deployment::targets::pmcp_run::logout(),
                            _ => {
                                bail!("Logout is not supported for target: {}", target_id);
                            },
                        }
                    },
                    DeployAction::Oauth { action } => {
                        // OAuth is only supported for pmcp-run target
                        if target_id != "pmcp-run" {
                            bail!("OAuth management is only supported for pmcp-run target");
                        }
                        emit_target_banner_if_resolved(global_flags, &project_root, None);
                        handle_oauth_action(action).await
                    },
                    DeployAction::Status { operation_id } => {
                        // Status is only supported for targets with async operations
                        if !target.supports_async_operations() {
                            bail!(
                                "Async operation status is not supported for target: {}",
                                target_id
                            );
                        }

                        if global_flags.should_output() {
                            println!("Checking operation status...");
                            println!();
                        }

                        emit_target_banner_if_resolved(global_flags, &project_root, None);
                        let status = target.get_operation_status(operation_id).await?;

                        // Status output is requested data
                        match status.status {
                            crate::deployment::OperationStatus::Initiated => {
                                println!("Status: Initiated");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Running => {
                                println!("Status: Running");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Completed => {
                                println!("Status: Completed");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Failed => {
                                println!("Status: Failed");
                                println!("   {}", status.message);
                            },
                        }

                        if let Some(metadata) = &status.metadata {
                            if let Some(updated_at) = metadata.get("updated_at") {
                                println!();
                                println!("   Last updated: {}", updated_at);
                            }
                        }

                        Ok(())
                    },
                }
            },
            None => {
                // No subcommand = deploy

                // --- Secret resolution (pre-deploy step) ---
                // Extract metadata for secret requirements, load .env, resolve.
                let metadata = crate::deployment::metadata::McpMetadata::extract(&project_root)?;
                let dotenv_vars = crate::secrets::load_dotenv(&project_root);
                let resolution =
                    crate::secrets::resolve_secrets(&metadata.resources.secrets, &dotenv_vars);
                crate::secrets::print_secret_report(
                    &resolution,
                    &metadata.server_id,
                    &target_id,
                    !global_flags.should_output(),
                );

                let mut config = crate::deployment::DeployConfig::load(&project_root)?;

                // DSTK-01: carry the --regenerate-stack/--force opt-in via the
                // config carrier (the #[serde(skip)] runtime field), mirroring
                // the config.secrets injection below — the flag travels on
                // `config`, not a new trait parameter, so both deploy targets
                // read it uniformly through DeployConfig.
                config.regenerate_stack = self.regenerate_stack;

                // For aws-lambda: inject resolved secrets into config.secrets (transient,
                // never saved). These flow to DeployExecutor.extra_env -> CDK process env.
                if target_id == "aws-lambda" {
                    config.secrets = resolution.found.clone();
                }

                // For pmcp-run: show server-side injection note if secrets are missing (D-08)
                if target_id == "pmcp-run"
                    && resolution.missing.iter().any(|r| r.required)
                    && global_flags.should_output()
                {
                    println!(
                        "   Note: pmcp.run injects secrets server-side from its managed Secrets Manager."
                    );
                }

                emit_target_banner_if_resolved(global_flags, &project_root, Some(&config));

                // Step 2.5: Widget pre-build (Phase 79 — Failure Mode A + B mitigation).
                //
                // REVISION 3 HIGH-C1: `PMCP_WIDGET_DIRS` (colon-separated list,
                // Unix PATH convention) is set ONCE here covering ALL widgets so
                // 79-04's generated build.rs picks up every output dir via
                // `cargo:rerun-if-env-changed=PMCP_WIDGET_DIRS` plus per-dir
                // `cargo:rerun-if-changed`. Replaces the pre-revision-3 single
                // `PMCP_WIDGET_DIR` which was last-widget-wins broken for
                // multi-widget projects.
                //
                // F-4 mitigation: there is no `cargo pmcp build` subcommand
                // (verified via `enum Commands` in `cargo-pmcp/src/main.rs:83`),
                // so this `cargo pmcp deploy` execute_async path is the ONLY
                // place that gets the Step 2.5 hook.
                // Single workspace scan — `detect_widgets` may invoke `cargo metadata`
                // on the convention path, so cache the result for both Step 2.5 and
                // the post-deploy `widgets_present` check below.
                let detected_widgets =
                    crate::deployment::widgets::detect_widgets(&config, &project_root);

                if !self.no_widget_build {
                    Self::pre_build_widgets_and_set_env(
                        &detected_widgets,
                        &project_root,
                        global_flags,
                    )
                    .await?;
                }

                if self.widgets_only {
                    if global_flags.should_output() {
                        println!(
                            "\n✓ Widgets built (--widgets-only); skipping cargo build + deploy"
                        );
                    }
                    return Ok(());
                }

                // Phase 86 Plan 05 (M3/H3): detect a config-driven single-crate
                // project and emit an informational note. Detection drives NO new
                // code path of its own — the existing `[assets]`-driven bundling
                // (the scaffold's deploy.toml lists config.toml + schema.sql) and
                // the single-crate `find_lambda_package_dir` fallback (H3) handle
                // this layout. pmcp.run is selected by the scaffold's
                // `target_type = "pmcp-run"` (M3), NOT inferred from project shape,
                // and NO `TargetEntry` variant is added (D-10).
                if is_config_driven_project(&project_root) && global_flags.should_output() {
                    println!(
                        "   Detected config-driven project (config.toml + schema.sql + pmcp-server-toolkit); \
                         bundling assets and building the project root as the Lambda package."
                    );
                }

                let artifact = target.build(&config).await?;
                let outputs = target.deploy(&config, artifact).await?;

                // Step 4.5: Post-deploy verification (Phase 79 — Failure Mode C
                // mitigation). Subprocess-spawn `cargo pmcp test {check, conformance,
                // apps} --format=json` via `current_exe()` per REVISION 3 HIGH-1
                // (Plan 79-05's JSON contract). REVISION 3 HIGH-C2: NO auth
                // resolution here — child inherits parent env and resolves via
                // existing `AuthMethod::None` Phase 74 cache + refresh path.
                //
                // F-6 mitigation per gsd-plan-checker REVISION 1 (retained): the
                // orchestrator (`run_post_deploy_tests` -> `interpret_outcomes`)
                // is the SOLE banner-print site. Caller MUST NOT eprintln the
                // failure banner.
                if !self.no_post_deploy_test {
                    let pdt_config = self.materialize_post_deploy_config(&config);
                    let widgets_present = !detected_widgets.is_empty();
                    let url = outputs.url.as_deref().unwrap_or("");
                    let target_id_str = target.id();
                    let quiet = !global_flags.should_output();

                    if let Err(failure) =
                        crate::deployment::post_deploy_tests::run_post_deploy_tests(
                            url,
                            target_id_str,
                            widgets_present,
                            &pdt_config,
                            quiet,
                        )
                        .await
                    {
                        // Print outputs first so user sees what's deployed before exiting.
                        if global_flags.should_output() {
                            println!();
                            outputs.display();
                        }
                        // REVISION 3 HIGH-2: exit code is 3 for broken-but-live, 2
                        // for infra. emit_ci_annotation already fired from inside
                        // interpret_outcomes.
                        std::process::exit(failure.exit_code());
                    }
                }

                if global_flags.should_output() {
                    println!();
                    outputs.display();
                }

                // Save deployment info for pmcp-run target (for landing page integration)
                if target_id == "pmcp-run" {
                    Self::save_deployment_info(&project_root, &outputs)?;

                    // Handle OAuth configuration if --shared-pool was provided
                    if let Some(ref pool_name) = self.shared_pool {
                        if global_flags.should_output() {
                            println!();
                            println!("Configuring OAuth with shared pool: {}", pool_name);
                        }

                        // Get the server ID from outputs
                        let server_id = outputs
                            .custom
                            .get("server_id")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                outputs.custom.get("deployment_id").and_then(|v| v.as_str())
                            })
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Could not determine server ID from deployment outputs"
                                )
                            })?;

                        // Configure OAuth with shared pool
                        let oauth_action = OAuthAction::Enable {
                            server: server_id.to_string(),
                            copy_from: None,
                            scopes: None, // Use defaults
                            dcr: true,
                            public_clients: None, // Use defaults
                            shared_pool: Some(pool_name.clone()),
                        };

                        handle_oauth_action(&oauth_action).await?;
                    } else if !self.no_oauth && global_flags.should_output() {
                        // Check if OAuth is already enabled (from backend query)
                        let oauth_already_enabled = outputs
                            .custom
                            .get("oauth_enabled")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if !oauth_already_enabled {
                            // Show hint about OAuth options only if not already configured
                            println!();
                            println!("OAuth not configured. To add authentication:");
                            if let Some(server_id) =
                                outputs.custom.get("server_id").and_then(|v| v.as_str())
                            {
                                println!(
                                    "   cargo pmcp deploy oauth enable --server {}",
                                    server_id
                                );
                                println!("   cargo pmcp deploy oauth enable --server {} --shared-pool <name>", server_id);
                            } else {
                                println!("   cargo pmcp deploy oauth enable --server <server_id>");
                            }
                        }
                    }
                }

                Ok(())
            },
        }
    }

    fn get_target_id(&self, project_root: &PathBuf) -> Result<String> {
        // Priority: --target-type/--target flag > config file > default.
        //
        // The flag value is resolved against BOTH the known backend types
        // (aws-lambda, cloudflare-workers, google-cloud-run, pmcp-run) AND the
        // operator's configured named targets, so `--target prod` (a named
        // target) and `--target pmcp-run` (a backend type) both work. An
        // unknown value hard-errors here with a helpful message BEFORE
        // `registry.get` would emit its bare "Unknown deployment target".
        if let Some(value) = &self.target_type {
            let registry = crate::deployment::TargetRegistry::new();
            let valid_owned: Vec<String> =
                registry.list().iter().map(|t| t.id().to_string()).collect();
            let valid_types: Vec<&str> = valid_owned.iter().map(String::as_str).collect();
            let named = read_named_target_kinds();
            return resolve_target_flag(value, &valid_types, &named);
        }

        // Try to read from config
        if let Ok(config) = crate::deployment::DeployConfig::load(project_root) {
            return Ok(config.target.target_type.clone());
        }

        // Default to AWS Lambda
        Ok("aws-lambda".to_string())
    }

    /// Resolve a project root from an explicit `--manifest-path` override.
    ///
    /// Accepts a directory (used directly) or a `.pmcp/deploy.toml` file (its
    /// parent-of-`.pmcp` is the root). Errors when the override does not resolve
    /// to an existing `.pmcp/deploy.toml`.
    fn resolve_manifest_override(path: &Path) -> Result<PathBuf> {
        // If it's a file named deploy.toml under a .pmcp dir → root is .pmcp's parent.
        if path.is_file() {
            let pmcp_dir = path
                .parent()
                .filter(|p| p.file_name().is_some_and(|n| n == ".pmcp"))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "--manifest-path file must be a .pmcp/deploy.toml: {}",
                        path.display()
                    )
                })?;
            let root = pmcp_dir.parent().ok_or_else(|| {
                anyhow::anyhow!(
                    "--manifest-path .pmcp dir has no parent: {}",
                    path.display()
                )
            })?;
            if !root.join(".pmcp/deploy.toml").exists() {
                bail!("No .pmcp/deploy.toml at resolved root: {}", root.display());
            }
            return Ok(root.to_path_buf());
        }
        // Otherwise treat as a directory.
        if path.join(".pmcp/deploy.toml").exists() {
            return Ok(path.to_path_buf());
        }
        bail!(
            "--manifest-path does not contain .pmcp/deploy.toml: {}",
            path.display()
        );
    }

    /// Resolve `project_root` honoring precedence: `--manifest-path` override
    /// first, then deploy-config anchoring on `.pmcp/deploy.toml`.
    ///
    /// `for_init` controls the fallback when no deploy config is found:
    /// - init → `current_dir()` (restores 0.6.x fresh-init behavior)
    /// - read → "Deployment not initialized" error
    fn resolve_project_root(&self, for_init: bool) -> Result<PathBuf> {
        if let Some(mp) = &self.manifest_path {
            return Self::resolve_manifest_override(mp);
        }
        match find_deploy_root()? {
            Some(root) => Ok(root),
            None if for_init => std::env::current_dir().context("Failed to get current directory"),
            None => bail!("Deployment not initialized. Run: cargo pmcp deploy init"),
        }
    }

    /// Jidoka footgun guard for init: refuse to scaffold into a resolved root
    /// that is neither the current directory nor an existing deploy root.
    fn guard_init_root(root: &Path) -> Result<()> {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        let same_as_cwd = root.canonicalize().ok() == cwd.canonicalize().ok();
        if !same_as_cwd && !root.join(".pmcp/deploy.toml").exists() {
            bail!(
                "Refusing to scaffold deployment into '{}': it is not the current directory \
                 and has no existing .pmcp/deploy.toml. Run `cargo pmcp deploy init` from your \
                 deploy directory, or pass --manifest-path <dir-or-deploy.toml>.",
                root.display()
            );
        }
        Ok(())
    }

    /// Save deployment info to .pmcp/deployment.toml for landing page integration
    fn save_deployment_info(
        project_root: &PathBuf,
        outputs: &crate::deployment::DeploymentOutputs,
    ) -> Result<()> {
        use std::io::Write;

        // Extract server_id from custom outputs (the server name, e.g., "chess")
        // NOT deployment_id which is like "dep_xxx" - landing pages use server_id
        let server_id = outputs
            .custom
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No server_id in outputs"))?;

        // Get URL
        let url = outputs
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No URL in deployment outputs"))?;

        // Create .pmcp directory if it doesn't exist
        let pmcp_dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir)?;

        // Create deployment.toml content
        let content = format!(
            r#"# Auto-generated deployment info for landing page integration
# This file is created automatically when deploying to pmcp-run

[deployment]
server_id = "{}"
endpoint = "{}"
"#,
            server_id, url
        );

        // Write to .pmcp/deployment.toml
        let deployment_file = pmcp_dir.join("deployment.toml");
        let mut file = std::fs::File::create(&deployment_file)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}

/// Read the operator's configured named targets from the user config and map
/// each name to the backend type it deploys to (its `type_tag()`).
///
/// A missing or unreadable user config is treated as "no named targets" (an
/// empty map) — never an error, because most operators have no named targets
/// and `--target <backend-type>` must keep working regardless.
fn read_named_target_kinds() -> std::collections::BTreeMap<String, String> {
    use crate::commands::configure::config::{default_user_config_path, TargetConfigV1};
    let path = default_user_config_path();
    TargetConfigV1::read(&path).map_or_else(
        |_| std::collections::BTreeMap::new(),
        |cfg| {
            cfg.targets
                .into_iter()
                .map(|(name, entry)| (name, entry.type_tag().to_string()))
                .collect()
        },
    )
}

/// Resolve a `--target` / `--target-type` flag value into a backend type id.
///
/// Resolution order:
/// 1. `value` is a known backend type (`valid_types`) → returns it unchanged.
/// 2. `value` is a configured named target (`named` maps name → backend type)
///    → returns the backend type that named target deploys to.
/// 3. Otherwise → hard-errors with a message listing the valid backend types,
///    the configured named-target names, and (when `value` is a near-miss of a
///    known type) a `--target-type` hint.
///
/// This is the single unified path for both `--target` and its
/// `--target-type` synonym; both flow through here so they behave identically.
///
/// # Errors
///
/// Returns an error when `value` is neither a known backend type nor a
/// configured named target.
fn resolve_target_flag(
    value: &str,
    valid_types: &[&str],
    named: &std::collections::BTreeMap<String, String>,
) -> Result<String> {
    if valid_types.contains(&value) {
        return Ok(value.to_string());
    }
    if let Some(backend) = named.get(value) {
        return Ok(backend.clone());
    }
    bail!("{}", unknown_target_message(value, valid_types, named));
}

/// Build the helpful hard-error message for an unknown `--target` value.
///
/// Lists the valid backend types and the configured named-target names, and
/// appends a `--target-type <type>` hint when `value` looks like a near-miss
/// of a known backend type (a prefix of, or within edit-distance 1 of, a known
/// type).
fn unknown_target_message(
    value: &str,
    valid_types: &[&str],
    named: &std::collections::BTreeMap<String, String>,
) -> String {
    let types_list = valid_types.join(", ");
    let named_list = if named.is_empty() {
        "(none configured)".to_string()
    } else {
        named.keys().cloned().collect::<Vec<_>>().join(", ")
    };

    let mut msg = format!(
        "Unknown --target value '{value}'.\n  \
         Valid backend types: {types_list}\n  \
         Configured named targets: {named_list}"
    );

    if let Some(suggestion) = near_miss_hint(value, valid_types) {
        msg.push_str(&format!("\n  Did you mean `--target-type {suggestion}`?"));
    }

    msg
}

/// Return the first known backend type that `value` is a near-miss of: either
/// `value` is a prefix of the type, or the two are within Levenshtein
/// edit-distance 1. Returns `None` when nothing is close.
fn near_miss_hint(value: &str, valid_types: &[&str]) -> Option<String> {
    valid_types
        .iter()
        .find(|t| t.starts_with(value) || levenshtein_within_one(value, t))
        .map(|t| (*t).to_string())
}

/// Returns `true` when `a` and `b` are within Levenshtein edit-distance 1
/// (equal, or differ by a single insertion, deletion, or substitution).
fn levenshtein_within_one(a: &str, b: &str) -> bool {
    let (a, b): (&[u8], &[u8]) = (a.as_bytes(), b.as_bytes());
    let (la, lb) = (a.len(), b.len());
    if la.abs_diff(lb) > 1 {
        return false;
    }
    if la == lb {
        // At most one substitution.
        return a.iter().zip(b).filter(|(x, y)| x != y).count() <= 1;
    }
    // Lengths differ by exactly 1: the longer must contain the shorter with a
    // single character skipped.
    let (short, long) = if la < lb { (a, b) } else { (b, a) };
    let (mut i, mut j, mut skipped) = (0usize, 0usize, false);
    while i < short.len() && j < long.len() {
        if short[i] == long[j] {
            i += 1;
            j += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
            j += 1;
        }
    }
    true
}

/// Handle OAuth subcommands for pmcp.run
async fn handle_oauth_action(action: &OAuthAction) -> Result<()> {
    use crate::deployment::targets::pmcp_run::auth;

    // Get credentials once; individual handlers consume access_token.
    let credentials = auth::get_credentials().await?;
    let access_token = &credentials.access_token;

    match action {
        OAuthAction::Enable {
            server,
            copy_from,
            scopes,
            dcr,
            public_clients,
            shared_pool,
        } => {
            handle_oauth_enable(
                access_token,
                server,
                copy_from.as_deref(),
                scopes.clone(),
                *dcr,
                public_clients.clone(),
                shared_pool.clone(),
            )
            .await
        },
        OAuthAction::Disable { server } => handle_oauth_disable(access_token, server).await,
        OAuthAction::Status { server } => handle_oauth_status(access_token, server).await,
    }
}

/// Return true if output is NOT suppressed (PMCP_QUIET unset).
fn oauth_not_quiet() -> bool {
    std::env::var("PMCP_QUIET").is_err()
}

/// Implement `OAuthAction::Enable`. Resolves config (copy_from + defaults +
/// explicit overrides), invokes graphql::configure_server_oauth, and prints
/// endpoint + SSO details.
#[allow(clippy::too_many_arguments)]
async fn handle_oauth_enable(
    access_token: &str,
    server: &str,
    copy_from: Option<&str>,
    scopes: Option<Vec<String>>,
    dcr: bool,
    public_clients: Option<Vec<String>>,
    shared_pool: Option<String>,
) -> Result<()> {
    use crate::deployment::targets::pmcp_run::graphql;

    if oauth_not_quiet() {
        println!("Enabling OAuth for server: {}", server);
        println!();
    }

    // Resolve final configuration values
    // Priority: explicit params > copied values > defaults
    let (final_scopes, final_dcr, final_public_clients, final_shared_pool) = resolve_oauth_config(
        access_token,
        copy_from,
        scopes,
        dcr,
        public_clients,
        shared_pool.clone(),
    )
    .await?;

    // Display what configuration will be applied
    if (copy_from.is_some() || shared_pool.is_some()) && oauth_not_quiet() {
        print_oauth_config_summary(
            &final_scopes,
            final_dcr,
            final_public_clients.as_ref(),
            final_shared_pool.as_ref(),
        );
    }

    let oauth_config = graphql::configure_server_oauth(
        access_token,
        server,
        true,
        Some(final_scopes),
        Some(final_dcr),
        final_public_clients,
        final_shared_pool,
    )
    .await
    .context("Failed to configure OAuth")?;

    let not_quiet = oauth_not_quiet();
    if not_quiet {
        println!("OAuth enabled successfully!");
    }
    println!();
    print_configure_result_endpoints(&oauth_config);

    // Show helpful next steps for SSO
    if (copy_from.is_some() || shared_pool.is_some()) && not_quiet {
        println!();
        println!("SSO enabled: Users from the shared pool can access this server");
    }

    Ok(())
}

/// Implement `OAuthAction::Disable`.
async fn handle_oauth_disable(access_token: &str, server: &str) -> Result<()> {
    use crate::deployment::targets::pmcp_run::graphql;

    let not_quiet = oauth_not_quiet();
    if not_quiet {
        println!("Disabling OAuth for server: {}", server);
        println!();
    }

    graphql::disable_server_oauth(access_token, server)
        .await
        .context("Failed to disable OAuth")?;

    if not_quiet {
        println!("OAuth disabled successfully!");
        println!();
        println!("Note: The Cognito User Pool was NOT deleted.");
        println!("   You can re-enable OAuth at any time with:");
        println!("   cargo pmcp deploy oauth enable --server {}", server);
    }

    Ok(())
}

/// Implement `OAuthAction::Status`.
async fn handle_oauth_status(access_token: &str, server: &str) -> Result<()> {
    use crate::deployment::targets::pmcp_run::graphql;

    println!("OAuth Status for server: {}", server);
    println!();

    match graphql::fetch_server_oauth_endpoints(access_token, server).await {
        Ok(endpoints) if endpoints.oauth_enabled => print_enabled_status(&endpoints),
        Ok(_) => print_inactive_status(server, "Disabled"),
        Err(_) => print_inactive_status(server, "Not configured"),
    }

    Ok(())
}

/// Print the resolved OAuth configuration summary (pre-apply).
fn print_oauth_config_summary(
    scopes: &[String],
    dcr: bool,
    public_clients: Option<&Vec<String>>,
    shared_pool: Option<&String>,
) {
    println!("OAuth Configuration:");
    println!("   Scopes:         {}", scopes.join(", "));
    println!(
        "   DCR:            {}",
        if dcr { "enabled" } else { "disabled" }
    );
    println!(
        "   Public clients: {}",
        public_clients
            .map(|p| p.join(", "))
            .unwrap_or_else(|| "(default)".to_string())
    );
    if let Some(pool) = shared_pool {
        println!("   Shared pool:    {}", pool);
    }
    println!();
}

/// Print endpoint block for an `OAuthConfig` (result of configure_server_oauth).
fn print_configure_result_endpoints(
    oauth_config: &crate::deployment::targets::pmcp_run::graphql::OAuthConfig,
) {
    println!("OAuth Endpoints:");
    if let Some(ref discovery) = oauth_config.discovery_url {
        println!("   Discovery:     {}", discovery);
    }
    if let Some(ref register) = oauth_config.registration_endpoint {
        println!("   Registration:  {}", register);
    }
    if let Some(ref authorize) = oauth_config.authorization_endpoint {
        println!("   Authorization: {}", authorize);
    }
    if let Some(ref token) = oauth_config.token_endpoint {
        println!("   Token:         {}", token);
    }
    if let Some(ref pool_id) = oauth_config.user_pool_id {
        println!();
        println!("   User Pool ID:  {}", pool_id);
    }
    if let Some(ref region) = oauth_config.user_pool_region {
        println!("   Region:        {}", region);
    }
}

/// Print status block when OAuth is currently enabled on the server.
fn print_enabled_status(endpoints: &crate::deployment::targets::pmcp_run::graphql::OAuthEndpoints) {
    println!("   Status: Enabled");
    if let Some(ref provider) = endpoints.provider {
        println!("   Provider: {}", provider);
    }
    if let Some(dcr) = endpoints.dcr_enabled {
        println!("   DCR: {}", if dcr { "enabled" } else { "disabled" });
    }
    if let Some(ref scopes) = endpoints.scopes {
        println!("   Scopes: {}", scopes.join(", "));
    }
    println!();
    println!("OAuth Endpoints:");
    if let Some(ref discovery) = endpoints.discovery_url {
        println!("   Discovery:     {}", discovery);
    }
    if let Some(ref register) = endpoints.registration_endpoint {
        println!("   Registration:  {}", register);
    }
    if let Some(ref authorize) = endpoints.authorization_endpoint {
        println!("   Authorization: {}", authorize);
    }
    if let Some(ref token) = endpoints.token_endpoint {
        println!("   Token:         {}", token);
    }
    println!();
    println!("Cognito Details:");
    if let Some(ref pool_id) = endpoints.user_pool_id {
        println!("   User Pool ID:  {}", pool_id);
    }
    if let Some(ref region) = endpoints.user_pool_region {
        println!("   Region:        {}", region);
    }
}

/// Print status block when OAuth is inactive (disabled or not configured).
fn print_inactive_status(server: &str, status_label: &str) {
    println!("   Status: {}", status_label);
    if oauth_not_quiet() {
        println!();
        println!("Enable OAuth with:");
        println!("   cargo pmcp deploy oauth enable --server {}", server);
    }
}

/// Tuple of OAuth values copied from a source server (None if not copying).
type CopiedOAuthConfig = (
    Option<Vec<String>>, // copied_scopes
    Option<bool>,        // copied_dcr
    Option<Vec<String>>, // copied_public_clients (always None — not returned by status endpoint)
    Option<String>,      // copied_pool
);

/// Resolve OAuth configuration with priority: explicit params > copied values > defaults
///
/// This function implements the configuration resolution logic:
/// 1. If `copy_from` is specified, fetch OAuth config from that server
/// 2. Use explicit parameters to override copied values
/// 3. Apply sensible defaults for any remaining unspecified values
async fn resolve_oauth_config(
    access_token: &str,
    copy_from: Option<&str>,
    explicit_scopes: Option<Vec<String>>,
    explicit_dcr: bool,
    explicit_public_clients: Option<Vec<String>>,
    explicit_shared_pool: Option<String>,
) -> Result<(Vec<String>, bool, Option<Vec<String>>, Option<String>)> {
    let default_scopes: Vec<String> = DEFAULT_OAUTH_SCOPES.iter().map(|s| s.to_string()).collect();
    let default_public_clients: Vec<String> = DEFAULT_PUBLIC_CLIENT_PATTERNS
        .iter()
        .map(|s| s.to_string())
        .collect();

    let (copied_scopes, copied_dcr, copied_public_clients, copied_pool) = match copy_from {
        Some(source_server) => fetch_copied_oauth_config(access_token, source_server).await?,
        None => (None, None, None, None),
    };

    let final_scopes = explicit_scopes.or(copied_scopes).unwrap_or(default_scopes);
    let final_dcr = resolve_dcr(copy_from.is_some(), explicit_dcr, copied_dcr);
    let final_public_clients = resolve_public_clients(
        explicit_public_clients,
        copied_public_clients,
        copy_from.is_some() || explicit_shared_pool.is_some(),
        default_public_clients,
    );
    let final_shared_pool = explicit_shared_pool.or(copied_pool);

    Ok((
        final_scopes,
        final_dcr,
        final_public_clients,
        final_shared_pool,
    ))
}

/// Fetch OAuth endpoints from `source_server` and translate them to the
/// copied-config tuple expected by `resolve_oauth_config`. Bails if the source
/// server has no OAuth enabled or no User Pool ID.
async fn fetch_copied_oauth_config(
    access_token: &str,
    source_server: &str,
) -> Result<CopiedOAuthConfig> {
    use crate::deployment::targets::pmcp_run::graphql;

    if oauth_not_quiet() {
        println!("Copying OAuth configuration from: {}", source_server);
    }

    let endpoints = graphql::fetch_server_oauth_endpoints(access_token, source_server)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to fetch OAuth configuration from '{}': {}\n\
                 Make sure the server exists and has OAuth enabled.\n\
                 You can check with: cargo pmcp deploy oauth status --server {}",
                source_server,
                e,
                source_server
            )
        })?;

    if !endpoints.oauth_enabled {
        bail!(
            "Source server '{}' does not have OAuth enabled. \
             Cannot copy configuration from a server without OAuth.",
            source_server
        );
    }

    let pool_id = endpoints.user_pool_id.ok_or_else(|| {
        anyhow::anyhow!(
            "Source server '{}' has OAuth enabled but no User Pool ID. \
             This is unexpected - please check the server configuration.",
            source_server
        )
    })?;

    if oauth_not_quiet() {
        println!("   Found User Pool: {}", pool_id);
        if let Some(ref scopes) = endpoints.scopes {
            println!("   Found scopes: {}", scopes.join(", "));
        }
        println!();
    }

    Ok((
        endpoints.scopes,
        endpoints.dcr_enabled,
        None, // Public client patterns not returned by status endpoint
        Some(pool_id),
    ))
}

/// Resolve the final DCR-enabled value:
/// - When copying from a source AND `explicit_dcr` is the default `true`, use the copied value.
/// - Otherwise use the explicit value (which has a clap default).
fn resolve_dcr(copying: bool, explicit_dcr: bool, copied_dcr: Option<bool>) -> bool {
    if copying && explicit_dcr {
        copied_dcr.unwrap_or(explicit_dcr)
    } else {
        explicit_dcr
    }
}

/// Resolve the final public-clients value with priority: explicit > copied >
/// default-when-pool-shared. Returns `None` if no signal is present (lets the
/// backend apply its own defaults).
fn resolve_public_clients(
    explicit: Option<Vec<String>>,
    copied: Option<Vec<String>>,
    pool_shared_or_copying: bool,
    default_public_clients: Vec<String>,
) -> Option<Vec<String>> {
    if explicit.is_some() {
        explicit
    } else if copied.is_some() {
        copied
    } else if pool_shared_or_copying {
        Some(default_public_clients)
    } else {
        None
    }
}

#[cfg(test)]
mod phase_77_banner_smoke_tests {
    use super::*;

    /// Phase 77 helper smoke: when no `~/.pmcp/config.toml` exists and no PMCP_TARGET is set,
    /// `emit_target_banner_if_resolved` must be a silent no-op (D-11 zero-touch). Establishing
    /// this for free preserves Phase 76 behavior for users who never ran `cargo pmcp configure`.
    #[test]
    #[serial_test::serial]
    fn helper_does_not_panic_when_no_config() {
        let home_tmp = tempfile::tempdir().unwrap();
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home_tmp.path());
        let saved_target = std::env::var_os("PMCP_TARGET");
        std::env::remove_var("PMCP_TARGET");

        let gf = crate::commands::GlobalFlags::default();
        let project_root = std::env::temp_dir();
        // Helper must NOT panic and MUST NOT print a banner when no config exists.
        emit_target_banner_if_resolved(&gf, &project_root, None);

        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_target {
            Some(v) => std::env::set_var("PMCP_TARGET", v),
            None => std::env::remove_var("PMCP_TARGET"),
        }
    }
}

#[cfg(test)]
mod config_driven_detection_tests {
    use super::*;

    fn write(path: &std::path::Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    /// M3: `is_config_driven_project` is true only when all THREE markers are
    /// present (config.toml + schema.sql + a `pmcp-server-toolkit` dep). Tested
    /// in-module because the `pub(crate)` helper is unreachable from an
    /// integration test.
    #[test]
    fn detects_full_config_driven_project() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            &root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\n\n[dependencies]\npmcp-server-toolkit = \"0.1.0\"\n",
        );
        write(&root.join("config.toml"), "[server]\nname = \"demo\"\n");
        write(
            &root.join("schema.sql"),
            "CREATE TABLE IF NOT EXISTS t(id INTEGER);\n",
        );

        assert!(is_config_driven_project(root));
    }

    /// Missing `schema.sql` ⇒ not config-driven (negative case the plan calls out).
    #[test]
    fn not_config_driven_when_schema_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            &root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\n\n[dependencies]\npmcp-server-toolkit = \"0.1.0\"\n",
        );
        write(&root.join("config.toml"), "[server]\nname = \"demo\"\n");
        // no schema.sql

        assert!(!is_config_driven_project(root));
    }

    /// Missing the toolkit dependency ⇒ not config-driven (a bare crate with the
    /// two files but no toolkit dep must NOT be treated as config-driven).
    #[test]
    fn not_config_driven_when_toolkit_dep_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(&root.join("Cargo.toml"), "[package]\nname = \"demo\"\n");
        write(&root.join("config.toml"), "[server]\nname = \"demo\"\n");
        write(
            &root.join("schema.sql"),
            "CREATE TABLE IF NOT EXISTS t(id INTEGER);\n",
        );

        assert!(!is_config_driven_project(root));
    }
}

#[cfg(test)]
mod manifest_resolution_tests {
    use super::*;

    fn write_deploy_toml(dir: &std::path::Path) {
        let pmcp = dir.join(".pmcp");
        std::fs::create_dir_all(&pmcp).unwrap();
        std::fs::write(pmcp.join("deploy.toml"), "[deployment]\n").unwrap();
    }

    /// (d) `--manifest-path` pointing at a DIR containing `.pmcp/deploy.toml`
    /// resolves to that dir.
    #[test]
    fn manifest_override_dir_form() {
        let tmp = tempfile::tempdir().unwrap();
        write_deploy_toml(tmp.path());

        let root =
            DeployCommand::resolve_manifest_override(tmp.path()).expect("dir form must resolve");
        assert_eq!(
            root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    /// (e) `--manifest-path` pointing at a `.pmcp/deploy.toml` FILE resolves to
    /// the file's grandparent (the parent-of-`.pmcp`).
    #[test]
    fn manifest_override_file_form() {
        let tmp = tempfile::tempdir().unwrap();
        write_deploy_toml(tmp.path());
        let file = tmp.path().join(".pmcp").join("deploy.toml");

        let root = DeployCommand::resolve_manifest_override(&file).expect("file form must resolve");
        assert_eq!(
            root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    /// (f) `--manifest-path` where no `.pmcp/deploy.toml` exists → clear error.
    #[test]
    fn manifest_override_missing_errors() {
        let tmp = tempfile::tempdir().unwrap();
        // No .pmcp/deploy.toml written.
        assert!(DeployCommand::resolve_manifest_override(tmp.path()).is_err());
    }

    /// (g) the init footgun guard fires when the resolved root is neither cwd
    /// nor a real deploy root; is_ok when root == cwd; is_ok when root has a
    /// deploy.toml.
    ///
    /// NOTE: this test mutates process-global cwd; CI runs `--test-threads=1`
    /// so serialization is guaranteed. Restore cwd before asserting.
    #[test]
    fn guard_init_root_fires_when_not_cwd_and_no_deploy_toml() {
        let tmp_other = tempfile::tempdir().unwrap(); // no deploy.toml
        let tmp_cwd = tempfile::tempdir().unwrap(); // a DIFFERENT dir
        let tmp_with_deploy = tempfile::tempdir().unwrap();
        write_deploy_toml(tmp_with_deploy.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp_cwd.path()).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let fires = DeployCommand::guard_init_root(tmp_other.path());
        let same_cwd_ok = DeployCommand::guard_init_root(&cwd);
        let deploy_root_ok = DeployCommand::guard_init_root(tmp_with_deploy.path());
        std::env::set_current_dir(&saved).unwrap();

        assert!(
            fires.is_err(),
            "guard must error for a non-cwd dir with no deploy.toml"
        );
        assert!(
            same_cwd_ok.is_ok(),
            "guard must allow the current directory"
        );
        assert!(
            deploy_root_ok.is_ok(),
            "guard must allow a dir with an existing .pmcp/deploy.toml"
        );
    }
}

/// Tests for `default_config_for_target` — guards n51-follow-up #1: the init
/// dispatch MUST pick the target-shape-appropriate constructor.
#[cfg(test)]
mod default_config_for_target_tests {
    use super::default_config_for_target;
    use std::path::PathBuf;

    /// Exercises the REAL init path: `cloud_run_init_from_flags` (what the
    /// `deploy init --target-type google-cloud-run` dispatch calls) with no
    /// `--region`, on a project without a deploy.toml. It used to test
    /// `default_config_for_target`'s Cloud Run arm directly, an arm init never
    /// reached (debug session `cargo-pmcp-deploy-targets`, finding A3).
    #[test]
    fn google_cloud_run_target_produces_gcp_shape() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test-server\"\nversion = \"0.1.0\"\n",
        )
        .expect("Cargo.toml");
        let cfg = super::cloud_run_init_from_flags(tmp.path(), None, None).expect("config");
        assert!(
            cfg.gcp.is_some(),
            "google-cloud-run init MUST produce a config with [gcp] set"
        );
        assert!(
            cfg.aws.is_none(),
            "google-cloud-run init MUST NOT produce a config with [aws] set"
        );
        assert_eq!(cfg.target.target_type, "google-cloud-run");
        assert_eq!(cfg.server.name, "test-server");
        let gcp = cfg.gcp.expect("[gcp]");
        // The operator replaces the placeholder via [gcp].project_id;
        // deploy::resolve_params treats it as unset (gcloud's project).
        assert_eq!(gcp.project_id, "your-gcp-project-id");
        assert_eq!(gcp.region, "us-central1");
    }

    #[test]
    fn pmcp_run_target_produces_aws_shape() {
        let cfg = default_config_for_target(
            "pmcp-run",
            "test-server".to_string(),
            "us-east-1".to_string(),
            PathBuf::from("/tmp/test"),
        );
        assert!(
            cfg.aws.is_some(),
            "pmcp-run init keeps the legacy aws-shape default (uses lambda primitives)"
        );
        assert!(cfg.gcp.is_none());
    }

    #[test]
    fn cloudflare_workers_target_produces_aws_shape() {
        // cloudflare-workers also uses default_for_server (no GCR-specific knobs).
        let cfg = default_config_for_target(
            "cloudflare-workers",
            "test-server".to_string(),
            "us-east-1".to_string(),
            PathBuf::from("/tmp/test"),
        );
        assert!(cfg.aws.is_some());
        assert!(cfg.gcp.is_none());
    }
}

#[cfg(test)]
mod target_resolution_tests {
    //! Unit tests for the unified `--target` / `--target-type` resolver.
    //!
    //! Hermetic: they call the pure `resolve_target_flag` with injected
    //! type-set + named-target-set, so they never read the operator's real
    //! `~/.pmcp/config.toml`.
    use super::{near_miss_hint, resolve_target_flag};
    use std::collections::BTreeMap;

    const VALID: &[&str] = &[
        "aws-lambda",
        "cloudflare-workers",
        "google-cloud-run",
        "pmcp-run",
    ];

    fn no_named() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn known_backend_types_resolve_to_themselves() {
        for ty in VALID {
            let got = resolve_target_flag(ty, VALID, &no_named())
                .unwrap_or_else(|e| panic!("{ty} must resolve: {e}"));
            assert_eq!(&got, ty);
        }
    }

    #[test]
    fn pmcp_run_still_works() {
        // The everyday flow must never regress.
        let got = resolve_target_flag("pmcp-run", VALID, &no_named()).expect("pmcp-run resolves");
        assert_eq!(got, "pmcp-run");
    }

    #[test]
    fn configured_named_target_resolves_to_its_backend_type() {
        let mut named = BTreeMap::new();
        named.insert("prod".to_string(), "aws-lambda".to_string());
        let got = resolve_target_flag("prod", VALID, &named).expect("named target resolves");
        assert_eq!(got, "aws-lambda");
    }

    #[test]
    fn unknown_value_hard_errors_with_helpful_message() {
        let mut named = BTreeMap::new();
        named.insert("prod".to_string(), "aws-lambda".to_string());
        let err = resolve_target_flag("totally-bogus", VALID, &named)
            .expect_err("unknown value must error");
        let msg = err.to_string();
        // (a) lists the valid backend types
        assert!(msg.contains("aws-lambda"), "must list valid types: {msg}");
        assert!(
            msg.contains("google-cloud-run"),
            "must list valid types: {msg}"
        );
        assert!(msg.contains("pmcp-run"), "must list valid types: {msg}");
        // (b) lists the configured named-target names
        assert!(
            msg.contains("prod"),
            "must list configured named targets: {msg}"
        );
        // never a bare "Unknown deployment target" / "<unset>"
        assert!(
            !msg.contains("<unset>"),
            "must not fall through to <unset>: {msg}"
        );
    }

    #[test]
    fn near_miss_of_known_type_hints_target_type() {
        // "aws-lamda" (one deletion) and "google" (prefix) are near-misses.
        let err =
            resolve_target_flag("aws-lamda", VALID, &no_named()).expect_err("typo must error");
        let msg = err.to_string();
        assert!(
            msg.contains("--target-type aws-lambda"),
            "edit-distance-1 typo must hint --target-type: {msg}"
        );

        let err2 =
            resolve_target_flag("google", VALID, &no_named()).expect_err("prefix must error");
        assert!(
            err2.to_string().contains("--target-type google-cloud-run"),
            "prefix near-miss must hint --target-type: {err2}"
        );
    }

    #[test]
    fn target_type_synonym_passes_a_real_type() {
        // --target-type is the explicit type-only synonym; a real type passes
        // through the same resolver unchanged.
        let got =
            resolve_target_flag("cloudflare-workers", VALID, &no_named()).expect("type resolves");
        assert_eq!(got, "cloudflare-workers");
    }

    #[test]
    fn near_miss_hint_returns_none_for_unrelated_input() {
        // A wholly-unrelated value yields no hint (so the message has no
        // misleading suggestion).
        assert!(near_miss_hint("totally-bogus", VALID).is_none());
    }
}

/// `deploy init` name handling for the targets that do not go through
/// `InitCommand` (debug session `cargo-pmcp-deploy-targets`, finding #3).
#[cfg(test)]
mod init_server_name_tests {
    use super::*;

    fn project(package: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("Cargo.toml");
        tmp
    }

    fn write_deploy_toml(root: &Path, text: &str) {
        std::fs::create_dir_all(root.join(".pmcp")).expect("mkdir .pmcp");
        std::fs::write(root.join(".pmcp/deploy.toml"), text).expect("write deploy.toml");
    }

    const CLOUD_RUN_TOML: &str = "\
[target]
type = \"google-cloud-run\"
version = \"1.0.0\"

[gcp]
project_id = \"acme-prod\"
region = \"europe-west2\"

[server]
name = \"acme-forecast\"
binary = \"serve\"

[environment]
RUST_LOG = \"info\"
";

    /// #3 on Cloud Run: a NEW deploy.toml defaults the name to the package
    /// name with one trailing `-lambda` removed.
    #[test]
    fn cloud_run_fresh_init_strips_a_trailing_lambda() {
        let tmp = project("forecast-coach-lambda");
        let config = cloud_run_init_config(tmp.path(), "us-central1", None).expect("config");
        assert_eq!(config.server.name, "forecast-coach");
    }

    #[test]
    fn cloud_run_fresh_init_with_name_uses_it() {
        let tmp = project("forecast-coach-lambda");
        let config =
            cloud_run_init_config(tmp.path(), "us-central1", Some("acme")).expect("config");
        assert_eq!(config.server.name, "acme");
    }

    /// `--name` on an existing Cloud Run deploy.toml renames the server in
    /// the file (Cloud Run init otherwise never rewrites an existing file)
    /// and keeps everything else.
    #[test]
    fn cloud_run_reinit_with_name_renames_in_place() {
        let tmp = project("forecast-coach-lambda");
        write_deploy_toml(tmp.path(), CLOUD_RUN_TOML);

        let config =
            cloud_run_init_config(tmp.path(), "us-central1", Some("acme")).expect("config");

        assert_eq!(config.server.name, "acme");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read"),
            CLOUD_RUN_TOML.replace("name = \"acme-forecast\"", "name = \"acme\"")
        );
    }

    #[test]
    fn cloud_run_reinit_without_name_keeps_the_file() {
        let tmp = project("forecast-coach-lambda");
        write_deploy_toml(tmp.path(), CLOUD_RUN_TOML);
        let config = cloud_run_init_config(tmp.path(), "us-central1", None).expect("config");
        assert_eq!(config.server.name, "acme-forecast");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read"),
            CLOUD_RUN_TOML
        );
    }

    /// pmcp-run / cloudflare: the package default for a new deploy.toml, an
    /// existing deploy.toml's name otherwise, and `--name` renames it there.
    #[test]
    fn other_targets_prefer_explicit_then_existing_then_default() {
        let tmp = project("forecast-coach-lambda");
        assert_eq!(
            init_server_name(tmp.path(), None).expect("default"),
            "forecast-coach"
        );
        assert_eq!(
            init_server_name(tmp.path(), Some("acme")).expect("explicit"),
            "acme"
        );
        write_deploy_toml(tmp.path(), CLOUD_RUN_TOML);
        assert_eq!(
            init_server_name(tmp.path(), None).expect("kept"),
            "acme-forecast"
        );
        assert_eq!(
            init_server_name(tmp.path(), Some("acme")).expect("explicit"),
            "acme"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read"),
            CLOUD_RUN_TOML.replace("name = \"acme-forecast\"", "name = \"acme\""),
            "--name renames the kept file in place, nothing else"
        );
    }

    /// A kept deploy.toml whose name the `--name` rule would reject (an
    /// underscore) is still kept: validation applies to `--name` only.
    #[test]
    fn an_existing_name_is_never_validated() {
        let tmp = project("forecast-coach-lambda");
        write_deploy_toml(
            tmp.path(),
            &CLOUD_RUN_TOML.replace("name = \"acme-forecast\"", "name = \"acme_forecast\""),
        );
        assert_eq!(
            init_server_name(tmp.path(), None).expect("kept"),
            "acme_forecast"
        );
    }
}

/// `deploy init --target-type google-cloud-run` from parsed CLI flags (debug
/// session `cargo-pmcp-deploy-targets`, findings #10 and A3).
///
/// These tests set and restore `AWS_REGION`; cargo-pmcp's bin tests run with
/// `--test-threads=1`.
#[cfg(test)]
mod cloud_run_init_tests {
    use super::*;

    struct EnvGuard {
        key: &'static str,
        saved: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let saved = std::env::var_os(key);
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
            Self { key, saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.saved.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// `(region, name)` of `cargo pmcp deploy init <args>`, as clap parses it.
    fn init_flags(args: &[&str]) -> (Option<String>, Option<String>) {
        let command_line = ["deploy", "init"].iter().chain(args.iter());
        let command = DeployCommand::try_parse_from(command_line).expect("parses");
        match command.action {
            Some(DeployAction::Init { region, name, .. }) => (region, name),
            other => panic!("not init: {other:?}"),
        }
    }

    fn project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        crate::deployment::targets::google_cloud_run::fixture::write_x_lambda(tmp.path());
        tmp
    }

    fn cloud_run_config_from_cli(root: &Path, args: &[&str]) -> crate::deployment::DeployConfig {
        let (region, name) = init_flags(args);
        cloud_run_init_from_flags(root, region.as_deref(), name.as_deref()).expect("config")
    }

    /// #10: without `--region`, a new Cloud Run deploy.toml gets
    /// `us-central1`, never the AWS default `us-east-1`.
    #[test]
    fn cloud_run_init_defaults_the_gcp_region_to_us_central1() {
        let _env = EnvGuard::set("AWS_REGION", None);
        let tmp = project();
        let config = cloud_run_config_from_cli(tmp.path(), &["--target-type", "google-cloud-run"]);
        assert_eq!(config.gcp.as_ref().expect("[gcp]").region, "us-central1");
        assert!(config.aws.is_none());
    }

    /// #10: the operator's `AWS_REGION` is an AWS region; it never becomes
    /// the `[gcp] region`.
    #[test]
    fn aws_region_in_the_environment_does_not_leak_into_gcp() {
        let _env = EnvGuard::set("AWS_REGION", Some("eu-west-3"));
        let tmp = project();
        let config = cloud_run_config_from_cli(tmp.path(), &["--target-type", "google-cloud-run"]);
        assert_eq!(config.gcp.as_ref().expect("[gcp]").region, "us-central1");
    }

    #[test]
    fn an_explicit_region_is_the_gcp_region() {
        let _env = EnvGuard::set("AWS_REGION", Some("eu-west-3"));
        let tmp = project();
        let config = cloud_run_config_from_cli(
            tmp.path(),
            &[
                "--target-type",
                "google-cloud-run",
                "--region",
                "europe-west2",
            ],
        );
        assert_eq!(config.gcp.as_ref().expect("[gcp]").region, "europe-west2");
    }

    /// The AWS targets keep the old resolution: `--region`, else
    /// `$AWS_REGION`, else `us-east-1`.
    #[test]
    fn aws_targets_keep_flag_then_aws_region_then_us_east_1() {
        for target in [
            "aws-lambda",
            "pmcp-run",
            "azure-container-apps",
            "cloudflare-workers",
        ] {
            {
                let _env = EnvGuard::set("AWS_REGION", None);
                assert_eq!(default_init_region(target, None), "us-east-1", "{target}");
            }
            {
                let _env = EnvGuard::set("AWS_REGION", Some("eu-west-3"));
                assert_eq!(default_init_region(target, None), "eu-west-3", "{target}");
                assert_eq!(
                    default_init_region(target, Some("ap-south-1")),
                    "ap-south-1",
                    "{target}"
                );
            }
        }
    }

    /// clap no longer fills `--region` from `AWS_REGION` or a default, so the
    /// dispatch can tell "not given" from "given".
    #[test]
    fn the_region_flag_is_never_filled_in_by_clap() {
        let _env = EnvGuard::set("AWS_REGION", Some("eu-west-3"));
        assert_eq!(init_flags(&[]).0, None);
        assert_eq!(
            init_flags(&["--region", "us-west-2"]).0.as_deref(),
            Some("us-west-2")
        );
    }

    /// A3: the Cloud Run init path IS `default_config_for_target`'s Cloud Run
    /// arm (the arm used to be unreachable, and differed: empty project id).
    #[test]
    fn cloud_run_init_uses_the_cloud_run_defaults() {
        let _env = EnvGuard::set("AWS_REGION", None);
        let tmp = project();
        let from_cli = cloud_run_config_from_cli(
            tmp.path(),
            &["--target-type", "google-cloud-run", "--name", "acme"],
        );
        let defaults = default_config_for_target(
            "google-cloud-run",
            "acme".to_string(),
            "us-central1".to_string(),
            tmp.path().to_path_buf(),
        );
        assert_eq!(
            toml::to_string(&from_cli).expect("ser"),
            toml::to_string(&defaults).expect("ser")
        );
        let gcp = defaults.gcp.expect("[gcp]");
        assert_eq!(gcp.project_id, "your-gcp-project-id");
        assert_eq!(gcp.region, "us-central1");
    }
}
