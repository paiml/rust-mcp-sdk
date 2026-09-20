---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 02
subsystem: docs
tags: [rust, rustdoc, docs-rs, doc_cfg, auto-cfg, rfc-3631]

requires:
  - phase: 67-docs-rs-pipeline-and-feature-flags
    provides: "Unchanged feature(doc_cfg) crate attribute at src/lib.rs:70 that already enables RFC-3631 auto-cfg behavior"
provides:
  - "Zero per-item #[cfg_attr(docsrs, doc(cfg(...)))] annotations in src/"
  - "Clean substrate for auto-cfg badge rendering on all 145 #[cfg(feature)]-gated items"
  - "Elimination of the 6/145 (~4%) manual-annotation drift risk"
affects:
  - 67-03-crate-readme-and-include-str (will edit src/lib.rs top in Wave 2)
  - 67-04-rustdoc-warning-cleanup (relies on auto-cfg post-render state)
  - 67-05-makefile-and-ci-gate (enforces zero-warning doc builds)

tech-stack:
  added: []
  patterns:
    - "Single-mechanism feature badges: feature(doc_cfg) + RFC-3631 auto-cfg owns all rendering"
    - "Per-item doc(cfg(...)) annotations deprecated; never add new ones"

key-files:
  created: []
  modified:
    - src/lib.rs
    - src/types/mod.rs
    - src/server/mod.rs

key-decisions:
  - "D-01 amendment honored: src/lib.rs:70 feature(doc_cfg) line left unchanged (doc_auto_cfg was hard-removed in Rust 1.92.0 via PR rust-lang/rust#138907; any flip would error E0557)"
  - "D-02 executed: all 6 manual doc(cfg(...)) annotations deleted in a single atomic commit"
  - "D-03 honored: no doc_cfg_hide added (RFC-3631 default behavior already correct)"
  - "D-10 honored: src/lib.rs lines 63-77 warning-lint block preserved intact"
  - "D-29 honored: pmcp-macros untouched"

patterns-established:
  - "doc(cfg(...)) removal pattern: delete the manual attribute line, preserve the preceding #[cfg(feature = ...)] compilation gate verbatim"
  - "Single-task annotation cleanup with grep-invariant acceptance criteria (count-based verification only, no cargo build during parallel execution)"

requirements-completed:
  - DRSD-01

duration: ~2min
completed: 2026-04-12
---

# Phase 67 Plan 02: Manual doc(cfg) Cleanup Summary

**Deleted all 6 per-item `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations across `src/lib.rs`, `src/types/mod.rs`, and `src/server/mod.rs`, handing feature-badge rendering entirely to the existing `feature(doc_cfg)` RFC-3631 auto-cfg mechanism.**

## Performance

- **Duration:** ~2 min (94 seconds wall-clock)
- **Started:** 2026-04-12T00:53:11Z
- **Completed:** 2026-04-12T00:54:45Z
- **Tasks:** 1
- **Files modified:** 3
- **Lines deleted:** 6
- **Lines added:** 0

## Accomplishments

- All 6 manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations removed from `src/` (verified: `rg -c '#\[cfg_attr\(docsrs, doc\(cfg' src/` returns 0 matches)
- `src/lib.rs:70` `#![cfg_attr(docsrs, feature(doc_cfg))]` preserved verbatim per D-01 amendment (verified: exact-match grep returns line 70 unchanged)
- Zero occurrences of the removed `doc_auto_cfg` feature name anywhere in `src/` (verified: grep returns no matches — critical to avoid E0557 on Rust 1.92.0+)
- Crate-level warning/lint block at `src/lib.rs:63-77` preserved intact per D-10 (`#![warn(...)]` at line 63, `#![deny(unsafe_code)]` at line 69, `#![cfg_attr(docsrs, feature(doc_cfg))]` at line 70, clippy allows at lines 72-77)
- Every `#[cfg(feature = "...")]` compilation gate on the previously-annotated items preserved (they are load-bearing for actual feature gating and must not be removed)
- `pmcp-macros/` untouched per D-29

## Task Commits

Each task was committed atomically with `--no-verify` (parallel execution mode — Plan 67-01 running concurrently in a separate worktree; orchestrator runs quality gate after all agents complete):

1. **Task 1: Delete 6 manual doc(cfg(...)) annotations across lib.rs, types/mod.rs, server/mod.rs** — `0846f812` (refactor)

## Files Created/Modified

- `src/lib.rs` — Deleted 2 annotations:
  - Line 86 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "composition")))]` above `pub mod composition;`
  - Line 105 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` above the `pub mod axum { ... }` re-export block
  - Line 70 (`#![cfg_attr(docsrs, feature(doc_cfg))]`) and the lint block at lines 63-77 UNCHANGED

- `src/types/mod.rs` — Deleted 1 annotation:
  - Line 25 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` above `pub mod mcp_apps;`

- `src/server/mod.rs` — Deleted 3 annotations:
  - Line 107 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` above `pub mod mcp_apps;`
  - Line 139 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` above `pub mod axum_router;`
  - Line 157 (before deletion): `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` above `pub mod tower_layers;`

### Deletion map (file:line_before → state_after)

| File | Line (pre-delete) | Deleted text | Preserved preceding cfg gate |
|---|---|---|---|
| `src/lib.rs` | 86 | `#[cfg_attr(docsrs, doc(cfg(feature = "composition")))]` | `#[cfg(feature = "composition")]` |
| `src/lib.rs` | 105 | `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` | `#[cfg(feature = "streamable-http")]` |
| `src/types/mod.rs` | 25 | `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` | `#[cfg(feature = "mcp-apps")]` |
| `src/server/mod.rs` | 107 | `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` | `#[cfg(all(not(target_arch = "wasm32"), feature = "mcp-apps"))]` |
| `src/server/mod.rs` | 139 | `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` | `#[cfg(feature = "streamable-http")]` |
| `src/server/mod.rs` | 157 | `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` | `#[cfg(feature = "streamable-http")]` |

