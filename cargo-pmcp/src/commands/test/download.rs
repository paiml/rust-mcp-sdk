//! Download test scenarios from pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// Download test scenarios from pmcp.run
pub async fn execute(
    scenario_id: String,
    output: Option<PathBuf>,
    format: Option<String>,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!(
            "\n{}",
            "Downloading test scenario from pmcp.run"
                .bright_cyan()
                .bold()
        );
        println!(
            "{}",
            "─────────────────────────────────────────".bright_cyan()
        );
    }

    // Get credentials
    let credentials = auth::get_credentials().await?;

    if global_flags.should_output() {
        println!("  {} Scenario ID: {}", "→".blue(), scenario_id);
    }

    let format_str = format.as_deref().unwrap_or("yaml");

    let result =
        graphql::download_test_scenario(&credentials.access_token, &scenario_id, format_str)
            .await
            .context("Failed to download scenario")?;

    // Determine output path
    let output_path = if let Some(path) = output {
        path
    } else {
        let ext = if format_str == "json" { "json" } else { "yaml" };
        PathBuf::from(format!("{}.{}", result.name, ext))
    };

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Write scenario content
    std::fs::write(&output_path, &result.content)
        .with_context(|| format!("Failed to write scenario to {}", output_path.display()))?;

    if global_flags.should_output() {
        println!();
        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );
        println!(
            "{} Downloaded '{}' (v{}) to {}",
            "✓".green().bold(),
            result.name,
            result.version,
            output_path.display()
        );
        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );
        println!();
        println!("{}", "Next steps:".bright_white().bold());
        println!("  - Edit the scenario locally: {}", output_path.display());
        println!(
            "  - Run tests: cargo pmcp test run --scenarios {}",
            output_path.display()
        );
        println!(
            "  - Upload changes: cargo pmcp test upload --server <id> {}",
            output_path.display()
        );
    }

    Ok(())
}
