//! Example: a `pmcp-agent` CONNECTOR reaching a 2026-07-28 (v2) server, and the
//! SAME connector falling back to 2025-11-25 (v1) when the server does not serve
//! v2.
//!
//! Start the paired SERVER first:
//! ```bash
//! cargo run --example s47_v2_stateless_mrtr --features full
//! ```
//!
//! Then run this agent client with:
//! ```bash
//! cargo run --example s53_v2_agent_client --features full
//! ```
//!
//! It takes the server address as `argv[1]` and defaults to `127.0.0.1:8147`,
//! which is where `s47` binds when it is given no address of its own. This is a
//! one-shot script: it exits 0 when every demonstration behaved as documented,
//! and NON-ZERO otherwise. Every `demo_*` below returns `Err` on a divergence and
//! `main` propagates it with `?`, so this file is an executable assertion rather
//! than a printout.
//!
//! # Which server it pairs with, and why
//!
//! `s47_v2_stateless_mrtr` — chosen over `s50_v2_tasks_server` because its
//! `weather` tool ANSWERS in one round trip when the city is supplied up front,
//! which is exactly what an autonomous connector does. `s50`'s `research` tool
//! returns a task that is ALREADY paused on `input_required`, and delivering
//! those answers needs `tasks/update`, which is deliberately NOT on the
//! `ConnectorClient` seam (see the note on the dropped task-polling demo below).
//! Pairing with `s50` would have produced a demo that polls until its cap and
//! then times out.
//!
//! # What this demonstrates
//!
//! 1. **The v2 happy path.** `UrlConnectorClientFactory::client_for` pins the
//!    2026-07-28 era and confirms it with `server/discover` — zero handshake
//!    bytes, no `initialize`, no `Mcp-Session-Id`. The connector then reports the
//!    version it NEGOTIATED, which this example prints and classifies rather than
//!    assuming.
//! 2. **The v1 fallback.** The same factory, pointed at a v1-only server, gets an
//!    answer that declines the v2 era and falls back — reporting `2025-11-25`
//!    because that is what the server echoed in its `initialize` result, not
//!    because anything here guessed. This direction is where dual-version bugs
//!    hide, so it is a first-class demonstration, not a footnote.
//! 3. **An unreachable host PROPAGATES.** Pointed at a closed loopback port, the
//!    factory returns an error instead of quietly reporting "connected via v1".
//!    A silent downgrade against a host that never answered would be a lie about
//!    the era, and it is the specific failure this demo exists to rule out.
//!
//! # Why there is no task-polling demonstration here
//!
//! Deliberately dropped rather than faked. Neither in-repo v2 server example
//! exposes a tool whose result carries a related task that settles WITHOUT a
//! `tasks/update` round trip: `s47` registers no task store at all, and `s50`'s
//! task pauses on `input_required`. A demo that pretended otherwise would teach
//! the wrong contract.
//!
//! CLNT-03's "including task polling" clause is instead proven by
//! `agent_drives_task_polling_to_terminal_on_v2` in
//! `crates/pmcp-agent/tests/agent_v2_e2e.rs`, which asserts task-id discovery, a
//! SERVER-observed poll count of at least one, and a terminal state — with no
//! conditional and against a harness built to guarantee a non-terminal poll.
//!
//! What this file DOES exercise is the path that would drive such a task:
//! `ClientToolInvoker::dispatch` inspects every result for a related-task
//! envelope and, when it finds one, drives it through
//! `ConnectorClient::wait_for_related_task` under a hard poll cap. There is no
//! poll loop written here, and there must never be one — the loop and its
//! timeout policy live in the SDK primitive, and a second copy would be a second
//! timeout policy.
//!
//! # Numbering
//!
//! `s53` is the next free slot, derived from what is actually on disk rather than
//! from the sequence: `s47` and `s48` are each occupied twice (the Phase-113 v2
//! pair alongside `s47_task_augmented_result` / `s48_durable_poll_decision`),
//! `s49` is occupied twice (`s49_sampling_host` and `s49_v2_subscriptions_client`),
//! and `s50`, `s51` and `s52` all exist. Cargo example NAMES are unique, so the
//! `sNN_` sequence has not been a bijection since Phase 113 — see the note at
//! `Cargo.toml` above the `s47_v2_stateless_mrtr` block.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::protocol::{protocol_era, Era, LATEST_PROTOCOL_VERSION};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};

