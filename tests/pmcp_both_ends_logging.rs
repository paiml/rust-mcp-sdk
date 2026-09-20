//! D-15.3 — the JOINT fence: pmcp on BOTH ends of a live v1 session stream.
//!
//! # What makes this file different from every other proof in the phase
//!
//! Every existing proof of the server-to-client channel puts a RAW TCP reader on
//! the client end. `tests/http_peer_roundtrip.rs` (12 fences) and
//! `tests/log_emitter.rs` (21 fences) both hand-write the GET, hand-read the
//! `text/event-stream` body and hand-parse the frames. That was deliberate and it
//! was diagnostic: a raw reader is a client that provably DOES hold a live
//! stream, so a green server-half fence beside a red end-to-end one localised
//! the defect to pmcp's own client. This file is the only test in the repo where
//! pmcp is on **both** ends — a real `StreamableHttpServer` on one side, a real
//! `StreamableHttpTransport` on the other — which is exactly what D-15.3 asked
//! for once phase 118.2 plans 01-04 (the client half) and 05-09 (the emitter
//! half) had both landed.
//!
//! # The assertion layer, and why it is the transport and not `Client`
//!
//! There is **no client-side notification observation API to assert through**.
//! `Client::notification_tx` is initialised to `None` at all three construction
//! sites (`src/client/mod.rs:406`, `:453`, `:496`), `ClientBuilder` has no field
//! for it and no setter, and the forwarding branch in `dispatch_request`'s wait
//! loop is therefore permanently dead: a `notifications/message` that reaches a
//! `pmcp::Client` while a request is outstanding is processed by the middleware
//! chain and then DROPPED.
//!
//! So this fence asserts at the transport: it drives `Transport::receive()`
//! directly and matches
//! `TransportMessage::Notification(Notification::Server(ServerNotification::LogMessage(..)))`.
//! That satisfies "pmcp on both ends" literally, costs ZERO new public API, and
//! keeps the phase's `cargo semver-checks` verdict trivially clean. Adding
//! `ClientBuilder::on_notification(..)` or `Client::subscribe_notifications()`
//! would be additive and is a real DX gap — it is recorded as a DEFERRED item in
//! this phase's `deferred-items.md` with its measurement, deliberately rather
//! than folded in here, because a new public surface owes its own
//! fuzz/property/unit/example package (CLAUDE.md ALWAYS-requirements).
//!
//! The handshake still runs through a real `pmcp::ClientBuilder`, so the GET
//! session stream is opened by the SHIPPED client path — the 202 branch plan 01
//! made reachable — and not by a hand-rolled call into `start_sse`. The client is
//! then left IDLE: an idle `pmcp::Client` runs no background receive loop (its
//! only `receive()` calls are inside `dispatch_request`'s wait loop, which runs
//! only while one of its own requests is outstanding), so the fence is the single
//! consumer of the shared receive queue and nothing can race it for the record.
//!
//! # Reliability doctrine
//!
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning — never a fixed sleep as a synchronization
//! device), SHUTDOWN (the transport is closed, then `abort()`, then `await`).
//! EVERY await that crosses the wire is wrapped in [`tokio::time::timeout`]: on
//! the unfixed client these fences HANG rather than fail, and a hung test reads
//! as a slow test in CI rather than as a red one.

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
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::Transport;
use pmcp::types::notifications::{LogMessageParams, LoggingLevel, ServerNotification};
use pmcp::types::{
    CallToolRequest, ClientCapabilities, ClientRequest, Notification, Request, RequestId,
    TransportMessage,
};
use pmcp::{ClientBuilder, RequestHandlerExtra, Server, ToolHandler};

// ===========================================================================
// Bounds. Upper bounds on wire operations, never synchronization devices.
// ===========================================================================

/// Upper bound on any single operation that crosses the wire.
const BOUND: Duration = Duration::from_secs(10);

/// Upper bound on the whole drain — the record AND the reply, together.
///
/// Separate from [`BOUND`] so a drain that receives an endless stream of
/// unrelated frames fails on the DRAIN rather than resetting its budget on every
/// individual `receive()`.
const DRAIN_BOUND: Duration = Duration::from_secs(20);

// ===========================================================================
// The wire vocabulary. Spelled as literals here, never imported from `src/`:
// a fence that reads its expectation out of the code under test asserts
// nothing.
// ===========================================================================

/// The message text the handler emits and the fence matches on.
///
/// Distinctive on purpose. Asserting only on the `LogMessage` VARIANT would let
/// any unrelated `notifications/message` — a future keepalive, a framework
/// record, a second tool's emission — satisfy the fence, which is T-118.2-10-03.
const SENTINEL: &str = "pmcp-both-ends-log-record-8c1e4f2a";

