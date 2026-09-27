//! Minimal wiring shape for the `pmcp-openapi-server` library (CF-5).
//!
//! This example demonstrates the ≤15-line wiring shape: dispatch the connector
//! pair from config, build the server, and (in production) serve it. It is
//! BUILD-ONLY safe for CI — `main` builds the server and returns WITHOUT
//! blocking, so `cargo build --example openapi_server_min` (and even
//! `cargo run`) do NOT hang on a live listener. The acceptance check uses
//! `cargo build --example`.
//!
//! To run a real server, use the binary: `pmcp-openapi-server --config c.toml`.

use pmcp_openapi_server::{build_server, dispatch};
use pmcp_server_toolkit::ServerConfig;
use pmcp_server_toolkit::ToolkitHooks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A minimal in-memory config with a [backend] so dispatch succeeds offline.
    let cfg = ServerConfig::from_toml_strict_validated(
        r#"
[server]
name = "openapi-min"
version = "0.1.0"

[backend]
base_url = "https://api.example.com"
"#,
    )?;

    // dispatch builds the (connector, executor) pair lazily (no network, CF-2).
    let (connector, http_exec) = dispatch(&cfg).await?;

    // build_server assembles the pmcp::Server (no --spec → curated-only, D-03).
    let _server = build_server(
        &cfg,
        connector,
        http_exec,
        None,
        // Phase 128: this example registers no E1/E2 hook, so it passes the empty
        // default — the shape every pre-Phase-128 caller now has.
        &ToolkitHooks::default(),
    )?;

    // In production: `serve(_server, addr).await?` then await the handle. This
    // example returns WITHOUT serving so it never blocks (build-only safe).
    println!("pmcp-openapi-server assembled successfully (build-only example)");
    Ok(())
}
