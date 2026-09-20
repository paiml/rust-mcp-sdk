//! The TOOLKIT half of the cross-crate `${VAR}` / `env:VAR` grammar-parity
//! assertion (plan 120-05, T-120-28).
//!
//! `pmcp-package` duplicates this crate's [`parse_env_ref`] grammar as its own
//! `is_env_reference` predicate, because neither crate may depend on the other:
//! `pmcp-package` is the workspace-EXCLUDED leaf, and a toolkit dependency on
//! it inverts the layering (plan 120-04 machine-checks that it does not exist).
//! Sharing one implementation is therefore not available, so the duplication is
//! made ACCOUNTABLE instead: both crates assert the same checked-in
//! accept/reject table, and a row one side disagrees with fails a test in
//! whichever crate is wrong.
//!
//! The failure this prevents is not cosmetic. If the two parsers diverge, a
//! config packs cleanly and then fails to resolve at boot (or the reverse — a
//! config the runtime resolves is refused at pack). One parser treating
//! `${VAR` as a reference and the other as a literal is precisely how a
//! placeholder reaches the wire.
//!
//! The table LIVES in `pmcp-package` (`tests/golden_fixtures/`) so that crate's
//! standalone `make pmcp-package-gate` always runs its half with no path
//! dependency at all. This side resolves it as a sibling path.

use pmcp_server_toolkit::env_ref::parse_env_ref;
use std::path::{Path, PathBuf};

/// The sibling crate that owns the table.
fn pmcp_package_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../pmcp-package")
}

fn grammar_table_path() -> PathBuf {
    pmcp_package_crate_dir()
        .join("tests")
        .join("golden_fixtures")
        .join("env_ref_grammar_v1.tsv")
}

/// One row of the shared table.
struct Case {
    input: String,
    /// Column 2 — what `pmcp-package`'s `is_env_reference` answers. Carried
    /// here so this test can assert the columns stay coherent even though it
    /// cannot call that crate's predicate.
    package_accepts: bool,
    /// Column 3 — what `parse_env_ref` must return.
    expected: Option<String>,
}

/// Parse the table. Deliberately the same column contract the package side
/// reads: `<input> TAB <accept|reject> TAB <name | <EMPTYNAME> | empty>`, with
/// `<EMPTY>` in column 1 standing for the empty string.
fn parse_table(text: &str) -> Vec<Case> {
    text.lines()
        .map(|line| line.trim_end_matches(['\r', '\n']))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(
                fields.len() >= 2,
                "every row needs at least 2 tab-separated fields; row was: {line:?}"
            );
            let input = match fields[0] {
                "<EMPTY>" => String::new(),
                other => other.to_string(),
            };
            let package_accepts = match fields[1] {
                "accept" => true,
                "reject" => false,
                other => panic!("column 2 must be accept|reject, was {other:?}"),
            };
            let expected = match fields.get(2).copied().unwrap_or("") {
                "" => None,
                "<EMPTYNAME>" => Some(String::new()),
                name => Some(name.to_string()),
            };
            Case {
                input,
                package_accepts,
                expected,
            }
        })
        .collect()
}

/// `parse_env_ref` must agree with every row of the shared table.
///
/// Gated on the sibling CRATE DIRECTORY, in the same direction of reasoning as
/// `pmcp-package`'s fixture drift guard: an absent crate directory means a
/// published tarball with no sibling to read, so skip with a printed note; a
/// PRESENT crate directory with a MISSING table means the table moved, and that
/// must FAIL rather than quietly stop asserting the contract.
#[test]
fn parse_env_ref_agrees_with_the_shared_grammar_table_on_every_row() {
    let crate_dir = pmcp_package_crate_dir();
    if !crate_dir.is_dir() {
        println!(
            "skipping env-ref grammar parity: sibling crate {crate_dir:?} is absent \
             (published-tarball build)"
        );
        return;
    }
    let path = grammar_table_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the sibling crate exists but the shared grammar table {path:?} is missing ({e}) — \
             the table moved; update this path rather than skipping, because skipping here \
             silently drops the only check that the two implementations still agree"
        )
    });

    let cases = parse_table(&text);
    assert!(
        cases.len() >= 10,
        "the shared table must be non-trivial; it had {} rows",
        cases.len()
    );

    for case in &cases {
        let actual = parse_env_ref(&case.input).map(str::to_string);
        assert_eq!(
            actual, case.expected,
            "grammar drift on row {:?}: the shared table expects parse_env_ref -> {:?}, this \
             crate returned {:?}. This table is the contract with pmcp-package's \
             is_env_reference — change it only deliberately, and move BOTH implementations in \
             the same commit.",
            case.input, case.expected, actual
        );
    }

    // The two columns describe the same grammar from two angles; assert their
    // documented correspondence so a future row cannot be internally
    // inconsistent. An `accept` row must name a NON-EMPTY variable; a `reject`
    // row must name either nothing or the empty name.
    for case in &cases {
        if case.package_accepts {
            assert!(
                matches!(case.expected.as_deref(), Some(name) if !name.is_empty()),
                "an accept row must resolve to a non-empty variable name; row was {:?}",
                case.input
            );
        } else {
            assert!(
                matches!(case.expected.as_deref(), None | Some("")),
                "a reject row must resolve to None or the empty name; row was {:?}",
                case.input
            );
        }
    }
}
