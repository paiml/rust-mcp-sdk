//! **`Mcp-Name` is required on name-bearing methods ONLY — proved over real HTTP.**
//!
//! Phase 118 **D-13** reverses the Phase-113 DRIFT-1 adjudication. pmcp used to
//! demand `Mcp-Name` on EVERY v2 request (empty for a method with no routing
//! name), deliberately stricter than the transport spec. The official
//! `@modelcontextprotocol/conformance` suite sends the header only for
//! name-bearing methods, so the stricter rule rejected effectively the whole v2
//! scored set at the header gate, before dispatch. The spec wins: the header is
//! now REQUIRED exactly where the method carries a routing name, OPTIONAL and
//! IGNORED everywhere else, and the strict name/body cross-check is unchanged
//! wherever a name exists.
//!
//! Phase 118 **D-18** widens "name-bearing" to the COMBINED table
//! (`crate::types::mrtr::name_bearing_key`), the one the client's `Mcp-Name`
//! EMITTER already resolves through. Before D-18 the client emitted a `Mcp-Name`
//! for `tasks/get` / `tasks/update` / `tasks/cancel` that the server neither
//! required nor cross-checked — an emitter/validator asymmetry that contradicted
//! D-13's own principle. This file is the wire-level proof of BOTH: the
//! relaxation, and the symmetry.
//!
//! # The remedy for a failure here
//!
//! Fix the gate. Never relax an assertion in this file to make it pass — each row
//! below is either a spec requirement or a security property (a stray, unvalidated
//! `Mcp-Name` must not be echoed back to the caller), and a "passing" run bought by
//! weakening a row proves nothing at all.
//!
//! # The rows
//!
//! | # | request | expected |
//! |---|---------|----------|
//! | 1 | v2 `tools/list`, NO `Mcp-Name` | 200 + `result` |
//! | 2 | v2 `tools/list`, `Mcp-Name: ""` | 200 + `result` (Phase-113 client compat) |
//! | 3 | v2 `tools/list`, stray `Mcp-Name` | 200 + `result`, response `Mcp-Name` EMPTY |
//! | 4 | v2 `tools/call`, NO `Mcp-Name` | rejected, message names `Mcp-Name` |
//! | 5 | v2 `tools/call`, mismatched `Mcp-Name` | rejected (cross-check retained) |
//! | 6 | v2 `tools/call`, correct `Mcp-Name` | 200 + `result` |
//! | 7 | v2 `tasks/get` / `tasks/update` / `tasks/cancel`, NO `Mcp-Name` | rejected, header-mismatch code |
//! | 8 | v2 `tasks/get`, `Mcp-Name` == `params.taskId` | NOT a header-mismatch rejection |
//! | 9 | v2 `tasks/get`, `Mcp-Name` != `params.taskId` | rejected, header-mismatch code |
//! | 10 | v2 `tools/list`, NO `Mcp-Method` | rejected |
//! | 11 | a v1 request | untouched (the era gate is per-request) |
//!
//! # Discipline
//!
//! * The "omit `Mcp-Name`" header vector is built by FILTERING what
//!   [`v2_headers`] returns, on `pmcp::shared::http_constants::MCP_NAME` — never
//!   by hand-writing a header name, which is how a typo turns a real assertion
//!   into a vacuous one.
//! * The protocol version comes from `common::v2::V2` / `V1` and the rejection
//!   code from `pmcp::types::protocol::error_codes::HEADER_MISMATCH`. No protocol
//!   literal is retyped here.
//! * The server is launched through the shared harness, which binds its socket
//!   before returning — so no timed readiness wait appears anywhere in this file.
//! * Failures speak in the `FAILURE MODE:` / `CONSEQUENCE:` / `WHAT TO DO:` shape
//!   used across `tests/ci_severance_gate_wiring.rs`, echoing the observed status
//!   and body.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server_with, extensions_capabilities, post, spawn_with, teardown, v1_body, v2_body,
    v2_headers, v2_headers_raw, Resp, V1, V2,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::shared::http_constants::{MCP_METHOD, MCP_NAME};
use pmcp::types::protocol::error_codes::HEADER_MISMATCH;
use serde_json::json;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

/// The tool the shared harness server registers, and therefore the only value a
/// `tools/call` can legitimately name.
const TOOL: &str = "search";

