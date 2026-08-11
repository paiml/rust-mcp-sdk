//! The HTTP twin of `tests/in_tool_peer_roundtrip.rs` — CONF-07 / G-3.
//!
//! `tests/in_tool_peer_roundtrip.rs` proves the peer METHODS (`sample`,
//! `list_roots`, `elicit`) work on the stock `Server::run` loop. This file asks
//! the other half of the question: does the **`StreamableHTTP` transport** carry a
//! server-to-client channel at all? The two files are deliberately separate so a
//! failure here is attributable to the TRANSPORT and never to the method.
//!
//! # The one test that matters most in this phase
//!
//! [`response_post_is_answered_while_a_tool_handler_holds_the_server_mutex`] is
//! the phase's most important test, and it is first in the file for that reason.
//! `dispatch_public_request` holds `state.server.lock().await` for the ENTIRE
//! duration of a tool handler, and every inbound POST passes through
//! `resolve_v2_gate` (two lock sites inside `run_v2_header_gate`) and
//! `extract_and_validate_auth` (a third) before it is classified for dispatch.
//! So a tool handler parked awaiting `peer.sample()` blocks the very POST that
//! carries the client's answer: a guaranteed deadlock, and a denial-of-service
//! control — ONE slow tool call otherwise takes the whole transport offline.
//!
//! That guard does not need a peer to be measurable. Any parked handler holds
//! the mutex, so the guard parks one deliberately and then times an inbound
//! JSON-RPC response POST against a bound far shorter than the handler's hold.
//! It goes RED on the unfixed transport and GREEN once the inbound response is
//! classified and routed BEFORE the three lock sites.
//!
//! # Why the peer round trips are RED at the end of plan 10
//!
//! Plan 10 builds the channel — dispatcher ownership on `ServerState`, the
//! outbound drain, correlation-id ownership, the `TransportBackchannel` carrier
//! and the inbound classification. Plan 11 performs the INJECTION that puts a
//! `SessionPeerHandle` on `RequestHandlerExtra`. Until then `extra.peer()` is
//! `None` on this transport and every round trip below fails with
//! [`PEER_ABSENT`] rather than hanging. That distinction is the signal plan 11
//! reads: "peer absent" means the transport no longer deadlocks and only the
//! injection is missing; a HANG would mean the mutex is still on the inbound
//! path.
//!
//! # Reliability doctrine
//!
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning — no fixed sleep as a synchronization
//! device), SHUTDOWN (sockets dropped, then `abort()`, then `await`). EVERY
//! await that crosses the wire is wrapped in `tokio::time::timeout`: on a
//! deadlock an unbounded await does not FAIL, it hangs, and a hung test reads as
//! a slow test in CI rather than as a red one.

#![cfg(all(
    feature = "streamable-http",
    feature = "v1-compat",
    not(target_arch = "wasm32")
))]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use pmcp::types::sampling::{CreateMessageParams, SamplingMessage, SamplingMessageContent};
use pmcp::types::Role;
use pmcp::{RequestHandlerExtra, Server, ToolHandler};

// ===========================================================================
// Bounds. Every one of them is an upper bound on a wire operation, never a
// synchronization device.
// ===========================================================================

/// Upper bound on any single client operation that crosses the wire.
const BOUND: Duration = Duration::from_secs(5);

/// Upper bound on how long the mutex-holding tool keeps `Mutex<Server>`.
///
/// A CEILING, not a schedule: the handler parks on [`HoldGate::release`] and the
/// guard releases it explicitly, so this only bounds the damage if the guard
/// panics before it gets there. There is no fixed sleep anywhere in this file —
/// every wait is either an explicit signal or a bounded read.
const HOLD: Duration = Duration::from_secs(10);

/// The bound the inbound response POST must beat while the mutex is held.
///
/// Measured only AFTER [`HoldGate::entered`] has fired, so the mutex is provably
/// held for the whole measurement rather than probably held after a guessed
/// settle.
const MUTEX_BOUND: Duration = Duration::from_secs(1);

