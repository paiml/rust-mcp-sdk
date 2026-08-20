//! Phase 118.1 plan 12 (CONF-07 / G-3's v2 half, D-16): the v2 POST response body
//! as a MULTI-FRAME SSE vehicle for `notifications/progress`.
//!
//! # Why this file asserts on `Resp.raw` and not on `Resp.body`
//!
//! [`common::v2::Resp::body`] is SSE-UNWRAPPED — it returns the FIRST parseable
//! `data:` payload. That is exactly the wrong instrument here: frame COUNT and
//! frame ORDER are the entire property under test, and `body` cannot see either.
//! Every assertion below therefore reads `raw`, the verbatim response text with
//! its SSE framing intact.
//!
//! # What A3 measured, and why this file exists
//!
//! Plan 12 Task 1 ran the PINNED conformance suite (0.2.0-alpha.11) against a
//! scratch v2 server that answered one `tools/call` with three
//! `notifications/progress` frames followed by the result frame on the POST
//! response body. The suite reported `tools-call-with-progress` SUCCESS with
//! `progressCount: 3`. So the vehicle is CONFIRMED, and the suite's own v2 client
//! (`wt` in `dist/index.js`) reads it like this:
//!
//! ```text
//! if ('method' in frame && !('id' in frame))  -> push onto `notifications`
//! if ('method' in frame &&  'id' in frame)    -> throw -32600
//!     "Server sent request '...' on response stream; stateless lifecycle
//!      forbids this (use MRTR)"
//! ```
//!
//! That second arm is `HttpServerNoIndependentRequestsOnStream`. pmcp enforces it
//! STRUCTURALLY rather than by convention: the stream's item type
//! (`V2ResponseFrame`) has no request variant, so an independent server-to-client
//! request is unrepresentable. `no_independent_request_frame_on_the_stream` is the
//! wire-level guard that the type-level control actually reaches the wire.
//!
//! Test reliability doctrine (carried from `tests/v2_stateless_http.rs`): EPHEMERAL
//! PORT, READINESS from `start()` binding before it returns, SHUTDOWN via
//! `JoinHandle::abort()`.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{
    post_with_accept, spawn_default_config, spawn_with, v2_body, v2_headers, Resp, V1, V2,
};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};

/// The `Accept` a conformant v2 client sends — verbatim what the pinned suite's
/// `vt()` header builder emits (`application/json, text/event-stream`).
const ACCEPT_SSE: &str = "application/json, text/event-stream";

/// An `Accept` that does NOT admit SSE. Content negotiation must keep such a
/// client on JSON even when the handler emitted progress (T-118.1-12-05).
const ACCEPT_JSON_ONLY: &str = "application/json";

/// The progress token this file drives every assertion from.
const TOKEN: &str = "progress-test-1";

/// How many progress frames [`ProgressTool`] puts on the wire.
///
/// THREE, because that is the floor the pinned suite's `tools-call-with-progress`
/// asserts (`i.length < 3` is a FAILURE). Delivered deterministically with no
/// timing dependence — see [`ProgressTool`].
const EXPECTED_PROGRESS_FRAMES: usize = 3;

// ===========================================================================
// Fixtures.
// ===========================================================================

/// A tool that emits exactly [`EXPECTED_PROGRESS_FRAMES`] progress notifications.
///
/// # The 100 ms rate limit is the reason this tool sleeps at all
///
/// `ServerProgressReporter` admits at most one notification per 100 ms, with two
/// unconditional exceptions: the FIRST report always goes, and a FINAL report
/// (`progress == total`) always goes. Frames 1 and 3 ride those exceptions and
/// need no timing at all. Frame 2 is neither, so it is admitted ONLY if the
/// reporter's window has elapsed — hence [`RATE_LIMIT_ESCAPE`], one tick above
/// 100 ms.
///
/// This is stated rather than hidden because it is a real defect elsewhere:
/// `examples/s54_v2_dual_conformance.rs` slept **50 ms** between the same three
/// reports, so its middle frame was silently swallowed and the tool delivered TWO
/// frames while its own comment claimed three — one short of the floor the pinned
/// suite asserts. Plan 12 Task 3 fixes that.
struct ProgressTool;

/// One tick above `ServerProgressReporter`'s 100 ms rate-limit window.
const RATE_LIMIT_ESCAPE: std::time::Duration = std::time::Duration::from_millis(120);

