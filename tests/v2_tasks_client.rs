//! The CLIENT half of v2 tasks negotiation (Phase 114, plan 06 — D-04 / DQ4).
//!
//! Four properties, measured rather than asserted about:
//!
//! 1. **Declaration emission** — a client built with
//!    `ClientBuilder::with_tasks_extension()` carries
//!    `_meta["io.modelcontextprotocol/clientCapabilities"].extensions["io.modelcontextprotocol/tasks"]`
//!    = `{}` on EVERY request; one that did not opt in carries no `extensions`
//!    key at all.
//! 2. **Fail fast** — a `tasks/*` call against a server whose stored
//!    `server/discover` projection lacks the extension is refused LOCALLY, with
//!    a typed error naming the key and ZERO bytes on the wire.
//! 3. **Header emission** — `tasks/get` / `tasks/update` / `tasks/cancel` set
//!    `Mcp-Name` to `params.taskId`; every other method keeps the empty value.
//! 4. **Table decoupling** — those same three methods are NOT MRTR-eligible.
//!
//! # Why a raw-TCP capture server rather than a real `pmcp` server
//!
//! Properties 1 and 3 are about the bytes and headers the CLIENT emits. Pointing
//! the real `StreamableHttpTransport` at a socket that records the request head
//! and body verbatim measures exactly that, with no server-side gate able to
//! reject the request before it has been recorded — and `tasks/*` is not yet
//! routed on the v2 wire (TASK-03), so a real server would answer `-32601` and
//! the interesting request would never be observed.

#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::ServerCapabilities;
use pmcp::{Client, ClientBuilder};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use url::Url;

// ===========================================================================
// The raw-TCP capture server
// ===========================================================================

/// One recorded HTTP request: its header map (lowercased names) and its body.
#[derive(Debug, Clone)]
struct Captured {
    headers: HashMap<String, String>,
    body: String,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn body_json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!("captured body is not JSON ({e}): {}", self.body);
        })
    }

    /// The `ClientCapabilities` the request declared, as raw JSON.
    ///
    /// Read as raw JSON on purpose: the assertions below are about key
    /// PRESENCE and ABSENCE on the wire, and deserializing into the typed
    /// struct first would erase the very distinction (`skip_serializing_if`
    /// means an absent key and a `None` field are indistinguishable once
    /// parsed).
    fn declared_capabilities(&self) -> Value {
        self.body_json()["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"].clone()
    }
}

/// Spawn a socket that records every request and answers a JSON-RPC `-32601`
/// echoing the request's id.
///
/// The canned answer is an ERROR on purpose. These tests assert on what the
/// client SENT, so the reply only has to unblock the caller — and `-32601` is
/// what a v2 server that does not route `tasks/*` genuinely answers today
/// (TASK-03). Every caller therefore ignores the `Result` of the client call.
async fn capture_server() -> (SocketAddr, Arc<Mutex<Vec<Captured>>>, JoinHandle<()>) {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("bind an ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let captured: Arc<Mutex<Vec<Captured>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = captured.clone();

    let handle = tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let sink = sink.clone();
            tokio::spawn(serve_one(socket, sink));
        }
    });

    (addr, captured, handle)
}

/// Record one request off `socket`, then answer it.
///
/// Split out of [`capture_server`] so neither function carries the accept loop
/// AND the framing loop: together they exceed the repository's cognitive
/// complexity cap of 25.
async fn serve_one(mut socket: tokio::net::TcpStream, sink: Arc<Mutex<Vec<Captured>>>) {
    let Some((head, body)) = read_one_request(&mut socket).await else {
        return;
    };
    let reply = method_not_found_reply(&body);
    sink.lock().expect("capture sink").push(Captured {
        headers: parse_headers(&head),
        body,
    });
    let _ = socket.write_all(reply.as_bytes()).await;
    let _ = socket.shutdown().await;
}