/// How long a stream is watched to prove NOTHING is delivered to it.
const QUIET: Duration = Duration::from_millis(600);

/// What a tool reports when the HTTP transport carries no peer.
///
/// The RED marker for every round trip in this file while plan 11's injection is
/// outstanding. It is a returned `Err`, never a panic: a panicking handler would
/// take down the axum worker and turn an informative red into noise.
const PEER_ABSENT: &str = "peer absent on this transport";

/// The model the canned sampling answer reports back.
const HOST_MODEL: &str = "host-model";

// ===========================================================================
// Tools.
// ===========================================================================

/// Tool that awaits `extra.peer().sample()` and echoes the model name.
struct SamplerTool;

#[async_trait]
impl ToolHandler for SamplerTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = peer_or_absent(&extra)?;
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "summarize".to_string(),
                meta: None,
            },
        )]);
        let result = peer.sample(params).await?;
        Ok(json!(format!("sampled:{}", result.model)))
    }
}

/// Tool that awaits `extra.peer().list_roots()` and echoes the root count.
struct RootsTool;

#[async_trait]
impl ToolHandler for RootsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = peer_or_absent(&extra)?;
        let roots = peer.list_roots().await?;
        Ok(json!(format!("roots:{}", roots.roots.len())))
    }
}

/// Tool that awaits `extra.peer().elicit()` and echoes the action plus field.
struct ElicitTool;

#[async_trait]
impl ToolHandler for ElicitTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = peer_or_absent(&extra)?;
        let answer = peer
            .elicit(ElicitRequestParams::Form {
                message: "which environment?".to_string(),
                requested_schema: json!({
                    "type": "object",
                    "properties": { "env": { "type": "string" } },
                    "required": ["env"],
                }),
            })
            .await?;
        let env = answer
            .content
            .as_ref()
            .and_then(|c| c.get("env"))
            .and_then(Value::as_str)
            .unwrap_or("<none>")
            .to_string();
        Ok(json!(format!("elicit:{:?}:{env}", answer.action)))
    }
}

/// Tool that returns immediately. Proves a SECOND request is still processed
/// while another handler is parked.
struct FastTool;

#[async_trait]
impl ToolHandler for FastTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!("fast-done"))
    }
}

/// The two-way signal between the guard and its mutex-holding tool.
///
/// [`tokio::sync::Notify::notify_one`] STORES a permit when nobody is waiting,
/// so neither half can lose the other's signal by arriving first. That is what
/// lets this file assert on a provably-held mutex with no fixed sleep and no
/// timing guess.
#[derive(Default)]
struct HoldGate {
    /// Fired by the handler once it is running — i.e. once
    /// `dispatch_public_request` has taken `state.server.lock().await`.
    entered: tokio::sync::Notify,
    /// Fired by the guard when the handler may finish and drop the lock.
    release: tokio::sync::Notify,
}

/// Tool that HOLDS the server mutex until the guard releases it.
///
/// The guard's stand-in for a handler parked on a peer call. It needs no peer at
/// all, and that is the point: the deadlock is a property of
/// `dispatch_public_request` holding `state.server.lock().await` across the
/// handler, not of what the handler is waiting for.
struct MutexHoldingTool(Arc<HoldGate>);

#[async_trait]
impl ToolHandler for MutexHoldingTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        self.0.entered.notify_one();
        // Bounded so an early guard panic cannot wedge the server task.
        let _ = tokio::time::timeout(HOLD, self.0.release.notified()).await;
        Ok(json!("held"))
    }
}

/// The peer, or the [`PEER_ABSENT`] error that names the missing transport wiring.
fn peer_or_absent(
    extra: &RequestHandlerExtra,
) -> pmcp::Result<Arc<dyn pmcp::shared::peer::PeerHandle>> {
    extra.peer().cloned().ok_or_else(|| {
        pmcp::Error::protocol(pmcp::ErrorCode::INTERNAL_ERROR, PEER_ABSENT.to_string())
    })
}