#[async_trait]
impl ToolHandler for ProgressTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // FIRST — always admitted.
        extra.report_progress(0.0, Some(100.0), None).await?;
        tokio::time::sleep(RATE_LIMIT_ESCAPE).await;
        // MIDDLE — admitted only because the interval above cleared the window.
        extra.report_progress(50.0, Some(100.0), None).await?;
        // FINAL (`progress == total`) — always admitted, no interval needed.
        extra.report_progress(100.0, Some(100.0), None).await?;
        Ok(json!("progress-done"))
    }
}

/// A tool that emits NO progress at all — the byte-identity control.
struct SilentTool;

#[async_trait]
impl ToolHandler for SilentTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!("silent-done"))
    }
}

/// A tool that emits progress in a TIGHT LOOP with no awaits, to prove the queue
/// is BOUNDED (T-118.1-12-01).
///
/// Every report after the first is inside the reporter's 100 ms window and is
/// NOT final, so the reporter itself drops them — which is precisely why this
/// fixture cannot be used to measure the transport's bound directly. What it CAN
/// prove is that a hostile handler does not make the response unbounded: the
/// frame count stays small and the response still terminates.
struct FloodTool;

/// How many reports [`FloodTool`] attempts.
const FLOOD_REPORTS: u32 = 5_000;

#[async_trait]
impl ToolHandler for FloodTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        for step in 1..=FLOOD_REPORTS {
            extra.report_progress(f64::from(step), None, None).await?;
        }
        Ok(json!("flood-done"))
    }
}

/// Build a v2-opted-in server carrying ONLY this file's fixtures.
///
/// A local builder rather than [`common::v2::build_v2_server`]: that shared
/// fixture returns a fully-BUILT `Server`, so a tool cannot be layered onto it.
fn build_progress_server() -> Server {
    Server::builder()
        .name("v2-progress-harness")
        .version("1.0.0")
        .with_supported_protocol_versions([
            pmcp::types::protocol::ProtocolVersion(V1.to_string()),
            pmcp::types::protocol::ProtocolVersion(V2.to_string()),
        ])
        .tool("progress", ProgressTool)
        .tool("silent", SilentTool)
        .tool("flood", FloodTool)
        .build()
        .expect("server builds")
}

// ===========================================================================
// Request bodies.
// ===========================================================================

/// A v2 `tools/call` body carrying `params._meta.progressToken`.
///
/// Built by ADDING one key to the harness's own `_meta` object rather than
/// re-spelling the three reserved keys locally — that seam is what protects the
/// D-113-A `_meta`-vs-`meta` regression.
fn v2_progress_call(tool: &str, id: Value, token: Option<&str>) -> String {
    let base = v2_body("tools/call", id, json!({ "name": tool, "arguments": {} }));
    let mut value: Value = serde_json::from_str(&base).expect("harness body parses");
    if let Some(token) = token {
        value["params"]["_meta"]["progressToken"] = json!(token);
    }
    value.to_string()
}

// ===========================================================================
// Frame decoding — the whole point of asserting on `raw`.
// ===========================================================================

/// Every `data:` payload in an SSE body, in wire order, parsed as JSON.
fn frames(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|payload| serde_json::from_str::<Value>(payload.trim()).ok())
        .collect()
}

/// The `notifications/progress` frames, in wire order.
fn progress_frames(raw: &str) -> Vec<Value> {
    frames(raw)
        .into_iter()
        .filter(|f| f.get("method").and_then(Value::as_str) == Some("notifications/progress"))
        .collect()
}

/// Drive a v2 `tools/call` with the SSE-admitting `Accept`.
async fn call_sse(tool: &str, id: Value, token: Option<&str>) -> Resp {
    let (addr, handle) = spawn_default_config(build_progress_server()).await;
    let response = post_with_accept(
        addr,
        ACCEPT_SSE,
        &v2_headers("tools/call", tool),
        &v2_progress_call(tool, id, token),
    )
    .await;
    handle.abort();
    response
}

// ===========================================================================
// D-16: the multi-frame vehicle.
// ===========================================================================

