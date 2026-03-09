//! Create new MCP workspace

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::templates;

/// Server tier for composition architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerTier {
    /// Foundation servers: Core data connectors (databases, CRMs, APIs)
    Foundation,
    /// Domain servers: Orchestration that composes foundation servers
    Domain,
}

impl ServerTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "foundation" => Some(Self::Foundation),
            "domain" => Some(Self::Domain),
            _ => None,
        }
    }
}

pub fn execute(
    name: String,
    path: Option<String>,
    tier: Option<String>,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let not_quiet = global_flags.should_output();
    let tier = tier.as_deref().and_then(ServerTier::from_str);

    let tier_label = match tier {
        Some(ServerTier::Foundation) => " (foundation)",
        Some(ServerTier::Domain) => " (domain)",
        None => "",
    };

    if not_quiet {
        println!(
            "\n{}",
            format!("Creating MCP workspace{}", tier_label)
                .bright_cyan()
                .bold()
        );
        println!("{}", "────────────────────────────────────".bright_cyan());
    }

    // Determine workspace directory
    let workspace_dir = if let Some(p) = path {
        PathBuf::from(p).join(&name)
    } else {
        PathBuf::from(&name)
    };

    // Check if directory already exists
    if workspace_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", workspace_dir.display());
    }

    // Create workspace structure
    create_workspace_structure(&workspace_dir, &name, tier)?;

    // Generate workspace files
    templates::workspace::generate(&workspace_dir, &name)?;

    // Generate server-common crate
    templates::server_common::generate(&workspace_dir)?;

    // For domain servers, add composition client setup
    if tier == Some(ServerTier::Domain) {
        create_domain_composition_structure(&workspace_dir, &name)?;
    }

    if not_quiet {
        println!("\n{} Workspace created successfully!", "✓".green().bold());

        // Print tier-specific next steps
        match tier {
            Some(ServerTier::Domain) => print_domain_next_steps(&name),
            Some(ServerTier::Foundation) => print_foundation_next_steps(&name),
            None => print_default_next_steps(&name),
        }
    }

    Ok(())
}

fn create_workspace_structure(
    workspace_dir: &Path,
    _name: &str,
    tier: Option<ServerTier>,
) -> Result<()> {
    // Create main directories
    fs::create_dir_all(workspace_dir).context("Failed to create workspace directory")?;

    fs::create_dir_all(workspace_dir.join("crates"))
        .context("Failed to create crates directory")?;

    fs::create_dir_all(workspace_dir.join("scenarios"))
        .context("Failed to create scenarios directory")?;

    fs::create_dir_all(workspace_dir.join("lambda"))
        .context("Failed to create lambda directory")?;

    // For domain servers, create schemas directory for foundation server schemas
    if tier == Some(ServerTier::Domain) {
        fs::create_dir_all(workspace_dir.join("schemas"))
            .context("Failed to create schemas directory")?;
    }

    if std::env::var("PMCP_QUIET").is_err() {
        println!("  {} Generated workspace structure", "✓".green());
    }

    Ok(())
}

/// Create additional structure for domain servers that will compose foundation servers
fn create_domain_composition_structure(workspace_dir: &Path, name: &str) -> Result<()> {
    // Create src/foundations directory for generated clients
    let foundations_dir = workspace_dir
        .join("crates")
        .join(format!("mcp-{}-core", name))
        .join("src")
        .join("foundations");
    fs::create_dir_all(&foundations_dir).context("Failed to create foundations directory")?;

    // Create mod.rs for foundations
    let mod_content = r#"//! Generated typed clients for foundation MCP servers
//!
//! This module contains auto-generated clients for foundation servers
//! that this domain server composes.
//!
//! To add a foundation server client:
//! 1. Export the schema: cargo pmcp schema export --server foundation-server-id
//! 2. Generate client: cargo pmcp generate client --schema schemas/foundation.json --output src/foundations/

// Example: pub mod calculator;
"#;

    fs::write(foundations_dir.join("mod.rs"), mod_content)
        .context("Failed to create foundations/mod.rs")?;

    if std::env::var("PMCP_QUIET").is_err() {
        println!(
            "  {} Created composition structure for domain server",
            "✓".green()
        );
    }

    Ok(())
}

fn print_default_next_steps(name: &str) {
    println!(
        "\n{}",
        "🚀 Next Steps (2-minute quick start):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!("  {} Add your first server:", "2.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp add server calculator".bright_yellow()
    );
    println!();
    println!("  {} Start the server:", "3.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp dev --server calculator".bright_yellow()
    );
    println!();
    println!("  {} Connect to Claude Code:", "4.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp connect --server calculator --client claude-code".bright_yellow()
    );
    println!();
    println!("  {} Try it:", "5.".bright_cyan().bold());
    println!("     Ask Claude: {}", "\"Add 5 and 3\"".bright_green());
}

fn print_foundation_next_steps(name: &str) {
    println!(
        "\n{}",
        "🔧 Foundation Server - Next Steps:".bright_white().bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Add tools for your data source:",
        "2.".bright_cyan().bold()
    );
    println!(
        "     {}",
        "cargo pmcp add server my-connector".bright_yellow()
    );
    println!();
    println!(
        "  {} Add output schemas for type-safe composition:",
        "3.".bright_cyan().bold()
    );
    println!(
        "     Use {} to define output types",
        "TypedToolWithOutput<Input, Output>".bright_green()
    );
    println!();
    println!("  {} Deploy to pmcp.run:", "4.".bright_cyan().bold());
    println!("     {}", "cargo pmcp deploy".bright_yellow());
}

fn print_domain_next_steps(name: &str) {
    println!(
        "\n{}",
        "🏗️  Domain Server - Next Steps:".bright_white().bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Export foundation server schemas:",
        "2.".bright_cyan().bold()
    );
    println!(
        "     {}",
        "cargo pmcp schema export --server calculator --output schemas/".bright_yellow()
    );
    println!();
    println!("  {} Generate typed clients:", "3.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp generate client --schema schemas/calculator.json".bright_yellow()
    );
    println!();
    println!("  {} Build domain capabilities:", "4.".bright_cyan().bold());
    println!(
        "     Use generated clients like {}",
        "calculator.add(1.0, 2.0).await".bright_green()
    );
    println!();
    println!("  {} Deploy to pmcp.run:", "5.".bright_cyan().bold());
    println!("     {}", "cargo pmcp deploy".bright_yellow());
}
