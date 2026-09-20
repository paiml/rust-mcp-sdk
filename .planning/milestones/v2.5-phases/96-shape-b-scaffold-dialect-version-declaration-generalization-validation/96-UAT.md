---
status: complete
phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
source: [96-01-SUMMARY.md, 96-02-SUMMARY.md, 96-03-SUMMARY.md, 96-04-SUMMARY.md, 96-05-SUMMARY.md]
started: 2026-06-15T14:49:39Z
updated: 2026-06-15T14:50:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Shape B scaffold — `cargo pmcp new --kind workbook-server` (WBCL-05)
expected: The scaffold emitter generates a runnable, purity-safe workbook server crate with the pre-compiled `tax-calc@1.1.0` bundle and source `.xlsx` embedded so they survive `cargo publish`.
result: pass
evidence: |
  `cargo run -p cargo-pmcp --example workbook_server_scaffold` exits 0 and emits an 11-file tree:
  Cargo.toml, bundle/tax-calc@1.1.0/{BUNDLE.lock,cell_map.json,executable.ir.json,layout.json,manifest.json,evidence/*},
  pmcp.toml, src/main.rs, workbook/tax-calc.xlsx — serves five workbook tools over streamable HTTP.
  `cargo test -p cargo-pmcp workbook_server` → 16 passed (template + TOOLKIT_VERSION drift guard).

### 2. Dialect-version gate is fail-closed in both lanes (WBDL-02)
expected: A workbook declaring `pmcp_dialect_version` is accepted when compatible (same major, minor ≤ supported), rejected with a typed error when incompatible (different major OR newer minor), and absent declaration compiles as baseline. The gate fires on BOTH the seed lane and the gated-update lane.
result: pass
evidence: |
  `cargo run -p pmcp-workbook-compiler --example dialect_version_demo` exits 0 and prints:
  (a) absent → baseline 1.0 accepted; (b) `1.0` accepted; (c) `2.0` and `1.5` → typed fail-closed Lint errors.
  `cargo test -p pmcp-workbook-compiler dialect_version` → 30 passed (incl. HI-01 gated-update-lane regression tests).

### 3. Generalization gate — second non-lighthouse loan workbook (WBEX-01)
expected: A synthetic loan/mortgage rate-tier workbook compiles end-to-end through the generic `compile_workbook` driver and serves its OWN schema — loan input/output keys present, tax-calc keys absent, the two key sets disjoint — behind the same five generic tool names, with zero per-workbook served Rust.
result: pass
evidence: |
  `cargo test -p pmcp-workbook-compiler reemit_loan` → 9 passed, including
  `loan_and_tax_served_key_sets_are_disjoint` and presence of in_loan_amount/in_term_months/in_credit_score.

### 4. Excel-quirk corpus across both D-08 layers (WBEX-02)
expected: An 8-quirk corpus (4 named + 4 curated) is encoded as fast scalar_eval unit assertions (runtime crate) AND mini penny-reconcile fixtures (compiler crate) graded through the real `within_tol` path against cached oracles, with a wrong-oracle negative test proving grading is real.
result: pass
evidence: |
  `cargo test -p pmcp-workbook-runtime scalar_eval` → 12 passed (8 quirk + 4 pre-existing).
  `cargo test -p pmcp-workbook-compiler quirks` → 5 passed (reconcile fixtures via within_tol).

### 5. Full compiler suite green (regression smoke)
expected: The whole governed-Excel compiler test suite passes from a clean run — no regressions introduced by the phase 96 work.
result: pass
evidence: |
  `cargo test -p pmcp-workbook-compiler` → 315 passed, 1 ignored, 0 failed.

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]
