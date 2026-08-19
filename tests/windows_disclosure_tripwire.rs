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
//! That is the exact repudiation failure ledger entries 12, 13, 19 and 20 each note
//! in their own text.
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

/// Primitive shared with `tests/keyword_list_mirrors.rs` and
/// `tests/phase115_contract_bindings.rs`. Cargo compiles each integration test as
/// its own binary, so they cannot import across files; duplicating the helper is
/// the documented house choice.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `.planning/WINDOWS.md`, or `None` when the whole `.planning/` tree is absent.
///
/// Guard semantics carried from `tests/v2_conformance_pin.rs:96-111`: `.planning/`
/// is excluded from the published crate, so a checkout that does not carry the tree
/// at all genuinely has nothing to read and returns early. A `.planning/` directory
/// that EXISTS but has no `WINDOWS.md` is a FAILURE — that is a deleted gate, not a
/// packaging artifact.
fn ledger() -> Option<String> {
    let tree = repo_root().join(".planning");
    if let Ok(text) = fs::read_to_string(tree.join("WINDOWS.md")) {
        return Some(text);
    }
    assert!(
        !tree.exists(),
        "FAILURE MODE: `.planning/` exists but `{LEDGER_REL}` is missing. The \
         broken-windows ledger is the source of truth for which behaviour changes are \
         consumer-observable, so deleting it silently disarms this tripwire and every \
         future undisclosed change ships unnoticed.\n\
         WHAT TO DO: restore `{LEDGER_REL}`, or update `LEDGER_REL` in this test if the \
         ledger moved — do not delete the assertion."
    );
    None
}

/// The migration chapter, or `None` when the whole `pmcp-book/src/` tree is absent.
///
/// Same two-branch guard as [`ledger`], applied independently: `pmcp-book/` is its
/// own entry in `Cargo.toml`'s `exclude` array, so the absent-tree fork has to be
/// evaluated per tree rather than once for both.
fn migration_chapter() -> Option<String> {
    let tree = repo_root().join("pmcp-book").join("src");
    if let Ok(text) = fs::read_to_string(repo_root().join(CHAPTER_REL)) {
        return Some(text);
    }
    assert!(
        !tree.exists(),
        "FAILURE MODE: `pmcp-book/src/` exists but `{CHAPTER_REL}` is missing. That \
         chapter is the ONLY place a downstream consumer learns of a behaviour change \
         that ships with no semver signal, so removing it repudiates every disclosure \
         the ledger records.\n\
         WHAT TO DO: restore `{CHAPTER_REL}`, or update `CHAPTER_REL` in this test if the \
         chapter was renamed — do not delete the assertion."
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
/// (`WINDOWS.md entry <id>`) AND that the id is not a prefix of a longer number,
/// so a citation of entry `23` cannot stand in for a missing entry `2`.
fn cites_entry(chapter: &str, id: u64) -> bool {
    let needle = format!("WINDOWS.md entry {id}");
    chapter.match_indices(&needle).any(|(at, _)| {
        chapter[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_ascii_digit())
    })
}

#[test]
fn every_consumer_observable_window_is_cited_in_the_migration_chapter() {
    let (Some(ledger_md), Some(chapter)) = (ledger(), migration_chapter()) else {
        return;
    };

    let marked: Vec<Window> = ledger_entries(&ledger_md)
        .into_iter()
        .filter(|entry| entry.description.contains(SENTINEL))
        .collect();

    // Positive control. A run that selects zero entries would then vacuously
    // satisfy "every selected entry is cited" — the same false green in a different
    // shape, and one this repository has recorded before. The non-empty assertion is
    // what makes this test's success mean something.
    assert!(
        !marked.is_empty(),
        "FAILURE MODE: no `{SENTINEL}` entry was found in `{LEDGER_REL}`, so this gate \
         checked nothing and would have passed regardless of the chapter's contents. \
         Either the sentinel was stripped from every entry, or the description field this \
         gate reads was renamed.\n\
         WHAT TO DO: confirm the entries plan 119-05 marked still carry the literal \
         `{SENTINEL}` in their `description`; if the marking convention changed, update \
         `SENTINEL` here — do not delete the assertion."
    );

    let uncited: Vec<u64> = marked
        .iter()
        .map(|entry| entry.id)
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
