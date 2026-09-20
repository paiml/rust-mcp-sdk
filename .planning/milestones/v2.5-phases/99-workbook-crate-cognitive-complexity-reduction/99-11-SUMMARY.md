---
phase: 99-workbook-crate-cognitive-complexity-reduction
plan: 11
subsystem: quality-gate / PMAT complexity
tags: [pmat, complexity, quality-gate, gate-closure, requirements-traceability]
requires:
  - "All 10 wave-1 refactor plans (99-01..99-10) merged into the base"
provides:
  - "Workspace-wide PMAT complexity gate verified at ZERO violations (CPLX-04 closed)"
  - "CPLX-01/02/03/04 marked Complete in REQUIREMENTS.md (matrix + checkboxes)"
affects:
  - "PR #279 CI complexity gate (now will go green)"
tech-stack:
  added: []
  patterns:
    - "pmat quality-gate --fail-on-violation --checks complexity is the authoritative oracle (not the stale jq JSON path)"
key-files:
  created:
    - .planning/phases/99-workbook-crate-cognitive-complexity-reduction/99-11-SUMMARY.md
  modified:
    - .planning/REQUIREMENTS.md
    - crates/pmcp-workbook-compiler/src/formula/token.rs
    - crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
decisions:
  - "Auto-fixed leftover rustfmt drift on two refactor-extracted helper files (cargo fmt) — required to clear the make quality-gate merge bar; behavior-preserving (formatting only)."
  - "Fuzz-build `error: current package believes it's in a workspace` lines are pre-existing, non-fatal (swallowed by `|| echo` in the test-fuzz recipe); NOT a regression from this phase."
metrics:
  duration: "~12 min"
  completed: 2026-06-17
  tasks: 3
  files: 3
---

# Phase 99 Plan 11: PMAT Complexity Gate Closure (CPLX-04) Summary

Verified the org-required PMAT complexity gate now passes workspace-wide with **zero violations** after all 21 cognitive-complexity functions were refactored across the three v2.3 workbook crates; the full relevant test suite stays green (no behavior drift), `make quality-gate` is green, and CPLX-01/02/03/04 are recorded Complete in the requirements matrix.

## What Was Done

### Task 1 — PMAT complexity gate (the CI oracle)
- Ran the EXACT CI command: `pmat quality-gate --fail-on-violation --checks complexity` (pmat 3.15.0, matching the CI pin).
- Result: **Quality Gate PASSED, Total violations: 0, exit 0.**
- Supplemental JSON filter (`pmat analyze complexity --max-cognitive 25 --format json | jq ... test("workbook") / test("pmcp-server-toolkit")`) returned **empty** — confirming clearance at both the 25 and recommended-23 tiers.
- Cross-referenced against `99-INVENTORY.txt`: **none of the original 21 file:function pairs remains flagged.**

### Task 2 — No gate-weakening + workspace test suite
- `.pmatignore` contains **no production crate** (`grep -E "pmcp-workbook|pmcp-server-toolkit"` → empty); exclusions remain only `fuzz/`, `packages/`, `examples/`.
- **No `#[allow(clippy::cognitive_complexity)]`** present in the refactored crates (would be a PMAT no-op per D-10-B anyway) — the gate was cleared by genuine refactoring, not suppression.
- `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler -p pmcp-server-toolkit`: **721 passed, 2 ignored, 0 failed** (25 suites) — penny-reconcile, quirk corpus, dialect linter, provenance gate, scalar_eval, backward_compat goldens all green. No behavior regression.

### Task 3 — `make quality-gate` merge bar + requirements closure
- `make quality-gate` (fmt-check, lint pedantic+nursery, build, test-all, audit, unused-deps, purity-check, validate-always): **PASSED end-to-end (exit 0)** — confirmed by the "✅ ALL TOYOTA WAY QUALITY CHECKS PASSED" banner and "purity-check PASSED (reader-free …)" in the full 4433-line output.
- Marked **CPLX-01, CPLX-02, CPLX-03, CPLX-04** complete in `.planning/REQUIREMENTS.md` — checkboxes `[ ]`→`[x]` and traceability rows `Pending`→`Complete`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Leftover rustfmt drift on two refactor-extracted files**
- **Found during:** Task 3 (`make quality-gate` first step `cargo fmt --all -- --check`).
- **Issue:** Wave-1 refactors left unformatted helper signatures / struct-pattern arms in `crates/pmcp-workbook-compiler/src/formula/token.rs` (`lex_delimited`, `scan_atom_run`, `classify_atom_char`) and `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs`. This failed the merge bar's fmt-check, blocking Task 3.
- **Fix:** `cargo fmt -p pmcp-workbook-compiler -p pmcp-workbook-runtime` (formatting-only; no logic change). `cargo fmt --all -- --check` then reported zero diffs.
- **Files modified:** `crates/pmcp-workbook-compiler/src/formula/token.rs`, `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs`.
- **Commit:** see final commit hash below.

## Deferred / Out-of-Scope Observations (NOT regressions)

- **Pre-existing fuzz-build noise:** the `test-fuzz` step emits `error: current package believes it's in a workspace when it's not` for each `cargo fuzz run` target. These are swallowed by the recipe's `|| echo "… completed"` (non-fatal) and stem from the standalone `fuzz/Cargo.toml` workspace-detection quirk — unrelated to this phase's files. `make quality-gate` still exits 0.
- **Pre-existing unused import:** `pmcp_code_mode::CodeExecutor` in `crates/pmcp-server-toolkit/src/code_mode.rs:557` emits a `warn(unused_imports)` during `cargo test`. Not gate-failing (`make quality-gate` lints only root `pmcp`), out of scope for this phase, untouched.

## Known Stubs

None.

## Threat Flags

None — verification + formatting + docs only; no new network/auth/file/schema surface.

## Self-Check: PASSED
- FOUND: .planning/phases/99-workbook-crate-cognitive-complexity-reduction/99-11-SUMMARY.md
- FOUND: .planning/REQUIREMENTS.md (CPLX-04 → Complete)
- PMAT gate re-run: PASSED, 0 violations, exit 0
- Workspace tests: 721 passed / 0 failed
- make quality-gate: exit 0, success banner present