/// Read one HTTP request as `(head, body)`, framing on `Content-Length`.
///
/// `None` when the peer closed before a complete head arrived.
async fn read_one_request(socket: &mut tokio::net::TcpStream) -> Option<(String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (head_len, content_length) = loop {
        if let Some(split) = find_head_end(&buffer) {
            break (split, content_length_of(&buffer[..split]));
        }
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    };
    while buffer.len() < head_len + content_length {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buffer[..head_len]).to_string();
    let body =
        String::from_utf8_lossy(&buffer[head_len..(head_len + content_length).min(buffer.len())])
            .to_string();
    Some((head, body))
}

/// A complete HTTP `200` carrying a JSON-RPC `-32601` for `request_body`'s id.
///
/// Echoing the id is what lets the client's in-flight request resolve promptly
/// instead of waiting on a receive that never completes — the tests then measure
/// the emitted bytes rather than a timeout.
fn method_not_found_reply(request_body: &str) -> String {
    let id = serde_json::from_str::<Value>(request_body)
        .ok()
        .and_then(|frame| frame.get("id").cloned())
        .unwrap_or(Value::Null);
    let payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": "capture server routes nothing" },
    })
    .to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{payload}",
        payload.len()
    )
}

/// Byte offset just past the `\r\n\r\n` that ends the request head.
fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|start| start + 4)
}

fn parse_headers(head: &str) -> HashMap<String, String> {
    head.lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect()
}

fn content_length_of(head: &[u8]) -> usize {
    parse_headers(&String::from_utf8_lossy(head))
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// A real v2 `StreamableHttpTransport` pointed at `addr`.
fn v2_transport(addr: SocketAddr) -> StreamableHttpTransport {
    // Builder, not a struct literal: `session_id` / `on_resumption_token` are
    // `v1-compat`-only, so a literal naming them breaks the `full-v2` build.
    StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(
            Url::parse(&format!("http://{addr}")).expect("a loopback URL"),
        )
        .enable_json_response()
        .build(),
    )
}

fn v2_client(addr: SocketAddr, declare_tasks: bool) -> Client<StreamableHttpTransport> {
    let builder = ClientBuilder::new(v2_transport(addr))
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        .expect("2026-07-28 is selectable");
    if declare_tasks {
        builder.with_tasks_extension().build()
    } else {
        builder.build()
    }
}

