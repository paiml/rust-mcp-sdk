pub mod artifact;
mod deploy;
pub(crate) mod engine;
pub mod init;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::process::Command;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    BinaryBuilder, DeployConfig,
};

pub struct AwsLambdaTarget;

impl AwsLambdaTarget {
    pub fn new() -> Self {
        Self
    }
}

/// Build Lambda binary - can be reused by other targets
pub async fn build_lambda_binary(config: &DeployConfig) -> Result<BuildArtifact> {
    println!("🔨 Building Rust binary for AWS Lambda...");

    let builder = BinaryBuilder::new(config.project_root.clone());
    let result = builder.build()?;

    Ok(BuildArtifact::Binary {
        path: result.binary_path,
        size: result.binary_size,
        deployment_package: result.deployment_package,
    })
}

impl Default for AwsLambdaTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeploymentTarget for AwsLambdaTarget {
    fn id(&self) -> &str {
        "aws-lambda"
    }

    fn name(&self) -> &str {
        "AWS Lambda"
    }

    fn description(&self) -> &str {
        "Deploy to AWS Lambda with API Gateway using CDK"
    }

    async fn is_available(&self) -> Result<bool> {
        // Task 8 (shape-aware artifact acquisition): the `npx cdk` probe is
        // dropped unconditionally — the T7 CFN-renderer extraction replaced
        // `cdk synth` on the normal path, and CDK was never required for a
        // built-in (config-only) deploy in the first place. `cargo-lambda` is
        // no longer a universal requirement either: it is a SHAPE-dependent
        // tool (only custom-Rust projects need it), and this trait method
        // has no `DeployConfig` to run `artifact::detect_shape` against — it
        // is used for coarse target-listing before a project is even known
        // (see `TargetRegistry::list_available`). The real, blocking probe
        // happens once the shape IS known: `artifact::acquire_custom_rust_artifact`
        // delegates to `build_lambda_binary` -> `BinaryBuilder::build()`,
        // which already calls `ensure_cargo_lambda()` first and bails with
        // an actionable install message. So `aws-lambda` is always
        // structurally available; built-in servers need zero dev tooling at
        // all, and custom-Rust servers get their cargo-lambda check later,
        // where it can name the actual requirement instead of guessing.
        Ok(true)
    }

    async fn prerequisites(&self) -> Vec<String> {
        // See `is_available`'s doc comment: no universal prerequisite exists
        // any more. Shape-dependent tooling (cargo-lambda, custom-Rust only)
        // is checked once `artifact::detect_shape` resolves the project's
        // shape, at build time.
        Vec::new()
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        init::init_aws_lambda(config).await
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        // Task 8: route through shape-aware acquisition. Built-in
        // (config-only) projects fetch a prebuilt published binary with zero
        // dev tooling; custom-Rust projects keep the existing cargo-lambda
        // pipeline via `artifact::acquire_custom_rust_artifact`'s delegation
        // to `build_lambda_binary` below — identical behavior to before this
        // wiring, just always wrapped in a zip.
        //
        // Deliberate recompute (simplify-wave item 10): `deploy()`
        // (`deploy.rs::try_render_and_deploy`) calls `detect_shape` again
        // rather than this call's result being threaded through
        // `BuildArtifact` — that enum is shared across every deploy target,
        // so adding an `aws-lambda`-only shape field would ripple into all
        // of them. Recomputing is cheap (two file-existence checks over an
        // immutable `config`), so the duplication is intentional.
        let shape = artifact::detect_shape(config)?;
        let zip_path = artifact::acquire_artifact(&shape, config).await?;
        let size = std::fs::metadata(&zip_path)
            .with_context(|| format!("failed to stat {}", zip_path.display()))?
            .len();
        Ok(BuildArtifact::Binary {
            path: zip_path.clone(),
            size,
            deployment_package: Some(zip_path),
        })
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        // Task 9 (CFN deploy engine): `artifact` (already acquired by
        // `build()` above, via `artifact::acquire_artifact`) is now threaded
        // through so the renderer+engine path can deploy it directly without
        // a second (and, for a `ServerShape::BuiltIn` project, IMPOSSIBLE —
        // no Cargo.toml to rebuild from) build. `deploy_env_vars()` (merged
        // `[environment]` + resolved `[secrets]`, secrets win) is still
        // threaded through for the legacy `DeployExecutor` fallback branch
        // ONLY — see `deploy::deploy_aws_lambda`'s doc comment for why the
        // renderer path does not use it.
        deploy::deploy_aws_lambda(config, artifact, config.deploy_env_vars()).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        let deploy_dir = config.project_root.join("deploy");

        if !deploy_dir.exists() {
            println!("⚠️  No deployment found (deploy/ directory missing)");
            return Ok(());
        }

        println!("🗑️  Destroying AWS resources...");
        println!();

        let stack_name = format!("{}-stack", config.server.name);

        let status = Command::new("npx")
            .args(&["cdk", "destroy", &stack_name, "--force"])
            .current_dir(&deploy_dir)
            .status()
            .context("Failed to run CDK destroy")?;

        if !status.success() {
            bail!("CDK destroy failed");
        }

        println!();
        println!("✅ AWS resources destroyed successfully");

        if clean {
            println!();
            println!("🧹 Cleaning up local deployment files...");

            // Remove deploy directory
            if deploy_dir.exists() {
                std::fs::remove_dir_all(&deploy_dir)
                    .context("Failed to remove deploy/ directory")?;
                println!("   ✓ Removed deploy/");
            }

            // Remove Lambda wrapper directory
            let lambda_dir = config
                .project_root
                .join(format!("{}-lambda", config.server.name));
            if lambda_dir.exists() {
                std::fs::remove_dir_all(&lambda_dir)
                    .context("Failed to remove Lambda wrapper directory")?;
                println!("   ✓ Removed {}-lambda/", config.server.name);
            }

            // Remove deployment config
            let config_file = config.project_root.join(".pmcp/deploy.toml");
            if config_file.exists() {
                std::fs::remove_file(&config_file).context("Failed to remove .pmcp/deploy.toml")?;
                println!("   ✓ Removed .pmcp/deploy.toml");
            }

            println!();
            println!("✅ All deployment files removed");
        }

        Ok(())
    }

    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs> {
        let stack_name = format!("{}-stack", config.server.name);
        crate::deployment::load_cdk_outputs(&config.project_root, &config.aws().region, &stack_name)
    }

    async fn logs(&self, _config: &DeployConfig, _tail: bool, _lines: usize) -> Result<()> {
        println!("🔄 Log streaming coming in Phase 2!");
        Ok(())
    }

    async fn metrics(&self, _config: &DeployConfig, period: &str) -> Result<MetricsData> {
        println!("🔄 Metrics dashboard coming in Phase 2!");
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
        println!("🔄 Secrets management coming in Phase 2!");
        Ok(())
    }

    async fn test(&self, _config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        println!("🔄 Deployment testing coming in Phase 2!");
        Ok(TestResults {
            success: true,
            tests_run: 0,
            tests_passed: 0,
            failures: vec![],
        })
    }

    async fn rollback(&self, _config: &DeployConfig, version: Option<&str>) -> Result<()> {
        println!("🔄 Rollback functionality coming in Phase 2!");
        println!(
            "   This will rollback to version: {}",
            version.unwrap_or("previous")
        );
        Ok(())
    }
}
