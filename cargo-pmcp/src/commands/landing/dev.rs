//! Run landing page development server

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::landing::config::LandingConfig;

/// Run the development server
pub async fn run_dev_server(
    _project_root: PathBuf,
    dir: PathBuf,
    port: u16,
    _watch: bool, // TODO: Implement watch mode in P1
) -> Result<()> {
    println!("🚀 Starting development server...");
    println!();

    // Check if landing directory exists
    if !dir.exists() {
        anyhow::bail!(
            "Landing directory not found: {}\n\
             Run 'cargo pmcp landing init' first",
            dir.display()
        );
    }

    // Check if pmcp-landing.toml exists
    let config_path = dir.join("pmcp-landing.toml");
    if !config_path.exists() {
        anyhow::bail!(
            "Configuration file not found: {}\n\
             Make sure you're in the correct directory",
            config_path.display()
        );
    }

    // Load configuration
    let config = LandingConfig::load(&config_path)?;
    println!("📝 Loaded configuration for: {}", config.display_title());
    println!();

    // Check if Node.js is installed
    check_node_installed()?;

    // Check if node_modules exists, if not run npm install
    let node_modules = dir.join("node_modules");
    if !node_modules.exists() {
        println!("📦 Installing dependencies...");
        run_npm_install(&dir)?;
        println!();
    }

    // Set environment variables
    let endpoint = config
        .deployment
        .endpoint
        .as_deref()
        .unwrap_or("http://localhost:3000");

    println!("🌐 Server configuration:");
    println!("   Endpoint: {}", endpoint);
    println!("   Port: {}", port);
    println!();

    // Run npm run dev
    println!("✨ Starting Next.js development server...");
    println!("   Open: http://localhost:{}", port);
    println!();
    println!("Press Ctrl+C to stop the server");
    println!();

    let status = Command::new("npm")
        .arg("run")
        .arg("dev")
        .current_dir(&dir)
        .env("MCP_ENDPOINT", endpoint)
        .env("MCP_SERVER_NAME", &config.landing.server_name)
        .env("PORT", port.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to run npm run dev")?;

    if !status.success() {
        anyhow::bail!("Development server exited with error");
    }

    Ok(())
}

/// Check if Node.js is installed
fn check_node_installed() -> Result<()> {
    let output = Command::new("node")
        .arg("--version")
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("✅ Node.js found: {}", version.trim());
            Ok(())
        }
        _ => {
            anyhow::bail!(
                "Node.js not found. Please install Node.js 18+ from:\n\
                 https://nodejs.org/"
            );
        }
    }
}

/// Run npm install
fn run_npm_install(dir: &PathBuf) -> Result<()> {
    print!("   Running npm install...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let output = Command::new("npm")
        .arg("install")
        .current_dir(dir)
        .output()
        .context("Failed to run npm install")?;

    if !output.status.success() {
        println!(" ❌");
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("npm install failed:\n{}", stderr);
    }

    println!(" ✅");
    Ok(())
}
