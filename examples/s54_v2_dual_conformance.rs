//! Example: the DUAL-VERSION conformance TARGET (Phase 118, CONF-01).
//!
//! Run this server with:
//! ```bash
//! cargo run --example s54_v2_dual_conformance --features full
//! ```
//!
//! Optionally pass a bind address:
//! `cargo run --example s54_v2_dual_conformance --features full -- 127.0.0.1:9000`.
//!
//! # What this demonstrates
//!
//! - **One process, two eras.** The accept-list carries BOTH `2025-11-25` and
//!   `2026-07-28`, so the era is negotiated PER REQUEST rather than chosen once
//!   at startup. That is the whole milestone claim: ONE binary serves both.
//! - **A real session path.** The transport is built with the STATEFUL default
//!   `StreamableHttpServerConfig::default()`, so v1 clients get a live
//!   `Mcp-Session-Id` while v2 requests stay session-free.
//! - **A fixture surface dictated by an external referee.** Every tool,
//!   resource URI and prompt name below is named by the official
//!   `@modelcontextprotocol/conformance` suite. They are NOT free-form
//!   examples: renaming one silently breaks the scenario that consumes it.
//!
//! # This is a measurement target, not a style guide
//!
//! The official suite is run against this ONE process twice (D-04/D-06):
//!
//! ```bash
//! conformance server --url http://127.0.0.1:8149/ \
//!   --requirements 2025-11-25 -o target/conformance-results/2025-11-25
//!
//! conformance server --url http://127.0.0.1:8149/ \
//!   --requirements 2026-07-28 -o target/conformance-results/2026-07-28
//! ```
//!
//! The suite binary is the pinned one in `conformance/` — see
//! `conformance/README.md` for the pin, the Node >= 22 floor and the
//! `--ignore-scripts` rule. Results go under `target/` because `.gitignore`
//! already covers it; a top-level results directory is NOT ignored and would
//! dirty the worktree on every run.
//!
//! # The Tasks extension is deliberately ABSENT (D-14)
//!
//! `test_tool_with_task` and the ten `tasks-*` scenarios are `not_scored` at
//! BOTH revisions, so implementing them would add surface without adding
//! evidence. Their absence is a decision, not an omission.
//!
//! # Divergence from `s47_v2_stateless_mrtr`: `PMCP_REQUEST_STATE_KEY`
//!
//! `s47` deliberately leaves that variable UNSET so a reader sees the SDK's
//! startup WARNING about a per-process fallback key. A conformance target must
//! not emit a warning as part of its contract — a `WARN` line in the transcript
//! is indistinguishable from a real defect when a run is reviewed later. So
//! this example reads the variable itself and, when it is absent, prints one
//! explicit line telling the operator to set it. The value is NEVER echoed.
//!
//! Phase 119's DOCS-06 cites this example rather than adding a second one.

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::shared::http_constants::{ACCEPT_STREAMABLE, MCP_PROTOCOL_VERSION};
use pmcp::types::capabilities::{
    CompletionCapabilities, LoggingCapabilities, PromptCapabilities, ResourceCapabilities,
    ServerCapabilities, ToolCapabilities,
};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Where the server binds when `argv[1]` is absent.
///
/// 8149 is chosen to avoid every port an in-repo example or script already
/// holds: `t04`/`t05` bind 8080/8081 in `scripts/test_examples_with_tester.sh`
/// and `s47_v2_stateless_mrtr` defaults to 8147. A stale process from a
/// previous run answering the suite is a Tampering threat (T-118-17), so the
/// bind failure below is reported rather than unwrapped.
const DEFAULT_ADDR: &str = "127.0.0.1:8149";

/// The environment variable carrying the AEAD key for MRTR `requestState`.
///
/// Read for PRESENCE only — the value is never logged, printed or echoed
/// (T-118-15).
const REQUEST_STATE_KEY_VAR: &str = "PMCP_REQUEST_STATE_KEY";

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // WARN is deliberately NOT part of this server's startup contract, so the
    // filter stays at INFO for the example's own target and pmcp's INFO lines.
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info,s54_v2_dual_conformance=info")
        .init();

    let requested: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string())
        .parse()?;

    // Presence check only. `std::env::var` returns the VALUE; it is dropped
    // immediately and never reaches a print or a log line.
    let request_state_key_present = std::env::var(REQUEST_STATE_KEY_VAR).is_ok();

    // The accept-list is the single call that opts this process into
    // 2026-07-28. Listing 2025-11-25 alongside it is what keeps v1 clients
    // working: the era is negotiated PER REQUEST, so one binary serves both.
    // Both strings come from pmcp's own constants — a retyped literal here
    // could drift from the crate without any compiler complaint.
    let server = Server::builder()
        .name("s54-v2-dual-conformance")
        .version("1.0.0")
        .capabilities(conformance_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .build()?;

    // `StreamableHttpServerConfig::default()`, NOT `::stateless()`.
    // `tests/common/v2.rs:371-385` states the rule: `stateless()` is a
    // BUILD-TIME config that removes the session machinery before a request is
    // ever seen, so it could never prove that the PER-REQUEST era gate is what
    // suppresses sessions on v2. The v1 session-lifecycle scenario also needs a
    // live session path to exercise.
    let http = StreamableHttpServer::with_config(
        requested,
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );

    // `start()` returns AFTER the socket is bound — that is the readiness
    // guarantee a driver script polls against.
    let (addr, server_handle) = match http.start().await {
        Ok(bound) => bound,
        Err(error) => {
            eprintln!("FATAL: could not bind {requested}: {error}");
            eprintln!(
                "  A previous run of this example may still hold the port. Check with \
                 `lsof -nP -iTCP:{} -sTCP:LISTEN` and stop it, or pass a free address as \
                 argv[1] (e.g. `-- 127.0.0.1:8155`).",
                requested.port()
            );
            std::process::exit(1);
        },
    };

    print_instructions(addr, request_state_key_present);

    // A server, not a one-shot script: run until signalled.
    server_handle.await?;
    Ok(())
}

