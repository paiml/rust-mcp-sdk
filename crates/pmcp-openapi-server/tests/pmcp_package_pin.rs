//! `pmcp-package` dev-dep tripwire (PKG-04 / D-03), Phase 121 Plan 01,
//! re-pointed by Plan 04 to close CR-01.
//!
//! Two things are enforced here:
//!
//! 1. **Publish safety** (`pmcp_package_dev_dep_is_path_only`) — this crate's
//!    `[dev-dependencies].pmcp-package` entry must be the table form carrying a
//!    `path` and **no** version requirement.
//! 2. **The D-03 drift guarantee** (`pmcp_package_resolved_crate_is_on_the_0_3_line`)
//!    — `crates/pmcp-package`'s own `[package].version` must stay on the 0.3
//!    line, i.e. the crate the path dep actually resolves to is the one the
//!    round-trip E2E was written against. Phase 122 moved this from the 0.2 line
//!    to the 0.3 line when four source-breaking attestation-carriage changes
//!    landed; the E2E was re-run green against 0.3 as part of that move.
//!
//! # Why there is no version requirement to assert any more
//!
//! Cargo strips a `[dev-dependencies]` entry from the published manifest ONLY
//! when it carries no version requirement. An entry that carries one is
//! retained, and `cargo publish` must resolve it against crates.io while
//! preparing the manifest. `pmcp-package` is workspace-EXCLUDED and publishes
//! from `.github/workflows/release.yml` FIVE publish steps AFTER
//! `pmcp-openapi-server` — so that lookup cannot
//! succeed, and the publish step's fallback (which tolerates only
//! `already exists` in the output) fails the whole release job. That is CR-01.
//!
//! The rule generalizes: a `[dev-dependencies]` entry carrying BOTH `path` and
//! `version` is safe only while that version is already on crates.io. Four
//! crates here still carry `mcp-tester = { version = "0.8.0", path = ... }`
//! while publishing before `mcp-tester` — green only because 0.8.0 is published.
//! See deferred item D12.
//!
//! Re-adding a version requirement to make this file "assert a pin again" is
//! therefore the one change it exists to prevent. Test 1 is the guard.
//!
//! # Where D-03's guarantee went
//!
//! D-03 asked that a silent line-to-line drift fail loudly rather than resolve to
//! a version the E2E was never written against. That guarantee moved from the
//! requirement STRING to the RESOLVED CRATE, and got stronger in the move: a
//! caret requirement only constrains what would be accepted from a registry,
//! but a path dep never consults the registry at all. Asserting
//! `crates/pmcp-package`'s own version is a statement about what the E2E
//! genuinely compiled against; asserting a caret string was a statement about a
//! lookup that never happens.
//!
//! # What this tripwire does NOT cover
//!
//! It checks exactly ONE `pmcp-package` version emitter: this crate's own
//! `[dev-dependencies]` entry, plus the sibling crate it resolves to. It is
//! deliberately narrow, and a reader must not assume one file guards every
//! in-repo pin:
//!
//! - `cargo-pmcp`'s own `[dependencies].pmcp-package` entry is covered by the
//!   SIBLING tripwire at `cargo-pmcp/tests/pmcp_package_pin.rs`. This file
//!   cannot see it, and that file cannot see this one. Note that the sibling
//!   still asserts a caret requirement string, correctly: `cargo-pmcp`
//!   publishes AFTER `pmcp-package` in the release order, so a version
//!   requirement there is safe where one here is not.
//! - `cargo-pmcp/src/templates/agent.rs` EMITS a `pmcp-package` dependency line
//!   into every project created by `cargo pmcp agent new`. A stale requirement
//!   there is invisible to `cargo build` — the workspace resolves green while
//!   the scaffold ships broken. That emitter has its own drift test.
//! - `crates/pmcp-agent`, `crates/pmcp-team-servers` and
//!   `crates/pmcp-cfn-renderer` each carry their own manifest requirement.
//!   Those DO fail `cargo build` if left behind (the workspace cannot resolve),
//!   so the compiler is their tripwire.
//!
//! A future bump must move all of them together. Phase 124 still owns the
//! release-time half — publish order, the release ledger, and the crates.io
//! tag — plus the out-of-repo pmcp.run pin check.
//!
//! Note that this crate reads the `[dev-dependencies]` table, NOT
//! `[dependencies]`: the round-trip E2E is test-only, and nothing published
//! depends on `pmcp-package` from here. A copy of the sibling tripwire left
//! reading `dependencies` would panic on a CORRECT manifest.

