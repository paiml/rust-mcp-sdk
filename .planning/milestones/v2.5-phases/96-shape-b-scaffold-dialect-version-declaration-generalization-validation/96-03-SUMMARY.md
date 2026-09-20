---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
plan: 03
subsystem: testing
tags: [rust_xlsxwriter, fixture-authoring, provenance, excel-quirk, 1900-leap, reconcile]

# Dependency graph
requires:
  - phase: 96-01
    provides: pub mod dialect_version (lib.rs module surface this plan appends alongside)
  - phase: 93
    provides: compile_workbook_with_fixture_override + provenance gate (classify/ProvenanceClass) + penny-reconcile
provides:
  - "Reusable #[cfg(test)] rust_xlsxwriter fixture author (fixture_author.rs) with genuine Excel identity, cached-<v> oracle, reproducible non-mutating generator, and a production-refusal guard"
  - "Direct provenance assertion API (classify_authored) asserting ExcelTrusted from authored bytes"
  - "SPIKE-1900-leap.md: disposition (A) DAG-expressible + WBEX-02 traceability; committed leap1900-probe.xlsx reconcile fixture"
affects: [96-04, 96-05, WBEX-01, WBEX-02]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Excel-identity fixture authoring via rust_xlsxwriter defaults (no DocProperties needed)"
    - "Env-gated #[ignore] generator (PMCP_REGEN_FIXTURES) for non-mutating reproducible committed fixtures"
    - "Direct classify() provenance assertion (not inferred from compile success)"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/SPIKE-1900-leap.md
    - crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.provenance-override.json
    - crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.gen.json
  modified:
    - crates/pmcp-workbook-compiler/src/lib.rs

key-decisions:
  - "rust_xlsxwriter 0.95 defaults already supply genuine Excel identity (Application=Microsoft Excel, AppVersion=12.0000, calcId=124519 non-sentinel) -> no DocProperties call needed for ExcelTrusted"
  - "1900-leap disposition (A): the quirk is DAG-expressible as f64 serial arithmetic IF(serial>59, serial+1, serial) with whitelisted ops only -- no DATE/DATEVALUE added"
  - "Committed fixtures written ONLY by the env-gated #[ignore] regenerate_fixtures generator; normal tests use TempDir"

patterns-established:
  - "WorkbookSpec/AuthoredCell/DefinedNameSpec author API consumed by Plans 04/05"
  - "Per-fixture *.provenance-override.json marker + *.gen.json generation metadata sidecar"

requirements-completed: [WBEX-01, WBEX-02]

# Metrics
duration: 35min
completed: 2026-06-15
---

# Phase 96 Plan 03: WBEX critical-path landmine retirement Summary

**A proven, reusable #[cfg(test)] rust_xlsxwriter fixture author (genuine Excel identity asserted directly via classify, cached-<v> reconcile oracle, env-gated non-mutating generator, production-refusal guard) plus a resolved 1900-leap-year disposition (A: DAG-expressible serial arithmetic, no date functions) — unblocking the WBEX-01/02 gates in Plans 96-04 and 96-05.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-06-15T08:27:35Z
- **Completed:** 2026-06-15T08:55Z (approx)
- **Tasks:** 2
- **Files modified:** 6 (1 modified, 5 created)

## Accomplishments
- Retired Pitfall 1 (the .xlsx authoring gap): one proven `#[cfg(test)]` author both WBEX gates ride, with a DIRECT `classify() == ExcelTrusted` assertion (not inferred from compile success) and an end-to-end compile through the trusted-fixture override.
- Closed RESEARCH Open Question 1 (1900-leap) with disposition **(A) DAG-expressible** — the strongest outcome — proven by a committed reconcile fixture that compiles + reconciles through the real penny path, with zero date-function scope creep.
- Established the reproducible NON-mutating generation workflow (env-gated `#[ignore]` generator + per-fixture metadata sidecars) so committed binaries are traceable and a normal `cargo test` never rewrites tracked fixtures.

## Task Commits

Each task was committed atomically:

1. **Task 1: reusable #[cfg(test)] rust_xlsxwriter fixture author** - `6392e10d` (feat)
2. **Task 2: 1900-leap-year spike + disposition doc + committed probe** - `f9b0fdc8` (feat)

**Plan metadata:** (this SUMMARY + STATE + ROADMAP) - see final docs commit.

## Files Created/Modified
- `crates/pmcp-workbook-compiler/src/fixture_author.rs` - the reusable author: `WorkbookSpec`/`AuthoredCell`/`DefinedNameSpec` API, `author_xlsx`, `classify_authored` (direct provenance assertion), `write_override_marker` + `write_gen_metadata` sidecars, the env-gated `regenerate_fixtures` generator, and 6 self-tests (1 ignored).
- `crates/pmcp-workbook-compiler/src/lib.rs` - appended `#[cfg(test)] mod fixture_author;` alongside `reemit_golden` (the existing `pub mod dialect_version;` from Plan 96-01 is preserved untouched).
- `crates/pmcp-workbook-compiler/SPIKE-1900-leap.md` - the resolved disposition (A) with a `## Disposition` and a `## WBEX-02 Traceability` section for Plan 96-05.
- `crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.xlsx` - the committed reconcile probe (genuine Excel identity, cached `<v>`=62 oracle).
- `crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.provenance-override.json` - the trusted-fixture marker.
- `crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.gen.json` - the generation metadata sidecar.