/// THE load-bearing assertion: progress frames, THEN the result frame, on one
/// SSE-framed POST response body.
#[tokio::test]
async fn v2_progress_call_returns_multi_frame_sse() {
    let response = call_sse("progress", json!(1), Some(TOKEN)).await;

    assert_eq!(response.status, 200, "raw: {}", response.raw);
    assert!(
        response
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/event-stream")),
        "a v2 call that emitted progress must answer text/event-stream; got {:?}; raw: {}",
        response.content_type,
        response.raw
    );

    let progress = progress_frames(&response.raw);
    assert_eq!(
        progress.len(),
        EXPECTED_PROGRESS_FRAMES,
        "expected {EXPECTED_PROGRESS_FRAMES} progress frames on the POST body; raw: {}",
        response.raw
    );
    for frame in &progress {
        assert_eq!(
            frame["params"]["progressToken"],
            json!(TOKEN),
            "every progress frame carries the request's token; raw: {}",
            response.raw
        );
    }
}

/// FRAME ORDER: every progress frame precedes the result frame.
///
/// This is the assertion `Resp.body` structurally cannot make — it unwraps to the
/// FIRST `data:` payload and discards the rest.
#[tokio::test]
async fn progress_frames_precede_the_result_frame() {
    let response = call_sse("progress", json!(7), Some(TOKEN)).await;
    let all = frames(&response.raw);

    let result_index = all
        .iter()
        .position(|f| f.get("result").is_some() || f.get("error").is_some())
        .unwrap_or_else(|| panic!("no result frame on the stream; raw: {}", response.raw));

    assert_eq!(
        result_index,
        all.len() - 1,
        "the result frame must be LAST; raw: {}",
        response.raw
    );
    for (index, frame) in all.iter().enumerate().take(result_index) {
        assert_eq!(
            frame.get("method").and_then(Value::as_str),
            Some("notifications/progress"),
            "frame {index} before the result must be a progress notification; raw: {}",
            response.raw
        );
    }
}

/// The result frame's `id` matches the request's, so the suite's `St()` drain
/// (which stops at the frame whose id matches) terminates.
#[tokio::test]
async fn result_frame_id_matches_the_request_id() {
    let response = call_sse("progress", json!(42), Some(TOKEN)).await;
    let all = frames(&response.raw);
    let result = all
        .iter()
        .find(|f| f.get("result").is_some() || f.get("error").is_some())
        .unwrap_or_else(|| panic!("no result frame; raw: {}", response.raw));

    assert_eq!(
        result["id"],
        json!(42),
        "the result frame must carry the LIVE request id (HTTP-05); raw: {}",
        response.raw
    );
}

/// T-118.1-12-02 at the wire: no frame is an independent server-to-client
/// REQUEST (a frame carrying BOTH `method` and `id`). The suite throws -32600 on
/// exactly this shape.
#[tokio::test]
async fn no_independent_request_frame_on_the_stream() {
    let response = call_sse("progress", json!(3), Some(TOKEN)).await;

    for frame in frames(&response.raw) {
        let is_request = frame.get("method").is_some() && frame.get("id").is_some();
        assert!(
            !is_request,
            "HttpServerNoIndependentRequestsOnStream: the v2 response stream may carry \
             notifications and the result ONLY, never a top-level server-to-client request. \
             Offending frame: {frame}; raw: {}",
            response.raw
        );
    }
}

// ===========================================================================
// The no-regression half: everything that emitted no progress is UNCHANGED.
// ===========================================================================

/// Assert a response is EXACTLY today's v2 shape: a bare JSON body, no SSE
/// framing anywhere in it, and the result inline.
///
/// # What "unchanged" means on v2, measured rather than assumed
///
/// `build_response` routes a v2 POST through its `SSE no-session fallback` arm —
/// v2 has no session to route to — so today's v2 reply is BARE JSON, NOT a
/// one-frame SSE body. This helper pins that: no `data:` prefix, no `event:`
/// line, and the whole body parses as one JSON-RPC response.
fn assert_unchanged_json_shape(response: &Resp, expected_id: &Value) {
    assert_eq!(response.status, 200, "raw: {}", response.raw);
    assert!(
        !response
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/event-stream")),
        "a v2 call that emitted no progress must NOT be switched onto SSE; got {:?}; raw: {}",
        response.content_type,
        response.raw
    );
    assert!(
        !response.raw.contains("data:") && !response.raw.contains("event:"),
        "and it carries NO SSE framing at all; raw: {}",
        response.raw
    );
    let parsed: Value = serde_json::from_str(&response.raw)
        .unwrap_or_else(|e| panic!("the whole body parses as JSON ({e}); raw: {}", response.raw));
    assert_eq!(&parsed["id"], expected_id, "raw: {}", response.raw);
    assert!(
        parsed.get("result").is_some(),
        "with the result inline; raw: {}",
        response.raw
    );
    assert!(
        progress_frames(&response.raw).is_empty(),
        "and no progress frames; raw: {}",
        response.raw
    );
}