fn build_server(gate: Arc<HoldGate>) -> Server {
    Server::builder()
        .name("http-peer-roundtrip")
        .version("1.0.0")
        .tool("sampler", SamplerTool)
        .tool("roots", RootsTool)
        .tool("elicit", ElicitTool)
        .tool("fast", FastTool)
        .tool("hold", MutexHoldingTool(gate))
        .build()
        .expect("server builds")
}

/// Spawn on an EPHEMERAL port and read the bound address back from `start()`.
///
/// The [`HoldGate`] comes back with it so the guard can drive the mutex-holding
/// tool; every other test ignores it.
async fn spawn() -> (SocketAddr, JoinHandle<()>, Arc<HoldGate>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let gate = Arc::new(HoldGate::default());
    let server = Arc::new(Mutex::new(build_server(gate.clone())));
    let (bound, handle) =
        StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::default())
            .start()
            .await
            .expect("server starts on an ephemeral port");
    (bound, handle, gate)
}

// ===========================================================================
// A minimal raw HTTP/1.1 client.
//
// Raw TCP rather than `reqwest` because this suite needs an INCREMENTALLY read
// `text/event-stream` body from a GET that never reaches EOF, and it needs each
// POST on its own connection so that HTTP keep-alive head-of-line ordering can
// never be mistaken for the server-side serialization the guard measures.
// ===========================================================================

/// One open HTTP response, readable frame by frame.
struct Conn {
    reader: BufReader<TcpStream>,
    status: u16,
    headers: Vec<(String, String)>,
    buffer: String,
    chunked: bool,
    remaining: usize,
    finished: bool,
}

impl Conn {
    /// Send a request and read only as far as the response headers.
    async fn open(addr: SocketAddr, verb: &str, extra: &[(String, String)], body: &str) -> Self {
        let stream = TcpStream::connect(addr).await.expect("connects");
        let accept = if verb == "GET" {
            "text/event-stream"
        } else {
            "application/json, text/event-stream"
        };
        let mut request = format!(
            "{verb} / HTTP/1.1\r\nHost: {addr}\r\nAccept: {accept}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in extra {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);

        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .expect("request written");

        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .await
            .expect("status line");
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("header line");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
            }
        }

        let chunked = headers
            .iter()
            .any(|(n, v)| n == "transfer-encoding" && v.contains("chunked"));
        let remaining = headers
            .iter()
            .find(|(n, _)| n == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        Self {
            reader,
            status,
            headers,
            buffer: String::new(),
            chunked,
            remaining,
            finished: false,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Pull more body bytes into [`Self::buffer`] in whichever framing applies.
    async fn pull(&mut self) -> bool {
        if self.finished {
            return false;
        }
        if !self.chunked {
            let mut payload = vec![0u8; self.remaining];
            let ok = self.remaining > 0 && self.reader.read_exact(&mut payload).await.is_ok();
            self.finished = true;
            if !ok {
                return false;
            }
            self.buffer.push_str(&String::from_utf8_lossy(&payload));
            return true;
        }
        let mut size_line = String::new();
        if self.reader.read_line(&mut size_line).await.unwrap_or(0) == 0 {
            self.finished = true;
            return false;
        }
        let size_token = size_line.trim().split(';').next().unwrap_or("").to_string();
        let Ok(size) = usize::from_str_radix(&size_token, 16) else {
            self.finished = true;
            return false;
        };
        if size == 0 {
            self.finished = true;
            return false;
        }
        let mut payload = vec![0u8; size];
        if self.reader.read_exact(&mut payload).await.is_err() {
            self.finished = true;
            return false;
        }
        let mut crlf = [0u8; 2];
        let _ = self.reader.read_exact(&mut crlf).await;
        self.buffer.push_str(&String::from_utf8_lossy(&payload));
        true
    }

    /// Pop one complete SSE block (`…\n\n`) from the buffer, if present.
    fn take_block(&mut self) -> Option<String> {
        let end = self.buffer.find("\n\n")?;
        let block = self.buffer[..end].to_string();
        self.buffer.drain(..end + 2);
        Some(block)
    }

    /// The next `data:` payload, or `None` at end of stream. UNBOUNDED — always
    /// reach it through [`Self::frame`] or [`Self::silent_for`].
    async fn next_data(&mut self) -> Option<String> {
        loop {
            if let Some(block) = self.take_block() {
                let mut data = String::new();
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        data.push_str(rest.trim_start());
                    }
                }
                if !data.is_empty() {
                    return Some(data);
                }
                continue;
            }
            if !self.pull().await {
                if !self.buffer.trim().is_empty() {
                    let rest = std::mem::take(&mut self.buffer);
                    return Some(rest.trim().to_string());
                }
                return None;
            }
        }
    }

