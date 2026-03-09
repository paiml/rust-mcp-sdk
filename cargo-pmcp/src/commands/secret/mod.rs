//! Secret management CLI commands.
//!
//! Provides `cargo pmcp secret` commands for managing secrets across
//! multiple providers (local, pmcp.run, AWS).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use crate::secrets::{
    config::{detect_target, SecretTarget, SecretsConfig},
    ListOptions, ProviderRegistry, SecretCharset, SecretValue, SetOptions,
};

/// Manage secrets for MCP servers
#[derive(Debug, Parser)]
pub struct SecretCommand {
    /// Target provider (pmcp, aws, local)
    #[arg(long, global = true)]
    target: Option<String>,

    /// Profile from .pmcp/config.toml
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Server ID for namespacing secrets
    #[arg(long, global = true)]
    server: Option<String>,

    /// Output format (text, json)
    #[arg(long, default_value = "text", global = true)]
    format: String,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    action: SecretAction,
}

#[derive(Debug, Subcommand)]
pub enum SecretAction {
    /// List secrets (names only, never values)
    List {
        /// Filter by name pattern (glob syntax)
        #[arg(long)]
        filter: Option<String>,

        /// Include metadata (creation date, version)
        #[arg(long)]
        metadata: bool,
    },

    /// Get a secret value
    Get {
        /// Secret name (format: server-id/SECRET_NAME)
        name: String,

        /// Write to file instead of stdout
        #[arg(long)]
        output: Option<PathBuf>,

        /// Omit trailing newline (for piping)
        #[arg(long)]
        no_newline: bool,
    },

    /// Set a secret value
    Set {
        /// Secret name (format: server-id/SECRET_NAME)
        name: String,

        /// Interactive hidden input (recommended)
        #[arg(long, conflicts_with_all = ["stdin", "file", "env", "value", "generate"])]
        prompt: bool,

        /// Read from stdin
        #[arg(long, conflicts_with_all = ["prompt", "file", "env", "value", "generate"])]
        stdin: bool,

        /// Read from file
        #[arg(long, conflicts_with_all = ["prompt", "stdin", "env", "value", "generate"])]
        file: Option<PathBuf>,

        /// Read from environment variable
        #[arg(long, conflicts_with_all = ["prompt", "stdin", "file", "value", "generate"])]
        env: Option<String>,

        /// Direct value (WARNING: visible in process list)
        #[arg(long, conflicts_with_all = ["prompt", "stdin", "file", "env", "generate"])]
        value: Option<String>,

        /// Generate random value
        #[arg(long, conflicts_with_all = ["prompt", "stdin", "file", "env", "value"])]
        generate: bool,

        /// Length for generated secrets
        #[arg(long, default_value = "32")]
        generate_length: usize,

        /// Charset for generated secrets (alphanumeric, ascii, hex)
        #[arg(long, default_value = "alphanumeric")]
        generate_charset: String,

        /// Human-readable description
        #[arg(long)]
        description: Option<String>,

        /// Fail if secret already exists
        #[arg(long)]
        no_overwrite: bool,
    },

    /// Delete a secret
    Delete {
        /// Secret name (format: server-id/SECRET_NAME)
        name: String,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },

    /// Show provider status
    Providers {
        /// Check connectivity to each provider
        #[arg(long)]
        check: bool,
    },

    /// Sync secrets from configuration
    Sync {
        /// TOML file to analyze
        #[arg(long, default_value = "pmcp.toml")]
        file: PathBuf,

        /// Check only, no changes
        #[arg(long)]
        check: bool,

        /// Prompt for each missing secret
        #[arg(long)]
        interactive: bool,
    },
}

impl SecretCommand {
    pub fn execute(&self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        // The secret module already has its own --quiet flag.
        // Merge with global --quiet so either suppresses output.
        let effective_quiet = self.quiet || global_flags.quiet;
        tokio::runtime::Runtime::new()?.block_on(self.execute_async_with_quiet(effective_quiet))
    }

