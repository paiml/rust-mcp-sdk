use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};

/// Detect server name from Cargo.toml in the project root or core workspace
fn detect_server_name(project_root: &Path) -> Result<String> {
    // Try to read the main Cargo.toml
    let cargo_toml_path = project_root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)?;
        if let Ok(cargo_toml) = toml::from_str::<toml::Value>(&cargo_toml_content) {
            // Check if it's a workspace
            if cargo_toml.get("workspace").is_some() {
                // Look for core-workspace or similar
                let core_workspace_dir = project_root.join("core-workspace");
                if core_workspace_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&core_workspace_dir) {
                        for entry in entries.flatten() {
                            let cargo_path = entry.path().join("Cargo.toml");
                            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                                if let Ok(core_cargo) = toml::from_str::<toml::Value>(&content) {
                                    if let Some(name) = core_cargo
                                        .get("package")
                                        .and_then(|p| p.get("name"))
                                        .and_then(|n| n.as_str())
                                    {
                                        // Remove "mcp-" prefix and "-core" suffix to get clean name
                                        let clean_name = name
                                            .strip_prefix("mcp-")
                                            .unwrap_or(name)
                                            .strip_suffix("-core")
                                            .unwrap_or(name);
                                        return Ok(clean_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Not a workspace, use the package name directly
            if let Some(name) = cargo_toml
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
            {
                return Ok(name.to_string());
            }
        }
    }

    // Fallback to directory name
    Ok(project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mcp-server")
        .to_string())
}

pub mod deploy;
pub mod init;
mod logs;
mod metrics;
mod secrets;
mod test;

use init::InitCommand;

#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target (aws-lambda, cloudflare-workers)
    #[clap(long, global = true)]
    target: Option<String>,

    #[clap(subcommand)]
    action: Option<DeployAction>,
}

#[derive(Debug, Parser)]
pub enum DeployAction {
    /// Initialize deployment configuration
    Init {
        /// AWS region (for AWS Lambda target, default: us-east-1)
        #[clap(long, default_value = "us-east-1")]
        region: String,

        /// Skip credentials check
        #[clap(long)]
        skip_credentials_check: bool,
    },

    /// View deployment logs
    Logs {
        /// Follow logs in real-time
        #[clap(long)]
        tail: bool,

        /// Number of lines to show
        #[clap(long, default_value = "100")]
        lines: usize,
    },

    /// View deployment metrics
    Metrics {
        /// Time period (1h, 24h, 7d, 30d)
        #[clap(long, default_value = "24h")]
        period: String,
    },

    /// Test the deployment
    Test {
        /// Verbose output
        #[clap(long)]
        verbose: bool,
    },

    /// Rollback to previous version
    Rollback {
        /// Version to rollback to (default: previous)
        version: Option<String>,

        /// Skip confirmation
        #[clap(long)]
        yes: bool,
    },

    /// Destroy the deployment
    Destroy {
        /// Skip confirmation prompt
        #[clap(long)]
        yes: bool,

        /// Remove all deployment files (CDK project, Lambda wrapper, config)
        #[clap(long)]
        clean: bool,
    },

    /// Manage secrets
    Secrets {
        #[clap(subcommand)]
        action: SecretsAction,
    },

    /// Show deployment outputs
    Outputs {
        /// Output format (text, json)
        #[clap(long, default_value = "text")]
        format: String,
    },

    /// Login to deployment target (pmcp-run, cloudflare, etc.)
    Login,

    /// Logout from deployment target
    Logout,
}

#[derive(Debug, Parser)]
pub enum SecretsAction {
    /// Set a secret value
    Set {
        /// Secret key
        key: String,

        /// Get value from environment variable
        #[clap(long)]
        from_env: Option<String>,
    },

    /// List all secrets
    List,

    /// Delete a secret
    Delete {
        /// Secret key
        key: String,

        /// Skip confirmation
        #[clap(long)]
        yes: bool,
    },
}

impl DeployCommand {
    pub fn execute(&self) -> Result<()> {
        // Run async code in tokio runtime
        tokio::runtime::Runtime::new()?.block_on(self.execute_async())
    }

    async fn execute_async(&self) -> Result<()> {
        let project_root = Self::find_project_root()?;

        // Get target from flag or config
        let target_id = self.get_target_id(&project_root)?;

        // Get target registry and resolve target
        let registry = crate::deployment::TargetRegistry::new();
        let target = registry.get(&target_id)?;

        match &self.action {
            Some(action) => match action {
                DeployAction::Init {
                    region,
                    skip_credentials_check,
                } => {
                    // For init, we can use the old approach or new depending on target
                    if target_id == "aws-lambda" {
                        InitCommand::new(project_root)
                            .with_region(region)
                            .with_credentials_check(!skip_credentials_check)
                            .execute()
                    } else {
                        // For other targets, use the new modular approach
                        // Auto-detect server name from workspace or use package name
                        let server_name = detect_server_name(&project_root)?;
                        let config = crate::deployment::DeployConfig::default_for_server(
                            server_name,
                            region.clone(),
                            project_root.clone(),
                        );
                        target.init(&config).await
                    }
                },
                DeployAction::Logs { tail, lines } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    target.logs(&config, *tail, *lines).await
                },
                DeployAction::Metrics { period } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let metrics = target.metrics(&config, period).await?;
                    println!("📊 Metrics for {}: {}", target.name(), metrics.period);
                    Ok(())
                },
                DeployAction::Test { verbose } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let results = target.test(&config, *verbose).await?;
                    if results.success {
                        println!(
                            "✅ All tests passed ({}/{})",
                            results.tests_passed, results.tests_run
                        );
                    } else {
                        println!(
                            "❌ Some tests failed ({}/{})",
                            results.tests_passed, results.tests_run
                        );
                    }
                    Ok(())
                },
                DeployAction::Rollback { version, yes: _ } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    target.rollback(&config, version.as_deref()).await
                },
                DeployAction::Destroy { yes, clean } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;

                    if !yes {
                        println!("⚠️  This will destroy deployment on {}", target.name());
                        print!("Type '{}' to confirm: ", config.server.name);
                        use std::io::{self, Write};
                        io::stdout().flush()?;

                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;

                        if input.trim() != config.server.name {
                            println!("❌ Confirmation failed. Aborting.");
                            return Ok(());
                        }
                    }

                    target.destroy(&config, *clean).await
                },
                DeployAction::Secrets { action } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
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
                    let outputs = target.outputs(&config).await?;

                    match format.as_str() {
                        "json" => {
                            println!("{}", serde_json::to_string_pretty(&outputs)?);
                        },
                        "text" => {
                            outputs.display();
                        },
                        _ => bail!("Unknown format: {}", format),
                    }
                    Ok(())
                },
                DeployAction::Login => {
                    // Login is target-specific
                    match target_id.as_str() {
                        "pmcp-run" => crate::deployment::targets::pmcp_run::login().await,
                        _ => {
                            bail!("Login is not supported for target: {}", target_id);
                        },
                    }
                },
                DeployAction::Logout => {
                    // Logout is target-specific
                    match target_id.as_str() {
                        "pmcp-run" => crate::deployment::targets::pmcp_run::logout(),
                        _ => {
                            bail!("Logout is not supported for target: {}", target_id);
                        },
                    }
                },
            },
            None => {
                // No subcommand = deploy
                let config = crate::deployment::DeployConfig::load(&project_root)?;
                let artifact = target.build(&config).await?;
                let outputs = target.deploy(&config, artifact).await?;

                println!();
                outputs.display();

                // Save deployment info for pmcp-run target (for landing page integration)
                if target_id == "pmcp-run" {
                    Self::save_deployment_info(&project_root, &outputs)?;
                }

                Ok(())
            },
        }
    }

    fn get_target_id(&self, project_root: &PathBuf) -> Result<String> {
        // Priority: --target flag > config file > default
        if let Some(target) = &self.target {
            return Ok(target.clone());
        }

        // Try to read from config
        if let Ok(config) = crate::deployment::DeployConfig::load(project_root) {
            return Ok(config.target.target_type.clone());
        }

        // Default to AWS Lambda
        Ok("aws-lambda".to_string())
    }

    fn find_project_root() -> Result<PathBuf> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;

        let mut dir = current_dir.as_path();

        loop {
            if dir.join("Cargo.toml").exists() {
                return Ok(dir.to_path_buf());
            }

            dir = dir.parent().ok_or_else(|| {
                anyhow::anyhow!("Could not find Cargo.toml in any parent directory")
            })?;
        }
    }

    /// Save deployment info to .pmcp/deployment.toml for landing page integration
    fn save_deployment_info(
        project_root: &PathBuf,
        outputs: &crate::deployment::DeploymentOutputs,
    ) -> Result<()> {
        use std::io::Write;

        // Extract deployment_id from custom outputs
        let deployment_id = outputs
            .custom
            .get("deployment_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No deployment_id in outputs"))?;

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
            deployment_id, url
        );

        // Write to .pmcp/deployment.toml
        let deployment_file = pmcp_dir.join("deployment.toml");
        let mut file = std::fs::File::create(&deployment_file)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}
