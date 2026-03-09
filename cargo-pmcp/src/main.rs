//! cargo-pmcp: Production-grade MCP server development toolkit
//!
//! This tool provides a batteries-included experience for building MCP servers in Rust,
//! based on proven patterns from 6 production servers.
#![allow(
    clippy::needless_borrows_for_generic_args,
    clippy::ptr_arg,
    clippy::double_ended_iterator_last,
    clippy::useless_format,
    clippy::deref_addrof,
    clippy::uninlined_format_args,
    clippy::too_many_arguments,
    clippy::collapsible_else_if,
    clippy::redundant_static_lifetimes,
    clippy::to_string_in_format_args,
    clippy::module_inception,
    clippy::print_literal,
    clippy::needless_borrow
)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::IsTerminal;

mod commands;
mod deployment;
mod landing;
mod publishing;
mod secrets;
mod templates;
mod utils;

use commands::GlobalFlags;

/// Production-grade MCP server development toolkit
#[derive(Parser)]
#[command(name = "cargo-pmcp")]
#[command(bin_name = "cargo pmcp")]
#[command(about = "Build production-ready MCP servers in Rust", long_about = None)]
#[command(version)]
struct Cli {
    /// Enable verbose output for debugging
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Suppress colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Suppress all non-error output
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new MCP workspace
    ///
    /// This creates a workspace with server-common template and scaffolding
    /// for building multiple MCP servers. The workspace pattern allows sharing
    /// common code (like HTTP bootstrap) across all servers.
    New {
        /// Name of the workspace to create
        name: String,

        /// Directory to create workspace in (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },

    /// Add a component to the workspace
    ///
    /// Supports adding servers, tools, workflows, and resources to existing servers.
    Add {
        #[command(subcommand)]
        component: AddCommands,
    },

    /// Test MCP servers with mcp-tester
    ///
    /// Run tests locally, generate scenarios, or manage scenarios on pmcp.run
    Test {
        #[command(subcommand)]
        command: commands::test::TestCommand,
    },

    /// Start development server
    ///
    /// Builds and runs the server with live logs
    Dev {
        /// Name of the server to run
        #[arg(long)]
        server: String,

        /// Port to run the server on
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Automatically connect to MCP client (claude-code, cursor, inspector)
        #[arg(long)]
        connect: Option<String>,
    },

    /// Connect server to an MCP client
    ///
    /// Helps configure connection to Claude Code, Cursor, or MCP Inspector
    Connect {
        /// Name of the server
        #[arg(long)]
        server: String,

        /// MCP client to connect to (claude-code, cursor, inspector)
        #[arg(long)]
        client: String,

        /// Server URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },

    /// Deploy MCP server to cloud platforms
    ///
    /// Deploy to AWS Lambda, Azure Container Apps, Google Cloud Run, etc.
    Deploy(commands::deploy::DeployCommand),

    /// Manage landing pages for MCP servers
    ///
    /// Create, develop, and deploy landing pages that showcase your MCP server
    Landing {
        #[command(subcommand)]
        command: commands::landing::LandingCommand,
    },

    /// Export schema from foundation MCP servers
    ///
    /// Connect to a foundation server and generate typed Rust client code
    /// for calling its tools. Supports both MCP HTTP and Lambda invocation.
    Schema {
        #[command(subcommand)]
        command: commands::schema::SchemaCommand,
    },

    /// Validate MCP server components
    ///
    /// Run validation checks on workflows, tools, and other server components.
    /// Helps catch structural errors before runtime.
    Validate {
        #[command(subcommand)]
        command: commands::validate::ValidateCommand,
    },

    /// Manage secrets for MCP servers
    ///
    /// Store and retrieve secrets across multiple providers (local, pmcp.run, AWS).
    /// Secrets are namespaced by server ID to avoid conflicts.
    Secret(commands::secret::SecretCommand),

    /// Run load tests against MCP servers
    ///
    /// Execute load tests with configurable virtual users, scenarios, and reports.
    Loadtest {
        #[command(subcommand)]
        command: commands::loadtest::LoadtestCommand,
    },

    /// MCP Apps project management
    ///
    /// Scaffold and manage MCP Apps projects with interactive widgets.
    App {
        #[command(subcommand)]
        command: commands::app::AppCommand,
    },

    /// Preview MCP Apps widgets in browser
    ///
    /// Launch a browser-based preview environment for testing MCP servers
    /// that return widget UI. Simulates the ChatGPT Apps runtime.
    Preview {
        /// URL of the running MCP server
        #[arg(long)]
        url: String,

        /// Port for the preview server
        #[arg(long, default_value = "8765")]
        port: u16,

        /// Open browser automatically
        #[arg(long)]
        open: bool,

        /// Auto-select this tool on start
        #[arg(long)]
        tool: Option<String>,

        /// Initial theme (light/dark)
        #[arg(long, default_value = "light")]
        theme: String,

        /// Initial locale
        #[arg(long, default_value = "en-US")]
        locale: String,

        /// Path to widgets directory for file-based authoring (hot-reload)
        ///
        /// When set, widget HTML files are read directly from this directory
        /// on each request. Browser refresh shows the latest HTML without
        /// server restart.
        #[arg(long)]
        widgets_dir: Option<String>,

        /// Preview mode: standard (default) or chatgpt (strict ChatGPT protocol validation)
        #[arg(long, default_value = "standard")]
        mode: String,
    },
}

#[derive(Subcommand)]
enum AddCommands {
    /// Add a new MCP server to the workspace
    Server {
        /// Name of the server (will create mcp-{name}-core and {name}-server)
        name: String,

        /// Server template to use
        #[arg(long, default_value = "minimal")]
        template: String,

        /// Port to assign to this server (auto-increments if not specified)
        #[arg(long)]
        port: Option<u16>,

        /// Replace existing server with same name (requires confirmation)
        #[arg(long)]
        replace: bool,
    },

    /// Add a tool to an existing server
    Tool {
        /// Name of the tool
        name: String,

        /// Server to add the tool to
        #[arg(long)]
        server: String,
    },

    /// Add a workflow to an existing server
    Workflow {
        /// Name of the workflow
        name: String,

        /// Server to add the workflow to
        #[arg(long)]
        server: String,
    },
}

fn main() -> Result<()> {
    // Handle cargo subcommand invocation
    // When called as `cargo pmcp`, cargo passes "pmcp" as the first argument
    let mut args = std::env::args();
    let cli = if args.nth(1).as_deref() == Some("pmcp") {
        // Skip the "pmcp" argument when invoked as cargo subcommand
        let args_vec: Vec<String> = std::env::args()
            .enumerate()
            .filter_map(|(i, arg)| if i != 1 { Some(arg) } else { None })
            .collect();
        Cli::parse_from(args_vec)
    } else {
        // Normal invocation as cargo-pmcp
        Cli::parse()
    };

    // Set verbose mode as environment variable for global access
    if cli.verbose {
        std::env::set_var("PMCP_VERBOSE", "1");
    }

    // Determine effective no_color: explicit flag, NO_COLOR env (no-color.org), or non-TTY
    let effective_no_color =
        cli.no_color || std::env::var("NO_COLOR").is_ok() || !std::io::stdout().is_terminal();

    if effective_no_color {
        // Suppress colored crate output globally
        colored::control::set_override(false);
        // Suppress console crate output globally
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }

    // Verbose wins over quiet (per user decision):
    // If both --verbose and --quiet are passed, quiet is disabled.
    let effective_quiet = cli.quiet && !cli.verbose;

    // Set global flag env vars for subprocess consumption
    if effective_no_color {
        std::env::set_var("PMCP_NO_COLOR", "1");
    }
    if effective_quiet {
        std::env::set_var("PMCP_QUIET", "1");
    }

    let global_flags = GlobalFlags {
        verbose: cli.verbose,
        no_color: effective_no_color,
        quiet: effective_quiet,
    };

    execute_command(cli.command, &global_flags)?;

    Ok(())
}

fn execute_command(command: Commands, global_flags: &GlobalFlags) -> Result<()> {
    match command {
        Commands::New { name, path } => {
            commands::new::execute(name, path, None, global_flags)?;
        },
        Commands::Add { component } => match component {
            AddCommands::Server {
                name,
                template,
                port,
                replace,
            } => {
                commands::add::server(name, template, port, replace, global_flags)?;
            },
            AddCommands::Tool { name, server } => {
                commands::add::tool(name, server, global_flags)?;
            },
            AddCommands::Workflow { name, server } => {
                commands::add::workflow(name, server, global_flags)?;
            },
        },
        Commands::Test { command } => {
            command.execute(global_flags)?;
        },
        Commands::Dev {
            server,
            port,
            connect,
        } => {
            commands::dev::execute(server, port, connect, global_flags)?;
        },
        Commands::Connect {
            server,
            client,
            url,
        } => {
            commands::connect::execute(server, client, url, global_flags)?;
        },
        Commands::Deploy(deploy_cmd) => {
            deploy_cmd.execute(global_flags)?;
        },
        Commands::Landing { command } => {
            let runtime = tokio::runtime::Runtime::new()?;
            let project_root = std::env::current_dir()?;
            runtime.block_on(command.execute(project_root, global_flags))?;
        },
        Commands::Schema { command } => {
            command.execute(global_flags)?;
        },
        Commands::Validate { command } => {
            command.execute(global_flags)?;
        },
        Commands::Secret(secret_cmd) => {
            secret_cmd.execute(global_flags)?;
        },
        Commands::Loadtest { command } => {
            command.execute(global_flags)?;
        },
        Commands::App { command } => {
            command.execute(global_flags)?;
        },
        Commands::Preview {
            url,
            port,
            open,
            tool,
            theme,
            locale,
            widgets_dir,
            mode,
        } => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(commands::preview::execute(
                url,
                port,
                open,
                tool,
                theme,
                locale,
                widgets_dir,
                mode,
                global_flags,
            ))?;
        },
    }
    Ok(())
}
