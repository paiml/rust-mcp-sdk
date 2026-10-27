//! PMCP Server binary - MCP developer tools over streamable HTTP.

use clap::Parser;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "pmcp-server",
    about = "PMCP SDK developer tools MCP server",
    long_about = "An MCP server providing protocol testing, scaffolding, schema export, \
                  documentation, and guided workflow prompts for PMCP SDK development. \
                  Served over streamable HTTP transport."
)]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value = "8080", env = "PMCP_SERVER_PORT")]
    port: u16,

    /// Bind address
    #[arg(long, default_value = "0.0.0.0", env = "PMCP_SERVER_HOST")]
    host: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pmcp_server=info,pmcp=warn".into()),
        )
        .init();

    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let server = pmcp_server::build_server()?;
    let http_server = StreamableHttpServer::new_concurrent(addr, Arc::new(server));
    let (bound_addr, handle) = http_server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start HTTP server: {}", e))?;

    tracing::info!("PMCP server listening on http://{}", bound_addr);
    eprintln!("PMCP server listening on http://{}", bound_addr);

    handle
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
    Ok(())
}
