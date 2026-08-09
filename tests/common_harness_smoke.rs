//! Smoke tests for the shared Phase-113 v2 HTTP harness (`tests/common/v2.rs`).
//!
//! Six downstream plans (04, 06, 08, 10, 11, 13) build their requests from that one
//! harness, so a defect in it would look like a server defect in all six. These
//! tests prove the two properties every consumer depends on:
//!
//! 1. **Happy path** — a `tools/call` built by [`common::v2::v2_body`] +
//!    [`common::v2::v2_headers`] reaches a real handler and comes back 200 with a
//!    `result`.
//! 2. **The empty-`Mcp-Name` header rule** — a NAME-LESS method sends
//!    `Mcp-Name: ""` and is ACCEPTED. Since Phase 118 D-13 the header is required
//!    only on name-bearing methods, so what this pins is the BACKWARD-COMPATIBILITY
//!    half of that decision: a Phase-113-era client still emits the empty value,
//!    and `require_v2_headers` must keep discarding it rather than treating it as
//!    a name that disagrees with the body. The absent-header half is proved in
//!    `tests/v2_mcp_name_name_bearing_only.rs`.
//!
//! Two name-less methods exercise (2): `server/discover` and `tools/list`. Before
//! plan 04 closed finding D-113-B, `ListToolsRequest` carried no `_meta` field at
//! all, so `extract_request_meta_value` returned `None` for it and a v2
//! `tools/list` was rejected as "header claims v2 but `_meta` disagrees" before any
//! header rule was reached. [`tools_list_is_a_valid_v2_request`] is the regression
//! guard for that fix.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, post, spawn_default_config, spawn_stateless_config, v2_body,
    v2_body_with_caps, v2_discover_body, v2_headers, META_CLIENT_CAPABILITIES, REQUEST_META_KEY,
};
use serde_json::json;

