---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
plan: 04
subsystem: workbook-compiler
tags: [wbex-01, generalization-gate, served-schema, rate-tier, vlookup, index-match, fixture-authoring]

# Dependency graph
requires:
  - phase: 96-03
    provides: "#[cfg(test)] rust_xlsxwriter fixture author (WorkbookSpec/AuthoredCell/DefinedNameSpec, env-gated regenerate_fixtures generator, override+gen sidecars)"
  - phase: 93
    provides: "compile_workbook_with_fixture_override + the generic compile_workbook driver + reconcile penny path + reemit_golden proof structure"
  - phase: 92
    provides: "pmcp-server-toolkit::workbook served-schema fns (input_schema_for_manifest/output_schema_for_manifest), GetManifestHandler, the five handler NAME consts, load_bundle/LocalDirSource"
provides:
  - "WBEX-01 generalization gate PROVEN: a second non-lighthouse loan workbook compiles + serves its OWN get_manifest/tools/list schema behind the SAME five generic tool names, zero per-workbook served Rust"
  - "committed synthetic loan-calc.xlsx rate-tier fixture (whitelist-legal: VLOOKUP/INDEX-MATCH/IFERROR/nested-IF/ROUND/CEILING; multiple out_* outputs; in_* named inputs) + provenance/gen sidecars"
  - "name_named_inputs() — the in_* INPUT named-range convention (the input analogue of out_*) giving the served input schema stable semantic keys"
affects: [96-05, WBEX-01]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "in_* input named-range convention (parallel to the proven out_* output convention) → served semantic input keys"
    - "Served-schema generalization assertion via the GENERIC toolkit fns over a compiled bundle (no hand-built schema)"
    - "Intrinsic-structure proof (no golden): seven-member contract + BUNDLE.lock recompute + served-schema disjointness"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/reemit_loan.rs
    - crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.provenance-override.json
    - crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.gen.json
  modified:
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/src/lib.rs

key-decisions:
  - "Compiled-workbook INPUTS have no semantic key path in the synth pipeline (synth sets name=None, unit=None; json_key falls back to the cell's numeric value). Added name_named_inputs() — the in_* analogue of promote_named_outputs — so the served input schema carries loan_amount, not \"240000\". Naming only, never re-roling (Rule 2)."
  - "The 'custom unit in the served schema' acceptance item is NOT achievable through the real compile path: synth sets every role.unit=None and the cell_map unit reads role.unit. Asserted instead that the generic { value, unit } projection RUNS uniformly per output (unit slot present, value object present) — the unit-projection mechanism is exercised, the value is null for a compiled (non-hand-authored) manifest."
  - "No-golden proof shape: unlike reemit_golden, the loan asserts INTRINSIC structure (seven members, lock recompute) + ITS OWN served schema + disjointness from tax-calc, never equality to a second file."

patterns-established:
  - "in_<name> input named range → stable served input json_key (mirrors out_<name>)"

requirements-completed: [WBEX-01]

# Metrics
duration: ~40min
completed: 2026-06-15
---

# Phase 96 Plan 04: WBEX-01 Generalization Gate Summary

**A second, non-lighthouse synthetic loan/mortgage rate-tier workbook compiles end-to-end through the GENERIC `compile_workbook` driver and serves ITS OWN `get_manifest`/`tools/list` schema (loan-specific input/output keys present, tax-calc keys absent, the two key sets disjoint) behind the SAME five generic tool names — proving the manifest-driven §5 serve path generalizes beyond the lighthouse with zero per-workbook served Rust.**

## Performance

- **Duration:** ~40 min
- **Started:** 2026-06-15T08:41:09Z
- **Tasks:** 2
- **Files:** 6 (4 created, 2 modified)

## Accomplishments

- **Authored the WBEX-01 generalization fixture (Task 1):** a fully synthetic loan/mortgage rate-tier calculator whose divergence from `tax-calc` is sourced ENTIRELY from whitelist-legal lookup families — a constant rate-tier TABLE (`D2:E4`) resolved by BOTH `VLOOKUP(...)` and `INDEX(.., MATCH(.., 0))` (cross-checked), `IFERROR(..)` guards, a nested `IF(.., IF(..))` credit-band tiering, and `ROUND`/`CEILING` to currency. NO `PMT`, NO `POWER`, NO exponentiation (D-02; arbitrary-term amortization deferred). Five `out_*` outputs (no privileged single headline) and three `in_*` named inputs, a `pmcp_dialect_version=1.0` cell (WBDL-02 present-path), the cached `<v>` reconcile oracle, and genuine Excel identity via the Plan-03 author.
- **Proved the gate (Task 2):** `reemit_loan.rs` (in-crate `#[cfg(test)]`) compiles the loan via the trusted-fixture override, loads it through the GENERIC toolkit loader, and asserts the SERVED schema directly off the generic fns — the five tool names unchanged, loan keys present, tax keys absent, the key sets disjoint, the get_manifest projection reflecting the loan's own cells, plus the production-refusal counter-test.
- **Closed a real pipeline gap (deviation, see below):** the compile path had no way to give an input a semantic served key. Added the `in_*` named-range convention as the exact mirror of the proven `out_*` one.