/// Wait for `count` captured requests, or fail with what actually arrived.
async fn wait_for(captured: &Arc<Mutex<Vec<Captured>>>, count: usize) -> Vec<Captured> {
    for _ in 0..200 {
        {
            let seen = captured.lock().expect("capture sink");
            if seen.len() >= count {
                return seen.clone();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let seen = captured.lock().expect("capture sink").len();
    panic!("expected {count} captured request(s), saw {seen}");
}

// ===========================================================================
// 1. Declaration emission (D-04 / TASK-01)
// ===========================================================================

/// The declaration rides on EVERY request, because v2 has no handshake to
/// carry it once.
///
/// Two different methods are driven, so a mechanism that stamped only the first
/// request (or only `tasks/*`) still fails.
#[tokio::test]
async fn a_declaring_client_carries_the_extension_on_every_request() {
    let (addr, captured, handle) = capture_server().await;
    let client = v2_client(addr, true);

    let _ = client.tasks_get("task-1").await;
    let _ = client.list_tools(None).await;

    let seen = wait_for(&captured, 2).await;
    for request in &seen {
        assert_eq!(
            request.declared_capabilities()["extensions"][TASKS_EXTENSION_KEY],
            json!({}),
            "every v2 request must declare the tasks extension as EXACTLY {{}}; \
             got {}",
            request.body
        );
    }
    handle.abort();
}

/// Absence is asserted as key ABSENCE on the RAW wire bytes.
///
/// `ClientCapabilities::extensions` carries `skip_serializing_if`, so a
/// regression that emitted `"extensions": null` would satisfy any check written
/// against the value.
#[tokio::test]
async fn a_non_declaring_client_carries_no_extensions_key_at_all() {
    let (addr, captured, handle) = capture_server().await;
    let client = v2_client(addr, false);

    let _ = client.tasks_get("task-1").await;

    let seen = wait_for(&captured, 1).await;
    let capabilities = seen[0].declared_capabilities();
    assert!(
        capabilities.get("extensions").is_none(),
        "a client that never opted in must emit no extensions key, got {capabilities}"
    );
    // Non-vacuity: the `_meta` block itself IS present, so the assertion above
    // is about the extensions key and not about a missing envelope.
    assert_eq!(
        seen[0].body_json()["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
        PROTOCOL_VERSION_2026_07_28
    );
    handle.abort();
}

// ===========================================================================
// 2. Fail fast, with zero bytes on the wire (D-04)
// ===========================================================================

/// Drive a real `server/discover` round trip so `server_capabilities` is stored
/// the way production stores it, then assert the refusal.
///
/// Building the client through `server_discover` rather than reaching into the
/// field is what makes this test cover the SEAM: it is the projection a v2
/// server actually returns that has to satisfy — or fail to satisfy — the
/// capability check.
async fn client_with_discovered_capabilities(
    capabilities: ServerCapabilities,
) -> (Client<DiscoverTransport>, Arc<Mutex<usize>>) {
    let transport = DiscoverTransport::new(capabilities);
    let sends = transport.sends.clone();
    let mut client = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
        .expect("2026-07-28 is selectable")
        .with_tasks_extension()
        .build();
    client
        .server_discover()
        .await
        .expect("the canned discover result parses");
    *sends.lock().expect("send counter") = 0;
    (client, sends)
}

/// A transport that answers exactly one `server/discover` with a canned
/// projection, then records every further send without answering it.
#[derive(Debug, Clone)]
struct DiscoverTransport {
    sends: Arc<Mutex<usize>>,
    pending: Arc<Mutex<Option<pmcp::types::TransportMessage>>>,
    capabilities: Arc<ServerCapabilities>,
}

impl DiscoverTransport {
    fn new(capabilities: ServerCapabilities) -> Self {
        Self {
            sends: Arc::new(Mutex::new(0)),
            pending: Arc::new(Mutex::new(None)),
            capabilities: Arc::new(capabilities),
        }
    }
}

#[async_trait::async_trait]
impl pmcp::shared::Transport for DiscoverTransport {
    async fn send(&mut self, _message: pmcp::types::TransportMessage) -> pmcp::Result<()> {
        *self.sends.lock().expect("send counter") += 1;
        Ok(())
    }

    async fn receive(&mut self) -> pmcp::Result<pmcp::types::TransportMessage> {
        self.pending
            .lock()
            .expect("pending response")
            .take()
            .ok_or_else(|| pmcp::Error::protocol_msg("no responses"))
    }

    async fn close(&mut self) -> pmcp::Result<()> {
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "discover"
    }

    fn supports_negotiated_protocol_version(&self) -> bool {
        true
    }

    async fn send_raw(&mut self, body: Vec<u8>) -> pmcp::Result<()> {
        *self.sends.lock().expect("send counter") += 1;
        let frame: Value = serde_json::from_slice(&body).expect("the client sends JSON");
        if frame["method"] == "server/discover" {
            let result = json!({
                "protocolVersion": PROTOCOL_VERSION_2026_07_28,
                "capabilities": serde_json::to_value(&*self.capabilities).expect("serializes"),
                "serverInfo": { "name": "capture", "version": "0.0.0" },
            });
            let id = pmcp::types::RequestId::from(
                frame["id"].as_str().expect("the client mints string ids"),
            );
            *self.pending.lock().expect("pending response") =
                Some(pmcp::types::TransportMessage::Response(
                    pmcp::types::JSONRPCResponse::success(id, result),
                ));
        }
        Ok(())
    }
}

/// A projection advertising tasks the v2 way.
///
/// Built by field assignment rather than a struct expression: `ServerCapabilities`
/// is `#[non_exhaustive]`, so an out-of-crate struct literal does not compile.
fn tasks_extension_capabilities() -> ServerCapabilities {
    let mut extensions = HashMap::new();
    extensions.insert(TASKS_EXTENSION_KEY.to_string(), json!({}));
    let mut capabilities = ServerCapabilities::default();
    capabilities.extensions = Some(extensions);
    capabilities
}

#[tokio::test]
async fn an_un_negotiated_tasks_call_is_refused_before_the_round_trip() {
    let (client, sends) = client_with_discovered_capabilities(ServerCapabilities::default()).await;

    let error = client
        .tasks_get("task-1")
        .await
        .expect_err("a server that never advertised the extension must be refused");
    let rendered = error.to_string();
    assert!(
        rendered.contains(TASKS_EXTENSION_KEY),
        "the refusal must name the extension key so the fix is discoverable: {rendered}"
    );
    assert_eq!(
        *sends.lock().expect("send counter"),
        0,
        "the refusal must precede the round trip — zero bytes on the wire"
    );
}

/// The non-vacuity half: the SAME call succeeds past the capability gate once
/// the projection carries the extension.
///
/// Without this, a `tasks_get` that failed for any other reason would satisfy
/// the test above.
#[tokio::test]
async fn a_negotiated_tasks_call_passes_the_gate_and_reaches_the_transport() {
    let (client, sends) = client_with_discovered_capabilities(tasks_extension_capabilities()).await;

    // The canned transport answers nothing after `server/discover`, so the call
    // still errors — but it errors at RECEIVE, having sent its request.
    let error = client
        .tasks_get("task-1")
        .await
        .expect_err("nothing answers");
    assert!(
        !error.to_string().contains(TASKS_EXTENSION_KEY),
        "a negotiated call must not be refused by the capability gate: {error}"
    );
    assert_eq!(
        *sends.lock().expect("send counter"),
        1,
        "a negotiated tasks call must reach the transport"
    );
}

// ===========================================================================
// 3. Routing header emission (DQ4)
// ===========================================================================

#[tokio::test]
async fn tasks_get_sets_mcp_name_to_the_task_id() {
    let (addr, captured, handle) = capture_server().await;
    let client = v2_client(addr, true);

    let _ = client.tasks_get("abc").await;

    let seen = wait_for(&captured, 1).await;
    assert_eq!(seen[0].header("mcp-method"), Some("tasks/get"));
    assert_eq!(
        seen[0].header("mcp-name"),
        Some("abc"),
        "the spec requires Mcp-Name = params.taskId so an intermediary can route \
         to the instance holding the task state"
    );
    // Header and body are derived from the SAME bytes (T-113-08 / T-114-20):
    // whatever the header says, the body says.
    assert_eq!(seen[0].body_json()["params"]["taskId"], "abc");
    handle.abort();
}

#[tokio::test]
async fn tasks_cancel_sets_mcp_name_to_the_task_id() {
    let (addr, captured, handle) = capture_server().await;
    let client = v2_client(addr, true);

    let _ = client.tasks_cancel("abc").await;

    let seen = wait_for(&captured, 1).await;
    assert_eq!(seen[0].header("mcp-method"), Some("tasks/cancel"));
    assert_eq!(seen[0].header("mcp-name"), Some("abc"));
    handle.abort();
}

/// The negative control for the two above: a method NOT in the tasks routing
/// table keeps the empty value every name-less method emits.
///
/// Fired against `tasks/list` specifically, because "every `tasks/*` method is
/// name-bearing" is the plausible wrong implementation, and only this
/// distinguishes it.
///
/// # Why this drives the TRANSPORT and not `Client::tasks_list` (Phase 114-19)
///
/// It used to call `client.tasks_list(None)`. Plan 114-19 then made
/// `tasks/list` — RETIRED on `2026-07-28` — fail fast LOCALLY on a v2 client
/// with ZERO bytes on the wire, which is a property that file asserts with a
/// send counter. The two are not in conflict; the VEHICLE was. A v2 pmcp client
/// can no longer put a `tasks/list` on the wire at all, so the only honest way
/// to keep observing the header derivation for a non-name-bearing `tasks/*`
/// method is to hand the frame to the layer that OWNS that derivation.
///
/// That is also the more precise test: `Mcp-Name` is derived by the transport
/// from the frame bytes (T-113-08), so this measures the derivation directly
/// rather than through a client method that may or may not still call it.
///
/// The TABLE-level half of the same fact already lives below in
/// `tasks_list_and_tasks_result_carry_no_routing_name`; it is deliberately NOT
/// restated here.
#[tokio::test]
async fn tasks_list_emits_an_empty_mcp_name() {
    use pmcp::shared::Transport;

    let (addr, captured, handle) = capture_server().await;
    let mut transport = v2_transport(addr);
    transport.set_negotiated_protocol_version(Some(PROTOCOL_VERSION_2026_07_28.to_string()));

    let frame = json!({
        "jsonrpc": "2.0",
        "id": "name-control-1",
        "method": "tasks/list",
        "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION_2026_07_28 } },
    });
    let _ = transport
        .send_raw(serde_json::to_vec(&frame).expect("serializes"))
        .await;

    let seen = wait_for(&captured, 1).await;
    assert_eq!(seen[0].header("mcp-method"), Some("tasks/list"));
    assert_eq!(
        seen[0].header("mcp-name"),
        Some(""),
        "tasks/list is deliberately absent from the tasks routing table"
    );
    handle.abort();
}

// ===========================================================================
// 4. The two tables stay decoupled (T-114-21)
// ===========================================================================

/// This test replaces a protection the design structurally loses.
///
/// Before Phase 114 there was exactly ONE method table, so "name-bearing" and
/// "MRTR-eligible" could not disagree. Splitting them buys `tasks/update` its
/// payload back — `splice_mrtr_params` strips `inputResponses` from an eligible
/// method unconditionally, and `inputResponses` IS the whole `tasks/update`
/// body — at the cost of the two properties now being separately settable. This
/// is the assertion that keeps them from being merged back.
#[test]
fn tasks_methods_are_name_bearing_and_never_mrtr_eligible() {
    for method in ["tasks/get", "tasks/update", "tasks/cancel"] {
        assert_eq!(
            pmcp::testing::routing_name_key(method),
            Some("taskId"),
            "{method} must be name-bearing"
        );
        assert!(
            !pmcp::testing::method_is_mrtr_eligible(method),
            "{method} must NOT be MRTR-eligible — eligibility would make \
             splice_mrtr_params delete its inputResponses payload"
        );
    }
}

/// MRTR eligibility is unchanged at exactly the three Phase-113 methods.
#[test]
fn mrtr_eligibility_is_unchanged_by_the_tasks_routing_table() {
    for method in ["tools/call", "prompts/get", "resources/read"] {
        assert!(pmcp::testing::method_is_mrtr_eligible(method));
    }
    for method in [
        "tasks/get",
        "tasks/update",
        "tasks/cancel",
        "tasks/list",
        "tasks/result",
        "tools/list",
        "server/discover",
    ] {
        assert!(
            !pmcp::testing::method_is_mrtr_eligible(method),
            "{method} must not be MRTR-eligible"
        );
    }
}

/// `tasks/list` and `tasks/result` are outside the routing table too.
#[test]
fn tasks_list_and_tasks_result_carry_no_routing_name() {
    assert_eq!(pmcp::testing::routing_name_key("tasks/list"), None);
    assert_eq!(pmcp::testing::routing_name_key("tasks/result"), None);
}
