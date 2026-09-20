//! Example: Stateless Streamable HTTP Server
//!
//! This example demonstrates:
//! - Running an MCP server over HTTP without session management
//! - Simplified stateless operation
//! - Perfect for serverless deployments (AWS Lambda, etc.)
//! - No session overhead or tracking
//!
//! Run this server with:
//! ```bash
//! cargo run --example 23_streamable_http_server_stateless
//! ```
//!
//! Then connect with the HTTP client example or any MCP-compatible HTTP client.

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::{Server, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// === Tool Implementations (same as stateful example) ===

/// Echo tool - returns the input message
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("(no message provided)");

        Ok(json!({
            "echo": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Calculator tool - performs basic arithmetic
#[derive(Debug, Deserialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize)]
struct CalculatorResult {
    result: f64,
    expression: String,
}

struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        let result = match params.operation.as_str() {
            "add" => params.a + params.b,
            "subtract" => params.a - params.b,
            "multiply" => params.a * params.b,
            "divide" => {
                if params.b == 0.0 {
                    return Err(pmcp::Error::validation("Division by zero"));
                }
                params.a / params.b
            },
            op => {
                return Err(pmcp::Error::validation(format!(
                    "Unknown operation: {}",
                    op
                )));
            },
        };

        let expression = format!(
            "{} {} {} = {}",
            params.a, params.operation, params.b, result
        );

        Ok(serde_json::to_value(CalculatorResult {
            result,
            expression,
        })?)
    }
}

/// Random number tool - generates pseudo-random numbers
struct RandomNumberTool;

#[async_trait]
impl ToolHandler for RandomNumberTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let min = args.get("min").and_then(|v| v.as_i64()).unwrap_or(0);
        let max = args.get("max").and_then(|v| v.as_i64()).unwrap_or(100);

        if min >= max {
            return Err(pmcp::Error::validation("min must be less than max"));
        }

        // Simple pseudo-random using timestamp
        // In a real application, you'd use a proper random number generator
        let now = chrono::Utc::now();
        let seed = now.timestamp_nanos_opt().unwrap_or(0);
        let range = (max - min) as u64;
        let random_number = min + ((seed as u64 % range) as i64);

        Ok(json!({
            "number": random_number,
            "range": format!("[{}, {})", min, max),
            "timestamp": now.to_rfc3339(),
            "note": "Using timestamp-based pseudo-random for demo purposes"
        }))
    }
}

/// Server info tool - returns information about the server mode
struct ServerInfoTool;

#[async_trait]
impl ToolHandler for ServerInfoTool {
    async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "message": "This is a stateless server - no session management",
            "server_mode": "stateless",
            "benefits": [
                "No session overhead",
                "Perfect for serverless",
                "Simplified operation",
                "Horizontal scaling friendly",
                "No state to manage"
            ],
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    info!("Starting Stateless Streamable HTTP Server Example");

    // Build the MCP server with tools
    let server = Server::builder()
        .name("stateless-http-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", EchoTool)
        .tool("calculate", CalculatorTool)
        .tool("random", RandomNumberTool)
        .tool("server_info", ServerInfoTool)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Configure the HTTP server address (different port from stateful example)
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8081);

    info!("Creating stateless HTTP server on {}", addr);

    // Create stateless configuration.
    //
    // `stateless()` IS this configuration — no session id generator, no event
    // store, no session callbacks, JSON responses — and it already carries the
    // `v1-compat` gates internally, so this example compiles and runs unchanged
    // on `--no-default-features --features full-v2` without naming a single
    // gated field. The one deviation is `allowed_origins`: `stateless()` uses
    // `AllowedOrigins::any()` for reverse-proxied serverless deployments, and
    // this example binds a local port directly.
    let config = StreamableHttpServerConfig {
        allowed_origins: None,
        ..StreamableHttpServerConfig::stateless()
    };

    // Create the streamable HTTP server in stateless mode
    let http_server = StreamableHttpServer::with_config(addr, server, config);

    // Start the server
    let (bound_addr, server_handle) = http_server
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       STATELESS STREAMABLE HTTP SERVER RUNNING            ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Address: http://{:43} ║", bound_addr);
    println!("║ Mode:    Stateless (no session management)                ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Features:                                                  ║");
    println!("║ • No session IDs generated or required                    ║");
    println!("║ • Each request is independent                             ║");
    println!("║ • Perfect for serverless deployments                      ║");
    println!("║ • Re-initialization allowed                               ║");
    println!("║ • Simplified operation with less overhead                 ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Ideal for:                                                 ║");
    println!("║ • AWS Lambda / Azure Functions                            ║");
    println!("║ • Kubernetes pods with horizontal scaling                 ║");
    println!("║ • Simple request/response workflows                       ║");
    println!("║ • Development and testing                                 ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Available Tools:                                           ║");
    println!("║ • echo        - Echo back messages                        ║");
    println!("║ • calculate   - Perform arithmetic                        ║");
    println!("║ • random      - Generate random numbers                   ║");
    println!("║ • server_info - Get server mode information               ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Connect with:                                              ║");
    println!("║ cargo run --example 24_streamable_http_client -- stateless║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Press Ctrl+C to stop the server");

    // Keep the server running
    server_handle.await.map_err(|e| {
        Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error>
    })?;

    Ok(())
}