/// A v2 call whose handler emits NO progress is shaped exactly as before.
#[tokio::test]
async fn v2_call_without_progress_is_unchanged() {
    let response = call_sse("silent", json!(11), Some(TOKEN)).await;
    assert_unchanged_json_shape(&response, &json!(11));
}

/// A v2 call with NO `progressToken` gets no reporter at all, so it too is
/// unchanged — even though the tool WOULD have emitted had it been asked.
///
/// The stronger half of the pair: the same `progress` tool that produces three
/// frames in [`v2_progress_call_returns_multi_frame_sse`] produces the untouched
/// JSON shape here, so the difference is attributable to the token alone.
#[tokio::test]
async fn v2_call_without_progress_token_is_unchanged() {
    let response = call_sse("progress", json!(12), None).await;
    assert_unchanged_json_shape(&response, &json!(12));
}

/// T-118.1-12-05: a client that does NOT accept `text/event-stream` keeps a JSON
/// response even when the handler emitted progress.
#[tokio::test]
async fn json_only_accept_keeps_a_json_response() {
    let (addr, handle) = spawn_default_config(build_progress_server()).await;
    let response = post_with_accept(
        addr,
        ACCEPT_JSON_ONLY,
        &v2_headers("tools/call", "progress"),
        &v2_progress_call("progress", json!(21), Some(TOKEN)),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "raw: {}", response.raw);
    assert!(
        !response
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/event-stream")),
        "a client that did not accept SSE must not be switched onto it; got {:?}; raw: {}",
        response.content_type,
        response.raw
    );
    assert!(
        progress_frames(&response.raw).is_empty(),
        "and it receives no progress frames; raw: {}",
        response.raw
    );
}

/// JSON mode (`enable_json_response: true`) short-circuits BEFORE the multi-frame
/// path, so a JSON-mode server never emits SSE.
#[tokio::test]
async fn json_mode_server_never_emits_sse() {
    let config = pmcp::server::streamable_http_server::StreamableHttpServerConfig {
        enable_json_response: true,
        ..Default::default()
    };
    let (addr, handle) = spawn_with(build_progress_server(), config).await;
    let response = post_with_accept(
        addr,
        ACCEPT_SSE,
        &v2_headers("tools/call", "progress"),
        &v2_progress_call("progress", json!(31), Some(TOKEN)),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "raw: {}", response.raw);
    assert!(
        !response
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/event-stream")),
        "JSON mode must win; got {:?}; raw: {}",
        response.content_type,
        response.raw
    );
}

// ===========================================================================
// T-118.1-12-01: the queue is BOUNDED.
// ===========================================================================

/// A handler that floods progress does not produce an unbounded response, and the
/// request still terminates with its result frame.
#[tokio::test]
async fn a_flooding_handler_cannot_grow_the_response_without_bound() {
    let response = call_sse("flood", json!(51), Some(TOKEN)).await;

    assert_eq!(response.status, 200, "raw: {}", response.raw);
    let progress = progress_frames(&response.raw);
    assert!(
        progress.len() <= pmcp::server::streamable_http_server::V2_PROGRESS_QUEUE_CAPACITY,
        "the per-request progress queue is bounded at {}; observed {} frames from {} attempted \
         reports; raw len {}",
        pmcp::server::streamable_http_server::V2_PROGRESS_QUEUE_CAPACITY,
        progress.len(),
        FLOOD_REPORTS,
        response.raw.len()
    );
    assert!(
        frames(&response.raw)
            .last()
            .is_some_and(|f| f.get("result").is_some()),
        "and the stream still terminates on the result frame; raw len {}",
        response.raw.len()
    );
}

// ===========================================================================
// Plan 12 Task 3: the dual-conformance EXAMPLE is RUN, on BOTH eras.
//
// `make test-examples` only BUILDS examples, so "the conformance target emits
// progress" would otherwise be an unenforced claim — the Phase-115 gate
// precedent, and the exact false green the cross-AI review flagged for this
// plan. This leg spawns the ALREADY-BUILT binary, drives `test_tool_with_progress`
// on both eras, records the observed bytes as artifacts, and asserts a NON-ZERO
// progress-frame count in each.
//
// It FAILS rather than skipping when the binary is absent (`spawn_example`'s own
// assertion), because a skip restores the criterion this leg exists to close.
// ===========================================================================

