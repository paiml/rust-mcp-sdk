//! Axum Router convenience function for MCP servers.
//!
//! Provides `concurrent_router` and `concurrent_router_with_config` (and the
//! request-serializing `router` and `router_with_config`) that return a
//! fully-configured `axum::Router` with DNS rebinding protection,
//! security response headers, and origin-locked CORS applied as Tower
//! Layers.
//!
//! # Example
//!
//! ```rust,no_run
//! use pmcp::server::axum_router::{concurrent_router, AllowedOrigins, RouterConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = pmcp::Server::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .build()?;
//!
//! // One-liner: secure MCP server with localhost protection. Requests are
//! // handled concurrently; see `concurrent_router`.
//! let app = concurrent_router(Arc::new(server));
//!
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
//! axum::serve(listener, app).await?;
//! # Ok(())
//! # }
//! ```

use crate::server::streamable_http_server::server_handle::ServerHandle;
use crate::server::streamable_http_server::{
    build_mcp_router, make_server_state, StreamableHttpServerConfig,
};
use crate::server::tower_layers::{DnsRebindingLayer, SecurityHeadersLayer};
use crate::server::Server;
use axum::Router;
use std::sync::Arc;

// Re-export for convenience so users can import from pmcp::axum::*
pub use crate::server::tower_layers::AllowedOrigins;

/// Configuration for the MCP Axum Router.
///
/// Controls allowed origins, security headers, and underlying streamable
/// HTTP settings (sessions, middleware, etc.).
#[derive(Debug, Default)]
pub struct RouterConfig {
    /// Allowed origins for DNS rebinding protection and CORS.
    /// Defaults to localhost aliases when `None`.
    pub allowed_origins: Option<AllowedOrigins>,
    /// Security headers configuration. Defaults to all enabled.
    pub security_headers: SecurityHeadersLayer,
    /// Streamable HTTP server configuration (sessions, middleware, etc.)
    pub server_config: StreamableHttpServerConfig,
}

/// Create a secure MCP Axum Router with default localhost protection.
///
/// Returns an [`axum::Router`] with:
/// - DNS rebinding protection (Host + Origin validation)
/// - Security response headers (nosniff, DENY, no-store)
/// - Origin-locked CORS (no wildcard `*`)
///
/// Bind a listener and serve:
///
/// ```rust,no_run
/// # async fn example(app: axum::Router) {
/// let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
/// axum::serve(listener, app).await.unwrap();
/// # }
/// ```
///
/// # Requests are handled one at a time
///
/// The transport holds `server`'s mutex while each handler runs, so one slow
/// tool call delays every other request. Prefer [`concurrent_router`].
pub fn router(server: Arc<tokio::sync::Mutex<Server>>) -> Router {
    router_with_config(server, RouterConfig::default())
}

/// Create a secure MCP Axum Router with explicit configuration.
///
/// Use this when deploying to production with specific allowed origins:
///
/// ```rust,no_run
/// # use pmcp::server::axum_router::*;
/// # fn example(server: std::sync::Arc<tokio::sync::Mutex<pmcp::Server>>) {
/// let app = router_with_config(server, RouterConfig {
///     allowed_origins: Some(AllowedOrigins::explicit(vec![
///         "https://myapp.example.com".to_string(),
///     ])),
///     ..Default::default()
/// });
/// # }
/// ```
///
/// Serializes request handling as [`router`] does; prefer
/// [`concurrent_router_with_config`].
pub fn router_with_config(server: Arc<tokio::sync::Mutex<Server>>, config: RouterConfig) -> Router {
    secure_router(server.into(), config)
}

/// Create a secure MCP Axum Router that handles requests concurrently, with
/// default localhost protection.
///
/// The same router as [`router`], except that requests share `server` without
/// an outer mutex, so a slow tool call never delays another request. Every
/// `Server` entry point takes `&self` and guards its own mutable state.
pub fn concurrent_router(server: Arc<Server>) -> Router {
    concurrent_router_with_config(server, RouterConfig::default())
}

/// Create a secure MCP Axum Router that handles requests concurrently, with
/// explicit configuration. See [`concurrent_router`] and [`router_with_config`].
pub fn concurrent_router_with_config(server: Arc<Server>, config: RouterConfig) -> Router {
    secure_router(server.into(), config)
}

/// The one router construction both server shapes share.
fn secure_router(server: ServerHandle, config: RouterConfig) -> Router {
    let allowed = config
        .allowed_origins
        .unwrap_or_else(AllowedOrigins::localhost);

    // Ensure server_config carries the resolved origins so that
    // make_server_state() stores the same value in ServerState.
    let mut server_config = config.server_config;
    server_config.allowed_origins = Some(allowed.clone());

    let state = make_server_state(server, server_config);
    let base_router = build_mcp_router(state);
    let cors = crate::server::tower_layers::build_mcp_cors_layer(&allowed);

    // Layer ordering: CORS (outermost) -> DnsRebinding -> SecurityHeaders -> handler
    base_router
        .layer(config.security_headers)
        .layer(DnsRebindingLayer::new(allowed))
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_returns_router() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = Server::builder()
                .name("test")
                .version("0.1.0")
                .build()
                .unwrap();
            let server = Arc::new(tokio::sync::Mutex::new(server));
            let _app = router(server);
        });
    }

    #[test]
    fn test_router_with_explicit_origins() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = Server::builder()
                .name("test")
                .version("0.1.0")
                .build()
                .unwrap();
            let server = Arc::new(tokio::sync::Mutex::new(server));
            let _app = router_with_config(
                server,
                RouterConfig {
                    allowed_origins: Some(AllowedOrigins::explicit(vec![
                        "https://example.com".to_string()
                    ])),
                    ..Default::default()
                },
            );
        });
    }

    #[test]
    fn test_router_config_default() {
        let config = RouterConfig::default();
        assert!(config.allowed_origins.is_none());
    }
}