## Task Commits

1. **Task 1: author synthetic loan rate-tier fixture** — `6b622e95` (feat)
2. **Task 2: reemit_loan served-schema generalization proof + `name_named_inputs`** — `a7529369` (feat)

## Files Created/Modified

- `crates/pmcp-workbook-compiler/src/fixture_author.rs` — added `loan_calc_spec()` (the documented rate-tier DAG with worked cached-oracle values) and registered it in the env-gated `regenerate_fixtures` generator.
- `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.{xlsx,provenance-override.json,gen.json}` — the committed fixture (genuine Excel identity), its trusted-fixture marker, and the generation-metadata sidecar.
- `crates/pmcp-workbook-compiler/src/reemit_loan.rs` — the 9-assertion compile-and-serve proof (`#[cfg(test)]`, declared in `lib.rs`).
- `crates/pmcp-workbook-compiler/src/lib.rs` — declared `#[cfg(test)] mod reemit_loan;` (appended alongside `fixture_author`/`reemit_golden`; existing decls untouched) and added `name_named_inputs()` wired into BOTH `compile_workbook_inner` and `prepare_candidate` after `promote_named_outputs`.

## The Served-Schema Generalization Assertions (the gate)

`reemit_loan.rs` — 9 tests, all green:

| Test | Asserts |
|------|---------|
| `five_generic_tool_names_unchanged` | handler `NAME` consts == `{calculate, explain, get_manifest, diff_version, render_workbook}` — identical to tax-calc |
| `served_input_schema_reflects_loan_inputs` | `input_schema_for_manifest` CONTAINS `in_loan_amount`/`in_term_months`/`in_credit_score`; NO tax input key |
| `served_output_schema_reflects_loan_outputs` | `output_schema_for_manifest` carries all five `out_*` keys, ≥3 outputs, each with the `{ value, unit }` projection |
| `tax_specific_fields_absent_from_loan_schema` | none of tax-calc's input/output keys appear |
| `loan_and_tax_served_key_sets_are_disjoint` | **THE proof (T-96-11)** — loan vs tax served input AND output key sets are disjoint, read off the generic-driver output |
| `served_get_manifest_reflects_loan_cells` | `GetManifestHandler::new(Arc<bundle>)` accepts the loan; the manifest projection has the loan's own inputs/outputs, ≥3 outputs |
| `loan_bundle_carries_seven_member_contract` | the generic driver emits the full seven-member bundle for a second workbook |
| `loan_bundle_lock_recomputes` | `BUNDLE.lock` combined hash recomputes via `build_bundle_lock`; `bundle_id == loan-calc` |
| `production_compile_refuses_loan_fixture` | **T-96-10** — bare `compile_workbook` (Enforce) refuses the same bytes |

## Deviations from Plan

### Auto-added missing critical functionality

