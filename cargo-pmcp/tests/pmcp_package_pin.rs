//! `pmcp-package` version-pin tripwire (CLI-04 / D-04b), originally Phase 110
//! Plan 05, moved to the 0.2 line by Phase 120 (D-08/D-09) and to the 0.3 line
//! by Phase 122 (four source-breaking attestation-carriage changes).
//!
//! cargo-pmcp MUST pin `pmcp-package` at the caret `"0.3"` string (NOT `=0.3.0`,
//! NOT `0.3.0`) so a portable `.pmcp` package produced by any published
//! `pmcp-package = 0.3.x` stays inspectable by this CLI. This test parses
//! cargo-pmcp's OWN `Cargo.toml` and asserts the exact version-req string,
//! mirroring the `const + include_str! + assert_eq!` drift-test pattern used by
//! `src/templates/workbook_server.rs`.
//!
//! # What this tripwire does NOT cover
//!
//! It checks ONE of the in-repo `pmcp-package` version emitters: cargo-pmcp's
//! own manifest. Phase 120 found six, and the two this file cannot see are the
//! ones that bit:
//!
//! - `cargo-pmcp/src/templates/agent.rs` EMITS its own `pmcp-package` dependency
//!   line into every project created by `cargo pmcp agent new`. A stale
//!   requirement there is invisible to `cargo build` — the workspace resolves
//!   green while the scaffold ships broken. That emitter now reads a named
//!   `PMCP_PACKAGE_VERSION_REQ` constant guarded by its own drift test
//!   (`emitted_package_requirement_matches_workspace_major_minor_line`), which
//!   is the tripwire for that path.
//! - `crates/pmcp-agent`, `crates/pmcp-team-servers` and
//!   `crates/pmcp-cfn-renderer` each carry their own manifest requirement.
//!   Those DO fail `cargo build` if left behind (the workspace cannot resolve),
//!   so the compiler is their tripwire.
//!
//! A future bump must move all of them together. Phase 124 still owns the
//! release-time half — publish order, the release ledger, and the crates.io
//! tag — plus the out-of-repo pmcp.run pin check.

/// cargo-pmcp's own manifest, embedded at compile time (`../Cargo.toml` is
/// resolved relative to THIS test file, i.e. `cargo-pmcp/Cargo.toml`).
const CARGO_PMCP_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// The exact version-requirement string cargo-pmcp must declare.
const EXPECTED_PIN: &str = "0.4";

/// Extract the version-requirement string for a `[dependencies]` entry, handling
/// BOTH the `name = "x.y"` shorthand and the `name = { version = "x.y", .. }`
/// table form (cargo-pmcp uses the table form with a `path`).
fn dependency_version_req(manifest: &toml::Value, name: &str) -> String {
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("cargo-pmcp Cargo.toml has no [dependencies].{name}"));
    match dep {
        // `pmcp-package = "0.3"` shorthand.
        toml::Value::String(s) => s.clone(),
        // `pmcp-package = { version = "0.3", path = "..." }` table form.
        toml::Value::Table(_) => dep
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("[dependencies].{name} table has no `version` key"))
            .to_string(),
        other => panic!("[dependencies].{name} has unexpected shape: {other:?}"),
    }
}

/// CLI-04 / D-04b: the `pmcp-package` pin must be exactly the caret `"0.3"`.
///
/// The caret matters. `=0.3.0` would refuse every later 0.3.x patch, and a
/// fully-qualified `0.3.0` reads like an exact pin to a human even though Cargo
/// treats it as a caret — both forms are rejected here so the manifest says
/// what it means.
#[test]
fn pmcp_package_pin_is_the_expected_caret_line() {
    let manifest: toml::Value =
        toml::from_str(CARGO_PMCP_CARGO_TOML).expect("parse cargo-pmcp Cargo.toml");
    let req = dependency_version_req(&manifest, "pmcp-package");
    assert_eq!(
        req, EXPECTED_PIN,
        "pmcp-package pin must be the caret \"{EXPECTED_PIN}\" (CLI-04 / D-04b); \
         do NOT use `=0.3.0` or a fully-qualified `0.3.0`"
    );
}