#[tokio::test]
async fn harness_happy_path_tools_call_returns_a_result() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body(
            "tools/call",
            json!(1),
            json!({ "name": "search", "arguments": {} }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body: {}", response.raw);
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
    assert_eq!(response.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(response.mcp_name.as_deref(), Some("search"));
}

#[tokio::test]
async fn harness_empty_mcp_name_is_accepted_for_a_name_less_method() {
    // THE cross-plan header-rule tripwire: `Mcp-Name` is emitted on EVERY v2
    // request, with the EMPTY STRING for a method that carries no logical name.
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let headers = v2_headers("server/discover", "");
    assert_eq!(
        headers[1],
        ("mcp-name".to_string(), String::new()),
        "the harness must emit an EMPTY Mcp-Name, not omit the header"
    );
    let response = post(addr, &headers, &v2_discover_body(json!(2))).await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "an EMPTY Mcp-Name on a name-less v2 method must be ACCEPTED; body: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
    assert_eq!(response.mcp_method.as_deref(), Some("server/discover"));
}

/// REGRESSION GUARD for D-113-B (was a forward tripwire before plan 04).
///
/// A stateless v2 server (HTTP-01) has no handshake, so EVERY method must be able
/// to carry the per-request `_meta` signal — including the list-shaped ones.
/// Before plan 04, `ListToolsRequest` had no `_meta` field, `extract_request_meta_value`
/// returned `None`, and the fail-closed matrix rejected a v2 `tools/list` with 400
/// "header claims v2 but `_meta` protocolVersion disagrees".
#[tokio::test]
async fn tools_list_is_a_valid_v2_request() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(2), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "a v2 tools/list must be accepted (D-113-B); body: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
    assert_eq!(response.mcp_method.as_deref(), Some("tools/list"));
}

/// REGRESSION GUARD for D-113-A (was a forward tripwire before plan 04).
///
/// pmcp's typed request structs carry `#[serde(rename_all = "camelCase")]`, which
/// renamed the `_meta` FIELD to `meta` — so they emitted and accepted a spelling
/// the MCP spec does not define, and a conformant v2 client sending `_meta` got NO
/// era detection at all. Plan 04 pinned the field with
/// `#[serde(rename = "_meta", alias = "meta")]`: conformant on egress,
/// backward-compatible on ingress. This guard proves BOTH halves at the wire level
/// so the fix cannot silently regress under a future `rename_all` edit.
#[tokio::test]
async fn typed_requests_use_the_spec_meta_spelling() {
    let mut probe = pmcp::types::CallToolRequest::new("probe", json!({}));
    probe._meta = Some(pmcp::types::RequestMeta::new().with_meta("ns/key", json!("v")));
    let wire = serde_json::to_value(&probe).expect("probe serializes");

    assert_eq!(
        wire.get(REQUEST_META_KEY),
        Some(&json!({ "ns/key": "v" })),
        "egress must use the spec spelling `_meta`: {wire}"
    );
    assert!(
        wire.get("meta").is_none(),
        "the camelCase-renamed `meta` spelling must not be emitted: {wire}"
    );

    // Ingress accepts BOTH spellings (the `alias` half of the fix).
    for key in [REQUEST_META_KEY, "meta"] {
        let incoming = json!({ "name": "probe", "arguments": {}, key: { "ns/key": "v" } });
        let back: pmcp::types::CallToolRequest =
            serde_json::from_value(incoming).unwrap_or_else(|e| panic!("`{key}` must parse: {e}"));
        assert!(
            back._meta.is_some(),
            "the `{key}` spelling must deserialize into `_meta`"
        );
    }
}

#[tokio::test]
async fn harness_prompts_get_and_resources_read_have_real_handlers() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;

    let prompt = post(
        addr,
        &v2_headers("prompts/get", "greeting"),
        &v2_body(
            "prompts/get",
            json!(3),
            json!({ "name": "greeting", "arguments": {} }),
        ),
    )
    .await;
    let resource = post(
        addr,
        &v2_headers("resources/read", "mem://greeting"),
        &v2_body(
            "resources/read",
            json!(4),
            json!({ "uri": "mem://greeting" }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(prompt.status, 200, "body: {}", prompt.raw);
    assert!(prompt.body.get("result").is_some(), "{}", prompt.raw);
    assert_eq!(resource.status, 200, "body: {}", resource.raw);
    assert!(resource.body.get("result").is_some(), "{}", resource.raw);
}

#[tokio::test]
async fn harness_always_declares_client_capabilities() {
    // Codex Plan-02 HIGH #3: a harness that omitted `clientCapabilities` would make
    // every MRTR test accidentally exercise the -32021 undeclared-capability path.
    let body: serde_json::Value = serde_json::from_str(&v2_body(
        "tools/call",
        json!(1),
        json!({ "name": "search" }),
    ))
    .unwrap();
    let caps = &body["params"]["_meta"][META_CLIENT_CAPABILITIES];
    assert!(caps.get("elicitation").is_some(), "body: {body}");
    assert!(caps.get("sampling").is_some(), "body: {body}");
    assert!(caps.get("roots").is_some(), "body: {body}");

    // ...and the under-declaring escape hatch really under-declares.
    let narrow: serde_json::Value = serde_json::from_str(&v2_body_with_caps(
        "tools/call",
        json!(1),
        json!({ "name": "search" }),
        json!({ "roots": {} }),
    ))
    .unwrap();
    let caps = &narrow["params"]["_meta"][META_CLIENT_CAPABILITIES];
    assert!(caps.get("elicitation").is_none(), "body: {narrow}");
    assert!(caps.get("roots").is_some(), "body: {narrow}");
}

/// REGRESSION GUARD for HTTP-01 / D-113-C (was a forward tripwire before plan 04).
///
/// [`spawn_default_config`] builds a STATEFUL server (`session_id_generator` is
/// live). Before plan 04, a v2 `tools/call` without an `Mcp-Session-Id` was
/// rejected by the server-wide session gate, because `stateless()` is a
/// BUILD-TIME config and every session decision keyed off it rather than off the
/// per-request era (RESEARCH Pitfall 1). HTTP-01 makes the ERA, not the config,
/// the decider — so this stateful-config server now runs a v2 request
/// handshake-free and session-free.
#[tokio::test]
async fn stateful_config_runs_v2_session_free() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body(
            "tools/call",
            json!(1),
            json!({ "name": "search", "arguments": {} }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "a v2 request on a STATEFUL-config server must not demand a session; body: {}",
        response.raw
    );
    assert_eq!(
        response.mcp_session_id, None,
        "a v2 response must not carry Mcp-Session-Id; raw: {}",
        response.raw
    );
}