**1. [Rule 2 - Missing functionality] `name_named_inputs()` — the `in_*` input named-range convention**
- **Found during:** Task 2 (first run: served input keys were `{"240000","360","700"}` — the cell VALUES).
- **Issue:** synthesis classifies a cell's ROLE from colour alone and never assigns a semantic `name` (synth.rs sets `name: None`); the cell-map `json_key_for_role` precedence is `name → meaning → cell key`, so a bare numeric INPUT leaf falls through to its own value string — a meaningless served input key. The pipeline had an OUTPUT-naming convention (`out_*` via `promote_named_outputs`) but NO input analogue, so a compiled workbook could not serve semantic input keys at all.
- **Fix:** added `name_named_inputs(&mut Manifest, &WorkbookMap)` — the exact mirror of `promote_named_outputs` for `in_*` single-cell named ranges, setting `name` on cells ALREADY classified `Role::Input` (naming only, never re-roling). Wired into both driver call sites (`compile_workbook_inner`, `prepare_candidate`) right after `promote_named_outputs`. The loan fixture declares `in_loan_amount`/`in_term_months`/`in_credit_score`.
- **Files modified:** `crates/pmcp-workbook-compiler/src/lib.rs`, `crates/pmcp-workbook-compiler/src/fixture_author.rs`.
- **Commit:** `a7529369`.
- **Scope note:** this is the natural parallel to the established `out_*` convention, not a structural change; all 306 compiler tests (incl. tax-calc's `reemit_golden`) still pass (tax-calc declares no `in_*` names, so it is byte-unaffected).

### Acceptance-criterion adjustment (custom unit)

**2. [Rule 3 - Pipeline constraint] The "custom unit in the served schema" item is not achievable via the compile path.**
- **Issue:** Task 1's acceptance criteria and the plan ask for at least one output carrying a custom unit (`percent`/`USD`) visible in the served schema. But the synth path sets `role.unit = None` for EVERY compiled cell, and `build_cell_map` reads `role.unit` — so a compiled bundle's served `unit` is always `null`. Units appear on the hand-authored `tax-calc` GOLDEN manifest, never on a freshly-compiled one. Achieving authored units would require either a unit-declaration convention in the workbook + synth support, or hand-editing the emitted manifest — both out of scope for this gate (the former is a new pipeline feature; the latter breaks the "generic-driver output" invariant the gate depends on).
- **Resolution:** the proof asserts the unit-PROJECTION mechanism is exercised — `served_output_schema_reflects_loan_outputs` checks every loan output carries the generic `{ value, unit }` nested shape (the `value` object present, the `unit` slot present), proving the served schema's unit projection RUNS uniformly for a non-golden workbook (the value is `null` because the compiled manifest carries no authored unit). The fixture's `gen.json` still documents the intended `percent`/`USD` semantics. The core generalization gate (loan keys present, tax keys absent, disjoint, 5 names unchanged, multiple outputs, no headline) is fully satisfied.

## Threat Model Outcomes

- **T-96-10 (production bypass via test override):** mitigated — `production_compile_refuses_loan_fixture` asserts bare `compile_workbook` (Enforce) refuses the authored bytes; the override is `#[cfg(test)]`-only.
- **T-96-11 (per-workbook Rust faking generalization):** mitigated — every served-schema assertion reads the output of the GENERIC toolkit fns (`input_schema_for_manifest`/`output_schema_for_manifest`/`GetManifestHandler`) over the loaded bundle; no loan-specific Rust builds any schema. `name_named_inputs` is a generic naming convention applied to ANY `in_*` workbook, not loan-specific code.
- **T-96-12 (customer/TowelRads material leaking):** mitigated — the loan fixture is fully synthetic toy data (a 3-tier rate table 0.08/0.06/0.045, `loan_amount=240000`, `credit_score=700`); ZERO customer/TowelRads content.
- **T-96-SC (package installs):** no new external packages — no install task, no legitimacy checkpoint.

## Verification

- `cargo test -p pmcp-workbook-compiler reemit_loan` → 9 passed (the served-schema generalization gate + production-refusal).
- `cargo test -p pmcp-workbook-compiler fixture_author` → 6 passed, 1 ignored (Task 1 verify; the Plan-03 author still green).
- `cargo test -p pmcp-workbook-compiler` → 306 passed, 1 ignored, 0 failed (was 297 + 9 new).
- `cargo clippy -p pmcp-workbook-compiler --all-targets` → clean (zero warnings, cog ≤25, zero SATD).
- `cargo fmt -p pmcp-workbook-compiler -- --check` → clean.
- `git diff` on `crates/pmcp-workbook-dialect/`, `tax-calc.xlsx`, `tax-calc.provenance-override.json`, and `leap1900-probe.xlsx` → empty (no edits to the WHITELIST or existing fixtures; the leap1900 fixture was restored after the regenerate run touched its non-deterministic metadata bytes).

## Deferred Full-Gate Runs

- The repo-wide `make quality-gate` (pedantic+nursery clippy across `--features full`, workspace fmt, audit, full workspace test) was NOT run for these per-task commits — only the targeted `pmcp-workbook-compiler` fmt/clippy/test were run (all clean). This plan touches only the compiler crate (one `#[cfg(test)]` proof module + the small generic `name_named_inputs` driver helper + fixtures). Run `make quality-gate` before opening any PR for Phase 96, per CLAUDE.md.

## Notes for Plan 96-05

- `name_named_inputs` is now available for any quirk fixture that wants semantic served input keys (most quirk fixtures assert via `scalar_eval`/reconcile and won't need it, but it exists).
- The `in_*` convention is undocumented in `docs/workbook-dialect-spec.md` (only the served-schema contract is exercised by tests). If a later plan documents the workbook authoring conventions, add `in_*` alongside `out_*`.

## Self-Check: PASSED
- `crates/pmcp-workbook-compiler/src/reemit_loan.rs` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.provenance-override.json` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.gen.json` — FOUND
- `.planning/phases/96-.../96-04-SUMMARY.md` — FOUND
- Commit `6b622e95` (Task 1) — FOUND
- Commit `a7529369` (Task 2) — FOUND
