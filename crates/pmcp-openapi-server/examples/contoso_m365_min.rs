//! `oauth_passthrough` wiring shape for `pmcp-openapi-server` (P902-EXAMPLE).
//!
//! The 90.2 analog of `london_tube_min.rs`, re-skinned to the Contoso M365 /
//! Microsoft Graph / **per-user delegated OAuth** showcase: an `oauth_passthrough`
//! backend (no standing credential — the inbound user `Authorization: Bearer` is
//! forwarded to Graph), one `get_customer` SCRIPT tool, a `[[resources]]` block, and
//! the `start_code_mode` `[[prompts]]` block. Like its sibling it is BUILD-AND-EXIT
//! safe for CI — `main` builds the server and returns WITHOUT serving, so neither
//! `cargo build --example contoso_m365_min` nor `cargo run --example contoso_m365_min`
//! hang on a live listener (and no token/backend is ever contacted).
//!
//! Run with:
//! ```sh
//! cargo run --example contoso_m365_min -p pmcp-openapi-server
//! ```
//!
//! The pointable, full-surface equivalent (the config a user runs the binary
//! against) ships alongside this file as `examples/contoso-m365.toml`:
//! `pmcp-openapi-server --config crates/pmcp-openapi-server/examples/contoso-m365.toml`.

use pmcp_openapi_server::{build_server, dispatch};
use pmcp_server_toolkit::ServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse the trimmed (oauth_passthrough + one script tool + resource + prompt)
    // config. The inline dev-only token_secret is guarded by
    // allow_inline_token_secret_for_dev, so the example needs no env wiring. The
    // oauth_passthrough auth path holds NO standing credential (the user bearer
    // arrives per-request); this example never serves, so the backend is never called.
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG)?;

    // dispatch builds the (connector, executor) pair lazily (no network, CF-2).
    let (connector, http_exec) = dispatch(&cfg).await?;

    // build_server assembles the pmcp::Server (no --spec → curated-only, D-03).
    let _server = build_server(&cfg, connector, http_exec, None)?;

    // Summary line proving the resources/prompts surface assembled.
    println!(
        "{} assembled: {} tool(s), {} resource(s), {} prompt(s) \
         (trimmed example — full surface in examples/contoso-m365.toml)",
        cfg.server.name,
        cfg.tools.len(),
        cfg.resources.len(),
        cfg.prompts.len()
    );

    // In production: `serve(_server, addr).await?` then await the handle. This
    // example returns WITHOUT serving so it never blocks (build-and-exit safe).
    Ok(())
}

/// Trimmed Contoso M365 config — `oauth_passthrough` backend auth, one
/// `get_customer` SCRIPT tool, one `[[resources]]` block, and the
/// `start_code_mode` `[[prompts]]` block. Kept close to `examples/contoso-m365.toml`
/// so drift is obvious; that pointable file carries the full two-tool /
/// three-resource surface. NO real secret value — the only credential literal is
/// the dev-guarded inline `token_secret`.
const CONFIG: &str = r#"
[server]
name = "contoso-m365"
version = "1.0.0"

[backend]
base_url = "https://graph.microsoft.com/v1.0"

[backend.auth]
type = "oauth_passthrough"
target_header = "Authorization"
required = true

[[tools]]
name = "get_customer"
description = "Fetch one customer row from the Contoso Customers sheet."
script = """
const resp = await api.get("/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='A2:D7')?$select=values");
const rows = resp.values;
const matches = rows.filter(row => row[0] === args.customer_id);
return matches;
"""

[[tools.parameters]]
name = "customer_id"
type = "string"
description = "Customer id, e.g. 'C007'."
required = true

[tools.annotations]
read_only_hint = true
idempotent_hint = true
open_world_hint = true
cost_hint = "low"

[[resources]]
uri = "docs://contoso-m365/schema"
name = "Contoso Workbook Schema"
description = "Customers/Orders sheet columns and the id->row addressing convention"
mime_type = "text/markdown"
content = """
# Contoso M365 Workbook
- Customers columns A..D: customer_id, name, segment, region (data from row 2).
- get_customer reads the whole block (A2:D7) and filters by the customer_id column.
"""

[[prompts]]
name = "start_code_mode"
description = "Load all context needed for Code Mode script generation"
include_resources = ["docs://contoso-m365/schema"]

[code_mode]
enabled = true
token_secret = "contoso-m365-min-example-dev-secret-32b"
allow_inline_token_secret_for_dev = true
"#;
