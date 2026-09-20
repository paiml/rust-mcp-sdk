//! Schema gate for the expected-difference baseline
//! (`crates/mcp-tester/baselines/era-deltas.yaml`).
//!
//! # What this file gates, and what it does NOT
//!
//! The baseline is a SPEC ARTIFACT: it is the written statement of what
//! "dual-version" means for this SDK, and it is meant to be reviewed by a human
//! who does not read Rust. So this file gates its SCHEMA — every entry has a
//! well-shaped machine-facing observation id, a real citation, and a named
//! owner when it is provisional — and deliberately does NOT gate its CONTENT.
//! Deciding that `resultType` is expected on v2 is a spec question for a
//! reviewer, not an assertion for a test.
//!
//! Presence and UNIQUENESS of `id` / `observation_id` are not gated here: they
//! are `parse_baseline` contracts, so `baseline()` cannot return a violation of
//! them. See the note above section 1.
//!
//! # Why there is a non-vacuity floor
//!
//! A reader that silently parses NOTHING would make every era diff built on top
//! of this file pass over an empty set — the failure mode that turns a drift
//! detector into a rubber stamp. [`MINIMUM_DELTAS`] exists so that failure lands
//! HERE, loudly, instead of being laundered into a green dual-run report.
//!
//! # Measured boundary of `parse_baseline` (117-08, Task 2)
//!
//! `parse_baseline` accepts a syntactically valid baseline whose `deltas:` list
//! is EMPTY, and `serde_yaml` coerces a bare YAML scalar (`v1_protocol: 1`) into
//! a `String` field. Neither is a parser bug: the parser's contract is the four
//! rejections its doc comment enumerates (empty or duplicated `id` /
//! `observation_id`), and NON-VACUITY IS THIS FILE'S JOB, not the parser's — see
//! [`MINIMUM_DELTAS`]. Do not "fix" the parser to reject an empty list; that
//! would move the floor out of the one place whose failure message explains it.
//!
//! # If a test in this file fails
//!
//! The remedy is ALWAYS to fix the reader or restore the file. It is NEVER to
//! lower the floor, relax a shape rule, or delete an assertion. Every failure
//! message below states this inline, because the tempting fix is the wrong one.

use mcp_tester::era_diff::{load_baseline, parse_baseline, EraBaseline};
use std::path::PathBuf;

// ===========================================================================
// Named constants — the `tests/phase115_contract_bindings.rs:87-90,149` idiom
// ===========================================================================

/// The checked-in baseline this gate resolves, relative to the crate root.
/// Nothing else in the repo reads it yet; a later plan's dual-run comparison
/// will.
///
/// A violation means the file moved or was deleted. The remedy is NOT to point
/// this constant somewhere else — it is to restore the file at this path, since
/// `era_diff::default_baseline_path` derives the same location for the shipped
/// loader.
const BASELINE_FILE: &str = "baselines/era-deltas.yaml";

/// Floor on the parsed entry count. Fourteen deltas were seeded from the phase
/// research, each with a checked citation.
///
/// Falling below this means either the reader broke or entries were removed
/// without replacement. The remedy is NOT to lower this number: a smaller
/// baseline silently reclassifies real expected differences as findings (and,
/// at zero, makes every diff pass over an empty set).
const MINIMUM_DELTAS: usize = 14;

/// Floor on the length of an entry's `source` citation. Below this a value is a
/// label ("D-07", "spec"), not something a reviewer can go and check.
///
/// The remedy for a violation is NOT to shorten this constant — it is to write
/// the citation out, file and line.
const MIN_SOURCE_CHARS: usize = 10;

// ===========================================================================
// Loader
// ===========================================================================

/// Absolute path to the baseline, derived from `CARGO_MANIFEST_DIR` so no
/// machine-specific path is ever baked into this file.
fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BASELINE_FILE)
}

