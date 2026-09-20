//! Offline blocking contract test (170-08 Task 3).
//!
//! Validates that the CLI's two hand-written `package capture` GraphQL
//! operations (`SUBMIT_PACKAGE_CAPTURE_QUERY` / `GET_PACKAGE_CAPTURE_STATUS_QUERY`
//! in `src/deployment/targets/pmcp_run/graphql.rs`) — and the response structs
//! that deserialize their results (`CaptureInfo` / `CaptureStatus`) — have not
//! drifted from the platform-owned, vendored SDL contract at
//! `contracts/pmcp-run/capture-v1.graphql`.
//!
//! This is a pure offline/static check: no network access, no pmcp.run
//! credentials.
//!
//! It is executed by the `test-cargo-pmcp-integration` Makefile target, which
//! is chained into `test-all` and therefore into `make quality-gate`. That
//! target does not merely run this file — it asserts a NONZERO passed count for
//! this binary BY NAME, via `scripts/named-test-binary-count.awk`. So any drift
//! between the CLI's queries and the platform contract fails the SDK build
//! immediately rather than surfacing at runtime against a live server, and a
//! future change that stops these tests from executing fails the build too.
//!
//! History, recorded because this file is the model that
//! `tests/package_attestation_contract.rs` copies: until Phase 122 added that
//! target, this binary was reachable by NO gate in this repo, and the module
//! docs here claimed otherwise. `make test-cargo-pmcp` is
//! `cargo test -p cargo-pmcp --lib`, which still does not select this file —
//! `--lib` covers the library target only. Do not re-inherit the old claim
//! that this "runs in the normal `cargo test` workspace gate"; name the target
//! that actually runs it.

use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/capture-v1.graphql"
);

// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read capture-v1.graphql");
    Schema::parse_and_validate(sdl, "capture-v1.graphql").expect("vendored SDL is itself valid")
}

/// Both runtime queries must validate against the vendored contract.
#[test]
fn capture_ops_validate_against_contract() {
    let schema = schema();
    for (name, op) in [
        (
            "submit",
            cargo_pmcp::pmcp_run_graphql::SUBMIT_PACKAGE_CAPTURE_QUERY,
        ),
        (
            "status",
            cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_CAPTURE_STATUS_QUERY,
        ),
    ] {
        ExecutableDocument::parse_and_validate(&schema, op, format!("{name}.graphql"))
            .unwrap_or_else(|e| panic!("`{name}` op does not match capture-v1.graphql: {e}"));
    }
}

/// `status` must be a plain String in the contract — never a GraphQL enum
/// (an enum would make the online schema-vs-schema diff show permanent drift).
#[test]
fn status_field_is_string_not_enum() {
    let sdl = std::fs::read_to_string(SDL_PATH).unwrap();
    assert!(
        sdl.contains("status: String"),
        "status must be typed String in capture-v1.graphql"
    );
    assert!(
        !sdl.contains("enum CaptureStatusValue"),
        "status must not be an enum in v1"
    );
}

/// The response structs' GraphQL field names must exactly equal each op's
/// selection set (struct <-> query <-> schema all agree).
///
/// NOTE: this greps the hardcoded field-name lists below against the query
/// STRINGS, not the actual `CaptureInfo`/`CaptureStatus` Rust struct
/// definitions — it does NOT parse those structs. The query itself is
/// separately validated against the vendored SDL by
/// `capture_ops_validate_against_contract` (apollo-compiler, field-existence
/// + type checking). So this test is a drift sanity check on the selection
/// set, not a full struct-vs-schema proof: a struct-only serde `rename` with
/// no accompanying query change would go uncaught here.
#[test]
fn response_structs_match_selection_sets() {
    // CaptureInfo (submit) selects: captureId, status, createdAt
    for f in ["captureId", "status", "createdAt"] {
        assert!(
            cargo_pmcp::pmcp_run_graphql::SUBMIT_PACKAGE_CAPTURE_QUERY.contains(f),
            "CaptureInfo field `{f}` missing from submit selection set"
        );
    }
    // CaptureStatus (status) selects: id, status, message, errorCode,
    // divergentComponents, manifestDigest, updatedAt
    for f in [
        "id",
        "status",
        "message",
        "errorCode",
        "divergentComponents",
        "manifestDigest",
        "updatedAt",
    ] {
        assert!(
            cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_CAPTURE_STATUS_QUERY.contains(f),
            "CaptureStatus field `{f}` missing from status selection set"
        );
    }
}
