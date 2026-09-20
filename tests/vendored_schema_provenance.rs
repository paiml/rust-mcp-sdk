//! Provenance tripwire for **every** vendored tree under `schema/vendored/`.
//!
//! # What this test is for
//!
//! `schema/vendored/` holds byte-for-byte copies of third-party MCP schemas,
//! each taken from an upstream repository at one pinned commit:
//!
//! - `ext-tasks/` — the tasks extension draft schema. Phase 114 drives every
//!   task-related wire value off those bytes.
//! - `core-2026-07-28/` — the core `2026-07-28` protocol schema. Phase 115
//!   drives `ttlMs`, `cacheScope`, `structuredContent` and `outputSchema` off
//!   those bytes.
//!
//! A reviewer's conclusions about wire correctness rest entirely on those copies
//! being unmodified. Nothing in the build reads them, so nothing in the build
//! would ever notice an edit. This test is what notices. For each tree it
//! recomputes the SHA-256 of every vendored file and asserts each digest is
//! recorded in that tree's own `PROVENANCE.md`.
//!
//! # Why this scans `schema/vendored/*` rather than one hard-coded path
//!
//! It used to hard-code a single `VENDORED_DIR` constant naming the `ext-tasks`
//! tree — one path, not a scan. That shape has a specific, silent failure mode:
//! vendoring a SECOND tree adds an unverified artifact and the suite stays
//! **green**, because it never looked. A green run after adding a tree would be
//! the failure, not the success.
//!
//! So the scan is now a runtime `read_dir` over every immediate subdirectory of
//! `schema/vendored/`, each requiring its own `PROVENANCE.md`, with
//! [`MINIMUM_VENDORED_TREES`] as the anti-vacuity floor and a dedicated test
//! ([`vendored_schema_every_tree_is_covered_and_the_scan_is_not_vacuous`])
//! asserting both known trees are discovered. A third tree is in scope the
//! moment it exists, without anyone remembering to edit this file.
//!
//! # What this test deliberately does NOT do
//!
//! It does not parse, validate, or assert anything about a schema's **content**
//! — not a type name, not a field, not a status string. This test is about
//! **attribution** only: these bytes are the bytes that were fetched, and this
//! record says where they came from. Wire shapes are asserted against the
//! vendored files by the plans that implement them —
//! `tests/v2_core_schema_facts.rs` does exactly that for `core-2026-07-28/`.
//!
//! # Idiom
//!
//! Modelled on `tests/v2_prohibited_error_codes.rs` (plan 113-29) and
//! `tests/v2_bounded_reads_tripwire.rs` (plan 113-21): runtime discovery from
//! `CARGO_MANIFEST_DIR`, no hard-coded file list, and failure messages that name
//! what a reader should do. The scanner primitives are re-stated here rather
//! than shared because a Rust integration test is its own crate and the files
//! cannot import each other; the IDIOM is deliberately identical so the
//! repository has one source-scanning shape, not two.
//!
//! Files are discovered with `read_dir` at runtime rather than listed, so a
//! newly vendored file is in scope automatically and **cannot be added
//! un-recorded**.
//!
//! # Zero new dependencies, and no subprocess
//!
//! The digest is computed IN-PROCESS with `sha2`, which is already a
//! non-optional `[dependencies]` entry of this crate (used by `shared::pkce`,
//! `types::mrtr`, `server::request_state` and the OAuth paths). An integration
//! test links the package's dependencies, so importing it adds nothing: plan
//! 114-01's threat register books `Cargo.toml` and `Cargo.lock` as
//! byte-unchanged, and they are.
//!
//! This deliberately replaced an earlier subprocess implementation that shelled
//! out to `shasum` / `sha256sum` and SKIPPED when neither was present. A skip
//! makes a tripwire always-green on the machines least likely to have the tool,
//! which is precisely the failure mode this file's own docs say must not exist.
//! Hashing in-process removes the skip path entirely: there is no environment in
//! which these assertions do not run.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The root that holds every vendored artifact tree, relative to the crate root.
///
/// Deliberately the ROOT and not one tree: see the module docs. Each immediate
/// subdirectory is a separately-attributed artifact with its own
/// `PROVENANCE.md`, and all of them are in scope.
const VENDORED_ROOT: &str = "schema/vendored";

