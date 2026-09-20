//! Source-text drift gate over the THREE literal copies of the two keyword
//! lists that define the dialect walk's traversal rule.
//!
//! # The defect this exists to make loud
//!
//! `DATA_ONLY_KEYWORDS` and `SUBSCHEMA_MAP_KEYWORDS` are each written out three
//! times — in `src/server/output_validation.rs`, in `tests/property_tests.rs`
//! and in `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — and every one of those
//! three rustdocs states that the mirror is REQUIRED. Until this file, nothing
//! asserted that they agree (`115-REVIEW.md` WR-01). The two failure modes are
//! asymmetric and both are silent:
//!
//! 1. **The crate list gains an entry and a mirror does not.** The property and
//!    fuzz scans then hold the OLD rule against the NEW behaviour and become
//!    FALSE-POSITIVE GENERATORS against a correct normalizer. Measured, not
//!    theorised: `115-18` observed the fuzz target exiting 77 on an input the
//!    shipped walk correctly leaves untouched, and `115-17` observed the
//!    property suite failing its surgical-scope assertion in the same shape.
//! 2. **An entry is removed from all three in lockstep.** Every copy still
//!    agrees, so a mirror check passes — and coverage disappears with ZERO test
//!    failures. That is how `patternProperties` and `dependentSchemas` sat
//!    unexercised from `115-14` until `115-16` (`115-REVIEW.md` WR-02), and how
//!    `dependencies` was absent from all of them (CR-01).
//!
//! Mode 1 is caught by comparing the three copies to each other. **Mode 2 is
//! caught only by comparing them to something none of them is** — which is why
//! every assertion here runs against a DERIVATION-anchored expectation as well.
//!
//! # Why a source-text gate, and not a fourth derivation
//!
//! Three mechanisms were weighed, and the two rejected ones were rejected on
//! measurement rather than taste:
//!
//! - **Derive every copy from the `fuzz_support` seam re-export.** Rejected for
//!   the fuzz target. `115-18`'s Control D vs Control F measured the trade
//!   directly: with the fuzz copy INDEPENDENT and correct, it detects a
//!   crate-list omission (invariant 5 fires); with it DERIVED, it agrees with
//!   the defective walk and the target exits 0 on the very seed that reproduces
//!   the defect. Deriving would buy tidiness with a real detection capability.
//! - **A compiled equality test.** Adopted for `tests/property_tests.rs` in
//!   `115-17` (`keyword_lists_mirror_the_shipped_ones`) — that file can host a
//!   `#[test]` and can see the seam. It cannot cover the fuzz copy, which lives
//!   in a crate the workspace `exclude` array hides from every gate
//!   (`D-115-AB`) and which has no test harness at all.
//! - **A source-text gate.** This file, adopted as the COVERING mechanism. It
//!   reads all three files as TEXT, so it needs no feature flag and no
//!   `fuzz/`-crate build; it runs under a bare `cargo test` and therefore under
//!   `make quality-gate`.
//!
//! # No import of the crate under test
//!
//! This file deliberately declares no `use` of the SDK crate, and an acceptance
//! criterion greps for that. The whole point is to reach a copy that lives in a
//! crate no gate compiles: reading text needs no types from the crate under
//! test, and importing them would tempt a future edit into deriving the
//! expectation from the seam — which is exactly the mode `115-17` measured going
//! green on every negative control (`D-115-AI(4)`): *a fence's reachability must
//! not be derived from the same artifact as the rule it checks, even when that
//! artifact is gated.*
//!
//! The criterion's grep pattern is deliberately NOT quoted anywhere in this
//! file. A grep-shaped criterion over a file also constrains what that file may
//! say ABOUT the criterion — `D-115-AI(1)` and `D-115-AH(1)` are the same
//! collision one round earlier, and the first draft of this rustdoc reproduced
//! it a third time.

use std::fs;
use std::path::PathBuf;

/// The shipped definition. Both walkers read it; it is the rule.
const SRC_FILE: &str = "src/server/output_validation.rs";

/// The property suite's module-local restatement, inside
/// `schema_dialect_normalization_properties`.
const PROPERTY_FILE: &str = "tests/property_tests.rs";

/// The fuzz target's file-level restatement. This is the copy no other gate can
/// see (`D-115-AB`), and the reason this file exists at all.
const FUZZ_FILE: &str = "fuzz/fuzz_targets/fuzz_schema_draft_pin.rs";

/// The three copies, in the order their failure messages name them.
const COPIES: &[&str] = &[SRC_FILE, PROPERTY_FILE, FUZZ_FILE];

