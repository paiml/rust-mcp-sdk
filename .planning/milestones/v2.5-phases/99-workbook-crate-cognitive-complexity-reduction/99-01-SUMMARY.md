---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 01
subsystem: pmcp-workbook-runtime / render
tags: [refactor, complexity, pmat-gate, xlsx-render, behavior-preserving]
requires:
  - "PR #279 PMAT complexity gate (CI-only)"
provides:
  - "render_xlsx decomposed below PMAT cog gate (was 93, now <=23 per fn)"
  - "render/mod.rs cleared from PMAT complexity violations (21 -> 20 workspace-wide)"
affects:
  - crates/pmcp-workbook-runtime/src/render/mod.rs
tech-stack:
  added: []
  patterns:
    - "Thin orchestrator over per-phase private helpers"
    - "Flat-match value dispatch helper (value/formula/bool/text/error)"
    - "Early-return guard clauses to flatten nesting"
key-files:
  created: []
  modified:
    - crates/pmcp-workbook-runtime/src/render/mod.rs
decisions:
  - "Split render_xlsx into init_workbook + render_sheet(loop) + save; render_sheet itself orchestrates apply_sheet_scaffold / build_top_left_text / replay_merges / write_cell"
  - "Extracted display_text (PASS 1 dispatch) and write_computed_value (PASS 2 dispatch) as separate flat-match helpers to keep both the map-builder and the writer loop under the gate"
  - "No #[allow(clippy::cognitive_complexity)] (no-op for PMAT per D-10-B); no .pmatignore edit"
metrics:
  duration: "~12 min"
  completed: 2026-06-16
  tasks: 3
  files-modified: 1
requirements: [CPLX-01]
---

# Phase 99 Plan 01: render_xlsx Cognitive-Complexity Reduction Summary

Decomposed `pmcp-workbook-runtime/src/render/mod.rs::render_xlsx` (the single highest cog in the phase at 93) into a thin orchestrator over seven named private helpers, clearing the org-required PMAT complexity gate while keeping XLSX output byte-identical — all 157 crate tests stay green.

## What Was Done

`render_xlsx` was a single function with a per-sheet loop nesting two inner per-cell loops (a PASS 1 top-left-text builder and a PASS 2 value writer), driving cog 93. It is now:

- `render_xlsx` — thin orchestrator: `init_workbook()` -> per-sheet `render_sheet()` loop -> `save_to_buffer()`.
- `init_workbook()` — builds the workbook with determinism-pinned document properties (fixed creation datetime + empty author).
- `render_sheet()` — per-sheet orchestrator: `apply_sheet_scaffold` -> `build_top_left_text` -> `replay_merges` (pre-existing) -> per-cell `write_cell`.
- `apply_sheet_scaffold()` — name, hidden flag, per-column widths, hidden columns.
- `build_top_left_text()` — PASS 1: validate each addr panic-free, build the merge-top-left display map.
- `display_text()` — flat-match deriving the display text for one cell (number/text/bool/fallback), with the non-finite-number `Err` guard.
- `write_cell()` — resolves `(row,col)`, skips merge-interior coords, dispatches to `write_computed_value`.
- `write_computed_value()` — flat-match value dispatch (finite number / text / bool / fallback literal), with the non-finite-number `Err` guard.

Pre-existing helpers (`replay_merges`, `write_number_cell`, `write_string_cell`, `cell_format`, etc.) were reused unchanged.

## Behavior Preservation

Zero behavior change. The write order is identical (PASS 1 builds the text map, merges replay top-left only, PASS 2 writes non-interior cells in descriptor order). All exact error paths are preserved as early returns:
- Malformed addr -> `RenderError::MalformedAddr`
- Malformed / single-cell merge -> `RenderError::MalformedMerge` (via unchanged `replay_merges`)
- Non-finite computed number -> `RenderError::NonFiniteValue` (preserved in both `display_text` PASS-1 and `write_computed_value` PASS-2)

The regression net (`render_xlsx_produces_valid_zip_container`, `render_xlsx_is_deterministic_byte_identical`, `render_xlsx_rejects_non_finite_computed_value`, `render_xlsx_surfaces_malformed_addr_as_error_not_panic`, `render_xlsx_writes_formula_with_finite_cached_result`, `render_xlsx_replays_merge_top_left_only`, `render_xlsx_rejects_single_cell_merge`, `render_xlsx_writes_text_and_bool_and_falls_back_on_error_value`, `render_xlsx_with_non_ascii_argb_renders_without_panic`) passed before and after with no assertion changes.

## Verification

| Check | Baseline | After |
| ----- | -------- | ----- |
| `pmat quality-gate --checks complexity` total violations | 21 | 20 |
| render/mod.rs in violations | render_xlsx @ cog 93 | none (empty) |
| `cargo test -p pmcp-workbook-runtime` | 157 passed | 157 passed |
| `cargo clippy -p pmcp-workbook-runtime --all-features -- -D warnings` | n/a | No issues found |
| `cargo fmt -p pmcp-workbook-runtime -- --check` | n/a | clean |

PMAT oracle used: `pmat quality-gate --fail-on-violation --checks complexity --format json` (the exact CI gate). The `analyze complexity` JSON drops files entirely once they clear, so the quality-gate `violations[]` list is the authoritative oracle — render/mod.rs no longer appears.

## Deviations from Plan

None - plan executed exactly as written. The plan's `analyze complexity | jq '.violations[]'` filter does not match the actual `analyze complexity` JSON shape (which uses `.files[].functions[]` and omits cleared files); I used the authoritative `pmat quality-gate ... --format json` `.violations[]` oracle instead, which is the CI gate and gives the definitive empty-for-render/mod.rs result.

## Known Stubs

None.

## Tasks

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Snapshot baseline + live complexity | (no code change) | render/mod.rs (read) |
| 2 | Decompose into per-phase helpers | e4cbd17f | crates/pmcp-workbook-runtime/src/render/mod.rs |
| 3 | Verify gate clears + behavior preserved | e4cbd17f | crates/pmcp-workbook-runtime/src/render/mod.rs |

Tasks 2 and 3 share one commit per the plan's commit instruction (`refactor(99): decompose render_xlsx below PMAT cog gate`).

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-runtime/src/render/mod.rs (fn render_xlsx present)
- FOUND: commit e4cbd17f
- FOUND: .planning/phases/99-workbook-crate-cognitive-complexity-reduction/99-01-SUMMARY.md
