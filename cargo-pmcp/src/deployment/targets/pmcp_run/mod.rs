pub mod auth;
mod deploy;
pub mod graphql;
// Dependency-light leaf shared with the `cargo-pmcp` lib target (mounted via
// `#[path]` in `lib.rs`) so the offline contract test can reach the two
// capture GraphQL query consts without pulling this tree's reqwest/auth/deploy
// deps into the lib target. See `graphql_contract.rs` for details.
mod graphql_contract;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

use crate::deployment::{
    operations::{AsyncOperation, DestroyResult, OperationStatus, OperationType},
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

impl PmcpRunTarget {
    /// Wait for an async operation to complete by polling
    async fn wait_for_operation(&self, operation_id: &str) -> Result<()> {
        use std::time::Duration;

        let mut dots = 0;

        loop {
            let status = self.get_operation_status(operation_id).await?;

            match status.status {
                OperationStatus::Completed => {
                    if dots > 0 {
                        println!();
                    }
                    println!("✅ {}", status.message);
                    return Ok(());
                },
                OperationStatus::Failed => {
                    if dots > 0 {
                        println!();
                    }
                    bail!("Operation failed: {}", status.message);
                },
                OperationStatus::Initiated | OperationStatus::Running => {
                    print!(".");
                    dots += 1;
                    if dots >= 60 {
                        println!();
                        dots = 0;
                    }
                    std::io::Write::flush(&mut std::io::stdout())?;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                },
            }
        }
    }

    /// Clean up local deployment files
    fn cleanup_local_files(&self, config: &DeployConfig) -> Result<()> {
        let deploy_dir = config.project_root.join("deploy");

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

        Ok(())
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
        // `[server] ephemeral_storage_mb` / `[build]`: refuse a bad value before
        // the (multi-minute) build, not after it.
        crate::deployment::lambda_function::validate_lambda_settings(config)?;

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
        // Use destroy_async and wait for completion
        let result = self.destroy_async(config, clean).await?;

        if let Some(operation) = result.async_operation {
            // Poll for completion
            println!("⏳ Waiting for destruction to complete...");
            self.wait_for_operation(&operation.operation_id).await?;
        }

        Ok(())
    }

    async fn destroy_async(&self, config: &DeployConfig, clean: bool) -> Result<DestroyResult> {
        let deploy_dir = config.project_root.join("deploy");

        if !deploy_dir.exists() {
            println!("⚠️  No pmcp.run deployment found");
            return Ok(DestroyResult::sync_success("No deployment found"));
        }

        println!("🗑️  Destroying pmcp.run deployment...");
        println!();

        // Call pmcp.run API to delete deployment
        let credentials = auth::get_credentials().await?;

        // First, find the deployment ID by project name
        let deployment_id =
            graphql::find_deployment_id_by_name(&credentials.access_token, &config.server.name)
                .await?;

        println!("   Found deployment: {}", deployment_id);

        // Destroy the deployment (complete cleanup including CloudFormation stack)
        let destroy_result =
            graphql::destroy_deployment(&credentials.access_token, &deployment_id).await?;

        // Check if this is an async operation (initiated) or sync (completed/failed)
        match destroy_result.status.as_str() {
            "initiated" | "deleting" => {
                // Async operation - return for polling
                println!("⏳ Destruction initiated...");
                if let Some(ref msg) = destroy_result.message {
                    println!("   {}", msg);
                }

                let operation = AsyncOperation {
                    operation_id: deployment_id.clone(),
                    operation_type: OperationType::Destroy,
                    status: OperationStatus::Initiated,
                    message: destroy_result
                        .message
                        .unwrap_or_else(|| "Destruction initiated".to_string()),
                    target: "pmcp-run".to_string(),
                    metadata: Some(serde_json::json!({
                        "execution_arn": destroy_result.execution_arn,
                        "stack_name": destroy_result.stack_name,
                    })),
                };

                // Store clean flag for later cleanup
                if clean {
                    // Note: Local cleanup will be done when polling completes
                    println!("   Local files will be cleaned up after destruction completes.");
                }

                Ok(DestroyResult::async_operation(operation))
            },
            "deleted" | "success" => {
                // Sync completion
                println!("✅ pmcp.run deployment destroyed successfully");

                if clean {
                    self.cleanup_local_files(config)?;
                }

                Ok(DestroyResult::sync_success(
                    destroy_result
                        .message
                        .unwrap_or_else(|| "Deployment destroyed".to_string()),
                ))
            },
            "failed" | "delete_failed" => {
                let error_msg = destroy_result
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string());
                bail!("Failed to destroy deployment: {}", error_msg);
            },
            status => {
                // Unknown status - treat as async and let caller poll
                println!("   Destruction status: {}", status);
                let operation = AsyncOperation {
                    operation_id: deployment_id,
                    operation_type: OperationType::Destroy,
                    status: OperationStatus::Running,
                    message: destroy_result
                        .message
                        .unwrap_or_else(|| format!("Status: {}", status)),
                    target: "pmcp-run".to_string(),
                    metadata: None,
                };
                Ok(DestroyResult::async_operation(operation))
            },
        }
    }

    fn supports_async_operations(&self) -> bool {
        true
    }

    async fn get_operation_status(&self, operation_id: &str) -> Result<AsyncOperation> {
        let credentials = auth::get_credentials().await?;
        let status =
            graphql::get_deployment_operation_status(&credentials.access_token, operation_id)
                .await?;

        // Map server status to OperationStatus
        let op_status = match status.status.as_str() {
            "initiated" | "pending" | "validating" => OperationStatus::Initiated,
            "deleting" | "deploying" | "running" => OperationStatus::Running,
            "deleted" | "success" | "completed" => OperationStatus::Completed,
            "failed" | "delete_failed" | "error" => OperationStatus::Failed,
            _ => OperationStatus::Running, // Unknown status, assume still running
        };

        Ok(AsyncOperation {
            operation_id: status.id,
            operation_type: OperationType::Destroy, // Assuming destroy for now
            status: op_status,
            message: status.message.unwrap_or_else(|| status.status.clone()),
            target: "pmcp-run".to_string(),
            metadata: Some(serde_json::json!({
                "execution_arn": status.execution_arn,
                "updated_at": status.updated_at,
            })),
        })
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
