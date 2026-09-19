//! `OAUTH_DARK_TESTS` must equal the set of oauth-gated test binaries — in
//! BOTH directions (issue #368).
//!
//! # Why this exists
//!
//! `make test-oauth` runs a HARDCODED list of seven test binaries under
//! `--features full,oauth`. The leg exists because those binaries contribute
//! ZERO tests under the `--features "full"` that every other test leg pins, so
//! they printed `running 0 tests` and exited 0 — green by omission, including
//! `tests/oauth_issuer_precedence.rs`, the RFC 8414 §3.3 issuer-identity
//! security fence.
//!
//! A hardcoded list has the same failure mode one level up. Add an eighth
//! oauth-gated test file, forget to list it, and it is dark again — with no
//! signal, because a missing entry is indistinguishable from a file that was
//! never meant to be covered. That is precisely the bug `test-oauth` was built
//! to prevent, so the list needs a guard of its own.
//!
//! # Oracle
//!
//! `specified`, not implicit. The list is mechanically derivable: a test binary
//! needs the leg exactly when its FILE-LEVEL inner attribute gates the whole
//! file on `feature = "oauth"`, because that is what makes it compile to an
//! empty binary without the feature. Both spellings in the tree count —
//! `#![cfg(feature = "oauth")]` and
//! `#![cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`.
//!
//! Files that gate on something ELSE are correctly excluded and must stay so:
//! `oauth_discovery_validation.rs` and `oauth_provider_discovery.rs` gate on
//! `feature = "http-client"`, which IS in `full`, so they already run. Name
//! alone is not the criterion — five further `oauth_*.rs` files carry no
//! file-level gate at all.
//!
//! # NOT feature-gated, on purpose
//!
//! This file has no `#![cfg]` header. A guard against green-by-omission that is
//! itself feature-gated would be the same bug one level up again: it must be
//! reached by `make test-integration` (`cargo test --test '*' --features full`),
//! which is the leg that does NOT enable `oauth`. Do not add a gate here.
//!
//! Modelled on `tests/ci_severance_gate_wiring.rs`, which pins an enumerated
//! constant against the script it describes — but bidirectionally, which that
//! one is not.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The repository root, derived from this crate's manifest directory.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Parse `OAUTH_DARK_TESTS := a \` + continuation lines out of the `Makefile`.
///
/// Deliberately a real parse of the assignment rather than a substring search:
/// a search would pass on a list that had been commented out or renamed.
fn declared_dark_tests(makefile: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    let start = makefile.find("\nOAUTH_DARK_TESTS :=").expect(
        "Makefile no longer declares `OAUTH_DARK_TESTS :=`. If `make test-oauth` was \
                 renamed or removed, update this guard deliberately — do not delete it, or \
                 oauth-gated tests silently go dark again (issue #368).",
    );

    // Consume the assignment: the value continues while lines end in a
    // backslash, and the first line after `:=` may itself be a continuation.
    let mut rest = &makefile[start + 1..];
    rest = &rest["OAUTH_DARK_TESTS :=".len()..];

    for raw in rest.lines() {
        let line = raw.trim();
        let continues = line.ends_with('\\');
        let token = line.trim_end_matches('\\').trim();

        if !token.is_empty() {
            names.insert(token.to_string());
        }

        if !continues {
            break;
        }
    }

    names
}

/// Scan `tests/*.rs` for files whose FILE-LEVEL inner attribute gates the whole
/// binary on `feature = "oauth"`.
///
/// Only the first `#![cfg(` in the file is considered, and only if it mentions
/// `feature = "oauth"` — an inner attribute must precede all items, so a
/// file-level gate can only appear there.
fn oauth_gated_test_binaries(tests_dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    let entries = std::fs::read_dir(tests_dir).expect("tests/ directory must be readable");

    for entry in entries {
        let path = entry.expect("tests/ entry must be readable").path();

        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };

        let Some(header) = source
            .lines()
            .find(|l| l.trim_start().starts_with("#![cfg("))
        else {
            continue;
        };

        if !header.contains(r#"feature = "oauth""#) {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("a .rs file has a UTF-8 stem")
            .to_string();

        names.insert(stem);
    }

    names
}

#[test]
fn oauth_dark_tests_equals_the_oauth_gated_binaries() {
    let root = repo_root();
    let makefile = std::fs::read_to_string(root.join("Makefile")).expect("Makefile must exist");

    let declared = declared_dark_tests(&makefile);
    let actual = oauth_gated_test_binaries(&root.join("tests"));

    // Non-vacuity first: an empty parse on either side would make the equality
    // below pass while proving nothing — the same class of false green this
    // whole leg exists to prevent.
    assert!(
        declared.len() >= 7,
        "parsed only {} entries from OAUTH_DARK_TESTS; the assignment parse has broken, so \
         this guard is no longer checking anything",
        declared.len()
    );
    assert!(
        actual.len() >= 7,
        "found only {} oauth-gated test binaries by scan; the header scan has broken, so this \
         guard is no longer checking anything",
        actual.len()
    );

    let missing_from_makefile: Vec<_> = actual.difference(&declared).cloned().collect();
    let stale_in_makefile: Vec<_> = declared.difference(&actual).cloned().collect();

    assert!(
        missing_from_makefile.is_empty(),
        "these test binaries are gated on `feature = \"oauth\"` but are NOT in \
         OAUTH_DARK_TESTS, so `make test-oauth` never runs them and they contribute \
         `running 0 tests` (exit 0) to every other leg — green by omission: {missing_from_makefile:?}\n\
         Add them to OAUTH_DARK_TESTS in the Makefile."
    );

    assert!(
        stale_in_makefile.is_empty(),
        "OAUTH_DARK_TESTS names these, but no `tests/<name>.rs` gates its whole file on \
         `feature = \"oauth\"`. Either the file was renamed/deleted, or its `#![cfg]` header \
         changed and it now runs under `--features full` like any other test: \
         {stale_in_makefile:?}"
    );
}

/// The exclusions are a CHECKED boundary, not an implicit allowlist.
///
/// These two files are named `oauth_*` and are deliberately absent from
/// `OAUTH_DARK_TESTS` because they gate on `http-client`, which `full` already
/// enables. If either is ever re-gated onto `oauth`, the test above starts
/// demanding it — this one documents why it is absent today so the omission
/// cannot be mistaken for the bug.
#[test]
fn the_http_client_gated_oauth_files_are_excluded_for_a_stated_reason() {
    let root = repo_root();

    for name in ["oauth_discovery_validation", "oauth_provider_discovery"] {
        let source = std::fs::read_to_string(root.join(format!("tests/{name}.rs")))
            .unwrap_or_else(|e| panic!("tests/{name}.rs must exist: {e}"));

        let header = source
            .lines()
            .find(|l| l.trim_start().starts_with("#![cfg("))
            .unwrap_or_else(|| panic!("tests/{name}.rs must carry a file-level #![cfg]"));

        assert!(
            header.contains(r#"feature = "http-client""#),
            "tests/{name}.rs is excluded from OAUTH_DARK_TESTS because it gates on \
             `http-client` (which `full` enables). Its header is now {header:?}. If it moved \
             onto `oauth`, add it to OAUTH_DARK_TESTS and update this test."
        );
    }
}
