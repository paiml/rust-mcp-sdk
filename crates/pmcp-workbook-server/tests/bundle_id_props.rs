//! Task 2 (Plan 95-02) — the ALWAYS property/fuzz coverage of the `--bundle-id`
//! fail-closed assertion (the one genuinely-new binary-owned seam, D-01).
//!
//! `build_server` runs an OPERATOR-CONVENIENCE identity guard layered on top of
//! the toolkit's already fail-closed integrity load: when `--bundle-id` is set,
//! the loaded bundle's `bundle_id` is compared against it and a mismatch returns
//! [`RunError::BundleIdMismatch`] BEFORE any tool is registered (no server is
//! ever constructed). This property test is the CLAUDE.md-mandated fuzz/property
//! coverage for that seam (T-95-07): every non-matching id MUST fail closed.
//!
//! The deeper boot-integrity fail-closed behavior itself is already covered by
//! the toolkit/runtime fuzz suite (Phase 91/92) — the binary adds no served
//! logic — so this test targets ONLY the binary-owned `--bundle-id` guard.
//!
//! `proptest` generates arbitrary `String` bundle ids (empty, whitespace,
//! unicode, very long) and asserts the invariant; the matching `"tax-calc"` id
//! and the `None` (no assertion) case are pinned as explicit examples.
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-workbook-server --test bundle_id_props
//! ```

use std::path::PathBuf;

use pmcp_workbook_server::{build_server, Args, RunError};
use proptest::prelude::*;

/// The loaded `bundle_id` of the committed synthetic golden bundle (its
/// `BUNDLE.lock` records `bundle_id = "tax-calc"`).
const GOLDEN_BUNDLE_ID: &str = "tax-calc";

/// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
/// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR`.
fn golden_bundle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

fn args_with_bundle_id(bundle_id: Option<String>) -> Args {
    Args {
        bundle_dir: golden_bundle_dir(),
        bundle_id,
        http: "127.0.0.1:0".to_string(),
    }
}

proptest! {
    /// FAIL-CLOSED INVARIANT (T-95-07): for ANY `--bundle-id` that is not the
    /// loaded golden id, `build_server` returns `Err(RunError::BundleIdMismatch)`
    /// and NEVER constructs a server. proptest-generated edge cases (empty,
    /// whitespace, unicode, very long) must all fail closed without panicking.
    #[test]
    fn non_matching_bundle_id_always_fails_closed(id in ".*") {
        // The generator can (rarely) produce the golden id itself — that case is
        // the success path, asserted explicitly below, so skip it here to keep
        // this property strictly about the mismatch invariant.
        prop_assume!(id != GOLDEN_BUNDLE_ID);

        let result = build_server(&args_with_bundle_id(Some(id.clone())));
        match result {
            Err(RunError::BundleIdMismatch { expected, actual }) => {
                prop_assert_eq!(expected, id, "the mismatch echoes the operator-typed id");
                prop_assert_eq!(
                    actual, GOLDEN_BUNDLE_ID,
                    "actual is the loaded BUNDLE.lock id"
                );
            },
            Ok(_) => prop_assert!(
                false,
                "a non-matching --bundle-id must NOT construct a server (fail-closed)"
            ),
            Err(other) => prop_assert!(
                false,
                "a non-matching --bundle-id must map to BundleIdMismatch, got {:?}",
                other
            ),
        }
    }
}

/// Explicit case: the matching `--bundle-id` (the golden's loaded id) succeeds
/// and registers the workbook tools (the guard is satisfied, assembly proceeds).
#[test]
fn matching_bundle_id_succeeds() {
    let server = build_server(&args_with_bundle_id(Some(GOLDEN_BUNDLE_ID.to_string())))
        .expect("matching --bundle-id must assemble a server");
    assert!(
        server.get_tool("calculate_tax").is_some(),
        "the matching-id server registers the workbook tools"
    );
}

/// Explicit case: `--bundle-id` absent (`None`) always succeeds — the assertion
/// is a guard, not a resolution input, so omitting it never blocks assembly.
#[test]
fn absent_bundle_id_succeeds() {
    let server = build_server(&args_with_bundle_id(None))
        .expect("absent --bundle-id must assemble a server");
    assert!(
        server.get_tool("calculate_tax").is_some(),
        "the no-assertion server registers the workbook tools"
    );
}
