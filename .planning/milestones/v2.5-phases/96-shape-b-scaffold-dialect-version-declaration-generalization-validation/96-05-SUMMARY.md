---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
plan: 05
subsystem: testing
tags: [excel-quirk, reconcile, scalar_eval, within_tol, fixture-authoring, WBEX-02]

# Dependency graph
requires:
  - phase: 96-03
    provides: fixture_author (WorkbookSpec/AuthoredCell author API + env-gated regenerate_fixtures generator) + committed leap1900-probe.xlsx + SPIKE-1900-leap.md disposition A
  - phase: 96-04
    provides: the reemit_loan compile->load->serve pattern reused for the per-quirk retrieve-and-grade harness
  - phase: 93
    provides: compile_workbook_with_fixture_override + reconcile::within_tol/TOL penny path + the trusted-fixture override
provides:
  - "WBEX-02 Excel-quirk corpus: 8 quirks across both D-08 layers (scalar_eval unit tests + penny-reconcile mini fixtures)"
  - "quirks_reconcile.rs: a #[cfg(test)] harness that retrieves the real recomputed value + cached oracle and grades via the real within_tol penny path (cannot pass on compile-success alone), a production-refusal spot check, and a quirk->WBEX-02 traceability map"
  - "5 committed quirk .xlsx fixtures (+ provenance-override.json + gen.json sidecars); the 1900-leap reconcile fixture reuses the committed leap1900-probe.xlsx"
affects: [WBEX-02]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-layer quirk encoding (D-08): a fast scalar_eval unit assertion + a penny-reconcile mini fixture per numerically-expressible quirk"
    - "Retrieve-and-grade reconcile harness: load bundle -> seed inputs -> run_executor -> read computed at the reconcile cell key -> within_tol vs the cached oracle (T-96-14b: cannot pass on compile-success alone)"
    - "Empty-cell-as-0 via a 2-arg IF (IF(cond,then) with no else -> Empty) since an ABSENT range member is a hard #REF! on this runtime, not Empty"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/quirks_reconcile.rs
    - crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-half-rounding.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-negative-rounding.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-empty-coercion.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-float-boundary.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-text-coercion.xlsx
  modified:
    - crates/pmcp-workbook-runtime/src/scalar_eval.rs
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/src/lib.rs

key-decisions:
  - "8-quirk corpus across both layers (D-09 ~7-9 cap): the four roadmap-named (1900-leap, empty-cell coercion, error propagation, half-rounding) + four curated (negative-rounding sign, text->number coercion, explicit #DIV/0! propagation, float boundary 0.1+0.2)"
  - "Error propagation (named) is the plan-sanctioned scalar_eval-only stand-in: the runtime Div binop clamps zero-divisor NaN->0 for kernel byte-parity (WR-02/IN-03) and Excel errors short-circuit at preflight_error, so a numeric reconcile fixture with a numeric oracle is not expressible; the explicit #DIV/0! curated quirk is the same class. The OTHER three named quirks each have a real reconcile fixture."
  - "Empty-cell coercion reconcile uses A2 + (A1=IF(A2>9999,1) -> Empty), NOT a blank cell in a SUM range, because an absent range member is a hard #REF! on this runtime (executor.rs Pitfall-5), whereas a 2-arg IF deterministically produces Empty (semantics::branch None -> Empty)"

patterns-established:
  - "quirk_reconcile_specs() in fixture_author.rs is the single env-gated generator source for the quirk corpus (extends the 96-03 regenerate_fixtures generator)"

requirements-completed: [WBEX-02]

# Metrics
duration: ~40min
completed: 2026-06-15
---

# Phase 96 Plan 05: WBEX-02 Excel-Quirk Fixture Corpus Summary

**An 8-quirk Excel-quirk corpus encoded in BOTH D-08 layers — fast `scalar_eval` unit assertions (runtime crate) and mini penny-reconcile fixtures (compiler crate) graded through the REAL `within_tol` path against cached oracles — proving reconcile determinism across a corpus of Excel edge cases beyond the single `tax-calc` golden, with a production-refusal spot check and an explicit quirk→WBEX-02 traceability map.**

## Performance

- **Duration:** ~40 min
- **Tasks:** 2
- **Files:** 3 modified, 6 created (5 quirk .xlsx + quirks_reconcile.rs; each fixture also carries a provenance-override.json + gen.json sidecar)

## Accomplishments