## Key Technical Findings

- **rust_xlsxwriter 0.95 gives Excel identity for free** (verified in the crate source: `app.rs` writes `<Application>Microsoft Excel</Application>` + `<AppVersion>12.0000</AppVersion>`; `workbook.rs` writes `calcId="124519" fullCalcOnLoad="1"`). `124519 != 122211` (the umya sentinel), so an authored workbook classifies `ProvenanceClass::ExcelTrusted` directly; the only residual signal is `fullCalcOnLoad=1` staleness, which the `#[cfg(test)]` override demotes. No `DocProperties` call is required for trust.
- **The 1900-leap quirk is serial arithmetic, not a date function.** `IF(A1>59, A1+1, A1)` over bare `f64` reproduces Excel's phantom-leap `+1` offset for serials past `1900-02-28` (serial 59 = 1900-02-28 no shift; serial 60 = phantom 1900-02-29). The committed probe compiles + reconciles through `reconcile::reconcile` against its cached `<v>` oracle, proving disposition (A).

## Threat Model Outcomes
- **T-96-07 (production bypass via test override):** mitigated — `production_compile_refuses_authored_fixture` asserts bare `compile_workbook` (Enforce) refuses the authored bytes; the override is `#[cfg(test)]`-only.
- **T-96-08 (umya-fabricated identity passing):** mitigated — `authored_xlsx_classifies_excel_trusted_directly` asserts `classify() == ExcelTrusted` directly on the authored bytes (a umya identity would classify `UmyaFabricated` and fail).
- **T-96-08b (committed fixtures silently mutated):** mitigated — committed `.xlsx` written ONLY by the env-gated `#[ignore]` generator; self-tests use `TempDir`.
- **T-96-09 (date functions smuggled into the whitelist):** held — disposition (A) needs no date code; the dialect crate is byte-unchanged (`git diff` clean on `pmcp-workbook-dialect`).
- **T-96-SC (package installs):** no new external packages (rust_xlsxwriter/tempfile already vetted dev-deps) — no legitimacy checkpoint required.

## Deviations from Plan

None — plan executed exactly as written. Both tasks landed within the documented scope; disposition (A) was the preferred outcome and was achieved without falling back to (B)/(C).

## Verification

- `cargo test -p pmcp-workbook-compiler fixture_author` → 6 passed, 1 ignored (the env-gated generator).
- `cargo test -p pmcp-workbook-compiler` → 297 passed, 1 ignored, 0 failed.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` → clean (zero warnings).
- `cargo fmt -p pmcp-workbook-compiler` → clean.
- `git diff` on `crates/pmcp-workbook-dialect/` and the existing `tax-calc.xlsx`/`tax-calc.provenance-override.json` → empty (no edits to the WHITELIST or existing fixtures).
- Task 2 plan verify (`test -f SPIKE-1900-leap.md && grep Disposition && grep "WBEX-02 Traceability" && cargo test`) → passes.

## Deferred / Notes for Plan 96-05
- Reuse `fixture_author::leap1900_probe_spec` (extend, do not hand-author) and add the D-08 `scalar_eval` unit-test layer for the same `IF(serial>59, serial+1, serial)` offset alongside the reconcile fixture.
- The reusable `CellPaint::{Plain,Constant}` and `AuthoredCell::Text` variants carry a scoped `#[allow(dead_code)]` with a `// Why:` rationale (they are the author surface Plan 04's loan workbook constructs — exercised next plan, not dead).

## Deferred Full-Gate Runs
- The repo-wide `make quality-gate` (pedantic+nursery clippy across `--features full`, workspace fmt, audit, full workspace test) was NOT run for these per-task commits — only the targeted `pmcp-workbook-compiler` fmt/clippy/test were run (all clean). This plan touches only the compiler crate under `#[cfg(test)]` + a doc + fixtures. Run `make quality-gate` before opening any PR for Phase 96, per CLAUDE.md.

## Self-Check: PASSED
- `crates/pmcp-workbook-compiler/src/fixture_author.rs` — FOUND
- `crates/pmcp-workbook-compiler/SPIKE-1900-leap.md` — FOUND
- `crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.xlsx` — FOUND
- `.planning/phases/96-.../96-03-SUMMARY.md` — FOUND
- Commit `6392e10d` (Task 1) — FOUND
- Commit `f9b0fdc8` (Task 2) — FOUND