/// The three `tasks/*` methods D-18 brings under the cross-check.
///
/// A LITERAL list on purpose: this file's job is to prove the server's table
/// contains them, so deriving the list from that table would make the proof
/// circular.
const TASKS_METHODS: [&str; 3] = ["tasks/get", "tasks/update", "tasks/cancel"];

/// The task id every `tasks/*` row addresses.
const TASK_ID: &str = "task-under-test";

/// Launch the shared dual-version harness server.
///
/// `spawn_with` binds the listening socket BEFORE returning, which is the
/// readiness guarantee that makes a timed wait unnecessary (and forbidden).
async fn spawn() -> (SocketAddr, JoinHandle<()>) {
    spawn_with(
        build_v2_server_with("mcp-name-name-bearing", extensions_capabilities()),
        StreamableHttpServerConfig::default(),
    )
    .await
}

/// [`v2_headers`] with the `Mcp-Name` entry REMOVED.
///
/// Built by filtering the real header vector on the crate's own constant, so this
/// helper cannot drift from the header the server reads. Hand-writing `"mcp-name"`
/// here would silently stop removing anything the day the constant changed, and
/// every "absent header" row below would quietly become an "empty header" row.
fn without_mcp_name(method: &str, name: &str) -> Vec<(String, String)> {
    let headers = v2_headers(method, name);
    let filtered: Vec<(String, String)> = headers
        .into_iter()
        .filter(|(header, _)| !header.eq_ignore_ascii_case(MCP_NAME))
        .collect();
    assert_eq!(
        filtered.len(),
        2,
        "FAILURE MODE: filtering on MCP_NAME removed {} headers, not exactly one.\n\
         CONSEQUENCE: every 'absent Mcp-Name' row in this file would be testing the \
         wrong request shape.\n\
         WHAT TO DO: check that `v2_headers` still emits exactly three headers and that \
         MCP_NAME still matches the one it emits.",
        3 - filtered.len()
    );
    filtered
}

/// [`v2_headers`] with the `Mcp-Method` entry REMOVED, by the same discipline.
fn without_mcp_method(method: &str, name: &str) -> Vec<(String, String)> {
    v2_headers(method, name)
        .into_iter()
        .filter(|(header, _)| !header.eq_ignore_ascii_case(MCP_METHOD))
        .collect()
}

/// The JSON-RPC error code on a response, if it carries one.
fn error_code(response: &Resp) -> Option<i64> {
    response.body["error"]["code"].as_i64()
}

