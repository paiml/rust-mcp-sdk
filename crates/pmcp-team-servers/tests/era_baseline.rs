//! Schema gate for the expected-difference baseline
//! (`crates/pmcp-team-servers/baselines/era-deltas.yaml`).
//!
//! # What this file gates, and what it does NOT
//!
//! The baseline is a SPEC ARTIFACT: it is the written statement of what
//! "dual-version" means for the Phase-118 era target, and it is meant to be
//! reviewed by a human who does not read Rust. So this file gates its SCHEMA —
//! every entry has a well-shaped machine-facing observation id, a real
//! citation, a named owner when it is provisional, and a probe on the other
//! side of the join — and deliberately does NOT gate its CONTENT. Deciding that
//! `resultType` is expected on v2 is a spec question for a reviewer, not an
//! assertion for a test.
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
//! HERE, loudly, instead of being laundered into a green comparison report.
//!
//! # Measured boundary of `parse_baseline`
//!
//! `parse_baseline` accepts a syntactically valid baseline whose `deltas:` list
//! is EMPTY, and `serde_yaml` coerces a bare YAML scalar (`v1_protocol: 1`) into
//! a `String` field. Neither is a parser bug: the parser's contract is the four
//! rejections its doc comment enumerates (empty or duplicated `id` /
//! `observation_id`), and NON-VACUITY IS THIS FILE'S JOB, not the parser's — see
//! [`MINIMUM_DELTAS`]. Do not "fix" the parser to reject an empty list; that
//! would move the floor out of the one place whose failure message explains it.
//!
//! # This file does NOT ship
//!
//! `crates/pmcp-team-servers/Cargo.toml` sets
//! `exclude = [".planning/", "fuzz/", "tests/"]`, so this gate stays out of the
//! published tarball while `baselines/` travels with the crate. That asymmetry
//! is why `era_diff` compiles the baseline bytes in with `include_str!` rather
//! than reading them at runtime.
//!
//! # If a test in this file fails
//!
//! The remedy is ALWAYS to fix the reader or restore the file. It is NEVER to
//! lower the floor, relax a shape rule, or delete an assertion. Every failure
//! message below states this inline, because the tempting fix is the wrong one.

use pmcp_team_servers::conformance::era_diff::{
    load_baseline, load_default_baseline, parse_baseline, EraBaseline, EraDelta,
};
use pmcp_team_servers::conformance::era_observations::{ObservationId, PROBE_REGISTRY};
use std::collections::BTreeSet;
use std::path::PathBuf;

// ===========================================================================
// Named constants
// ===========================================================================

/// The checked-in baseline this gate resolves, relative to the crate root.
///
/// A violation means the file moved or was deleted. The remedy is NOT to point
/// this constant somewhere else — it is to restore the file at this path, since
/// `era_diff::default_baseline_path` derives the same location for the shipped
/// loader and `include_str!` compiles the same bytes in.
const BASELINE_FILE: &str = "baselines/era-deltas.yaml";

/// Floor on the parsed entry count. Fourteen deltas were seeded by plan 118-03,
/// one per `PROBE_REGISTRY` entry, each with a checked citation, and plan 118-07
/// RECONCILED all fourteen against measurement against the live era target —
/// twelve confirmed as seeded, two (ERA-01, ERA-11) with their v2 token
/// corrected to the measured fact. The reconciled count is therefore still 14,
/// and this floor stays 14.
///
/// Falling below this means either the reader broke or entries were removed
/// without replacement. The remedy is NEVER to lower this number: a smaller
/// baseline silently reclassifies real expected differences as findings (and,
/// at zero, makes every diff pass over an empty set). A later phase may RAISE
/// it; nothing ever lowers it.
const MINIMUM_DELTAS: usize = 14;

/// The `kind` marking a row that records a MEASURED SAMENESS (v1 token == v2
/// token) rather than a measured difference.
///
/// Kept in step with `baselines/era-deltas.yaml` and with
/// `tests/era_matrix.rs::AGREEMENT_KIND`, which additionally requires the
/// OBSERVED tokens to equal the recorded ones before treating such a row as
/// satisfied.
const AGREEMENT_KIND: &str = "era-agreement";

