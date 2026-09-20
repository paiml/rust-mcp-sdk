//! Resources/prompts-bearing wiring shape for `pmcp-openapi-server` (P901-EXAMPLE).
//!
//! This example mirrors `openapi_server_min.rs` but showcases the **enriched**
//! london-tube config surface: a `[[resources]]` block + the `start_code_mode`
//! `[[prompts]]` block (the Code Mode context the SQL showcase also ships). Like
//! its sibling it is BUILD-AND-EXIT safe for CI — `main` builds the server and
//! returns WITHOUT serving, so neither `cargo build --example london_tube_min`
//! nor `cargo run --example london_tube_min` hang on a live listener.
//!
//! Run with:
//! ```sh
//! cargo run --example london_tube_min -p pmcp-openapi-server
//! ```
//!
//! The pointable, full-surface equivalent (the config a user runs the binary
//! against) ships alongside this file as `examples/london-tube.toml`. Its
//! `base_url` is a slot, not a baked literal, so BOTH variables must be set:
//! ```sh
//! TFL_BASE_URL=https://api.tfl.gov.uk TFL_APP_KEY=<your-key> \
//!   pmcp-openapi-server --config crates/pmcp-openapi-server/examples/london-tube.toml
//! ```

use pmcp_openapi_server::{build_server, dispatch};
use pmcp_server_toolkit::ServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse the enriched (resources + prompt + annotations) config. The inline
    // dev-only token_secret is guarded by allow_inline_token_secret_for_dev, so
    // the example needs no env wiring. `${TFL_APP_KEY}` stays a placeholder
    // (required = false → omitted when unset); this example never serves, so the
    // backend is never called.
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG)?;

    // dispatch builds the (connector, executor) pair lazily (no network, CF-2).
    let (connector, http_exec) = dispatch(&cfg).await?;

    // build_server assembles the pmcp::Server (no --spec → curated-only, D-03).
    let _server = build_server(&cfg, connector, http_exec, None)?;

    // Summary line proving the resources/prompts surface assembled.
    println!(
        "{} assembled: {} tool(s), {} resource(s), {} prompt(s) \
         (trimmed example — full surface in examples/london-tube.toml)",
        cfg.server.name,
        cfg.tools.len(),
        cfg.resources.len(),
        cfg.prompts.len()
    );

    // In production: `serve(_server, addr).await?` then await the handle. This
    // example returns WITHOUT serving so it never blocks (build-and-exit safe).
    Ok(())
}

/// Trimmed enriched london-tube config — one single-call tool with full
/// annotations, one `[[resources]]` block, and the `start_code_mode`
/// `[[prompts]]` block. Kept close to `examples/london-tube.toml` so drift is
/// obvious; that pointable file carries the full two-tool / three-resource surface.
const CONFIG: &str = r#"
[server]
name = "london-tube"
version = "1.1.0"

[backend]
base_url = "https://api.tfl.gov.uk"

[backend.auth]
type = "api_key"
query_params = { app_key = "${TFL_APP_KEY}" }
required = false

[[tools]]
name = "get-tube-status"
description = "Get the current status of all London Underground lines."
path = "/Line/Mode/tube/Status"
method = "GET"

[tools.annotations]
read_only_hint = true
idempotent_hint = true
open_world_hint = true
cost_hint = "low"

[[resources]]
uri = "docs://london-tube/schema"
name = "TfL Line API Schema"
description = "Endpoints, response shapes, and line ids for the TfL Line API"
mime_type = "text/markdown"
content = """
# TfL Line API (subset)
- GET /Line/Mode/tube/Status — status for every tube line.
- GET /Line/{lineId}/Disruption — per-line disruption detail (lineId lowercase).
statusSeverity < 10 means the line is disrupted.
"""

[[prompts]]
name = "start_code_mode"
description = "Load all context needed for Code Mode script generation"
include_resources = ["docs://london-tube/schema"]

[code_mode]
enabled = true
token_secret = "london-tube-min-example-dev-secret-32b"
allow_inline_token_secret_for_dev = true
"#;
