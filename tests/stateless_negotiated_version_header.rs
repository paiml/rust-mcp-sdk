#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]
//! A STATELESS server keeps echoing the version it negotiated, on every request
//! after `initialize` — proven by RUNNING one over a real socket.
//!
//! # The defect this file closes
//!
//! `tests/v2_initialize_negotiated_version_header.rs` closed one half of this
//! and documented the mechanism precisely:
//!
//! ```text
//! compute_outbound_protocol_version(.., is_init_request = false, None)
//!                               -> crate::DEFAULT_PROTOCOL_VERSION == "2025-03-26"
//! ```
//!
//! It fixed the `is_init_request` classification. The FALLBACK it names on the
//! very next line was left standing, and it is reachable by an entirely
//! different route that has nothing to do with feature severance:
//!
//! ```text
//! is_init_request = false                  <- every request after the handshake
//! response_session_id = None               <- a STATELESS server mints no session
//! -> session_protocol_version() unreachable
//! -> crate::DEFAULT_PROTOCOL_VERSION == "2025-03-26"
//! ```
//!
//! The non-initialize branch can only recover the negotiated version by looking
//! it up in a session. `StreamableHttpServerConfig::stateless()` — the
//! serverless/Lambda constructor — has none, so EVERY response after the
//! handshake advertised `2025-03-26` while the handshake itself had negotiated
//! `2025-11-25`. `StreamableHttpTransport` latches that header
//! (`src/shared/streamable_http.rs`) and replays it on every subsequent
//! request, so the client is dragged down with the server: a silent protocol
//! DOWNGRADE decided by nothing but whether the deployment happened to be
//! serverless.
//!
//! # Why the existing suites could not catch it
//!
//! The sibling file runs on a severed `full-v2` build and sends `initialize` —
//! the one request whose branch was already fixed. `tests/v2_stateless_http.rs`
//! is explicit in its own header that it spawns the STATEFUL default config on
//! purpose, so the `stateless()` constructor had no live-HTTP coverage of its
//! response headers at all. Between them: strong about the init branch, strong
//! about the per-request era gate, silent about the fallback that serves every
//! other request of every serverless deployment.
//!
//! # What this file pins
//!
//! The version a server ADVERTISES after the handshake must equal the one it
//! NEGOTIATED during it, in both session configurations. Stated as an equality
//! between two observed responses rather than against a literal, so it stays
//! true when the negotiation table changes — the same formulation, and for the
//! same reason, as the sibling file.

mod common;

use std::net::SocketAddr;

use common::v2::{header, post, spawn_with, teardown, Resp, V1};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::server::Server;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::Content;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

struct EchoTool;

#[async_trait::async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "content": [Content::Text { text: args.to_string() }] }))
    }
}

/// A v1-only server: the accept-list carries [`V1`] and nothing else, because
/// the downgrade under test is a v1 fallback and an opted-in v2 request has its
/// header set by `apply_v2_outbound_headers` instead.
fn v1_server() -> Server {
    Server::builder()
        .name("stateless-version-fixture")
        .version("1.0.0")
        .tool("echo", EchoTool)
        .with_supported_protocol_versions([ProtocolVersion(V1.to_string())])
        .build()
        .expect("server builds")
}

/// Handshake, then one ordinary request — the smallest sequence that can
/// observe the fallback, and exactly what every client does.
async fn handshake_then_list(config: StreamableHttpServerConfig) -> (Resp, Resp, JoinHandle<()>) {
    let (addr, handle) = spawn_with(v1_server(), config).await;
    let init = initialize(addr).await;
    let session = init.mcp_session_id.clone();

    // The client asserts the version it just negotiated, which is what
    // `StreamableHttpTransport` does on every post-handshake request.
    let mut extra = vec![header("mcp-protocol-version", V1)];
    if let Some(sid) = &session {
        extra.push(header("mcp-session-id", sid));
    }
    let list = post(
        addr,
        &extra,
        &json!({"jsonrpc":"2.0","id":"2","method":"tools/list","params":{}}).to_string(),
    )
    .await;
    (init, list, handle)
}

