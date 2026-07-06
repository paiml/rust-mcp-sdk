mod deploy;
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
        // Check for required tools
        let has_cdk = Command::new("npx")
            .args(&["cdk", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_cargo_lambda = Command::new("cargo")
            .args(&["lambda", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(has_cdk && has_cargo_lambda)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check CDK
        if !Command::new("npx")
            .args(&["cdk", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("AWS CDK (install: npm install -g aws-cdk)".to_string());
        }

        // Check cargo-lambda
        if !Command::new("cargo")
            .args(&["lambda", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("cargo-lambda (install: cargo install cargo-lambda)".to_string());
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        init::init_aws_lambda(config).await
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        build_lambda_binary(config).await
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        _artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        // Thread developer-declared [environment] alongside resolved [secrets]
        // through the same transient CDK-process-env path (fix for
        // deploy-toml-inert-for-preserved-stack, FIX #2). `deploy_env_vars()`
        // merges both maps (secrets win on collision); the stack.ts decides
        // which keys it consumes via process.env.
        deploy::deploy_aws_lambda(config, config.deploy_env_vars()).await
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
