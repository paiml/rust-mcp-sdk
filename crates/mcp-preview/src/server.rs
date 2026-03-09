//! Preview server implementation

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::handlers;
use crate::proxy::McpProxy;
use crate::wasm_builder::{find_workspace_root, WasmBuilder};

/// Preview mode controlling protocol validation strictness
#[derive(Debug, Clone, Default, PartialEq)]
pub enum PreviewMode {
    /// Standard MCP preview (default)
    #[default]
    Standard,
    /// ChatGPT strict protocol validation
    ChatGpt,
}

impl std::fmt::Display for PreviewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::ChatGpt => write!(f, "chatgpt"),
        }
    }
}

/// Configuration for the preview server
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// URL of the target MCP server
    pub mcp_url: String,
    /// Port for the preview server
    pub port: u16,
    /// Initial tool to select
    pub initial_tool: Option<String>,
    /// Initial theme (light/dark)
    pub theme: String,
    /// Initial locale
    pub locale: String,
    /// Optional directory containing widget `.html` files for file-based authoring.
    ///
    /// When set, the preview server reads widget HTML directly from disk on each
    /// request (hot-reload without file watchers). Widgets are discovered by
    /// scanning this directory for `.html` files and mapping each to a
    /// `ui://app/{stem}` resource URI.
    pub widgets_dir: Option<PathBuf>,
    /// Preview mode (standard or chatgpt)
    pub mode: PreviewMode,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            mcp_url: "http://localhost:3000".to_string(),
            port: 8765,
            initial_tool: None,
            theme: "light".to_string(),
            locale: "en-US".to_string(),
            widgets_dir: None,
            mode: PreviewMode::default(),
        }
    }
}

/// Shared application state
pub struct AppState {
    pub config: PreviewConfig,
    pub proxy: McpProxy,
    pub wasm_builder: WasmBuilder,
}

/// MCP Preview Server
pub struct PreviewServer;

impl PreviewServer {
    /// Start the preview server
    pub async fn start(config: PreviewConfig) -> Result<()> {
        let proxy = McpProxy::new(&config.mcp_url);

        // Locate the workspace root to find the WASM client source
        let cwd = std::env::current_dir().unwrap_or_default();
        let workspace_root = find_workspace_root(&cwd).unwrap_or_else(|| cwd.clone());
        let wasm_source_dir = workspace_root.join("examples").join("wasm-client");
        let wasm_cache_dir = workspace_root.join("target").join("wasm-bridge");
        let wasm_builder = WasmBuilder::new(wasm_source_dir, wasm_cache_dir);

        let state = Arc::new(AppState {
            config: config.clone(),
            proxy,
            wasm_builder,
        });

        // Build CORS layer
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Build router
        let app = Router::new()
            // Main preview page
            .route("/", get(handlers::page::index))
            // API endpoints - tools
            .route("/api/config", get(handlers::api::get_config))
            .route("/api/tools", get(handlers::api::list_tools))
            .route("/api/tools/call", post(handlers::api::call_tool))
            // API endpoints - resources
            .route("/api/resources", get(handlers::api::list_resources))
            .route("/api/resources/read", get(handlers::api::read_resource))
            // API endpoints - session management
            .route("/api/reconnect", post(handlers::api::reconnect))
            .route("/api/status", get(handlers::api::status))
            // API endpoints - WASM bridge
            .route("/api/wasm/build", post(handlers::wasm::trigger_build))
            .route("/api/wasm/status", get(handlers::wasm::build_status))
            // WASM artifact serving (catch-all for nested snippets/ paths)
            .route("/wasm/{*path}", get(handlers::wasm::serve_artifact))
            // Static assets
            .route("/assets/{*path}", get(handlers::assets::serve))
            // WebSocket for live updates
            .route("/ws", get(handlers::websocket::handler))
            .layer(cors)
            .with_state(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

        println!();
        println!("\x1b[1;36m╔══════════════════════════════════════════════════╗\x1b[0m");
        println!("\x1b[1;36m║          MCP Apps Preview Server                 ║\x1b[0m");
        println!("\x1b[1;36m╠══════════════════════════════════════════════════╣\x1b[0m");
        println!(
            "\x1b[1;36m║\x1b[0m  Preview:    \x1b[1;33mhttp://localhost:{:<5}\x1b[0m             \x1b[1;36m║\x1b[0m",
            config.port
        );
        println!(
            "\x1b[1;36m║\x1b[0m  MCP Server: \x1b[1;32m{:<30}\x1b[0m   \x1b[1;36m║\x1b[0m",
            truncate_url(&config.mcp_url, 30)
        );
        if let Some(ref widgets_dir) = config.widgets_dir {
            println!(
                "\x1b[1;36m║\x1b[0m  Widgets:    \x1b[1;35m{:<30}\x1b[0m   \x1b[1;36m║\x1b[0m",
                truncate_url(&widgets_dir.display().to_string(), 30)
            );
            info!(
                "Widgets directory: {} (hot-reload enabled)",
                widgets_dir.display()
            );
        }
        println!(
            "\x1b[1;36m║\x1b[0m  Mode:       {:<30}   \x1b[1;36m║\x1b[0m",
            match config.mode {
                PreviewMode::ChatGpt => "\x1b[1;31mChatGPT Strict\x1b[0m",
                PreviewMode::Standard => "\x1b[1;32mStandard\x1b[0m",
            }
        );
        println!("\x1b[1;36m╠══════════════════════════════════════════════════╣\x1b[0m");
        println!(
            "\x1b[1;36m║\x1b[0m  Press Ctrl+C to stop                           \x1b[1;36m║\x1b[0m"
        );
        println!("\x1b[1;36m╚══════════════════════════════════════════════════╝\x1b[0m");
        println!();

        info!("Preview server starting on http://{}", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
    }
}
