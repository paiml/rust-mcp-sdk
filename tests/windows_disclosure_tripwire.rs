//! D-03 — the disclosure tripwire.
//!
//! Phase 119's D-02 deliberately accepted a two-places-to-sync cost: `CHANGELOG.md`
//! stays the per-*release* record, while the per-*migration* view of
//! consumer-observable behaviour changes lives in
//! `pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md`. This test is what pays
//! for that decision.
//!
//! Every change marked `[CONSUMER-OBSERVABLE]` in the broken-windows ledger
//! (`.planning/WINDOWS.md`) ships with **no semver signal** — the symbols involved
//! are private, so `cargo semver-checks` reports no update required and the
//! migration chapter is the only place a downstream consumer learns of the change.
//! Without a mechanical fence, the next such disclosure lands in the ledger, never
//! reaches the guide, and a consumer is surprised by a wire change with no record.
//! That is the exact repudiation failure the marked ledger entries note in their own
//! text. (Deliberately not enumerated here: a by-hand id list in this header would
//! need the same manual sync the test below exists to eliminate.)
//!
//! # Derived, never enumerated
//!
//! The expected set is *computed* by scanning the ledger for the sentinel. It is
//! deliberately NOT a hard-coded id list: such a list passes on the day it is
//! written and goes blind on the very next disclosure, which is precisely the rot
//! this test exists to prevent. A future entry 24 carrying the sentinel is in scope
//! automatically, with no edit here.
//!
//! Phase 115 learned the same lesson the expensive way: a fence that restates the
//! rule under test cannot detect a defect in that rule, which is why
//! `tests/keyword_list_mirrors.rs` carries its own container literal rather than
//! importing the crate's.
//!
//! # Packaging
//!
//! This file is listed in `Cargo.toml`'s `exclude` array, alongside
//! `tests/ci_conformance_gate_wiring.rs`, `tests/keyword_list_mirrors.rs`,
//! `tests/phase115_contract_bindings.rs` and `tests/team_contracts_conformance.rs`.
//! Both trees it reads (`.planning/` and `pmcp-book/`) are excluded from the
//! published crate, and a reader whose whole job is to FAIL when a disclosure is
//! absent must not degrade to a silent no-op downstream. The absent-tree guards
//! below therefore cover sparse or partial checkouts, not the published crate —
//! that path is closed by exclusion instead.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

/// The marker plan 119-05 attached to every consumer-observable ledger entry.
///
/// Marking is done in the entry `description` rather than by a schema change,
/// because `.planning/WINDOWS.md` is owned by an external tool (`gsd-tools
/// windows`) that regenerates the rendered table from the JSON block.
const SENTINEL: &str = "[CONSUMER-OBSERVABLE]";

/// The ledger, relative to the repository root.
const LEDGER_REL: &str = ".planning/WINDOWS.md";

/// The migration guide, relative to the repository root.
const CHAPTER_REL: &str = "pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md";

/// The number of `[CONSUMER-OBSERVABLE]` entries the ledger carried when this gate
/// was written, used as a FLOOR rather than as an expected set.
///
/// `!marked.is_empty()` alone is too weak: all but one of the marked entries could
/// lose their sentinel — a `gsd-tools windows` re-record, or a description rewritten
/// in place, which entries in this ledger explicitly document happening ("SEVERITY
/// CORRECTED IN PLACE", "RESTATED … rewritten in place") — and one survivor would
/// still satisfy it. Coverage would shrink with no signal at all, and the gate would
/// keep reporting green while checking less.
///
/// This is the same never-lower-the-floor doctrine as `MINIMUM_GATE_NEEDS` in
/// `tests/ci_conformance_gate_wiring.rs` and `MIN_FULL_ENTRIES` in
/// `tests/v1_severability_tripwire.rs`. If it fires, FIX THE LEDGER or the reader —
/// raise this floor when entries are added, never lower it to make a red go green.
///
/// NOTE this is a floor on the COUNT only. The id set itself stays derived; nothing
/// here enumerates which entries are marked.
///
/// RAISED 5 -> 9 by Phase 118.2 plan 24, which appended four marked entries (24, 25,
/// 26, 27). Raised, per the doctrine two paragraphs up, because a floor left at the
/// old value keeps reporting green while checking less: four of the nine could lose
/// their sentinel and a floor of 5 would still pass.
const MIN_MARKED_ENTRIES: usize = 9;

