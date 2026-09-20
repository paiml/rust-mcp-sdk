//! Phase 118.2-09 (CONF-10): **proof that the dual-conformance EXAMPLE puts real
//! `notifications/message` records ON THE WIRE** — not that it compiles, and not
//! that its source mentions `extra.log`.
//!
//! # The false green this closes
//!
//! `examples/s54_v2_dual_conformance.rs` "logged" three times per call for the
//! whole of Phase 118.1 and scored zero on `tools-call-with-logging`, because it
//! logged with `tracing::info!`. A `tracing` record goes to the process's
//! subscriber; a `notifications/message` goes to the client. The two are
//! indistinguishable in a diff, indistinguishable to `cargo build`, and
//! indistinguishable to `grep` unless you already know which one is which — so
//! the only assertion that can tell them apart is one that reads the socket.
//!
//! `make test-examples` BUILDS examples and does not run them (`Makefile:257`
//! says so in its own banner), and the official suite that would catch this
//! needs Node >= 22, which is not a thing every developer's machine has. This
//! file is the in-repo referee for the one property the official scenario
//! measures.
//!
//! # The three legs, and why none of them is redundant
//!
//! | Leg | Era | Tool | Claim |
//! |-----|-----|------|-------|
//! | A | v1 (2025-11-25) | `test_tool_with_logging` | at least 3 records on the SESSION stream — the referee's exact path |
//! | B | v2 (2026-07-28) | `test_logging_tool` with NO `_meta` log level | **zero** records — SEP-2575's prohibition |
//! | C | v2 (2026-07-28) | `test_logging_tool` WITH `_meta` log level | at least 1 record |
//!
//! Leg C exists so that leg B cannot pass vacuously. "No records appeared" is
//! satisfied just as well by a broken v2 sink, a mis-framed request or a tool
//! that was never reached, and a negative assertion with no positive control
//! beside it is the most comfortable false green in this repo's history.
//!
//! # Why leg A reads a raw socket
//!
//! A v1 session SSE stream never ends, so any reader that reads to EOF hangs —
//! the client-transport gap `tests/v2_sse_progress.rs` and
//! `tests/http_peer_roundtrip.rs` both work around the same way. The server half
//! is not in question here; the reader is.
//!
//! # Port 8159, deliberately
//!
//! 8147, 8149, 8150 and 8151 are held by in-repo examples and scripts; 8153
//! (`tests/completion_complete.rs`), 8155 (`tests/v2_sse_progress.rs`) and 8157
//! (`tests/embedded_resource_example_run.rs`) are held by sibling example-run
//! legs that nextest runs CONCURRENTLY with this one. A shared port would make
//! this file fail intermittently and for a reason that has nothing to do with
//! logging.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    feature = "v1-compat",
    not(target_arch = "wasm32")
))]

mod common;

use common::example_process::{
    spawn_example, target_dir, wait_until_listening, wait_until_released,
};
use common::v2::{header, post, post_with_accept, v1_body, v2_body, v2_headers_for};
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;

/// The example's compiled path, relative to the crate manifest.
const EXAMPLE_REL_PATH: &str = "debug/examples/s54_v2_dual_conformance";

/// See the module header for why this port and not another.
const BIND_ADDR: &str = "127.0.0.1:8159";

/// The tool `tools-call-with-logging` drives. Quoted from the scenario body.
const LOGGING_TOOL: &str = "test_tool_with_logging";

/// The tool `sep-2575-server-no-log-without-loglevel` drives. A DIFFERENT tool
/// with the OPPOSITE contract, which is why the example no longer answers both
/// from one match arm.
const DIAGNOSTIC_TOOL: &str = "test_logging_tool";

/// The notification method the referee counts, as a wire literal.
///
/// Deliberately NOT imported from `src/`: a constant shared with the
/// implementation asserts that the server agrees with itself, which it always
/// does. The spec spelling is what matters.
const LOG_METHOD: &str = "notifications/message";

/// The v2 per-request opt-in key (SEP-2575), as a wire literal for the same
/// reason as [`LOG_METHOD`]. It is private to
/// `src/server/streamable_http_server.rs` and cannot be imported anyway.
const LOG_LEVEL_META_KEY: &str = "io.modelcontextprotocol/logLevel";

