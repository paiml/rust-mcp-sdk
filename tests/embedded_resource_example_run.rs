//! Phase 118.1-03 (CONF-04, G-2): **proof that the dual-conformance EXAMPLE is
//! RUN, not merely built** — a live `resources/read` on `test://static-binary`
//! answered with a `blob` on BOTH eras.
//!
//! # Why this file exists at all
//!
//! CLAUDE.md's ALWAYS requirements demand a working example per feature, and
//! `examples/s54_v2_dual_conformance.rs` is the example for the nine conformance
//! gaps. `make test-examples` only BUILDS examples (`Makefile:254-258` says so in
//! its own banner), so "the example demonstrates the fix" was, until this file,
//! an unenforced claim. A compiled example that never answers a request cannot
//! distinguish a closed gap from a gap whose fix was written into a handler that
//! is never reached.
//!
//! This leg therefore spawns the ALREADY-BUILT binary, drives a real HTTP request
//! against it on both eras, and records both answers as an artifact. It FAILS
//! rather than skipping when the binary is absent: a skip would silently restore
//! exactly the unenforced criterion the file exists to close.
//!
//! # Why the request is framed through `tests/common/v2.rs`
//!
//! The v2 era signal is three headers (`Mcp-Method`, `Mcp-Name`,
//! `MCP-Protocol-Version`) plus a reserved `params._meta`, and a hand-rolled
//! `curl` probe gets that contract wrong in ways that produce a red saying
//! nothing about the gap under test. `v1_body` / `v2_body` / `v2_headers_for`
//! build both framings through pmcp's own production tables, so a mis-framed
//! request is not a reachable failure mode here.
//!
//! # Port 8157, deliberately
//!
//! 8147 (`s47_v2_stateless_mrtr`), 8149 (this example's own default), 8150
//! (`s50`/`s51`) and 8151 (`scripts/run-conformance-suite.sh`) are held by
//! in-repo examples and scripts; 8155 is named as the fallback hint in this
//! example's own bind-failure message; 8153 is taken by plan 04's sibling leg,
//! which runs concurrently with this one under nextest.
//!
//! # Why both legs run before either is asserted
//!
//! Plan 118.1-02 measured the cost of the other order: an assertion on the v1 leg
//! panics before the v2 leg executes, and the v2 half of the claim then has zero
//! executed evidence. Here the two legs share ONE child process, so they cannot
//! be split into two tests; instead both round trips complete and the artifact is
//! written BEFORE the first assertion, so a failure on either era is diagnosed
//! against a recording of both.
// `v1-compat` is load-bearing, not decoration: this file's single test is a
// DUAL-era assertion, and its v1 leg opens a real session. A
// `--no-default-features --features full-v2` build mints no session at all, so
// `v1_open_session` panics and the leg reports a v1 failure about a build that
// deliberately has no v1. CI's v1 Severance Gate runs the AGGREGATE
// `cargo test -p pmcp --no-default-features --features full-v2`, which compiles
// and RUNS every test target, so a dual-era file must be gated out of it rather
// than left to fail in it. Same gate, same reason, as
// `tests/log_records_example_run.rs`.
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
use common::v2::{header, post, v1_body, v2_body, v2_headers_for, Resp};
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::protocol::LATEST_PROTOCOL_VERSION;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;

/// The example's compiled path, relative to the crate manifest.
const EXAMPLE_REL_PATH: &str = "debug/examples/s54_v2_dual_conformance";

/// See the module header for why this port and not another.
const BIND_ADDR: &str = "127.0.0.1:8157";

/// The URI whose handler G-2 changed from `Content::image` to
/// `Content::resource_with_blob`.
const BINARY_URI: &str = "test://static-binary";

/// Where the recorded answers land, for the SUMMARY to quote verbatim.
const ARTIFACT_REL_PATH: &str = "118.1-03-blob-response.json";

/// How long the child gets to bind its socket before the leg gives up.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the port gets to become free again after the child is killed.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// `resources/read` params for the binary resource.
fn read_params() -> Value {
    json!({ "uri": BINARY_URI })
}