/// The attribution record. Excluded from digest computation — it is the thing
/// the digests are recorded *in*, so digesting it would be circular.
const PROVENANCE_FILE: &str = "PROVENANCE.md";

/// A floor on how many files the scan must find **per tree**, so a passing run
/// can never mean "the directory was empty".
///
/// This is an anti-vacuity guard, **not** a manifest: it is deliberately a
/// minimum rather than an exact count or a file list, so vendoring a third file
/// puts that file in scope without editing this test.
const MINIMUM_VENDORED_FILES: usize = 2;

/// A floor on how many vendored TREES the scan must find.
///
/// The per-tree floor above cannot catch the failure this one catches: if the
/// enumeration of `schema/vendored/` returned nothing at all — a moved root, a
/// broken `read_dir`, a typo'd constant — every per-tree loop below would
/// iterate zero times and the whole suite would report green while asserting
/// nothing whatsoever.
///
/// Two, because two trees are vendored today (`ext-tasks` from Phase 114 and
/// `core-2026-07-28` from Phase 115). Like the file floor it is a MINIMUM, so a
/// third tree needs no edit here — but a *removed* tree does, and that edit is
/// the conversation this constant exists to force.
const MINIMUM_VENDORED_TREES: usize = 2;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vendored_root() -> PathBuf {
    repo_root().join(VENDORED_ROOT)
}

/// Every vendored artifact tree, discovered at runtime.
///
/// Immediate subdirectories only — the tree is the unit of attribution, because
/// a `PROVENANCE.md` describes one upstream repository at one pinned commit.
/// Files sitting loose directly under `schema/vendored/` are not trees and are
/// not returned; there are none, and one appearing would be a structural
/// mistake worth noticing rather than silently hashing against no record.
///
/// Sorted, so failure ordering is deterministic across platforms.
fn vendored_trees() -> Vec<PathBuf> {
    let root = vendored_root();
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(err) => panic!(
            "cannot read the vendored schema root {}: {err}\n\
             This tripwire asserts every vendored artifact is unmodified and attributed; if the \
             root is gone, so is every artifact under it.",
            rel(&root)
        ),
    };

    let mut trees: Vec<PathBuf> = entries
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    trees.sort();

    assert!(
        trees.len() >= MINIMUM_VENDORED_TREES,
        "found only {} vendored tree(s) under {} ({:?}), expected at least \
         {MINIMUM_VENDORED_TREES}.\n\
         This is an anti-vacuity guard: with an empty root, every per-tree check in this file \
         would loop over nothing and report green while asserting nothing at all.\n\n\
         WHAT TO DO: if a vendored tree was deliberately removed, delete its directory AND its \
         PROVENANCE.md in the same commit and lower this floor with a written reason. If it was \
         moved, fix VENDORED_ROOT. Do not lower the floor to make a red run green.",
        trees.len(),
        rel(&root),
        trees.iter().map(|p| tree_name(p)).collect::<Vec<_>>(),
    );

    trees
}

/// The directory name of a vendored tree (e.g. `ext-tasks`), for failure
/// messages that name the offending artifact and not just a path.
fn tree_name(tree: &Path) -> String {
    tree.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<unnamed>")
        .to_string()
}

fn provenance_path(tree: &Path) -> PathBuf {
    tree.join(PROVENANCE_FILE)
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Every vendored file in one tree, discovered at runtime, `PROVENANCE.md`
/// excluded.
///
/// Recurses, so a file cannot be hidden from the scan by putting it in a
/// subdirectory. Returns paths sorted, so failure messages are deterministic.
fn discover_vendored_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => panic!(
            "cannot read the vendored schema directory {}: {err}\n\
             This tripwire asserts the vendored artifact is unmodified; if the directory is \
             gone, so is the artifact.",
            rel(dir)
        ),
    };
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            discover_vendored_files(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) != Some(PROVENANCE_FILE) {
            out.push(path);
        }
    }
    out.sort();
}

