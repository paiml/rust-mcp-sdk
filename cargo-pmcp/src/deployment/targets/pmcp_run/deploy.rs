use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::deployment::{
    metadata::McpMetadata,
    r#trait::{BuildArtifact, DeploymentOutputs},
    stack_routing::{
        cloudformation_metadata_from, custom_stack_ts_reason, emit_descriptor_warnings,
        extract_metadata_with_log, load_deploy_descriptor, mark_custom_stack,
    },
    DeployConfig,
};

use super::{auth, graphql};

/// Extract the server version from the Cargo workspace.
///
/// Uses `cargo metadata` which properly handles:
/// 1. Workspace root versions
/// 2. Package versions
/// 3. Workspace inheritance (`version.workspace = true`)
///
/// Returns None if version cannot be determined.
fn extract_version_from_cargo(project_root: &Path) -> Option<String> {
    let metadata = run_cargo_metadata(project_root)?;
    let workspace_root = metadata.get("workspace_root")?.as_str()?;
    let packages = metadata.get("packages")?.as_array()?;
    select_best_version(packages, workspace_root)
}

/// Invoke `cargo metadata` and parse stdout into a JSON Value. Returns None
/// on any failure (process spawn, non-zero status, invalid JSON).
fn run_cargo_metadata(project_root: &Path) -> Option<serde_json::Value> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

/// From a list of cargo-metadata package entries, prefer the workspace-root
/// package's version; fallback to the first package with a version field.
fn select_best_version(packages: &[serde_json::Value], workspace_root: &str) -> Option<String> {
    let mut root_package_version: Option<String> = None;
    let mut any_version: Option<String> = None;

    for package in packages {
        let Some(version) = package.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        let manifest_path = package
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if is_workspace_root_manifest(manifest_path, workspace_root) {
            root_package_version = Some(version.to_string());
        }

        if any_version.is_none() {
            any_version = Some(version.to_string());
        }
    }

    root_package_version.or(any_version)
}

/// Return true when `manifest_path` is the workspace-root Cargo.toml
/// (single-path-component suffix relative to workspace_root).
fn is_workspace_root_manifest(manifest_path: &str, workspace_root: &str) -> bool {
    if manifest_path.is_empty() || !manifest_path.starts_with(workspace_root) {
        return false;
    }
    let relative = &manifest_path[workspace_root.len()..];
    relative == "/Cargo.toml" || relative.matches('/').count() == 1
}

/// Deploy to pmcp.run managed service using 3-step flow:
/// 1. Get presigned S3 URLs
/// 2. Upload files directly to S3
/// 3. Create deployment from S3 files
pub async fn deploy_to_pmcp_run(
    config: &DeployConfig,
    artifact: BuildArtifact,
) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to pmcp.run...");
    println!();

    // Fail-closed IAM gate + stack.ts regeneration. Mirrors the aws-lambda
    // path (commands/deploy/deploy.rs) so the operator-declared `[iam]`
    // contract is identical across targets. Must run before any network call.
    validate_and_regenerate_stack_ts(config)?;

    // Get credentials (OAuth tokens)
    let credentials = auth::get_credentials().await?;

    // Step 0: Extract MCP metadata for the CloudFormation template, then apply
    // the operator `[metadata]` override (DSTK-02/DSTK-03) so config-declared
    // server_type / snapshot_baked reach the synth context.
    let metadata = extract_metadata_with_log(&config.project_root).map(|mut m| {
        m.apply_config_overrides(&config.metadata);
        m
    });

    // Step 1: Synthesize the CloudFormation template. Task 7 (CFN-renderer
    // extraction) routes between the pure `pmcp-cfn-renderer` crate and the
    // legacy `npx cdk synth` subprocess — see `synth_template`'s doc comment
    // for the routing rule. `[environment]` (never `[secrets]`) reaches
    // either path; see `synth_template`/`run_legacy_synth`.
    println!("📝 Synthesizing CloudFormation template...");
    let synth = synth_template(config, metadata.as_ref())?;
    match &synth.path {
        SynthPath::Renderer => {
            println!("✅ CloudFormation template rendered (pmcp-cfn-renderer)");
        },
        SynthPath::LegacyCdk { .. } => {
            println!("✅ CloudFormation template synthesized (cdk synth)");
        },
    }

    // Step 3: Extract bootstrap data + content-type from the build artifact.
    let upload = read_bootstrap_upload(artifact)?;
    println!();

    // Step 4: apply every post-synth template merge (see
    // `apply_post_synth_merges`), then upload.
    let template = apply_post_synth_merges(synth.template_json, config)?;

    log_upload_sizes(template.len(), upload.data.len(), upload.has_assets);
    println!();

    // Step 5: Get presigned S3 URLs from GraphQL
    println!("🔑 Getting upload URLs from pmcp.run...");
    let urls = graphql::get_upload_urls(
        &credentials.access_token,
        &config.server.name,
        template.len(),
        upload.data.len(),
    )
    .await
    .context("Failed to get upload URLs")?;
    println!("   URLs expire in {} seconds", urls.expires_in);
    println!();

    // Step 6: Upload files to S3 in parallel
    upload_template_and_bootstrap(&urls, template.into_bytes(), upload).await?;

    // Step 7: Create deployment via GraphQL with composition settings and version
    println!("🚀 Creating deployment...");
    let deployment =
        create_deployment_with_composition(&credentials.access_token, &urls, config).await?;
    println!("   Deployment ID: {}", deployment.deployment_id);
    println!();

    // Step 8: Poll deployment status (wait for completion)
    let deployment_outputs =
        poll_deployment_status(&credentials.access_token, &deployment.deployment_id)
            .await
            .context("Deployment failed")?;

    // Step 9: Configure OAuth (explicit config or backend-registered)
    let oauth_config =
        resolve_oauth_for_deployment(&credentials.access_token, config, &deployment).await;

    // Step 10: Build URLs + print summary + assemble outputs
    let mcp_url = compute_mcp_url(&deployment_outputs, &deployment.deployment_id);
    let health_url = compute_health_url(&mcp_url);
    let server_id = deployment_outputs
        .custom
        .get("project_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&config.server.name);

    print_deployment_summary(
        &config.server.name,
        server_id,
        &deployment.deployment_id,
        &mcp_url,
        &health_url,
        oauth_config.as_ref(),
    );

    Ok(build_deployment_outputs(
        &mcp_url,
        &health_url,
        server_id,
        &deployment.deployment_id,
        oauth_config,
    ))
}

// ============================================================================
// Task 7 (CFN-renderer extraction): synth routing between the pure
// `pmcp-cfn-renderer` crate and the legacy `npx cdk synth` subprocess.
// ============================================================================

/// Outcome of synthesizing the pmcp-run CloudFormation template.
///
/// `path` records which code path produced `template_json` — Task 10's
/// runbook and the `mcp:customStack` taint recording (see
/// [`mark_custom_stack`]) both key off it.
pub(crate) struct SynthOutput {
    pub(crate) template_json: String,
    pub(crate) path: SynthPath,
}

/// Which code path produced a [`SynthOutput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SynthPath {
    /// The pure `pmcp-cfn-renderer` crate rendered the template directly
    /// from `.pmcp/deploy.toml`'s `DeployDescriptor` — no `cdk synth`
    /// subprocess, no Node.js.
    Renderer,
    /// Fell back to `npx cdk synth`. `reason` names why: a hand-modified
    /// `deploy/lib/stack.ts`, a `.pmcp/deploy.toml` the renderer's
    /// closed-set `DeployDescriptor` can't parse yet, or a declared section
    /// the renderer doesn't implement for this target yet.
    LegacyCdk { reason: String },
}

/// Synthesize the pmcp-run CloudFormation template, routing between the pure
/// [`pmcp_cfn_renderer`] renderer and the legacy `npx cdk synth` subprocess.
///
/// # Routing rule
///
/// `deploy/lib/stack.ts` on disk (post `validate_and_regenerate_stack_ts`,
/// which already ran by the time this is called) must byte-match what
/// `cargo pmcp` itself would (re)generate for the renderer path to even be
/// attempted — see [`custom_stack_ts_reason`]. A hand-modified stack.ts
/// always falls back to `cdk synth` so operator customizations keep
/// working, and is additionally tainted via [`mark_custom_stack`] so the
/// platform can tell the two shapes apart from the synthesized template's
/// own `mcp:*` metadata.
///
/// Even on the untainted (scaffold) path the renderer attempt can still
/// fall back gracefully — never a hard error — because two things are still
/// growing: `.pmcp/deploy.toml` may declare a table outside the renderer's
/// closed-set `DeployDescriptor` (e.g. `[aws].account_id`, which
/// `AwsSection` does not model), or a section the renderer doesn't
/// implement for this target yet (e.g. pmcp-run `[auth].enabled = true` —
/// the platform's own OAuth registration path is unaffected either way,
/// since it never rendered into CFN in the legacy path). See [`try_render`].
fn synth_template(config: &DeployConfig, metadata: Option<&McpMetadata>) -> Result<SynthOutput> {
    let deploy_dir = config.project_root.join("deploy");
    let cdk_out = deploy_dir.join("cdk.out");

    if let Some(reason) = custom_stack_ts_reason(config)? {
        warn_falling_back_to_cdk(&reason);
        let tainted = mark_custom_stack(metadata);
        return run_legacy_synth(&deploy_dir, &cdk_out, tainted.as_ref(), config, reason);
    }

    match try_render(config, metadata) {
        Ok(template_json) => Ok(SynthOutput {
            template_json,
            path: SynthPath::Renderer,
        }),
        Err(reason) => {
            warn_falling_back_to_cdk(&reason);
            run_legacy_synth(&deploy_dir, &cdk_out, metadata, config, reason)
        },
    }
}

/// Print the standard "falling back to cdk synth" advisory, in the same
/// yellow `warning:` style as `crate::deployment::iam::emit_warnings`.
fn warn_falling_back_to_cdk(reason: &str) {
    eprintln!(
        "  {} {reason} — falling back to `cdk synth` for this deploy.",
        console::style("warning:").yellow()
    );
}

/// Attempt the pure-renderer synth path.
///
/// Parses `.pmcp/deploy.toml` as a [`DeployDescriptor`], surfaces the
/// renderer's own `iam`/`cognito` advisory warnings directly (closing the
/// T4/T6 review gap where `pmcp_cfn_renderer::render` discards them — see
/// `crate::deployment::iam::emit_warnings` for the print style this
/// mirrors), then renders. `Err` carries a human-readable reason for the
/// caller to fall back to `cdk synth` on — never a hard failure, since both
/// the descriptor's closed set and the renderer's resource-family surface
/// are still growing (see `synth_template`'s doc comment).
fn try_render(config: &DeployConfig, metadata: Option<&McpMetadata>) -> Result<String, String> {
    let descriptor = load_deploy_descriptor(config).map_err(|e| {
        format!(
            "{} does not parse as pmcp-cfn-renderer's DeployDescriptor: {e:#}",
            ".pmcp/deploy.toml"
        )
    })?;

    emit_descriptor_warnings(&descriptor);

    let params = build_render_params(config, metadata);

    pmcp_cfn_renderer::render(&descriptor, &params)
        .map(|template| template.to_canonical_json())
        .map_err(|e| format!("pmcp-cfn-renderer cannot render this descriptor yet: {e}"))
}