/// `pmcp-openapi-server`'s own manifest, embedded at compile time
/// (`../Cargo.toml` is resolved relative to THIS test file, i.e.
/// `crates/pmcp-openapi-server/Cargo.toml`).
const OPENAPI_SERVER_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// The manifest of the crate the path dep actually resolves to, embedded at
/// compile time (`../../pmcp-package/Cargo.toml` is resolved relative to THIS
/// test file: `crates/pmcp-openapi-server/tests/` -> `crates/pmcp-package/`).
const PMCP_PACKAGE_CARGO_TOML: &str = include_str!("../../pmcp-package/Cargo.toml");

/// The major.minor line the round-trip E2E was written against.
const EXPECTED_VERSION_LINE: &str = "0.4.";

/// The path this crate's `pmcp-package` dev-dep must point at.
const EXPECTED_DEP_PATH: &str = "../pmcp-package";

/// CR-01: the `pmcp-package` dev-dep must be path-only, with no version
/// requirement of any kind.
///
/// The table form is required because the string shorthand
/// (`pmcp-package = "0.3"`) IS a bare version requirement by definition — there
/// is no shorthand that expresses a path.
#[test]
fn pmcp_package_dev_dep_is_path_only() {
    let manifest: toml::Value =
        toml::from_str(OPENAPI_SERVER_CARGO_TOML).expect("parse pmcp-openapi-server Cargo.toml");

    let dep = manifest
        .get("dev-dependencies")
        .and_then(|table| table.get("pmcp-package"))
        .expect("pmcp-openapi-server Cargo.toml has no [dev-dependencies].pmcp-package");

    let table = dep.as_table().unwrap_or_else(|| {
        panic!(
            "[dev-dependencies].pmcp-package must be the table form \
             `{{ path = \"{EXPECTED_DEP_PATH}\" }}`, found {dep:?}. The string shorthand IS a \
             bare version requirement, which is exactly the shape CR-01 forbids."
        )
    });

    let path = table
        .get("path")
        .and_then(toml::Value::as_str)
        .expect("[dev-dependencies].pmcp-package must carry a `path` key (it is a path dep)");
    assert_eq!(
        path, EXPECTED_DEP_PATH,
        "[dev-dependencies].pmcp-package must point at the in-repo sibling crate"
    );

    assert!(
        table.get("version").is_none(),
        "[dev-dependencies].pmcp-package must NOT carry a `version` key (CR-01). Cargo strips a \
         dev-dep from the published manifest only when it has no version requirement; one that \
         has a requirement is retained, and `cargo publish -p pmcp-openapi-server` must then \
         resolve it against crates.io while preparing the manifest. pmcp-package publishes five \
         steps after pmcp-openapi-server in release.yml, so \
         that lookup cannot succeed — and the step's fallback tolerates only \"already exists\", \
         so the whole release job dies. The `exclude` list does not save it: the failure is at \
         manifest-prep time, and excluding tests/ removes the consumers, not the manifest entry."
    );
}

/// PKG-04 / D-03: the crate this path dep resolves to must stay on the 0.3 line.
///
/// This replaces the former caret-requirement assertion. A path dep has no
/// version requirement left to refuse a drifting sibling, so the sibling's own
/// version field is the only thing that can still carry the guarantee.
#[test]
fn pmcp_package_resolved_crate_is_on_the_0_3_line() {
    let manifest: toml::Value =
        toml::from_str(PMCP_PACKAGE_CARGO_TOML).expect("parse pmcp-package Cargo.toml");

    let version = manifest
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .expect("crates/pmcp-package/Cargo.toml has no [package].version");

    assert!(
        version.starts_with(EXPECTED_VERSION_LINE),
        "crates/pmcp-package is at version {version}, off the expected \
         {EXPECTED_VERSION_LINE}x line (PKG-04 / D-03). roundtrip_e2e.rs was written against the \
         {EXPECTED_VERSION_LINE}x pmcp-package slot and package APIs, and this is a PATH \
         dependency — there is no \
         version requirement left to refuse a drifting sibling, so the E2E would silently \
         compile against whatever the sibling became. Move the E2E to the new line deliberately, \
         then update this constant."
    );
}