/// Every vendored file in `tree`, sorted.
fn files_in(tree: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    discover_vendored_files(tree, &mut files);
    files
}

/// The SHA-256 of `path`'s bytes as lowercase hex.
///
/// Computed in-process, so there is no environment-dependent path and no skip:
/// either the file reads and hashes, or the test fails naming the file. The hex
/// encoding is lowercase to match what `shasum -a 256` prints, which is the form
/// `PROVENANCE.md` records.
fn sha256_of(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|err| {
        panic!(
            "cannot read {} to compute its SHA-256: {err}\n\
             This tripwire asserts the vendored artifact is unmodified; an unreadable file \
             cannot be verified.",
            rel(path)
        )
    });
    let digest = Sha256::digest(&bytes);
    digest.iter().fold(String::with_capacity(64), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

fn is_lower_hex(ch: char) -> bool {
    matches!(ch, '0'..='9' | 'a'..='f')
}

/// Every MAXIMAL run of lowercase-hex characters in `text`.
///
/// Maximal runs, not regex matches at arbitrary offsets: a 64-character SHA-256
/// digest contains a 40-character prefix, so a naive "does a 40-hex string
/// appear?" search is satisfied by a digest and proves nothing about a commit
/// SHA being recorded. Taking maximal runs and then filtering by exact length
/// makes the two kinds of hex distinguishable.
fn maximal_hex_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if is_lower_hex(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// One tree's `PROVENANCE.md` text, or a failure naming that tree as
/// unattributed.
///
/// This is FAILURE MODE 2. It is separated from the digest checks because a
/// missing record is a different defect from a mismatched one: there is nothing
/// to compare against, and the vendored bytes have no stated origin at all.
fn read_provenance_or_fail(tree: &Path) -> String {
    let path = provenance_path(tree);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "FAILURE MODE 2 — THE VENDORED ARTIFACT `{}` IS UNATTRIBUTED.\n\
             Could not read {}: {err}\n\n\
             The files in {}/ are a copy of a third-party schema. Without PROVENANCE.md nothing \
             records which upstream repository and commit they came from, whether they were \
             edited after the fetch, or what obligation is held against them.\n\n\
             WHAT TO DO: restore {} from git history (`git log -- {}`). Do NOT write a fresh \
             record from memory — re-fetch at a pinned commit and record the measured digests.",
            tree_name(tree),
            rel(&path),
            rel(tree),
            rel(&path),
            rel(&path),
        )
    })
}

/// The 64-hex digests recorded in `PROVENANCE.md`.
fn recorded_digests(provenance: &str) -> BTreeSet<String> {
    maximal_hex_runs(provenance)
        .into_iter()
        .filter(|run| run.len() == 64)
        .collect()
}

// ===========================================================================
// 1. Anti-vacuity: every tree's scan finds files.
// ===========================================================================

#[test]
fn vendored_schema_dir_is_discovered_at_runtime_and_is_not_empty() {
    for tree in vendored_trees() {
        assert!(
            tree.is_dir(),
            "{} does not exist. A vendored schema artifact is the offline, diff-able source for \
             every wire value the phase that vendored it writes; without it every such value is \
             an unreviewable claim. Restore it from git history.",
            rel(&tree)
        );

        let files = files_in(&tree);

        assert!(
            files.len() >= MINIMUM_VENDORED_FILES,
            "vendored tree `{}`: found only {} file(s) under {} ({:?}), expected at least \
             {MINIMUM_VENDORED_FILES}.\n\
             This is an anti-vacuity guard: with an empty or near-empty directory the digest \
             checks below would pass over nothing and report green while asserting nothing. If a \
             vendored file was deliberately removed, remove its digest from that tree's \
             PROVENANCE.md in the same commit and lower this floor with a written reason.",
            tree_name(&tree),
            files.len(),
            rel(&tree),
            files.iter().map(|p| rel(p)).collect::<Vec<_>>(),
        );
    }
}

