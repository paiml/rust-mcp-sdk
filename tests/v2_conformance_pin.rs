//! Phase 113-32 (HTTP-08): binds pmcp's `advertises_subscriptions` to the
//! conformance predicate pinned in `113-SPEC-RECHECK.md` § B.6.
//!
//! # Why this file exists
//!
//! HTTP-08's advertise-implies-serve rule — *a server that advertises a
//! subscription-delivered capability but answers `subscriptions/listen` with
//! `-32601` is graded FAILURE* — **has no specification sentence behind it**.
//! `subscriptions.mdx` contains no capability-gating rule and
//! `ServerCapabilities` has no `subscriptions` capability. The entire grading
//! comes from the TypeScript predicate `advertisesSubscriptions` in the
//! **conformance repository**, which moves on its own release cadence.
//!
//! `113-SPEC-RECHECK.md`'s re-verification obligation used to pin only a
//! *schema* sha. A schema re-check can never detect drift in a predicate that
//! lives somewhere else, so HTTP-08 was graded by a source nothing watched
//! (`113-SPEC-RECHECK-ADDENDUM-2026-07-26.md` Finding 12). § B.6 is the second
//! arm of that gate; this file is its enforcement.
//!
//! # What is bound to what
//!
//! `src/types/subscriptions.rs` already carries
//! `advertises_subscriptions_over_all_sixteen_capability_combinations`, which
//! sweeps the same space against a **hardcoded** four-flag expectation. That
//! test proves pmcp is internally self-consistent; it asserts nothing about
//! upstream. The hardcoded four is precisely what would silently disagree with
//! a changed predicate. This file adds the missing binding — the expectation is
//! **read from § B.6.3 at runtime** — and does not replace that unit test.
//!
//! # Both directions fail
//!
//! * an upstream-GAINED disjunct: § B.6.3 lists a capability path
//!   [`advertise`] has no arm for, and the fallthrough panics by name.
//! * an upstream-LOST, RENAMED or REORDERED disjunct:
//!   [`pinned_disjunct_list_matches_pmcp_supported_flags`] compares the parsed
//!   list against [`PMCP_COUNTERPARTS`] in order.
//! * a NARROWED pmcp predicate: the combination sweep exercises every
//!   non-empty subset, so dropping any one arm of `supported_flags` fails on
//!   the singleton subset only that arm satisfies.
//!
//! # Strictness is the point
//!
//! [`pinned_disjuncts`] FAILS on a row it cannot parse rather than skipping it.
//! A lenient parser here would silently restore the exact blindness § B.6
//! exists to remove (threat T-113-158).
//!
//! This file drives no socket and needs no transport feature; it reads a
//! document from disk and calls one pure function.

#![cfg(not(target_arch = "wasm32"))]

use pmcp::types::subscriptions::advertises_subscriptions;
use pmcp::types::{PromptCapabilities, ResourceCapabilities, ServerCapabilities, ToolCapabilities};
use std::fs;
use std::path::{Path, PathBuf};

/// The capability paths pmcp's `supported_flags` implements, in its index
/// order (`src/types/subscriptions.rs:488-511`).
///
/// Spelled the conformance way, so the pinned TypeScript, § B.6.3, this file
/// and `CAPABILITY_NAMES` in `tests/v2_subscriptions.rs` all use one naming.
const PMCP_COUNTERPARTS: [&str; 4] = [
    "tools.listChanged",
    "prompts.listChanged",
    "resources.listChanged",
    "resources.subscribe",
];

/// The heading whose table [`pinned_disjuncts`] parses.
const DISJUNCT_HEADING: &str = "#### B.6.3 Disjuncts";

/// The heading whose table carries § B.6's copy of the conformance sha.
const PROVENANCE_HEADING: &str = "#### B.6.1 Provenance";

// ===========================================================================
// Document access.
//
// `phase_dir`, `section` and `table_rows` are lifted from
// `tests/v2_mrtr.rs` (`manifest_maps_every_pinned_scenario` and its helpers).
// Cargo compiles each integration test as its own binary, so they cannot be
// imported across files; duplicating ~40 lines is cheaper than coupling this
// subscriptions check to a MRTR conformance mirror that other plans are
// editing concurrently.
// ===========================================================================

/// The phase directory holding the spec re-check record.
fn phase_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".planning")
        .join("phases")
        .join("113-stateless-http-multi-round-trip-elicitation")
}