/// The subschema-map keywords, in the SHIPPED ORDER.
///
/// # This literal is a DERIVATION, not a hand-kept list
///
/// These six are the UNION, over the draft-04 / draft-06 / draft-07 / 2019-09 /
/// 2020-12 meta-schema documents `jsonschema` 0.49.2 ships offline, of the
/// keywords each meta-schema's own `.properties` map binds to an OBJECT-typed
/// schema whose `additionalProperties` REFERENCES THE META-SCHEMA ITSELF —
/// `{"$ref":"#"}` (draft-04/06/07), `{"$recursiveRef":"#"}` (2019-09),
/// `{"$dynamicRef":"#meta"}` (2020-12), or an `anyOf` carrying such a branch
/// (`dependencies`). That shape is precisely "a map whose keys are
/// author-chosen and whose values are subschemas".
///
/// Two keywords are EXCLUDED by that same criterion, and both exclusions are
/// part of the derivation rather than exceptions to it:
///
/// - `$vocabulary` — `additionalProperties` is `{"type": "boolean"}`. Its
///   values are vocabulary ENABLEMENT FLAGS, so there is no schema position to
///   descend into.
/// - `dependentRequired` — `additionalProperties` is a `stringArray` `$ref`.
///   Its values are LISTS OF PROPERTY NAMES. (It is the half of draft-07
///   `dependencies` that 2020-12 split off; `dependentSchemas` is the other
///   half, and only that half is a subschema map.)
///
/// The one-command re-derivation, and the full per-(file, keyword) table it
/// produces, are recorded in `115-16-SUMMARY.md` § *THE DERIVATION*. A guarded
/// `select((.value|type)=="object")` is load-bearing there: `draft7.json` binds
/// `default` and `const` to booleans, and an unguarded `.value.type` exits 5
/// with the error on stderr and NOTHING on stdout — which is that criterion's
/// own pass condition.
///
/// **Changing this literal is therefore a deliberate act that requires
/// re-running the derivation.** That friction is the point: it is what WR-01's
/// lockstep-removal mode has to get past, and the reason this gate is more than
/// a mirror check.
const EXPECTED_SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
    "dependencies",
];

/// The data-only keywords, in the SHIPPED ORDER.
///
/// A `$schema` string inside one of these is instance DATA — part of the value
/// a `const` pins, an `enum` alternative, a `default` a client may substitute,
/// or an `examples` entry. Rewriting it would change which instances conform,
/// which is a semantic corruption rather than a normalization.
///
/// This list is a DENY-list over an open keyword space and so cannot be
/// "completed" by derivation the way the subschema-map list can. It is pinned
/// here anyway, because the mode this file exists to catch is REMOVAL from all
/// three copies at once, and a removal from this list silently turns a data
/// payload back into a rewritable position. Round-1 `WR-04` (`OpenAPI`'s singular
/// `example`) is a candidate ADDITION and is booked, not silently absorbed.
const EXPECTED_DATA_ONLY_KEYWORDS: &[&str] = &["const", "enum", "default", "examples"];

// ===========================================================================
// Primitives — same shape as `tests/phase115_contract_bindings.rs`, so there is
// one convention in this repository for "an integration test that reads
// repository source files", not two.
// ===========================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Read at RUNTIME, deliberately not compile-time `include_str`: a keyword-list
/// edit must move these assertions without anyone remembering to rebuild.
fn read(relative: &str) -> String {
    let full = repo_root().join(relative);
    fs::read_to_string(&full).unwrap_or_else(|e| {
        panic!(
            "cannot read {relative}: {e}\n\
             FAILURE MODE: this gate is the only instrument that reads the fuzz target's copy \
             of the keyword lists at all — the `fuzz/` crate is in the workspace `exclude` array \
             and no other gate compiles it (D-115-AB).\n\
             WHAT TO DO: restore the file, or update the path constant in this test — do not \
             delete the assertion."
        )
    })
}

/// The part of `line` that is code, i.e. everything before a `//` that is not
/// inside a string literal.
///
/// Comment-stripping is load-bearing rather than cosmetic: `115-16` attached a
/// trailing `// draft-04..2019-09; values keyed by INSTANCE PROPERTY NAME
/// (D-115-03-C)` comment to the `dependencies` entry in all three copies. A
/// comment that MENTIONS another keyword must never be read as an entry.
fn code_before_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => i += 1,
            b'"' => in_string = !in_string,
            b'/' if !in_string && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {},
        }
        i += 1;
    }
    line
}