/// The example's compiled path, relative to the crate manifest's target dir.
const EXAMPLE_REL_PATH: &str = "debug/examples/s54_v2_dual_conformance";

/// Port 8155, deliberately.
///
/// 8147 is `s47_v2_stateless_mrtr`, 8149 is this example's own default, 8150 is
/// `s50`/`s51`, 8151 belongs to `scripts/run-conformance-suite.sh`, 8153 is held
/// by plan 04's leg in `tests/completion_complete.rs` and 8157 by plan 03's leg
/// in `tests/embedded_resource_example_run.rs` — both of which run CONCURRENTLY
/// with this one under nextest. 8155 appears only as the fallback hint inside the
/// example's own bind-failure message and is bound by nothing.
const EXAMPLE_BIND_ADDR: &str = "127.0.0.1:8155";

/// The tool name the pinned scenario requires, verbatim from `dist/index.js`:
/// `n.request("tools/call", { name: "test_tool_with_progress", arguments: {},
/// _meta: { progressToken: "progress-test-1" } })`.
const SCENARIO_TOOL: &str = "test_tool_with_progress";

/// Where the v1 leg's raw SSE bytes land, for the SUMMARY to quote.
const V1_FRAMES_ARTIFACT: &str = "118.1-12-example-v1-frames.txt";

/// Where the v2 leg's raw POST body lands.
const V2_BODY_ARTIFACT: &str = "118.1-12-example-v2-body.txt";

/// How long the child gets to bind before the leg gives up.
const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// How long the port gets to become free again after the child is killed.
const RELEASE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long the v1 SSE reader holds the stream open collecting frames.
///
/// The tool spends 2 x 120 ms sleeping (see `PROGRESS_INTERVAL` in the example),
/// so this must comfortably exceed that. The reader stops EARLY once it has seen
/// the result frame, so the full window is only spent on a failure.
const V1_COLLECT_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Read a v1 session's GET SSE stream RAW, holding it open.
///
/// # Why a raw socket and not the harness's `get()`
///
/// `common::v2::get` (and pmcp's own `StreamableHttpTransport` client) read the
/// body to EOF. A v1 session SSE stream NEVER ends, so both hang — that is the
/// client-transport gap plan 11 recorded in `deferred-items.md`, and it is why
/// `tests/http_peer_roundtrip.rs` drives its v1 assertions from a raw TCP reader
/// too. The server half is not in question here; the reader is.
///
/// Returns the bytes observed, chunked-transfer framing included. The assertion
/// downstream counts occurrences of `notifications/progress` in those bytes
/// rather than parsing them, so a chunk boundary landing mid-line cannot turn a
/// real frame into a false negative for the COUNT — and the raw bytes are written
/// to disk either way.
#[cfg(feature = "v1-compat")]
async fn read_v1_session_stream(
    addr: std::net::SocketAddr,
    session: &str,
    trigger: impl std::future::Future<Output = ()>,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("the example accepts a GET connection");
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n\
         Mcp-Session-Id: {session}\r\nConnection: keep-alive\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the GET request is written");

    // The stream must be OPEN before the call is issued, or the server has
    // nowhere to route the notifications and drops them. Read the response head
    // first, which the server writes as soon as it accepts the stream.
    let mut observed = String::new();
    let mut buffer = [0_u8; 8192];
    let deadline = tokio::time::Instant::now() + V1_COLLECT_WINDOW;
    while !observed.contains("\r\n\r\n") {
        let read = tokio::time::timeout_at(deadline, stream.read(&mut buffer))
            .await
            .expect("the example answers the GET before the deadline")
            .expect("the GET stream is readable");
        assert!(read > 0, "the example closed the SSE stream immediately");
        observed.push_str(&String::from_utf8_lossy(&buffer[..read]));
    }

    trigger.await;

    // Collect until the result frame arrives or the window closes. Stopping on
    // the result frame keeps the happy path fast; the window is the failure bound.
    while tokio::time::Instant::now() < deadline && !observed.contains("\"result\"") {
        match tokio::time::timeout_at(deadline, stream.read(&mut buffer)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(read)) => observed.push_str(&String::from_utf8_lossy(&buffer[..read])),
            Ok(Err(error)) => panic!("the v1 SSE stream errored: {error}"),
        }
    }
    observed
}

