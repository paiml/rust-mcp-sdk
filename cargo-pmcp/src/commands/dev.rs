//! Development server command

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;

use crate::utils::config::WorkspaceConfig;

/// Start development server
pub fn execute(
    server: String,
    mut port: u16,
    connect_client: Option<String>,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Starting development server".bright_cyan().bold());
        println!("{}", "────────────────────────────────────".bright_cyan());
    }

    // Verify we're in a workspace
    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo pmcp new <name>' first.");
    }

    // Load workspace config and use configured port if available
    let config = WorkspaceConfig::load()?;
    if let Some(server_config) = config.get_server(&server) {
        // Use configured port (overrides CLI --port unless explicitly set)
        if port == 3000 {
            // Default port, use configured one
            port = server_config.port;
            if global_flags.should_output() {
                println!(
                    "  {} Using configured port {}",
                    "→".blue(),
                    port.to_string().bright_yellow()
                );
            }
        }
    }

    // Verify server exists
    let server_binary = format!("{}-server", server);

    if global_flags.should_output() {
        println!("\n{}", "Step 1: Building server".bright_white().bold());
    }
    let build_status = Command::new("cargo")
        .args(["build", "--bin", &server_binary])
        .status()
        .context("Failed to build server")?;

    if !build_status.success() {
        anyhow::bail!("Server build failed");
    }
    if global_flags.should_output() {
        println!("  {} Server built successfully", "✓".green());

        println!("\n{}", "Step 2: Starting server".bright_white().bold());
    }
    let url = format!("http://0.0.0.0:{}", port);
    if global_flags.should_output() {
        println!("  {} Server URL: {}", "→".blue(), url.bright_yellow());
    }

    // If connect_client is specified, run connect command first
    if let Some(client) = connect_client {
        if global_flags.should_output() {
            println!(
                "\n{}",
                "Step 3: Connecting to MCP client".bright_white().bold()
            );
        }
        super::connect::execute(server.clone(), client, url.clone(), global_flags)?;
        if global_flags.should_output() {
            println!();
        }
    }

    if global_flags.should_output() {
        println!("{}", "─────────────────────────────────────".bright_cyan());
        println!("{}", "Server is starting...".bright_white().bold());
        println!("Press Ctrl+C to stop");
        println!("{}", "─────────────────────────────────────".bright_cyan());
        println!();
    }

    // Start server in foreground (user sees logs)
    let status = Command::new("cargo")
        .args(["run", "--bin", &server_binary])
        .env("MCP_HTTP_PORT", port.to_string())
        .env("RUST_LOG", "info")
        .status()
        .context("Failed to start server")?;

    if !status.success() {
        anyhow::bail!("Server exited with error");
    }

    Ok(())
}
