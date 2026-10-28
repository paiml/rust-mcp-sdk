//! server-common template generator
//!
//! Generates the shared HTTP bootstrap crate with OAuth support used by all servers.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate server-common crate
pub fn generate(workspace_dir: &Path) -> Result<()> {
    let server_common_dir = workspace_dir.join("crates/server-common");
    fs::create_dir_all(&server_common_dir).context("Failed to create server-common directory")?;

    generate_cargo_toml(&server_common_dir)?;
    generate_lib_rs(&server_common_dir)?;

    println!("  {} Generated server-common crate", "✓".green());
    Ok(())
}

fn generate_cargo_toml(server_common_dir: &Path) -> Result<()> {
    let content = r#"[package]
name = "server-common"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = { workspace = true, features = ["full"] }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true, features = ["env-filter", "json"] }
async-trait = { workspace = true }
"#;

    fs::write(server_common_dir.join("Cargo.toml"), content)
        .context("Failed to create server-common Cargo.toml")?;

    Ok(())
}

/// Generated `server-common` `lib.rs` source.
///
/// Exposed as a module-level const so template-defaults tests can assert the
/// rendered source carries the spike-007 proxy-safe defaults without
/// re-rendering the whole crate.
const LIB_RS_TEMPLATE: &str = r#"//! Shared HTTP bootstrap for all MCP servers
//!
//! This module provides production-grade HTTP server setup used by all servers.
//! Binary servers just call `run_http()` with their configured server (~6 LOC).
//!
//! Features:
//! - Structured JSON logging for CloudWatch Logs compatibility
//! - Request ID tracking and correlation
//! - Performance metrics and tracing
//! - Error tracking and categorization
//! - Tool invocation logging for observability
//! - OAuth/OIDC authentication (optional, env-configured)
//!
//! # Authentication
//!
//! Enable OAuth authentication via environment variables:
//!
//! ```bash
//! # For AWS Cognito:
//! AUTH_PROVIDER=cognito
//! AUTH_REGION=us-east-1
//! AUTH_USER_POOL_ID=us-east-1_xxxxx
//! AUTH_CLIENT_ID=your-client-id
//!
//! # For generic OIDC (Google, Auth0, Okta, Entra):
//! AUTH_PROVIDER=oidc
//! AUTH_ISSUER=https://accounts.google.com
//! AUTH_CLIENT_ID=your-client-id
//!
//! # To disable auth (default):
//! AUTH_PROVIDER=none
//! ```

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::http_middleware::{
    ServerHttpMiddlewareChain, ServerHttpLoggingMiddleware,
    ServerHttpMiddleware, ServerHttpRequest, ServerHttpContext,
};
use pmcp::server::auth::{
    IdentityProvider, CognitoProvider, GenericOidcConfig, GenericOidcProvider,
};
use pmcp::{AllowedOrigins, Server};
use pmcp::error::Error as PmcpError;
use async_trait::async_trait;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use std::time::Duration;

/// Authentication provider type for runtime configuration.
pub enum AuthProviderType {
    /// No authentication required.
    None,
    /// AWS Cognito authentication.
    Cognito(Arc<CognitoProvider>),
    /// Generic OIDC authentication (Google, Auth0, Okta, Entra, etc.).
    Oidc(Arc<GenericOidcProvider>),
}

impl std::fmt::Debug for AuthProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Cognito(_) => write!(f, "Cognito"),
            Self::Oidc(_) => write!(f, "Oidc"),
        }
    }
}

/// OAuth authentication middleware using IdentityProvider.
pub struct OAuthMiddleware {
    provider: Arc<dyn IdentityProvider>,
}