use pmcp_agent::invoker::{
    ClientToolInvoker, ConnectorClient, ConnectorClientFactory, UrlConnectorClientFactory,
};
use pmcp_agent::seams::{ToolCall, ToolInvoker};

/// Where `s47_v2_stateless_mrtr` binds when it is given no address.
const DEFAULT_ADDR: &str = "127.0.0.1:8147";

/// The tool `s47` exposes.
const V2_TOOL: &str = "weather";

/// The argument `s47`'s tool needs. Supplying it up front is what makes the call
/// a ONE-round-trip answer instead of an elicitation.
const CITY_KEY: &str = "city";

/// The city this example asks about.
const CITY: &str = "Berlin";

/// The tool the in-process v1-only server registers for demonstration 2.
const V1_TOOL: &str = "echo";

/// The hard task-poll cap every `ClientToolInvoker` below is built with.
///
/// The invoker promises this reaches the SDK's task-wait primitive as
/// `WaitForTaskOptions::max_poll_duration_secs`, so a task that never settles
/// cannot hang this process. Nothing here polls today (see the header), but the
/// cap is supplied anyway because that is the only correct way to construct one.
const POLL_CAP_SECS: u64 = 15;

/// The start command this example prints when the paired server is not up.
const START_PAIRED_SERVER: &str = "cargo run --example s47_v2_stateless_mrtr --features full";

/// A trivial tool for the in-process v1 server, so the fallback demonstration can
/// prove the connection WORKS rather than merely that it was established.
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "echoed": args }))
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let endpoint = format!("http://{addr}/");

    println!();
    println!("=============================================================");
    println!("  pmcp-agent CONNECTOR  ->  {endpoint}");
    println!("=============================================================");

    demo_v2_connection(&endpoint).await?;
    demo_v1_fallback().await?;
    demo_unreachable_propagates().await?;

    println!();
    println!("=============================================================");
    println!("  All three demonstrations behaved as documented.");
    println!("=============================================================");
    Ok(())
}

/// 1. The v2 happy path: pin the era, confirm it, discover tools, call one.
async fn demo_v2_connection(endpoint: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[1] v2 (2026-07-28) connection — the paired s47 server");
    println!("-------------------------------------------------------------");

    let factory = UrlConnectorClientFactory::new();
    let connector = factory.client_for(endpoint).await.map_err(|error| {
        format!(
            "could not connect to {endpoint}: {error}\n    \
             Is the paired server running? Start it with:\n      {START_PAIRED_SERVER}"
        )
    })?;

    let negotiated = report_era(connector.as_ref(), Era::V2)?;
    println!("    negotiated    : {negotiated} (classified as the v2 era)");
    println!("    handshake     : none — server/discover was the first request");

    // The AGENT-side surface, not a raw client: the invoker is what an agent
    // loop holds, and it is what would drive a related task to terminal.
    let invoker = ClientToolInvoker::new(Arc::clone(&connector), POLL_CAP_SECS);

    let tools = invoker.list_tools().await;
    if tools.is_empty() {
        return Err(
            format!("the v2 server advertised no tools; expected at least {V2_TOOL}").into(),
        );
    }
    println!("    tools/list    : {}", tool_names(&tools));

    let outcome = invoker
        .invoke(ToolCall {
            id: "demo-1".to_string(),
            name: V2_TOOL.to_string(),
            arguments: json!({ CITY_KEY: CITY }),
            connector: None,
        })
        .await;
    if outcome.is_error {
        return Err(format!(
            "calling {V2_TOOL} over v2 failed: {}",
            outcome.error.unwrap_or_default()
        )
        .into());
    }
    println!("    tools/call    : {}", outcome.content);
    Ok(())
}

/// 2. The v1 fallback: the SAME factory against a server that does not serve v2.
///
/// The server runs IN THIS PROCESS on an ephemeral loopback port, so the
/// demonstration needs no third terminal. Its accept-list is the pmcp DEFAULT,
/// which is v1-only — that is the whole configuration difference.
async fn demo_v1_fallback() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[2] v1 (2025-11-25) fallback — an in-process v1-only server");
    println!("-------------------------------------------------------------");

    let server = Server::builder()
        .name("s53-v1-only")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        // No `.with_supported_protocol_versions(..)`: the default accept-list
        // carries v1 ONLY, so a 2026-07-28 request is answered with a protocol
        // error. The server ANSWERED, which is what makes this an era rejection
        // rather than an infrastructure failure.
        .tool(V1_TOOL, EchoTool)
        .build()?;

    let http = StreamableHttpServer::with_config(
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0),
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );
    let (bound, handle) = http.start().await?;
    let endpoint = format!("http://{bound}/");
    println!("    v1-only server: {endpoint}");

    let outcome = run_v1_fallback(&endpoint).await;

    // Teardown order: sockets die before the server task, then abort, then await.
    handle.abort();
    let _ = handle.await;
    outcome
}