/// The floor the pinned scenario asserts: `a.length < 3` is a FAILURE.
const EXPECTED_LOG_RECORDS: usize = 3;

/// `Accept` value that makes a v2 answer multi-frame eligible.
///
/// Load-bearing rather than decorative: the v2 log vehicle attaches only when
/// the request is multi-frame eligible, so a JSON-only `Accept` would produce
/// zero records on legs B *and* C and leg B would pass for the wrong reason.
const ACCEPT_SSE: &str = "application/json, text/event-stream";

/// How long the child gets to bind its socket before the leg gives up.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the port gets to become free again after the child is killed.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// Upper bound on the v1 stream read. NOT a synchronization device: the reader
/// stops as soon as it has seen the result frame, so the full window is only
/// ever spent on a failure.
const V1_COLLECT_WINDOW: Duration = Duration::from_secs(5);

/// Where the recorded frames land, for the SUMMARY to quote verbatim.
const ARTIFACT_REL_PATH: &str = "118.2-09-log-records.json";

/// Complete the v1 session handshake and return the minted session id.
async fn v1_open_session(addr: SocketAddr) -> String {
    let params = json!({
        "protocolVersion": LATEST_PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "log-records-example-run", "version": "0.0.0" },
    });
    let response = post(addr, &[], &v1_body("initialize", json!(0), params)).await;
    assert_eq!(
        response.status, 200,
        "the v1 handshake must succeed before the logging call: HTTP {} {}",
        response.status, response.raw
    );
    response.mcp_session_id.unwrap_or_else(|| {
        panic!(
            "the example minted no Mcp-Session-Id on initialize, so the v1 leg cannot \
             proceed. Response was: {}",
            response.raw
        )
    })
}

/// Read a v1 session's GET SSE stream RAW, holding it open across `trigger`.
///
/// Returns the bytes observed, chunked-transfer framing included. The assertion
/// downstream COUNTS occurrences rather than parsing, so a chunk boundary
/// landing mid-line cannot turn a real frame into a false negative — and the raw
/// bytes are recorded either way.
async fn read_v1_session_stream(
    addr: SocketAddr,
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
    // nowhere to route the records and drops them.
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

    while tokio::time::Instant::now() < deadline && !observed.contains("\"result\"") {
        match tokio::time::timeout_at(deadline, stream.read(&mut buffer)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(read)) => observed.push_str(&String::from_utf8_lossy(&buffer[..read])),
            Ok(Err(error)) => panic!("the v1 SSE stream errored: {error}"),
        }
    }
    observed
}

/// A v2 `tools/call` body for `tool`, optionally carrying the SEP-2575 opt-in.
///
/// Built by adding ONE key to the harness's own v2 body, so the authorized and
/// unauthorized requests differ in exactly that key and nothing else. Two
/// hand-written bodies could differ in a second way and neither leg would say so.
fn v2_call_body(tool: &str, id: Value, level: Option<&str>) -> String {
    let params = json!({ "name": tool, "arguments": {} });
    let base = v2_body("tools/call", id, params);
    let Some(level) = level else {
        return base;
    };
    let mut value: Value = serde_json::from_str(&base).expect("the harness body parses");
    value["params"]["_meta"][LOG_LEVEL_META_KEY] = json!(level);
    value.to_string()
}