async fn initialize(addr: SocketAddr) -> Resp {
    post(
        addr,
        &[],
        &json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "initialize",
            "params": {
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "fixture", "version": "1.0.0" },
            }
        })
        .to_string(),
    )
    .await
}

/// THE regression. A stateless server has no session to recover the negotiated
/// version from, and must fall back to what the client asserted on the request
/// — never to `DEFAULT_PROTOCOL_VERSION`.
#[tokio::test(flavor = "multi_thread")]
async fn a_stateless_server_advertises_the_version_it_negotiated() {
    let (init, list, handle) = handshake_then_list(StreamableHttpServerConfig::stateless()).await;

    assert_eq!(
        init.mcp_session_id, None,
        "the `stateless()` constructor must mint no session — if this fails the \
         fixture is not exercising the stateless fallback at all and every \
         assertion below is vacuous"
    );
    assert_eq!(
        init.mcp_version.as_deref(),
        Some(V1),
        "the handshake negotiated {V1}"
    );
    assert_eq!(
        list.mcp_version, init.mcp_version,
        "a request after the handshake must advertise the SAME version the \
         handshake negotiated. A `2025-03-26` here is the \
         `compute_outbound_protocol_version` fallback firing because a \
         stateless server has no session to look the version up in — the \
         client latches this header and replays it, so the server has \
         downgraded the whole connection"
    );

    teardown(handle, ()).await;
}

/// The stateful twin, which recovers the version from its session and was never
/// broken. Present so a future change cannot fix one configuration by breaking
/// the other.
///
/// `v1-compat`-gated because the whole point of this test is that a session IS
/// minted: a `--no-default-features --features full-v2` build mints none, so on
/// that build its first assertion fires and reports a v1 defect against a
/// configuration that deliberately has no v1. Its stateless sibling above needs
/// no gate — it asserts what the header says, not that a session exists.
#[cfg(feature = "v1-compat")]
#[tokio::test(flavor = "multi_thread")]
async fn a_stateful_server_advertises_the_version_it_negotiated() {
    let (init, list, handle) = handshake_then_list(StreamableHttpServerConfig::default()).await;

    assert!(
        init.mcp_session_id.is_some(),
        "the default config mints a session; without one this test is the \
         stateless case wearing the wrong name"
    );
    assert_eq!(init.mcp_version.as_deref(), Some(V1));
    assert_eq!(
        list.mcp_version, init.mcp_version,
        "the stateful path recovers the negotiated version from its session"
    );

    teardown(handle, ()).await;
}

/// The echo must never become a reflection of arbitrary client bytes.
///
/// `validate_protocol_version_supported` already answers `400` to a version
/// outside `SUPPORTED_PROTOCOL_VERSIONS`, which is what makes echoing the
/// asserted header safe. Pinned here because that guard is now load-bearing for
/// a response header rather than only for request admission — every outbound
/// value stays drawn from the SDK's own table.
#[tokio::test(flavor = "multi_thread")]
async fn an_unsupported_asserted_version_is_refused_rather_than_echoed() {
    let (addr, handle) = spawn_with(v1_server(), StreamableHttpServerConfig::stateless()).await;
    let _ = initialize(addr).await;

    let list = post(
        addr,
        &[header("mcp-protocol-version", "1999-01-01")],
        &json!({"jsonrpc":"2.0","id":"2","method":"tools/list","params":{}}).to_string(),
    )
    .await;

    assert_eq!(list.status, 400, "an unsupported version is refused");
    assert_ne!(
        list.mcp_version.as_deref(),
        Some("1999-01-01"),
        "an unsupported version must never be reflected into a response header"
    );

    teardown(handle, ()).await;
}