/// Sentinel used for [`pmcp_cfn_renderer::RenderParams::account_id`] when
/// the account is not resolvable in this flow.
///
/// Deliberately NOT a plausible-looking fake (unlike the renderer's own
/// golden fixtures, which use AWS's docs placeholder `123456789012`):
/// investigation for Task 7 found that the pmcp-run `cdk synth` path never
/// sets `CDK_DEFAULT_ACCOUNT` either (only the aws-lambda `cdk deploy` path
/// does, from `[aws].account_id` — see `commands/deploy/deploy.rs`), so
/// `this.account` in the generated `stack.ts` resolves to CloudFormation's
/// own `Ref: AWS::AccountId` pseudo-parameter — resolved server-side, in
/// whatever account actually applies the stack. `pmcp-cfn-renderer` has no
/// equivalent of a CFN pseudo-parameter: `RenderParams::account_id` is
/// baked as a literal into IAM/ARN strings. This all-zeros sentinel stands
/// in until the platform side of that gap is resolved (see Task 7's report
/// / Task 10's runbook) — an operator CAN unblock it today by declaring
/// `[aws] account_id = "..."`, which this function reads first.
const UNRESOLVED_ACCOUNT_ID: &str = "000000000000";

/// Placeholder S3 bucket for the renderer's `ArtifactRef` (Task 7,
/// Interfaces §2).
///
/// The real upload key is only known after `graphql::get_upload_urls` runs
/// (Step 5, strictly AFTER synth) — today's `cdk synth` leaves the same kind
/// of synth-time placeholder in `Code.S3Bucket`/`Code.S3Key` (its local CDK
/// asset-staging location), and the platform never deploys straight from
/// either value: it deploys the bootstrap ZIP it receives at
/// `bootstrapS3Key` instead (see `graphql::UploadUrls`). Kept as a distinct
/// named constant (rather than reusing the account sentinel) so the two
/// "unknown at synth time" gaps stay independently greppable.
const ARTIFACT_PLACEHOLDER_BUCKET: &str = "pmcp-run-pending-upload";

/// Build [`pmcp_cfn_renderer::RenderParams`] from the existing config/
/// credential plumbing already threaded through `deploy.rs` — never from the
/// `DeployDescriptor` (Task 7, Interfaces §2's identity/environment split).
fn build_render_params(
    config: &DeployConfig,
    metadata: Option<&McpMetadata>,
) -> pmcp_cfn_renderer::RenderParams {
    let aws = config.aws();
    pmcp_cfn_renderer::RenderParams {
        account_id: aws
            .account_id
            .clone()
            .unwrap_or_else(|| UNRESOLVED_ACCOUNT_ID.to_string()),
        region: aws.region.clone(),
        stack_name: format!("{}-stack", config.server.name),
        artifact: pmcp_cfn_renderer::ArtifactRef {
            s3_bucket: ARTIFACT_PLACEHOLDER_BUCKET.to_string(),
            s3_key: format!("{}/bootstrap.zip", config.server.name),
            digest: None,
        },
        environment: config
            .environment
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        cloudformation_metadata: cloudformation_metadata_from(metadata),
        // The `pmcp-run` target never routes through `aws_lambda::artifact`'s
        // `ServerShape` detection (that module is `aws-lambda`-target-only —
        // see its own module doc), so a `pmcp-run` deploy never needs the
        // AWS Lambda Web Adapter bridge (T8 review fix). `None` here renders
        // byte-identical output to before that field existed.
        runtime_adapter: None,
    }
}

/// Run the legacy `npx cdk synth` subprocess and read back the synthesized
/// template — the pre-Task-7 Step 1/Step 2 body, now a fallback path.
fn run_legacy_synth(
    deploy_dir: &Path,
    cdk_out: &Path,
    metadata: Option<&McpMetadata>,
    config: &DeployConfig,
    reason: String,
) -> Result<SynthOutput> {
    run_cdk_synth(deploy_dir, metadata, &config.environment)?;
    let template_path = find_template_file(cdk_out)?;
    println!("   Template: {}", template_path.display());
    let template_json = std::fs::read_to_string(&template_path)
        .context("Failed to read CloudFormation template")?;
    Ok(SynthOutput {
        template_json,
        path: SynthPath::LegacyCdk { reason },
    })
}

/// Run `npx cdk synth --quiet` with optional metadata context args.
///
/// `environment` carries developer-declared `[environment]` values from
/// `.pmcp/deploy.toml`; they are set as process env vars on the `cdk synth`
/// child process so the stack.ts can consume matching `process.env.<KEY>`
/// reads (FIX #2, `deploy-toml-inert-for-preserved-stack`). This is the
/// pmcp-run equivalent of the aws-lambda `DeployExecutor.extra_env` path.
fn run_cdk_synth(
    deploy_dir: &Path,
    metadata: Option<&McpMetadata>,
    environment: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let synth_output = build_cdk_synth_command(deploy_dir, metadata, environment)
        .output()
        .context("Failed to run cdk synth. Make sure Node.js and npm are installed")?;

    if !synth_output.status.success() {
        let stderr = String::from_utf8_lossy(&synth_output.stderr);
        bail!("CDK synthesis failed:\n{}", stderr);
    }
    Ok(())
}

/// Build (but do not run) the `npx cdk synth` child-process command, with the
/// developer-declared `[environment]` set as process env vars.
///
/// Factored out of [`run_cdk_synth`] so the env-var threading (FIX #2) is
/// unit-testable via [`std::process::Command::get_envs`] without spawning a
/// real `cdk synth`.
fn build_cdk_synth_command(
    deploy_dir: &Path,
    metadata: Option<&McpMetadata>,
    environment: &std::collections::HashMap<String, String>,
) -> std::process::Command {
    let shell_cmd = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let cdk_context_args = metadata
        .map(|m| m.to_cdk_context().join(" "))
        .unwrap_or_default();

    let synth_command = if cdk_context_args.is_empty() {
        "npx cdk synth --quiet".to_string()
    } else {
        format!("npx cdk synth --quiet {}", cdk_context_args)
    };

    let mut cmd = std::process::Command::new(shell_cmd);
    cmd.current_dir(deploy_dir)
        .envs(environment)
        .arg(shell_arg)
        .arg(&synth_command);
    cmd
}

/// Payload prepared for upload to S3: raw bytes, content-type, whether the
/// zip contains runtime assets (affects log label).
struct BootstrapUpload {
    data: Vec<u8>,
    content_type: &'static str,
    has_assets: bool,
}

/// Read the correct upload payload from a BuildArtifact — prefer the
/// deployment package zip if present, otherwise fall back to the raw binary.
fn read_bootstrap_upload(artifact: BuildArtifact) -> Result<BootstrapUpload> {
    let (bootstrap_path, deployment_package) = match artifact {
        BuildArtifact::Binary {
            path,
            deployment_package,
            ..
        }
        | BuildArtifact::Wasm {
            path,
            deployment_package,
            ..
        }
        | BuildArtifact::Custom {
            path,
            deployment_package,
            ..
        } => (path, deployment_package),
    };

    if let Some(ref package_path) = deployment_package {
        if package_path.exists() {
            println!("   📦 Using deployment package with assets");
            println!("   Package: {}", package_path.display());
            let data = std::fs::read(package_path).context("Failed to read deployment package")?;
            return Ok(BootstrapUpload {
                data,
                content_type: "application/zip",
                has_assets: true,
            });
        }
    }

    println!("   Bootstrap: {}", bootstrap_path.display());
    let data = std::fs::read(&bootstrap_path).with_context(|| {
        format!(
            "Bootstrap binary not found or unreadable: {}",
            bootstrap_path.display()
        )
    })?;
    Ok(BootstrapUpload {
        data,
        content_type: "application/octet-stream",
        has_assets: false,
    })
}

/// Log KB sizes for the template + the (bootstrap or package) payload.
fn log_upload_sizes(template_len: usize, upload_len: usize, has_assets: bool) {
    println!("📦 Template size: {} KB", template_len / 1024);
    if has_assets {
        println!("📦 Deployment package size: {} KB", upload_len / 1024);
    } else {
        println!("📦 Bootstrap size: {} KB", upload_len / 1024);
    }
}

/// Upload template + bootstrap to their presigned S3 URLs in parallel.
async fn upload_template_and_bootstrap(
    urls: &graphql::UploadUrls,
    template_bytes: Vec<u8>,
    upload: BootstrapUpload,
) -> Result<()> {
    println!("⬆️  Uploading files to S3...");

    let bootstrap_label = if upload.has_assets {
        "Package"
    } else {
        "Bootstrap"
    };
    let (template_result, bootstrap_result) = tokio::join!(
        graphql::upload_to_s3(
            &urls.template_upload_url,
            template_bytes,
            "application/json",
            "Template",
        ),
        graphql::upload_to_s3(
            &urls.bootstrap_upload_url,
            upload.data,
            upload.content_type,
            bootstrap_label,
        )
    );

    template_result.context("Template upload to S3 failed")?;
    bootstrap_result.context("Bootstrap upload to S3 failed")?;

    println!("✅ Files uploaded successfully to S3");
    println!();
    Ok(())
}

/// Extract version + build composition settings and invoke graphql to create
/// the deployment record.
async fn create_deployment_with_composition(
    access_token: &str,
    urls: &graphql::UploadUrls,
    config: &DeployConfig,
) -> Result<graphql::DeploymentInfo> {
    let server_version = extract_version_from_cargo(&config.project_root);
    if let Some(ref version) = server_version {
        println!("   Version: {}", version);
    }

    let composition = graphql::CompositionSettings {
        tier: config.composition.tier.clone(),
        allow_composition: config.composition.allow_composition,
        internal_only: config.composition.internal_only,
        description: config.composition.description.clone(),
        server_version,
    };
    graphql::create_deployment_from_s3_with_composition(
        access_token,
        urls,
        &config.server.name,
        composition,
    )
    .await
    .context("Failed to create deployment")
}

/// Determine OAuth configuration for the freshly-created deployment. If the
/// local config enables OAuth, call configure_server_oauth; otherwise check
/// backend state (may have been enabled in a prior session).
async fn resolve_oauth_for_deployment(
    access_token: &str,
    config: &DeployConfig,
    deployment: &graphql::DeploymentInfo,
) -> Option<graphql::OAuthConfig> {
    if config.auth.enabled {
        configure_new_oauth(access_token, config, &deployment.deployment_id).await
    } else {
        fetch_existing_oauth(access_token, &config.server.name).await
    }
}

/// Configure OAuth on a new deployment using local config's DCR settings.
async fn configure_new_oauth(
    access_token: &str,
    config: &DeployConfig,
    deployment_id: &str,
) -> Option<graphql::OAuthConfig> {
    println!("🔐 Configuring OAuth for MCP server...");

    let scopes = if config.auth.dcr.default_scopes.is_empty() {
        None
    } else {
        Some(config.auth.dcr.default_scopes.clone())
    };

    let public_patterns = if config.auth.dcr.public_client_patterns.is_empty() {
        None
    } else {
        Some(config.auth.dcr.public_client_patterns.clone())
    };

    match graphql::configure_server_oauth(
        access_token,
        deployment_id,
        true,
        scopes,
        Some(config.auth.dcr.enabled),
        public_patterns,
        None, // shared_pool_name - not supported in local config yet
    )
    .await
    {
        Ok(oauth) => {
            println!("✅ OAuth configured successfully");
            println!();
            Some(oauth)
        },
        Err(e) => {
            eprintln!("⚠️  Failed to configure OAuth: {}", e);
            eprintln!("   You can manually enable OAuth with:");
            eprintln!("   cargo pmcp oauth enable --server {}", deployment_id);
            println!();
            None
        },
    }
}

