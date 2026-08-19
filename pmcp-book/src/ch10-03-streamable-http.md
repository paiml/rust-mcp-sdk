# Chapter 10.3: Streamable HTTP (Server + Client)

> **Note on vocabulary**: **v1** means the MCP protocol revision `2025-11-25` and **v2** means `2026-07-28`. These are *protocol eras*, not crate versions — the crate is always written with its name attached ("pmcp 2.18"), so a bare "2.18" can never be mistaken for a protocol revision. One pmcp binary speaks both, negotiated per request. Unless a passage says otherwise, this chapter describes the **v1** behaviour: the stateless/stateful choice below is a build-time configuration, whereas v2 statelessness is a per-request property of the era itself. For the v2 story see [Chapter 12.17: Migrating to MCP 2026-07-28 (v2)](ch12-17-migrating-to-mcp-2026-07-28.md#for-servers).

Streamable HTTP is PMCP’s preferred transport for servers. It combines JSON requests with Server‑Sent Events (SSE) for notifications, supports both stateless and stateful operation, and uses a single endpoint with content negotiation via the `Accept` header.

## Why Streamable HTTP?

- Single endpoint for requests and notifications
- Works well through proxies and enterprise firewalls
- Optional sessions for stateful, multi-request workflows
- Built with Axum, provided by the SDK

## Server (Axum-based)

Types: `pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig}` (feature: `streamable-http`).

```rust
use pmcp::{Server, ServerCapabilities, ToolHandler, RequestHandlerExtra};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

struct Add;

#[async_trait]
impl ToolHandler for Add {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"result": a + b}))
    }
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("streamable-http-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", Add)
        .build()?;

    let server = Arc::new(Mutex::new(server));
    let addr: SocketAddr = ([0,0,0,0], 8080).into();

    // Default: stateful with SSE support
    let http = StreamableHttpServer::new(addr, server.clone());
    let (bound, _handle) = http.start().await?;
    println!("Streamable HTTP listening on {}", bound);
    Ok(())
}
```

### Stateless vs Stateful

```rust
// Stateless (serverless-friendly): no session tracking
let cfg = StreamableHttpServerConfig {
    session_id_generator: None,
    enable_json_response: false, // prefer SSE for notifications
    event_store: None,
    on_session_initialized: None,
    on_session_closed: None,
};
let http = StreamableHttpServer::with_config(addr, server, cfg);
```

## Protocol Details

Headers enforced by the server:
- `mcp-protocol-version`: Protocol version — `2025-11-25` (v1) or `2026-07-28` (v2)
- `Accept`: Must include `application/json` or `text/event-stream`
- `mcp-session-id`: Present in stateful mode
- `Last-Event-Id`: For SSE resumption

Accept rules:
- `Accept: application/json` → JSON responses only
- `Accept: text/event-stream` → SSE stream for notifications

## Client (Streamable HTTP)

Types: `pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpConfig}` (feature: `streamable-http`).

### Basic Client

```rust
use pmcp::{ClientBuilder, ClientCapabilities};
use pmcp::shared::{StreamableHttpTransport, StreamableHttpConfig};

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let config = StreamableHttpConfig::new("http://localhost:8080".to_string())
        .with_extra_headers(vec![
            ("X-Custom-Header".to_string(), "value".to_string())
        ]);

    let transport = StreamableHttpTransport::with_config(config).await?;
    let mut client = ClientBuilder::new(transport).build();

    let _info = client.initialize(ClientCapabilities::minimal()).await?;
    Ok(())
}
```

### HTTP Middleware Support

StreamableHttpTransport supports HTTP-level middleware for authentication, headers, and request/response processing:

```rust
use pmcp::{ClientBuilder, ClientCapabilities};
use pmcp::shared::{StreamableHttpTransport, StreamableHttpConfig};
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth_middleware::{OAuthClientMiddleware, BearerToken};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // 1. Create HTTP middleware chain
    let mut http_chain = HttpMiddlewareChain::new();

    // Add OAuth middleware for automatic token injection
    let token = BearerToken::with_expiry(
        "api-token-12345".to_string(),
        Duration::from_secs(3600) // 1 hour
    );
    http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // 2. Create transport config with HTTP middleware
    let config = StreamableHttpConfig::new("http://localhost:8080".to_string())
        .with_http_middleware(Arc::new(http_chain));

    let transport = StreamableHttpTransport::with_config(config).await?;

    // 3. Create client (protocol middleware optional)
    let mut client = ClientBuilder::new(transport).build();

    let _info = client.initialize(ClientCapabilities::minimal()).await?;
    Ok(())
}
```

**HTTP Middleware Features**:
- Automatic OAuth token injection (OAuthClientMiddleware)
- Token expiry checking and refresh triggers
- Custom header injection (implement HttpMiddleware trait)
- Request/response logging and metrics at HTTP layer
- Priority-based execution ordering

**OAuth Precedence**: If both `auth_provider` and HTTP middleware OAuth are configured, `auth_provider` takes precedence to avoid duplicate authentication.

See [Chapter 11: Middleware](ch11-middleware.md#http-level-middleware) for complete HTTP middleware documentation.

## Examples

- `examples/22_streamable_http_server_stateful.rs` – Stateful mode with SSE notifications
- `examples/23_streamable_http_server_stateless.rs` – Stateless/serverless-friendly configuration
- `examples/24_streamable_http_client.rs` – Client connecting to both modes

## Feature Flags

```toml
[dependencies]
pmcp = { version = "2.0", features = ["streamable-http"] }
```

