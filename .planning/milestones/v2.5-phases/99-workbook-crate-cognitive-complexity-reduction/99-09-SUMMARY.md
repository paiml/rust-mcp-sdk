---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 09
subsystem: pmcp-workbook-compiler
tags: [refactor, complexity, pmat-gate, behavior-preserving]
requires:
  - "PMAT gate (pmat quality-gate --fail-on-violation --checks complexity)"
provides:
  - "extract_function_tokens cog <=23 (was 29)"
  - "author_xlsx cog <=23 (was 29)"
  - "dag walk cog <=23 (was 25)"
affects:
  - PR #279 PMAT complexity gate (3 of 21 violations cleared; workspace total 21 -> 18)
tech-stack:
  added: []
  patterns:
    - "Helper extraction to remove nested-loop depth (extract_function_tokens)"
    - "Two-way match dispatch into leaf-vs-recursive helpers (dag walk)"
    - "Thin orchestrator over per-cell write helpers + format struct (author_xlsx)"
key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/dialect/linter.rs
    - crates/pmcp-workbook-compiler/src/dag/resolve.rs
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
decisions:
  - "No #[allow(clippy::cognitive_complexity)] used — PMAT ignores it (99-CONTEXT D-10-B); only genuine structural decomposition clears the gate."
  - "Targeted cog <=23 (recommended-23 tier) per 99-CONTEXT, not just <=25, to guarantee a clean pass."
metrics:
  duration: ~25m
  completed: 2026-06-16
  tasks: 3
  files: 3
requirements: [CPLX-02]
---

# Phase 99 Plan 09: Linter / Fixture-Author / DAG-Walk Complexity Reduction Summary

Behavior-preserving structural decomposition of the last three single-function compiler hotspots — `extract_function_tokens` (29), `author_xlsx` (29), and `walk` (25) — clearing them below the PMAT recommended-23 tier so PR #279's complexity gate drops from 21 to 18 violations, with zero behavior change.

## What Was Done

### Task 1 — Flatten `extract_function_tokens` and dag `walk` (commit 6aaf7f01)

- **`dialect/linter.rs::extract_function_tokens` (cog 29 -> <=23):** extracted the identifier-handling branch (which carried the nested whitespace-skip `while` and the `(`-lookahead) into two private helpers: `scan_function_token` (returns the recognised token plus the resume index) and `followed_by_open_paren`. The main scan loop is now flat. The emitted token list (including `_xlfn.` prefix stripping and string-literal skipping) is byte-for-byte unchanged.
- **`dag/resolve.rs::walk` (cog 25 -> <=23):** split the 8-arm recursive `match` into a thin two-way dispatch over `collect_leaf_refs` (the `Ref`/`Range`/`Name` resolution arms) and `walk_children` (the `BinaryOp`/`UnaryOp`/`Call` recursive-descent arms), with literal variants returning `Ok(())`. Traversal order and the resolved `cell_key` set are unchanged.

### Task 2 — Decompose `author_xlsx` (commit b18377fe)

- **`fixture_author.rs::author_xlsx` (cog 29 -> <=23):** the nested per-cell `match` (cell kind × paint) drove complexity. Extracted `write_cell` (owns the cell-kind dispatch) and `write_number_cell` (owns the paint dispatch); grouped the two paint formats into a `CellFormats` struct built once per call. `author_xlsx` is now a thin orchestrator over the cell loop + defined-names loop + save. The bytes/structure written (formats, formula cached `<v>` oracles, defined names) are unchanged, so the trusted-fixture provenance identity and reconcile oracles are preserved.

### Task 3 — Final gate + clippy + test verification (no code change)

Verification-only gate; the code lives in the Task 1/2 commits.

## Verification

PMAT oracle (`pmat quality-gate --fail-on-violation --checks complexity`, pmat 3.15.0):

- `extract_function_tokens`, `author_xlsx`, and `walk` — **all absent** from the violation list.
- **Zero** violations remain across all three target files.
- Workspace total: **21 -> 18** violations (the three this plan owns cleared).

Tests: `cargo test -p pmcp-workbook-compiler` — **315 passed; 0 failed; 1 ignored** (unchanged from baseline).

Clippy: `cargo clippy -p pmcp-workbook-compiler --all-features --tests -- -D warnings` — **clean**.

Note: the plan's `pmat analyze complexity ... --format json | jq '.violations[]'` recipe does not match pmat 3.15.0's JSON shape (it emits `.files[].functions[].metrics.cognitive`, not a `.violations[]` array, and `--format json` only lists files with a function over the *maximum* 25 — so the recommended-23-tier `walk` at cog 25 never appears there). The authoritative oracle used throughout was `pmat quality-gate --fail-on-violation --checks complexity`, which is exactly the CI gate command and does enumerate the recommended-23-tier violations including `walk`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Plan's jq verification recipe is wrong for pmat 3.15.0**
- **Found during:** Baseline verification (before Task 1)
- **Issue:** The plan's automated `<verify>` blocks use `pmat analyze complexity --max-cognitive 25 --format json | jq '.violations[] | ...'`. pmat 3.15.0 emits no `.violations[]` array (the schema is `.files[].functions[].metrics.cognitive`), and `--format json` truncates to the top-N files over the *maximum* (25), excluding the recommended-23-tier `walk` (cog 25) entirely. Following the literal recipe would have given false "no violations" readings.
- **Fix:** Used the authoritative gate command `pmat quality-gate --fail-on-violation --checks complexity` (the exact CI gate per 99-CONTEXT and CLAUDE.md) as the oracle, cross-checked against `--top-files 0 --format json` for the two cog-29 functions. No source change — verification-method correction only.
- **Files modified:** none (process correction)
- **Commit:** n/a

No `#[allow]`, no `.pmatignore` edit, no behavior change — exactly as the plan's success criteria required.

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-compiler/src/dialect/linter.rs (modified)
- FOUND: crates/pmcp-workbook-compiler/src/dag/resolve.rs (modified)
- FOUND: crates/pmcp-workbook-compiler/src/fixture_author.rs (modified)
- FOUND: commit 6aaf7f01 (Task 1)
- FOUND: commit b18377fe (Task 2)
- Gate confirms all three target functions cleared; tests green; clippy clean.
