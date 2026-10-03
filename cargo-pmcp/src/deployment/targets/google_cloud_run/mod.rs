//! Google Cloud Run deployment target.
//!
//! # Operator workflow
//!
//! Closes upstream issues paiml/rust-mcp-sdk#258 (multi-crate isolated
//! layout), #259 (distroless runtime default), and #260 (deploy.toml
//! schema). The canonical workflow for the pmcp.run team:
//!
//! ```bash
//! # 1. Scaffold the deployment artifacts (Dockerfile, .dockerignore,
//! #    cloudbuild.yaml, .pmcp/deploy.toml). Idempotent — re-running
//! #    keeps every existing file (delete one to regenerate it). New files
//! #    get [gcp] region = "us-central1" unless --region says otherwise.
//! cargo pmcp deploy init --target-type google-cloud-run
//!
//! # 2. Edit .pmcp/deploy.toml to fill in the GCP project, region, and
//! #    any [environment] keys your server requires at startup. For a
//! #    multi-crate isolated layout (e.g. a non-workspace sibling-crate
//! #    test harness), add the [layout] block.
//! cat .pmcp/deploy.toml
//! ```
//!
//! ## Minimum-viable deploy.toml for Cloud Run
//!
//! ```toml
//! [target]
//! type = "google-cloud-run"
//! version = "1.0.0"
//!
//! [gcp]
//! project_id = "my-gcp-project"
//! region = "us-central1"
//! repository = "pmcp"          # Artifact Registry; absent = gcr.io (pre-0.28.0 files)
//!
//! [server]
//! name = "auth-echo-cloud-run"
//! mcp_path = "/mcp"            # the default; "/" for a server mounted at the root
//! memory = "256Mi"
//! cpu = "1"
//! ingress = "all"
//! allow_unauthenticated = true
//!
//! [environment]
//! EXPECTED_AUDIENCE = "abc.apps.googleusercontent.com"
//! RUST_LOG = "info"
//! ```
//!
//! ## Which binary the image runs
//!
//! The workspace and single-crate Dockerfiles build ONE binary and copy it
//! by name (`cargo build --release [-p <package>] --bin <binary>`, then
//! `cp target/release/<binary>`). `<binary>` is `[server] binary`; when it
//! is unset, `deploy init` picks the project's only non-Lambda binary (not
//! `bootstrap`, no `lambda` word in its name) from `cargo metadata
//! --no-deps`, records it in a new deploy.toml, and otherwise stops with the
//! candidates (see `binary.rs`). A crate shaped like package `x-lambda` with
//! bins `bootstrap` + `serve` builds `serve`.
//!
//! ## Multi-crate isolated layout (issue #258)
//!
//! For non-workspace sibling crates with path-dep relationships (e.g.
//! a Cloud Run binary crate that declares
//! `auth-echo-core = { path = "../auth-echo-core" }`), add a `[layout]`
//! block. The scaffolder emits surgical per-crate `COPY` lines in the
//! Dockerfile and `cargo build --manifest-path <primary>/Cargo.toml
//! --bin <binary>` instead of `COPY . .` (which would over-bundle any
//! sibling `aws-lambda` crates that intentionally sit outside the
//! workspace).
//!
//! ```toml
//! [layout]
//! kind = "multi-crate-isolated"
//! primary = "gcp-cloud-run"
//! path_deps = ["auth-echo-core"]
//!
//! [server]
//! name = "auth-echo-cloud-run"
//! binary = "server"  # passed as `cargo build --bin server`
//! ```
//!
//! ## Distroless runtime + opt-out (issue #259)
//!
//! The default runtime FROM image is `gcr.io/distroless/cc-debian12`
//! (~20 MB, no shell, no apt, no package manager) — the right default
//! for the cargo-pmcp toolchain shape (rustls is pinned, so no system
//! libssl is needed at runtime). Opt back to a shell-enabled base by
//! setting `[runtime].base`. Declarative apt packages are honored only
//! when the base resolves to a debian-family image:
//!
//! ```toml
//! [runtime]
//! base = "debian:bookworm-slim"
//! apt_packages = ["ca-certificates", "libssl3"]
//! ```
//!
//! ## Deploy
//!
//! ```bash
//! gcloud auth login
//! gcloud config set project my-gcp-project
//! cargo pmcp deploy --target google-cloud-run
//! ```
//!
//! All `gcloud run deploy` flags (`--memory`, `--cpu`,
//! `--allow-unauthenticated`, `--ingress`, `--set-env-vars`, etc.) are
//! sourced from `deploy.toml`. The previous workflow of patching
//! env vars via `gcloud run services update --set-env-vars` after
//! every deploy is no longer required (#260).
//!
//! Before building, the deploy refuses a dependency with an absolute `path`
//! (it does not exist inside the Docker build context). The manifests are
//! parsed, so comments never count (see `manifest.rs`).
//!
//! The image goes to the Artifact Registry repository `[gcp] repository`,
//! created on first deploy, or to `gcr.io` when the key is absent (see
//! `image.rs`). The post-deploy verification probes `<service URL>/mcp`
//! unless `[server] mcp_path` says otherwise (`deployment::mcp_endpoint`).