/// Load the checked-in baseline, failing with the file path when it cannot be
/// read or parsed.
fn baseline() -> EraBaseline {
    let path = baseline_path();
    load_baseline(&path).unwrap_or_else(|err| {
        panic!(
            "FAILURE MODE: the checked-in baseline at {} did not load: {err:#}\n\
             WHAT TO DO: fix the reader or restore the file; do not delete this gate.",
            path.display()
        )
    })
}

/// Does `text` name a phase by number (e.g. "Phase 114")? Hand-rolled rather
/// than pulled through `regex`, so this gate has no dependency of its own.
fn names_a_phase(text: &str) -> bool {
    text.split("Phase ")
        .skip(1)
        .any(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

// ===========================================================================
// 1. Ids are unique — ENFORCED BY THE PARSER, not here
// ===========================================================================
//
// `parse_baseline` rejects an empty or duplicated `id`/`observation_id`, and
// `baseline()` above goes through it — so a violation panics in the loader and
// every test in this file fails together, naming the offender. Re-asserting
// those four properties here would be unreachable code: the assertion arms
// could never run. The parser's own negative cases live in `era_diff.rs`'s
// `mod tests`; the checks below are the ones the parser deliberately does NOT
// make, because they are baseline-CONTENT rules rather than properties of
// arbitrary input.

// ===========================================================================
// 2. Observation ids are well shaped
// ===========================================================================

#[test]
fn every_delta_observation_id_is_well_shaped() {
    let baseline = baseline();

    for delta in &baseline.deltas {
        // Non-empty and unique are PARSER guarantees (see the note above), so
        // `trim()` here only normalises for the shape checks that follow.
        let observation_id = delta.observation_id.trim();

        assert!(
            observation_id.contains('.'),
            "FAILURE MODE: `observation_id` `{observation_id}` (entry `{}`) is not namespaced — it \
             has no `.`. Un-namespaced keys collide across surfaces.\n\
             WHAT TO DO: prefix it with its surface, e.g. `method.`, `header.`, `result.`, \
             `http.status.`.",
            delta.id
        );
        assert!(
            observation_id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_'),
            "FAILURE MODE: `observation_id` `{observation_id}` (entry `{}`) is not lowercase \
             dot-separated. It is a MACHINE-facing key, and a case or punctuation change silently \
             fails to join.\n\
             WHAT TO DO: use `[a-z0-9_.]` only; never rename one for readability.",
            delta.id
        );
    }
}

// ===========================================================================
// 3. Every entry is citable
// ===========================================================================

#[test]
fn every_delta_carries_a_nonempty_source() {
    let baseline = baseline();

    for delta in &baseline.deltas {
        let source = delta.source.trim();
        assert!(
            source.len() >= MIN_SOURCE_CHARS,
            "FAILURE MODE: entry `{}` in {BASELINE_FILE} has the citation {source:?}, shorter than \
             the {MIN_SOURCE_CHARS}-character floor — that is a label, not a citation, and a \
             reviewer cannot check it without reading Rust.\n\
             WHAT TO DO: write out file and line; do not lower the floor.",
            delta.id
        );
    }
}

// ===========================================================================
// 4. The parse is not vacuous
// ===========================================================================

#[test]
fn the_baseline_parse_is_not_vacuous() {
    let baseline = baseline();

    assert!(
        baseline.deltas.len() >= MINIMUM_DELTAS,
        "FAILURE MODE: parsed {} delta(s) from {BASELINE_FILE}, below the {MINIMUM_DELTAS} floor. A \
         reader that silently reads nothing makes every era diff built on this file pass over an \
         empty set, and every other test in this file pass vacuously.\n\
         WHAT TO DO: fix the reader or restore the file; do not lower the floor.",
        baseline.deltas.len()
    );

    assert_eq!(
        baseline.schema_version, 1,
        "FAILURE MODE: {BASELINE_FILE} declares schema_version {}, which this gate does not know \
         how to read.\n\
         WHAT TO DO: update this gate together with the schema; do not delete the check.",
        baseline.schema_version
    );
}

// ===========================================================================
// 5. The baseline is pinned to the SDK's own protocol constants
// ===========================================================================

#[test]
fn the_protocol_versions_match_the_sdk_constants() {
    let baseline = baseline();

    assert_eq!(
        baseline.v1_protocol,
        pmcp::LATEST_PROTOCOL_VERSION,
        "FAILURE MODE: {BASELINE_FILE} claims v1 is `{}` while the SDK's LATEST_PROTOCOL_VERSION is \
         `{}`. A baseline pinned to a version the SDK no longer speaks reports conformance against \
         a spec that moved.\n\
         WHAT TO DO: re-review every entry against the new version, then update the file; do not \
         hardcode the string here.",
        baseline.v1_protocol,
        pmcp::LATEST_PROTOCOL_VERSION
    );

    assert_eq!(
        baseline.v2_protocol,
        pmcp::types::protocol::version::PROTOCOL_VERSION_2026_07_28,
        "FAILURE MODE: {BASELINE_FILE} claims v2 is `{}` while the SDK's v2 constant is `{}`.\n\
         WHAT TO DO: re-review every entry against the new version, then update the file; do not \
         hardcode the string here.",
        baseline.v2_protocol,
        pmcp::types::protocol::version::PROTOCOL_VERSION_2026_07_28
    );
}

// ===========================================================================
// 6. Provisional entries name their owner
// ===========================================================================

#[test]
fn provisional_entries_name_their_owning_phase() {
    let baseline = baseline();

    let provisional: Vec<&mcp_tester::EraDelta> =
        baseline.deltas.iter().filter(|d| d.provisional).collect();

    assert!(
        !provisional.is_empty(),
        "FAILURE MODE: no entry in {BASELINE_FILE} is marked provisional, yet the v2 schema \
         is still settling and at least the task-surface rows are owned by a phase that is not \
         signed off. An all-final baseline turns an expected upstream churn into a mystery \
         failure.\n\
         WHAT TO DO: flag the entries whose owning phase has not signed off; do not delete this \
         check."
    );

    for delta in provisional {
        let note = delta.note.as_deref().unwrap_or_default();
        assert!(
            names_a_phase(note),
            "FAILURE MODE: provisional entry `{}` in {BASELINE_FILE} has the note {note:?}, which \
             names no phase. A provisional entry with no owner cannot be re-reviewed when its \
             phase signs off, so it silently rots.\n\
             WHAT TO DO: name the owning phase in the note (e.g. \"Phase 114 owns this\"); do not \
             clear the provisional flag to dodge this check.",
            delta.id
        );
    }
}

// ===========================================================================
// 7. The parser is total
// ===========================================================================

#[test]
fn the_parser_rejects_garbage_without_panicking() {
    let garbage = [
        "",
        "\u{0}\u{1}\u{2}",
        "deltas",
        "deltas: []",
        "schema_version: 1\ndeltas: not-a-list\n",
        // A delta missing the REQUIRED `observation_id` (and the rest of the
        // mandatory fields) — the shape a hand-edit produces most often.
        "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas:\n  - id: ERA-01\n",
        // A delta whose `observation_id` is present but empty — a documented
        // `parse_baseline` rejection, not merely a serde failure.
        "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas:\n  - id: ERA-01\n    \
         observation_id: \"\"\n    subject: s\n    v1: a\n    v2: b\n    kind: k\n    \
         source: c\n",
        "- - - -",
        "{{{{",
    ];

    for input in garbage {
        assert!(
            parse_baseline(input).is_err(),
            "FAILURE MODE: `parse_baseline` ACCEPTED the malformed input {input:?}. A parser that \
             accepts garbage yields a baseline whose entries were never really there.\n\
             WHAT TO DO: tighten the parser; do not weaken this list."
        );
    }
}