/// Backend OAuth state check for a server not enabling OAuth in local config.
async fn fetch_existing_oauth(
    access_token: &str,
    server_name: &str,
) -> Option<graphql::OAuthConfig> {
    match graphql::fetch_server_oauth_endpoints(access_token, server_name).await {
        Ok(oauth) => {
            if oauth.oauth_enabled {
                Some(graphql::OAuthConfig {
                    server_id: oauth.server_id,
                    oauth_enabled: oauth.oauth_enabled,
                    user_pool_id: oauth.user_pool_id,
                    user_pool_region: oauth.user_pool_region,
                    discovery_url: oauth.discovery_url,
                    registration_endpoint: oauth.registration_endpoint,
                    authorization_endpoint: oauth.authorization_endpoint,
                    token_endpoint: oauth.token_endpoint,
                })
            } else {
                eprintln!(
                    "   (OAuth query returned oauthEnabled=false for {})",
                    server_name
                );
                None
            }
        },
        Err(e) => {
            eprintln!("   (OAuth status check failed for {}: {})", server_name, e);
            None
        },
    }
}

/// Resolve the MCP endpoint URL: backend-provided, with fallback to constructing
/// from deployment ID.
fn compute_mcp_url(deployment_outputs: &DeploymentOutputs, deployment_id: &str) -> String {
    deployment_outputs
        .url
        .clone()
        .unwrap_or_else(|| format!("https://api.pmcp.run/{}/mcp", deployment_id))
}

/// Derive the health-check URL from the MCP URL (replace trailing /mcp,
/// not /mcp- in subdomains).
fn compute_health_url(mcp_url: &str) -> String {
    if let Some(base) = mcp_url.strip_suffix("/mcp") {
        format!("{}/health", base)
    } else {
        mcp_url.replace("/mcp", "/health")
    }
}

/// Print the final human-readable "deployment successful" summary with
/// OAuth-aware branching (endpoint labels + auth hints).
fn print_deployment_summary(
    server_name: &str,
    server_id: &str,
    deployment_id: &str,
    mcp_url: &str,
    health_url: &str,
    oauth_config: Option<&graphql::OAuthConfig>,
) {
    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Name: {}", server_name);
    println!("   Server ID: {}", server_id);
    println!("   Deployment ID: {}", deployment_id);

    if let Some(oauth) = oauth_config {
        print_oauth_endpoint_block(mcp_url, health_url, oauth);
    } else {
        print_open_endpoint_block(mcp_url, health_url, deployment_id);
    }

    println!();
    println!("💡 Next steps:");
    println!("   • View logs: cargo pmcp deploy logs --target pmcp-run");
    println!("   • Test deployment: cargo pmcp deploy test --target pmcp-run");
    println!("   • View dashboard: https://pmcp.run/dashboard");
    println!();
}

/// Print the OAuth-protected endpoint block.
fn print_oauth_endpoint_block(mcp_url: &str, health_url: &str, oauth: &graphql::OAuthConfig) {
    println!();
    println!("🔐 MCP Endpoint (OAuth Protected):");
    println!("   URL: {}", mcp_url);
    println!();
    println!("🔑 OAuth Configuration:");
    if let Some(ref discovery) = oauth.discovery_url {
        println!("   Discovery:     {}", discovery);
    }
    if let Some(ref register) = oauth.registration_endpoint {
        println!("   Registration:  {}", register);
    }
    if let Some(ref authorize) = oauth.authorization_endpoint {
        println!("   Authorization: {}", authorize);
    }
    if let Some(ref token) = oauth.token_endpoint {
        println!("   Token:         {}", token);
    }
    println!();
    println!("🏥 Health Check:");
    println!("   URL: {}", health_url);
    println!();
    println!("Clients must authenticate via OAuth to access this server.");
}

/// Print the open-access endpoint block + enable-OAuth hint.
fn print_open_endpoint_block(mcp_url: &str, health_url: &str, deployment_id: &str) {
    println!();
    println!("🔌 MCP Endpoint:");
    println!("   URL: {}", mcp_url);
    println!();
    println!("🏥 Health Check:");
    println!("   URL: {}", health_url);
    println!();
    println!("No authentication required. Anyone can access this server.");
    println!("To enable OAuth: cargo pmcp oauth enable {}", deployment_id);
}

/// Assemble the final `DeploymentOutputs` record with custom fields populated
/// for downstream save_deployment_info (server_id, deployment_id, endpoints,
/// OAuth metadata).
fn build_deployment_outputs(
    mcp_url: &str,
    health_url: &str,
    server_id: &str,
    deployment_id: &str,
    oauth_config: Option<graphql::OAuthConfig>,
) -> DeploymentOutputs {
    let mut outputs = DeploymentOutputs {
        url: Some(mcp_url.to_string()),
        additional_urls: vec![health_url.to_string()],
        regions: vec![],
        stack_name: None,
        version: None,
        custom: std::collections::HashMap::new(),
    };

    outputs.custom.insert(
        "server_id".to_string(),
        serde_json::Value::String(server_id.to_string()),
    );
    outputs.custom.insert(
        "deployment_id".to_string(),
        serde_json::Value::String(deployment_id.to_string()),
    );
    outputs.custom.insert(
        "mcp_endpoint".to_string(),
        serde_json::Value::String(mcp_url.to_string()),
    );
    outputs.custom.insert(
        "health_endpoint".to_string(),
        serde_json::Value::String(health_url.to_string()),
    );

    insert_oauth_fields(&mut outputs.custom, oauth_config);
    outputs
}

/// Insert OAuth-related custom fields (or the `oauth_enabled=false` flag).
fn insert_oauth_fields(
    custom: &mut std::collections::HashMap<String, serde_json::Value>,
    oauth_config: Option<graphql::OAuthConfig>,
) {
    match oauth_config {
        Some(oauth) => {
            custom.insert(
                "oauth_enabled".to_string(),
                serde_json::Value::Bool(oauth.oauth_enabled),
            );
            if let Some(discovery) = oauth.discovery_url {
                custom.insert(
                    "oauth_discovery_url".to_string(),
                    serde_json::Value::String(discovery),
                );
            }
            if let Some(pool_id) = oauth.user_pool_id {
                custom.insert(
                    "cognito_user_pool_id".to_string(),
                    serde_json::Value::String(pool_id),
                );
            }
        },
        None => {
            custom.insert("oauth_enabled".to_string(), serde_json::Value::Bool(false));
        },
    }
}

/// Poll deployment status until complete or failed
async fn poll_deployment_status(
    access_token: &str,
    deployment_id: &str,
) -> Result<DeploymentOutputs> {
    println!("⏳ Waiting for deployment to complete...");

    let mut dots = 0;

    loop {
        let status = graphql::get_deployment(access_token, deployment_id).await?;

        match status.status.as_str() {
            "pending" | "validating" | "deploying" => {
                print!(".");
                dots += 1;
                if dots >= 60 {
                    println!();
                    dots = 0;
                }
                std::io::Write::flush(&mut std::io::stdout())?;
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            "success" => {
                if dots > 0 {
                    println!();
                }
                println!("✅ Deployment completed successfully!");

                // Debug: Log the URL from server response
                if let Some(ref url) = status.url {
                    println!("   Server URL: {}", url);
                } else {
                    println!("   ⚠️  Server did not return URL");
                }
                println!();

                // Include project_name in outputs for use by save_deployment_info
                let mut custom = std::collections::HashMap::new();
                custom.insert(
                    "project_name".to_string(),
                    serde_json::Value::String(status.project_name),
                );

                return Ok(DeploymentOutputs {
                    url: status.url,
                    additional_urls: vec![],
                    regions: vec![],
                    stack_name: None,
                    version: None,
                    custom,
                });
            },
            "failed" => {
                if dots > 0 {
                    println!();
                }
                bail!(
                    "Deployment failed: {}",
                    status
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
            },
            _ => {
                bail!("Unknown deployment status: {}", status.status);
            },
        }
    }
}

/// Find the CloudFormation template file in cdk.out directory
fn find_template_file(cdk_out: &Path) -> Result<PathBuf> {
    let entries = std::fs::read_dir(cdk_out).with_context(|| {
        format!(
            "CDK output directory not found or unreadable: {}",
            cdk_out.display()
        )
    })?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.ends_with(".template.json") {
                    return Ok(path);
                }
            }
        }
    }

    bail!("No CloudFormation template found in {}", cdk_out.display());
}

/// Apply every post-synth CloudFormation template merge, in order, to the
/// template `synth_template` produced.
///
/// This exists as its own function so the WIRING is unit-testable: the async
/// `deploy_to_pmcp_run` that calls it needs OAuth credentials, S3 presigned
/// URLs and a live GraphQL endpoint, so nothing can assert from there that a
/// merge is actually reached in production. Deleting a merge call inside this
/// function fails `post_synth_merges_apply_environment_and_sizing` — deleting
/// it from an inline chain inside `deploy_to_pmcp_run` would fail nothing.
///
/// # Why post-synth
///
/// Both merges run AFTER engine routing, so they cover `SynthPath::Renderer`
/// and `SynthPath::LegacyCdk` alike. That is the whole point: a hand-modified
/// `deploy/lib/stack.ts` always routes to `npx cdk synth` (see
/// [`custom_stack_ts_reason`]), and hand-edited stacks are exactly the
/// population that reported both of these bugs.
///
/// 1. `[environment]` — construct-agnostic env-var delivery into EVERY
///    `AWS::Lambda::Function`'s `Environment.Variables`
///    (`environment-inert-for-shared-cdk-constructs`). FIX #2 exported
///    `[environment]` only as `process.env` to the `cdk synth` child, so
///    shared/managed constructs that ignore `process.env` (e.g.
///    `OpenApiMcpServerStack`) silently dropped the declared keys. Secrets are
///    EXCLUDED (they keep their server-side injection path per D-08).
///    Precedence: `[environment]` OVERRIDES a construct's hardcoded value on
///    key collision (locked product decision).
/// 2. `[server]` sizing — `Properties.MemorySize`/`Timeout` on the MCP
///    function ONLY (debug session `deploy-server-memory-timeout`). Threading
///    these into the stack.ts template instead was measured and rejected; see
///    `init::AWS_LAMBDA_SCAFFOLD_MEMORY_MB`'s doc comment for the byte-match
///    fallout. Unlike (1) this one must NOT touch every Lambda: an
///    OAuth-enabled stack renders three functions at three different sizings,
///    and resizing the 10-second authorizer would be a regression.
fn apply_post_synth_merges(template: String, config: &DeployConfig) -> Result<String> {
    let template = apply_environment_merge(template, &config.environment, &config.secrets)?;
    apply_sizing_merge(
        template,
        &config.server.name,
        config.server.memory_mb,
        config.server.timeout_seconds,
    )
}

/// Outcome of merging `[environment]` into a synthesized CloudFormation
/// template. See [`merge_environment_into_template`].
#[derive(Debug)]
struct TemplateMergeOutcome {
    /// The re-serialized template JSON with `[environment]` applied.
    template: String,
    /// Sorted logical IDs of the `AWS::Lambda::Function` resources that were
    /// visited (and thus available to inject into). Empty means the template
    /// contained no Lambda function — the caller uses this for the fail-loud
    /// warning.
    lambdas_updated: Vec<String>,
}