/// The tool the fence calls.
const TOOL: &str = "logger";

/// The id the fence's own `tools/call` carries.
///
/// A STRING id, so it can never collide with the numeric ids the real
/// `pmcp::Client` mints for its own handshake requests on the same transport.
const CALL_ID: &str = "both-ends-call-1";

// ===========================================================================
// The server half: a real pmcp server whose handler emits through the public
// `RequestHandlerExtra::log` surface (phase 118.2 plan 05).
// ===========================================================================

/// A tool that emits exactly ONE record, at `info`, carrying [`SENTINEL`].
///
/// `info` and not `debug`: nothing in this fence calls `logging/setLevel`, so the
/// `info` default (D-12) applies and a `debug` record would be filtered at the
/// emitter — the fence would then be measuring the filter rather than the
/// channel.
struct LoggingTool;

#[async_trait]
impl ToolHandler for LoggingTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        extra.log(LoggingLevel::Info, SENTINEL)?;
        Ok(json!("logged"))
    }
}

/// A stock v1 server: NOT opted into v2, which is what a default deployment is.
fn build_v1_server() -> Server {
    Server::builder()
        .name("pmcp-both-ends-logging")
        .version("1.0.0")
        .tool(TOOL, LoggingTool)
        .build()
        .expect("server builds")
}

/// Spawn on an EPHEMERAL port, STATEFUL config, address read back from `start()`.
///
/// `StreamableHttpServerConfig::default()` keeps a live `session_id_generator`,
/// which is what makes the v1 SESSION stream — the vehicle under test — exist at
/// all.
async fn spawn_server(server: Server) -> (SocketAddr, JoinHandle<()>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let server = Arc::new(Mutex::new(server));
    StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::default())
        .start()
        .await
        .expect("server starts on an ephemeral port")
}

// ===========================================================================
// The client half: a real pmcp `StreamableHttpTransport`.
// ===========================================================================

/// The transport the fence drives, pointed at `addr`.
fn transport_for(addr: SocketAddr) -> StreamableHttpTransport {
    StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(
            url::Url::parse(&format!("http://{addr}")).expect("the bound address is a URL"),
        )
        .build(),
    )
}

/// A `tools/call` for [`TOOL`], as a typed transport message.
fn call_tool_request() -> TransportMessage {
    TransportMessage::Request {
        id: RequestId::from(CALL_ID),
        request: Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
            TOOL,
            json!({}),
        )))),
    }
}

/// The `LogMessageParams` of a received `notifications/message`, if that is what
/// this message is.
fn log_record_of(message: &TransportMessage) -> Option<&LogMessageParams> {
    match message {
        TransportMessage::Notification(Notification::Server(ServerNotification::LogMessage(
            params,
        ))) => Some(params),
        _ => None,
    }
}

/// Whether this message is the JSON-RPC reply to [`CALL_ID`].
fn is_call_reply(message: &TransportMessage) -> bool {
    match message {
        TransportMessage::Response(response) => response.id == RequestId::from(CALL_ID),
        _ => false,
    }
}

/// What the drain saw, in ARRIVAL ORDER.
///
/// The order is the substance of the fence's second claim, so it is recorded as
/// a sequence rather than as two booleans: "the record arrived before the reply"
/// is not expressible over flags that only say WHETHER each arrived.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    /// A `notifications/message` carrying [`SENTINEL`].
    Record,
    /// The JSON-RPC reply to [`CALL_ID`].
    Reply,
    /// Anything else that reached `receive()`.
    Other,
}

/// Drive `receive()` until BOTH the sentinel record and the call's reply have
/// arrived, recording arrival order.
///
/// Bounded twice over: each individual `receive()` by [`BOUND`], and the whole
/// drain by [`DRAIN_BOUND`]. Returns the order; the caller asserts on it.
async fn drain_until_record_and_reply(transport: &mut StreamableHttpTransport) -> Vec<Seen> {
    let mut order = Vec::new();
    let drained = timeout(DRAIN_BOUND, async {
        loop {
            let message = timeout(BOUND, transport.receive())
                .await
                .expect(
                    "a frame must reach Transport::receive() within BOUND — on a client that \
                     collects the GET body to completion instead of reading it live, this is \
                     where the defect surfaces as a HANG rather than as a failure",
                )
                .expect("the frame parses into a transport message");

            if log_record_of(&message).is_some_and(|params| params.message == SENTINEL) {
                order.push(Seen::Record);
            } else if is_call_reply(&message) {
                order.push(Seen::Reply);
            } else {
                order.push(Seen::Other);
            }

            if order.contains(&Seen::Record) && order.contains(&Seen::Reply) {
                return;
            }
        }
    })
    .await;

    assert!(
        drained.is_ok(),
        "the drain must complete within DRAIN_BOUND. Observed arrivals: {order:?}"
    );
    order
}

