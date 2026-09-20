---
phase: 92-bundlesource-served-tool-toolkit-module
plan: 06
subsystem: workbook-served-toolkit
tags: [bugfix, gap-closure, executor, fixture, regression-test, CR-01]
requires:
  - "pmcp-workbook-runtime executor + seed env (eval_bridge CellEnv::get)"
  - "validate_input tier-default + caller-overlay seeding (input.rs)"
  - "tax-calc@1.1.0 committed golden + byte-stability harness"
provides:
  - "caller inputs flow through calculate/explain/render_workbook (CR-01 closed)"
  - "seed-preserving executor literal arm (defense-in-depth against future input-literal bundles)"
  - "value-asserting regression test (calculate_honors_non_default_input)"
affects:
  - "crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs"
  - "crates/pmcp-server-toolkit (workbook served tools)"
tech-stack:
  added: []
  patterns:
    - "seed-preserving topo-walk: a validated caller seed wins over an IR literal of the same key"
    - "Role::Input cells absent from IR (executor seed contract) — output formulas Ref them, resolved from seed env"
key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/tests/support/fixture_gen.rs"
    - "crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs"
    - "crates/pmcp-server-toolkit/src/workbook/handler.rs"
    - "crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/executable.ir.json"
    - "crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock"
decisions:
  - "Removed the now-unused literal_text() helper rather than #[allow(dead_code)] — clean over suppressed."
  - "Kept BOTH fixes (generator omission + seed-preserving executor) — the executor guard is defense-in-depth so a future compiler-emitted bundle repeating the input-literal shape cannot silently reintroduce CR-01."
metrics:
  duration: "~12m"
  completed: "2026-06-11"
  tasks: 2
  files: 5
requirements: [WBSV-01, WBSV-02, WBSV-05]
---

# Phase 92 Plan 06: CR-01 Caller-Input Honoring Summary

Closed Blocker 1 (CR-01) from 92-VERIFICATION.md: caller-supplied inputs were silently
discarded — every `calculate`/`explain`/`render_workbook` call computed from the bundle's
baked-in defaults (gross_income=60000) regardless of caller input. Root cause: the fixture
generator emitted the three `Role::Input` cells as IR literals, and the executor's literal
arm unconditionally re-seeded them at topo-walk time, clobbering `validate_input`'s
caller-seeded values. Fixed by (1) omitting the input cells from the IR per the executor's
documented seed contract, (2) making the executor literal arm seed-preserving as
defense-in-depth, (3) regenerating the byte-stable golden, and (4) adding a value-asserting
regression test.

## What Was Built

### Task 1 — Generator omission + seed-preserving executor (commit 730e2d8f)
- **fixture_gen.rs `build_ir()`**: deleted the three `Role::Input` literal entries
  (`1_Inputs!B2` gross_income, `1_Inputs!B3` filing_status, `1_Inputs!B4` deductions).
  The output formulas still `Ref` these cells; those refs now resolve from the seed env, not
  from an IR literal. A doc comment explains the executor seed contract. The manifest
  `Role::Input` rows and `cell_map.inputs` are untouched, so `validate_input` still seeds and
  dtype/enum-gates the three inputs. Removed the now-unused `literal_text()` helper.
- **executor.rs literal arm**: `env = env.seed_cell(&key, v)` is now guarded by
  `if env.get(&key).is_none()` — a pre-seeded caller value of the same key is never
  overwritten. The doc comment states a caller seed wins over an IR literal.

### Task 2 — Golden regeneration + regression test (commit a7942abf)
- Regenerated the committed `tax-calc@1.1.0` golden via the ignored
  `regenerate_committed_golden` test: `executable.ir.json` lost the three input cell entries
  (now eight IR cells: four governed bracket cells + four output formulas), `BUNDLE.lock`
  `h_exec`/combined hashes updated to match.
- Added `calculate_honors_non_default_input` in handler.rs: asserts
  `outputs.taxable_income.value == 88000.0` for `gross_income=100000` (100000 − 12000 default
  deduction) and `68000.0` for `gross_income=80000`. References CR-01 / 92-VERIFICATION.md.
  This test FAILS (returning 48000.0) without the Task-1 fix.

## Verification

- `calculate_honors_non_default_input` PASSES (lib unit test).
- `golden_regeneration_is_byte_identical` PASSES — committed golden equals a fresh
  regeneration of the fixed generator.
- `golden_passes_boot_integrity` PASSES — regenerated golden still boots through the
  fail-closed loader.
- Full toolkit `--features workbook` suite: lib 172 passed, all integration binaries pass
  (35 + 7 + 7 + 5 + 3 + …), 0 failures.
- Runtime suite `pmcp-workbook-runtime`: 147 + 1 passed, 0 failures — seed-preserving change
  does not regress runtime.
- Input cells confirmed ABSENT as IR cell entries (Python structural check on
  `executable.ir.json` top-level keys); the three remaining `1_Inputs!B2/B3/B4` string matches
  are `Ref` operands inside output formulas, which is correct.
- `make quality-gate` PASSES (fmt --all, clippy pedantic+nursery, build, tests, audit, Phase
  91/92 purity gate).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed now-unused `literal_text()` helper**
- **Found during:** Task 1
- **Issue:** `literal_text()` was used only to emit the filing-status input literal; after
  removing the input literals it became dead code, which the workbook build flags
  (`#[warn(unused)]`).
- **Fix:** Deleted the `literal_text()` function. `CellValue` import remains used elsewhere.
- **Files modified:** crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
- **Commit:** 730e2d8f

### Note on the plan's grep-based acceptance check

The plan's Task-2 automated verify uses
`grep -E "1_Inputs!B2|1_Inputs!B3|1_Inputs!B4" executable.ir.json` and expects ZERO matches.
That command returns 3 matches because the output formulas legitimately `Ref` the input cells
by their address string (e.g. taxable_income = `1_Inputs!B2` − `1_Inputs!B4`). The acceptance
**intent** ("the regenerated golden IR omits the input cells [as literal cell entries]") IS
satisfied — verified structurally: the three cells are absent from the IR's top-level cell
map. The grep is a naive proxy for a structural property; the structural check is authoritative.

## Deferred Issues

None introduced by this plan. One pre-existing, out-of-scope warning
(`pmcp-server-toolkit/src/code_mode.rs:557` unused import) was already documented in
`deferred-items.md` and left untouched per the executor scope boundary; it does not block
`make quality-gate`.

## Known Stubs

None.

## Threat Flags

None — pure source edits + fixture regeneration; no new network/auth/file-access surface.
The two mitigations in the plan's threat register (T-92-06-01 seed-preserving literal arm,
T-92-06-02 served numbers reflect caller inputs) are both implemented and test-covered.

## Self-Check: PASSED
- FOUND: crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
- FOUND: crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs
- FOUND: crates/pmcp-server-toolkit/src/workbook/handler.rs
- FOUND: crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/executable.ir.json
- FOUND: crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock
- FOUND commit: 730e2d8f
- FOUND commit: a7942abf