/// `113-SPEC-RECHECK.md`, or `None` when there is nothing to check.
///
/// Guard semantics carried from `manifest_maps_every_pinned_scenario`:
/// `.planning/` is excluded from the published crate (`Cargo.toml` `exclude`),
/// so a downstream `cargo test` has no record to read and returns early. A
/// phase directory that EXISTS but has no record is a FAILURE — that is a
/// deleted gate, not a packaging artifact.
fn recheck_doc() -> Option<String> {
    let dir = phase_dir();
    if let Ok(text) = fs::read_to_string(dir.join("113-SPEC-RECHECK.md")) {
        return Some(text);
    }
    assert!(
        !dir.exists(),
        "the phase directory exists but 113-SPEC-RECHECK.md is missing — HTTP-08's \
         conformance-predicate pin would go unenforced"
    );
    None
}

/// The lines under `heading`, up to the next markdown heading of any level.
fn subsection(text: &str, heading: &str) -> String {
    let mut lines = text.lines().skip_while(|line| line.trim() != heading);
    let found = lines.next();
    assert!(
        found.is_some(),
        "`{heading}` is missing from 113-SPEC-RECHECK.md — § B.6 is the second arm of the \
         re-verification obligation and removing it silently un-watches HTTP-08's only source"
    );
    let mut collected = String::new();
    for line in lines.take_while(|line| !line.starts_with('#')) {
        collected.push_str(line);
        collected.push('\n');
    }
    collected
}

/// The cells of every markdown table row in `text`, minus separator rows.
fn table_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches('`').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|cells| {
            !cells
                .iter()
                .all(|cell| cell.chars().all(|ch| ch == '-' || ch == ':'))
        })
        .collect()
}

/// The value cell of the first row whose label cell contains `label`.
fn labelled_value(text: &str, label: &str) -> Option<String> {
    table_rows(text)
        .into_iter()
        .find(|cells| cells.first().is_some_and(|cell| cell.contains(label)))
        .and_then(|cells| cells.get(1).cloned())
}

// ===========================================================================
// § B.6.3 — the strict parser.
// ===========================================================================

/// The ordered conformance capability paths of § B.6.3's disjunct table.
///
/// STRICT BY DESIGN. Every table row that is neither the header nor the
/// separator MUST parse: a malformed row is a FAILURE, never a skip. The
/// ordinal column is checked to be dense and 1-based so that a deleted row
/// cannot pass as a renumbering.
fn pinned_disjuncts(recheck: &str) -> Vec<String> {
    let rows = table_rows(&subsection(recheck, DISJUNCT_HEADING));
    assert!(
        rows.len() >= 2,
        "{DISJUNCT_HEADING} must contain a header row and at least one disjunct; found \
         {} row(s)",
        rows.len()
    );
    let mut paths: Vec<String> = Vec::new();
    for (index, cells) in rows.into_iter().enumerate() {
        assert_eq!(
            cells.len(),
            4,
            "{DISJUNCT_HEADING} row {index} has {} cell(s), expected exactly 4 \
             (`#`, capability path, verbatim disjunct, pmcp counterpart). The task-2 \
             parser is written against that layout and rejects anything else rather \
             than skipping it: {cells:?}",
            cells.len()
        );
        if index == 0 {
            assert_eq!(
                cells[1], "Conformance capability path",
                "{DISJUNCT_HEADING}'s second column must stay the capability path — the \
                 parser keys on position and a reordered header means it is now reading \
                 the wrong column"
            );
            continue;
        }
        push_disjunct(&mut paths, index, &cells);
    }
    paths
}

/// Validates one § B.6.3 body row and appends its capability path.
fn push_disjunct(paths: &mut Vec<String>, index: usize, cells: &[String]) {
    let ordinal: usize = cells[0].parse().unwrap_or_else(|_| {
        panic!(
            "{DISJUNCT_HEADING} row {index}: first cell `{}` is not an ordinal. A row this \
             parser cannot read is a failure, not a skip",
            cells[0]
        )
    });
    assert_eq!(
        ordinal,
        paths.len() + 1,
        "{DISJUNCT_HEADING} ordinals must be dense and 1-based; row {index} reads {ordinal} \
         after {} earlier disjunct(s). A gap hides a deleted disjunct",
        paths.len()
    );
    let path = cells[1].clone();
    assert!(
        !path.is_empty(),
        "{DISJUNCT_HEADING} row {index} has an empty capability path"
    );
    paths.push(path);
}

// ===========================================================================
// The pmcp side of the binding.
// ===========================================================================