impl OAuthMiddleware {
    /// Create new OAuth middleware with the given provider.
    pub fn new(provider: Arc<dyn IdentityProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ServerHttpMiddleware for OAuthMiddleware {
    /// Validate Bearer token on incoming requests.
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<(), PmcpError> {
        // Extract Bearer token from Authorization header
        let auth_header = request.headers
            .get("authorization")
            .or_else(|| request.headers.get("Authorization"));

        let token = match auth_header {
            Some(header) => {
                let header_str = header.to_str().unwrap_or("");
                if header_str.starts_with("Bearer ") {
                    Some(header_str.trim_start_matches("Bearer ").to_string())
                } else {
                    None
                }
            }
            None => None,
        };

        // Validate token if present
        if let Some(token) = token {
            match self.provider.validate_token(&token).await {
                Ok(auth_context) => {
                    tracing::debug!(
                        request_id = %context.request_id,
                        user_id = %auth_context.user_id(),
                        "Token validated successfully"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        request_id = %context.request_id,
                        error = %e,
                        "Token validation failed"
                    );
                    return Err(PmcpError::authentication("Invalid or expired token"));
                }
            }
        } else {
            // No token provided - log but allow (handler can enforce auth)
            tracing::debug!(
                request_id = %context.request_id,
                "No Bearer token in request"
            );
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        10 // Run early (before logging at 90)
    }
}

/// Run HTTP server with production middleware and logging
///
/// This function:
/// - Sets up production JSON logging for enterprise observability
/// - Adds request ID tracking for distributed tracing
/// - Configures performance monitoring and metrics
/// - Resolves port from PORT/MCP_HTTP_PORT env vars (default: 3000)
/// - Binds to 0.0.0.0 for container compatibility
/// - Starts StreamableHttpServer with observability middleware
///
/// # Example
/// ```no_run
/// use server_common::run_http;
/// use pmcp::Server;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = Server::builder()
///         .name("calculator")
///         .version("1.0.0")
///         .build()?;
///     run_http(server, "calculator", "1.0.0").await
/// }
/// ```
pub async fn run_http(server: Server, server_name: &str, server_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize production logging
    init_logging();

    // Log server info for observability
    let server_name = server_name.to_string();
    let server_version = server_version.to_string();

    tracing::info!(
        server_name = %server_name,
        server_version = %server_version,
        "Initializing MCP server"
    );

    // Initialize auth provider from environment
    let auth_provider = init_auth_provider().await;

    tracing::info!(
        auth_provider = ?auth_provider,
        "Authentication configured"
    );

    // Resolve port (PORT > MCP_HTTP_PORT > 3000)
    let port = resolve_port();
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    tracing::info!(
        port = port,
        address = %addr,
        "Starting MCP HTTP server"
    );

    // Share the server; `with_config_concurrent` serves requests in parallel
    let server = Arc::new(server);

    // Create HTTP middleware chain with logging and optional auth
    let mut middleware_chain = ServerHttpMiddlewareChain::new();
    middleware_chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));

    // Add OAuth middleware if auth is enabled
    match &auth_provider {
        AuthProviderType::Cognito(provider) => {
            tracing::info!("Adding Cognito OAuth middleware");
            middleware_chain.add(Arc::new(OAuthMiddleware::new(provider.clone())));
        }
        AuthProviderType::Oidc(provider) => {
            tracing::info!("Adding OIDC OAuth middleware");
            middleware_chain.add(Arc::new(OAuthMiddleware::new(provider.clone())));
        }
        AuthProviderType::None => {
            tracing::info!("Authentication disabled");
        }
    }

    // Create stateless configuration with observability
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: Some(Arc::new(middleware_chain)),
        // Proxy-safe: required behind ACA/Lambda/Cloud Run ingress — None = localhost-only = 403.
        // Deliberate GLOBAL default for scaffolded servers: it broadens browser-origin access so
        // the server works through any reverse proxy out of the box.
        // Production: restrict to your deployment FQDN and/or add auth once the hostname is known —
        // see DEPLOYMENT.md security note.
        allowed_origins: Some(AllowedOrigins::any()),
        ..Default::default()
    };

    // Create and start the HTTP server
    let http_server = StreamableHttpServer::with_config_concurrent(addr, server, config);
    let (bound_addr, server_handle) = http_server.start().await?;

    tracing::info!(
        actual_address = %bound_addr,
        server_name = %server_name,
        "Server started successfully"
    );