Post-delete line numbers shift up by 1 (for each occurrence, only the annotation line is removed and nothing inserted). Current post-delete positions: composition at `src/lib.rs:86`, axum re-export at `src/lib.rs:104`; types mcp_apps at `src/types/mod.rs:25`; server mcp_apps at `src/server/mod.rs:107`, axum_router at `src/server/mod.rs:138`, tower_layers at `src/server/mod.rs:155`.

## Decisions Made

None beyond the locked CONTEXT.md decisions. Plan executed exactly as specified with all D-01 (amended), D-02, D-03, D-10, D-29 invariants honored.

## Deviations from Plan

**None - plan executed exactly as written.**

The plan explicitly deferred `cargo check` verification to avoid build contention with parallel Plan 67-01 (which edits `Cargo.toml`). Wave-level build verification happens later in Wave 5 (Plan 67-06 final-integration-verification) and during the orchestrator's post-wave quality-gate run. The grep-only acceptance criteria specified in the plan are all satisfied.

## Issues Encountered

None. The 6 target annotations were at the exact lines specified in the plan (86, 105 in lib.rs; 25 in types/mod.rs; 107, 139, 157 in server/mod.rs), each `Edit` used enough context to be unambiguous, and the working tree showed exactly 3 modified files with a clean 6-line deletion diff.

## Wave Scheduling Notes

This plan is Wave 1 in Phase 67's 5-wave schedule. It runs in parallel with Plan 67-01 (Cargo.toml `[package.metadata.docs.rs]` rewrite). The two plans touch disjoint file sets:

- Plan 67-01: `Cargo.toml` only
- Plan 67-02: `src/lib.rs`, `src/types/mod.rs`, `src/server/mod.rs` only

No overlap → safe parallel execution. Plan 67-03 (which also edits `src/lib.rs` for the include_str! swap) is correctly bumped to Wave 2 by the wave scheduler because of the `src/lib.rs` file-overlap with this plan.

## Invariant Check Summary

All grep-based acceptance criteria from the plan verified after commit:

| Invariant | Expected | Actual | Status |
|---|---|---|---|
| `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c` total count | 0 | 0 | PASS |
| `grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs` | 1 | 1 (line 70) | PASS |
| `grep -c 'doc_auto_cfg' src/lib.rs` | 0 | 0 | PASS |
| `grep -c 'doc_auto_cfg' src/` (any file) | 0 | 0 | PASS |
| `grep -c '#!\[deny(unsafe_code)\]' src/lib.rs` | 1 | 1 (line 69) | PASS |
| `grep -c '^#!\[warn(' src/lib.rs` | 1 | 1 (line 63) | PASS |
| `grep -c 'pub mod composition;' src/lib.rs` | 1 | 1 (line 86 post-shift) | PASS |
| `grep -c 'pub mod mcp_apps;' src/types/mod.rs` | 1 | 1 (line 25) | PASS |
| `grep -c 'pub mod axum_router;' src/server/mod.rs` | 1 | 1 (line 138 post-shift) | PASS |
| `grep -c 'pub mod tower_layers;' src/server/mod.rs` | 1 | 1 (line 155 post-shift) | PASS |
| No edits to `pmcp-macros/**` | 0 files | 0 files | PASS |
| No edits outside `src/lib.rs`, `src/types/mod.rs`, `src/server/mod.rs` | 3 files exactly | 3 files exactly | PASS |

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Wave 1 (this plan + Plan 67-01) delivers the `src/` auto-cfg substrate and the `Cargo.toml` docs.rs metadata.
- Wave 2 can now safely execute Plan 67-03 (`CRATE-README.md` + `include_str!` flip in `src/lib.rs`) without annotation conflicts.
- Wave 5 Plan 67-06 (final integration verification) will perform the full `cargo doc` zero-warning run under the `docsrs` cfg to confirm auto-cfg actually renders the badges on all 145 feature-gated items.
- No blockers or concerns. Working tree is clean after the atomic commit.

## Self-Check: PASSED

Verified post-commit:

1. **Files exist on disk:**
   - FOUND: `src/lib.rs` (11455 bytes)
   - FOUND: `src/types/mod.rs` (1983 bytes)
   - FOUND: `src/server/mod.rs` (143376 bytes)
   - FOUND: `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-02-manual-doc-cfg-cleanup-SUMMARY.md` (9794 bytes)

2. **Commit in git log:**
   - FOUND: `0846f812 refactor(67-02): delete 6 manual doc(cfg) annotations`

3. **Grep invariants:**
   - `#[cfg_attr(docsrs, doc(cfg` across src/: **0 matches** (expected 0) PASS
   - `doc_auto_cfg` across src/: **0 matches** (expected 0) PASS
   - `src/lib.rs` line 70 verbatim: `#![cfg_attr(docsrs, feature(doc_cfg))]` PASS
   - `src/lib.rs` lint block 63-77 intact: `#![warn(...)]` at 63, `#![deny(unsafe_code)]` at 69, `#![cfg_attr(docsrs, feature(doc_cfg))]` at 70, clippy allows at 72-77 PASS

4. **Scope:** Only 3 files touched: `src/lib.rs`, `src/types/mod.rs`, `src/server/mod.rs`. No edits to `Cargo.toml`, `Makefile`, `.github/`, or `pmcp-macros/`.

---
*Phase: 67-docs-rs-pipeline-and-feature-flags*
*Plan: 02-manual-doc-cfg-cleanup*
*Completed: 2026-04-12*