/// Every `"…"` literal in `code`, in order.
///
/// No escape handling: every entry in both lists is a plain ASCII keyword, and
/// an entry that needed an escape would fail the expectation assertion below
/// rather than pass silently.
fn string_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// The ordered string literals of `const <name>: &[&str] = &[ … ];` in `text`.
///
/// # The two guards, and why they are the difference between a gate and a decoration
///
/// - The definition must be found **exactly once**. A second definition, or
///   none, is a hard failure naming the file — not a silently-empty result.
/// - The extracted list must be **non-empty**. An extractor that quietly
///   returns `vec![]` for all three files makes them trivially equal, which is
///   the fail-open shape `D-115-AE` and `D-115-AA` each record in another
///   guise: a criterion whose failure mode is indistinguishable from its
///   success condition verifies nothing.
///
/// The marker deliberately includes `= &[`, so the `fuzz_support` seam
/// re-exports (`pub const SUBSCHEMA_MAP_KEYWORDS: &[&str] = super::…;`) are not
/// mistaken for a second definition.
fn extract_list(text: &str, path: &str, name: &str) -> Vec<String> {
    let marker = format!("const {name}: &[&str] = &[");
    let starts: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(&marker))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "expected EXACTLY ONE definition of `{name}` in {path}, found {}.\n\
         FAILURE MODE: this gate locates the list by its definition line. Zero matches means the \
         constant was renamed, moved or deleted and this gate is now asserting nothing about \
         that file; two or more means it would silently compare the first one it found.\n\
         WHAT TO DO: restore the definition, or update the marker in this test deliberately.",
        starts.len()
    );

    let mut out = Vec::new();
    for line in text.lines().skip(starts[0]) {
        let code = code_before_line_comment(line);
        out.extend(string_literals(code));
        if code.trim_end().ends_with("];") {
            break;
        }
    }
    assert!(
        !out.is_empty(),
        "extracted an EMPTY `{name}` from {path}.\n\
         FAILURE MODE: three empty lists are trivially equal, so an extractor that fails this way \
         turns this whole gate green while checking nothing.\n\
         WHAT TO DO: read the definition in that file — the literal shape this extractor \
         understands is `const {name}: &[&str] = &[ \"a\", \"b\" ];`, on one line or many."
    );
    out
}

/// Assert the three copies of `constant` agree with each other AND with the
/// derivation-anchored `expected` list.
fn assert_copies_agree(constant: &str, expected: &[&str]) {
    let extracted: Vec<(&str, Vec<String>)> = COPIES
        .iter()
        .map(|path| (*path, extract_list(&read(path), path, constant)))
        .collect();

    // ASSERTION 1 — the three copies agree as ORDERED sequences.
    //
    // Order equality is deliberate: a reorder is also a failure, so the three
    // stay literally comparable and `115-16`'s decision to append
    // `dependencies` LAST in every copy stays enforced.
    let (reference_path, reference) = &extracted[0];
    for (path, list) in &extracted[1..] {
        assert_eq!(
            list, reference,
            "`{constant}` has DRIFTED between {path} and {reference_path}.\n\
               {path}: {list:?}\n\
               {reference_path}: {reference:?}\n\
             CONSEQUENCE, which is why this is a hard failure and not a lint: a copy that LAGS \
             the crate turns the property and fuzz scans into FALSE-POSITIVE generators against \
             CORRECT behaviour — 115-18 measured the fuzz target exiting 77 on a document the \
             shipped walk rightly left untouched. A copy that LEADS the crate means the shipped \
             walk is skipping a real schema position.\n\
             WHAT TO DO: bring every copy onto the same ordered list — and if the SHIPPED list \
             is the one that changed, re-run the derivation recorded above before changing \
             `EXPECTED_{constant}` in this file."
        );
    }

    // ASSERTION 2 — each copy equals the DERIVATION-anchored expectation.
    //
    // This is what makes the file more than a mirror check: assertion 1 passes
    // when all three drift in lockstep, and a lockstep removal deletes coverage
    // with zero other test failures.
    let expected_owned: Vec<String> = expected.iter().map(|k| (*k).to_string()).collect();
    for (path, list) in &extracted {
        assert_eq!(
            *list, expected_owned,
            "`{constant}` in {path} disagrees with the DERIVATION-anchored expectation.\n\
               found:    {list:?}\n\
               expected: {expected_owned:?}\n\
             CONSEQUENCE: if all three copies changed together, assertion 1 above PASSED and \
             this is the only instrument that fired. A lockstep removal deletes coverage with \
             zero other test failures — `patternProperties` and `dependentSchemas` sat \
             unexercised from 115-14 until 115-16 exactly that way (115-REVIEW.md WR-02), and \
             `dependencies` was missing from every copy at once (CR-01).\n\
             WHAT TO DO: re-run the meta-schema derivation documented on \
             `EXPECTED_SUBSCHEMA_MAP_KEYWORDS` in this file and change the expectation only if \
             the derivation itself produces a different union. Editing the expectation to match \
             the code is the failure this gate exists to prevent."
        );
    }
}

// ===========================================================================
// The gate
// ===========================================================================

/// All three copies of `SUBSCHEMA_MAP_KEYWORDS` agree, and all three equal the
/// six keywords the meta-schema derivation produces.
#[test]
fn subschema_map_keywords_agree_across_all_three_copies() {
    assert_copies_agree("SUBSCHEMA_MAP_KEYWORDS", EXPECTED_SUBSCHEMA_MAP_KEYWORDS);
}

/// All three copies of `DATA_ONLY_KEYWORDS` agree, and all three equal the four
/// keywords whose values are instance data.
#[test]
fn data_only_keywords_agree_across_all_three_copies() {
    assert_copies_agree("DATA_ONLY_KEYWORDS", EXPECTED_DATA_ONLY_KEYWORDS);
}
