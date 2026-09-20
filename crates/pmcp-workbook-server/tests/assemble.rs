//! Task 1 (Plan 95-02) — server-surface integration test for
//! [`pmcp_workbook_server::build_server`].
//!
//! Builds a [`pmcp::Server`] from the committed synthetic golden bundle
//! (`tax-calc@1.1.0`) through the real `build_server` path the binary uses and
//! asserts the assembled IN-PROCESS surface via the VERIFIED-stable
//! [`pmcp::Server::get_tool`] inspection API (the same one
//! `pmcp-sql-server`'s `tests/assemble.rs` uses): WBV2-04's ONE named compute
//! tool per output Table (`calculate_tax` / `estimate_refund` for this bundle)
//! plus the four workbook-wide meta tools (`explain` / `get_manifest` /
//! `diff_version` / `render_workbook`).
//!
//! The `workbook://` render resource's LIVE wire surface (`resources/list`) is
//! additionally asserted in `parity_workbook.rs` — the two tests together cover
//! both the in-process surface and the wire surface (Codex MEDIUM #4). The
//! built [`pmcp::Server`] exposes no public resource-handler accessor, so the
//! resource listability is proven over the wire where it is observable.
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-workbook-server --test assemble -- --test-threads=1
//! ```

use std::path::PathBuf;

use pmcp_workbook_server::{build_server, Args};

/// The served workbook tools the golden bundle must register: WBV2-04 fans out
/// ONE named compute tool per output Table (`Calculate_Tax` +
/// `Estimate_Refund`), and the four meta tools are workbook-wide. The generic
/// single `calculate` is RETIRED — see [`RETIRED_TOOL`].
const WORKBOOK_TOOLS: &[&str] = &[
    "calculate_tax",
    "estimate_refund",
    "explain",
    "get_manifest",
    "diff_version",
    "render_workbook",
];

/// The pre-WBV2-04 generic compute tool. Asserted ABSENT: this crate's tests
/// kept expecting it for the whole life of the fan-out change, so its absence
/// is pinned rather than merely implied by the list above.
const RETIRED_TOOL: &str = "calculate";

/// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
/// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR` so the test is
/// invariant to the cwd.
fn golden_bundle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

fn golden_args() -> Args {
    Args {
        bundle_dir: golden_bundle_dir(),
        bundle_id: None,
        http: "127.0.0.1:0".to_string(),
    }
}

#[test]
fn build_server_from_golden_registers_the_per_table_and_meta_tools() {
    let server = build_server(&golden_args()).expect("golden bundle assembles a server");

    for name in WORKBOOK_TOOLS {
        assert!(
            server.get_tool(name).is_some(),
            "built server must expose the '{name}' workbook tool"
        );
    }

    assert!(
        server.get_tool(RETIRED_TOOL).is_none(),
        "the retired generic '{RETIRED_TOOL}' tool must not come back"
    );
}

#[test]
fn build_server_with_matching_bundle_id_succeeds() {
    // The golden bundle's BUNDLE.lock bundle_id is "tax-calc".
    let args = Args {
        bundle_id: Some("tax-calc".to_string()),
        ..golden_args()
    };
    let server = build_server(&args).expect("matching --bundle-id assembles a server");
    assert!(
        server.get_tool("calculate_tax").is_some(),
        "the matching-id server still registers the workbook tools"
    );
}