/// Merge the synthesized template with the declared `[environment]` and emit
/// operator feedback.
///
/// Thin deploy-time wrapper around the pure [`merge_environment_into_template`]
/// helper: it computes the secret-key exclusion set, prints either a success
/// summary or the fail-loud "no Lambda resource" warning, and returns the
/// (possibly modified) template string. When `[environment]` is empty the
/// template is returned unchanged.
fn apply_environment_merge(
    template: String,
    environment: &HashMap<String, String>,
    secrets: &HashMap<String, String>,
) -> Result<String> {
    if environment.is_empty() {
        return Ok(template);
    }

    let secret_keys: HashSet<String> = secrets.keys().cloned().collect();
    let outcome = merge_environment_into_template(&template, environment, &secret_keys)?;

    if outcome.lambdas_updated.is_empty() {
        // FIX (fail-loud): `[environment]` was declared but the synthesized
        // template has no Lambda to inject into. Warn prominently instead of
        // silently dropping the keys.
        eprintln!(
            "{}",
            environment_no_lambda_warning(environment, &secret_keys)
        );
    } else {
        println!(
            "   ✅ Applied [environment] to {} Lambda function(s): {}",
            outcome.lambdas_updated.len(),
            outcome.lambdas_updated.join(", ")
        );
    }

    Ok(outcome.template)
}

/// Merge developer-declared `[environment]` values into every
/// `AWS::Lambda::Function` resource's `Properties.Environment.Variables` in a
/// synthesized CloudFormation template. Pure and unit-testable — no synth, no
/// I/O.
///
/// This is the construct-agnostic delivery mechanism for `[environment]`
/// (`environment-inert-for-shared-cdk-constructs`). FIX #2 passed
/// `[environment]` only as `process.env` to the `cdk synth` child, which lands
/// the keys only when the stack.ts explicitly reads `process.env.<KEY>`.
/// Shared/managed constructs hardcode their `environment: {}` and read no
/// arbitrary process env, so declared keys were silently dropped. Merging
/// directly into the post-synth template guarantees delivery regardless of how
/// the stack.ts was authored.
///
/// # Precedence
/// `environment` OVERRIDES a construct's hardcoded value on key collision
/// (e.g. `RUST_LOG=warn` beats a construct default of `info`) — a locked
/// product decision. `secret_keys` are EXCLUDED from the merge entirely:
/// secrets keep their existing server-side injection path and never appear in
/// the template.
///
/// # Returns
/// A [`TemplateMergeOutcome`] carrying the re-serialized template JSON plus the
/// sorted logical IDs of the Lambda resources visited. An empty
/// `lambdas_updated` means no Lambda resource was present (fail-loud signal).
fn merge_environment_into_template(
    template_json: &str,
    environment: &HashMap<String, String>,
    secret_keys: &HashSet<String>,
) -> Result<TemplateMergeOutcome> {
    let mut template: serde_json::Value = serde_json::from_str(template_json)
        .context("Failed to parse synthesized CloudFormation template JSON")?;

    // Effective merge set = declared `[environment]` minus any secret keys.
    let effective: Vec<(String, String)> = environment
        .iter()
        .filter(|(k, _)| !secret_keys.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut lambdas_updated: Vec<String> = Vec::new();

    if let Some(resources) = template
        .get_mut("Resources")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (logical_id, resource) in resources.iter_mut() {
            if !is_lambda_function(resource) {
                continue;
            }
            apply_env_to_lambda(resource, &effective);
            lambdas_updated.push(logical_id.clone());
        }
    }

    lambdas_updated.sort();

    let merged = serde_json::to_string_pretty(&template)
        .context("Failed to re-serialize merged CloudFormation template")?;

    Ok(TemplateMergeOutcome {
        template: merged,
        lambdas_updated,
    })
}

/// True when `resource` has `Type == "AWS::Lambda::Function"`.
fn is_lambda_function(resource: &serde_json::Value) -> bool {
    resource.get("Type").and_then(serde_json::Value::as_str) == Some("AWS::Lambda::Function")
}

/// Insert each `effective` key/value into a Lambda resource's
/// `Properties.Environment.Variables`, creating the nested objects if absent.
/// Existing values for the same key are OVERWRITTEN (precedence: declared
/// `[environment]` wins over the construct default). A no-op when `effective`
/// is empty.
fn apply_env_to_lambda(resource: &mut serde_json::Value, effective: &[(String, String)]) {
    if effective.is_empty() {
        return;
    }
    let variables = resource
        .as_object_mut()
        .and_then(|r| ensure_object(r, "Properties"))
        .and_then(|p| ensure_object(p, "Environment"))
        .and_then(|e| ensure_object(e, "Variables"));
    if let Some(vars) = variables {
        for (key, value) in effective {
            vars.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
    }
}

/// Get or create a JSON object at `key` within `parent`, returning a mutable
/// reference to it. Returns `None` only when an existing non-object value
/// occupies `key` (we never clobber a non-object).
fn ensure_object<'a>(
    parent: &'a mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a mut serde_json::Map<String, serde_json::Value>> {
    parent
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
}

/// Build the fail-loud warning shown when `[environment]` is declared but the
/// synthesized template contains no `AWS::Lambda::Function` to inject into.
fn environment_no_lambda_warning(
    environment: &HashMap<String, String>,
    secret_keys: &HashSet<String>,
) -> String {
    let mut applied: Vec<&str> = environment
        .keys()
        .filter(|k| !secret_keys.contains(*k))
        .map(String::as_str)
        .collect();
    applied.sort_unstable();
    let keys = if applied.is_empty() {
        "(none — all declared keys are secrets)".to_string()
    } else {
        applied.join(", ")
    };
    format!(
        "⚠️  [environment] declared but NOT applied — the synthesized CloudFormation \
         template contains no AWS::Lambda::Function resource to inject into.\n     \
         Affected keys: {keys}\n     \
         If your server runs on Lambda, verify the CDK stack synthesized a Lambda \
         function; otherwise these keys will not reach the runtime."
    )
}

// ── [server] sizing: post-synth MemorySize/Timeout merge ────────────────────
// (debug session `deploy-server-memory-timeout`)

/// Outcome of merging `[server]` sizing into a synthesized CloudFormation
/// template. See [`merge_sizing_into_template`].
#[derive(Debug)]
struct SizingMergeOutcome {
    /// The re-serialized template JSON with the declared sizing applied.
    template: String,
    /// One human-readable line per MCP-function resource that was rewritten,
    /// naming the before/after values (e.g.
    /// `"McpFunction: MemorySize 256 -> 1024"`). Empty means no matching
    /// Lambda was found — the caller uses this for the fail-loud warning.
    ///
    /// A property already carrying the declared value produces no entry for
    /// that property, so an idempotent re-deploy stays quiet about it while
    /// the resource is still reported as matched.
    changes: Vec<String>,
    /// `true` when at least one `AWS::Lambda::Function` whose
    /// `Properties.FunctionName` equals the configured server name was found.
    /// Distinct from `changes` being non-empty: a matched-but-already-correct
    /// template yields `matched = true` with no changes, which must NOT trip
    /// the fail-loud path.
    matched: bool,
}

/// Merge the synthesized template with the declared `[server]` sizing and emit
/// operator feedback.
///
/// Thin deploy-time wrapper around the pure [`merge_sizing_into_template`]
/// helper: it prints either a per-property before/after summary or the
/// fail-loud "no matching Lambda" warning, and returns the (possibly modified)
/// template string. When neither `memory_mb` nor `timeout_seconds` is declared
/// the template is returned unchanged and nothing is printed.
///
/// # Precedence — a DELIBERATE divergence from the `[environment]` fix
///
/// The sibling session (`deploy-toml-inert-for-preserved-stack`) ruled that a
/// `stack.ts` literal WINS over `deploy.toml` for `[environment]`. That rule
/// is deliberately NOT followed here: declared sizing OVERRIDES the stack.ts
/// literal. The two cases are not analogous — `[environment]` is a MAP, where
/// "the literal wins" is a coherent additive-fill (deploy.toml contributes
/// keys the construct did not set), whereas `memorySize` is a SCALAR the
/// construct always sets, so "the literal wins" would degenerate to "the
/// config is inert", which is the bug being fixed. The sibling already broke
/// its own rule once, for construct collisions. The divergence and this
/// rationale are documented in `cargo-pmcp/docs/commands/deploy.md`.
fn apply_sizing_merge(
    template: String,
    function_name: &str,
    memory_mb: Option<u32>,
    timeout_seconds: Option<u32>,
) -> Result<String> {
    if memory_mb.is_none() && timeout_seconds.is_none() {
        return Ok(template);
    }

    let outcome = merge_sizing_into_template(&template, function_name, memory_mb, timeout_seconds)?;

    if outcome.matched {
        if outcome.changes.is_empty() {
            println!("   ✅ [server] sizing already matches the synthesized template");
        } else {
            for change in &outcome.changes {
                println!("   ✅ Applied [server] sizing — {change}");
            }
        }
    } else {
        // Fail-loud: sizing was declared but no Lambda in the synthesized
        // template carries this server's FunctionName, so there is nothing to
        // apply it to. Warn prominently instead of dropping it silently — the
        // silence is precisely what made this bug survive three sessions.
        eprintln!(
            "{}",
            sizing_no_lambda_warning(function_name, memory_mb, timeout_seconds)
        );
    }

    Ok(outcome.template)
}

/// Rewrite `Properties.MemorySize` / `Properties.Timeout` on the MCP function
/// in a synthesized CloudFormation template. Pure and unit-testable — no
/// synth, no I/O.
///
/// # Which resource is targeted
///
/// ONLY `AWS::Lambda::Function` resources whose `Properties.FunctionName`
/// equals `function_name`. Matching on `Type` alone (the way the
/// `[environment]` merge does) would be a regression: an OAuth-enabled stack
/// renders THREE Lambdas at three different sizings — `<name>-oauth-proxy`
/// at 256/30, `<name>` at 512/30, and `<name>-authorizer` at 256/**10** — and
/// resizing the authorizer to the MCP function's memory and timeout would
/// silently reconfigure infrastructure the operator never mentioned. Both
/// synth engines set the discriminating property the same way (the renderer
/// via `function_name: d.server.name`, the TS scaffold via
/// `functionName: serverId`), so this is exact on either path. Logical IDs
/// are deliberately NOT used: they are CDK-generated and unknowable for
/// hand-authored or shared constructs.
///
/// # Precedence
/// A declared value OVERRIDES whatever the construct emitted — see
/// [`apply_sizing_merge`]'s doc comment for why this deliberately diverges
/// from the `[environment]` merge's additive-fill rule. A `None` argument
/// leaves that property exactly as synthesized.
fn merge_sizing_into_template(
    template_json: &str,
    function_name: &str,
    memory_mb: Option<u32>,
    timeout_seconds: Option<u32>,
) -> Result<SizingMergeOutcome> {
    let mut template: serde_json::Value = serde_json::from_str(template_json)
        .context("Failed to parse synthesized CloudFormation template JSON")?;

    let mut changes: Vec<String> = Vec::new();
    let mut matched = false;

    if let Some(resources) = template
        .get_mut("Resources")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (logical_id, resource) in resources.iter_mut() {
            if !is_mcp_lambda_function(resource, function_name) {
                continue;
            }
            matched = true;
            apply_sizing_to_lambda(
                resource,
                logical_id,
                memory_mb,
                timeout_seconds,
                &mut changes,
            );
        }
    }

    changes.sort();

    let merged = serde_json::to_string_pretty(&template)
        .context("Failed to re-serialize merged CloudFormation template")?;

    Ok(SizingMergeOutcome {
        template: merged,
        changes,
        matched,
    })
}