/// Sets exactly the flag named by the conformance `path` on `caps`.
///
/// EXHAUSTIVE BY DESIGN, and the fallthrough IS the upstream-drift detector.
fn advertise(caps: &mut ServerCapabilities, path: &str) {
    match path {
        "tools.listChanged" => {
            caps.tools = Some(ToolCapabilities {
                list_changed: Some(true),
            });
        },
        "prompts.listChanged" => {
            caps.prompts = Some(PromptCapabilities {
                list_changed: Some(true),
            });
        },
        "resources.listChanged" => {
            let mut resources = resources_of(caps);
            resources.list_changed = Some(true);
            caps.resources = Some(resources);
        },
        "resources.subscribe" => {
            let mut resources = resources_of(caps);
            resources.subscribe = Some(true);
            caps.resources = Some(resources);
        },
        other => panic!("{}", unmapped_disjunct_message(other)),
    }
}

/// The existing `resources` capability, or a fresh default one.
///
/// Two of the four disjuncts live on the SAME sub-capability, so setting one
/// must not clear the other — otherwise the 2-element subset containing both
/// would silently degrade to a 1-element one and the sweep would weaken.
fn resources_of(caps: &ServerCapabilities) -> ResourceCapabilities {
    caps.resources.clone().unwrap_or_default()
}

/// What to tell the reader when § B.6.3 names a path pmcp cannot map.
fn unmapped_disjunct_message(path: &str) -> String {
    format!(
        "113-SPEC-RECHECK.md § B.6.3 lists the conformance capability path \
         `{path}`, which pmcp's `advertises_subscriptions` has NO counterpart for.\n\
         \n\
         The conformance predicate `advertisesSubscriptions` \
         (conformance/src/scenarios/server/stateless.ts) has GAINED a disjunct upstream, so \
         HTTP-08's obligation has CHANGED: a server advertising only `{path}` must now serve \
         `subscriptions/listen`, and pmcp would answer -32601 and be graded FAILURE — \
         \"claims a feature it does not serve\".\n\
         \n\
         Per § B.6.4 this is a PHASE-REOPENING event, NOT an advisory. Do NOT resolve it by \
         deleting the row or loosening this test: reconcile `supported_flags` in \
         src/types/subscriptions.rs against the new pin, then re-run arm 2 of the \
         re-verification obligation."
    )
}

/// `ServerCapabilities` advertising exactly the paths selected by `mask`.
fn capabilities_for(paths: &[String], mask: usize) -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    for (bit, path) in paths.iter().enumerate() {
        if mask & (1 << bit) != 0 {
            advertise(&mut caps, path);
        }
    }
    caps
}

// ===========================================================================
// Tests.
// ===========================================================================

/// Every capability path § B.6.3 pins maps to a pmcp counterpart.
///
/// This is the GAINED-disjunct direction. It runs the mapping on its own, so
/// an unmapped path surfaces [`unmapped_disjunct_message`] rather than an
/// arity mismatch from the equality check below.
#[test]
fn every_pinned_disjunct_maps_to_a_pmcp_counterpart() {
    let Some(recheck) = recheck_doc() else { return };
    let paths = pinned_disjuncts(&recheck);
    assert!(
        !paths.is_empty(),
        "§ B.6.3 pins no disjuncts at all — the predicate cannot be empty"
    );
    for path in &paths {
        let mut caps = ServerCapabilities::default();
        advertise(&mut caps, path);
        assert!(
            advertises_subscriptions(&caps),
            "§ B.6.3 pins `{path}` as a disjunct of `advertisesSubscriptions`, and pmcp maps \
             it, but `advertises_subscriptions` still reports false for a server advertising \
             ONLY that path. pmcp's `supported_flags` no longer reads it"
        );
    }
}

/// § B.6.3's disjunct list equals pmcp's, in order.
///
/// This is the LOST / RENAMED / REORDERED direction, which the mapping alone
/// cannot see: a table with a row deleted maps fine and would sweep a smaller
/// space in silence.
#[test]
fn pinned_disjunct_list_matches_pmcp_supported_flags() {
    let Some(recheck) = recheck_doc() else { return };
    let paths = pinned_disjuncts(&recheck);
    assert_eq!(
        paths.as_slice(),
        PMCP_COUNTERPARTS.map(String::from).as_slice(),
        "§ B.6.3's pinned disjunct list and pmcp's `supported_flags` arms have diverged.\n\
         \n\
         § B.6.3 (the conformance predicate, pinned): {paths:?}\n\
         pmcp `supported_flags` order:               {PMCP_COUNTERPARTS:?}\n\
         \n\
         A LOST disjunct means pmcp over-obliges itself; a RENAMED one means the two sides \
         have silently stopped describing the same thing. § B.6.4: phase-reopening for \
         HTTP-08, not advisory."
    );
}