    /// The next protocol frame on this stream, parsed as JSON and BOUNDED.
    async fn frame(&mut self, what: &str) -> Value {
        let data = tokio::time::timeout(BOUND, self.next_data())
            .await
            .unwrap_or_else(|_| panic!("{what} must not hang"))
            .unwrap_or_else(|| panic!("{what}: the stream ended with no frame"));
        serde_json::from_str(&data).expect("every frame on this stream is JSON")
    }

    /// Assert NOTHING is delivered to this stream within `window`.
    async fn silent_for(&mut self, window: Duration, whose: &str) {
        if let Ok(Some(data)) = tokio::time::timeout(window, self.next_data()).await {
            panic!("{whose} must receive nothing, but a frame was delivered to it: {data}");
        }
    }

    /// Read the whole `Content-Length` body of a completed response.
    async fn body(&mut self) -> String {
        while self.pull().await {}
        std::mem::take(&mut self.buffer)
    }
}

// ===========================================================================
// Request construction.
// ===========================================================================

fn envelope(method: &str, id: &Value, params: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn init_body() -> String {
    envelope(
        "initialize",
        &json!(1),
        &json!({
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": { "sampling": {}, "roots": {}, "elicitation": {} },
            "clientInfo": { "name": "http-peer-fence", "version": "1.0.0" }
        }),
    )
}

fn call_body(id: i64, tool: &str) -> String {
    envelope(
        "tools/call",
        &json!(id),
        &json!({ "name": tool, "arguments": {} }),
    )
}

fn session_header(session: &str) -> Vec<(String, String)> {
    vec![("Mcp-Session-Id".to_string(), session.to_string())]
}

/// POST `body` and return `(status, body)`, BOUNDED.
async fn post(
    addr: SocketAddr,
    extra: &[(String, String)],
    body: &str,
    what: &str,
) -> (u16, String) {
    let mut conn = tokio::time::timeout(BOUND, Conn::open(addr, "POST", extra, body))
        .await
        .unwrap_or_else(|_| panic!("{what} must not hang"));
    let status = conn.status;
    let text = tokio::time::timeout(BOUND, conn.body())
        .await
        .unwrap_or_else(|_| panic!("{what} body must not hang"));
    (status, text)
}

/// Handshake: POST `initialize` and return the minted session id.
async fn open_session(addr: SocketAddr) -> String {
    let mut conn = tokio::time::timeout(BOUND, Conn::open(addr, "POST", &[], &init_body()))
        .await
        .expect("initialize must not hang");
    assert_eq!(conn.status, 200, "initialize is answered inline");
    let session = conn
        .header("mcp-session-id")
        .expect("a stateful server mints a session id")
        .to_string();
    let _ = tokio::time::timeout(BOUND, conn.body())
        .await
        .expect("initialize body must not hang");
    session
}

/// Open the session's live SSE stream (the v1 server-to-client vehicle).
async fn open_stream(addr: SocketAddr, session: &str) -> Conn {
    let conn = tokio::time::timeout(BOUND, Conn::open(addr, "GET", &session_header(session), ""))
        .await
        .expect("opening the SSE stream must not hang");
    assert_eq!(conn.status, 200, "the SSE stream opens");
    conn
}

/// Fire a `tools/call` WITHOUT awaiting its HTTP response.
///
/// With a live SSE stream the tool's reply is delivered onto that stream and the
/// POST answers `202 Accepted` only once the handler has finished, so a test
/// that awaited it here could never read the server-to-client request the
/// handler is parked on.
fn spawn_call(addr: SocketAddr, session: &str, id: i64, tool: &str) -> JoinHandle<(u16, String)> {
    let headers = session_header(session);
    let body = call_body(id, tool);
    tokio::spawn(async move { post(addr, &headers, &body, "a queued tools/call").await })
}

/// The id of a server-to-client request frame, or a RED diagnosis.
///
/// A response frame here means the tool returned before ever reaching its peer
/// call — i.e. the transport carries no peer. Naming that explicitly is what
/// separates "peer absent" from "peer present but deadlocked".
fn require_server_request(frame: &Value, method: &str) -> Value {
    if frame.get("method").and_then(Value::as_str) == Some(method) {
        return frame
            .get("id")
            .cloned()
            .expect("a server-to-client request carries an id");
    }
    panic!(
        "expected a server-to-client {method} request on this session's stream, got {frame} \
         — the HTTP transport carries no peer ({PEER_ABSENT}); plan 11 injects it"
    );
}

/// POST the client's answer to a server-to-client request.
async fn answer(addr: SocketAddr, session: &str, id: &Value, result: Value) {
    let body = json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string();
    let (status, _) = post(
        addr,
        &session_header(session),
        &body,
        "the client's answer POST",
    )
    .await;
    assert_eq!(status, 202, "an inbound response is accepted");
}

fn sampling_answer() -> Value {
    json!({
        "role": "assistant",
        "content": { "type": "text", "text": "done" },
        "model": HOST_MODEL,
    })
}

/// The text of a `tools/call` reply frame, whatever shape it took.
fn reply_text(frame: &Value) -> String {
    frame.to_string()
}

async fn teardown(handle: JoinHandle<()>, sockets: impl Send) {
    drop(sockets);
    handle.abort();
    let _ = handle.await;
}

// ===========================================================================
// (1) THE GUARD — the single most important test in this phase.
//
// T-118.1-10-01. `dispatch_public_request` holds `Mutex<Server>` across the whole
// tool handler; `run_v2_header_gate` takes that mutex twice and
// `extract_and_validate_auth` a third time, and all three run BEFORE an inbound
// POST is classified. So a client's answer to a server-to-client request cannot
// reach the dispatcher while the handler that asked for it is parked — a
// guaranteed deadlock, and a DoS control besides.
//
// Measured without a peer on purpose: the hazard belongs to the mutex, not to
// what the handler awaits, so a tool that merely sleeps reproduces it exactly
// and keeps this guard meaningful before plan 11's injection lands.
// ===========================================================================

#[tokio::test]
async fn response_post_is_answered_while_a_tool_handler_holds_the_server_mutex() {
    let (addr, handle, gate) = spawn().await;
    let session = open_session(addr).await;

    // Park a handler. `dispatch_public_request` takes `state.server.lock().await`
    // BEFORE calling the handler, so once `entered` fires the mutex is provably
    // held — no settle guess, no fixed sleep.
    let parked = spawn_call(addr, &session, 2, "hold");
    tokio::time::timeout(BOUND, gate.entered.notified())
        .await
        .expect("the parked handler must start while holding the server mutex");

    // An inbound JSON-RPC response carries no authority — it invokes no method,
    // it only resolves a correlation the SERVER minted — so it must be routed
    // before the gate and auth stages that take the mutex. Unknown correlation
    // ids are accepted indistinguishably (T-118.1-10-03), so 202 is the answer
    // here whether or not anything was pending.
    let body = json!({ "jsonrpc": "2.0", "id": "dispatch-not-live", "result": {} }).to_string();
    let started = std::time::Instant::now();
    let posted = tokio::time::timeout(
        MUTEX_BOUND,
        post(
            addr,
            &session_header(&session),
            &body,
            "the inbound response POST",
        ),
    )
    .await;
    let elapsed = started.elapsed();

    // Release the handler BEFORE asserting, so an assertion failure never leaves
    // the server task holding the mutex for the rest of the binary.
    gate.release.notify_one();

    let (status, _) = posted.unwrap_or_else(|_| {
        panic!(
            "an inbound response POST must not hang behind a parked tool handler: \
             still unanswered after {MUTEX_BOUND:?} while a handler holds Mutex<Server>. \
             The inbound response is not classified before \
             run_v2_header_gate / extract_and_validate_auth."
        )
    });
    assert_eq!(status, 202, "an inbound response is accepted");
    assert!(
        elapsed < MUTEX_BOUND,
        "the inbound response POST waited {elapsed:?} on the server mutex"
    );

    let _ = tokio::time::timeout(BOUND, parked).await;
    teardown(handle, ()).await;
}

// ===========================================================================
// (2) peer.sample() over v1 stateful HTTP.
// ===========================================================================

#[tokio::test]
async fn http_peer_sample_completes_over_a_v1_session() {
    let (addr, handle, _gate) = spawn().await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let call = spawn_call(addr, &session, 2, "sampler");

    let frame = stream.frame("the server-to-client sampling request").await;
    let id = require_server_request(&frame, "sampling/createMessage");
    answer(addr, &session, &id, sampling_answer()).await;

    let reply = stream.frame("the tools/call reply").await;
    assert!(
        reply_text(&reply).contains(&format!("sampled:{HOST_MODEL}")),
        "the tool must observe the host completion model: {reply}"
    );

    call.abort();
    teardown(handle, stream).await;
}

// ===========================================================================
// (3) peer.list_roots() over v1 stateful HTTP.
// ===========================================================================

#[tokio::test]
async fn http_peer_list_roots_completes_over_a_v1_session() {
    let (addr, handle, _gate) = spawn().await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let call = spawn_call(addr, &session, 2, "roots");

    let frame = stream.frame("the server-to-client roots request").await;
    let id = require_server_request(&frame, "roots/list");
    answer(
        addr,
        &session,
        &id,
        json!({ "roots": [ { "uri": "file:///a" }, { "uri": "file:///b" } ] }),
    )
    .await;

    let reply = stream.frame("the tools/call reply").await;
    assert!(
        reply_text(&reply).contains("roots:2"),
        "the tool must observe the two host roots: {reply}"
    );

    call.abort();
    teardown(handle, stream).await;
}

// ===========================================================================
// (4) peer.elicit() over v1 stateful HTTP.
//
// Plan 09 proved this method GREEN on the in-process loop, so a red here is
// attributable to the transport and to nothing else.
// ===========================================================================

#[tokio::test]
async fn http_peer_elicit_completes_over_a_v1_session() {
    let (addr, handle, _gate) = spawn().await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let call = spawn_call(addr, &session, 2, "elicit");

    let frame = stream
        .frame("the server-to-client elicitation request")
        .await;
    let id = require_server_request(&frame, "elicitation/create");
    answer(
        addr,
        &session,
        &id,
        json!({ "action": "accept", "content": { "env": "staging" } }),
    )
    .await;

    let reply = stream.frame("the tools/call reply").await;
    assert!(
        reply_text(&reply).contains("staging"),
        "the tool must observe the accepted form: {reply}"
    );

    call.abort();
    teardown(handle, stream).await;
}

// ===========================================================================
// (5) SATURATION: two requests on the wire before either is answered.
//
// The HTTP twin of `second_request_is_processed_while_first_handler_parks`.
// Both POSTs go out BEFORE the sampling answer, so the second must be processed
// while the first handler is parked on its peer call.
// ===========================================================================

#[tokio::test]
async fn a_second_request_is_processed_while_the_first_handler_parks() {
    let (addr, handle, _gate) = spawn().await;
    let session = open_session(addr).await;
    let mut stream = open_stream(addr, &session).await;

    let first = spawn_call(addr, &session, 2, "sampler");
    let second = spawn_call(addr, &session, 3, "fast");

    let mut sampler_reply: Option<Value> = None;
    let mut fast_reply: Option<Value> = None;
    let mut answered = false;

    // ONE driver loop answers the inbound server-to-client request and collects
    // BOTH tool replies off the same stream, exactly as the in-process twin's
    // driver does over its duplex transport.
    while sampler_reply.is_none() || fast_reply.is_none() {
        let frame = stream.frame("a frame in the saturation driver").await;
        if frame.get("method").and_then(Value::as_str) == Some("sampling/createMessage") {
            let id = require_server_request(&frame, "sampling/createMessage");
            answer(addr, &session, &id, sampling_answer()).await;
            answered = true;
            continue;
        }
        match frame.get("id").and_then(Value::as_i64) {
            Some(2) => sampler_reply = Some(frame),
            Some(3) => fast_reply = Some(frame),
            _ => {},
        }
    }

    let fast = fast_reply.expect("the second call is answered");
    assert!(
        reply_text(&fast).contains("fast-done"),
        "the second request must be processed while the first parks: {fast}"
    );
    let sampled = sampler_reply.expect("the first call is answered");
    assert!(
        answered,
        "the first handler must have reached its peer call — it never issued a \
         sampling request ({PEER_ABSENT}); its reply was {sampled}"
    );
    assert!(
        reply_text(&sampled).contains(&format!("sampled:{HOST_MODEL}")),
        "the parked call must complete once answered: {sampled}"
    );

    first.abort();
    second.abort();
    teardown(handle, stream).await;
}

// ===========================================================================
// (6) SESSION ISOLATION — T-118.1-10-04, the T-113-07 class.
//
// `Server::peer_handle` is a SINGLE global field, so a naive global handle
// passes every other test in this file and fails only this one. That is the
// point: a server-to-client request must reach the session that ISSUED it and
// no other. `build_response`'s rustdoc records the same defect class for direct
// responses; this is its server-to-client twin.
// ===========================================================================

#[tokio::test]
async fn a_server_to_client_request_reaches_only_the_issuing_session() {
    let (addr, handle, _gate) = spawn().await;

    let session_a = open_session(addr).await;
    let session_b = open_session(addr).await;
    assert_ne!(session_a, session_b, "two distinct sessions");
    let mut stream_a = open_stream(addr, &session_a).await;
    let mut stream_b = open_stream(addr, &session_b).await;

    let call = spawn_call(addr, &session_a, 2, "sampler");

    // The SECURITY claim first: B is a bystander and must observe nothing.
    stream_b
        .silent_for(QUIET, "the bystander session's stream")
        .await;

    // Then the delivery claim: A, the issuer, is where it lands.
    let frame = stream_a
        .frame("the issuing session's sampling request")
        .await;
    let id = require_server_request(&frame, "sampling/createMessage");
    answer(addr, &session_a, &id, sampling_answer()).await;

    let reply = stream_a
        .frame("the issuing session's tools/call reply")
        .await;
    assert!(
        reply_text(&reply).contains(&format!("sampled:{HOST_MODEL}")),
        "the issuing session's call completes: {reply}"
    );
    stream_b
        .silent_for(QUIET, "the bystander session's stream")
        .await;

    call.abort();
    teardown(handle, (stream_a, stream_b)).await;
}
