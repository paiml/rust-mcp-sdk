---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 04
subsystem: workbook-compiler
tags: [pmat, cognitive-complexity, refactor, change-classification, ir-identity, rust]

requires:
  - phase: 99-CONTEXT
    provides: "PMAT gate oracle (cog <=23 target), refactor-only mandate, regression net"
provides:
  - "classify_cell_roles decomposed into per-role helpers, cleared from PMAT gate (was cog 74)"
  - "dependency_order flattened into a thin Kahn's-algorithm driver, cleared from PMAT gate (was cog 24)"
  - "change_class subsystem has ZERO PMAT complexity-gate violations"
affects: [99-08-final-gate, PR-279-pmat-gate]

tech-stack:
  added: []
  patterns:
    - "Per-decision predicate helpers (output_redefined / input_retyped / assumption_changed) feeding a thin classifier loop"
    - "Topo-sort decomposed into build_deps / ready_nodes / decrement_dependents stages"

key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/change_class/mod.rs
    - crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs

key-decisions:
  - "Used pmat quality-gate (not analyze complexity) as the oracle: the gate flags dependency_order at cog 24 against the recommended-23 tier; analyze complexity under-reported it, so the gate is authoritative."
  - "Extracted boolean predicates rather than inlining match arms, so classify_current_role reads as a flat 4-arm match and each predicate carries the CR-01 / ENUM-07 comment locally."
  - "Elided the needless lifetime on visited_precedents (owned Vec<String> return) to satisfy default clippy -D warnings."

patterns-established:
  - "Classifier loop -> per-cell delegate (classify_assumption short-circuits; classify_role_flip_away + classify_current_role for the rest; classify_removed_cell for the removed pass)"
  - "Kahn topo loop -> staged helpers (build_deps, ready_nodes, decrement_dependents) with the driver holding only loop control"

requirements-completed: [CPLX-02]

duration: ~12min
completed: 2026-06-16
---

# Phase 99 Plan 04: change_class Cognitive-Complexity Reduction Summary

**`classify_cell_roles` (cog 74, over the 50 hard cap) and `dependency_order` (cog 24, over the recommended-23 tier) were decomposed into named per-decision helpers and a thin orchestrator, clearing both from the PMAT complexity gate with zero behavior change (315 compiler tests still green).**

## Performance

- **Duration:** ~12 min
- **Tasks:** 3 (2 refactor tasks + 1 verify/clippy task; clippy fix committed separately)
- **Files modified:** 2

## Accomplishments

### Task 1 — Decompose `classify_cell_roles` (mod.rs)
Split the cog-74 per-cell diff loop into:
- `classify_assumption` (+ `assumption_changed` predicate) — the assumption-first hard-rule short-circuit (CR-01), returns whether the cell was fully handled.
- `classify_role_flip_away` — Input/Output flip-away detection.
- `classify_current_role` (+ `output_redefined` / `input_retyped` predicates) — the per-`Role` Output/Input arms (ENUM-07 allowed_values domain change preserved).
- `classify_removed_cell` — the removed-cells pass.

`classify_cell_roles` is now a thin two-loop orchestrator. The exact set and ordering of emitted `(ChangeClass, region)` tuples is preserved (verified by the unchanged change-class + backward_compat goldens). Top function cog in mod.rs now < 18.

### Task 2 — Flatten `dependency_order` (ir_identity.rs)
Split the cog-24 Kahn's-algorithm topo sort into:
- `visited_precedents` / `build_deps` — adjacency map construction.
- `ready_nodes` — zero-in-degree set, lexicographically sorted (determinism preserved).
- `decrement_dependents` — the post-emit in-degree decrement (the formerly nested inner loop).

`dependency_order` is now a thin driver. Deterministic ordering + residual-cyclic-append totality preserved.

### Task 3 — Gate + clippy verification
- `pmat quality-gate --fail-on-violation --checks complexity` reports **zero** `change_class` violations (overall exit 1 is from other unrelated crates owned by sibling plans).
- `cargo test -p pmcp-workbook-compiler`: 315 passed, 0 failed (matches pre-refactor baseline).
- `cargo clippy -p pmcp-workbook-compiler --all-features -- -D warnings`: clean.

## Verification

| Check | Result |
|-------|--------|
| `classify_cell_roles` in PMAT gate | cleared (was cog 74) |
| `dependency_order` in PMAT gate | cleared (was cog 24) |
| change_class subsystem PMAT violations | 0 |
| `cargo test -p pmcp-workbook-compiler` | 315 passed, 0 failed (== baseline) |
| `cargo clippy -p pmcp-workbook-compiler --all-features -- -D warnings` | clean |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Lifetime variance error on `decrement_dependents`**
- **Found during:** Task 2
- **Issue:** Initial extraction took `deps: &HashMap<&str, ...>` and `remaining: &mut HashMap<&str, ...>` with inferred (distinct) lifetimes; the mutable map is invariant, so the borrow checker rejected `remaining.get_mut`.
- **Fix:** Added a shared named lifetime `<'a>` tying the `deps` keys and `remaining` keys.
- **Files modified:** crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs
- **Commit:** 9bb3c927

**2. [Rule 1 - Bug] clippy::needless_lifetimes on `visited_precedents`**
- **Found during:** Task 3
- **Issue:** Extracted `visited_precedents<'a>` carried a `'a` on `&visited` that the owned `Vec<String>` return never used; default `cargo clippy -D warnings` flagged it.
- **Fix:** Elided the lifetime.
- **Files modified:** crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs
- **Commit:** 3cd7a6f0

### Note on oracle choice
`pmat analyze complexity --format json` (PR #279's documented query) under-reports cognitive complexity vs `pmat quality-gate` for these functions — e.g. it did not surface `dependency_order` at cog 24, while the gate did. The gate is the merge-blocking oracle, so it was used as the authoritative pass/fail signal here.

## Known Stubs

None — pure behavior-preserving refactor; no placeholders, mocks, or empty data sources introduced.

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-compiler/src/change_class/mod.rs (classify_cell_roles cleared)
- FOUND: crates/pmcp-workbook-compiler/src/change_class/ir_identity.rs (dependency_order cleared)
- FOUND: commit d0736c32 (Task 1)
- FOUND: commit 9bb3c927 (Task 2)
- FOUND: commit 3cd7a6f0 (clippy fix)