mod auth;
mod binary;
mod deploy;
mod dockerfile;
mod env;
#[cfg(test)]
mod fake_cloud;
#[cfg(test)]
pub mod fixture;
mod image;
mod init;
mod manifest;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

pub use image::validate_repository;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    DeployConfig,
};

// Auth functions are used internally

pub struct GoogleCloudRunTarget;

impl GoogleCloudRunTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoogleCloudRunTarget {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve `(project_id, region)` for Cloud Run lifecycle ops.
///
/// Mirrors `deploy::resolve_params`'s precedence so `destroy`/`outputs`/`logs`/
/// `metrics` target the SAME project and region as the preceding `deploy()` —
/// previously these methods ignored `config.gcp` and hit the ambient `gcloud
/// config get-value project` + `CLOUD_RUN_REGION` env var, which silently
/// targets the wrong project whenever the gcloud CLI is logged into a
/// different account than the operator's deploy.toml declares (review
/// finding #2 / n51 follow-up).
///
/// Precedence:
/// 1. `config.gcp.project_id` / `config.gcp.region` (deploy.toml — source of truth)
///    — skipping empty strings and the placeholder `"your-gcp-project-id"`
/// 2. `CLOUD_RUN_REGION` env var (region only — gcloud has no analogous env
///    for project_id)
/// 3. `gcloud config get-value project` / `run/region` (ambient gcloud)
fn resolve_project_and_region(config: &DeployConfig) -> Result<(String, String)> {
    let project_id = config
        .gcp
        .as_ref()
        .map(|g| g.project_id.clone())
        .filter(|p| !p.is_empty() && p != "your-gcp-project-id")
        .map_or_else(auth::get_project_id, Ok)?;
    let region = config
        .gcp
        .as_ref()
        .map(|g| g.region.clone())
        .filter(|r| !r.is_empty())
        .or_else(|| std::env::var("CLOUD_RUN_REGION").ok())
        .unwrap_or_else(auth::get_region);
    Ok((project_id, region))
}

#[async_trait]
impl DeploymentTarget for GoogleCloudRunTarget {
    fn id(&self) -> &str {
        "google-cloud-run"
    }

    fn name(&self) -> &str {
        "Google Cloud Run"
    }

    fn description(&self) -> &str {
        "Deploy to Google Cloud Run (managed containers)"
    }

    async fn is_available(&self) -> Result<bool> {
        // Check for required tools
        let has_docker = std::process::Command::new("docker")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_gcloud = std::process::Command::new("gcloud")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(has_docker && has_gcloud)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check Docker
        if !std::process::Command::new("docker")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("Docker (install: https://docs.docker.com/get-docker/)".to_string());
        }

        // Check gcloud CLI
        if !std::process::Command::new("gcloud")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push(
                "Google Cloud SDK (install: https://cloud.google.com/sdk/docs/install)".to_string(),
            );
        }

        // Check authentication
        if auth::check_gcloud_auth().is_err() {
            missing.push("Google Cloud authentication (run: gcloud auth login)".to_string());
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        init::init_google_cloud_run(config)
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        println!("🔨 Building container image for Google Cloud Run...");

        // The build happens during deploy for Cloud Run (Docker build)
        // Return a placeholder artifact
        Ok(BuildArtifact::Custom {
            path: config.project_root.clone(),
            artifact_type: "docker".to_string(),
            deployment_package: None, // Cloud Run uses Docker image, not deployment packages
        })
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        _artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        deploy::deploy_to_cloud_run(config).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        println!("🗑️  Destroying Google Cloud Run deployment...");
        println!();

        // Source-of-truth: deploy.toml [gcp]. Falls back to gcloud ambient
        // only when [gcp] is absent/empty — matches the deploy() precedence.
        let (project_id, region) = resolve_project_and_region(config)?;
        let service_name = &config.server.name;

        // Delete Cloud Run service
        let output = std::process::Command::new("gcloud")
            .args(&[
                "run",
                "services",
                "delete",
                service_name,
                "--region",
                &region,
                "--project",
                &project_id,
                "--quiet",
            ])
            .output()
            .context("Failed to run gcloud run services delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to delete Cloud Run service:\n{}", stderr);
        }

