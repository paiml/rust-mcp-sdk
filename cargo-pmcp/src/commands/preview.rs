//! Preview command - MCP Apps UI testing environment

use anyhow::Result;
use colored::Colorize;

/// Start the MCP Apps preview server
pub async fn execute(
    url: String,
    port: u16,
    open: bool,
    tool: Option<String>,
    theme: String,
    locale: String,
    widgets_dir: Option<String>,
    mode: String,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let preview_mode = if mode == "chatgpt" {
        mcp_preview::PreviewMode::ChatGpt
    } else {
        mcp_preview::PreviewMode::Standard
    };

    if global_flags.should_output() {
        println!("\n{}", "Starting MCP Apps Preview".bright_cyan().bold());
        println!("{}", "─────────────────────────────────".bright_cyan());
        println!("  {} MCP Server: {}", "→".blue(), url.bright_yellow());
        println!(
            "  {} Preview URL: {}",
            "→".blue(),
            format!("http://localhost:{}", port).bright_green()
        );
        if let Some(ref dir) = widgets_dir {
            println!(
                "  {} Widgets Dir: {} (hot-reload)",
                "→".blue(),
                dir.bright_magenta()
            );
        }
        let mode_display = match preview_mode {
            mcp_preview::PreviewMode::ChatGpt => "ChatGPT Strict".bright_red().bold(),
            mcp_preview::PreviewMode::Standard => "Standard".bright_green().bold(),
        };
        println!("  {} Mode:        {}", "→".blue(), mode_display);
        println!();
    }

    let widgets_path = widgets_dir.map(std::path::PathBuf::from);

    let config = mcp_preview::PreviewConfig {
        mcp_url: url,
        port,
        initial_tool: tool,
        theme,
        locale,
        widgets_dir: widgets_path,
        mode: preview_mode,
    };

    // Open browser if requested
    if open {
        let preview_url = format!("http://localhost:{}", port);
        tokio::spawn(async move {
            // Wait a bit for the server to start
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            if let Err(e) = open::that(&preview_url) {
                eprintln!("Failed to open browser: {}", e);
            }
        });
    }

    mcp_preview::PreviewServer::start(config).await
}
