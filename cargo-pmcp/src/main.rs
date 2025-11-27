//! cargo-pmcp: Production-grade MCP server development toolkit
//!
//! This tool provides a batteries-included experience for building MCP servers in Rust,
//! based on proven patterns from 6 production servers.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod deployment;
mod landing;
mod templates;
mod utils;

/// Production-grade MCP server development toolkit
#[derive(Parser)]
#[command(name = "cargo-pmcp")]
#[command(bin_name = "cargo pmcp")]
#[command(about = "Build production-ready MCP servers in Rust", long_about = None)]
#[command(version)]
struct Cli {
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

    /// Test a server with mcp-tester
    ///
    /// Builds the server, starts it, and runs mcp-tester scenarios
    Test {
        /// Name of the server to test
        #[arg(long)]
        server: String,

        /// Port to run the server on
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Generate scenarios before testing
        #[arg(long)]
        generate_scenarios: bool,

        /// Show detailed test output
        #[arg(long)]
        detailed: bool,
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
        #[arg(long, default_value = "http://0.0.0.0:3000")]
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
    if args.nth(1).as_deref() == Some("pmcp") {
        // Skip the "pmcp" argument when invoked as cargo subcommand
        let args_vec: Vec<String> = std::env::args()
            .enumerate()
            .filter_map(|(i, arg)| if i != 1 { Some(arg) } else { None })
            .collect();
        let cli = Cli::parse_from(args_vec);
        execute_command(cli.command)?;
    } else {
        // Normal invocation as cargo-pmcp
        let cli = Cli::parse();
        execute_command(cli.command)?;
    }

    Ok(())
}

fn execute_command(command: Commands) -> Result<()> {
    match command {
        Commands::New { name, path } => {
            commands::new::execute(name, path)?;
        },
        Commands::Add { component } => match component {
            AddCommands::Server {
                name,
                template,
                port,
                replace,
            } => {
                commands::add::server(name, template, port, replace)?;
            },
            AddCommands::Tool { name, server } => {
                commands::add::tool(name, server)?;
            },
            AddCommands::Workflow { name, server } => {
                commands::add::workflow(name, server)?;
            },
        },
        Commands::Test {
            server,
            port,
            generate_scenarios,
            detailed,
        } => {
            commands::test::execute(server, port, generate_scenarios, detailed)?;
        },
        Commands::Dev {
            server,
            port,
            connect,
        } => {
            commands::dev::execute(server, port, connect)?;
        },
        Commands::Connect {
            server,
            client,
            url,
        } => {
            commands::connect::execute(server, client, url)?;
        },
        Commands::Deploy(deploy_cmd) => {
            deploy_cmd.execute()?;
        },
        Commands::Landing { command } => {
            let runtime = tokio::runtime::Runtime::new()?;
            let project_root = std::env::current_dir()?;
            runtime.block_on(command.execute(project_root))?;
        },
    }
    Ok(())
}
