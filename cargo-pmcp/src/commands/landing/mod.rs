//! Landing page commands for MCP servers
//!
//! This module provides commands to create, develop, and deploy landing pages
//! for MCP servers. Landing pages help users discover and install MCP servers.

pub mod deploy;
pub mod dev;
pub mod init;

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum LandingCommand {
    /// Initialize a new landing page for your MCP server
    Init {
        /// Template to use
        #[arg(long, default_value = "nextjs")]
        template: String,

        /// Output directory for landing page
        #[arg(long, default_value = "./landing")]
        output: PathBuf,

        /// MCP server name (reads from pmcp.toml if not provided)
        #[arg(long)]
        server_name: Option<String>,
    },

    /// Run the landing page locally for development
    Dev {
        /// Landing page directory
        #[arg(long, default_value = "./landing")]
        dir: PathBuf,

        /// Port to run on
        #[arg(long, default_value = "3001")]
        port: u16,

        /// Watch for changes in pmcp-landing.toml
        #[arg(long)]
        watch: bool,
    },

    /// Build the landing page for production
    Build {
        /// Landing page directory
        #[arg(long, default_value = "./landing")]
        dir: PathBuf,

        /// Build output directory
        #[arg(long, default_value = "./landing/.next")]
        output: PathBuf,
    },

    /// Deploy the landing page to a target
    Deploy {
        /// Landing page directory
        #[arg(long, default_value = "./landing")]
        dir: PathBuf,

        /// Deployment target
        #[arg(long, default_value = "pmcp-run")]
        target: String,

        /// MCP server ID to link to (optional - auto-detected from deployment)
        #[arg(long)]
        server_id: Option<String>,
    },
}

impl LandingCommand {
    pub async fn execute(self, project_root: PathBuf) -> Result<()> {
        match self {
            LandingCommand::Init {
                template,
                output,
                server_name,
            } => init::init_landing_page(project_root, template, output, server_name).await,

            LandingCommand::Dev { dir, port, watch } => {
                dev::run_dev_server(project_root, dir, port, watch).await
            },

            LandingCommand::Build { dir, output: _ } => {
                // TODO: Implement in P1
                println!("🚧 Build command coming in Phase 1!");
                println!("   For now, use: cd {} && npm run build", dir.display());
                Ok(())
            },

            LandingCommand::Deploy {
                dir,
                target,
                server_id,
            } => deploy::deploy_landing_page(project_root, dir, target, server_id).await,
        }
    }
}
