pub mod auth;
mod deploy;
pub mod graphql;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    DeployConfig,
};

pub use auth::{login, logout};

pub struct PmcpRunTarget;

impl PmcpRunTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PmcpRunTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeploymentTarget for PmcpRunTarget {
    fn id(&self) -> &str {
        "pmcp-run"
    }

    fn name(&self) -> &str {
        "pmcp.run"
    }

    fn description(&self) -> &str {
        "Deploy to pmcp.run managed service (AWS Lambda backend)"
    }

    async fn is_available(&self) -> Result<bool> {
        // Check for required tools
        let has_cargo_lambda = std::process::Command::new("cargo-lambda")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_cdk = std::process::Command::new("cdk")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(has_cargo_lambda && has_cdk)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check cargo-lambda
        if !std::process::Command::new("cargo-lambda")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("cargo-lambda (install: brew install cargo-lambda)".to_string());
        }

        // Check aws-cdk
        if !std::process::Command::new("cdk")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("aws-cdk (install: npm install -g aws-cdk)".to_string());
        }

        // Check authentication
        if auth::get_credentials().await.is_err() {
            missing.push(
                "pmcp.run authentication (run: cargo pmcp deploy login --target pmcp-run)"
                    .to_string(),
            );
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        println!("🚀 Initializing pmcp.run deployment...");
        println!("   Using AWS Lambda + CDK backend");
        println!();

        // Reuse AWS Lambda initialization logic
        // The scaffolding is identical, just the deployment target differs
        crate::deployment::targets::aws_lambda::init::init_aws_lambda(config).await?;

        println!();
        println!("✅ pmcp.run deployment initialized!");
        println!();
        println!("📝 Next steps:");
        println!("   1. Authenticate: cargo pmcp login --target pmcp-run");
        println!("   2. Deploy: cargo pmcp deploy --target pmcp-run");
        println!();
        println!("💡 The CDK scaffolding in deploy/ can be customized");
        println!("   before deployment for advanced configurations.");

        Ok(())
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        println!("🔨 Building Lambda binary for pmcp.run...");

        // Reuse AWS Lambda build logic
        crate::deployment::targets::aws_lambda::build_lambda_binary(config).await
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        deploy::deploy_to_pmcp_run(config, artifact).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        let deploy_dir = config.project_root.join("deploy");

        if !deploy_dir.exists() {
            println!("⚠️  No pmcp.run deployment found");
            return Ok(());
        }

        println!("🗑️  Destroying pmcp.run deployment...");
        println!();

        // Call pmcp.run API to delete deployment
        let credentials = auth::get_credentials().await?;
        graphql::delete_deployment(&credentials.access_token, &config.server.name).await?;

        println!("✅ pmcp.run deployment destroyed successfully");

        if clean {
            println!();
            println!("🧹 Cleaning up local deployment files...");

            // Remove deploy directory
            if deploy_dir.exists() {
                std::fs::remove_dir_all(&deploy_dir)
                    .context("Failed to remove deployment directory")?;
                println!("   ✓ Removed {}/", deploy_dir.display());
            }

            // Remove config if this is the only target
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
        let credentials = auth::get_credentials().await?;
        graphql::get_deployment_outputs(&credentials.access_token, &config.server.name).await
    }

    async fn logs(&self, _config: &DeployConfig, _tail: bool, _lines: usize) -> Result<()> {
        println!("📜 Log streaming coming in Phase 2!");
        println!("   View logs at: https://pmcp.run/dashboard");
        Ok(())
    }

    async fn metrics(&self, _config: &DeployConfig, period: &str) -> Result<MetricsData> {
        println!("📊 pmcp.run metrics coming soon!");
        println!("   View metrics at: https://pmcp.run/dashboard");
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
        println!("🔐 Secrets management coming in Phase 2!");
        println!("   View secrets at: https://pmcp.run/dashboard");
        Ok(())
    }

    async fn test(&self, config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        println!("🧪 Testing pmcp.run deployment...");

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
        println!("🔄 Rollback functionality coming in Phase 2!");
        println!(
            "   This will rollback to version: {}",
            version.unwrap_or("previous")
        );
        Ok(())
    }
}