/// The body of demonstration 2, factored out so the server is torn down on every
/// path — including the failing one.
async fn run_v1_fallback(endpoint: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let factory = UrlConnectorClientFactory::new();
    let connector = factory
        .client_for(endpoint)
        .await
        .map_err(|error| format!("the v1 fallback did not connect to {endpoint}: {error}"))?;

    let negotiated = report_era(connector.as_ref(), Era::V1)?;
    println!("    negotiated    : {negotiated} (classified as the v1 era)");
    if negotiated != LATEST_PROTOCOL_VERSION {
        return Err(format!(
            "the fallback must report the version the server ECHOED in its \
             initialize result ({LATEST_PROTOCOL_VERSION}), not {negotiated}"
        )
        .into());
    }
    println!("    fallback rule : the endpoint ANSWERED, so v2 rejection => try v1");

    let invoker = ClientToolInvoker::new(Arc::clone(&connector), POLL_CAP_SECS);
    let outcome = invoker
        .invoke(ToolCall {
            id: "demo-2".to_string(),
            name: V1_TOOL.to_string(),
            arguments: json!({ "message": "hello from the fallback" }),
            connector: None,
        })
        .await;
    if outcome.is_error {
        return Err(format!(
            "calling {V1_TOOL} over the v1 fallback failed: {}",
            outcome.error.unwrap_or_default()
        )
        .into());
    }
    println!("    tools/call    : {}", outcome.content);
    Ok(())
}

/// 3. An unreachable host is INFRASTRUCTURE, and must propagate.
///
/// The endpoint is a loopback port that was bound and then released, so nothing
/// is listening and the connect is refused immediately — deterministic, and
/// without touching the network.
async fn demo_unreachable_propagates() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[3] Unreachable host — the error PROPAGATES, no silent downgrade");
    println!("-------------------------------------------------------------");

    let endpoint = closed_loopback_endpoint()?;
    println!("    closed port   : {endpoint}");

    let factory = UrlConnectorClientFactory::new();
    match factory.client_for(&endpoint).await {
        Ok(connector) => Err(format!(
            "a host that never answered must NOT yield a connector; got one \
             reporting era {}",
            connector.negotiated_protocol_version().unwrap_or("<none>")
        )
        .into()),
        Err(error) => {
            println!("    factory says  : {error}");
            println!("    no v1 attempt was made — nothing answered, so there was");
            println!("    no protocol signal to fall back on.");
            Ok(())
        },
    }
}

/// Read the connector's NEGOTIATED version and require it to classify as `want`.
///
/// The version is reported by the connection rather than assumed by the caller,
/// and it is classified with `protocol_era` rather than compared as a string —
/// that classifier's conservative unknown-to-v1 fallback gives the right answer
/// for any value, including one a future server invents.
fn report_era(
    connector: &dyn ConnectorClient,
    want: Era,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let Some(negotiated) = connector.negotiated_protocol_version() else {
        return Err("the connector reported no negotiated protocol version".into());
    };
    let era = protocol_era(negotiated);
    if era != want {
        return Err(
            format!("expected the {want:?} era, but {negotiated} classifies as {era:?}").into(),
        );
    }
    Ok(negotiated.to_string())
}

/// A comma-separated list of advertised tool names, for the transcript.
fn tool_names(tools: &[pmcp::types::ToolInfo]) -> String {
    tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// An endpoint that is GUARANTEED unreachable, without touching the network.
///
/// Binds an ephemeral loopback port, captures the OS-assigned address, then DROPS
/// the listener. Nothing is listening afterwards, so a connect attempt is refused
/// immediately and deterministically.
fn closed_loopback_endpoint() -> std::result::Result<String, Box<dyn std::error::Error>> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(format!("http://{addr}/"))
}