/// True when `resource` is an `AWS::Lambda::Function` whose
/// `Properties.FunctionName` equals `function_name` — the MCP function, as
/// opposed to an OAuth proxy or authorizer sharing the same stack.
fn is_mcp_lambda_function(resource: &serde_json::Value, function_name: &str) -> bool {
    is_lambda_function(resource)
        && resource
            .get("Properties")
            .and_then(|p| p.get("FunctionName"))
            .and_then(serde_json::Value::as_str)
            == Some(function_name)
}

/// Write the declared sizing onto one matched Lambda resource, appending a
/// `"<logical id>: <Property> <before> -> <after>"` line to `changes` for each
/// property whose value actually moved. Creates `Properties` if absent.
fn apply_sizing_to_lambda(
    resource: &mut serde_json::Value,
    logical_id: &str,
    memory_mb: Option<u32>,
    timeout_seconds: Option<u32>,
    changes: &mut Vec<String>,
) {
    let Some(properties) = resource
        .as_object_mut()
        .and_then(|r| ensure_object(r, "Properties"))
    else {
        return;
    };

    for (key, declared) in [("MemorySize", memory_mb), ("Timeout", timeout_seconds)] {
        let Some(declared) = declared else { continue };
        let before = properties.get(key).and_then(serde_json::Value::as_u64);
        if before == Some(u64::from(declared)) {
            continue;
        }
        let before_label = before.map_or_else(|| "(unset)".to_string(), |v| v.to_string());
        changes.push(format!("{logical_id}: {key} {before_label} -> {declared}"));
        properties.insert(key.to_string(), serde_json::Value::from(declared));
    }
}

/// Build the fail-loud warning shown when `[server]` sizing is declared but the
/// synthesized template contains no `AWS::Lambda::Function` whose
/// `FunctionName` matches the configured server name.
fn sizing_no_lambda_warning(
    function_name: &str,
    memory_mb: Option<u32>,
    timeout_seconds: Option<u32>,
) -> String {
    let mut declared: Vec<String> = Vec::new();
    if let Some(m) = memory_mb {
        declared.push(format!("memory_mb = {m}"));
    }
    if let Some(t) = timeout_seconds {
        declared.push(format!("timeout_seconds = {t}"));
    }
    let declared = declared.join(", ");
    format!(
        "⚠️  [server] sizing declared but NOT applied — the synthesized CloudFormation \
         template contains no AWS::Lambda::Function whose FunctionName is \
         '{function_name}'.\n     \
         Declared: {declared}\n     \
         Check that [server] name matches the function your stack.ts creates; otherwise the \
         deployed function keeps whatever size the template hardcodes."
    )
}

