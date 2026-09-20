//! Example 55: Server HTTP Middleware Demo
//!
//! Demonstrates server HTTP middleware with:
//! - ServerHttpLoggingMiddleware with redaction
//! - Custom HTTP middleware (CORS)
//! - Query redaction and sensitive header protection
//! - Body gating for safe content types
//! - Complete server setup

use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpLoggingMiddleware, ServerHttpMiddleware,
    ServerHttpMiddlewareChain, ServerHttpResponse,
};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Simple echo tool for testing
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        Ok(json!({
            "echo": args,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }))
    }
}

/// Custom CORS middleware for browser clients
#[derive(Debug, Clone)]
struct CorsMiddleware {
    allowed_origins: Vec<String>,
}

#[async_trait]
impl ServerHttpMiddleware for CorsMiddleware {
    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        _context: &ServerHttpContext,
    ) -> Result<()> {
        // Add CORS headers
        response.add_header(
            "Access-Control-Allow-Origin",
            &self.allowed_origins.join(", "),
        );
        response.add_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
        response.add_header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization, MCP-Session-ID",
        );
        response.add_header("Access-Control-Max-Age", "86400");

        tracing::info!("CORS headers added for origins: {:?}", self.allowed_origins);
        Ok(())
    }

    fn priority(&self) -> i32 {
        90 // Run after logging (priority 50)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("🚀 Starting Server Middleware Demo");

    // Step 1: Create HTTP middleware chain
    tracing::info!("📦 Creating HTTP middleware chain...");
    let mut http_chain = ServerHttpMiddlewareChain::new();

    // Add logging middleware with secure defaults
    let logging = ServerHttpLoggingMiddleware::new()
        .with_level(tracing::Level::INFO)
        .with_redact_query(true) // Strip query parameters from logs
        .with_max_body_bytes(1024); // Log first 1KB of body

    http_chain.add(Arc::new(logging));

    // Add custom CORS middleware
    http_chain.add(Arc::new(CorsMiddleware {
        allowed_origins: vec![
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ],
    }));

    tracing::info!("✅ HTTP middleware chain configured:");
    tracing::info!("   1. ServerHttpLoggingMiddleware (priority 50)");
    tracing::info!("      - Redacts: authorization, cookie, x-api-key");
    tracing::info!("      - Query stripping enabled");
    tracing::info!("      - Body logging: first 1KB only");
    tracing::info!("   2. CorsMiddleware (priority 90)");
    tracing::info!("      - Allows: localhost:3000, example.com");

    // Step 2: Build server with HTTP middleware
    tracing::info!("🔧 Building server...");
    let server = Server::builder()
        .name("middleware-demo-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", EchoTool)
        .with_http_middleware(Arc::new(http_chain))
        .build()?;

    // Step 3: Create HTTP server config (middleware retrieved from server)
    let config = StreamableHttpServerConfig {
        http_middleware: server.http_middleware(),
        // v1-only as of Phase 117; gated per-field so the MIDDLEWARE this example
        // is actually about keeps building on `--features full-v2`.
        #[cfg(feature = "v1-compat")]
        session_id_generator: Some(Box::new(|| {
            format!("demo-session-{}", uuid::Uuid::new_v4())
        })),
        enable_json_response: true,
        ..Default::default()
    };

    let server = Arc::new(Mutex::new(server));

    let http_server =
        StreamableHttpServer::with_config("127.0.0.1:8080".parse().unwrap(), server, config);

    // Step 4: Start server
    tracing::info!("🌐 Starting HTTP server...");
    let (addr, handle) = http_server.start().await?;

    tracing::info!("✅ Server listening on: http://{}", addr);
    tracing::info!("");
    tracing::info!("📊 Server Features:");
    tracing::info!("   ✓ HTTP logging with sensitive data redaction");
    tracing::info!("   ✓ CORS headers for browser clients");
    tracing::info!("   ✓ Query parameter stripping");
    tracing::info!("   ✓ Body gating (safe content types only)");
    tracing::info!("   ✓ Session management");
    tracing::info!("");
    tracing::info!("🔬 Test the server with:");
    tracing::info!("   curl -X POST http://{}/messages \\", addr);
    tracing::info!("     -H 'Content-Type: application/json' \\");
    tracing::info!("     -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{}},\"clientInfo\":{{\"name\":\"test\",\"version\":\"1.0.0\"}}}}}}'");
    tracing::info!("");
    tracing::info!("💡 Features demonstrated:");
    tracing::info!("   ✓ HTTP middleware chain");
    tracing::info!("   ✓ ServerHttpLoggingMiddleware with redaction");
    tracing::info!("   ✓ Custom CORS middleware");
    tracing::info!("   ✓ Sensitive header protection");
    tracing::info!("   ✓ Query parameter stripping");
    tracing::info!("   ✓ Body logging with size limits");
    tracing::info!("");
    tracing::info!("Press Ctrl+C to stop the server");

    // Wait for server to finish
    let _ = handle.await;

    Ok(())
}