/// Hard cap on how many rows may carry [`AGREEMENT_KIND`].
///
/// Exactly the two plan 118-07 measured: ERA-01 (`initialize` still served on
/// v2) and ERA-11 (`logging/setLevel` still served on v2), both declared gaps
/// under Phase 118 decision D-21.
///
/// The cap exists because an agreement row is the ONE row shape whose MISSING
/// classification `tests/era_matrix.rs` does not treat as a finding. Capping it
/// at the measured two is what stops the shape from growing into a general
/// escape hatch for "the eras stopped differing and we would rather not look".
/// The remedy for a violation is to fix the server so the eras differ again, or
/// to record the new sameness AS A PHASE DECISION and raise this cap in the same
/// commit, saying why. Never raise it silently.
const MAXIMUM_AGREEMENT_ROWS: usize = 2;

/// Floor on the length of an entry's `source` citation. Below this a value is a
/// label ("D-07", "spec"), not something a reviewer can go and check.
///
/// The remedy for a violation is NOT to shorten this constant — it is to write
/// the citation out, file and line.
const MIN_SOURCE_CHARS: usize = 10;

/// The only schema version this gate knows how to read.
///
/// The remedy for a mismatch is to update this gate TOGETHER with the schema,
/// never to delete the check.
const EXPECTED_SCHEMA_VERSION: u32 = 1;

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
            "FAILURE MODE: the checked-in baseline at {} did not load: {err}\n\
             WHAT TO DO: fix the reader or restore the file; do not delete this gate.",
            path.display()
        )
    })
}

/// Does `text` name a phase by number (e.g. "Phase 118")? Hand-rolled rather
/// than pulled through a pattern-matching crate, so this gate adds no
/// dependency of its own.
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
//
// NOT ASSERTED HERE, AND WHY:
//   * that a given v1/v2 token pair is CORRECT — that is a spec judgement for a
//     reviewer, and encoding it here would just restate the file;
//   * that `kind` comes from a closed vocabulary — grouping is presentational,
//     and a closed set would block a legitimately new difference class;
//   * that a `source` line number still points at the cited symbol — a test
//     that read the cited file would derive its expectation from the artifact
//     under test, which is the standing `D-115-AI(4)` anti-pattern.

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
             `meta.`, `http.status.`.",
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
// 3b. `kind: era-agreement` is shaped as it claims, and is CAPPED
// ===========================================================================
//
// This is a CONTENT rule and the file's preamble says content rules do not
// belong here — with one carve-out, which this is. Every other `kind` value is
// presentational (it only groups rows in a rendered report), but
// `era-agreement` is the one value that CHANGES A VERDICT: it is the sole row
// shape whose MISSING classification `tests/era_matrix.rs` does not treat as a
// finding. A field that decides an outcome has to be gated where it is written.

