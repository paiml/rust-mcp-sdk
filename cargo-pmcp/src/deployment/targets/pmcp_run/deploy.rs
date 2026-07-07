use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::deployment::{
    metadata::McpMetadata,
    r#trait::{BuildArtifact, DeploymentOutputs},
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

    // Paths
    let deploy_dir = config.project_root.join("deploy");
    let cdk_out = deploy_dir.join("cdk.out");

    // Step 0: Extract MCP metadata for the CloudFormation template, then apply
    // the operator `[metadata]` override (DSTK-02/DSTK-03) so config-declared
    // server_type / snapshot_baked reach the synth context.
    let metadata = extract_metadata_with_log(&config.project_root).map(|mut m| {
        m.apply_config_overrides(&config.metadata);
        m
    });

    // Step 1: Synthesize CloudFormation template with metadata context.
    // FIX #2 (deploy-toml-inert-for-preserved-stack): pass developer-declared
    // [environment] to the `cdk synth` child process — the pmcp-run equivalent
    // of the aws-lambda `extra_env` path. The stack.ts consumes matching
    // process.env.<KEY> reads, so [environment] is no longer globally inert.
    // Secrets are intentionally NOT passed here (pmcp.run injects them
    // server-side per D-08); only non-sensitive [environment] flows to synth.
    println!("📝 Synthesizing CloudFormation template...");
    run_cdk_synth(&deploy_dir, metadata.as_ref(), &config.environment)?;
    println!("✅ CloudFormation template synthesized");

    // Step 2: Find the synthesized template
    let template_path = find_template_file(&cdk_out)?;
    println!("   Template: {}", template_path.display());

    // Step 3: Extract bootstrap data + content-type from the build artifact.
    let upload = read_bootstrap_upload(artifact)?;
    println!();

    // Step 4: Read template file
    let template = std::fs::read_to_string(&template_path)
        .context("Failed to read CloudFormation template")?;

    // Step 4b: Construct-agnostic `[environment]` delivery
    // (`environment-inert-for-shared-cdk-constructs`). FIX #2 exported
    // `[environment]` only as `process.env` to the `cdk synth` child, so
    // shared/managed constructs that ignore `process.env` (e.g.
    // `OpenApiMcpServerStack`) silently dropped the declared keys. Merge the
    // declared `[environment]` directly into every `AWS::Lambda::Function`'s
    // `Environment.Variables` in the synthesized template — construct-agnostic,
    // guaranteed delivery regardless of how the stack.ts was authored. Secrets
    // are EXCLUDED (they keep their server-side injection path per D-08).
    // Precedence: `[environment]` OVERRIDES a construct's hardcoded value on
    // key collision (locked product decision).
    let template = apply_environment_merge(template, &config.environment, &config.secrets)?;

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

/// Extract MCP metadata and log what was found. Returns None when the project
/// has no metadata (defaults apply).
fn extract_metadata_with_log(project_root: &Path) -> Option<McpMetadata> {
    println!("📋 Extracting MCP server metadata...");
    match McpMetadata::extract(project_root) {
        Ok(m) => {
            println!("   Server: {} ({})", m.server_id, m.server_type);
            if !m.resources.secrets.is_empty() {
                println!("   Secrets: {}", m.resources.secrets.len());
            }
            if !m.capabilities.tools.is_empty() {
                println!("   Tools: {}", m.capabilities.tools.len());
            }
            Some(m)
        },
        Err(_) => {
            println!("   No metadata found (using defaults)");
            None
        },
    }
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
fn find_template_file(cdk_out: &PathBuf) -> Result<PathBuf> {
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
        if let Some(warning) = crate::deployment::config::stack_ts_preserved_inert_warning(
            config.iam.is_empty(),
            config.environment.is_empty(),
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
