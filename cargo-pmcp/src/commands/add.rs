//! Add components to workspace

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::templates;
use crate::utils::config::WorkspaceConfig;

pub fn server(
    name: String,
    template: String,
    port: Option<u16>,
    replace: bool,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let not_quiet = global_flags.should_output();
    if not_quiet {
        println!("\n{}", "Adding MCP server".bright_cyan().bold());
        println!("{}", "─────────────────────────".bright_cyan());
    }

    // Verify we're in a workspace
    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo-pmcp new <name>' first.");
    }

    // Load workspace config
    let mut config = WorkspaceConfig::load()?;

    // Check if server already exists
    if config.has_server(&name) && !replace {
        anyhow::bail!(
            "Server '{}' already exists. Use --replace to upgrade it.",
            name
        );
    }

    // Handle replacement
    if replace && config.has_server(&name) {
        let existing = config.get_server(&name).unwrap();
        if not_quiet {
            println!(
                "\n{} Server '{}' already exists:",
                "Warning:".yellow().bold(),
                name.bright_yellow()
            );
            println!("  Current template: {}", existing.template.bright_cyan());
            println!(
                "  Current port:     {}",
                existing.port.to_string().bright_cyan()
            );
            println!("  New template:     {}", template.bright_cyan());
        }

        print!(
            "\n{} This will delete the existing server crates. Continue? [y/N]: ",
            "Warning:".yellow().bold()
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{} Cancelled", "x".red());
            return Ok(());
        }

        // Delete existing crate directories
        let core_dir = PathBuf::from(format!("crates/mcp-{}-core", name));
        let server_dir = PathBuf::from(format!("crates/{}-server", name));

        if core_dir.exists() {
            fs::remove_dir_all(&core_dir).context("Failed to remove old core crate")?;
            if not_quiet {
                println!("  {} Removed {}", "ok".green(), core_dir.display());
            }
        }

        if server_dir.exists() {
            fs::remove_dir_all(&server_dir).context("Failed to remove old server crate")?;
            if not_quiet {
                println!("  {} Removed {}", "ok".green(), server_dir.display());
            }
        }

        if not_quiet {
            println!();
        }
    }

    // Determine port
    let assigned_port = if let Some(p) = port {
        // User specified port - check if it's available (unless replacing)
        if config.is_port_used(p)
            && !(replace && config.get_server(&name).map(|s| s.port) == Some(p))
        {
            anyhow::bail!("Port {} is already in use by another server", p);
        }
        p
    } else if replace && config.has_server(&name) {
        // Keep existing port when replacing
        config.get_server(&name).unwrap().port
    } else {
        // Auto-assign next available port
        config.next_available_port()
    };

    // Generate server crates
    templates::server::generate(&name, &template)?;

    // Update config
    config.add_server(name.clone(), assigned_port, template.clone());
    config.save().context("Failed to save workspace config")?;

    if not_quiet {
        println!(
            "  {} Assigned port {}",
            "ok".green(),
            assigned_port.to_string().bright_yellow()
        );

        println!(
            "\n{} Server '{}' added successfully!",
            "ok".green().bold(),
            name.bright_yellow()
        );

        // Show template-specific information
        if template == "complete" {
            println!("\n{}", "Complete Template Includes:".bright_white().bold());
            println!();
            println!("  Tools (5):");
            println!("    - add, subtract, multiply    - Basic arithmetic");
            println!("    - divide                     - With zero-division check");
            println!("    - power                      - Exponentiation");
            println!();
            println!("  Prompts (1):");
            println!("    - quadratic                  - Solve quadratic equations");
            println!("                                   Shows how prompts compose tools");
            println!();
            println!("  Resources (1):");
            println!("    - quadratic-formula          - Educational guide");
            println!("                                   Explains the mathematical theory");
            println!();
            println!("  What You'll Learn:");
            println!("    - Tool patterns    - See how similar tools follow the same structure");
            println!("    - Error handling   - Division by zero validation");
            println!("    - Composition      - How prompts orchestrate multiple tools");
            println!("    - Resources        - Providing static knowledge/documentation");
            println!("    - Workflows        - Multi-step mathematical operations");
            println!();
        } else if template == "sqlite-explorer" {
            println!(
                "\n{}",
                "SQLite Explorer Template Includes:".bright_white().bold()
            );
            println!();
            println!("  Tools (3):");
            println!("    - execute_query      - Run SELECT queries (read-only, validated)");
            println!("    - list_tables        - Show all tables with row counts");
            println!("    - get_sample_rows    - Preview table data");
            println!();
            println!("  Workflow Prompts (3):");
            println!("    - monthly_sales_report              - Sales metrics for a month");
            println!("    - analyze_customer                  - Customer purchase history & LTV");
            println!("    - customers_who_bought_top_tracks   - Multi-step workflow");
            println!("                                          (demonstrates step bindings!)");
            println!();
            println!("  Resources (2):");
            println!("    - sqlite://schema                   - Complete database schema");
            println!("    - sqlite://table/{{name}}/schema      - Per-table schema details");
            println!();
            println!("  What You'll Learn:");
            println!("    - Workflow prompts  - Multi-step orchestration with bindings");
            println!("    - SQL safety        - Prepared statements, read-only validation");
            println!("    - Schema discovery  - Resources for context-aware queries");
            println!("    - Step composition  - Output from step 1 -> input to step 2");
            println!("    - Real database     - Chinook sample DB (music store)");
            println!();
            println!("  Database Setup:");
            println!("    See DATABASE.md for chinook.db download instructions");
            println!();
        }

        println!("{}", "Quick Start (2 minutes):".bright_white().bold());
        println!();
        println!("  {} Start your server:", "1.".bright_cyan().bold());
        println!(
            "     {}",
            format!("cargo pmcp dev --server {}", name).bright_yellow()
        );
        println!();
        println!("  {} Connect to a client:", "2.".bright_cyan().bold());
        println!(
            "     Claude Code:  {}",
            format!("cargo pmcp connect --server {} --client claude-code", name).bright_yellow()
        );
        println!(
            "     Inspector:    {}",
            format!("cargo pmcp connect --server {} --client inspector", name).bright_yellow()
        );
        println!(
            "     Cursor:       {}",
            format!("cargo pmcp connect --server {} --client cursor", name).bright_yellow()
        );
        println!();
        println!("  {} Try it out:", "3.".bright_cyan().bold());
        if template == "complete" {
            println!("     In Claude Code:");
            println!("       {}", "\"Multiply 7 and 8\"".bright_green());
            println!("       {}", "\"What's 100 divided by 5?\"".bright_green());
            println!(
                "       {}",
                "\"Solve the quadratic equation: x^2 - 5x + 6 = 0\"".bright_green()
            );
            println!(
                "       {}",
                "\"Show me the quadratic formula guide\"".bright_green()
            );
        } else if template == "sqlite-explorer" {
            println!("     In Claude Code (using /prompts):");
            println!(
                "       {}",
                "\"/monthly_sales_report month: 3 year: 2009\"".bright_green()
            );
            println!(
                "       {}",
                "\"/analyze_customer customer_id: 5\"".bright_green()
            );
            println!(
                "       {}",
                "\"/customers_who_bought_top_tracks limit: 10\"".bright_green()
            );
            println!();
            println!("     Or ask Claude:");
            println!(
                "       {}",
                "\"Show me all tables in the database\"".bright_green()
            );
            println!(
                "       {}",
                "\"Get sample rows from the customers table\"".bright_green()
            );
        } else {
            println!("     In Claude Code: {}", "\"Add 5 and 3\"".bright_green());
        }
        println!();

        println!("{}", "Additional Commands:".bright_white().bold());
        println!(
            "  - Generate tests: {}",
            format!("cargo pmcp test --server {} --generate-scenarios", name).bright_cyan()
        );
        println!(
            "  - Run tests:      {}",
            format!("cargo pmcp test --server {}", name).bright_cyan()
        );
        println!(
            "  - Unit tests:     {}",
            format!("cargo test -p mcp-{}-core", name).bright_cyan()
        );
    }

    Ok(())
}

pub fn tool(
    name: String,
    server: String,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Adding tool".bright_cyan().bold());
        println!("{}", "────────────────".bright_cyan());

        // TODO: Implement tool scaffolding
        println!(
            "  {} Tool '{}' scaffolding for server '{}'",
            "ok".green(),
            name,
            server
        );
        println!("\n{} Coming in next phase", "Warning:".yellow().bold());
    }

    Ok(())
}

pub fn workflow(
    name: String,
    server: String,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Adding workflow".bright_cyan().bold());
        println!("{}", "──────────────────────".bright_cyan());

        // TODO: Implement workflow scaffolding
        println!(
            "  {} Workflow '{}' scaffolding for server '{}'",
            "ok".green(),
            name,
            server
        );
        println!("\n{} Coming in next phase", "Warning:".yellow().bold());
    }

    Ok(())
}
