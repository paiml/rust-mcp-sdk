//! Task 1 (Plan 95-02) — the binding **reference-parity** assertion for the
//! Shape A workbook binary.
//!
//! This test stands up the server **through the REAL `run_serving` binary
//! path** (NOT a hand-built / injected server) against the committed synthetic
//! golden bundle (`tax-calc@1.1.0`), drives it via the `mcp-tester` library
//! (a `[dev-dependencies]` parity harness ONLY — never a published dep), and:
//!
//! 1. constructs a programmatic [`Args`] (clap-free) pointing at the read-only
//!    golden bundle dir (no temp copy — the bundle is read-only and the binary
//!    never writes it) with an ephemeral `127.0.0.1:0` port,
//! 2. invokes the REAL [`pmcp_workbook_server::run_serving`] — the same path
//!    `pmcp-workbook-server --bundle-dir <golden>` takes (BundleSource select →
//!    fail-closed boot gate → `build_server` → `StreamableHttpServer`),
//! 3. polls `ServerTester::test_initialize()` with bounded backoff for
//!    readiness,
//! 4. FIRST asserts the FULL live wire surface — `tools/list` exposes every
//!    workbook tool AND `resources/list` exposes the `workbook://` render
//!    resource (Codex suggestion: assert the whole surface over the wire before
//!    invoking) — proving the in-process surface (`tests/assemble.rs`) actually
//!    reaches the wire,
//! 5. THEN invokes `get_manifest` (the curated no-input projection tool) and
//!    asserts a non-error result, exercising a live tool call end-to-end,
//! 6. `handle.abort()`s the listener deterministically (T-95-08).
//!
//! Only the SYNTHETIC golden bundle is used — zero customer material (D-05).
//!
//! Run with (single-threaded — ephemeral port + per-process env):
//! ```sh
//! cargo test -p pmcp-workbook-server --test parity_workbook -- --test-threads=1
//! ```

use std::time::Duration;

use mcp_tester::ServerTester;
use pmcp_workbook_server::{run_serving, Args};

/// The served workbook tools the golden bundle must expose over the wire:
/// WBV2-04's ONE named compute tool per output Table (`Calculate_Tax` +
/// `Estimate_Refund`) plus the four workbook-wide meta tools. The generic
/// single `calculate` is retired and asserted ABSENT below.
const WORKBOOK_TOOLS: &[&str] = &[
    "calculate_tax",
    "estimate_refund",
    "explain",
    "get_manifest",
    "diff_version",
    "render_workbook",
];

/// The pre-WBV2-04 generic compute tool, asserted absent from `tools/list`.
const RETIRED_TOOL: &str = "calculate";

/// The listable `workbook://` render-resource scheme root (the stable handle
/// `resources/list` advertises — concrete `workbook://render/<payload>` URIs
/// are minted per call by `render_workbook`).
const RENDER_RESOURCE_LIST_URI: &str = "workbook://render/";

fn golden_bundle_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

#[tokio::test]
async fn workbook_reference_parity_through_real_binary_path() {
    // (1)+(2) The REAL binary path: programmatic Args → run_serving (BundleSource
    // select → fail-closed boot gate → build_server → StreamableHttpServer).
    // Ephemeral loopback port; capture the REAL bound addr. The golden bundle is
    // read-only and the binary never writes it, so no temp copy is needed.
    let args = Args {
        bundle_dir: golden_bundle_dir(),
        bundle_id: None,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = run_serving(&args)
        .await
        .expect("REAL --bundle-dir binary path must assemble + serve the golden bundle");

    // (3) Construct the mcp-tester harness against the live HTTP server and poll
    // readiness via test_initialize() with backoff (it sets up the reusable pmcp
    // client the wire assertions need).
    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned HTTP server");

    let mut initialized = false;
    for attempt in 0..20u32 {
        let result = tester.test_initialize().await;
        if matches!(result.status, mcp_tester::report::TestStatus::Passed) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(
        initialized,
        "MCP initialize must succeed against the spawned server (readiness)"
    );

    // (4a) LIVE tools/list — every workbook tool must be visible over the
    // wire BEFORE invoking any of them.
    let tools = tester
        .list_tools()
        .await
        .expect("live tools/list must succeed");
    let tool_names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_str()).collect();
    for expected in WORKBOOK_TOOLS {
        assert!(
            tool_names.contains(expected),
            "live tools/list must expose the '{expected}' workbook tool; saw {tool_names:?}"
        );
    }
    assert!(
        !tool_names.contains(&RETIRED_TOOL),
        "the retired generic '{RETIRED_TOOL}' tool must not reappear on the wire; \
         saw {tool_names:?}"
    );

    // (4b) LIVE resources/list — the `workbook://` render resource must be
    // listable over the wire.
    let resources = tester
        .list_resources()
        .await
        .expect("live resources/list must succeed");
    let resource_uris: Vec<&str> = resources.resources.iter().map(|r| r.uri.as_str()).collect();
    assert!(
        resource_uris.contains(&RENDER_RESOURCE_LIST_URI),
        "live resources/list must expose the '{RENDER_RESOURCE_LIST_URI}' render resource; \
         saw {resource_uris:?}"
    );

    // (5) Drive a live tool call: get_manifest is the curated no-input projection
    // tool, so it needs no arguments and a successful (non-error) result proves
    // the full request → handler → response path end-to-end over HTTP.
    let manifest = tester
        .call_tool_raw("get_manifest", serde_json::json!({}))
        .await
        .expect("get_manifest must be callable over the wire");
    assert!(
        !manifest.is_error,
        "get_manifest must return a non-error result, got {manifest:?}"
    );

    // (6) Tear down the listener deterministically.
    handle.abort();
}