// ===========================================================================
// 2. FAILURE MODE 2: a tree's PROVENANCE.md missing entirely.
// ===========================================================================

#[test]
fn vendored_schema_provenance_md_exists_so_the_artifact_is_attributed() {
    for tree in vendored_trees() {
        let provenance = read_provenance_or_fail(&tree);
        assert!(
            !provenance.trim().is_empty(),
            "FAILURE MODE 2 — THE VENDORED ARTIFACT `{}` IS UNATTRIBUTED.\n\
             {} exists but is empty. An empty record attributes nothing; it is indistinguishable \
             from no record at all, except that it is quieter.\n\n\
             WHAT TO DO: restore it from git history, or re-fetch that tree's vendored files at a \
             pinned commit and write the record from the measured digests.",
            tree_name(&tree),
            rel(&provenance_path(&tree))
        );
    }
}

// ===========================================================================
// 3. FAILURE MODE 1: a vendored file whose digest is not recorded.
// ===========================================================================

#[test]
fn vendored_schema_every_file_digest_is_recorded_in_provenance_md() {
    for tree in vendored_trees() {
        let provenance = read_provenance_or_fail(&tree);
        let recorded = recorded_digests(&provenance);

        for file in &files_in(&tree) {
            let digest = sha256_of(file);
            assert!(
                recorded.contains(&digest),
                "FAILURE MODE 1 — A VENDORED FILE WAS EDITED OR REPLACED WITHOUT UPDATING \
                 PROVENANCE.\n\n\
                 Vendored tree:     {}\n\
                 File:              {}\n\
                 Computed SHA-256:  {digest}\n\
                 Recorded digests:  {:?}\n\n\
                 That file is a byte-for-byte copy of a third-party schema and is a READ-ONLY \
                 reference artifact: nothing in the build reads it, and its whole value is that a \
                 reviewer can trust it is what upstream published. An edit — including a reformat, \
                 a line-ending conversion, or a whitespace fix — destroys that.\n\n\
                 WHAT TO DO:\n\
                 1. If the edit was accidental: `git checkout -- {}` and re-run.\n\
                 2. If upstream genuinely changed: do NOT patch in place. Re-fetch at a NEW pinned \
                 commit SHA and rewrite {} following its own § Change protocol — the pinned commit, \
                 its date, the fetch date, the sizes and every digest.",
                tree_name(&tree),
                rel(file),
                recorded,
                rel(file),
                rel(&provenance_path(&tree)),
            );
        }
    }
}

// ===========================================================================
// 4. FAILURE MODE 3: a recorded digest whose file is gone.
// ===========================================================================

#[test]
fn vendored_schema_every_recorded_digest_belongs_to_a_file_that_still_exists() {
    for tree in vendored_trees() {
        let provenance = read_provenance_or_fail(&tree);
        let recorded = recorded_digests(&provenance);
        assert!(
            !recorded.is_empty(),
            "vendored tree `{}`: {} records no SHA-256 digest at all (no 64-character \
             lowercase-hex run found).\n\
             A record with no digests cannot detect an edit, so the tripwire it is half of would \
             pass over nothing.\n\n\
             WHAT TO DO: recompute `shasum -a 256` for every file in {}/ and record each digest \
             in that record's file table.",
            tree_name(&tree),
            rel(&provenance_path(&tree)),
            rel(&tree),
        );

        let files = files_in(&tree);
        let computed: BTreeSet<String> = files.iter().map(|file| sha256_of(file)).collect();

        for digest in &recorded {
            assert!(
                computed.contains(digest),
                "FAILURE MODE 3 — A STALE ENTRY: PROVENANCE RECORDS A DIGEST FOR A FILE THAT NO \
                 LONGER EXISTS.\n\n\
                 Vendored tree:     {}\n\
                 Recorded SHA-256:  {digest}\n\
                 Files present:     {:?}\n\
                 Their digests:     {:?}\n\n\
                 Every digest in the record must belong to a file on disk. A digest left behind \
                 after its file was deleted or renamed makes the record describe an artifact that \
                 is not there — and a reader checking the record against the directory would be \
                 comparing against a ghost.\n\n\
                 WHAT TO DO: if the file was deliberately removed, delete its row from {} in the \
                 same commit. If it was renamed, update the row's local path AND re-verify the \
                 digest still matches.",
                tree_name(&tree),
                files.iter().map(|p| rel(p)).collect::<Vec<_>>(),
                computed,
                rel(&provenance_path(&tree)),
            );
        }
    }
}

