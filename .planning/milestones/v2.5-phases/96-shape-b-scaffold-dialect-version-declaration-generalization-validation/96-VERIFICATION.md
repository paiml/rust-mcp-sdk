---
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
verified: 2026-06-15T10:00:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 96: Shape B Scaffold + Dialect-Version + Generalization Validation — Verification Report

**Phase Goal:** `cargo pmcp new --kind workbook-server` scaffolds a thin binary over EmbeddedSource + the served-tool toolkit module (Shape B); workbooks declare the dialect version they target (forward-compatible evolution); and the generalization gates — a second non-lighthouse workbook compiling and serving end-to-end, plus an Excel-quirk fixture corpus — prove the manifest is truly synth-driven with no per-workbook Rust and no privileged single output.

**Verified:** 2026-06-15T10:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | WBCL-05 — `cargo pmcp new --kind workbook-server` scaffolds a runnable crate (purity-safe, EmbeddedSource) | VERIFIED | `cargo-pmcp/src/commands/new.rs` line 75-79 dispatch arm; `workbook_server.rs` emitter with `default-features = false, features = ["workbook-embedded", "http"]`; scaffold example runs and emits 11-file tree including `bundle/tax-calc@1.1.0/*` |
| 2 | WBDL-02 — dialect version gate is fail-closed in BOTH the seed lane AND the gated-update lane | VERIFIED | `validate_dialect_version_step` at `lib.rs:306` (seed) and `lib.rs:755` (gated-update); four regression tests in `dialect_version.rs:662-704` prove incompatible major/minor → `CompileError::Lint` on the gated-update path; `cargo test -p pmcp-workbook-compiler dialect_version` → 30 passed |
| 3 | WBEX-01 — a second non-lighthouse loan workbook compiles and serves its OWN schema with loan keys present, tax keys absent, key sets disjoint, five generic tool names unchanged | VERIFIED | `reemit_loan.rs` 9-assertion module; `cargo test -p pmcp-workbook-compiler reemit_loan` → 9 passed; `loan_and_tax_served_key_sets_are_disjoint` is the explicit disjointness proof; `in_loan_amount`/`in_term_months`/`in_credit_score` confirmed present; no per-workbook served Rust |
| 4 | WBEX-02 — 8-quirk corpus covers the four named + four curated quirks across both layers (scalar_eval unit tests + penny-reconcile fixtures via within_tol) | VERIFIED | `scalar_eval.rs` 8 quirk tests (12 total passing); `quirks_reconcile.rs` 5 reconcile fixtures + leap probe reuse; `cargo test -p pmcp-workbook-runtime scalar_eval` → 12 passed; `cargo test -p pmcp-workbook-compiler quirks` → 5 passed; wrong-oracle negative test proves grading is real |

**Score:** 4/4 truths verified

---

### Locked Decision Verification (CONTEXT.md D-01..D-09)

| Decision | Claim | Verification |
|----------|-------|--------------|
| D-01 (domain = loan/mortgage) | Second workbook is a loan/mortgage rate-tier calculator | VERIFIED — `fixture_author.rs:668` documents rate-tier DAG; `loan-calc.xlsx` committed |
| D-02 (whitelist-legal, no PMT/POWER) | Loan workbook uses VLOOKUP/INDEX-MATCH/IFERROR/nested-IF/ROUND/CEILING only | VERIFIED — `fixture_author.rs:398-399` explicitly states "NO PMT/POWER/exponentiation"; grep for `"PMT"\|"POWER"` returns only negative-assertion comments |
| D-03 (reserved named range `pmcp_dialect_version`) | Version declared in a single-cell defined name | VERIFIED — `DIALECT_VERSION_NAME = "pmcp_dialect_version"` at `dialect_version.rs:36`; loan fixture has the cell at `'Loan'!$A$10` |
| D-04 (fail-closed: different major / newer minor → typed error) | Both lanes gate on this | VERIFIED — `validate_dialect_version_step` called in both `compile_workbook_inner` (line 306) and `prepare_candidate_inner` (line 755); HI-01 fix confirmed |
| D-05 (absent → baseline, no error) | Missing cell compiles normally | VERIFIED — `gated_update_step_absent_declaration_recompiles_as_baseline` test passes; existing `tax-calc` fixture (no version cell) still compiles |
| D-06 (full round-trip payload over EmbeddedSource) | Scaffold ships Cargo.toml + main.rs + pmcp.toml + .xlsx + pre-compiled bundle | VERIFIED — scaffold example output shows 11-file tree; packaging smoke confirms assets ship with `cargo publish` |
| D-07 (sample workbook = tax-calc golden) | Scaffold reuses tax-calc@1.1.0; no new .xlsx authored in Plan 02 | VERIFIED — `workbook_bundle/tax-calc@1.1.0/` embedded under cargo-pmcp package root |
| D-08 (both layers: scalar_eval units + reconcile fixtures) | Every quirk has a live scalar_eval assertion; numerically-expressible quirks also have a reconcile fixture | VERIFIED — 8 scalar_eval tests + 6 reconcile fixtures (5 in `quirk_cases()` + leap probe); traceability map in `quirks_reconcile.rs` module doc |
| D-09 (~7-9 quirks, no customer/TowelRads material) | 8 quirks total; all synthetic data | VERIFIED — 8 quirks: 4 named (1900-leap, empty-cell coercion, error propagation, half-rounding) + 4 curated; no `customer\|TowelRads` string found in fixture_author.rs or quirks_reconcile.rs |