    async fn execute_async_with_quiet(&self, quiet: bool) -> Result<()> {
        let project_root = std::env::current_dir()?;
        let config = SecretsConfig::load(&project_root)?;

        // Determine target
        let target = if let Some(ref target_str) = self.target {
            target_str.parse::<SecretTarget>()?
        } else {
            config.get_target(self.profile.as_deref())
        };

        // Auto-detect if still default
        let target = if target == SecretTarget::Local && self.target.is_none() {
            detect_target()
        } else {
            target
        };

        let registry = ProviderRegistry::new(&project_root, &config);
        let provider = registry.get_for_target(target.clone())?;

        match &self.action {
            SecretAction::List { filter, metadata } => {
                let options = ListOptions {
                    filter: filter.clone(),
                    server_id: self.server.clone(),
                    include_metadata: *metadata,
                };

                let result = provider.list(options).await?;

                if self.format == "json" {
                    println!("{}", serde_json::to_string_pretty(&result.secrets)?);
                } else {
                    if result.secrets.is_empty() {
                        if !quiet {
                            println!("No secrets found.");
                        }
                    } else {
                        println!("{:<40} {:<10} {}", "NAME", "VERSION", "MODIFIED");
                        for secret in &result.secrets {
                            let version = secret
                                .metadata
                                .version
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let modified = secret.metadata.modified_at.as_deref().unwrap_or("-");
                            println!("{:<40} {:<10} {}", secret.name, version, modified);
                        }
                    }
                }
            },

            SecretAction::Get {
                name,
                output,
                no_newline,
            } => {
                // Security warning for terminal output
                if output.is_none() && io::stdout().is_terminal() && !quiet {
                    eprintln!("⚠️  Warning: Outputting secret to terminal.");
                    eprintln!("   Consider using --output <file> or piping.");
                    eprintln!();
                }

                let secret_name = self.resolve_secret_name(name)?;
                let value = provider.get(&secret_name).await?;

                if let Some(output_path) = output {
                    // Write to file with restricted permissions
                    let mut file = std::fs::File::create(output_path)?;
                    file.write_all(value.expose().as_bytes())?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = file.metadata()?.permissions();
                        perms.set_mode(0o600);
                        std::fs::set_permissions(output_path, perms)?;
                    }

                    if !quiet {
                        println!("Secret written to: {}", output_path.display());
                    }
                } else {
                    // Output to stdout
                    if *no_newline {
                        print!("{}", value.expose());
                    } else {
                        println!("{}", value.expose());
                    }
                }
            },

            SecretAction::Set {
                name,
                prompt,
                stdin,
                file,
                env,
                value,
                generate,
                generate_length,
                generate_charset,
                description,
                no_overwrite,
            } => {
                // Determine input method
                let secret_value = if *generate {
                    let charset: SecretCharset = generate_charset
                        .parse()
                        .map_err(|e: String| anyhow::anyhow!(e))?;
                    SecretValue::generate(*generate_length, charset)
                } else if let Some(env_var) = env {
                    let val = std::env::var(env_var)
                        .with_context(|| format!("Environment variable '{}' not set", env_var))?;
                    SecretValue::new(val)
                } else if let Some(file_path) = file {
                    let val = std::fs::read_to_string(file_path)?;
                    SecretValue::new(val.trim_end().to_string())
                } else if *stdin {
                    let mut val = String::new();
                    io::stdin().read_to_string(&mut val)?;
                    SecretValue::new(val.trim_end().to_string())
                } else if let Some(direct_value) = value {
                    // Warn about direct value
                    if !quiet {
                        eprintln!("⚠️  SECURITY WARNING: Passing secrets via --value is insecure!");
                        eprintln!("   The value may appear in:");
                        eprintln!("     - Shell history");
                        eprintln!("     - Process listings");
                        eprintln!("     - System logs");
                        eprintln!();
                        eprintln!("   Recommended alternatives:");
                        eprintln!("     - cargo pmcp secret set NAME --prompt");
                        eprintln!("     - echo -n 'value' | cargo pmcp secret set NAME --stdin");
                        eprintln!();
                    }
                    SecretValue::new(direct_value.clone())
                } else if *prompt || io::stdin().is_terminal() {
                    // Default to prompt for interactive use
                    let val = rpassword::prompt_password("Enter secret value: ")?;
                    SecretValue::new(val)
                } else {
                    // Fall back to stdin if not a tty
                    let mut val = String::new();
                    io::stdin().read_to_string(&mut val)?;
                    SecretValue::new(val.trim_end().to_string())
                };

                let secret_name = self.resolve_secret_name(name)?;
                let options = SetOptions {
                    description: description.clone(),
                    no_overwrite: *no_overwrite,
                    server_id: self.server.clone(),
                    ..Default::default()
                };

                let metadata = provider.set(&secret_name, secret_value, options).await?;

                if !quiet {
                    println!("✅ Secret '{}' set successfully.", secret_name);
                    if let Some(version) = metadata.version {
                        println!("   Version: {}", version);
                    }
                }
            },

            SecretAction::Delete { name, force } => {
                let secret_name = self.resolve_secret_name(name)?;

                // Confirm deletion
                if !force {
                    print!(
                        "Are you sure you want to delete '{}'? Type the secret name to confirm: ",
                        secret_name
                    );
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim() != secret_name {
                        println!("❌ Confirmation failed. Aborting.");
                        return Ok(());
                    }
                }

                provider.delete(&secret_name, *force).await?;

                if !quiet {
                    println!("✅ Secret '{}' deleted.", secret_name);
                }
            },

            SecretAction::Providers { check } => {
                if *check {
                    let health_results = registry.check_all_health().await;

                    println!("{:<15} {:<15} {}", "PROVIDER", "STATUS", "AUTH METHOD");
                    for (id, health) in health_results {
                        let status = if health.available {
                            "✓ connected"
                        } else {
                            "✗ not auth"
                        };
                        let auth = health.auth_method.as_deref().unwrap_or("-");
                        let extra = health.user.as_deref().unwrap_or("");
                        println!("{:<15} {:<15} {} {}", id, status, auth, extra);

                        if let Some(ref message) = health.message {
                            if !health.available {
                                println!("               {}", message);
                            }
                        }
                    }
                } else {
                    println!("Available providers:");
                    for provider in registry.list() {
                        let caps = provider.capabilities();
                        println!("  {} ({})", provider.id(), provider.name());
                        println!("    Max size: {} bytes", caps.max_value_size);
                        println!(
                            "    Features: {}{}{}",
                            if caps.versioning { "versioning " } else { "" },
                            if caps.tags { "tags " } else { "" },
                            if caps.descriptions {
                                "descriptions"
                            } else {
                                ""
                            }
                        );
                    }
                    println!();
                    println!("Use --check to verify connectivity.");
                }
            },

            SecretAction::Sync {
                file,
                check,
                interactive,
            } => {
                // Parse the TOML file to find secret references
                if !file.exists() {
                    anyhow::bail!("Configuration file not found: {}", file.display());
                }

                let content = std::fs::read_to_string(file)?;
                let secrets_refs = parse_secret_references(&content);

                if secrets_refs.is_empty() {
                    println!("No secret references found in {}", file.display());
                    return Ok(());
                }

                println!("Secrets referenced in configuration:");

                // Check which secrets exist
                let list_result = provider.list(ListOptions::default()).await?;
                let existing: std::collections::HashSet<_> =
                    list_result.secrets.iter().map(|s| &s.name).collect();

                let mut missing = Vec::new();
                for secret_ref in &secrets_refs {
                    if existing.contains(secret_ref) {
                        println!("  ✓ {}  exists", secret_ref);
                    } else {
                        println!("  ✗ {}  MISSING", secret_ref);
                        missing.push(secret_ref.clone());
                    }
                }

                if missing.is_empty() {
                    println!();
                    println!("All secrets are configured.");
                } else {
                    println!();
                    println!("{} secret(s) missing.", missing.len());

                    if *check {
                        // Just report, don't create
                        println!("Run with --interactive to create missing secrets.");
                    } else if *interactive {
                        // Prompt for each missing secret
                        for name in &missing {
                            println!();
                            println!("Creating secret: {}", name);
                            let value = rpassword::prompt_password("  Enter value (hidden): ")?;

                            provider
                                .set(name, SecretValue::new(value), SetOptions::default())
                                .await?;
                            println!("  ✓ Created");
                        }
                    }
                }
            },
        }

        Ok(())
    }

    /// Resolve secret name, adding server prefix if not present.
    fn resolve_secret_name(&self, name: &str) -> Result<String> {
        if name.contains('/') {
            // Already has server prefix
            Ok(name.to_string())
        } else if let Some(ref server) = self.server {
            // Add server prefix
            Ok(format!("{}/{}", server, name))
        } else {
            anyhow::bail!(
                "Secret name must include server prefix (e.g., 'my-server/API_KEY') \
                 or use --server flag"
            )
        }
    }
}

/// Parse secret references from TOML content.
fn parse_secret_references(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let re = regex::Regex::new(r#"secret:([a-zA-Z0-9_\-/]+)"#).unwrap();

    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            refs.push(m.as_str().to_string());
        }
    }

    refs
}
