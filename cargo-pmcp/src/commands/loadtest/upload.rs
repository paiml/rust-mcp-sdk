//! Upload loadtest configs to pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};
use cargo_pmcp::loadtest::config::LoadTestConfig;

/// Upload a loadtest TOML config to pmcp.run for cloud execution.
pub async fn execute(
    server_id: String,
    path: PathBuf,
    name: Option<String>,
    description: Option<String>,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!(
            "\n{}",
            "Uploading loadtest config to pmcp.run".bright_cyan().bold()
        );
        println!(
            "{}",
            "─────────────────────────────────────────".bright_cyan()
        );
    }

    // Step 1: Read the file
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    // Step 2: Validate TOML — parse and check scenarios exist
    //
    // This catches: missing file (above), invalid TOML syntax, missing [settings],
    // missing [[scenario]], zero total weight, invalid stage durations.
    // Uses the same validation as `cargo pmcp loadtest run`.
    if let Err(e) = LoadTestConfig::from_toml(&content) {
        if global_flags.should_output() {
            eprintln!("\n  {} {}", "Error:".red().bold(), e);
            eprintln!();
            eprintln!(
                "  {}",
                "The config file failed validation. To fix:".yellow()
            );
            eprintln!("    - Ensure the file is valid TOML syntax");
            eprintln!(
                "    - Include a [settings] block with virtual_users, duration_secs, timeout_ms"
            );
            eprintln!(
                "    - Include at least one [[scenario]] block with type, weight, and operation fields"
            );
            eprintln!("    - Run `cargo pmcp loadtest init` to generate a valid starter config");
        }
        anyhow::bail!("Config validation failed: {}", e);
    }

    // Step 3: Derive config name from filename if not provided
    let config_name = name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("loadtest")
            .to_string()
    });

    if global_flags.should_output() {
        println!("  {} Config: {}", "->".blue(), path.display());
        println!("  {} Name: {}", "->".blue(), config_name);
        println!("  {} Server ID: {}", "->".blue(), server_id);
        println!("  {} Validated: config has valid scenarios", "OK".green());
    }

    // Step 4: Authenticate with pmcp.run
    let credentials = auth::get_credentials().await?;

    // Step 5: Upload via GraphQL
    if global_flags.should_output() {
        println!("\n  Uploading config...");
    }

    match graphql::upload_loadtest_scenario(
        &credentials.access_token,
        &server_id,
        &config_name,
        description.as_deref(),
        &content,
    )
    .await
    {
        Ok(result) => {
            if global_flags.should_output() {
                println!();
                println!(
                    "{}",
                    "===========================================".bright_cyan()
                );
                println!(
                    "{} Uploaded '{}' (scenario: {}, v{})",
                    "OK".green().bold(),
                    config_name,
                    result.scenario_id,
                    result.version
                );
                println!();
                println!("{}", "Next steps:".bright_white().bold());
                println!(
                    "  - View config at: https://pmcp.run/servers/{}/loadtest",
                    server_id
                );
                println!("  - Trigger a cloud load test from the pmcp.run dashboard");
                println!(
                    "  - Or run locally: cargo pmcp loadtest run <url> --config {}",
                    path.display()
                );
                println!(
                    "{}",
                    "===========================================".bright_cyan()
                );
            }
        },
        Err(e) => {
            if global_flags.should_output() {
                eprintln!();
                eprintln!(
                    "{}",
                    "===========================================".bright_cyan()
                );
                eprintln!("{} Upload failed: {}", "X".red().bold(), e);
                eprintln!();
                eprintln!("{}", "Troubleshooting:".bright_white().bold());
                eprintln!("  - Verify the server ID is correct");
                eprintln!("  - Check your pmcp.run authentication: cargo pmcp auth login");
                eprintln!(
                    "  - Ensure the server exists: https://pmcp.run/servers/{}",
                    server_id
                );
                eprintln!(
                    "{}",
                    "===========================================".bright_cyan()
                );
            }
            anyhow::bail!("Upload failed: {}", e);
        },
    }

    Ok(())
}