        println!("✅ Cloud Run service deleted successfully");

        if clean {
            println!();
            println!("🧹 Cleaning up local files...");

            // Remove generated files
            let files_to_remove = vec!["Dockerfile", ".dockerignore", "cloudbuild.yaml"];
            for file in files_to_remove {
                let path = config.project_root.join(file);
                if path.exists() {
                    std::fs::remove_file(&path).context(format!("Failed to remove {}", file))?;
                    println!("   ✓ Removed {}", file);
                }
            }

            println!();
            println!("✅ All deployment files removed");
        }

        Ok(())
    }

    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs> {
        let (project_id, region) = resolve_project_and_region(config)?;
        let service_name = &config.server.name;

        // Get service URL
        let output = std::process::Command::new("gcloud")
            .args(&[
                "run",
                "services",
                "describe",
                service_name,
                "--region",
                &region,
                "--project",
                &project_id,
                "--format",
                "value(status.url)",
            ])
            .output()
            .context("Failed to get Cloud Run service URL")?;

        if !output.status.success() {
            bail!("Failed to get Cloud Run service information");
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(DeploymentOutputs {
            url: Some(url),
            additional_urls: vec![],
            regions: vec![region],
            stack_name: Some(service_name.clone()),
            version: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn logs(&self, config: &DeployConfig, tail: bool, lines: usize) -> Result<()> {
        // `gcloud logging read` filters by the service-name label, not by region,
        // so region is bound to `_region` (computed for parity with deploy/destroy
        // precedence; future change can promote it into the filter).
        let (project_id, _region) = resolve_project_and_region(config)?;
        let service_name = &config.server.name;

        println!("📜 Fetching logs from Google Cloud Run...");
        println!();

        let mut args = vec!["logs", "read", "--project", &project_id];

        let filter = format!(
            "resource.type=cloud_run_revision AND resource.labels.service_name={}",
            service_name
        );
        let limit_str = lines.to_string();

        args.extend(&["--filter", &filter]);
        args.extend(&["--limit", &limit_str]);

        if tail {
            args.push("--tail");
        }

        let output = std::process::Command::new("gcloud")
            .args(&args)
            .output()
            .context("Failed to fetch logs")?;

        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to fetch logs:\n{}", stderr);
        }

        Ok(())
    }

    async fn metrics(&self, _config: &DeployConfig, period: &str) -> Result<MetricsData> {
        // metrics() is a stub today (just prints a Cloud Console URL); when it
        // grows a real implementation it should call resolve_project_and_region
        // to stay consistent with destroy/outputs/logs. The previous version
        // bound `_project_id`/`_region`/`_service_name` via the gcloud-ambient
        // path, which silently masked the wrapper-bypass bug — dropped here.
        println!("📊 Google Cloud Run metrics available in Cloud Console");
        println!("   View at: https://console.cloud.google.com/run");

        Ok(MetricsData {
            period: period.to_string(),
            requests: None,
            errors: None,
            avg_latency_ms: None,
            p99_latency_ms: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn secrets(&self, _config: &DeployConfig, _action: SecretsAction) -> Result<()> {
        println!("🔐 Use Google Secret Manager for secrets:");
        println!("   1. Create secret: gcloud secrets create SECRET_NAME --data-file=-");
        println!("   2. Grant access: gcloud secrets add-iam-policy-binding SECRET_NAME \\");
        println!("      --member=serviceAccount:SERVICE_ACCOUNT --role=roles/secretmanager.secretAccessor");
        println!("   3. Mount in Cloud Run via --set-secrets flag");

        Ok(())
    }

    async fn test(&self, config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        println!("🧪 Testing Google Cloud Run deployment...");

        let outputs = self.outputs(config).await?;

        if let Some(url) = outputs.url {
            println!("   Testing endpoint: {}", url);

            let response = reqwest::get(&url).await?;
            let success = response.status().is_success();

            if success {
                println!("✅ Deployment is healthy");
            } else {
                println!("❌ Deployment returned error: {}", response.status());
            }

            Ok(TestResults {
                success,
                tests_run: 1,
                tests_passed: if success { 1 } else { 0 },
                failures: vec![],
            })
        } else {
            bail!("No deployment URL found");
        }
    }

    async fn rollback(&self, _config: &DeployConfig, version: Option<&str>) -> Result<()> {
        println!("🔄 Cloud Run rollback:");
        println!("   Use: gcloud run services update-traffic SERVICE_NAME \\");
        println!(
            "        --to-revisions=REVISION={}",
            version.unwrap_or("100")
        );

        Ok(())
    }
}

/// Tests for `resolve_project_and_region` — guards n51 follow-up #2: lifecycle
/// methods MUST read deploy.toml `[gcp]`, not the ambient gcloud config.
///
/// These tests are env-mutating (CLOUD_RUN_REGION); CI runs `--test-threads=1`
/// so serialization is guaranteed. Each test save/restores the env var so
/// failures don't poison subsequent tests in the same process.
#[cfg(test)]
mod resolve_project_and_region_tests {
    use super::resolve_project_and_region;
    use crate::deployment::config::DeployConfig;
    use std::path::PathBuf;

    /// Save+restore guard for an env var across a single test.
    struct EnvGuard {
        key: &'static str,
        saved: Option<String>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let saved = std::env::var(key).ok();
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

    fn cfg_with_gcp(project_id: &str, region: &str) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "test-svc".to_string(),
            project_id.to_string(),
            region.to_string(),
            PathBuf::from("/tmp/test"),
        )
    }

    /// Happy path: config.gcp fully populated → returned verbatim, never
    /// reaches gcloud or env-var fallback.
    #[test]
    fn prefers_config_gcp_over_env() {
        let _g = EnvGuard::set("CLOUD_RUN_REGION", Some("us-east-1"));
        let cfg = cfg_with_gcp("prod-billing", "europe-west1");
        let (project, region) =
            resolve_project_and_region(&cfg).expect("must resolve from config.gcp");
        assert_eq!(project, "prod-billing");
        assert_eq!(
            region, "europe-west1",
            "config.gcp.region wins over CLOUD_RUN_REGION env var"
        );
    }

    /// region empty in config.gcp + CLOUD_RUN_REGION set → falls back to env.
    /// project_id stays from config.gcp (still populated).
    #[test]
    fn region_falls_back_to_env_when_gcp_region_empty() {
        let _g = EnvGuard::set("CLOUD_RUN_REGION", Some("us-east-1"));
        let mut cfg = cfg_with_gcp("prod-billing", "");
        assert_eq!(cfg.gcp.as_ref().map(|g| g.region.as_str()), Some(""));
        // Sanity: ensure region is truly empty post-construction.
        cfg.gcp.as_mut().unwrap().region = String::new();
        let (project, region) =
            resolve_project_and_region(&cfg).expect("must resolve via env fallback");
        assert_eq!(project, "prod-billing");
        assert_eq!(region, "us-east-1");
    }

    /// Both env and config.gcp.region set → config wins (precedence).
    #[test]
    fn config_gcp_region_wins_over_env() {
        let _g = EnvGuard::set("CLOUD_RUN_REGION", Some("us-east-1"));
        let cfg = cfg_with_gcp("p", "asia-northeast1");
        let (_, region) = resolve_project_and_region(&cfg).expect("must resolve");
        assert_eq!(
            region, "asia-northeast1",
            "config.gcp.region is highest precedence"
        );
    }

    /// Placeholder project_id `"your-gcp-project-id"` is rejected (treated as
    /// empty so the gcloud fallback fires). We can't easily assert the gcloud
    /// outcome (it shells out), so this test verifies the placeholder is at
    /// LEAST not blindly returned.
    #[test]
    fn rejects_placeholder_project_id() {
        let cfg = cfg_with_gcp("your-gcp-project-id", "us-central1");
        let result = resolve_project_and_region(&cfg);
        // If gcloud is configured, result is Ok with project != placeholder.
        // If gcloud is unconfigured, result is Err. Either way, the literal
        // "your-gcp-project-id" must NOT appear as the returned project_id.
        if let Ok((project, _)) = result {
            assert_ne!(
                project, "your-gcp-project-id",
                "placeholder must be rejected, falling through to gcloud or Err"
            );
        }
    }
}