#[test]
fn agreement_rows_are_shaped_as_they_claim_and_are_capped() {
    let baseline = baseline();

    let agreements: Vec<&EraDelta> = baseline
        .deltas
        .iter()
        .filter(|delta| delta.kind == AGREEMENT_KIND)
        .collect();

    for delta in &agreements {
        assert_eq!(
            delta.v1, delta.v2,
            "FAILURE MODE: entry `{}` in {BASELINE_FILE} carries `kind: {AGREEMENT_KIND}` but \
             records v1 `{}` and v2 `{}`, which are DIFFERENT.\n\
             CONSEQUENCE: an \"agreement\" row that records a difference would be exempted from \
             the MISSING arm while claiming a difference nothing has to reproduce — an \
             allowlist wearing a measurement's clothes.\n\
             WHAT TO DO: give the row the kind that matches what it records.",
            delta.id, delta.v1, delta.v2
        );
    }

    // The inverse direction: a row that records v1 == v2 without the marker
    // could only ever be reported MISSING, forever.
    for delta in &baseline.deltas {
        assert!(
            delta.v1 != delta.v2 || delta.kind == AGREEMENT_KIND,
            "FAILURE MODE: entry `{}` in {BASELINE_FILE} records v1 == v2 (`{}`) but its kind is \
             `{}`, not `{AGREEMENT_KIND}`.\n\
             CONSEQUENCE: a row recording no difference can only ever be reported MISSING, in \
             every run, forever — the permanent-false-finding failure mode.\n\
             WHAT TO DO: mark it `{AGREEMENT_KIND}` with a citation for the sameness, or fix the \
             tokens.",
            delta.id,
            delta.v1,
            delta.kind
        );
    }

    let ids: Vec<&str> = agreements.iter().map(|d| d.id.as_str()).collect();
    assert!(
        agreements.len() <= MAXIMUM_AGREEMENT_ROWS,
        "FAILURE MODE: {} rows in {BASELINE_FILE} carry `kind: {AGREEMENT_KIND}` ({ids:?}); the \
         cap is {MAXIMUM_AGREEMENT_ROWS}.\n\
         CONSEQUENCE: agreement rows are the one shape whose MISSING classification is not a \
         finding. Letting them accumulate turns the era matrix into a record of what stopped \
         differing rather than a gate on it.\n\
         WHAT TO DO: fix the server so the eras differ again, or record the new sameness as a \
         PHASE DECISION and raise the cap in the same commit, saying why. Never raise it \
         silently.",
        agreements.len()
    );
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
        baseline.schema_version, EXPECTED_SCHEMA_VERSION,
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
        pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28,
        "FAILURE MODE: {BASELINE_FILE} claims v2 is `{}` while the SDK's v2 constant is `{}`.\n\
         WHAT TO DO: re-review every entry against the new version, then update the file; do not \
         hardcode the string here.",
        baseline.v2_protocol,
        pmcp::types::protocol::PROTOCOL_VERSION_2026_07_28
    );
}

// ===========================================================================
// 6. Provisional entries name their owner
// ===========================================================================