/// `advertises_subscriptions` is the exact disjunction § B.6.3 pins.
///
/// The sweep is driven by the PARSED list's length, not by a hardcoded 4, so
/// it grows automatically if the pin and the mapping are ever extended
/// together. False for the empty set, true for every non-empty subset — which
/// binds the disjunction SHAPE, not merely its arity.
#[test]
fn advertises_subscriptions_over_the_pinned_combination_space() {
    let Some(recheck) = recheck_doc() else { return };
    let paths = pinned_disjuncts(&recheck);
    assert!(
        paths.len() <= 16,
        "the sweep is exponential; {} pinned disjuncts is implausible and would hang CI",
        paths.len()
    );

    assert!(
        !advertises_subscriptions(&capabilities_for(&paths, 0)),
        "a server advertising NONE of the pinned disjuncts must not advertise \
         subscriptions — that is pmcp's stateless enterprise default, for which answering \
         `subscriptions/listen` with -32601 is graded SKIPPED, not FAILURE"
    );

    for mask in 1..(1_usize << paths.len()) {
        let selected: Vec<&str> = paths
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, path)| path.as_str())
            .collect();
        assert!(
            advertises_subscriptions(&capabilities_for(&paths, mask)),
            "a server advertising {selected:?} must advertise subscriptions: every one of \
             those paths is a disjunct of the pinned `advertisesSubscriptions`, so the \
             conformance suite would require the `subscriptions/listen` stream to be SERVED \
             and would grade a -32601 as FAILURE. pmcp's `supported_flags` disagrees"
        );
    }
}

/// § B.6.1 and § B.1 record the SAME conformance sha.
///
/// Two pins in one document that disagree is worse than one pin: a re-checker
/// would diff the predicate at one commit and the scenario ids at another and
/// believe both were current.
#[test]
fn b6_and_b1_record_the_same_conformance_sha() {
    let Some(recheck) = recheck_doc() else { return };
    let b1 = labelled_value(&recheck, "Pinned sha")
        .expect("§ B.1 records the conformance pin in a `**Pinned sha**` row");
    let b6 = labelled_value(
        &subsection(&recheck, PROVENANCE_HEADING),
        "Sha (as pinned in",
    )
    .expect("§ B.6.1's provenance table records the sha it quoted the predicate at");

    assert_eq!(
        b1.len(),
        40,
        "§ B.1's `Pinned sha` must be a full 40-character sha, not `{b1}`"
    );
    assert!(
        b1.chars().all(|ch| ch.is_ascii_hexdigit()),
        "§ B.1's `Pinned sha` is not hexadecimal: `{b1}`"
    );
    assert_eq!(
        b6, b1,
        "§ B.6.1 (`Sha (as pinned in § B.1)`) records `{b6}` but § B.1 (`Pinned sha`) records \
         `{b1}`. § B.1 is the section's single pin and § B.6.1 carries a copy purely so this \
         check can exist — reconcile them before running arm 2"
    );
}

/// § B.6.2's verbatim quotation still contains the predicate it claims to pin.
///
/// Cheap, but it catches the quotation being emptied or replaced by prose
/// while the surrounding tables keep passing.
#[test]
fn b6_quotes_the_predicate_verbatim() {
    let Some(recheck) = recheck_doc() else { return };
    let quoted = subsection(&recheck, "#### B.6.2 The predicate, verbatim");
    assert!(
        quoted.contains("const advertisesSubscriptions = !!("),
        "§ B.6.2 must carry the `advertisesSubscriptions` declaration VERBATIM — a pin whose \
         text is gone cannot be diffed against upstream, which is the whole point of arm 2"
    );
    for path in PMCP_COUNTERPARTS {
        let (owner, field) = path
            .split_once('.')
            .expect("every counterpart path is `owner.field`");
        let disjunct = format!("discoverCapabilities?.{owner}?.{field}");
        assert!(
            quoted.contains(&disjunct),
            "§ B.6.2's verbatim quotation does not contain `{disjunct}`, yet § B.6.3 pins \
             `{path}` as a disjunct. The quotation and the table must describe one predicate"
        );
    }
}
