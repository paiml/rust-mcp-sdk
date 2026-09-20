---
phase: 93-workbook-compiler-5-generalization-fixes-promote-gate
plan: 04
subsystem: workbook-compiler
tags: [manifest-synth, penny-reconcile, cellsource-seam, generalization, WBCO-02, WBCO-04, WBCO-06]
requires:
  - pmcp-workbook-runtime::Manifest/Role/Dtype/InputTier/AnnotationDecl (re-exported)
  - pmcp-workbook-runtime::sheet_ir executor + rounding (run_executor, excel_round/roundup/ceiling)
  - 93-02 WorkbookMap + cached oracle (ingest)
  - 93-03 CellSource trait + dialect linter (dialect)
provides:
  - manifest::synthesize (workbook-driven, workflow as a parameter — kills the §5 build_reference_manifest gap)
  - manifest::ratify + is_conformant (recorded BA sign-off, content-hash conformance)
  - manifest::WorkbookCellSource (the 93-02 ⋈ 93-03 wiring seam)
  - manifest::projections sanitize_capped + resolve_inline_list (BA-string info-flow caps + DV→enum)
  - reconcile::classifier (operand-anchored MismatchClass, non-numeric cached cells)
  - reconcile::drift::mismatch_severity (D-03 named-output ERROR / helper WARNING)
  - reconcile::reconcile + ReconcileReport (collect-all driver)
  - sheet_ir::eval (compiler-side bridge to the runtime pure-Rust executor)
affects:
  - crates/pmcp-workbook-compiler (manifest/, reconcile/, sheet_ir/ modules filled)
tech-stack:
  added: []  # no new deps — runtime executor re-used; NO pmcp-code-mode/SWC
  patterns: [re-export-never-redeclare, collect-all-findings, operand-anchored-tolerance, info-flow-string-caps]
key-files:
  created:
    - crates/pmcp-workbook-compiler/src/manifest/model.rs
    - crates/pmcp-workbook-compiler/src/manifest/projections.rs
    - crates/pmcp-workbook-compiler/src/manifest/synth.rs
    - crates/pmcp-workbook-compiler/src/manifest/ratify.rs
    - crates/pmcp-workbook-compiler/src/reconcile/classifier.rs
    - crates/pmcp-workbook-compiler/src/reconcile/drift.rs
    - crates/pmcp-workbook-compiler/src/sheet_ir/eval_bridge.rs
  modified:
    - crates/pmcp-workbook-compiler/src/manifest/mod.rs
    - crates/pmcp-workbook-compiler/src/reconcile/mod.rs
    - crates/pmcp-workbook-compiler/src/sheet_ir/mod.rs
    - crates/pmcp-workbook-compiler/src/lib.rs
decisions:
  - "ratify is reader-free: the source content hash is PASSED IN (the ingest/provenance stage already holds it) rather than re-opening the .xlsx with umya inside ratify — keeps the module off the reader and matches D-04 'recorded sign-off, not a re-computation'"
  - "the lighthouse customer-golden IR builders (build_supply_total_ir/build_coil_ir, the 7_Quote/5_Quantities chain) were NOT lifted — they are per-workbook golden, not generalizable; the o1_parity_suite proves parity with a representative dialect-formula set instead"
  - "drift.rs repurposed from the lighthouse parsed-vs-handbuilt equivalence gate (which depended on the customer golden IR) to the generic D-03 named-output/helper severity split"
  - "added a NonNumericMismatch class so a cached text/bool/blank divergence has an explicit audit class instead of falling into Unclassified — it is still graded by the same named-output/helper severity split"
metrics:
  duration: ~1 session
  completed: 2026-06-12
  tasks: 2
  files: 11
---

# Phase 93 Plan 04: Manifest Synth + Penny-Reconcile Summary