// ===========================================================================
// 5. No record can ever degrade into "fetched from main".
// ===========================================================================

#[test]
fn vendored_schema_provenance_pins_a_full_40_character_commit_sha() {
    for tree in vendored_trees() {
        let provenance = read_provenance_or_fail(&tree);
        let pins_a_commit_sha = maximal_hex_runs(&provenance)
            .iter()
            .any(|run| run.len() == 40);

        assert!(
            pins_a_commit_sha,
            "vendored tree `{}`: {} contains no full 40-character commit SHA.\n\n\
             `main` is a moving, force-pushable ref. A record that says \"fetched from main\" \
             cannot be reproduced by anyone and cannot detect upstream drift, which defeats the \
             entire purpose of vendoring. The pin must be a SHA.\n\n\
             Note this assertion counts MAXIMAL hex runs of exactly 40 characters. A 64-character \
             SHA-256 digest contains a 40-character prefix, so a looser search would be satisfied \
             by a digest and would prove nothing.\n\n\
             WHAT TO DO: resolve the SHA with `gh api repos/<owner>/<repo>/commits/main --jq \
             .sha`, re-fetch AT THAT SHA, and record the full 40 characters.",
            tree_name(&tree),
            rel(&provenance_path(&tree)),
        );
    }
}

// ===========================================================================
// 6. FAILURE MODE 4: a vendored tree the scan never looked at.
// ===========================================================================

/// The assertion whose absence was the whole defect this file was reshaped to
/// fix.
///
/// Tests 1-5 all iterate `vendored_trees()`. If that enumeration silently
/// returned fewer trees than exist — or the wrong ones — every one of them would
/// pass having skipped an unverified artifact, which is indistinguishable from
/// success in the output. This test looks at the enumeration itself.
#[test]
fn vendored_schema_every_tree_is_covered_and_the_scan_is_not_vacuous() {
    let trees = vendored_trees();
    let names: Vec<String> = trees.iter().map(|tree| tree_name(tree)).collect();

    assert!(
        trees.len() >= MINIMUM_VENDORED_TREES,
        "the scan of {} found {} tree(s) ({names:?}), below the floor of \
         {MINIMUM_VENDORED_TREES}.\n\n\
         WHAT TO DO: see MINIMUM_VENDORED_TREES' doc comment. A removed tree is a deliberate \
         decision that must be written down, not a floor to lower quietly.",
        VENDORED_ROOT,
        trees.len(),
    );

    // The two trees that exist today, named explicitly. This is NOT a manifest —
    // the floor above is what makes a THIRD tree in-scope automatically — it is a
    // guard against the enumeration returning the wrong two, or against one of
    // these two being deleted without the deletion being noticed.
    for expected in ["ext-tasks", "core-2026-07-28"] {
        assert!(
            names.iter().any(|name| name == expected),
            "FAILURE MODE 4 — A VENDORED TREE IS NOT COVERED BY THE PROVENANCE SCAN.\n\n\
             Expected tree:     {expected}\n\
             Trees discovered:  {names:?}\n\
             Scanned root:      {VENDORED_ROOT}\n\n\
             Every check in this file iterates the discovered trees. A tree that is not \
             discovered is not checked, and the suite reports GREEN — a green run after adding \
             or moving a tree is the failure, not the success.\n\n\
             WHAT TO DO: if `{expected}` was deliberately removed, delete its expectation here \
             and lower MINIMUM_VENDORED_TREES in the same commit, with a written reason. If it \
             was renamed or moved, the artifact it attributes moved with it — update this name \
             and every reference to the old path.",
        );
    }
}