/// Run the fail-closed IAM validator and rewrite `deploy/lib/stack.ts` from
/// the loaded [`DeployConfig`], so `[iam]` declared in `.pmcp/deploy.toml`
/// lands in the synthesized CloudFormation template. Mirrors
/// `DeployExecutor::regenerate_stack_ts` from the aws-lambda path.
fn validate_and_regenerate_stack_ts(config: &DeployConfig) -> Result<()> {
    let warnings = crate::deployment::iam::validate(&config.iam)
        .context("IAM validation failed — fix .pmcp/deploy.toml before deploying")?;
    crate::deployment::iam::emit_warnings(&warnings);

    let lib_dir = config.project_root.join("deploy").join("lib");
    let stack_ts = crate::commands::deploy::init::render_stack_ts_for_deploy(
        &config.target.target_type,
        &config.server.name,
        &config.iam,
        &config.metadata,
    );
    // DSTK-01: skip the write (preserving an operator-curated stack.ts) unless
    // `--regenerate-stack`/`--force` was passed. IAM validation above always
    // runs, so the guard never disables validation.
    let wrote = crate::deployment::config::write_stack_ts_guarded(
        &lib_dir,
        &stack_ts,
        config.regenerate_stack,
    )?;
    if !wrote {
        println!("{}", crate::deployment::config::STACK_TS_PRESERVED_NOTICE);
        // FIX #1 (deploy-toml-inert-for-preserved-stack): warn loudly when the
        // preserved stack.ts means declared [iam]/[environment] are not
        // auto-applied. Mirrors the aws-lambda path
        // (commands/deploy/deploy.rs) so the signal is target-uniform.
        // `sizing_inert = false`: unlike the aws-lambda target, this one
        // rewrites `Properties.MemorySize`/`Timeout` post-synth (see
        // `apply_sizing_merge`), on BOTH synth engines — so a preserved
        // stack.ts does not make `[server]` sizing inert here and warning
        // about it would be false (debug session
        // `deploy-server-memory-timeout`).
        if let Some(warning) = crate::deployment::config::stack_ts_preserved_inert_warning(
            config.iam.is_empty(),
            config.environment.is_empty(),
            false,
        ) {
            eprintln!("{warning}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::config::{IamConfig, IamStatement, TablePermission};

    fn cfg_with_target_and_iam(
        project_root: PathBuf,
        target_type: &str,
        iam: IamConfig,
    ) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-east-1".to_string(),
            project_root,
        );
        cfg.target.target_type = target_type.to_string();
        cfg.iam = iam;
        cfg
    }

    #[test]
    fn pmcp_run_deploy_regenerates_stack_ts_with_iam_block() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "Users".to_string(),
                actions: vec!["read".to_string()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let config = cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", iam);

        validate_and_regenerate_stack_ts(&config).expect("should succeed with valid iam");

        let stack_ts =
            std::fs::read_to_string(tmp.path().join("deploy").join("lib").join("stack.ts"))
                .expect("stack.ts written");

        assert!(
            stack_ts.contains("Operator-declared IAM"),
            "pmcp-run stack.ts missing user-declared IAM banner — renderer was not invoked"
        );
        assert!(
            stack_ts.contains("table/Users"),
            "pmcp-run stack.ts missing the Users table resource ARN"
        );
        assert!(
            stack_ts.contains("pmcp-${serverId}-McpRoleArn"),
            "pmcp-run branch signature (McpRoleArn exportName) missing — wrong template branch was rendered"
        );
    }

    #[test]
    fn pmcp_run_deploy_rejects_iam_footgun_before_writing_stack_ts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let iam = IamConfig {
            statements: vec![IamStatement {
                effect: "Allow".to_string(),
                actions: vec!["*".to_string()],
                resources: vec!["*".to_string()],
            }],
            ..IamConfig::default()
        };
        let config = cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", iam);

        let err = validate_and_regenerate_stack_ts(&config)
            .expect_err("Allow-*-* must be rejected by the validator gate");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("IAM validation failed"),
            "expected validator gate message, got: {msg}"
        );

        assert!(
            !tmp.path()
                .join("deploy")
                .join("lib")
                .join("stack.ts")
                .exists(),
            "stack.ts must not be written when validator rejects config (fail-closed)"
        );
    }

    /// Seed a curated `deploy/lib/stack.ts` under `project_root` and return its
    /// path + the curated content for byte-identity assertions.
    fn seed_curated_stack_ts(project_root: &std::path::Path) -> (PathBuf, String) {
        let lib_dir = project_root.join("deploy").join("lib");
        std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
        let path = lib_dir.join("stack.ts");
        let curated = "// operator-curated stack.ts — DO NOT CLOBBER\n".to_string();
        std::fs::write(&path, &curated).expect("seed curated stack.ts");
        (path, curated)
    }

    /// DSTK-01: a pre-existing curated stack.ts is preserved byte-for-byte when
    /// no `--regenerate-stack`/`--force` flag is set, while IAM validation
    /// (which precedes the guarded write) still runs successfully.
    #[test]
    fn pmcp_run_preserves_existing_stack_ts_without_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (path, curated) = seed_curated_stack_ts(tmp.path());

        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        config.regenerate_stack = false;

        validate_and_regenerate_stack_ts(&config).expect("guard succeeds, IAM still validates");

        let after = std::fs::read_to_string(&path).expect("read stack.ts back");
        assert_eq!(
            after, curated,
            "curated stack.ts must be byte-identical when regenerate_stack is false"
        );
    }

    /// DSTK-01: with `--regenerate-stack`/`--force` the curated file is
    /// re-rendered from the template (overwritten).
    #[test]
    fn pmcp_run_overwrites_existing_stack_ts_with_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (path, curated) = seed_curated_stack_ts(tmp.path());

        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        config.regenerate_stack = true;

        validate_and_regenerate_stack_ts(&config).expect("regenerate succeeds");

        let after = std::fs::read_to_string(&path).expect("read stack.ts back");
        assert_ne!(
            after, curated,
            "stack.ts must be overwritten when regenerate_stack is true"
        );
        assert!(
            after.contains("pmcp-${serverId}-McpRoleArn"),
            "overwritten stack.ts must carry the pmcp-run rendered template signature"
        );
    }

    /// FIX #2 (deploy-toml-inert-for-preserved-stack): developer-declared
    /// `[environment]` must be threaded onto the `cdk synth` child process as
    /// env vars, so a preserved-or-generated stack.ts can consume it via
    /// `process.env.<KEY>`. Inspect the built command's env without spawning.
    #[test]
    fn cdk_synth_command_carries_deploy_toml_environment() {
        use std::ffi::OsStr;

        let tmp = tempfile::tempdir().expect("tempdir");
        let mut env = std::collections::HashMap::new();
        env.insert("GRAPHRAG_ENDPOINT".to_string(), "https://x".to_string());

        let cmd = build_cdk_synth_command(tmp.path(), None, &env);

        let found = cmd.get_envs().any(|(k, v)| {
            k == OsStr::new("GRAPHRAG_ENDPOINT") && v == Some(OsStr::new("https://x"))
        });
        assert!(
            found,
            "[environment] entry must be set on the cdk synth child process (FIX #2)"
        );
    }

    // ========================================================================
    // Task 7 (CFN-renderer extraction): synth routing + renderer-path tests.
    // ========================================================================

    /// Write `.pmcp/deploy.toml` to `project_root` by serializing `config` —
    /// the on-disk file `load_deploy_descriptor`/`try_render` actually read.
    /// Task 7 parses the renderer's `DeployDescriptor` straight from disk,
    /// never from the in-memory `DeployConfig` (Interfaces §2), so routing
    /// tests need a real file on top of the in-memory fixture.
    fn write_deploy_toml(project_root: &std::path::Path, config: &DeployConfig) {
        let dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&dir).expect("create .pmcp dir");
        let text = toml::to_string_pretty(config).expect("serialize DeployConfig");
        std::fs::write(dir.join("deploy.toml"), text).expect("write .pmcp/deploy.toml");
    }

    /// Routing rule (Step 1): a stack.ts that still matches the regenerated
    /// scaffold takes the renderer path — `custom_stack_ts_reason` returns
    /// `None`, and `try_render` actually renders a template from the
    /// on-disk `.pmcp/deploy.toml` (no `cdk synth` subprocess involved).
    #[test]
    fn synth_routes_to_renderer_when_stack_ts_matches_scaffold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        write_deploy_toml(tmp.path(), &config);
        validate_and_regenerate_stack_ts(&config).expect("scaffold stack.ts written");

        assert_eq!(
            custom_stack_ts_reason(&config).expect("taint check succeeds"),
            None,
            "a freshly (re)generated stack.ts must match its own scaffold"
        );

        let rendered =
            try_render(&config, None).expect("renderer must succeed on a fresh scaffold");
        let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("valid JSON");
        assert!(
            parsed.get("Resources").is_some(),
            "rendered template must carry a Resources section, got: {rendered}"
        );
    }

    /// Routing rule (Step 1): a hand-modified stack.ts falls back to the
    /// legacy path — `custom_stack_ts_reason` names the file, and the taint
    /// is recorded onto the deploy metadata map (`custom_stack`) alongside
    /// `server_type`/`snapshot_baked` via `mark_custom_stack`.
    #[test]
    fn synth_routes_to_legacy_when_stack_ts_is_hand_modified() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        write_deploy_toml(tmp.path(), &config);
        let (path, _curated) = seed_curated_stack_ts(tmp.path());

        let reason = custom_stack_ts_reason(&config)
            .expect("taint check succeeds")
            .expect("hand-curated stack.ts must be detected as modified");
        assert!(
            reason.contains(&path.display().to_string()),
            "reason must name the stack.ts path, got: {reason}"
        );

        let metadata = McpMetadata::extract(tmp.path()).ok();
        let tainted = mark_custom_stack(metadata.as_ref()).expect("metadata always present");
        assert!(
            tainted.custom_stack,
            "custom_stack must be recorded onto the metadata map"
        );
        assert!(
            tainted
                .to_cdk_context()
                .iter()
                .any(|c| c.contains("mcp:customStack=true")),
            "the taint must reach the cdk synth context args"
        );
    }

    /// `mark_custom_stack` is a no-op on `None` — a project with no
    /// discoverable metadata has nothing to tag.
    #[test]
    fn mark_custom_stack_none_stays_none() {
        assert!(mark_custom_stack(None).is_none());
    }

    /// A `.pmcp/deploy.toml` that doesn't exist yet (or fails to parse as
    /// the renderer's closed-set `DeployDescriptor`) is a graceful `Err`
    /// (fallback reason), never a panic or hard failure — `synth_template`
    /// relies on this to fall back to `cdk synth` instead of breaking the
    /// deploy outright.
    #[test]
    fn try_render_reports_a_reason_when_deploy_toml_is_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        // Deliberately do not write .pmcp/deploy.toml.

        let reason = try_render(&config, None).expect_err("missing deploy.toml must not panic");
        assert!(
            reason.contains("deploy.toml"),
            "reason must name the missing/unparseable file, got: {reason}"
        );
    }

    /// `RenderParams::account_id` falls back to the documented all-zeros
    /// sentinel when `.pmcp/deploy.toml`'s `[aws]` has no `account_id` —
    /// matching what today's `cdk synth` receives for pmcp-run (nothing;
    /// `CDK_DEFAULT_ACCOUNT` is never set on that path).
    #[test]
    fn build_render_params_account_id_falls_back_to_sentinel_when_unset() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        assert!(config.aws().account_id.is_none());

        let params = build_render_params(&config, None);
        assert_eq!(params.account_id, UNRESOLVED_ACCOUNT_ID);
    }

    /// When the operator DOES declare `[aws] account_id`, it is used
    /// verbatim — the same field the sibling aws-lambda `cdk deploy` path
    /// already reads (`commands/deploy/deploy.rs`).
    #[test]
    fn build_render_params_uses_declared_account_id_when_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        config.aws.as_mut().expect("aws present").account_id = Some("111122223333".to_string());

        let params = build_render_params(&config, None);
        assert_eq!(params.account_id, "111122223333");
    }

    /// `RenderParams.environment` mirrors `config.environment` exactly —
    /// never `config.secrets` (Interfaces §5: secret VALUES must never
    /// reach the renderer).
    #[test]
    fn build_render_params_environment_excludes_secrets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        config
            .environment
            .insert("RUST_LOG".to_string(), "debug".to_string());
        config
            .secrets
            .insert("API_TOKEN".to_string(), "shhh".to_string());

        let params = build_render_params(&config, None);
        assert_eq!(
            params.environment.get("RUST_LOG"),
            Some(&"debug".to_string())
        );
        assert!(
            !params.environment.contains_key("API_TOKEN"),
            "secret keys/values must never reach RenderParams.environment"
        );
    }

    /// `RenderParams.stack_name` mirrors the `${serverName}-stack` name the
    /// legacy `app.ts` scaffold hardcodes (Task 7, byte-parity with the
    /// upload flow's expectations).
    #[test]
    fn build_render_params_stack_name_matches_app_ts_convention() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());

        let params = build_render_params(&config, None);
        assert_eq!(params.stack_name, "demo-server-stack");
    }

    /// T7 review fix: `cloudformation_metadata_from` populates
    /// `RenderParams::cloudformation_metadata` from the EXISTING maintained
    /// `McpMetadata::to_cloudformation_metadata` (DSTK-03 shape) —
    /// previously called nowhere in the renderer path, so the uploaded
    /// template's `Metadata` block lost all `mcp:*` provenance. Asserts the
    /// object's keys/values, not just that it's non-empty.
    #[test]
    fn cloudformation_metadata_from_maps_the_maintained_dstk03_shape() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "graphql-api".to_string(),
            server_id: "srv-1".to_string(),
            template_id: Some("types/graphql".to_string()),
            template_version: None,
            resources: crate::deployment::metadata::ResourceRequirements::default(),
            capabilities: crate::deployment::metadata::ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        };

        let cf_metadata = cloudformation_metadata_from(Some(&metadata));
        assert_eq!(
            cf_metadata.get("mcp:version"),
            Some(&serde_json::json!("1.0"))
        );
        assert_eq!(
            cf_metadata.get("mcp:serverType"),
            Some(&serde_json::json!("graphql-api"))
        );
        assert_eq!(
            cf_metadata.get("mcp:serverId"),
            Some(&serde_json::json!("srv-1"))
        );
        assert_eq!(
            cf_metadata.get("mcp:templateId"),
            Some(&serde_json::json!("types/graphql"))
        );
        assert!(cf_metadata.contains_key("mcp:resources"));
        assert!(cf_metadata.contains_key("mcp:capabilities"));
        // DSTK-03 conditional emission: not opted into snapshot_baked here,
        // so that key must be absent, matching `to_cloudformation_metadata`'s
        // own byte-identity-for-non-opting-servers rule.
        assert!(!cf_metadata.contains_key("mcp:snapshotBaked"));
    }

    /// `cloudformation_metadata_from(None)` yields an empty map — "no
    /// metadata resolved yet" falls back to an empty map, and (via
    /// `CfnTemplate`'s envelope rule) results in no `Metadata` key in the
    /// rendered template at all.
    #[test]
    fn cloudformation_metadata_from_none_is_empty() {
        assert!(cloudformation_metadata_from(None).is_empty());
    }

    /// End-to-end: `build_render_params` actually wires
    /// `cloudformation_metadata_from`'s output onto
    /// `RenderParams::cloudformation_metadata` — not just that the helper
    /// function works in isolation.
    #[test]
    fn build_render_params_populates_cloudformation_metadata_from_mcp_metadata() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config =
            cfg_with_target_and_iam(tmp.path().to_path_buf(), "pmcp-run", IamConfig::default());
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "custom".to_string(),
            server_id: "srv-build-params".to_string(),
            template_id: None,
            template_version: None,
            resources: crate::deployment::metadata::ResourceRequirements::default(),
            capabilities: crate::deployment::metadata::ServerCapabilities::default(),
            available_operations: None,
            snapshot_baked: false,
            custom_stack: false,
        };

        let params = build_render_params(&config, Some(&metadata));
        assert_eq!(
            params.cloudformation_metadata,
            metadata
                .to_cloudformation_metadata()
                .as_object()
                .cloned()
                .unwrap()
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
        );
        assert_eq!(
            params.cloudformation_metadata.get("mcp:serverId"),
            Some(&serde_json::json!("srv-build-params"))
        );
    }

    /// Item 3 (the tracked T4/T6 warnings-discard gap): `emit_descriptor_warnings`
    /// must not panic against a descriptor that actually triggers BOTH
    /// `pmcp_cfn_renderer::resources::iam::validate` warning classes —
    /// `iam.unknown_service_prefix` and `iam.cross_account_arn` — proving the
    /// wiring (field access + the `Vec<Warning>` extend/print loop) is
    /// correct end-to-end, not just that the underlying `validate` functions
    /// work (those are already exhaustively tested in `pmcp-cfn-renderer`
    /// itself). Run with `-- --nocapture` to see the printed advisories.
    #[test]
    fn emit_descriptor_warnings_prints_both_iam_warning_classes() {
        use pmcp_package::package::{DeployDescriptor, IamSection, IamStatement};
        let descriptor_iam = IamSection {
            statements: vec![IamStatement {
                effect: "Allow".to_string(),
                actions: vec!["foobar:DoSomething".to_string()],
                resources: vec!["arn:aws:foobar:us-east-1:999999999999:thing/x".to_string()],
            }],
        };
        let descriptor = DeployDescriptor {
            target: pmcp_package::package::TargetSection {
                target_type: "pmcp-run".to_string(),
                version: "1.0.0".to_string(),
            },
            metadata: None,
            aws: pmcp_package::package::AwsSection {
                region: "us-east-1".to_string(),
            },
            server: pmcp_package::package::ServerSection {
                name: "scratch".to_string(),
                memory_mb: Some(512),
                timeout_seconds: 30,
                memory: None,
                cpu: None,
                ingress: None,
                allow_unauthenticated: None,
                binary: None,
            },
            environment: Default::default(),
            secrets: Default::default(),
            auth: pmcp_package::package::AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: None,
                groups: None,
                scopes: None,
            },
            observability: pmcp_package::package::ObservabilitySection {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: None,
            },
            composition: None,
            assets: None,
            iam: Some(descriptor_iam),
            gcp: None,
            layout: None,
        };
        emit_descriptor_warnings(&descriptor);
    }
}