Workbook-driven manifest synthesis (colour/Guide/header → roles, workflow as a parameter) with BA-string info-flow caps and inline-DV enums, plus an operand-anchored penny-reconcile (no blanket abs-delta tolerance) graded by a D-03 named-output-ERROR / helper-WARNING split — all reconciling through the runtime's pure-Rust executor with no SWC/JS oracle.

## What Was Built

### Task 1 — Manifest synth + ratify + the CellSource wiring seam (WBCO-02/06)
- **`synth.rs`** — `synthesize(wb, rules, workflow)` PROPOSES a candidate `Manifest` from colour + Guide + headers over the real `WorkbookMap`. The **WBCO-02 §5 fix**: the `workflow` name is a PARAMETER, never a hardcoded literal and never a per-workbook reference-manifest builder. Two-layer `check_overlap` (named-range prefix vs manifest role) emits `manifest/role-conflict` errors.
- **`projections.rs`** — the BA-string info-flow boundary (T-93-04-INJ): `sanitize_capped` strips control chars, collapses whitespace, and truncates at `MAX_MEANING_LEN`/`MAX_UNIT_LEN`/`MAX_ENUM_LABEL_LEN` with an ellipsis; an overflow is a WARNING, never a block. Plus `resolve_inline_list` (inline DV literal ≤10 → closed enum; D-06).
- **`mod.rs`** — `WorkbookCellSource`: the **93-02 ⋈ 93-03 wiring seam**. The real `WorkbookMap` implements the 93-03 `CellSource` trait (converting `CellRecord`/`SheetRecord`/`DefinedNameRecord` → `CellView`/`SheetView`/`DefinedName`), so the linter now runs on the real workbook.
- **`ratify.rs`** — recorded BA sign-off (D-04): stamps `ratified`/`ratified_by`/`ratified_at`/`workbook_hash`, appends a truthful JSONL audit line; `is_conformant` binds to the content hash. Reader-free — the hash is passed in.
- **`model.rs`** — re-exports the runtime `Manifest`/`Role`/`Dtype`/`InputTier`/`AnnotationDecl` (never re-declared). Every `Manifest { … }` literal supplies `annotations: vec![]` + the ratification stamps.
- DV fork (D-06): inline≤10 → enum; range/named-range/formula/non-text → DYNAMIC input + a precise reason-code WARNING (WR-01-safe default). D-05: unclassifiable cells stay internal; a warning fires only when a cell LOOKS exposable (a bare hardcoded number).

### Task 2 — Penny-reconcile (WBCO-04)
- **`classifier.rs`** — operand-anchored `MismatchClass`: `RoundingBoundary` fires ONLY when the deciding `Expr` carries a ROUND/ROUNDUP/CEILING call, the operand sits within `BOUNDARY_EPSILON` of the boundary, AND the gap ≤ one operand-derived rounding step. **Grep-gate clean** of the literal abs-of-delta pattern. Non-numeric cached cells (text/bool/blank/`#REF!`/`#DIV/0!`) are handled without panic (`NonNumericMismatch` + `ErrorPropagation` routes).
- **`drift.rs`** — the **D-03 split**: `mismatch_severity` → `Error` for a named output (`Role::Output`, blocks emit), `Warning` for a helper cell.
- **`mod.rs`** — collect-all `reconcile` driver: `within_tol` is penny-tolerant (never exact-float, non-finite always out of tolerance); an absent required output is an `Unclassified` hard-fail. `ReconcileReport::has_errors()` (named-output) + `is_hard_fail()` (any Unclassified) are the emit gates.
- **`sheet_ir/eval_bridge.rs` + `mod.rs`** — compiler-side `eval` re-exporting the runtime executor + rounding (never re-declared). `loop_exec.rs`/`RoomAggregator` NOT lifted.

## O-1 Parity (pure-Rust reconcile, no SWC)