**version.rs byte-unchanged:** `git diff 14047806 -- crates/pmcp-workbook-compiler/src/version.rs` returns empty. Only one commit in history touches version.rs (94-00); phase 96 added zero changes.

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-compiler/src/dialect_version.rs` | pmcp_dialect_version reader + parse_dialect_version (public) + resolve/validate | VERIFIED | 26.6K file; `pub fn parse_dialect_version`, `pub fn resolve_dialect_version`, `pub fn validate_dialect_version_step` all present |
| `crates/pmcp-workbook-compiler/examples/dialect_version_demo.rs` | ALWAYS example: 3 cases, exits 0 | VERIFIED | `cargo run --example dialect_version_demo` exits 0, prints absent/compatible/incompatible outcomes |
| `crates/pmcp-workbook-dialect/src/lib.rs` | SUPPORTED_DIALECT_VERSION + BASELINE_DIALECT_VERSION consts + drift guard | VERIFIED | Both consts at lines 45 and 52; `dialect_version_spec` drift-guard test module present |
| `docs/workbook-dialect-spec.md` | §7 version declaration policy | VERIFIED | Section 7 "Dialect version declaration & compatibility policy (WBDL-02)" at line 160 |
| `cargo-pmcp/src/templates/workbook_server.rs` | Shape B emitter with embedded assets | VERIFIED | 22.8K file; `EmbeddedSource`, `default-features = false`, `workbook-embedded`+`http`; TOOLKIT_VERSION drift-guard test |
| `cargo-pmcp/src/commands/new.rs` | workbook-server dispatch arm | VERIFIED | Lines 75-79: `Some("workbook-server") => execute_workbook_server`; supported list updated |
| `cargo-pmcp/src/templates/workbook_bundle/` | Embedded tax-calc@1.1.0 + tax-calc.xlsx | VERIFIED | Directory exists with BUNDLE.lock, cell_map.json, executable.ir.json, layout.json, manifest.json, evidence/, and tax-calc.xlsx |
| `crates/pmcp-workbook-compiler/src/reemit_loan.rs` | 9-assertion WBEX-01 generalization proof | VERIFIED | 16.3K file; all 9 assertion functions confirmed present including disjointness proof |
| `crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx` | Synthetic loan fixture | VERIFIED | 5.8K file; provenance-override.json + gen.json sidecars present |
| `crates/pmcp-workbook-compiler/src/quirks_reconcile.rs` | WBEX-02 reconcile harness with traceability map | VERIFIED | 17.2K file; `quirk_cases()` with 5 fixtures; wrong-oracle negative test; traceability table in module doc |
| `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dialect_version_parse.rs` | Fuzz target calling public parser | VERIFIED | Calls `pmcp_workbook_compiler::dialect_version::parse_dialect_version`; registered in fuzz/Cargo.toml as `dialect_version_parse` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `cargo-pmcp/src/commands/new.rs` | `cargo-pmcp/src/templates/workbook_server.rs` | `templates::workbook_server::generate` | WIRED | Line 292: `templates::workbook_server::generate(workspace_dir, name)?` |
| `lib.rs` (compile_workbook_inner) | `dialect_version.rs` | `dialect_version::validate_dialect_version_step` | WIRED | Line 306 (seed lane) |
| `lib.rs` (prepare_candidate_inner) | `dialect_version.rs` | `dialect_version::validate_dialect_version_step` | WIRED | Line 755 (gated-update lane) — HI-01 fix confirmed |
| `dialect_version.rs` | `pmcp-workbook-dialect` | `SUPPORTED_DIALECT_VERSION` | WIRED | Import of `SUPPORTED_DIALECT_VERSION` from dialect crate |
| `reemit_loan.rs` | toolkit served-schema fns | `input_schema_for_manifest` / `output_schema_for_manifest` / `GetManifestHandler` | WIRED | Generic fns over compiled bundle, not loan-specific Rust |
| `quirks_reconcile.rs` | `reconcile::within_tol` | `within_tol(&computed, &oracle)` | WIRED | Line 76 imports `within_tol, TOL`; every reconcile assertion uses it |

---

### Code Review Finding Resolution (96-REVIEW.md)

| Finding | Severity | Resolution Status | Evidence |
|---------|----------|-------------------|----------|
| HI-01: dialect-version check absent from gated-update lane | HIGH | RESOLVED | `validate_dialect_version_step` at `lib.rs:755`; 4 regression tests in `dialect_version.rs:662-704`; commit `2b6c531a` |
| ME-01: scaffold pins `pmcp-server-toolkit = "0.1.0"` with no drift-guard | MEDIUM | RESOLVED | `TOOLKIT_VERSION` const + `emitted_toolkit_version_matches_workspace_pin` test at `workbook_server.rs:490`; commit `4b75415b` |
| LO-01: emitted Cargo.toml declares `clap` unused | LOW | RESOLVED | No `^clap` line in emitted Cargo.toml section; grep for `clap` in workbook_server.rs returns no emitted dependency |
| LO-02: `include-dir`/`include_dir` package-rename inconsistency | LOW | RESOLVED | Emitted dep is `include_dir = "0.7.4"` (no hyphen rename); commit `4b75415b` |
| LO-03: corpus-count assertion tautological | LOW | RESOLVED | `assert_eq!(cases.len(), 5, ...)` exact equality; commit `c6a82aa1` |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `reemit_loan.rs` served-schema assertions | `input_schema_for_manifest` / `output_schema_for_manifest` results | GENERIC toolkit fns over real compiled `loan-calc.xlsx` bundle | Yes — bundle compiled from real xlsx via `compile_workbook_with_fixture_override`; loan keys confirmed present, tax keys confirmed absent | FLOWING |
| `quirks_reconcile.rs` reconcile harness | `(computed, oracle)` pair from `recompute_at_reconcile_key` | `run_executor` over compiled quirk bundle → `CellValue`; oracle from committed `.xlsx` cached `<v>` | Yes — `a_wrong_oracle_does_not_reconcile_proving_the_value_is_graded` proves the value is graded, not compile-success | FLOWING |
| `scaffold example` | emitted file tree | `generate()` from `workbook_server.rs` emitter over `EMBEDDED_BUNDLE` / `EMBEDDED_XLSX` | Yes — 11 files including real bundle files visible in example output | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| dialect_version_demo ALWAYS example runs and prints 3 outcomes | `cargo run -p pmcp-workbook-compiler --example dialect_version_demo` | Prints absent→baseline/compatible→accepted/incompatible→typed error; exits 0 | PASS |
| WBEX-01 9-assertion served-schema gate | `cargo test -p pmcp-workbook-compiler reemit_loan` | 9 passed | PASS |
| WBDL-02 both-lane gating (30 dialect_version tests) | `cargo test -p pmcp-workbook-compiler dialect_version` | 30 passed | PASS |
| WBEX-02 reconcile corpus (5 fixtures) | `cargo test -p pmcp-workbook-compiler quirks` | 5 passed | PASS |
| WBEX-02 scalar_eval layer (8 quirk unit tests + 4 pre-existing) | `cargo test -p pmcp-workbook-runtime scalar_eval` | 12 passed | PASS |
| Full compiler suite | `cargo test -p pmcp-workbook-compiler` | 315 passed, 1 ignored | PASS |
| Shape B scaffold example | `cargo run -p cargo-pmcp --example workbook_server_scaffold` | Emits 11-file tree; OK | PASS |
| workbook_server template tests | `cargo test -p cargo-pmcp workbook_server` | 16 passed | PASS |
| Combined runtime + compiler | `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler` | 472 passed, 1 ignored | PASS |
| cargo-pmcp pre-existing failures (unrelated) | `cargo test -p cargo-pmcp --lib` | 435 passed; 1 failed (`normalize_round_trip_idempotent` proptest — file last touched `daa091c5`, predates phase 96 by many releases, zero phase 96 commits on `auth_cmd/cache.rs`) | NOT a phase 96 gap — pre-existing |

---

### Probe Execution

| Probe | Command | Result | Status |
|-------|---------|--------|--------|
| N/A (no probe scripts defined for this phase) | — | — | SKIPPED |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WBCL-05 | 96-02-PLAN.md | `cargo pmcp new --kind workbook-server` scaffold | SATISFIED | Dispatch arm, template emitter, embedded assets, 16 tests passing, runnable example |
| WBDL-02 | 96-01-PLAN.md | Workbook dialect version declaration | SATISFIED | Both compile lanes gated; 30 dialect_version tests; drift guard; fuzz target registered |
| WBEX-01 | 96-04-PLAN.md | Generalization gate — second workbook | SATISFIED | 9 served-schema assertions pass; loan/tax key sets disjoint; zero per-workbook Rust |
| WBEX-02 | 96-05-PLAN.md | Excel-quirk corpus | SATISFIED | 8 quirks; 12 scalar_eval tests; 6 reconcile fixtures; wrong-oracle negative test |

---

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| None in phase 96 files | — | — | — |

Notes:
- `quirks_reconcile.rs`: the `assert!((5..=9).contains(...))` tautological assertion was found and fixed (LO-03) to `assert_eq!(cases.len(), 5, ...)`.
- `workbook_server.rs`: the unused `clap` dep (LO-01) and `include-dir` rename (LO-02) were found and fixed.
- Pre-existing `cargo test -p cargo-pmcp --lib` failures in `auth_cmd/cache.rs`, `commands/doctor.rs`, `templates/mcp_app.rs` are NOT phase 96 regressions — those files were last modified in commits predating phase 96 by multiple releases. Phase 96 made zero commits to those files.

---

### Human Verification Required

None. All verifiable behaviors are observable programmatically. The scaffold purity gate (`make purity-check`) was recorded as passing in 96-02-SUMMARY; the scaffolded crate build smoke (`PMCP_RUN_SCAFFOLD_BUILD=1`) was recorded as passing in 96-02-SUMMARY (ignored in default `cargo test`, run explicitly by the executor). No human UAT items remain.

---

## Gaps Summary

None. All four ROADMAP success criteria are verified:

1. **WBCL-05** — Shape B scaffold exists, emits a purity-safe runnable crate with embedded assets, is wired to the `--kind workbook-server` dispatch arm, and the example runs cleanly.
2. **WBDL-02** — The dialect-version gate is fail-closed in BOTH lanes. The HI-01 finding from 96-REVIEW.md was fixed (commit `2b6c531a`); the gated-update lane now calls `validate_dialect_version_step` and four regression tests prove the gate cannot be bypassed on re-compile. The version contract is bound to the spec doc by a drift guard.
3. **WBEX-01** — The second non-lighthouse loan workbook compiles and serves its own schema through the generic toolkit fns. The 9-assertion `reemit_loan.rs` module proves loan keys present, tax keys absent, key sets disjoint, and five generic tool names unchanged — with zero per-workbook served Rust.
4. **WBEX-02** — An 8-quirk corpus covers the four roadmap-named quirks plus four curated additions, encoded at both D-08 layers (scalar_eval unit tests + penny-reconcile fixtures via `within_tol`). The wrong-oracle negative test proves the reconcile harness grades real values, not compile-success.

All locked decisions (D-01..D-09) are honored. `version.rs` is byte-unchanged. No customer/TowelRads material in fixtures. The pre-existing `cargo-pmcp --lib` test failure is unrelated to phase 96.

---

_Verified: 2026-06-15_
_Verifier: Claude (gsd-verifier)_