/// A `tools/call` body for the scenario's tool with its exact progress token.
fn scenario_call_body(era_body: fn(&str, Value, Value) -> String, id: Value) -> String {
    let base = era_body(
        "tools/call",
        id,
        json!({ "name": SCENARIO_TOOL, "arguments": {} }),
    );
    let mut value: Value = serde_json::from_str(&base).expect("harness body parses");
    value["params"]["_meta"]["progressToken"] = json!(TOKEN);
    value.to_string()
}

/// `v1-compat`-gated: this is the file's ONE dual-era test and its v1 leg opens a
/// real session. A `--no-default-features --features full-v2` build mints none,
/// so on that build it panics with "the example minted no session id" — a v1
/// complaint against a configuration that deliberately has no v1. The nine
/// v2-only tests above are deliberately NOT gated; they are exactly the coverage
/// the severed build should still be running.
#[cfg(feature = "v1-compat")]
#[tokio::test]
async fn the_dual_conformance_example_emits_progress_on_both_eras() {
    use common::example_process::{
        spawn_example, target_dir, wait_until_listening, wait_until_released,
    };
    use common::v2::{header, post, v1_body, v2_headers};
    use pmcp::shared::http_constants::MCP_SESSION_ID;
    use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;

    let (addr, mut guard) = spawn_example(EXAMPLE_REL_PATH, EXAMPLE_BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // --- v1: handshake, hold the session stream open, then call. -----------
    let init = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(0),
            json!({
                "protocolVersion": LATEST_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "v2-sse-progress-example-run", "version": "0.0.0" },
            }),
        ),
    )
    .await;
    assert_eq!(
        init.status, 200,
        "the v1 handshake must succeed before the progress call: {}",
        init.raw
    );
    let session = init
        .mcp_session_id
        .clone()
        .unwrap_or_else(|| panic!("the example minted no session id: {}", init.raw));

    let v1_frames = read_v1_session_stream(addr, &session, async {
        let call = post(
            addr,
            &[header(MCP_SESSION_ID, &session)],
            &scenario_call_body(v1_body, json!(1)),
        )
        .await;
        // 202, not 200, and that is the POINT: with a live session stream open,
        // `build_response` routes the reply INTO that stream and answers the POST
        // with a bare `202 Accepted`. A 200 here would mean the stream was NOT
        // open and the reply fell back to the one-shot SSE body — in which case
        // every progress notification would have been dropped and the frame count
        // below would be measuring nothing.
        assert_eq!(
            call.status, 202,
            "the v1 progress call must be ACCEPTED into the open session stream: {}",
            call.raw
        );
    })
    .await;

    // --- v2: the POST response body IS the stream. -------------------------
    let v2 = post_with_accept(
        addr,
        ACCEPT_SSE,
        &v2_headers("tools/call", SCENARIO_TOOL),
        &scenario_call_body(v2_body, json!(2)),
    )
    .await;

    // BOTH legs are recorded BEFORE either is asserted, so a failure on one era
    // is still diagnosed against a recording of both (the plan-02 lesson).
    let v1_path = target_dir().join(V1_FRAMES_ARTIFACT);
    let v2_path = target_dir().join(V2_BODY_ARTIFACT);
    std::fs::write(&v1_path, &v1_frames)
        .unwrap_or_else(|e| panic!("could not write {}: {e}", v1_path.display()));
    std::fs::write(&v2_path, &v2.raw)
        .unwrap_or_else(|e| panic!("could not write {}: {e}", v2_path.display()));

    let v1_count = v1_frames.matches("notifications/progress").count();
    let v2_count = progress_frames(&v2.raw).len();

    assert!(
        v1_count >= EXPECTED_PROGRESS_FRAMES,
        "v1 (2025-11-25): the example must put at least {EXPECTED_PROGRESS_FRAMES} progress \
         frames on the session stream — that is the floor the pinned scenario asserts. \
         Saw {v1_count}. Recorded at {}. Stream was:\n{v1_frames}",
        v1_path.display()
    );
    assert!(
        v2_count >= EXPECTED_PROGRESS_FRAMES,
        "v2 (2026-07-28): the example must put at least {EXPECTED_PROGRESS_FRAMES} progress \
         frames on the POST RESPONSE BODY. Saw {v2_count}. Recorded at {}. Body was:\n{}",
        v2_path.display(),
        v2.raw
    );
    assert!(
        v2.content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/event-stream")),
        "and the v2 answer is SSE-framed; got {:?}: {}",
        v2.content_type,
        v2.raw
    );

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
