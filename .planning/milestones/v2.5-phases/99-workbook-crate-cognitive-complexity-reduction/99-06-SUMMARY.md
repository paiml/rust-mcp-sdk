---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 06
subsystem: pmcp-workbook-compiler/ingest
tags: [refactor, pmat-gate, cognitive-complexity, behavior-preserving]
requires:
  - PMAT 3.15.0 complexity gate (max-cognitive 25, recommended 23)
provides:
  - "ingest decomposed into a thin orchestrator over per-phase helpers (cog 57 -> 8)"
  - "references_external_workbook flattened via guard-clause helpers (cog 31 -> ~7)"
affects:
  - crates/pmcp-workbook-compiler/src/ingest/mod.rs
tech-stack:
  added: []
  patterns:
    - "Per-phase helper extraction (open -> per-sheet collect -> defined-names -> assemble)"
    - "Per-aspect worksheet readers (hidden rows/cols, widths, ranges, validations, notes, cells)"
    - "Early-return guard clauses for external-ref scan"
key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/ingest/mod.rs
decisions:
  - "Threaded mutable accumulators (external_links, findings, total_cells) through &mut params rather than returning tuples per helper, keeping each helper single-purpose and the orchestrator flat"
  - "Split the per-cell builder into cell_record + cell_formula so the external-link detection/finding emission is isolated and early-returnable"
metrics:
  duration: ~20m
  completed: 2026-06-16
  tasks: 3
  files: 1
requirements: [CPLX-02]
---

# Phase 99 Plan 06: ingest + references_external_workbook Complexity Reduction Summary

Behavior-preserving decomposition of the workbook ingest entry point: `ingest`
(cog 57, over the cog-50 hard cap) became a thin orchestrator over per-phase /
per-aspect helpers (cog 8), and `references_external_workbook` (cog 31) was
flattened with two guard-clause helpers (cog ~7) — both now clear the PMAT
complexity gate with the WorkbookMap output, LintFinding set/order, error
variants, and external-reference decision unchanged.

## What Was Built

### Task 1 — Decompose `ingest` (cog 57 → 8)
`ingest` previously inlined: workbook open, a ~200-line per-sheet loop building
each `SheetRecord` (with a deeply nested per-cell loop doing external-link
detection), the cell-cap DoS finding, defined-name collection, and final
`WorkbookMap` assembly. It was split into:

- `collect_sheet(ws, …) -> (SheetRecord, bool)` — builds one sheet, returns the
  record + whether the cell cap was hit.
- Per-aspect readers: `hidden_rows`, `hidden_cols`, `col_widths`, `merge_ranges`,
  `conditional_format_ranges`, `table_ranges`, `data_validations`, `sheet_notes`.
- `collect_cells(ws, …) -> (Vec<CellRecord>, bool)` — the bounded cell scan with
  early return on the `MAX_CELL_COUNT` DoS cap.
- `cell_record(cell, …) -> CellRecord` and `cell_formula(cell, …) -> Option<String>`
  — the per-cell builder, with external-link detection/finding isolated in
  `cell_formula` (early returns for non-formula / empty-formula).
- `collect_defined_names`, `cell_cap_finding`, `save_timestamp`, `source_extension`
  — the remaining tail phases.

`ingest` is now a flat orchestrator: open → loop `collect_sheet` → optional
cap finding → assemble `WorkbookMap`.

### Task 2 — Flatten `references_external_workbook` (cog 31 → ~7)
Extracted the two nested decision sub-steps into named helpers:
- `prev_byte_is_ident(bytes, i) -> bool` — distinguishes `Table1[1]` (structured
  table ref) from `[1]Sheet1!A1` (external link index).
- `is_external_bracket(inner, prev_is_ident) -> bool` — the path-vs-index
  classification for one bracketed token.

The main loop is now a single scan with one early `return true` on the first
external-ref match.

### Task 3 — Verify gate + clippy and commit
PMAT oracle (`pmat analyze complexity --max-cognitive 25 --top-files 0` and
`pmat quality-gate --fail-on-violation --checks complexity`) report no
`ingest/mod.rs` function as a violation. Clippy clean, fmt clean, tests green.

## Verification Results

| Check | Result |
| ----- | ------ |
| `ingest` cognitive | 57 → **8** (PMAT 3.15.0 oracle) |
| `references_external_workbook` cognitive | 31 → **~7** (cleared from violation output) |
| highest helper cognitive | 9 (`cell_formula`) — all ≤ 23 |
| `pmat analyze complexity --max-cognitive 25 --top-files 0` | no `ingest` path present |
| `pmat quality-gate --fail-on-violation --checks complexity` | no ingest / references_external_workbook mention |
| `cargo test -p pmcp-workbook-compiler` | 315 passed; 0 failed; 1 ignored |
| `cargo clippy -p pmcp-workbook-compiler --all-features --all-targets` | 0 warnings / 0 errors |
| `cargo fmt -p pmcp-workbook-compiler -- --check` | clean |

Regression guard `ingests_authored_colours_and_workbook_metadata` and the
external-link / defined-name / note unit tests all pass — the ingest output and
external-ref detection are byte-for-byte preserved.

## Deviations from Plan

None — plan executed as written. Tasks 1 and 2 are refactor-only (the plan
deferred the single commit to Task 3, per its `done` notes), so all three tasks
landed in one `refactor(99):` commit `042203d8` as the plan's Task 3 specifies.

No `#[allow(clippy::cognitive_complexity)]` added; no `.pmatignore` edit — the
gate was cleared by genuine structural decomposition only.

## Known Stubs

None.

## Commits

- `042203d8` — refactor(99): decompose ingest + references_external_workbook below PMAT gate

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-compiler/src/ingest/mod.rs (modified)
- FOUND: commit 042203d8
- PMAT oracle confirms ingest/mod.rs cleared from complexity violations