    // Log periodic health check for monitoring
    let health_check_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            tracing::info!(
                server_name = %server_name,
                "Health check - server running"
            );
        }
    });

    // Wait for server to finish
    tokio::select! {
        result = server_handle => {
            result?;
        }
        _ = health_check_handle => {
            // Health check task ended unexpectedly
        }
    }

    Ok(())
}

/// Initialize authentication provider from environment variables.
///
/// Supports:
/// - `AUTH_PROVIDER=cognito` with `AUTH_REGION`, `AUTH_USER_POOL_ID`, `AUTH_CLIENT_ID`
/// - `AUTH_PROVIDER=oidc` with `AUTH_ISSUER`, `AUTH_CLIENT_ID`
/// - `AUTH_PROVIDER=none` (default)
async fn init_auth_provider() -> AuthProviderType {
    let provider_type = std::env::var("AUTH_PROVIDER")
        .unwrap_or_else(|_| "none".to_string())
        .to_lowercase();

    match provider_type.as_str() {
        "cognito" => {
            let region = std::env::var("AUTH_REGION")
                .expect("AUTH_REGION required for Cognito auth");
            let user_pool_id = std::env::var("AUTH_USER_POOL_ID")
                .expect("AUTH_USER_POOL_ID required for Cognito auth");
            let client_id = std::env::var("AUTH_CLIENT_ID")
                .expect("AUTH_CLIENT_ID required for Cognito auth");

            tracing::info!(
                region = %region,
                user_pool_id = %user_pool_id,
                "Initializing Cognito provider"
            );

            match CognitoProvider::new(&region, &user_pool_id, &client_id).await {
                Ok(provider) => AuthProviderType::Cognito(Arc::new(provider)),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize Cognito provider");
                    panic!("Failed to initialize Cognito provider: {}", e);
                }
            }
        }
        "oidc" | "google" | "auth0" | "okta" | "entra" => {
            let issuer = std::env::var("AUTH_ISSUER")
                .expect("AUTH_ISSUER required for OIDC auth");
            let client_id = std::env::var("AUTH_CLIENT_ID")
                .expect("AUTH_CLIENT_ID required for OIDC auth");
            let client_secret = std::env::var("AUTH_CLIENT_SECRET").ok();

            tracing::info!(
                issuer = %issuer,
                provider_type = %provider_type,
                "Initializing OIDC provider"
            );

            // Create appropriate config based on provider type
            let mut config = match provider_type.as_str() {
                "google" => GenericOidcConfig::google(&client_id),
                "auth0" => {
                    // Extract domain from issuer for Auth0
                    let domain = issuer.trim_start_matches("https://").trim_end_matches('/');
                    GenericOidcConfig::auth0(domain, &client_id)
                }
                "okta" => {
                    let domain = issuer.trim_start_matches("https://");
                    GenericOidcConfig::okta(domain, &client_id)
                }
                "entra" => {
                    // Extract tenant ID from issuer for Entra
                    // Format: https://login.microsoftonline.com/{tenant}/v2.0
                    let parts: Vec<&str> = issuer.split('/').collect();
                    let tenant_id = *parts.get(3).unwrap_or(&"common");
                    GenericOidcConfig::entra(tenant_id, &client_id)
                }
                _ => GenericOidcConfig::new(
                    "oidc",
                    "Generic OIDC",
                    &issuer,
                    &client_id,
                ),
            };

            if let Some(secret) = client_secret {
                config = config.with_client_secret(secret);
            }

            match GenericOidcProvider::new(config).await {
                Ok(provider) => AuthProviderType::Oidc(Arc::new(provider)),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize OIDC provider");
                    panic!("Failed to initialize OIDC provider: {}", e);
                }
            }
        }
        "none" | "" => {
            tracing::info!("Authentication disabled");
            AuthProviderType::None
        }
        other => {
            tracing::warn!(provider = %other, "Unknown auth provider, disabling auth");
            AuthProviderType::None
        }
    }
}

