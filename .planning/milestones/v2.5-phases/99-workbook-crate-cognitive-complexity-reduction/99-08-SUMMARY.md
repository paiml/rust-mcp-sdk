---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 08
subsystem: pmcp-workbook-compiler / approval-corpus gate
tags: [refactor, pmat, cognitive-complexity, behavior-preserving]
requires:
  - "PR #279 PMAT complexity gate (org-required)"
provides:
  - "derive_case_grid cog 34 -> <=23 (gate clear)"
  - "no_seeded_value_outside_allowed cog 46 -> <=23 (gate clear)"
affects:
  - "crates/pmcp-workbook-compiler/src/gate/corpus.rs"
tech-stack:
  added: []
  patterns:
    - "helper extraction (push_*/numeric_boundaries; enum_input_domains/case_seeded_values_allowed/seeded_value_in_domain)"
    - "guard-clause flattening (let-else early return)"
    - "iterator .all() chains replacing nested for-loop + early-return"
key-files:
  created: []
  modified:
    - "crates/pmcp-workbook-compiler/src/gate/corpus.rs"
decisions:
  - "Behavior preserved exactly: case-grid rows + order and the seeded-value accept/reject decision are byte-identical; no #[allow], no .pmatignore edit"
  - "Used .all()/is_none_or() iterator chains over the original short-circuit for-loops (compiles + tests green on the active toolchain)"
metrics:
  duration: "~12 min"
  completed: "2026-06-16"
  tasks: 3
  files: 1
---

# Phase 99 Plan 08: Approval-Corpus Gate Complexity Reduction Summary

Behavior-preserving decomposition of the two flagged approval-corpus gate functions in `gate/corpus.rs` — `derive_case_grid` (cog 34) and `no_seeded_value_outside_allowed` (cog 46) — into named private helpers so both clear the PMAT cognitive-complexity gate (target ≤23) blocking PR #279, with the generated case grid and the seeded-value validation decision preserved exactly.

## What Was Built

### Task 1 — `derive_case_grid` (cog 34 → cleared)
Extracted the per-input case construction out of the manifest loop into three private helpers, leaving a thin orchestrator:
- `push_enum_cases` — appends one case per declared enum member (others at base default), in `allowed_values` order.
- `push_numeric_boundary_cases` — appends the numeric-boundary cases for a numeric input.
- `numeric_boundaries` — returns the ordered `(case_id, value)` rows: `default-step`, `default+step`, then declared `min`/`max` when present.

Guard-clause (`let-else`) flattening replaced the nested `if let` skip branches. The default-first ordering, grid row contents, BTreeMap seeds, and `MAX_CORPUS_CASES` truncation are unchanged.

Commit: `767cedee`

### Task 2 — `no_seeded_value_outside_allowed` (cog 46 → cleared)
Replaced the cases × seeded-values × allowed-set nested loop with iterator `.all()` chains over three helpers:
- `enum_input_domains` — builds the `cell -> allowed_values` domain map.
- `case_seeded_values_allowed` — `true` iff every enum-input seed in one case is a domain member (`let-else` for the non-object input; `is_none_or` for non-enum cells).
- `seeded_value_in_domain` — per-value check for a raw JSON string or a `CellValue::Text` seed; non-text seeds impose no constraint.

The accept/reject decision is identical: any out-of-domain text seed yields `false`, otherwise `true`; the `is_computed` re-export keep-alive line is retained.

Commit: `fc1ec8f6`

### Task 3 — Gate + clippy verification (folded into Tasks 1–2)
Verification was run after each task rather than as a separate commit (the refactor is already split into the two natural function-group commits the plan's `<specifics>` asked for). No separate Task-3 commit was created — see Deviations.

## Verification

- **PMAT oracle (scoped):** `pmat analyze complexity --max-cognitive 25 --format json --include crates/pmcp-workbook-compiler/src/gate/corpus.rs` → zero cognitive-complexity violations for both functions.
- **PMAT oracle (full workspace):** `pmat analyze complexity --max-cognitive 25 --format json` → no `gate/corpus.rs` entry in `.summary.violations`.
- **Tests:** `cargo test -p pmcp-workbook-compiler` → 315 passed, 0 failed, 1 ignored (identical to pre-refactor baseline).
- **Clippy:** `cargo clippy -p pmcp-workbook-compiler --all-features -- -D warnings` → clean.

## Deviations from Plan

**1. [Rule 3 - Process adaptation] No separate Task-3 commit**
- **Found during:** Task 3.
- **Issue:** The plan's Task 3 specified a single `refactor(99): decompose gate corpus functions below PMAT gate` commit, but Tasks 1 and 2 were already committed per-function (matching the CONTEXT.md `<specifics>` directive: "Commit per function-group; keep each diff reviewable on the already-large PR #279").
- **Resolution:** Task 3's verification (full-workspace PMAT oracle empty for corpus.rs, clippy `-D warnings` clean, tests green) was executed and confirmed before finalizing; the two per-function commits carry the complete refactor. No third code commit was warranted (no code changes remained).
- **Files modified:** none beyond Tasks 1–2.
- **Commit:** n/a (verification-only).

No behavior changes. No `#[allow(...)]` added. No `.pmatignore` edits.

## Known Stubs

None.

## Threat Flags

None — the refactor touches only an internal property-guard / case-grid builder; no new network, auth, file-access, or schema surface.

## Self-Check: PASSED

- FOUND: `.planning/phases/99-workbook-crate-cognitive-complexity-reduction/99-08-SUMMARY.md`
- FOUND commit `767cedee` (Task 1: derive_case_grid)
- FOUND commit `fc1ec8f6` (Task 2: no_seeded_value_outside_allowed)
- FOUND commit `13036c4c` (SUMMARY)