- **Layer 1 (scalar_eval, runtime crate):** 8 new `#[test]` quirk assertions, each documenting its `{formula+context, cached Excel oracle, runtime expected}` tuple. The coercion quirks pin the operator/function context explicitly (empty-cell in additive `+`; text→number in `*` with the pinned `+`-concat divergence). The half-rounding tests assert against `excel_round`/`excel_roundup` (the source of truth), not a naive round. The 1900-leap test asserts the `>59` boundary + `+1` serial offset components (IF is a Call owned by the semantics layer) per SPIKE-1900-leap.md — no DATE added.
- **Layer 2 (penny-reconcile, compiler crate):** `quirks_reconcile.rs` compiles each numerically-expressible quirk fixture via the trusted override, loads the bundle through the generic toolkit loader, seeds the authored inputs, runs the runtime executor, **retrieves the recomputed value AND the cached oracle**, and grades them through the real `reconcile::within_tol` penny path (TOL=0.01). A wrong-oracle negative test proves the value is graded, not compile-success (T-96-14b). No exact-float `==` on money anywhere (T-96-14).
- **Production-refusal spot check (T-96-13):** bare `compile_workbook` (Enforce) refuses a quirk fixture's `fullCalcOnLoad` staleness — only the `#[cfg(test)]` override accepts it.
- **Quirk→WBEX-02 traceability map:** a table in the `quirks_reconcile.rs` module doc mapping all 8 quirks to their layer-1 + layer-2 witnesses, with the explicit stand-in rationale for error propagation.

## Quirk → WBEX-02 Traceability (corpus = 8)

