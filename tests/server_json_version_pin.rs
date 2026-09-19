//! `server.json`'s `version` must equal the root crate's version.
//!
//! # The defect this fences
//!
//! `release.yml`'s `publish-mcp` job runs `./mcp-publisher publish`, which reads
//! `server.json` from the checkout and publishes THAT version to the MCP
//! Registry. Nothing bumped it, so it sat at `1.10.3` while `pmcp` moved to
//! `2.20.3`, and every tag since re-published an already-published version:
//!
//! ```text
//! Logging in with github-oidc... ✓ Successfully logged in
//! Error: publish failed: server returned status 400:
//!   {"detail":"Failed to publish server",
//!    "errors":[{"message":"invalid version: cannot publish duplicate version"}]}
//! ```
//!
//! Measured on tag `v2.20.3` (run 35456958817, job 105935595972). Note the
//! OIDC login SUCCEEDS — the failure is the stale version, and repo notes that
//! blamed OIDC were wrong about it for several releases.
//!
//! # Why a test, and not a release-time check
//!
//! The two files were in lockstep once (`dcfc9dc1 chore: bump versions for
//! v1.10.3 release` set both to `1.10.3`, and `773cd834 fix: ... sync version`
//! repaired an earlier drift), but nothing ENFORCED it, so the third drift went
//! unnoticed through five releases. `server.json` had zero references anywhere
//! in the repo — no test, no script, no Makefile leg.
//!
//! A release-time check would be too late: the tag is already pushed and the
//! crates are already published by the time `publish-mcp` runs, so the only
//! remedy left is a follow-up tag. Failing `cargo test` instead makes the bump
//! a precondition of the version change that requires it.
//!
//! This is the same defect class as `PMCP_VERSION` in
//! `cargo-pmcp/src/templates/workbook_server.rs` — a hand-maintained version
//! emitter — and it is fenced the same way: parse both sides, compare exactly,
//! and name the fix in the failure message.
//!
//! # Oracle
//!
//! `specified`, not implicit. The registry rejects a duplicate version, and the
//! established convention in this repo is that `server.json` carries the `pmcp`
//! crate version verbatim (`dcfc9dc1`: both `1.10.3`). Equality is therefore the
//! contract, not an approximation of one.
//!
//! # NOT feature-gated, on purpose
//!
//! No `#![cfg]` header: this must be reached by `make test-integration`
//! (`cargo test --test '*' --features full`). A guard that only runs under a
//! feature nothing enables is the bug it exists to prevent, one level up.

use std::fs;

/// The workspace-root manifest, whose `[package] version` is the source of truth.
const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// The MCP Registry descriptor that `mcp-publisher publish` reads.
const SERVER_JSON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/server.json");

/// The root crate version, PARSED rather than pattern-matched.
///
/// Panics naming the file on a read or parse failure: a manifest this test
/// cannot read must be a loud failure, not a skipped comparison.
fn crate_version() -> String {
    let text =
        fs::read_to_string(MANIFEST).unwrap_or_else(|e| panic!("cannot read {MANIFEST}: {e}"));
    let parsed: toml::Value =
        toml::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {MANIFEST} as TOML: {e}"));
    parsed
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{MANIFEST} has no [package] version"))
        .to_string()
}

/// The `server.json` document, parsed as JSON for the same reason.
fn server_json() -> serde_json::Value {
    let text = fs::read_to_string(SERVER_JSON)
        .unwrap_or_else(|e| panic!("cannot read {SERVER_JSON}: {e}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {SERVER_JSON} as JSON: {e}"))
}

#[test]
fn server_json_version_matches_the_crate_version() {
    let expected = crate_version();
    let doc = server_json();
    let actual = doc
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{SERVER_JSON} has no string `version` field"));

    // Non-vacuity: an empty string on either side would satisfy a careless
    // comparison while proving nothing about either file.
    assert!(
        !expected.is_empty() && !actual.is_empty(),
        "empty version string — crate `{expected}`, server.json `{actual}`; this guard is \
         not comparing anything"
    );

    assert_eq!(
        actual, expected,
        "server.json declares version `{actual}` but the pmcp crate is `{expected}`.\n\
         \n\
         `release.yml`'s publish-mcp job publishes server.json's version to the MCP \
         Registry, which REFUSES a version it already has:\n\
         \x20   400 invalid version: cannot publish duplicate version\n\
         \n\
         So a stale server.json makes every release fail that job — silently, because the \
         crates.io publish succeeds independently and the run's only red job is this one.\n\
         \n\
         FIX: set \"version\" in server.json to `{expected}`, in the SAME commit as the \
         crate bump."
    );
}

#[test]
fn server_json_still_declares_the_registry_identity_the_publisher_expects() {
    // The version pin above is worthless if the document it lives in stops being
    // the one `mcp-publisher` publishes. `name` is the registry's primary key, so
    // a change here is a different server, not a new version of this one — which
    // would make the duplicate-version error above impossible for a reason that
    // has nothing to do with the pin.
    let doc = server_json();

    let name = doc
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{SERVER_JSON} has no string `name` field"));
    assert_eq!(
        name, "io.github.paiml/pmcp",
        "server.json `name` changed to `{name}`. That renames the registry entry rather \
         than versioning it. If the rename is deliberate, update this test in the same \
         commit and expect the first publish under the new name to create a fresh entry."
    );

    assert!(
        doc.get("$schema").and_then(|v| v.as_str()).is_some(),
        "server.json lost its `$schema` — mcp-publisher validates against it, and a \
         document that fails validation fails the publish for a reason the version pin \
         cannot describe."
    );
}