/// After plan 118-07 the baseline is FULLY RECONCILED: every row was measured
/// against the live era target, so nothing is awaiting measurement.
///
/// The owner rule below is deliberately kept even though it cannot run while
/// this assertion holds. It is the rule a future phase must satisfy at the
/// MOMENT it re-introduces a provisional row, and deleting it now would mean
/// rediscovering it then — by which time the exemption in
/// `tests/era_matrix.rs::assert_no_missing` would already be live again with no
/// named owner attached to it.
#[test]
fn provisional_entries_name_their_owning_phase() {
    let baseline = baseline();

    let provisional: Vec<&EraDelta> = baseline.deltas.iter().filter(|d| d.provisional).collect();

    let ids: Vec<&str> = provisional.iter().map(|d| d.id.as_str()).collect();
    assert!(
        provisional.is_empty(),
        "FAILURE MODE: {ids:?} in {BASELINE_FILE} are marked provisional, but plan 118-07 \
         reconciled every row against measurement and cleared the last flag.\n\
         CONSEQUENCE: a provisional row is EXEMPT from the MISSING arm in \
         tests/era_matrix.rs, so it is a claim nothing checks — strictly worse than no claim.\n\
         WHAT TO DO: measure the row and clear the flag, or delete the row AND its probe. If a \
         phase legitimately needs to re-introduce a provisional row, relax THIS assertion in the \
         same commit and keep the owner rule below."
    );

    for delta in provisional {
        let note = delta.note.as_deref().unwrap_or_default();
        assert!(
            names_a_phase(note),
            "FAILURE MODE: provisional entry `{}` in {BASELINE_FILE} has the note {note:?}, which \
             names no phase. A provisional entry with no owner cannot be re-reviewed when its \
             phase signs off, so it silently rots.\n\
             WHAT TO DO: name the owning phase in the note (e.g. \"Phase 118 plan 07 owns this\"); \
             do not clear the provisional flag to dodge this check.",
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
        // Two entries sharing an `observation_id` — the T-118-14 shape, which
        // would merge two distinct wire facts into one row.
        "schema_version: 1\nv1_protocol: a\nv2_protocol: b\ndeltas:\n  - id: ERA-01\n    \
         observation_id: method.initialize\n    subject: s\n    v1: a\n    v2: b\n    kind: k\n    \
         source: c\n  - id: ERA-02\n    observation_id: method.initialize\n    subject: s\n    \
         v1: a\n    v2: b\n    kind: k\n    source: c\n",
        // `schema_version` as a string rather than an integer.
        "schema_version: \"one\"\nv1_protocol: a\nv2_protocol: b\ndeltas: []\n",
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

// ===========================================================================
// 8. The two-direction coverage contract
//
// The registry and the baseline are two halves of one join. Either side
// silently drifting from the other converts the whole gate into permanent noise
// (a probe with no row: every run an UNEXPECTED finding) or permanent silence
// (a row with no probe: every run a false MISSING finding). Both directions are
// asserted, and both are proved by an executed negative control recorded in
// `118-03-SUMMARY.md`.
// ===========================================================================

/// DIRECTION 1: every baseline entry has a probe.
///
/// A baseline entry nothing observes can only ever be reported MISSING, in
/// every run, forever — the permanent-false-finding defect the observation-id
/// design exists to remove. The REGISTRY is the authority here, not the file.
#[test]
fn every_baseline_entry_has_a_probe() {
    let baseline = load_default_baseline().expect("the shipped baseline must load");
    let probes: BTreeSet<&str> = PROBE_REGISTRY.iter().map(|id| id.as_str()).collect();
    let unprobed: Vec<&str> = baseline
        .observation_ids()
        .into_iter()
        .filter(|id| !probes.contains(id))
        .collect();
    assert!(
        unprobed.is_empty(),
        "FAILURE MODE: these baseline observation_ids have NO probe and would report a \
         permanent false MISSING finding: {unprobed:?}\n\
         WHAT TO DO: add the probe id to PROBE_REGISTRY, or delete the row; do NOT delete this \
         check to make the baseline the authority."
    );
}

/// DIRECTION 2: every probe has a baseline entry.
///
/// A probe with no entry would report every difference it sees as an UNEXPECTED
/// finding, which is the same defect pointing the other way.
#[test]
fn every_probe_has_a_baseline_entry() {
    let baseline = load_default_baseline().expect("the shipped baseline must load");
    let entries: BTreeSet<&str> = baseline.observation_ids().into_iter().collect();
    let unbaselined: Vec<&str> = PROBE_REGISTRY
        .iter()
        .map(|id| id.as_str())
        .filter(|id| !entries.contains(id))
        .collect();
    assert!(
        unbaselined.is_empty(),
        "FAILURE MODE: these probes have NO baseline entry and would report every run as an \
         UNEXPECTED finding: {unbaselined:?}\n\
         WHAT TO DO: add a cited row to {BASELINE_FILE}; do not remove the probe to dodge this."
    );
}

/// The four mcp-tester ids the port DELIBERATELY dropped must be absent from
/// BOTH halves of the join.
///
/// This assertion lives here rather than in `era_observations.rs` because
/// spelling these ids as literals in that file would defeat the mechanical
/// check that its registry does not carry them.
#[test]
fn the_four_deliberately_dropped_ids_are_absent_from_both_halves() {
    let baseline = load_default_baseline().expect("the shipped baseline must load");
    for dropped in [
        "method.tasks_list",
        "capability.tasks_location",
        "method.resources_subscribe",
        "method.subscriptions_listen",
    ] {
        assert_eq!(
            ObservationId::from_registry(dropped),
            None,
            "FAILURE MODE: `{dropped}` resolved through PROBE_REGISTRY. The module doc of \
             `era_observations.rs` records WHY it was not ported: the era target implements no \
             Tasks surface, and the two subscription ids are client-side or capability-gated in \
             BOTH eras, so each could only ever produce a false finding.\n\
             WHAT TO DO: read that section before restoring the id; do not delete this check."
        );
        assert!(
            baseline.find_by_observation_id(dropped).is_none(),
            "FAILURE MODE: {BASELINE_FILE} carries a row for `{dropped}`, which has no probe.\n\
             WHAT TO DO: delete the row; see DIRECTION 1 in section 8 above."
        );
    }
}