| # | Quirk | Class | scalar_eval (layer 1) | penny-reconcile (layer 2) |
|---|-------|-------|-----------------------|---------------------------|
| 1 | 1900 leap-year | NAMED | `quirk_1900_leap_serial_offset_components` | `leap1900-probe.xlsx` (SPIKE disposition A, reused; re-graded) |
| 2 | empty-cell coercion | NAMED | `quirk_empty_cell_coerces_to_zero_in_additive_context` | `quirk-empty-coercion.xlsx` (`A2 + (A1=IF(A2>9999,1) -> Empty)` → 5) |
| 3 | error propagation | NAMED | `quirk_error_propagates_through_arithmetic` | scalar_eval-only stand-in (documented runtime limitation) |
| 4 | half-rounding boundaries | NAMED | `quirk_half_rounding_uses_excel_round_source_of_truth` | `quirk-half-rounding.xlsx` (`ROUND(1594.925,2)` → 1594.93) |
| 5 | negative-value rounding sign | curated | `quirk_negative_rounding_sign_away_from_zero` | `quirk-negative-rounding.xlsx` (`ROUND(-2.5,0)` → -3) |
| 6 | text→number coercion | curated | `quirk_text_to_number_coercion_is_context_specific` | `quirk-text-coercion.xlsx` (`"5.5"*2` → 11) |
| 7 | explicit `#DIV/0!` propagation | curated | `quirk_explicit_div_zero_error_propagates` | scalar_eval-only stand-in (same class as #3) |
| 8 | float boundary (`0.1+0.2`) | curated | `quirk_float_boundary_compares_within_tol_not_exact` | `quirk-float-boundary.xlsx` (`0.1+0.2` ≈ 0.3 within TOL) |

Named-quirk reconcile coverage: 3 of 4 named quirks have a real reconcile fixture (1900-leap, empty-cell coercion, half-rounding); error propagation is the plan-sanctioned scalar_eval-only documented stand-in.

## Task Commits

1. **Task 1: scalar_eval unit-test layer** — `e3cce105` (test)
2. **Task 2: quirk reconcile corpus + harness + traceability map** — `7fa7458f` (feat)

**Plan metadata:** (this SUMMARY + STATE + ROADMAP + REQUIREMENTS) — final docs commit.

## Key Technical Findings

- **An absent range member is a hard `#REF!`, not Empty** (`executor.rs` Pitfall-5, line ~328-336). The first empty-cell-coercion design (`SUM(A1:A3)` over blank cells) failed the compile-time reconcile because the blank members surfaced as `#REF!` that SUM propagated. Empty-cell-as-0 applies to a cell that *resolves* to Empty — so the fixture was redesigned to produce the empty cell deterministically via a 2-arg `IF` (`IF(A2>9999,1)` with no else → Empty per `semantics::branch`), then `A2 + A1` adds the number to the empty cell (→ 5).
- **Error / `#DIV/0!` propagation is a preflight/short-circuit concept, not a reconcilable numeric oracle on this runtime.** The `Div` binop clamps a zero-divisor NaN to `0.0` for byte-parity with the locked JS kernel (`scalar_eval.rs` WR-02 / IN-03), and an Excel error short-circuits at `preflight_error` before reaching a numeric reconcile cell. Both are proven faithfully at the scalar_eval layer (where `preflight_error` IS the mechanism under test).
- **The text→number coercion operand must be seeded as text in the harness.** A non-input text constant cell is not auto-resolved in a standalone executor run, so the harness seeds A1 as `CellValue::Text("5.5")` — which is exactly on-point for the quirk (the operand IS a string the `*` context coerces).
- The `<v>` for an empty formula result is written by rust_xlsxwriter as `0`; A1 is a helper cell (not a named output), so a helper-cell reconcile delta is a non-blocking Warning while the named output B1 reconciles cleanly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Empty-cell coercion fixture re-designed (blank-range → 2-arg IF)**
- **Found during:** Task 2 (the `quirk-empty-coercion` fixture failed the compile-time reconcile).
- **Issue:** `SUM(A1:A3)` over blank cells produced `#REF!` (absent range member = hard #REF! on this runtime), not the empty-cell-as-0 the quirk intends.
- **Fix:** Encode the empty cell deterministically as `A1 = IF(A2>9999,1)` (false 2-arg IF → Empty) and `B1 = A2 + A1` → 5. The quirk semantics (empty cell coerces to 0 in additive arithmetic) are preserved and now reconcile through the real penny path.
- **Files modified:** `crates/pmcp-workbook-compiler/src/fixture_author.rs` (spec) + the regenerated `quirk-empty-coercion.xlsx`.
- **Commit:** `7fa7458f`

**2. [Rule 1 - Bug] Text-coercion harness seeds the text operand**
- **Found during:** Task 2 (`quirk-text-coercion` recomputed `Error(Value)` in the standalone executor run).
- **Issue:** The non-input text constant A1 ("5.5") was not resolved in the harness's separate executor run.
- **Fix:** Added a `text_inputs` field to the harness `QuirkCase` and seeded A1 as `CellValue::Text("5.5")` — correct for the quirk (the operand is genuinely a string the `*` context coerces).
- **Files modified:** `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs`.
- **Commit:** `7fa7458f`

**3. [Rule 3 - Blocking] Reverted incidental rewrites of committed fixtures**
- **Found during:** Task 2 (running `regenerate_fixtures` to author the quirk fixtures also rewrote `leap1900-probe.xlsx` and `loan-calc.xlsx` with 1-byte non-deterministic zip-metadata diffs).
- **Fix:** `git checkout --` restored both committed fixtures (the plan forbids editing existing fixtures). The quirks + leap-probe self-tests pass against the original committed bytes.

### Plan-expected dispositions (not deviations)

- The plan's `<action>` listed `SUM(A1:A3)` as a candidate empty-coercion shape — superseded by the 2-arg-IF shape above (the plan also says "fall back / document if not expressible"; the quirk IS expressible, just via a different whitelisted shape).
- Error propagation as a scalar_eval-only documented stand-in is explicitly permitted by the plan ("UNLESS Plan-03 marks it impossible / the runtime makes it impossible — then the scalar_eval assertion + a note stands in").

## Threat Model Outcomes

- **T-96-13 (a quirk fixture weakening the freshness/reconcile gate):** mitigated — `production_compile_refuses_a_quirk_fixture` asserts bare `compile_workbook` (Enforce) refuses the class; the override is `#[cfg(test)]`-only; reconcile grades every value through the unmodified `within_tol` penny path; the gate code is untouched.
- **T-96-14 (exact-float `==` smuggled into a money assertion):** mitigated — every numeric compare goes through `within_tol` (±0.01); no exact-float `==` on money in the harness (the float-boundary quirk is the in-corpus proof of why).
- **T-96-14b (a quirk test silently degrading to compile-success only):** mitigated — the harness retrieves the real recomputed value + cached oracle at the reconcile cell key and grades via `within_tol`; `a_wrong_oracle_does_not_reconcile_proving_the_value_is_graded` proves a wrong value fails.
- **T-96-15 (leap quirk forcing date functions):** accept-by-design held — the 1900-leap reconcile fixture reuses the disposition-A `leap1900-probe.xlsx` (serial f64 arithmetic); no DATE/DATEVALUE added; dialect crate byte-unchanged.
- **T-96-SC (package installs):** no new external packages — no legitimacy checkpoint required.

## Verification

- `cargo test -p pmcp-workbook-runtime scalar_eval` → 12 passed (8 new quirk tests + 4 pre-existing).
- `cargo test -p pmcp-workbook-compiler quirks` → 5 passed.
- `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler` → 468 passed, 1 ignored (the env-gated generator), 0 failed.
- `cargo fmt -p pmcp-workbook-runtime` / `-p pmcp-workbook-compiler` → clean.
- `cargo clippy -p pmcp-workbook-runtime --all-targets` / `-p pmcp-workbook-compiler --all-targets` → zero warnings.
- `git diff` on `crates/pmcp-workbook-dialect/` and the existing `tax-calc*`/`loan-calc*`/`leap1900*` fixtures → empty (no WHITELIST change, no edits to existing fixtures).

## Deferred Full-Gate Runs

- The repo-wide `make quality-gate` (pedantic+nursery clippy across `--features full`, workspace fmt, audit, full workspace test) was **NOT** run for these per-task commits — only the targeted `pmcp-workbook-runtime` + `pmcp-workbook-compiler` fmt/clippy/test were run (all clean). This plan touches only those two crates under `#[cfg(test)]` + test fixtures. As this is the LAST plan of Phase 96, the orchestrator's full verifier (and a `make quality-gate` before any PR, per CLAUDE.md) should run next.

## Known Stubs

None — every quirk has a live witness (a scalar_eval assertion and, for the numerically-expressible quirks, a penny-reconcile fixture graded against a real cached oracle). The two scalar_eval-only quirks (error propagation, explicit #DIV/0!) are documented runtime limitations, not stubs.

## Self-Check: PASSED
- `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-half-rounding.xlsx` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-empty-coercion.xlsx` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/quirks/quirk-text-coercion.xlsx` — FOUND
- Commit `e3cce105` (Task 1) — FOUND
- Commit `7fa7458f` (Task 2) — FOUND