// ── FIX #1: construct-agnostic post-synth [environment] template merge ───────
// (environment-inert-for-shared-cdk-constructs)
#[cfg(test)]
mod env_merge_tests {
    use super::{
        apply_env_to_lambda, environment_no_lambda_warning, is_lambda_function,
        merge_environment_into_template,
    };
    use serde_json::{json, Value};
    use std::collections::{HashMap, HashSet};

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn secret_keys(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| (*k).to_string()).collect()
    }

    fn merged_value(
        template: &str,
        env: &HashMap<String, String>,
        secrets: &HashSet<String>,
    ) -> Value {
        let out = merge_environment_into_template(template, env, secrets)
            .expect("merge must parse and re-serialize valid template JSON");
        serde_json::from_str(&out.template).expect("merged template must be valid JSON")
    }

    fn variables(v: &Value, logical_id: &str) -> Value {
        v["Resources"][logical_id]["Properties"]["Environment"]["Variables"].clone()
    }

    /// Branch (a): a key is added to a Lambda that has no existing
    /// `Environment` block — the nested objects are created.
    #[test]
    fn branch_a_adds_key_to_lambda_without_environment() {
        let template = json!({
            "Resources": {
                "Fn": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "Runtime": "provided.al2023" }
                }
            }
        })
        .to_string();

        let out = merge_environment_into_template(
            &template,
            &env(&[("RUST_LOG", "warn")]),
            &secret_keys(&[]),
        )
        .expect("merge succeeds");

        assert_eq!(out.lambdas_updated, vec!["Fn".to_string()]);
        let parsed: Value = serde_json::from_str(&out.template).unwrap();
        assert_eq!(variables(&parsed, "Fn"), json!({ "RUST_LOG": "warn" }));
    }

    /// Branch (b): a key is added alongside existing `Variables` without
    /// clobbering the construct's other entries.
    #[test]
    fn branch_b_adds_key_alongside_existing_variables() {
        let template = json!({
            "Resources": {
                "Fn": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "Environment": { "Variables": { "EXISTING": "kept" } }
                    }
                }
            }
        })
        .to_string();

        let parsed = merged_value(&template, &env(&[("NEW_KEY", "added")]), &secret_keys(&[]));
        assert_eq!(
            variables(&parsed, "Fn"),
            json!({ "EXISTING": "kept", "NEW_KEY": "added" })
        );
    }

    /// Branch (c): on key collision the declared `[environment]` value OVERRIDES
    /// the construct's hardcoded value (locked precedence).
    #[test]
    fn branch_c_environment_overrides_construct_value() {
        let template = json!({
            "Resources": {
                "Fn": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "Environment": { "Variables": { "RUST_LOG": "info" } }
                    }
                }
            }
        })
        .to_string();

        let parsed = merged_value(&template, &env(&[("RUST_LOG", "warn")]), &secret_keys(&[]));
        assert_eq!(
            variables(&parsed, "Fn")["RUST_LOG"],
            json!("warn"),
            "declared [environment] must override the construct default"
        );
    }

    /// Branch (d): keys present in the secret-key set are EXCLUDED from the
    /// merge — secret values never enter the template.
    #[test]
    fn branch_d_secret_keys_excluded_from_merge() {
        let template = json!({
            "Resources": {
                "Fn": { "Type": "AWS::Lambda::Function", "Properties": {} }
            }
        })
        .to_string();

        let parsed = merged_value(
            &template,
            &env(&[("PUBLIC_URL", "https://x"), ("API_TOKEN", "shhh")]),
            &secret_keys(&["API_TOKEN"]),
        );

        let vars = variables(&parsed, "Fn");
        assert_eq!(vars["PUBLIC_URL"], json!("https://x"));
        assert!(
            vars.get("API_TOKEN").is_none(),
            "secret key must NOT be merged into the template"
        );
    }

    /// Branch (e): every `AWS::Lambda::Function` resource is updated when the
    /// template declares more than one.
    #[test]
    fn branch_e_multiple_lambdas_all_updated() {
        let template = json!({
            "Resources": {
                "FnA": { "Type": "AWS::Lambda::Function", "Properties": {} },
                "FnB": { "Type": "AWS::Lambda::Function", "Properties": {} }
            }
        })
        .to_string();

        let out = merge_environment_into_template(
            &template,
            &env(&[("RUST_LOG", "warn")]),
            &secret_keys(&[]),
        )
        .expect("merge succeeds");

        assert_eq!(
            out.lambdas_updated,
            vec!["FnA".to_string(), "FnB".to_string()],
            "logical IDs must be reported sorted"
        );
        let parsed: Value = serde_json::from_str(&out.template).unwrap();
        assert_eq!(variables(&parsed, "FnA"), json!({ "RUST_LOG": "warn" }));
        assert_eq!(variables(&parsed, "FnB"), json!({ "RUST_LOG": "warn" }));
    }

    /// Branch (f): non-Lambda resources are left untouched.
    #[test]
    fn branch_f_non_lambda_resources_untouched() {
        let template = json!({
            "Resources": {
                "Fn": { "Type": "AWS::Lambda::Function", "Properties": {} },
                "Bucket": {
                    "Type": "AWS::S3::Bucket",
                    "Properties": { "BucketName": "assets" }
                }
            }
        })
        .to_string();

        let out = merge_environment_into_template(
            &template,
            &env(&[("RUST_LOG", "warn")]),
            &secret_keys(&[]),
        )
        .expect("merge succeeds");

        assert_eq!(out.lambdas_updated, vec!["Fn".to_string()]);
        let parsed: Value = serde_json::from_str(&out.template).unwrap();
        assert_eq!(
            parsed["Resources"]["Bucket"],
            json!({ "Type": "AWS::S3::Bucket", "Properties": { "BucketName": "assets" } }),
            "non-Lambda resource must be byte-preserved"
        );
    }

    /// Branch (g): fail-loud path — a non-empty `[environment]` but zero Lambda
    /// resources yields an empty `lambdas_updated` list (the caller's warning
    /// trigger) and a warning naming the affected keys.
    #[test]
    fn branch_g_fail_loud_when_no_lambda_resource() {
        let template = json!({
            "Resources": {
                "Bucket": { "Type": "AWS::S3::Bucket", "Properties": {} }
            }
        })
        .to_string();

        let environment = env(&[("RUST_LOG", "warn"), ("PUBLIC_URL", "https://x")]);
        let secrets = secret_keys(&[]);
        let out = merge_environment_into_template(&template, &environment, &secrets)
            .expect("merge succeeds even with no Lambda");

        assert!(
            out.lambdas_updated.is_empty(),
            "no Lambda resource must yield an empty updated list (fail-loud trigger)"
        );

        let warning = environment_no_lambda_warning(&environment, &secrets);
        assert!(warning.contains("NOT applied"), "warning is prominent");
        assert!(
            warning.contains("PUBLIC_URL"),
            "warning names affected keys"
        );
        assert!(warning.contains("RUST_LOG"), "warning names affected keys");
    }

    /// Fail-loud wording when every declared key is a secret (nothing to apply).
    #[test]
    fn fail_loud_all_secret_keys_notes_none() {
        let environment = env(&[("API_TOKEN", "shhh")]);
        let secrets = secret_keys(&["API_TOKEN"]);
        let warning = environment_no_lambda_warning(&environment, &secrets);
        assert!(
            warning.contains("all declared keys are secrets"),
            "warning explains there is nothing non-secret to apply"
        );
    }

    /// Guard: `is_lambda_function` matches only the exact CFN type.
    #[test]
    fn is_lambda_function_type_matching() {
        assert!(is_lambda_function(
            &json!({ "Type": "AWS::Lambda::Function" })
        ));
        assert!(!is_lambda_function(&json!({ "Type": "AWS::Lambda::Url" })));
        assert!(!is_lambda_function(&json!({ "Properties": {} })));
    }

    /// Guard: an empty effective set is a no-op — the Lambda is unchanged.
    #[test]
    fn apply_env_to_lambda_empty_effective_is_noop() {
        let mut resource = json!({
            "Type": "AWS::Lambda::Function",
            "Properties": { "Runtime": "provided.al2023" }
        });
        apply_env_to_lambda(&mut resource, &[]);
        assert!(
            resource["Properties"].get("Environment").is_none(),
            "empty effective set must not create an Environment block"
        );
    }

    /// Invalid template JSON surfaces a parse error rather than silently
    /// dropping the merge.
    #[test]
    fn invalid_template_json_errors() {
        let err = merge_environment_into_template(
            "{ not valid json",
            &env(&[("K", "v")]),
            &secret_keys(&[]),
        )
        .expect_err("invalid JSON must error");
        assert!(
            err.to_string().contains("parse synthesized CloudFormation"),
            "error must name the parse failure"
        );
    }
}

// ── [server] sizing: post-synth MemorySize/Timeout merge ────────────────────
// (debug session `deploy-server-memory-timeout`)
#[cfg(test)]
mod sizing_merge_tests {
    use super::{
        apply_sizing_merge, is_mcp_lambda_function, merge_sizing_into_template,
        sizing_no_lambda_warning,
    };
    use serde_json::{json, Value};

    const SERVER: &str = "okf-demo";

