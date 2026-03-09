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

use init::InitCommand;

#[derive(Debug, Parser)]
pub struct DeployCommand {
    /// Deployment target (aws-lambda, cloudflare-workers)
    #[clap(long, global = true)]
    target: Option<String>,

    /// Use shared OAuth pool for SSO (pmcp-run only).
    ///
    /// Enables OAuth authentication using an existing shared Cognito User Pool.
    /// This enables Single Sign-On (SSO) with other MCP servers using the same pool.
    ///
    /// Example: --shared-pool agent-framework
    ///
    /// Equivalent to running:
    ///   cargo pmcp deploy
    ///   cargo pmcp deploy oauth enable --server <server> --shared-pool <name>
    #[clap(long, value_name = "POOL_NAME")]
    shared_pool: Option<String>,

    /// Skip OAuth configuration during deployment.
    ///
    /// Deploy without OAuth first, then add it later with:
    ///   cargo pmcp deploy oauth enable --server <server> [--shared-pool <name>]
    ///
    /// This is useful for testing the server before adding authentication.
    #[clap(long)]
    no_oauth: bool,

    #[clap(subcommand)]
    action: Option<DeployAction>,
}

#[derive(Debug, Parser)]
pub enum DeployAction {
    /// Initialize deployment configuration
    Init {
        /// AWS region for deployment (uses AWS_REGION or AWS_DEFAULT_REGION env vars if set)
        #[clap(long, env = "AWS_REGION", default_value = "us-east-1")]
        region: String,

        /// Skip credentials check
        #[clap(long)]
        skip_credentials_check: bool,

        /// OAuth provider (cognito, oidc, none)
        #[clap(long, value_name = "PROVIDER")]
        oauth: Option<String>,

        /// Use shared OAuth infrastructure (format: shared:<name>)
        #[clap(long, value_name = "NAME")]
        oauth_shared: Option<String>,

        /// Existing Cognito User Pool ID (skip creation)
        #[clap(long, value_name = "POOL_ID")]
        cognito_user_pool_id: Option<String>,

        /// Cognito User Pool name (when creating new)
        #[clap(long, value_name = "NAME")]
        cognito_pool_name: Option<String>,

        /// Enable social login providers (comma-separated: github,google,apple)
        #[clap(long, value_name = "PROVIDERS", value_delimiter = ',')]
        social_providers: Option<Vec<String>>,
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

        /// Don't wait for async operations to complete (pmcp-run only)
        #[clap(long)]
        no_wait: bool,
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

    /// Manage OAuth configuration for pmcp.run servers
    Oauth {
        #[clap(subcommand)]
        action: OAuthAction,
    },

    /// Check status of an async operation (pmcp-run only)
    Status {
        /// Operation ID to check (deployment ID for destroy operations)
        operation_id: String,
    },
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

/// Default OAuth scopes for MCP servers
const DEFAULT_OAUTH_SCOPES: &[&str] = &["openid", "email", "mcp/read"];

/// Default public client patterns for MCP OAuth
const DEFAULT_PUBLIC_CLIENT_PATTERNS: &[&str] =
    &["claude", "cursor", "desktop", "mcp-inspector", "chatgpt"];

#[derive(Debug, Parser)]
pub enum OAuthAction {
    /// Enable OAuth for an MCP server on pmcp.run
    ///
    /// Configures OAuth authentication using AWS Cognito. When using --shared-pool
    /// or --copy-from, multiple MCP servers can share the same user pool, enabling
    /// Single Sign-On (SSO) across servers.
    Enable {
        /// Server ID (deployment ID) to enable OAuth for
        #[clap(long)]
        server: String,

        /// Copy OAuth configuration from an existing server.
        ///
        /// Fetches OAuth settings (scopes, DCR, public clients, user pool) from
        /// another server and applies them to this server. This is the easiest
        /// way to enable SSO across multiple MCP servers.
        ///
        /// Example: --copy-from advanced-mcp-course
        #[clap(long, value_name = "SERVER_ID")]
        copy_from: Option<String>,

        /// OAuth scopes for this server (comma-separated).
        ///
        /// Defines what permissions clients can request:
        ///   - openid:    Required for OIDC (always include)
        ///   - email:     Access to user's email address
        ///   - mcp/read:  Read-only MCP operations
        ///   - mcp/write: Read-write MCP operations
        ///
        /// Default: "openid,email,mcp/read"
        ///
        /// Note: When using --shared-pool, scopes are server-specific and
        /// do NOT affect other servers sharing the same pool.
        #[clap(long, value_delimiter = ',', value_name = "SCOPES")]
        scopes: Option<Vec<String>>,

        /// Enable Dynamic Client Registration (RFC 7591).
        ///
        /// When enabled, MCP clients (Claude, Cursor, ChatGPT) can automatically
        /// register themselves when users add your server URL.
        ///
        /// Default: true (recommended for MCP servers)
        #[clap(long, default_value = "true")]
        dcr: bool,

        /// Public client patterns (comma-separated).
        ///
        /// Client names matching these patterns are treated as public OAuth
        /// clients (no client_secret required). This is correct for desktop
        /// and native apps that cannot securely store secrets.
        ///
        /// Default: "claude,cursor,desktop,mcp-inspector,chatgpt"
        ///
        /// Example: --public-clients "claude,cursor,my-app"
        #[clap(long, value_delimiter = ',', value_name = "PATTERNS")]
        public_clients: Option<Vec<String>>,

        /// Use an existing Cognito User Pool instead of creating a new one.
        ///
        /// This enables Single Sign-On (SSO) across multiple MCP servers.
        /// Users with accounts on other servers sharing this pool can
        /// automatically access this server.
        ///
        /// Value can be:
        ///   - Cognito User Pool ID (e.g., "us-east-1_TSTigvdHH")
        ///   - Shared pool name from organization setup
        ///
        /// TIP: To find an existing pool ID, run:
        ///   cargo pmcp deploy oauth status --server <existing-server>
        ///
        /// Note: Other parameters (--scopes, --dcr, --public-clients) configure
        /// THIS server's OAuth behavior, not the shared pool itself.
        #[clap(long, value_name = "POOL_ID_OR_NAME")]
        shared_pool: Option<String>,
    },

    /// Disable OAuth for an MCP server
    ///
    /// Disables OAuth authentication. The Cognito User Pool is NOT deleted,
    /// so you can re-enable OAuth at any time.
    Disable {
        /// Server ID (deployment ID)
        #[clap(long)]
        server: String,
    },

    /// Show OAuth status and endpoints for an MCP server
    ///
    /// Displays current OAuth configuration including endpoints, scopes,
    /// and Cognito User Pool details. Use this to find pool IDs for
    /// sharing with other servers.
    Status {
        /// Server ID (deployment ID)
        #[clap(long)]
        server: String,
    },
}

impl DeployCommand {
    pub fn execute(&self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        // Run async code in tokio runtime
        tokio::runtime::Runtime::new()?.block_on(self.execute_async(global_flags))
    }

    async fn execute_async(&self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        let project_root = Self::find_project_root()?;

        // Get target from flag or config
        let target_id = self.get_target_id(&project_root)?;

        // Get target registry and resolve target
        let registry = crate::deployment::TargetRegistry::new();
        let target = registry.get(&target_id)?;

        match &self.action {
            Some(action) => {
                match action {
                    DeployAction::Init {
                        region,
                        skip_credentials_check,
                        oauth,
                        oauth_shared,
                        cognito_user_pool_id,
                        cognito_pool_name,
                        social_providers,
                    } => {
                        // For init, we can use the old approach or new depending on target
                        if target_id == "aws-lambda" {
                            let mut cmd = InitCommand::new(project_root)
                                .with_region(region)
                                .with_credentials_check(!skip_credentials_check);

                            // Configure OAuth if specified
                            if let Some(provider) = oauth {
                                cmd = cmd.with_oauth_provider(provider);
                            }
                            if let Some(shared_name) = oauth_shared {
                                cmd = cmd.with_oauth_shared(&shared_name);
                            }
                            if let Some(pool_id) = cognito_user_pool_id {
                                cmd = cmd.with_cognito_user_pool_id(&pool_id);
                            }
                            if let Some(pool_name) = cognito_pool_name {
                                cmd = cmd.with_cognito_pool_name(&pool_name);
                            }
                            if let Some(providers) = social_providers {
                                cmd = cmd.with_social_providers(providers.clone());
                            }

                            cmd.execute()
                        } else {
                            // For other targets (pmcp-run, etc.), use the new modular approach
                            // Auto-detect server name from workspace or use package name
                            let server_name = detect_server_name(&project_root)?;
                            let mut config = crate::deployment::DeployConfig::default_for_server(
                                server_name,
                                region.clone(),
                                project_root.clone(),
                            );

                            // Update target type to match the actual target
                            config.target.target_type = target_id.clone();

                            // Configure OAuth if specified (for pmcp-run target)
                            if let Some(provider) = oauth {
                                if provider == "cognito" || provider == "oidc" {
                                    config.auth.enabled = true;
                                    config.auth.provider = provider.clone();

                                    // Set default scopes if not specified
                                    if config.auth.dcr.default_scopes.is_empty() {
                                        config.auth.dcr.default_scopes = vec![
                                            "openid".to_string(),
                                            "email".to_string(),
                                            "mcp/read".to_string(),
                                            "mcp/write".to_string(),
                                        ];
                                    }
                                }
                            }

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
                        // Requested data -- always show
                        println!("Metrics for {}: {}", target.name(), metrics.period);
                        Ok(())
                    },
                    DeployAction::Test { verbose } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        let results = target.test(&config, *verbose).await?;
                        // Test results are requested output
                        if results.success {
                            println!(
                                "All tests passed ({}/{})",
                                results.tests_passed, results.tests_run
                            );
                        } else {
                            println!(
                                "Some tests failed ({}/{})",
                                results.tests_passed, results.tests_run
                            );
                        }
                        Ok(())
                    },
                    DeployAction::Rollback { version, yes: _ } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        target.rollback(&config, version.as_deref()).await
                    },
                    DeployAction::Destroy {
                        yes,
                        clean,
                        no_wait,
                    } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;

                        if !yes {
                            println!("WARNING: This will destroy deployment on {}", target.name());
                            print!("Type '{}' to confirm: ", config.server.name);
                            use std::io::{self, Write};
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;

                            if input.trim() != config.server.name {
                                println!("Confirmation failed. Aborting.");
                                return Ok(());
                            }
                        }

                        // Use async destroy if --no-wait is specified and target supports it
                        if *no_wait && target.supports_async_operations() {
                            let result = target.destroy_async(&config, *clean).await?;
                            if let Some(op) = result.async_operation {
                                if global_flags.should_output() {
                                    println!();
                                    println!("{}", op.message);
                                    println!();
                                    println!("Destruction initiated. Use the following to check progress:");
                                    println!("   cargo pmcp deploy status {}", op.operation_id);
                                }
                            } else if global_flags.should_output() {
                                println!("{}", result.message);
                            }
                            Ok(())
                        } else {
                            // Default behavior: wait for completion
                            target.destroy(&config, *clean).await
                        }
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
                    DeployAction::Oauth { action } => {
                        // OAuth is only supported for pmcp-run target
                        if target_id != "pmcp-run" {
                            bail!("OAuth management is only supported for pmcp-run target");
                        }
                        handle_oauth_action(action).await
                    },
                    DeployAction::Status { operation_id } => {
                        // Status is only supported for targets with async operations
                        if !target.supports_async_operations() {
                            bail!(
                                "Async operation status is not supported for target: {}",
                                target_id
                            );
                        }

                        if global_flags.should_output() {
                            println!("Checking operation status...");
                            println!();
                        }

                        let status = target.get_operation_status(operation_id).await?;

                        // Status output is requested data
                        match status.status {
                            crate::deployment::OperationStatus::Initiated => {
                                println!("Status: Initiated");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Running => {
                                println!("Status: Running");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Completed => {
                                println!("Status: Completed");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Failed => {
                                println!("Status: Failed");
                                println!("   {}", status.message);
                            },
                        }

                        if let Some(metadata) = &status.metadata {
                            if let Some(updated_at) = metadata.get("updated_at") {
                                println!();
                                println!("   Last updated: {}", updated_at);
                            }
                        }

                        Ok(())
                    },
                }
            },
            None => {
                // No subcommand = deploy
                let config = crate::deployment::DeployConfig::load(&project_root)?;
                let artifact = target.build(&config).await?;
                let outputs = target.deploy(&config, artifact).await?;

                if global_flags.should_output() {
                    println!();
                    outputs.display();
                }

                // Save deployment info for pmcp-run target (for landing page integration)
                if target_id == "pmcp-run" {
                    Self::save_deployment_info(&project_root, &outputs)?;

                    // Handle OAuth configuration if --shared-pool was provided
                    if let Some(ref pool_name) = self.shared_pool {
                        if global_flags.should_output() {
                            println!();
                            println!("Configuring OAuth with shared pool: {}", pool_name);
                        }

                        // Get the server ID from outputs
                        let server_id = outputs
                            .custom
                            .get("server_id")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                outputs.custom.get("deployment_id").and_then(|v| v.as_str())
                            })
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Could not determine server ID from deployment outputs"
                                )
                            })?;

                        // Configure OAuth with shared pool
                        let oauth_action = OAuthAction::Enable {
                            server: server_id.to_string(),
                            copy_from: None,
                            scopes: None, // Use defaults
                            dcr: true,
                            public_clients: None, // Use defaults
                            shared_pool: Some(pool_name.clone()),
                        };

                        handle_oauth_action(&oauth_action).await?;
                    } else if !self.no_oauth && global_flags.should_output() {
                        // Check if OAuth is already enabled (from backend query)
                        let oauth_already_enabled = outputs
                            .custom
                            .get("oauth_enabled")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if !oauth_already_enabled {
                            // Show hint about OAuth options only if not already configured
                            println!();
                            println!("OAuth not configured. To add authentication:");
                            if let Some(server_id) =
                                outputs.custom.get("server_id").and_then(|v| v.as_str())
                            {
                                println!(
                                    "   cargo pmcp deploy oauth enable --server {}",
                                    server_id
                                );
                                println!("   cargo pmcp deploy oauth enable --server {} --shared-pool <name>", server_id);
                            } else {
                                println!("   cargo pmcp deploy oauth enable --server <server_id>");
                            }
                        }
                    }
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

        // Extract server_id from custom outputs (the server name, e.g., "chess")
        // NOT deployment_id which is like "dep_xxx" - landing pages use server_id
        let server_id = outputs
            .custom
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No server_id in outputs"))?;

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
            server_id, url
        );

        // Write to .pmcp/deployment.toml
        let deployment_file = pmcp_dir.join("deployment.toml");
        let mut file = std::fs::File::create(&deployment_file)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}

/// Handle OAuth subcommands for pmcp.run
async fn handle_oauth_action(action: &OAuthAction) -> Result<()> {
    use crate::deployment::targets::pmcp_run::{auth, graphql};

    // Get credentials
    let credentials = auth::get_credentials().await?;

    match action {
        OAuthAction::Enable {
            server,
            copy_from,
            scopes,
            dcr,
            public_clients,
            shared_pool,
        } => {
            if std::env::var("PMCP_QUIET").is_err() {
                println!("Enabling OAuth for server: {}", server);
                println!();
            }

            // Resolve final configuration values
            // Priority: explicit params > copied values > defaults
            let (final_scopes, final_dcr, final_public_clients, final_shared_pool) =
                resolve_oauth_config(
                    &credentials.access_token,
                    copy_from.as_deref(),
                    scopes.clone(),
                    *dcr,
                    public_clients.clone(),
                    shared_pool.clone(),
                )
                .await?;

            // Display what configuration will be applied
            if (copy_from.is_some() || shared_pool.is_some())
                && std::env::var("PMCP_QUIET").is_err()
            {
                println!("OAuth Configuration:");
                println!("   Scopes:         {}", final_scopes.join(", "));
                println!(
                    "   DCR:            {}",
                    if final_dcr { "enabled" } else { "disabled" }
                );
                println!(
                    "   Public clients: {}",
                    final_public_clients
                        .as_ref()
                        .map(|p| p.join(", "))
                        .unwrap_or_else(|| "(default)".to_string())
                );
                if let Some(ref pool) = final_shared_pool {
                    println!("   Shared pool:    {}", pool);
                }
                println!();
            }

            let oauth_config = graphql::configure_server_oauth(
                &credentials.access_token,
                server,
                true,
                Some(final_scopes),
                Some(final_dcr),
                final_public_clients,
                final_shared_pool,
            )
            .await
            .context("Failed to configure OAuth")?;

            let not_quiet = std::env::var("PMCP_QUIET").is_err();
            if not_quiet {
                println!("OAuth enabled successfully!");
            }
            println!();
            println!("OAuth Endpoints:");
            if let Some(ref discovery) = oauth_config.discovery_url {
                println!("   Discovery:     {}", discovery);
            }
            if let Some(ref register) = oauth_config.registration_endpoint {
                println!("   Registration:  {}", register);
            }
            if let Some(ref authorize) = oauth_config.authorization_endpoint {
                println!("   Authorization: {}", authorize);
            }
            if let Some(ref token) = oauth_config.token_endpoint {
                println!("   Token:         {}", token);
            }
            if let Some(ref pool_id) = oauth_config.user_pool_id {
                println!();
                println!("   User Pool ID:  {}", pool_id);
            }
            if let Some(ref region) = oauth_config.user_pool_region {
                println!("   Region:        {}", region);
            }

            // Show helpful next steps for SSO
            if (copy_from.is_some() || shared_pool.is_some()) && not_quiet {
                println!();
                println!("SSO enabled: Users from the shared pool can access this server");
            }

            Ok(())
        },
        OAuthAction::Disable { server } => {
            let not_quiet = std::env::var("PMCP_QUIET").is_err();
            if not_quiet {
                println!("Disabling OAuth for server: {}", server);
                println!();
            }

            graphql::disable_server_oauth(&credentials.access_token, server)
                .await
                .context("Failed to disable OAuth")?;

            if not_quiet {
                println!("OAuth disabled successfully!");
                println!();
                println!("Note: The Cognito User Pool was NOT deleted.");
                println!("   You can re-enable OAuth at any time with:");
                println!("   cargo pmcp deploy oauth enable --server {}", server);
            }

            Ok(())
        },
        OAuthAction::Status { server } => {
            println!("OAuth Status for server: {}", server);
            println!();

            match graphql::fetch_server_oauth_endpoints(&credentials.access_token, server).await {
                Ok(endpoints) => {
                    if endpoints.oauth_enabled {
                        println!("   Status: Enabled");
                        if let Some(provider) = endpoints.provider {
                            println!("   Provider: {}", provider);
                        }
                        if let Some(dcr) = endpoints.dcr_enabled {
                            println!("   DCR: {}", if dcr { "enabled" } else { "disabled" });
                        }
                        if let Some(scopes) = endpoints.scopes {
                            println!("   Scopes: {}", scopes.join(", "));
                        }
                        println!();
                        println!("OAuth Endpoints:");
                        if let Some(ref discovery) = endpoints.discovery_url {
                            println!("   Discovery:     {}", discovery);
                        }
                        if let Some(ref register) = endpoints.registration_endpoint {
                            println!("   Registration:  {}", register);
                        }
                        if let Some(ref authorize) = endpoints.authorization_endpoint {
                            println!("   Authorization: {}", authorize);
                        }
                        if let Some(ref token) = endpoints.token_endpoint {
                            println!("   Token:         {}", token);
                        }
                        println!();
                        println!("Cognito Details:");
                        if let Some(ref pool_id) = endpoints.user_pool_id {
                            println!("   User Pool ID:  {}", pool_id);
                        }
                        if let Some(ref region) = endpoints.user_pool_region {
                            println!("   Region:        {}", region);
                        }
                    } else {
                        println!("   Status: Disabled");
                        if std::env::var("PMCP_QUIET").is_err() {
                            println!();
                            println!("Enable OAuth with:");
                            println!("   cargo pmcp deploy oauth enable --server {}", server);
                        }
                    }
                },
                Err(_) => {
                    println!("   Status: Not configured");
                    if std::env::var("PMCP_QUIET").is_err() {
                        println!();
                        println!("Enable OAuth with:");
                        println!("   cargo pmcp deploy oauth enable --server {}", server);
                    }
                },
            }

            Ok(())
        },
    }
}

/// Resolve OAuth configuration with priority: explicit params > copied values > defaults
///
/// This function implements the configuration resolution logic:
/// 1. If `copy_from` is specified, fetch OAuth config from that server
/// 2. Use explicit parameters to override copied values
/// 3. Apply sensible defaults for any remaining unspecified values
async fn resolve_oauth_config(
    access_token: &str,
    copy_from: Option<&str>,
    explicit_scopes: Option<Vec<String>>,
    explicit_dcr: bool,
    explicit_public_clients: Option<Vec<String>>,
    explicit_shared_pool: Option<String>,
) -> Result<(Vec<String>, bool, Option<Vec<String>>, Option<String>)> {
    use crate::deployment::targets::pmcp_run::graphql;

    // Start with defaults
    let default_scopes: Vec<String> = DEFAULT_OAUTH_SCOPES.iter().map(|s| s.to_string()).collect();
    let default_public_clients: Vec<String> = DEFAULT_PUBLIC_CLIENT_PATTERNS
        .iter()
        .map(|s| s.to_string())
        .collect();

    // If copy_from is specified, fetch config from source server
    let (copied_scopes, copied_dcr, copied_public_clients, copied_pool) =
        if let Some(source_server) = copy_from {
            if std::env::var("PMCP_QUIET").is_err() {
                println!("Copying OAuth configuration from: {}", source_server);
            }

            match graphql::fetch_server_oauth_endpoints(access_token, source_server).await {
                Ok(endpoints) => {
                    if !endpoints.oauth_enabled {
                        bail!(
                            "Source server '{}' does not have OAuth enabled. \
                         Cannot copy configuration from a server without OAuth.",
                            source_server
                        );
                    }

                    let pool_id = endpoints.user_pool_id.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Source server '{}' has OAuth enabled but no User Pool ID. \
                         This is unexpected - please check the server configuration.",
                            source_server
                        )
                    })?;

                    if std::env::var("PMCP_QUIET").is_err() {
                        println!("   Found User Pool: {}", pool_id);
                        if let Some(ref scopes) = endpoints.scopes {
                            println!("   Found scopes: {}", scopes.join(", "));
                        }
                        println!();
                    }

                    (
                        endpoints.scopes,
                        endpoints.dcr_enabled,
                        None, // Public client patterns not returned by status endpoint
                        Some(pool_id),
                    )
                },
                Err(e) => {
                    bail!(
                        "Failed to fetch OAuth configuration from '{}': {}\n\
                     Make sure the server exists and has OAuth enabled.\n\
                     You can check with: cargo pmcp deploy oauth status --server {}",
                        source_server,
                        e,
                        source_server
                    );
                },
            }
        } else {
            (None, None, None, None)
        };

    // Resolve final values with priority: explicit > copied > default
    // For scopes: explicit provided OR copied from source OR default
    let final_scopes = explicit_scopes.or(copied_scopes).unwrap_or(default_scopes);

    // For DCR: explicit is always used (it has a default_value in clap)
    // But if copying and explicit wasn't changed from default, prefer copied
    let final_dcr = if copy_from.is_some() && explicit_dcr {
        // User didn't override DCR (it's still the default true)
        // Use copied value if available, otherwise use explicit (which is default true)
        copied_dcr.unwrap_or(explicit_dcr)
    } else {
        explicit_dcr
    };

    // For public clients: explicit OR copied OR default (when using shared pool)
    let final_public_clients = if explicit_public_clients.is_some() {
        explicit_public_clients
    } else if copied_public_clients.is_some() {
        copied_public_clients
    } else if copy_from.is_some() || explicit_shared_pool.is_some() {
        // When using shared pool or copying, apply default public clients
        Some(default_public_clients)
    } else {
        None // Let backend use its defaults
    };

    // For shared pool: explicit OR copied
    let final_shared_pool = explicit_shared_pool.or(copied_pool);

    Ok((
        final_scopes,
        final_dcr,
        final_public_clients,
        final_shared_pool,
    ))
}
