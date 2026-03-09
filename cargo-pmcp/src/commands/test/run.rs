//! Run tests locally against an MCP server

use anyhow::Result;
use colored::Colorize;
use mcp_tester::run_scenario_with_transport;
use std::path::PathBuf;

use crate::commands::GlobalFlags;

/// Run test scenarios against a local or remote MCP server
pub fn execute(
    server: Option<String>,
    url: Option<String>,
    port: u16,
    scenarios: Option<PathBuf>,
    transport: Option<String>,
    detailed: bool,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Running MCP server tests".bright_cyan().bold());
        println!("{}", "─────────────────────────────────────".bright_cyan());
    }

    // Determine the target URL
    let target_url = if let Some(url) = url {
        url
    } else if server.is_some() {
        format!("http://0.0.0.0:{}", port)
    } else {
        anyhow::bail!("Either --url or --server must be specified");
    };

    // Determine scenarios directory
    let scenarios_dir = if let Some(dir) = scenarios {
        dir
    } else if let Some(server) = &server {
        PathBuf::from("scenarios").join(server)
    } else {
        // Try to find scenarios in current directory
        PathBuf::from("scenarios")
    };

    if global_flags.should_output() {
        if let Some(server) = &server {
            println!("\n{}", "Prerequisites:".bright_white().bold());
            println!("  {} Server must be running on port {}", "→".blue(), port);
            println!(
                "  {} Run in another terminal: {}",
                "→".blue(),
                format!("cargo pmcp dev --server {}", server).bright_cyan()
            );
        }

        // Run tests
        println!("\n{}", "Running tests".bright_white().bold());
        println!("  {} Target: {}", "→".blue(), target_url);
    }

    // Try scenario-based testing if scenarios exist
    let test_result = if scenarios_dir.exists() && scenarios_dir.read_dir()?.next().is_some() {
        // Find YAML scenarios
        let scenarios: Vec<_> = std::fs::read_dir(&scenarios_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "yaml" || s == "yml")
                    .unwrap_or(false)
            })
            .collect();

        if !scenarios.is_empty() {
            if global_flags.should_output() {
                println!(
                    "  {} Running {} scenario file(s) from {}",
                    "→".blue(),
                    scenarios.len(),
                    scenarios_dir.display()
                );
            }

            // Run each scenario using library
            let mut all_passed = true;
            for scenario in scenarios {
                let scenario_path = scenario.path();
                if global_flags.should_output() {
                    println!("\n  Testing: {}", scenario_path.display());
                }

                let result = tokio::runtime::Runtime::new()?.block_on(async {
                    run_scenario_with_transport(
                        scenario_path.to_str().unwrap(),
                        &target_url,
                        detailed,
                        transport.as_deref(),
                    )
                    .await
                });

                match result {
                    Ok(_) => {
                        println!("  {} Passed", "✓".green());
                    },
                    Err(e) => {
                        println!("  {} Failed: {}", "✗".red(), e);
                        all_passed = false;
                    },
                }
            }

            Ok(all_passed)
        } else {
            // No scenarios found
            if global_flags.should_output() {
                println!(
                    "  {} No scenarios found in {}",
                    "⚠".yellow(),
                    scenarios_dir.display()
                );
                println!("    Run 'cargo pmcp test generate' to create test scenarios");
            }
            Ok(true)
        }
    } else {
        // No scenarios directory
        if global_flags.should_output() {
            println!(
                "  {} No scenarios directory found at {}",
                "⚠".yellow(),
                scenarios_dir.display()
            );
            println!("    Run 'cargo pmcp test generate' to create test scenarios");
        }
        Ok(true)
    };

    if global_flags.should_output() {
        println!();
        println!("{}", "═════════════════════════════════════".bright_cyan());
    }

    match test_result {
        Ok(true) => {
            if global_flags.should_output() {
                println!("{} All tests passed!", "✓".green().bold());
                println!("{}", "═════════════════════════════════════".bright_cyan());
            }
            Ok(())
        },
        Ok(false) => {
            println!("{} Some tests failed", "✗".red().bold());
            if global_flags.should_output() {
                println!("{}", "═════════════════════════════════════".bright_cyan());
                println!("\n{}", "Troubleshooting:".bright_white().bold());
                println!("  - Review scenario files in {}", scenarios_dir.display());
                println!("  - Check server logs for errors");
                println!("  - Run with --detailed for more output");
            }
            anyhow::bail!("Tests failed");
        },
        Err(e) => Err(e),
    }
}