    /// A single-Lambda pmcp-run template as the scaffold/`cdk synth` engine
    /// emits it: `memorySize: 256`, `timeout: cdk.Duration.seconds(30)`.
    fn scaffold_template() -> String {
        json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 30,
                        "Runtime": "provided.al2023"
                    }
                }
            }
        })
        .to_string()
    }

    fn props(v: &Value, logical_id: &str) -> Value {
        v["Resources"][logical_id]["Properties"].clone()
    }

    fn merged(
        template: &str,
        memory_mb: Option<u32>,
        timeout_seconds: Option<u32>,
    ) -> (Value, Vec<String>, bool) {
        let out = merge_sizing_into_template(template, SERVER, memory_mb, timeout_seconds)
            .expect("merge must parse and re-serialize valid template JSON");
        let parsed: Value =
            serde_json::from_str(&out.template).expect("merged template must be valid JSON");
        (parsed, out.changes, out.matched)
    }

    /// (1) No-op when nothing is declared: `apply_sizing_merge` returns the
    /// template BYTE-IDENTICALLY rather than round-tripping it through serde.
    ///
    /// This is the case that protects the installed base. `memory_mb` /
    /// `timeout_seconds` are `Option` precisely so a `deploy.toml` that never
    /// mentioned sizing leaves the engine's own default alone, instead of
    /// materializing the schema default (512) over the pmcp-run engine's 256
    /// and silently doubling every Lambda nobody asked to resize.
    #[test]
    fn no_op_when_nothing_declared() {
        let template = scaffold_template();
        let out =
            apply_sizing_merge(template.clone(), SERVER, None, None).expect("no-op merge succeeds");
        assert_eq!(
            out, template,
            "an undeclared sizing must return the template untouched, byte for byte"
        );
    }

    /// (2) Declared sizing lands on the MCP function's `MemorySize`/`Timeout`.
    #[test]
    fn applies_declared_memory_and_timeout() {
        let (parsed, changes, matched) = merged(&scaffold_template(), Some(1024), Some(60));
        assert!(matched, "the MCP function must be matched");
        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(1024));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(60));
        assert_eq!(
            changes,
            vec![
                "McpFunction: MemorySize 256 -> 1024".to_string(),
                "McpFunction: Timeout 30 -> 60".to_string(),
            ],
            "both moves must be reported with before/after values"
        );
    }

    /// (3) A Lambda with no `Properties` at all gets one created.
    #[test]
    fn creates_properties_when_absent() {
        // A resource with no Properties cannot carry a FunctionName, so it is
        // not the MCP function and must NOT be matched. Give it the name and
        // nothing else to exercise the create-the-nested-object branch.
        let template = json!({
            "Resources": {
                "Fn": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": SERVER }
                }
            }
        })
        .to_string();

        let (parsed, changes, matched) = merged(&template, Some(1024), Some(60));
        assert!(matched);
        assert_eq!(props(&parsed, "Fn")["MemorySize"], json!(1024));
        assert_eq!(props(&parsed, "Fn")["Timeout"], json!(60));
        assert_eq!(
            changes,
            vec![
                "Fn: MemorySize (unset) -> 1024".to_string(),
                "Fn: Timeout (unset) -> 60".to_string(),
            ],
            "an absent property must report `(unset)` as its before value"
        );
    }

    /// (4) Every other property on the matched Lambda is preserved.
    #[test]
    fn other_properties_preserved() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 30,
                        "Runtime": "provided.al2023",
                        "Handler": "bootstrap",
                        "Environment": { "Variables": { "RUST_LOG": "info" } }
                    }
                }
            }
        })
        .to_string();

        let (parsed, _, _) = merged(&template, Some(1024), None);
        let p = props(&parsed, "McpFunction");
        assert_eq!(p["Runtime"], json!("provided.al2023"));
        assert_eq!(p["Handler"], json!("bootstrap"));
        assert_eq!(
            p["Environment"],
            json!({ "Variables": { "RUST_LOG": "info" } })
        );
        assert_eq!(
            p["Timeout"],
            json!(30),
            "an undeclared property must be left exactly as synthesized"
        );
    }

    /// (5) THE REGRESSION GUARD. An OAuth-enabled stack renders THREE Lambdas
    /// at three DIFFERENT sizings — measured from
    /// `crates/pmcp-cfn-renderer/tests/goldens/oauth-cognito-dcr.golden.json`:
    /// `<name>-oauth-proxy` 256/30, `<name>` 512/30, `<name>-authorizer`
    /// 256/**10**. Matching on `Type == "AWS::Lambda::Function"` alone (the way
    /// the `[environment]` merge does) would resize the 10-second authorizer to
    /// the MCP function's timeout — a real regression, not a fix.
    #[test]
    fn discriminates_the_mcp_function_in_a_three_lambda_oauth_stack() {
        let template = json!({
            "Resources": {
                "OAuthProxy": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": format!("{SERVER}-oauth-proxy"),
                        "MemorySize": 256, "Timeout": 30
                    }
                },
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 512, "Timeout": 30
                    }
                },
                "Authorizer": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": format!("{SERVER}-authorizer"),
                        "MemorySize": 256, "Timeout": 10
                    }
                }
            }
        })
        .to_string();

        let (parsed, changes, matched) = merged(&template, Some(3008), Some(900));
        assert!(matched);
        assert_eq!(
            changes,
            vec![
                "McpFunction: MemorySize 512 -> 3008".to_string(),
                "McpFunction: Timeout 30 -> 900".to_string(),
            ],
            "ONLY the MCP function may be reported as changed"
        );

        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(3008));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(900));

        assert_eq!(
            props(&parsed, "OAuthProxy"),
            json!({ "FunctionName": format!("{SERVER}-oauth-proxy"), "MemorySize": 256, "Timeout": 30 }),
            "the OAuth proxy must be byte-preserved"
        );
        assert_eq!(
            props(&parsed, "Authorizer"),
            json!({ "FunctionName": format!("{SERVER}-authorizer"), "MemorySize": 256, "Timeout": 10 }),
            "the 10-second authorizer must be byte-preserved"
        );
    }

    /// (6) Non-Lambda resources are never touched.
    #[test]
    fn non_lambda_resources_untouched() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": SERVER, "MemorySize": 256, "Timeout": 30 }
                },
                "ClientsTable": {
                    "Type": "AWS::DynamoDB::Table",
                    "Properties": { "TableName": SERVER, "MemorySize": 1 }
                }
            }
        })
        .to_string();

        let (parsed, _, _) = merged(&template, Some(1024), Some(60));
        assert_eq!(
            props(&parsed, "ClientsTable"),
            json!({ "TableName": SERVER, "MemorySize": 1 }),
            "a non-Lambda resource must be byte-preserved even when its own \
             properties collide by name and its FunctionName-equivalent matches"
        );
    }

    /// (7) Fail-loud: sizing declared, but no Lambda carries this server's
    /// FunctionName. `matched` stays false (the caller's warning trigger) and
    /// the warning names the server and the declared values.
    #[test]
    fn fail_loud_when_no_matching_lambda() {
        let template = json!({
            "Resources": {
                "SomeoneElse": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": "a-different-server", "MemorySize": 256 }
                }
            }
        })
        .to_string();

        let (parsed, changes, matched) = merged(&template, Some(1024), Some(60));
        assert!(
            !matched,
            "no FunctionName match must yield matched = false (fail-loud trigger)"
        );
        assert!(changes.is_empty());
        assert_eq!(
            props(&parsed, "SomeoneElse")["MemorySize"],
            json!(256),
            "the non-matching Lambda must be left alone"
        );

        let warning = sizing_no_lambda_warning(SERVER, Some(1024), Some(60));
        assert!(warning.contains("NOT applied"), "warning is prominent");
        assert!(
            warning.contains(SERVER),
            "warning names the expected function"
        );
        assert!(
            warning.contains("memory_mb = 1024"),
            "warning names the declared memory"
        );
        assert!(
            warning.contains("timeout_seconds = 60"),
            "warning names the declared timeout"
        );
    }

    /// (8) A partial declaration touches only the declared property.
    #[test]
    fn partial_declaration_leaves_the_other_property_alone() {
        let (parsed, changes, _) = merged(&scaffold_template(), None, Some(120));
        assert_eq!(
            changes,
            vec!["McpFunction: Timeout 30 -> 120".to_string()],
            "only the declared property may be reported"
        );
        assert_eq!(
            props(&parsed, "McpFunction")["MemorySize"],
            json!(256),
            "an undeclared memory_mb must leave the engine's own value in place"
        );
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(120));
    }

    /// (9) Idempotent: a template already carrying the declared values is
    /// matched with ZERO changes. `matched` and `changes.is_empty()` must stay
    /// distinct signals — collapsing them would send an already-correct
    /// re-deploy down the fail-loud path.
    #[test]
    fn idempotent_when_already_correct() {
        let (parsed, changes, matched) = merged(&scaffold_template(), Some(256), Some(30));
        assert!(matched, "an already-correct template is still a MATCH");
        assert!(
            changes.is_empty(),
            "no property moved, so nothing may be reported as changed"
        );
        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(256));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(30));
    }

    /// (10) THE ENGINE-DIVERGENCE FIXTURE (and_gate condition C). Before this
    /// fix the two pmcp-run synth engines emitted DIFFERENT timeouts for the
    /// SAME `deploy.toml`: `pmcp-cfn-renderer` threaded
    /// `timeout_seconds: d.server.timeout_seconds`, while the TypeScript
    /// scaffold hardcoded `cdk.Duration.seconds(30)` — so whether a human had
    /// ever touched `stack.ts` (which routes the deploy to `npx cdk synth`)
    /// silently decided the deployed Timeout.
    ///
    /// The whole test corpus was blind to this because every golden fixture
    /// declares the DEFAULT `timeout_seconds = 30`, where the two engines
    /// agree by coincidence. This is that masking removed: a NON-default 60,
    /// asserted to converge from BOTH engine outputs.
    #[test]
    fn both_synth_engines_converge_on_the_declared_timeout() {
        // What `npx cdk synth` produces: the stack.ts literal, 30.
        let legacy_cdk = scaffold_template();
        // What `pmcp-cfn-renderer` produces: the descriptor's 60 for Timeout,
        // but still its own module const for MemorySize.
        let renderer = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 60,
                        "Runtime": "provided.al2023"
                    }
                }
            }
        })
        .to_string();

        let (from_legacy, legacy_changes, _) = merged(&legacy_cdk, Some(1024), Some(60));
        let (from_renderer, renderer_changes, _) = merged(&renderer, Some(1024), Some(60));

        assert_eq!(
            from_legacy["Resources"]["McpFunction"]["Properties"]["Timeout"],
            json!(60)
        );
        assert_eq!(
            from_renderer["Resources"]["McpFunction"]["Properties"]["Timeout"],
            json!(60)
        );
        assert_eq!(
            from_legacy, from_renderer,
            "the two synth engines must land on IDENTICAL sizing for the same deploy.toml"
        );

        // And the divergence is visible in what each engine needed corrected:
        // only the legacy path had a Timeout to move.
        assert!(
            legacy_changes.contains(&"McpFunction: Timeout 30 -> 60".to_string()),
            "the cdk-synth engine's hardcoded 30 must be corrected, got {legacy_changes:?}"
        );
        assert!(
            !renderer_changes
                .iter()
                .any(|c| c.starts_with("McpFunction: Timeout")),
            "the renderer already honored the descriptor timeout, got {renderer_changes:?}"
        );
    }

    /// (11) `is_mcp_lambda_function` matches on BOTH the CFN type and the
    /// FunctionName — neither alone is sufficient.
    #[test]
    fn is_mcp_lambda_function_requires_type_and_name() {
        assert!(is_mcp_lambda_function(
            &json!({ "Type": "AWS::Lambda::Function", "Properties": { "FunctionName": SERVER } }),
            SERVER
        ));
        assert!(
            !is_mcp_lambda_function(
                &json!({ "Type": "AWS::Lambda::Url", "Properties": { "FunctionName": SERVER } }),
                SERVER
            ),
            "the right name on the wrong type must not match"
        );
        assert!(
            !is_mcp_lambda_function(
                &json!({ "Type": "AWS::Lambda::Function", "Properties": { "FunctionName": "other" } }),
                SERVER
            ),
            "the right type with the wrong name must not match"
        );
        assert!(
            !is_mcp_lambda_function(
                &json!({ "Type": "AWS::Lambda::Function", "Properties": {} }),
                SERVER
            ),
            "a missing FunctionName must not match"
        );
    }

    /// (12) WIRING GUARD — proves both post-synth merges are actually reached
    /// in production, not merely unit-tested in isolation.
    ///
    /// `deploy_to_pmcp_run` cannot be asserted from a unit test (it needs OAuth
    /// credentials, presigned S3 URLs and a live GraphQL endpoint), which is
    /// why the merge chain was extracted into `apply_post_synth_merges`.
    /// Deleting either call there fails THIS test. Without it, a correct
    /// `merge_sizing_into_template` that nothing ever calls would still show a
    /// fully green suite — which is the exact shape of the original bug:
    /// `memory_mb` parsed fine and was read by zero production code paths.
    #[test]
    fn post_synth_merges_apply_environment_and_sizing() {
        use super::apply_post_synth_merges;

        let mut config = crate::deployment::config::DeployConfig::default_for_server(
            SERVER.to_string(),
            "us-east-1".to_string(),
            std::path::PathBuf::from("/tmp/pmcp-run-post-synth-merges"),
        );
        config.server.memory_mb = Some(1024);
        config.server.timeout_seconds = Some(120);
        config
            .environment
            .insert("FEATURE_FLAG".to_string(), "on".to_string());

        let out = apply_post_synth_merges(scaffold_template(), &config)
            .expect("post-synth merges succeed on a well-formed template");
        let parsed: Value = serde_json::from_str(&out).expect("merged template is valid JSON");
        let p = props(&parsed, "McpFunction");

        assert_eq!(
            p["Environment"]["Variables"]["FEATURE_FLAG"],
            json!("on"),
            "the [environment] merge must still be wired into the deploy path"
        );
        assert_eq!(
            p["MemorySize"],
            json!(1024),
            "the [server] sizing merge must be wired into the deploy path"
        );
        assert_eq!(p["Timeout"], json!(120));
    }

    /// (12) Invalid template JSON surfaces a parse error rather than silently
    /// dropping the merge.
    #[test]
    fn invalid_template_json_errors() {
        let err = merge_sizing_into_template("{ not valid json", SERVER, Some(1024), None)
            .expect_err("invalid JSON must error");
        assert!(
            err.to_string().contains("parse synthesized CloudFormation"),
            "error must name the parse failure"
        );
    }
}