/// Complete the v1 session handshake and return the minted session id.
///
/// The example runs on `StreamableHttpServerConfig::default()` — a LIVE
/// `session_id_generator`, deliberately, because that is the configuration whose
/// per-request era gate the whole example exists to demonstrate. So the v1 leg
/// has to do what a real v1 client does: `initialize` first, then echo
/// `Mcp-Session-Id` on every subsequent request. v2 has no session at all
/// (Phase 117 severed it), which is why only this leg needs the handshake.
async fn v1_open_session(addr: SocketAddr) -> String {
    let params = json!({
        "protocolVersion": LATEST_PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "embedded-resource-example-run", "version": "0.0.0" },
    });
    let response = post(addr, &[], &v1_body("initialize", json!(0), params)).await;
    assert_eq!(
        response.status, 200,
        "the v1 handshake must succeed before resources/read: HTTP {} {}",
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

/// Assert one era's answer carries a spec `BlobResourceContents`
/// (`schema.ts:1548`) for [`BINARY_URI`].
fn assert_blob_answer(era: &str, response: &Resp) {
    assert_eq!(
        response.status, 200,
        "{era}: the example must serve resources/read on {BINARY_URI}, got HTTP {}: {}",
        response.status, response.raw
    );

    let contents = response.body["result"]["contents"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "{era}: result.contents must be an array, body was {}",
                response.body
            )
        });
    assert_eq!(
        contents.len(),
        1,
        "{era}: exactly one ResourceContents element expected, got {}",
        response.body
    );

    let item = &contents[0];
    assert_eq!(
        item["uri"].as_str(),
        Some(BINARY_URI),
        "{era}: the answer is not for {BINARY_URI}: {item}"
    );
    assert_eq!(
        item["mimeType"].as_str(),
        Some("image/png"),
        "{era}: BlobResourceContents extends ResourceContents, which declares \
         mimeType (schema.ts:1514): {item}"
    );
    let blob = item["blob"].as_str().unwrap_or_else(|| {
        panic!(
            "{era}: G-2 is only closed if this element carries `blob` \
             (BlobResourceContents, schema.ts:1548). Element was: {item}"
        )
    });
    assert!(
        !blob.is_empty(),
        "{era}: `blob` is present but empty, so nothing was actually served: {item}"
    );
    assert!(
        item.get("type").is_none(),
        "{era}: ReadResourceResult.contents is ResourceContents[] and carries NO type \
         discriminator (schema.ts:1514-1560) — the D-01 boundary: {item}"
    );
}

#[tokio::test]
async fn the_dual_conformance_example_serves_a_real_blob_on_both_eras() {
    let (addr, mut guard) = spawn_example(EXAMPLE_REL_PATH, BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // BOTH legs run before EITHER is asserted — see the module header.
    let session = v1_open_session(addr).await;
    let v1 = post(
        addr,
        &[header(MCP_SESSION_ID, &session)],
        &v1_body("resources/read", json!(1), read_params()),
    )
    .await;
    let v2 = post(
        addr,
        &v2_headers_for("resources/read", &read_params()),
        &v2_body("resources/read", json!(2), read_params()),
    )
    .await;

    // The artifact carries the PARSED body as well as the raw text: the raw text
    // alone would be JSON-escaped inside this file, so a `"blob"` key would not
    // appear as `"blob"` to a reader (or a grep) of the artifact.
    let artifact = json!({
        "note": format!(
            "Live `resources/read` on {BINARY_URI}, served by \
             target/{EXAMPLE_REL_PATH} bound to {BIND_ADDR}. Phase 118.1-03, G-2."
        ),
        "v1": { "raw": v1.raw, "body": v1.body },
        "v2": { "raw": v2.raw, "body": v2.body },
    });
    let artifact_path = target_dir().join(ARTIFACT_REL_PATH);
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&artifact).expect("the artifact always serializes"),
    )
    .unwrap_or_else(|error| panic!("could not write {}: {error}", artifact_path.display()));

    assert_blob_answer("v1 (2025-11-25)", &v1);
    assert_blob_answer("v2 (2026-07-28)", &v2);

    // The v2 leg must genuinely have been served as v2, or "both eras" is a claim
    // about a request that was silently downgraded. `resultType` is Phase 112's
    // v2-only result envelope key.
    assert!(
        v2.raw.contains("resultType"),
        "the v2 leg carries no v2 result-envelope key, so it was not served as v2: {}",
        v2.raw
    );
    assert!(
        !v1.raw.contains("resultType"),
        "the v1 leg carries a v2 result-envelope key, so the per-request era gate \
         was bypassed: {}",
        v1.raw
    );

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