Confirmed by the **named `o1_parity_suite`** (`reconcile::tests::o1_parity_suite`), not a summary note: a representative dialect-formula set (ROUND half-away-from-zero at the decimal boundary, ROUNDUP, CEILING-to-multiple, SUM, `cost/(1-margin)` arithmetic, IF dispatch) reconciles via the runtime's pure-Rust `scalar_eval` + `sheet_ir` executor with NO JS oracle. No `pmcp-code-mode` dependency exists in the crate (AC grep + `Cargo.toml` confirm).

## Acceptance Criteria

| AC | Status |
|----|--------|
| `cargo test -p pmcp-workbook-compiler manifest::` (incl. 6 behavior tests) | PASS — 30 tests |
| No local Manifest re-declaration | PASS |
| WorkbookMap implements CellSource | PASS (`mod.rs:63`) |
| BA-string caps applied (MAX_MEANING/UNIT/ENUM_LABEL) + ba_string_metadata_capped | PASS |
| build_reference_manifest absent on non-test paths | PASS |
| `cargo test -p pmcp-workbook-compiler reconcile::` (incl. non-numeric + o1_parity) | PASS — 20 tests |
| GREP GATE: no literal abs-of-delta in classifier.rs | PASS |
| classifier imports runtime rounding anchors | PASS |
| No pmcp-code-mode/SWC dep | PASS |
| loop_exec.rs NOT lifted | PASS |
| Zero customer identifiers in manifest/ reconcile/ sheet_ir/ | PASS |

Full crate: 162 lib tests green, `cargo clippy --all-targets` clean (the crate's `#![deny(clippy::unwrap_used, expect_used, panic)]` value-path gate holds).

## Deviations from Plan

### [Rule 1 - Bug] Output cell must be a DAG node for an all-literal formula
- **Found during:** Task 2 (o1_parity_suite RED→GREEN)
- **Issue:** The runtime executor walks `toposort(dag)` order; a formula cell absent from the DAG is never computed (returned `Empty`). A literal-only formula (`ROUND(1594.925, 2)`) with an empty DAG was never evaluated.
- **Fix:** `add_node` the output cell to the DAG even when it has no dependency edges, so the executor walks it.
- **Files modified:** `crates/pmcp-workbook-compiler/src/reconcile/mod.rs` (test helper `eval_one`)
- **Commit:** d7d42ef5

### [Scope — generalization] Customer-golden IR builders NOT lifted
- The lighthouse `reconcile/mod.rs` shipped `build_supply_total_ir`/`build_coil_ir` (the `7_Quote!C11` / `5_Quantities` chain) and `drift.rs` shipped the parsed-vs-hand-built equivalence gate keyed on those builders. Both are per-workbook golden, not generalizable. They were intentionally NOT lifted; `drift.rs` was repurposed to the generic D-03 severity split and the O-1 parity was proven with a representative dialect-formula set. This is the explicit "do not copy lighthouse debt as-is" milestone posture.

### [Design] ratify is reader-free (hash passed in)
- The lighthouse re-opened the `.xlsx` with umya inside `ratify` to compute a content hash. Here the ingest/provenance stage already holds the canonical content hash, so `ratify` takes it as a parameter — keeping the module off the reader and matching D-04's "recorded sign-off, not a re-computation".

## Known Stubs

None — the `manifest::synthesize` → `manifest::ratify` and `reconcile::reconcile` paths are fully wired against the real runtime executor and the real WorkbookMap. The crate-root `compile_workbook` driver remains a `NotImplemented` stub (Plan 07 wiring, by design — out of this plan's scope).

## Threat Flags

None — no new network/auth/file-access/schema surface beyond the documented `<threat_model>` mitigations (T-93-04-INJ length-caps, T-93-04-TOL operand-anchored classifier, T-93-04-ENUM inline-only enums, T-93-04-PANIC non-numeric handling) all implemented.

## Self-Check: PASSED

All 10 created/modified source files present; both task commits (03e6f132, d7d42ef5) exist in git history.
