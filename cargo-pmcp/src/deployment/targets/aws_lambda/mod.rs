pub mod artifact;
mod deploy;
pub(crate) mod engine;
pub mod init;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::ffi::OsStr;
use std::process::Command;

use crate::deployment::{
    cdk_stack_guard::{ensure_app_declares_stack, expected_stack_name, CdkOperation},
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

/// Run `cdk destroy {[server] name}-stack --force` in the project's `deploy/`
/// directory, spawned through `npx` (`npx` is a parameter only so tests can
/// substitute a recording stand-in for it).
///
/// Guarded first: `cdk destroy` matches no stack and still exits 0 when the
/// CDK app does not declare the name (aws-cdk issue #27179), which made this
/// command report success, and `--clean` delete the local deploy files, while
/// the stack kept running. See [`crate::deployment::cdk_stack_guard`].
fn destroy_stack(npx: &OsStr, config: &DeployConfig) -> Result<()> {
    let deploy_dir = config.project_root.join("deploy");
    let stack_name = expected_stack_name(&config.server.name);

    // Same environment as the `cdk destroy` below (none added), so the guard
    // lists exactly the app that destroy would synthesize.
    let mut list = Command::new(npx);
    list.args(["cdk", "list"]).current_dir(&deploy_dir);
    ensure_app_declares_stack(
        list,
        &config.server.name,
        CdkOperation::Destroy,
        &config.aws().region,
    )?;

    let status = Command::new(npx)
        .args(["cdk", "destroy", &stack_name, "--force"])
        .current_dir(&deploy_dir)
        .status()
        .context("Failed to run CDK destroy")?;

    if !status.success() {
        bail!("CDK destroy failed");
    }
    Ok(())
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

        destroy_stack(OsStr::new("npx"), config)?;

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

/// Stack-identity guard on `destroy` (debug session `cargo-pmcp-deploy-targets`,
/// guard PR). `cdk destroy <name>` matches no stack and exits 0 when the CDK app
/// does not declare `<name>` (aws-cdk issue #27179), so without the guard
/// `destroy` reported success, and `--clean` deleted the local deploy files,
/// while the stack kept running.
#[cfg(all(test, unix))]
mod destroy_guard_tests {
    use super::*;
    use crate::deployment::fake_npx::FakeNpx;

    fn config(root: &std::path::Path) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(
            "acme".to_string(),
            "eu-west-2".to_string(),
            root.to_path_buf(),
        );
        cfg.target.target_type = "aws-lambda".to_string();
        cfg
    }

    #[test]
    fn destroy_refuses_when_app_declares_another_stack() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy")).expect("create deploy/");
        let npx = FakeNpx::new(&tmp.path().join("bin"), "old-server-stack\n", "", 0);

        let result = destroy_stack(npx.program().as_os_str(), &config(tmp.path()));

        let err = format!(
            "{:#}",
            result.expect_err("a stack mismatch must refuse destroy")
        );
        assert!(
            err.contains("acme-stack"),
            "error must name the expected stack: {err}"
        );
        assert!(
            err.contains("old-server-stack"),
            "error must name the declared stack: {err}"
        );
        assert!(
            err.contains(
                "aws cloudformation delete-stack --stack-name acme-stack --region eu-west-2"
            ),
            "error must give the direct recovery command: {err}"
        );
        assert!(
            !npx.ran("cdk destroy"),
            "cdk destroy must never run on a mismatch; calls: {:?}",
            npx.calls()
        );
    }

    #[test]
    fn destroy_runs_cdk_destroy_for_the_declared_stack() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("deploy")).expect("create deploy/");
        let npx = FakeNpx::new(&tmp.path().join("bin"), "acme-stack\n", "", 0);

        destroy_stack(npx.program().as_os_str(), &config(tmp.path()))
            .expect("matching stack destroys");

        assert_eq!(
            npx.argv_lines(),
            vec![
                "cdk list".to_string(),
                "cdk destroy acme-stack --force".to_string()
            ],
            "full call log: {:?}",
            npx.calls()
        );
    }
}
