//! List test scenarios on pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// List test scenarios for an MCP server on pmcp.run
pub async fn execute(server_id: String, show_all: bool, global_flags: &GlobalFlags) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Test scenarios on pmcp.run".bright_cyan().bold());
        println!("{}", "─────────────────────────────────────".bright_cyan());
    }

    // Get credentials
    let credentials = auth::get_credentials().await?;

    if global_flags.should_output() {
        println!("  {} Server ID: {}", "→".blue(), server_id);
    }

    let result = graphql::list_test_scenarios(&credentials.access_token, &server_id)
        .await
        .context("Failed to list scenarios")?;

    println!();

    if result.scenarios.is_empty() {
        println!("{}", "No test scenarios found".yellow());
        if global_flags.should_output() {
            println!();
            println!("{}", "To add scenarios:".bright_white().bold());
            println!("  1. Generate scenarios locally:");
            println!("     cargo pmcp test generate --url <server-url>");
            println!();
            println!("  2. Upload to pmcp.run:");
            println!(
                "     cargo pmcp test upload --server {} scenarios/",
                server_id
            );
        }
        return Ok(());
    }

    // Count by status
    let enabled_count = result.scenarios.iter().filter(|s| s.enabled).count();
    let disabled_count = result.scenarios.len() - enabled_count;

    // Requested output: scenario listing
    println!(
        "{} {} scenario(s) ({} enabled, {} disabled)",
        "📋".to_string(),
        result.scenarios.len(),
        enabled_count,
        disabled_count
    );
    println!();

    // Print header
    println!(
        "  {:<40} {:<12} {:<10} {:<8} {}",
        "NAME".bright_white().bold(),
        "SOURCE".bright_white().bold(),
        "STATUS".bright_white().bold(),
        "VERSION".bright_white().bold(),
        "LAST RUN".bright_white().bold()
    );
    println!("  {}", "─".repeat(90));

    for scenario in &result.scenarios {
        // Skip disabled if not showing all
        if !show_all && !scenario.enabled {
            continue;
        }

        let status = if scenario.enabled {
            "enabled".green().to_string()
        } else {
            "disabled".yellow().to_string()
        };

        let last_run = match &scenario.last_execution_status {
            Some(status) => {
                let status_colored = match status.as_str() {
                    "passed" => "passed".green().to_string(),
                    "failed" => "failed".red().to_string(),
                    "error" => "error".red().to_string(),
                    "running" => "running".blue().to_string(),
                    _ => status.clone(),
                };
                format!(
                    "{} ({})",
                    scenario.last_executed_at.as_deref().unwrap_or("-"),
                    status_colored
                )
            },
            None => "-".to_string(),
        };

        let source_display = match scenario.source.as_str() {
            "auto_generated" => "auto".cyan().to_string(),
            "user_created" => "user".to_string(),
            "user_modified" => "modified".to_string(),
            "imported" => "imported".to_string(),
            _ => scenario.source.clone(),
        };

        println!(
            "  {:<40} {:<12} {:<10} v{:<7} {}",
            truncate_string(&scenario.name, 38),
            source_display,
            status,
            scenario.version,
            last_run
        );

        // Show description if available
        if let Some(ref desc) = scenario.description {
            if !desc.is_empty() {
                println!("    {}", desc.bright_black());
            }
        }
    }

    if global_flags.should_output() {
        println!();
        println!(
            "{}",
            "═════════════════════════════════════════════════════════════════".bright_cyan()
        );
        println!();
        println!("{}", "Commands:".bright_white().bold());
        println!("  Download a scenario:  cargo pmcp test download --scenario-id <id>");
        println!(
            "  Upload scenarios:     cargo pmcp test upload --server {} <path>",
            server_id
        );

        if disabled_count > 0 && !show_all {
            println!();
            println!(
                "  {} {} disabled scenario(s) hidden. Use --all to show all.",
                "ℹ".blue(),
                disabled_count
            );
        }
    }

    Ok(())
}

/// Truncate string to max length, adding ellipsis if needed
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