/// One broken-windows ledger entry.
///
/// Only the two fields this gate reasons about are declared; the ledger carries
/// several more (`kind`, `phase`, `status`, timestamps) and `serde` ignores them,
/// so a future field addition by `gsd-tools` does not break this test.
#[derive(Deserialize)]
struct Window {
    id: u64,
    description: String,
}

/// Primitive restated from `tests/keyword_list_mirrors.rs` and
/// `tests/phase115_contract_bindings.rs`.
///
/// KNOWN DEBT, stated accurately. This is a CHOICE, not a constraint. An earlier
/// version of this note claimed integration tests "cannot import across files, so
/// duplicating the helper is the documented house choice" — that is false, and
/// `tests/common/mod.rs` says so in its own header: files under `tests/common/` are
/// not compiled as their own test binaries and are the correct home for shared
/// helpers, which 42 files in this tree already consume via `mod common;`.
/// `tests/v1_byte_identity_after_cut.rs` retracts the identical false claim.
/// The standing fix is a `tests/common/planning.rs` carrying this plus
/// [`read_guarded`], consumed by the four readers that now restate them.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The contents of a repo-relative planning/doc file, or `None` when the whole
/// tree that would contain it is absent.
///
/// Guard semantics carried from `recheck_doc` in `tests/v2_conformance_pin.rs`
/// (cited by symbol, not line: line citations in doc comments drift on every edit
/// to the cited file). `.planning/` and `pmcp-book/` are each excluded from the
/// published crate, so a checkout that does not carry the tree at all genuinely has
/// nothing to read and returns early. A tree that EXISTS but is missing the file is
/// a FAILURE — that is a deleted gate, not a packaging artifact. The fork is
/// evaluated per tree, which is why the tree is derived from the file's own path
/// rather than passed in.
///
/// The read goes through `rel` — the SAME constant the failure message tells the
/// reader to update. An earlier shape named the path in a `*_REL` const but rebuilt
/// it from separate literals to read it, so following the printed remedy changed
/// nothing and left the gate red with the same message.
fn read_guarded(rel: &str, consequence: &str) -> Option<String> {
    let path = repo_root().join(rel);
    if let Ok(text) = fs::read_to_string(&path) {
        return Some(text);
    }
    let tree = path
        .parent()
        .expect("a repo-relative file path always has a parent directory");
    assert!(
        !tree.exists(),
        "FAILURE MODE: `{}` exists but `{rel}` is missing. {consequence}\n\
         WHAT TO DO: restore `{rel}`, or update its `*_REL` constant in this test if it \
         moved — do not delete the assertion.",
        tree.display()
    );
    None
}

/// The entries in the four-backtick ` ````json ` block of the ledger.
///
/// That block is the AUTHORITATIVE representation. The rendered markdown table
/// above it is the human view and is regenerated from this array by `gsd-tools`,
/// so parsing the table instead would key this gate on a derived artifact.
///
/// Parsed with `serde_json` — already a first-order dependency — rather than by a
/// hand-rolled scan, so a description containing brackets, braces or escaped quotes
/// cannot desynchronise the reader from the writer.
fn ledger_entries(markdown: &str) -> Vec<Window> {
    let fence = "````";
    let mut lines = markdown.lines();
    let opened = lines.any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(fence) && trimmed.contains("json")
    });
    assert!(
        opened,
        "FAILURE MODE: no ` {fence}json ` block found in `{LEDGER_REL}`. This gate parses \
         the authoritative JSON array, not the rendered table, so a missing or reformatted \
         fence makes it read nothing and pass vacuously.\n\
         WHAT TO DO: restore the four-backtick JSON block in `{LEDGER_REL}`, or teach \
         `ledger_entries` the new fence — do not delete the assertion."
    );

    let body: String = lines
        .take_while(|line| !line.trim().starts_with(fence))
        .collect::<Vec<_>>()
        .join("\n");

    serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: the JSON block in `{LEDGER_REL}` does not parse as a ledger \
             array: {e}. An unparseable ledger yields an empty expected set, which would \
             make this tripwire pass while checking nothing.\n\
             WHAT TO DO: repair the JSON block (`gsd-tools windows status` reports the same \
             breakage) — do not delete the assertion."
        )
    })
}

