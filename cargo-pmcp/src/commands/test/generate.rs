//! Generate test scenarios from MCP server capabilities

use anyhow::Result;
use colored::Colorize;
use mcp_tester::{generate_scenarios_with_transport, GenerateOptions};
use std::path::PathBuf;

use crate::commands::GlobalFlags;

/// Generate test scenarios from server capabilities
pub fn execute(
    server: Option<String>,
    url: Option<String>,
    port: u16,
    output: Option<PathBuf>,
    transport: Option<String>,
    all_tools: bool,
    with_resources: bool,
    with_prompts: bool,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Generating test scenarios".bright_cyan().bold());
        println!("{}", "─────────────────────────────────────".bright_cyan());
    }

    // Determine target URL
    let target_url = if let Some(url) = url {
        url
    } else if server.is_some() {
        format!("http://0.0.0.0:{}", port)
    } else {
        anyhow::bail!("Either --url or --server must be specified");
    };

    // Determine output path
    let output_path = if let Some(path) = output {
        path
    } else if let Some(ref server) = server {
        PathBuf::from("scenarios")
            .join(server)
            .join("generated.yaml")
    } else {
        PathBuf::from("scenarios").join("generated.yaml")
    };

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if global_flags.should_output() {
        println!("  {} Connecting to server at {}...", "→".blue(), target_url);
    }

    let options = GenerateOptions {
        all_tools,
        with_resources,
        with_prompts,
    };

    // Use tokio runtime for async generation
    let generation_result = tokio::runtime::Runtime::new()?.block_on(async {
        generate_scenarios_with_transport(
            &target_url,
            output_path.to_str().unwrap(),
            options,
            transport.as_deref(),
        )
        .await
    });

    match generation_result {
        Ok(_) => {
            if global_flags.should_output() {
                println!(
                    "  {} Scenarios generated at {}",
                    "✓".green(),
                    output_path.display()
                );
                println!();
                println!("{}", "Next steps:".bright_white().bold());
                println!(
                    "  1. Edit {} to customize test values",
                    output_path.display()
                );
                println!("  2. Add assertions to validate responses");
                println!("  3. Run tests with: cargo pmcp test run --server <name>");
                println!();
                println!("{}", "Tip:".bright_cyan().bold());
                println!("  Upload scenarios to pmcp.run for scheduled testing:");
                println!(
                    "    cargo pmcp test upload --server <id> {}",
                    output_path.display()
                );
            }
            Ok(())
        },
        Err(e) => {
            if let Some(ref server) = server {
                anyhow::bail!(
                    "Failed to generate scenarios: {}\n\n  Make sure the server is running:\n  {}",
                    e,
                    format!("cargo pmcp dev --server {}", server).bright_cyan()
                );
            } else {
                anyhow::bail!(
                    "Failed to generate scenarios: {}\n\n  Make sure the server is running at {}",
                    e,
                    target_url
                );
            }
        },
    }
}