#[tokio::test]
async fn the_dual_conformance_example_emits_log_records_on_the_wire() {
    let (addr, mut guard) = spawn_example(EXAMPLE_REL_PATH, BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // --- Leg A: v1, the referee's exact path. ------------------------------
    let session = v1_open_session(addr).await;

    // The scenario sets the level FIRST. Sending it here is not what makes the
    // records appear — the example emits at `info`, which clears the `info`
    // default on its own — but the leg must exercise the path the referee
    // exercises, including this call.
    let set_level = post(
        addr,
        &[header(MCP_SESSION_ID, &session)],
        &v1_body("logging/setLevel", json!(1), json!({ "level": "debug" })),
    )
    .await;

    let v1_frames = read_v1_session_stream(addr, &session, async {
        let call = post(
            addr,
            &[header(MCP_SESSION_ID, &session)],
            &v1_body(
                "tools/call",
                json!(2),
                json!({ "name": LOGGING_TOOL, "arguments": {} }),
            ),
        )
        .await;
        // 202, not 200, and that is the POINT: with a live session stream open
        // the reply is routed INTO that stream. A 200 here would mean the stream
        // was NOT open, every record was dropped, and the count below would be
        // measuring nothing.
        assert_eq!(
            call.status, 202,
            "the v1 logging call must be ACCEPTED into the open session stream: {}",
            call.raw
        );
    })
    .await;

    // --- Legs B and C: v2, the SEP-2575 pair. ------------------------------
    let params = json!({ "name": DIAGNOSTIC_TOOL, "arguments": {} });
    let unauthorized = post_with_accept(
        addr,
        ACCEPT_SSE,
        &v2_headers_for("tools/call", &params),
        &v2_call_body(DIAGNOSTIC_TOOL, json!(3), None),
    )
    .await;
    let authorized = post_with_accept(
        addr,
        ACCEPT_SSE,
        &v2_headers_for("tools/call", &params),
        &v2_call_body(DIAGNOSTIC_TOOL, json!(4), Some("info")),
    )
    .await;

    // EVERY leg is recorded BEFORE any is asserted: a panic on leg A would
    // otherwise leave legs B and C with zero executed evidence.
    let v1_records = v1_frames.matches(LOG_METHOD).count();
    let unauthorized_records = unauthorized.raw.matches(LOG_METHOD).count();
    let authorized_records = authorized.raw.matches(LOG_METHOD).count();
    let artifact = json!({
        "note": format!(
            "Live `tools/call` logging records served by target/{EXAMPLE_REL_PATH} \
             bound to {BIND_ADDR}. Phase 118.2-09, CONF-10."
        ),
        "v1_set_level": { "status": set_level.status, "raw": set_level.raw },
        "v1_session_stream": { "records": v1_records, "raw": v1_frames },
        "v2_unauthorized": { "records": unauthorized_records, "raw": unauthorized.raw },
        "v2_authorized": { "records": authorized_records, "raw": authorized.raw },
    });
    let artifact_path = target_dir().join(ARTIFACT_REL_PATH);
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&artifact).expect("the artifact always serializes"),
    )
    .unwrap_or_else(|error| panic!("could not write {}: {error}", artifact_path.display()));

    // Pitfall 8, in passing: the referee's `logging-set-level` scenario fails on
    // `Object.keys(r).length > 0`, so the result must be a LITERAL `{}` and not
    // an echo of the level. This leg answers 200 rather than 202 because it is
    // issued BEFORE the session stream is opened, so the reply rides the
    // one-shot SSE body instead of the session stream — a transport routing
    // detail, not a difference in the answer.
    assert_eq!(
        set_level.status, 200,
        "v1 `logging/setLevel` must be served: {}",
        set_level.raw
    );
    assert!(
        set_level.raw.contains(r#""result":{}"#),
        "v1 `logging/setLevel` must answer a literal empty object: {}",
        set_level.raw
    );
    assert!(
        v1_records >= EXPECTED_LOG_RECORDS,
        "v1 (2025-11-25): `{LOGGING_TOOL}` must put at least {EXPECTED_LOG_RECORDS} \
         `{LOG_METHOD}` frames on the session stream — that is the floor the pinned \
         `tools-call-with-logging` scenario asserts (`a.length < 3` fails it). Saw \
         {v1_records}. If this is 0, check whether the tool is emitting with \
         `tracing::info!` again, which reaches the operator and never the client. \
         Recorded at {}. Stream was:\n{v1_frames}",
        artifact_path.display()
    );

    assert!(
        authorized_records >= 1,
        "v2 (2026-07-28): `{DIAGNOSTIC_TOOL}` called WITH \
         `_meta[\"{LOG_LEVEL_META_KEY}\"]` must emit at least one `{LOG_METHOD}` frame \
         on the POST response body. Saw {authorized_records}. Without this the \
         zero-record assertion below proves nothing. Body was:\n{}",
        authorized.raw
    );
    assert_eq!(
        unauthorized_records, 0,
        "v2 (2026-07-28): SEP-2575 — a request that did NOT set \
         `_meta[\"{LOG_LEVEL_META_KEY}\"]` must receive NO `{LOG_METHOD}` frame at all \
         (\"If absent, the server MUST NOT send any notifications/message\"). Saw \
         {unauthorized_records}. Body was:\n{}",
        unauthorized.raw
    );

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
