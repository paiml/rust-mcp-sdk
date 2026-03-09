//! MCP Apps project management commands.
//!
//! Provides `cargo pmcp app new <name>` for scaffolding,
//! `cargo pmcp app manifest --url <URL>` for generating ChatGPT-compatible
//! manifest JSON, `cargo pmcp app landing` for generating a standalone demo
//! page, and `cargo pmcp app build --url <URL>` for producing both artifacts.

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::publishing;
use crate::templates;

/// MCP Apps project commands.
#[derive(Subcommand)]
pub enum AppCommand {
    /// Create a new MCP Apps project
    New {
        /// Name of the project
        name: String,
        /// Directory to create project in (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },
    /// Generate ChatGPT-compatible manifest JSON
    Manifest {
        /// Server URL (required)
        #[arg(long)]
        url: String,
        /// Logo URL (overrides [package.metadata.pmcp].logo)
        #[arg(long)]
        logo: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },
    /// Generate standalone landing page HTML
    Landing {
        /// Widget to showcase (defaults to first alphabetically)
        #[arg(long)]
        widget: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },
    /// Generate both manifest and landing page
    Build {
        /// Server URL (required for manifest)
        #[arg(long)]
        url: String,
        /// Logo URL
        #[arg(long)]
        logo: Option<String>,
        /// Widget to showcase in landing page (defaults to first)
        #[arg(long)]
        widget: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },
}

impl AppCommand {
    /// Execute the app subcommand.
    pub fn execute(self, global_flags: &crate::commands::GlobalFlags) -> Result<()> {
        let _ = global_flags; // quiet mode conveyed via PMCP_QUIET env var
        match self {
            AppCommand::New { name, path } => create_app(name, path),
            AppCommand::Manifest { url, logo, output } => run_manifest(url, logo, output),
            AppCommand::Landing { widget, output } => create_landing(widget, output),
            AppCommand::Build {
                url,
                logo,
                widget,
                output,
            } => build_all(url, logo, widget, output),
        }
    }
}

/// Scaffold a new MCP Apps project directory.
///
/// Creates a project directory containing `src/main.rs`, `widgets/hello.html`,
/// `Cargo.toml`, and `README.md`. Errors if the target directory already exists,
/// matching `cargo new` semantics.
fn create_app(name: String, path: Option<String>) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    if not_quiet {
        println!("\n{}", "Creating MCP Apps project".bright_cyan().bold());
        println!("{}", "------------------------------------".bright_cyan());
    }

    // Determine project directory
    let project_dir = if let Some(p) = path {
        PathBuf::from(p).join(&name)
    } else {
        PathBuf::from(&name)
    };

    // Error if directory already exists (cargo new semantics)
    if project_dir.exists() {
        anyhow::bail!(
            "directory '{}' already exists. Use a different name or remove the existing directory.",
            project_dir.display()
        );
    }

    // Create directory structure
    fs::create_dir_all(project_dir.join("src")).context("Failed to create src/ directory")?;
    fs::create_dir_all(project_dir.join("widgets"))
        .context("Failed to create widgets/ directory")?;

    if not_quiet {
        println!("  {} Created project structure", "ok".green());
    }

    // Generate all template files
    templates::mcp_app::generate(&project_dir, &name)?;

    if not_quiet {
        println!(
            "\n{} Created MCP Apps project '{}'",
            "ok".green().bold(),
            name
        );

        // Print next steps
        print_next_steps(&name);
    }

    Ok(())
}

/// Generate a ChatGPT-compatible manifest JSON from the current project.
///
/// Detects the MCP Apps project in the current directory, auto-discovers
/// widgets, and writes `manifest.json` to the output directory.
fn run_manifest(url: String, logo: Option<String>, output: String) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    if not_quiet {
        println!("\n{}", "Generating manifest".bright_cyan().bold());
        println!("{}", "------------------------------------".bright_cyan());
    }

    let cwd = std::env::current_dir().context("Failed to read current directory")?;
    let project = publishing::detect::detect_project(&cwd)?;

    let widget_count = project.widgets.len();
    let json = publishing::manifest::generate_manifest(&project, &url, logo.as_deref())?;
    publishing::manifest::write_manifest(&output, &json)?;

    if not_quiet {
        println!("  {} Found {} widget(s)", "ok".green(), widget_count);
        println!(
            "\n{} Manifest written to {}/manifest.json",
            "ok".green().bold(),
            output
        );
    }

    Ok(())
}

/// Generate a standalone landing page from the current project.
///
/// Detects the project, loads mock data, generates a self-contained HTML
/// page with the widget embedded in an iframe using a mock bridge, and
/// writes it to the output directory.
fn create_landing(widget: Option<String>, output: String) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    if not_quiet {
        println!("\n{}", "Generating landing page".bright_cyan().bold());
        println!("{}", "------------------------------------".bright_cyan());
    }

    let cwd = std::env::current_dir().context("Failed to read current directory")?;
    let project = publishing::detect::detect_project(&cwd)?;
    let mock_data = publishing::landing::load_mock_data(&cwd)?;
    let html = publishing::landing::generate_landing(&project, &mock_data, widget.as_deref())?;
    publishing::landing::write_landing(&output, &html)?;

    if not_quiet {
        println!(
            "\n{} Landing page written to {}/landing.html",
            "ok".green().bold(),
            output
        );
    }

    Ok(())
}

/// Generate both manifest JSON and landing page HTML.
///
/// Detects the project once and produces `manifest.json` (for app directory
/// listing) and `landing.html` (for standalone demo) in the output directory.
fn build_all(
    url: String,
    logo: Option<String>,
    widget: Option<String>,
    output: String,
) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();
    if not_quiet {
        println!("\n{}", "Building MCP App".bright_cyan().bold());
        println!("{}", "------------------------------------".bright_cyan());
    }

    let cwd = std::env::current_dir().context("Failed to read current directory")?;
    let project = publishing::detect::detect_project(&cwd)?;

    // Generate manifest
    let manifest_json = publishing::manifest::generate_manifest(&project, &url, logo.as_deref())?;
    publishing::manifest::write_manifest(&output, &manifest_json)?;

    // Generate landing page
    let mock_data = publishing::landing::load_mock_data(&cwd)?;
    let landing_html =
        publishing::landing::generate_landing(&project, &mock_data, widget.as_deref())?;
    publishing::landing::write_landing(&output, &landing_html)?;

    if not_quiet {
        println!(
            "\n{} Built MCP App artifacts in {}/",
            "ok".green().bold(),
            output
        );
        println!("    - manifest.json");
        println!("    - landing.html");
    }

    Ok(())
}

/// Print post-scaffold next-step instructions.
fn print_next_steps(name: &str) {
    println!("\n{}", "  Next steps:".bright_white().bold());
    println!("    {}", format!("cd {}", name).bright_yellow());
    println!("    {}", "cargo build".bright_yellow());
    println!("    {}", "cargo run &".bright_yellow());
    println!(
        "    {}",
        "cargo pmcp preview --url http://localhost:3000 --open".bright_yellow()
    );
    println!();
    println!(
        "  {}",
        "Add widgets by dropping .html files in the widgets/ directory.".dimmed()
    );
    println!(
        "  {}",
        "Preview auto-refreshes -- just reload your browser.".dimmed()
    );
}