/// Initialize production logging with structured JSON format
fn init_logging() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default logging configuration for production
            // - INFO level for pmcp and server crates
            // - WARN level for dependencies
            "info,tower_http=debug,pmcp=info,server=info".into()
        });

    // Detect if we're running in AWS Lambda or containerized environment
    let is_lambda = std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();
    let is_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(is_lambda); // Default to JSON in Lambda

    if is_json {
        // JSON format for CloudWatch Logs and structured logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(true)
                    .with_current_span(true)
                    .with_span_list(true)
            )
            .init();
    } else {
        // Human-readable format for local development
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    // Log initialization info
    tracing::info!(
        log_format = if is_json { "json" } else { "pretty" },
        "Logging initialized"
    );
}

/// Resolve HTTP port from environment variables
///
/// Priority: PORT > MCP_HTTP_PORT > 3000 (default)
fn resolve_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| {
            std::env::var("MCP_HTTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(3000)
}

/// Log MCP tool invocation for observability
///
/// This should be called by tool implementations to track usage
pub fn log_tool_invocation(tool_name: &str, request_id: Option<&str>) {
    tracing::info!(
        tool = %tool_name,
        request_id = request_id.unwrap_or("unknown"),
        event = "tool_invoked",
        "MCP tool invoked"
    );
}

/// Log MCP tool error for monitoring
pub fn log_tool_error(tool_name: &str, error: &str, request_id: Option<&str>) {
    tracing::error!(
        tool = %tool_name,
        error = %error,
        request_id = request_id.unwrap_or("unknown"),
        event = "tool_error",
        "MCP tool error"
    );
}

/// Validate a token using the configured auth provider.
///
/// Returns the authenticated user's subject (user ID) or an error.
/// This is useful for tools that need to know who is calling them.
pub async fn validate_token(
    provider: &AuthProviderType,
    token: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match provider {
        AuthProviderType::None => Ok("anonymous".to_string()),
        AuthProviderType::Cognito(p) => {
            let context = p.validate_token(token).await?;
            Ok(context.user_id().to_string())
        }
        AuthProviderType::Oidc(p) => {
            let context = p.validate_token(token).await?;
            Ok(context.user_id().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Environment variable tests require sequential execution
    // Run with: cargo test --test-threads=1

    #[test]
    fn test_resolve_port_default() {
        // Test runs with whatever env vars are set
        let port = resolve_port();
        assert!(port >= 1 && port <= 65535, "Port should be valid");
    }

    #[test]
    fn test_auth_provider_type_debug() {
        let none = AuthProviderType::None;
        assert_eq!(format!("{:?}", none), "None");
    }
}
"#;

fn generate_lib_rs(server_common_dir: &Path) -> Result<()> {
    let content = LIB_RS_TEMPLATE;

    fs::write(server_common_dir.join("src").join("lib.rs"), content)
        .or_else(|_| {
            // Create src/ directory if it doesn't exist
            fs::create_dir(server_common_dir.join("src"))?;
            fs::write(server_common_dir.join("src").join("lib.rs"), content)
        })
        .context("Failed to create server-common lib.rs")?;

    Ok(())
}

#[cfg(test)]
mod template_defaults_tests {
    use super::LIB_RS_TEMPLATE;

    // Locks the two spike-007 traps into the generated server template against
    // future edits (same defensive shape as the azure `dockerfile_carries_007_findings` test).

    #[test]
    fn template_sets_proxy_safe_allowed_origins() {
        // 007 Finding 1: allowed_origins: None 403s every request through ingress.
        assert!(
            LIB_RS_TEMPLATE.contains("AllowedOrigins::any()"),
            "scaffolded server must set allowed_origins: Some(AllowedOrigins::any()) for proxy-safe ingress"
        );
    }

    #[test]
    fn template_binds_unspecified_not_localhost() {
        // 007 Finding 2: binding 127.0.0.1 makes the container unreachable behind ingress.
        assert!(
            LIB_RS_TEMPLATE.contains("Ipv4Addr::UNSPECIFIED"),
            "scaffolded server must bind 0.0.0.0 (Ipv4Addr::UNSPECIFIED) for container reachability"
        );
        assert!(
            !LIB_RS_TEMPLATE.contains("Ipv4Addr::LOCALHOST"),
            "scaffolded server must NOT bind Ipv4Addr::LOCALHOST — unreachable behind ingress"
        );
    }
}