// ===========================================================================
// The joint fence.
// ===========================================================================

#[tokio::test]
async fn a_pmcp_client_receives_a_handler_log_record_over_a_live_v1_session_stream() {
    let (addr, handle) = spawn_server(build_v1_server()).await;

    // The handshake runs through the REAL client, so the GET session stream is
    // opened by the shipped path (plan 01's 202 branch). `observer` shares every
    // field behind an `Arc` with the clone the client owns, including the receive
    // queue — which is what lets the fence read the stream the client opened.
    let transport = transport_for(addr);
    let mut observer = transport.clone();
    let mut client = ClientBuilder::new(transport).build();
    timeout(BOUND, client.initialize(ClientCapabilities::default()))
        .await
        .expect("initialize returns within BOUND — a hang here IS the client-half defect")
        .expect("the pmcp server answers initialize");

    // The call is issued on the OBSERVER, not through `client.call_tool(..)`,
    // and the client is left idle for the rest of the fence. A real `call_tool`
    // would park the client in `dispatch_request`'s wait loop, which drains the
    // same shared queue and DROPS every notification it sees (`notification_tx`
    // is `None` — see the module doc), so the record would be consumed before it
    // could be observed. Issuing it here makes the fence the single consumer.
    timeout(BOUND, observer.send(call_tool_request()))
        .await
        .expect("the tools/call POST completes within BOUND")
        .expect("the pmcp server accepts the tools/call");

    let order = drain_until_record_and_reply(&mut observer).await;

    // CLAIM 1 — the record reached pmcp's own client, and it is THE record: the
    // sentinel is matched, not merely the variant.
    let record_index = order
        .iter()
        .position(|seen| *seen == Seen::Record)
        .expect("the drain returns only once the sentinel record has arrived");
    let reply_index = order
        .iter()
        .position(|seen| *seen == Seen::Reply)
        .expect("the drain returns only once the reply has arrived");

    // CLAIM 2 — ORDERING. On v1 the record is framed onto the session SSE stream
    // by the handler WHILE it runs, and the tool reply is framed after the
    // handler returns, so the record must arrive first. This is the "while the
    // tool call was still outstanding" half of the plan's step 5, expressed as an
    // ordering over the client's own receive queue rather than as a timing race:
    // the reply had not yet been observed when the record was.
    assert!(
        record_index < reply_index,
        "the handler's log record must reach the client BEFORE the call's reply — a record that \
         only arrives after the reply is a record the client could not have acted on during the \
         call, which is the whole point of a streaming log channel. Observed arrivals: {order:?}"
    );

    // Shutdown in the documented order: close the transport (which stops the SSE
    // reader task), drop the client, then abort and await the server.
    drop(client);
    let _ = timeout(BOUND, observer.close()).await;
    handle.abort();
    let _ = handle.await;
}

// ===========================================================================
// The level and the method, on the SAME record — asserted separately so a
// failure names which property moved.
// ===========================================================================

#[tokio::test]
async fn the_record_the_client_receives_carries_the_emitted_level() {
    let (addr, handle) = spawn_server(build_v1_server()).await;

    let transport = transport_for(addr);
    let mut observer = transport.clone();
    let mut client = ClientBuilder::new(transport).build();
    timeout(BOUND, client.initialize(ClientCapabilities::default()))
        .await
        .expect("initialize returns within BOUND")
        .expect("the pmcp server answers initialize");

    timeout(BOUND, observer.send(call_tool_request()))
        .await
        .expect("the tools/call POST completes within BOUND")
        .expect("the pmcp server accepts the tools/call");

    // Re-drain rather than share state with the fence above: two tests that
    // depend on one another's leftovers are two tests that fail together for
    // reasons neither names.
    let mut level = None;
    let drained = timeout(DRAIN_BOUND, async {
        loop {
            let message = timeout(BOUND, observer.receive())
                .await
                .expect("a frame reaches receive() within BOUND")
                .expect("the frame parses into a transport message");
            if let Some(params) = log_record_of(&message) {
                if params.message == SENTINEL {
                    level = Some(params.level);
                    return;
                }
            }
        }
    })
    .await;
    assert!(
        drained.is_ok(),
        "the sentinel record arrives within DRAIN_BOUND"
    );

    assert_eq!(
        level,
        Some(LoggingLevel::Info),
        "the level the handler emitted at must survive the round trip as a TYPED level on the \
         client side — a record that arrives at the wrong severity cannot be filtered correctly \
         by any consumer"
    );

    drop(client);
    let _ = timeout(BOUND, observer.close()).await;
    handle.abort();
    let _ = handle.await;
}