/// The JSON-RPC error message on a response, or the empty string.
fn error_message(response: &Resp) -> String {
    response.body["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Assert a response is a header-gate rejection.
fn assert_header_rejection(response: &Resp, what: &str) {
    assert_eq!(
        error_code(response),
        Some(i64::from(HEADER_MISMATCH)),
        "FAILURE MODE: {what} was NOT rejected by the v2 header gate (status {}).\n\
         CONSEQUENCE: a request whose Mcp-Name is missing or disagrees with the body \
         reaches dispatch, so an intermediary's routing decision and the server's own \
         decision can differ — the exact smuggling gap the cross-check exists to close.\n\
         WHAT TO DO: fix the gate in src/server/streamable_http_server.rs; do not relax \
         this assertion. Body: {}",
        response.status,
        response.raw
    );
}

/// Assert a response reached dispatch and carries a JSON-RPC `result`.
fn assert_served(response: &Resp, what: &str) {
    assert_eq!(
        response.status, 200,
        "FAILURE MODE: {what} did not reach dispatch (status {}).\n\
         CONSEQUENCE: this is the Phase-113 DRIFT-1 rule coming back — it rejects the \
         official conformance suite's v2 requests before any handler runs, which fails \
         effectively the whole v2 scored set.\n\
         WHAT TO DO: fix the gate per Phase 118 D-13; do not relax this assertion. \
         Body: {}",
        response.status, response.raw
    );
    assert!(
        !response.body["result"].is_null(),
        "FAILURE MODE: {what} answered 200 but carries no JSON-RPC result.\n\
         CONSEQUENCE: a 200 with an error payload would let this row pass while the \
         request never actually served.\n\
         WHAT TO DO: inspect the body: {}",
        response.raw
    );
}

// ===========================================================================
// 1-3 — a method with NO routing name.
// ===========================================================================

/// Rows 1-3: `tools/list` carries no routing name, so `Mcp-Name` is optional —
/// absent, empty and stray all serve, and the stray value is DISCARDED.
#[tokio::test]
async fn a_name_less_method_serves_with_absent_empty_or_stray_mcp_name() {
    let (addr, handle) = spawn().await;

    // Row 1: the header is not sent at all. This is the shape the official
    // conformance suite sends, and the shape D-13 exists to admit.
    let absent = post(
        addr,
        &without_mcp_name("tools/list", ""),
        &v2_body("tools/list", json!(1), json!({})),
    )
    .await;

    // Row 2: the Phase-113 client's spelling — present but empty.
    let empty = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(2), json!({})),
    )
    .await;

    // Row 3: an attacker-supplied value on a method that has no name to check it
    // against.
    let stray = post(
        addr,
        &v2_headers_raw("tools/list", "not-a-real-name"),
        &v2_body("tools/list", json!(3), json!({})),
    )
    .await;

    teardown(handle, ()).await;

    assert_served(&absent, "a v2 tools/list with NO Mcp-Name header");
    assert_served(&empty, "a v2 tools/list with an EMPTY Mcp-Name header");
    assert_served(&stray, "a v2 tools/list with a stray Mcp-Name header");

    // The D-20 sanitization, observable on the wire: the value the caller sent is
    // not reflected back.
    assert_eq!(
        stray.mcp_name.as_deref(),
        Some(""),
        "FAILURE MODE: the response echoed a stray Mcp-Name back to the caller.\n\
         CONSEQUENCE: an unvalidated, attacker-supplied string is reflected in a \
         response header, and may also have branched downstream logic — the gate is \
         supposed to discard it on a method with no routing name (T-118-53).\n\
         WHAT TO DO: restore the sanitization in require_v2_headers. Observed headers \
         carried Mcp-Name = {:?}",
        stray.mcp_name
    );
}

// ===========================================================================
// 4-6 — an MRTR name-bearing method.
// ===========================================================================

/// Rows 4-6: `tools/call` carries `params.name`, so `Mcp-Name` stays REQUIRED and
/// its value stays cross-checked.
#[tokio::test]
async fn an_mrtr_name_bearing_method_still_requires_and_cross_checks_mcp_name() {
    let (addr, handle) = spawn().await;
    let body = |id: i64| {
        v2_body(
            "tools/call",
            json!(id),
            json!({ "name": TOOL, "arguments": {} }),
        )
    };

    let missing = post(addr, &without_mcp_name("tools/call", TOOL), &body(10)).await;
    let mismatched = post(addr, &v2_headers("tools/call", "not-the-tool"), &body(11)).await;
    let correct = post(addr, &v2_headers("tools/call", TOOL), &body(12)).await;

    teardown(handle, ()).await;

    assert_header_rejection(&missing, "a v2 tools/call with NO Mcp-Name");
    assert!(
        error_message(&missing).contains("Mcp-Name"),
        "FAILURE MODE: the rejection for a missing Mcp-Name does not name Mcp-Name.\n\
         CONSEQUENCE: an operator reading the error cannot tell which header is \
         missing, which is exactly why Phase 118 split the catch-all message in two.\n\
         WHAT TO DO: keep the two distinct error strings. Message was: {:?}",
        error_message(&missing)
    );

    assert_header_rejection(
        &mismatched,
        "a v2 tools/call whose Mcp-Name disagrees with params.name",
    );
    assert_served(&correct, "a v2 tools/call with the correct Mcp-Name");
}

// ===========================================================================
// 7-9 — the tasks methods D-18 brings under the same rule.
// ===========================================================================

/// Row 7: all three `tasks/*` methods reject a MISSING `Mcp-Name`.
///
/// This is the D-18 half. Before Phase 118 the server's predicate read the
/// narrower MRTR-only table, so these three sailed through the gate while the
/// client dutifully emitted a header nobody checked.
#[tokio::test]
async fn every_tasks_method_rejects_a_missing_mcp_name() {
    let (addr, handle) = spawn().await;

    let mut responses = Vec::new();
    for (offset, method) in TASKS_METHODS.iter().enumerate() {
        let id = 20 + i64::try_from(offset).expect("offset fits in i64");
        let response = post(
            addr,
            &without_mcp_name(method, TASK_ID),
            &v2_body(method, json!(id), json!({ "taskId": TASK_ID })),
        )
        .await;
        responses.push((*method, response));
    }

    teardown(handle, ()).await;

    for (method, response) in &responses {
        assert_header_rejection(response, &format!("a v2 {method} with NO Mcp-Name"));
    }
}