/// Whether `chapter` cites ledger entry `id`.
///
/// The matcher is deliberately tighter than a bare number search: entry `12` must
/// not be satisfied by `12` appearing inside a version string such as `pmcp 2.19`
/// or a line reference. It requires the citation phrase plan 119-05 used
/// (`WINDOWS.md entry <id>`) AND that the id is bounded by a non-alphanumeric on
/// BOTH sides, so a citation of entry `23` cannot stand in for a missing entry `2`
/// and `entry 12a` cannot stand in for entry `12`.
///
/// The left bound is asserted rather than inherited from the literal prefix. An
/// earlier shape checked only the character AFTER the id and relied on the trailing
/// space in `"WINDOWS.md entry "` to bound the left — true today, but it made the
/// stated guarantee an accident of the citation wording, so rewording the phrase
/// would have silently dropped it. Same both-sides rule as `numeric_hits` in
/// `tests/v2_tasks_tripwires.rs` and `contains_word` in
/// `tests/phase115_contract_bindings.rs`.
fn cites_entry(chapter: &str, id: u64) -> bool {
    let needle = format!("WINDOWS.md entry {id}");
    chapter.match_indices(&needle).any(|(at, _)| {
        let before_ok = chapter[..at]
            .chars()
            .next_back()
            .is_none_or(|prev| !prev.is_ascii_alphanumeric());
        let after_ok = chapter[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_ascii_alphanumeric());
        before_ok && after_ok
    })
}

#[test]
fn every_consumer_observable_window_is_cited_in_the_migration_chapter() {
    let (Some(ledger_md), Some(chapter)) = (
        read_guarded(
            LEDGER_REL,
            "The broken-windows ledger is the source of truth for which behaviour changes \
             are consumer-observable, so deleting it silently disarms this tripwire and \
             every future undisclosed change ships unnoticed.",
        ),
        read_guarded(
            CHAPTER_REL,
            "That chapter is the ONLY place a downstream consumer learns of a behaviour \
             change that ships with no semver signal, so removing it repudiates every \
             disclosure the ledger records.",
        ),
    ) else {
        return;
    };

    let marked: Vec<u64> = ledger_entries(&ledger_md)
        .into_iter()
        .filter(|entry| entry.description.contains(SENTINEL))
        .map(|entry| entry.id)
        .collect();

    // Positive control. A run that selects zero entries would then vacuously
    // satisfy "every selected entry is cited" — the same false green in a different
    // shape, and one this repository has recorded before. The non-empty assertion is
    // what makes this test's success mean something.
    assert!(
        marked.len() >= MIN_MARKED_ENTRIES,
        "FAILURE MODE: only {} `{SENTINEL}` entries were found in `{LEDGER_REL}`, below the \
         floor of {MIN_MARKED_ENTRIES}. At zero this gate would check nothing and pass \
         regardless of the chapter's contents; below the floor it silently checks LESS than \
         it did when it was written. Either the sentinel was stripped from entries that \
         still need it, or the description field this gate reads was renamed.\n\
         WHAT TO DO: confirm every consumer-observable entry still carries the literal \
         `{SENTINEL}` in its `description`; if the marking convention changed, update \
         `SENTINEL` here. Raise `MIN_MARKED_ENTRIES` when entries are added — never lower \
         it to make this red go green, and do not delete the assertion.",
        marked.len()
    );

    let uncited: Vec<u64> = marked
        .iter()
        .copied()
        .filter(|id| !cites_entry(&chapter, *id))
        .collect();

    assert!(
        uncited.is_empty(),
        "FAILURE MODE: `{LEDGER_REL}` entries {uncited:?} are marked `{SENTINEL}` but are \
         not cited in `{CHAPTER_REL}`. Each such entry is a behaviour change a downstream \
         consumer can observe that ships with NO semver signal, and that chapter is the only \
         place they learn of it — an uncited entry is a change disclosed to nobody.\n\
         WHAT TO DO: add a bullet for each id under `## Behaviour changes & known \
         limitations` → `### Consumer-observable changes with no semver signal` in \
         `{CHAPTER_REL}`, in the existing form `**WINDOWS.md entry <id> — <what a consumer \
         observes>**`, stating the old behaviour, the new behaviour and what the consumer \
         must do. If an entry is NOT in fact consumer-observable, remove the `{SENTINEL}` \
         marker from its `description` in `{LEDGER_REL}` instead — do not delete this \
         assertion."
    );
}