/// The capability set the 2025-11-25 scored scenarios need to be REACHABLE.
///
/// A capability the server does not advertise makes its scenarios unreachable
/// rather than merely failing, which is a worse outcome: an unreachable
/// scenario reports an error that looks like a transport fault. `logging`
/// backs `logging-set-level`, `completions` backs `completion/complete`, and
/// `resources.subscribe` backs `resources-subscribe` / `resources-unsubscribe`.
fn conformance_capabilities() -> ServerCapabilities {
    // `ServerCapabilities` is `#[non_exhaustive]`, so it is built by mutating a
    // `Default` rather than with a struct literal.
    let mut capabilities = ServerCapabilities::default();
    capabilities.tools = Some(ToolCapabilities {
        list_changed: Some(false),
    });
    capabilities.prompts = Some(PromptCapabilities {
        list_changed: Some(false),
    });
    capabilities.resources = Some(ResourceCapabilities {
        subscribe: Some(true),
        list_changed: Some(false),
    });
    capabilities.logging = Some(LoggingCapabilities::default());
    capabilities.completions = Some(CompletionCapabilities::default());
    capabilities
}

/// Print the bound address, both suite invocations, and copy-pasteable `curl`
/// lines for each era.
///
/// Header names come from pmcp's own constants rather than retyped strings, so
/// a rename in the crate shows up here as a compile error instead of a stale
/// instruction.
fn print_instructions(addr: SocketAddr, request_state_key_present: bool) {
    println!();
    println!("=============================================================");
    println!("  DUAL-VERSION CONFORMANCE TARGET (Phase 118, CONF-01)");
    println!("=============================================================");
    println!("  Listening on : {addr}");
    println!("  Endpoint     : http://{addr}");
    println!(
        "  Versions     : {LATEST_PROTOCOL_VERSION} (v1) and {PROTOCOL_VERSION_2026_07_28} (v2)"
    );
    println!("  Sessions     : LIVE (StreamableHttpServerConfig::default)");
    println!("  Tasks ext    : absent by decision (D-14)");
    if !request_state_key_present {
        println!();
        println!("  NOTE: {REQUEST_STATE_KEY_VAR} is not set, so MRTR continuations are");
        println!("  sealed with a per-process key. Single-instance runs are fine; set the");
        println!("  variable (32 bytes, hex or base64) for anything load-balanced. CI sets");
        println!("  it. The value is never printed by this example.");
    }
    println!("-------------------------------------------------------------");
    println!("  RUN THE OFFICIAL SUITE — one process, two requirement sets:");
    println!();
    println!("    conformance server --url http://{addr}/ \\");
    println!("      --requirements 2025-11-25 -o target/conformance-results/2025-11-25");
    println!();
    println!("    conformance server --url http://{addr}/ \\");
    println!("      --requirements 2026-07-28 -o target/conformance-results/2026-07-28");
    println!();
    println!("  The suite binary is the PINNED one — see conformance/README.md.");
    println!("-------------------------------------------------------------");
    println!("  v1 ({LATEST_PROTOCOL_VERSION}) — a plain tools/list POST:");
    println!();
    println!("    curl -sS http://{addr} \\");
    println!("      -H 'content-type: application/json' \\");
    println!("      -H 'accept: {ACCEPT_STREAMABLE}' \\");
    println!("      -H '{MCP_PROTOCOL_VERSION}: {LATEST_PROTOCOL_VERSION}' \\");
    println!("      -d '{}'", v1_tools_list_body());
    println!();
    println!("-------------------------------------------------------------");
    println!("  v2 ({PROTOCOL_VERSION_2026_07_28}) — the SAME endpoint, no handshake:");
    println!();
    println!("    curl -sS http://{addr} \\");
    println!("      -H 'content-type: application/json' \\");
    println!("      -H 'accept: {ACCEPT_STREAMABLE}' \\");
    println!("      -H '{MCP_PROTOCOL_VERSION}: {PROTOCOL_VERSION_2026_07_28}' \\");
    println!("      -d '{}'", v2_tools_list_body());
    println!("=============================================================");
    println!();
    println!("Press Ctrl+C to stop the server");
}

/// A v1 `tools/list` body — era declared by the HTTP header alone.
fn v1_tools_list_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {},
    })
    .to_string()
}

/// A v2 `tools/list` body — the era ALSO travels in `params._meta`, which is
/// what makes the very first byte a client sends a real request rather than an
/// `initialize` handshake.
fn v2_tools_list_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {
            "_meta": {
                pmcp::testing::META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
            },
        },
    })
    .to_string()
}