/// Rows 8-9: the VALUE cross-check reaches the tasks methods too.
///
/// The agreeing leg deliberately asserts only that the request is not a
/// header-gate rejection. Whether this harness server implements `tasks/get` at
/// all is a different question, owned by a different plan, and asserting on it
/// here would couple this proof to the harness's task backend.
#[tokio::test]
async fn a_tasks_method_cross_checks_mcp_name_against_the_task_id() {
    let (addr, handle) = spawn().await;

    let agreeing = post(
        addr,
        &v2_headers("tasks/get", TASK_ID),
        &v2_body("tasks/get", json!(30), json!({ "taskId": TASK_ID })),
    )
    .await;
    let disagreeing = post(
        addr,
        &v2_headers("tasks/get", "a-different-task"),
        &v2_body("tasks/get", json!(31), json!({ "taskId": TASK_ID })),
    )
    .await;

    teardown(handle, ()).await;

    assert_ne!(
        error_code(&agreeing),
        Some(i64::from(HEADER_MISMATCH)),
        "FAILURE MODE: a tasks/get whose Mcp-Name EQUALS params.taskId was rejected by \
         the header gate.\n\
         CONSEQUENCE: the conformant client shape the ext-tasks routing rule mandates \
         is unserviceable — the server would reject exactly the header the spec tells \
         clients to send.\n\
         WHAT TO DO: the value comparison must succeed when the two agree. Body: {}",
        agreeing.raw
    );
    assert_header_rejection(
        &disagreeing,
        "a v2 tasks/get whose Mcp-Name disagrees with params.taskId",
    );
}

// ===========================================================================
// 10-11 — what did NOT change.
// ===========================================================================

/// Row 10: `Mcp-Method` is still mandatory on every v2 request, name-bearing or
/// not. D-13 relaxed exactly one header.
#[tokio::test]
async fn mcp_method_is_still_required_on_a_name_less_method() {
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &without_mcp_method("tools/list", ""),
        &v2_body("tools/list", json!(40), json!({})),
    )
    .await;
    teardown(handle, ()).await;

    assert_header_rejection(&response, "a v2 tools/list with NO Mcp-Method");
    assert!(
        !error_message(&response).contains("Mcp-Name"),
        "FAILURE MODE: the missing-Mcp-Method rejection still names Mcp-Name.\n\
         CONSEQUENCE: the operator is sent looking for a header that is no longer \
         universally required, which is precisely the confusion D-13 created and the \
         message split resolves.\n\
         WHAT TO DO: keep ERR_MISSING_V2_HEADERS free of Mcp-Name. Message was: {:?}",
        error_message(&response)
    );
}

/// Row 11, the negative control: the era gate is PER-REQUEST, so a v1 request on
/// the same server is untouched by any of this.
///
/// On a `--no-default-features --features full-v2` build there is no v1 half to
/// exercise, so the leg is skipped there rather than asserted vacuously.
#[tokio::test]
async fn a_v1_request_is_unaffected() {
    if !cfg!(feature = "v1-compat") {
        return;
    }
    let (addr, handle) = spawn().await;
    let response = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(50),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "mcp-name-control", "version": "1.0.0" },
            }),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_served(&response, "a v1 initialize carrying none of the v2 headers");
}

// ===========================================================================
// Smoke.
// ===========================================================================

/// A run that reports `running 0 tests` exits 0 and proves nothing — the 117-14
/// handoff rule. This always-true assertion makes a green run visibly different
/// from a vacuous one, and pins the two version constants apart.
#[test]
fn the_suite_ran() {
    assert_ne!(
        V1, V2,
        "the harness must carry two DISTINCT protocol versions or every era \
         assertion in this file is vacuous"
    );
    assert_eq!(TASKS_METHODS.len(), 3);
}
