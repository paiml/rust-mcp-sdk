//! Upload test scenarios to pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// Upload test scenarios to pmcp.run for scheduled testing
pub async fn execute(
    server_id: String,
    paths: Vec<PathBuf>,
    name: Option<String>,
    description: Option<String>,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!(
            "\n{}",
            "Uploading test scenarios to pmcp.run".bright_cyan().bold()
        );
        println!(
            "{}",
            "─────────────────────────────────────────".bright_cyan()
        );
    }

    // Get credentials
    let credentials = auth::get_credentials().await?;

    // Collect scenario files
    let mut scenario_files: Vec<PathBuf> = Vec::new();

    for path in paths {
        if path.is_dir() {
            // Find all YAML files in directory
            for entry in std::fs::read_dir(&path)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("yaml")
                    || file_path.extension().and_then(|s| s.to_str()) == Some("yml")
                {
                    scenario_files.push(file_path);
                }
            }
        } else if path.exists() {
            scenario_files.push(path);
        } else {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
    }

    if scenario_files.is_empty() {
        anyhow::bail!("No scenario files found");
    }

    if global_flags.should_output() {
        println!(
            "  {} Found {} scenario file(s)",
            "→".blue(),
            scenario_files.len()
        );
        println!("  {} Server ID: {}", "→".blue(), server_id);
    }

    let mut uploaded = 0;
    let mut failed = 0;

    for file_path in scenario_files {
        let file_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();

        let scenario_name = name.clone().unwrap_or(file_name);

        if global_flags.should_output() {
            println!("\n  Uploading: {}", file_path.display());
        }

        // Read scenario content
        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read scenario file: {}", file_path.display()))?;

        // Determine format from extension
        let format = if file_path.extension().and_then(|s| s.to_str()) == Some("json") {
            "json"
        } else {
            "yaml"
        };

        match graphql::upload_test_scenario(
            &credentials.access_token,
            &server_id,
            &scenario_name,
            description.as_deref(),
            &content,
            format,
        )
        .await
        {
            Ok(result) => {
                if global_flags.should_output() {
                    println!(
                        "    {} Uploaded as '{}' (v{})",
                        "✓".green(),
                        scenario_name,
                        result.version
                    );
                }
                uploaded += 1;
            },
            Err(e) => {
                println!("    {} Failed: {}", "✗".red(), e);
                failed += 1;
            },
        }
    }

    if global_flags.should_output() {
        println!();
        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );

        if failed == 0 {
            println!(
                "{} Uploaded {} scenario(s) successfully!",
                "✓".green().bold(),
                uploaded
            );
            println!();
            println!("{}", "Next steps:".bright_white().bold());
            println!(
                "  - View scenarios at: https://pmcp.run/servers/{}",
                server_id
            );
            println!("  - Configure scheduled testing in the dashboard");
            println!(
                "  - Or run tests manually: cargo pmcp test remote --server {}",
                server_id
            );
        } else {
            println!(
                "{} Uploaded {} scenario(s), {} failed",
                "⚠".yellow().bold(),
                uploaded,
                failed
            );
        }

        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );
    }

    if failed > 0 && uploaded == 0 {
        anyhow::bail!("All scenario uploads failed");
    }

    Ok(())
}
