use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::config::DeployConfig;
use super::operations::{AsyncOperation, DestroyResult};

/// Represents a built artifact ready for deployment
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BuildArtifact {
    /// Native binary (e.g., ARM64 Linux for Lambda)
    Binary {
        path: PathBuf,
        size: u64,
        /// Optional deployment package (zip) containing binary + assets
        deployment_package: Option<PathBuf>,
    },
    /// WebAssembly module
    Wasm {
        path: PathBuf,
        size: u64,
        /// Optional deployment package containing WASM + assets
        deployment_package: Option<PathBuf>,
    },
    /// Custom artifact type
    Custom {
        path: PathBuf,
        artifact_type: String,
        /// Optional deployment package
        deployment_package: Option<PathBuf>,
    },
}

/// Deployment outputs from a successful deployment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploymentOutputs {
    /// Primary endpoint URL
    pub url: Option<String>,
    /// Additional endpoint URLs (e.g., regional endpoints)
    pub additional_urls: Vec<String>,
    /// Deployment region(s)
    pub regions: Vec<String>,
    /// Stack/deployment name
    pub stack_name: Option<String>,
    /// Version/revision identifier
    pub version: Option<String>,
    /// Custom outputs specific to the target
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

#[cfg(test)]
thread_local! {
    /// How many times [`DeploymentOutputs::display`] ran on this thread. Lets
    /// a test assert that a deploy path leaves printing the outputs to the
    /// CLI, which prints them exactly once (debug session
    /// `cargo-pmcp-deploy-targets`, A2).
    pub(crate) static DISPLAY_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl DeploymentOutputs {
    /// Display outputs in human-readable format
    pub fn display(&self) {
        #[cfg(test)]
        DISPLAY_CALLS.with(|calls| calls.set(calls.get() + 1));

        println!("📊 Deployment Outputs:");
        println!();

        if let Some(url) = &self.url {
            println!("   🌐 URL: {}", url);
        }

        if !self.additional_urls.is_empty() {
            println!("   🔗 Additional URLs:");
            for url in &self.additional_urls {
                println!("      - {}", url);
            }
        }

        if !self.regions.is_empty() {
            println!("   🌍 Regions: {}", self.regions.join(", "));
        }

        if let Some(stack) = &self.stack_name {
            println!("   📦 Stack: {}", stack);
        }

        if let Some(version) = &self.version {
            println!("   🏷️  Version: {}", version);
        }

        if !self.custom.is_empty() {
            println!("   ⚙️  Custom Outputs:");
            for (key, value) in &self.custom {
                println!("      {}: {}", key, value);
            }
        }

        println!();
    }
}

/// Metrics data from a deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData {
    /// Time period for metrics
    pub period: String,
    /// Total requests
    pub requests: Option<u64>,
    /// Error count
    pub errors: Option<u64>,
    /// Average latency in milliseconds
    pub avg_latency_ms: Option<f64>,
    /// P99 latency in milliseconds
    pub p99_latency_ms: Option<f64>,
    /// Custom metrics specific to the target
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

/// Test results from deployment testing
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestResults {
    /// Whether tests passed
    pub success: bool,
    /// Number of tests run
    pub tests_run: usize,
    /// Number of tests passed
    pub tests_passed: usize,
    /// Test failures with details
    pub failures: Vec<TestFailure>,
}

/// A single test failure
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestFailure {
    /// Test name
    pub name: String,
    /// Error message
    pub error: String,
}

/// Actions for secret management
#[derive(Debug, Clone)]
pub enum SecretsAction {
    /// Set a secret value
    Set {
        key: String,
        from_env: Option<String>,
    },
    /// List all secrets (values hidden)
    List,
    /// Delete a secret
    Delete { key: String, yes: bool },
}

/// Trait that all deployment targets must implement
#[async_trait]
#[allow(dead_code)]
pub trait DeploymentTarget: Send + Sync {
    /// Unique identifier for the target (e.g., "aws-lambda", "cloudflare-workers")
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "AWS Lambda", "Cloudflare Workers")
    fn name(&self) -> &str;

    /// Description of the target
    fn description(&self) -> &str;

    /// Check if this target is available (CLI tools installed, credentials configured)
    async fn is_available(&self) -> Result<bool>;

    /// Get list of missing prerequisites
    async fn prerequisites(&self) -> Vec<String>;

    /// Initialize deployment configuration for this target
    async fn init(&self, config: &DeployConfig) -> Result<()>;

    /// Build the binary/WASM for this target
    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact>;

    /// Deploy to the target platform
    async fn deploy(
        &self,
        config: &DeployConfig,
        artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs>;

    /// Destroy deployment and optionally clean up local files
    ///
    /// This is the legacy synchronous destroy method. For targets that support
    /// async operations, consider using `destroy_async` instead.
    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()>;

    /// Destroy deployment with async operation support
    ///
    /// Returns a `DestroyResult` that may contain an async operation for polling.
    /// By default, this calls the legacy `destroy` method and returns a sync result.
    async fn destroy_async(&self, config: &DeployConfig, clean: bool) -> Result<DestroyResult> {
        self.destroy(config, clean).await?;
        Ok(DestroyResult::sync_success(
            "Deployment destroyed successfully",
        ))
    }

    /// Whether this target supports async operations
    ///
    /// If true, long-running operations like destroy may return async operations
    /// that can be polled for completion status.
    fn supports_async_operations(&self) -> bool {
        false
    }

    /// Check the status of an async operation
    ///
    /// This is only supported for targets where `supports_async_operations` returns true.
    async fn get_operation_status(&self, _operation_id: &str) -> Result<AsyncOperation> {
        anyhow::bail!("Async operations not supported by target: {}", self.id())
    }

    /// Get deployment outputs (URLs, etc.)
    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs>;

    /// Stream logs from the deployment
    async fn logs(&self, config: &DeployConfig, tail: bool, lines: usize) -> Result<()>;

    /// Get metrics for the deployment
    async fn metrics(&self, config: &DeployConfig, period: &str) -> Result<MetricsData>;

    /// Manage secrets for the deployment
    async fn secrets(&self, config: &DeployConfig, action: SecretsAction) -> Result<()>;

    /// Test the deployment
    async fn test(&self, config: &DeployConfig, verbose: bool) -> Result<TestResults>;

    /// Rollback to a previous version
    async fn rollback(&self, config: &DeployConfig, version: Option<&str>) -> Result<()>;
}
