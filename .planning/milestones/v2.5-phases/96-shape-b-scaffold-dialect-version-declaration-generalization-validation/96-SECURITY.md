---
phase: 96
slug: shape-b-scaffold-dialect-version-declaration-generalization-validation
status: verified
threats_open: 0
asvs_level: 1
created: 2026-06-15
---

# Phase 96 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Verdict: **SECURED** — 18/18 threats CLOSED, 0 open, 0 unregistered flags.
> Mode: VERIFY MITIGATIONS EXIST (register authored at plan time across 5 PLAN files).

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| workbook `.xlsx` → compiler ingest | The `pmcp_dialect_version` cell value is untrusted input parsed during ingest. | version string (untrusted) |
| CLI arg (crate name) → filesystem | `cargo pmcp new --kind workbook-server <name>` writes files under a name-derived dir. | crate name (untrusted path component) |
| scaffolded crate → served binary tree | Emitted Cargo.toml feature set determines whether reader/code-mode leak into the served tree. | feature flags / dependency cone |
| cargo-pmcp package → published crate (crates.io) | Embedded assets must live inside the package root to survive `cargo publish`. | embedded bundle bytes |
| authored fixture `.xlsx` → compiler freshness/provenance gate | Test-authored fixtures ride the `#[cfg(test)]` trusted-fixture override; production must still refuse. | Excel provenance identity |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation (evidence) | Status |
|-----------|----------|-----------|-------------|-----------------------|--------|
| T-96-01 | Tampering | `pmcp_dialect_version` cell parse | mitigate | `dialect_version.rs:114-132` base-10 `parse_component` rejects empty/non-digit/embedded-ws and `u64` overflow as typed `CompileError::Lint` (never panic); tests `rejects_malformed_strings_without_panic`/`rejects_u64_overflow_component`. Registered fuzz target `fuzz/fuzz_targets/dialect_version_parse.rs` over `parse_dialect_version`, declared `[[bin]] dialect_version_parse` in `fuzz/Cargo.toml`. | closed |
| T-96-02 | Spoofing | dialect version source | accept-by-design | `declared_dialect_version` (`dialect_version.rs:235-255`) reads SOLELY the `pmcp_dialect_version` workbook cell; no CLI/pmcp.toml override path exists. Spec `docs/workbook-dialect-spec.md:160-174` §7.1. | closed |
| T-96-03 | Elevation | newer/incompatible dialect served as compatible | mitigate | `validate_declared` (`dialect_version.rs:191-205`) fail-closed via `is_compatible_with` (l.70-72): same major AND minor≤supported, else typed `Lint`. `SUPPORTED_DIALECT_VERSION`/`BASELINE_DIALECT_VERSION` consts (`pmcp-workbook-dialect/src/lib.rs:45,52`) bound by drift-guard `dialect_version_spec::doc_versions_match_consts` (l.418-466). Shared `validate_dialect_version_step` (l.180-182) closes HI-01 gated-update-lane drift; parity tests `gated_update_*`. | closed |
| T-96-04 | Tampering | crate-name path traversal (`new --kind workbook-server ../evil`) | mitigate | `execute_workbook_server` (`new.rs:281-292`) calls `validate_crate_name(name)?` at l.288 BEFORE `fs::create_dir_all`/generate (l.290-292). Guard (l.119-143) rejects `/`,`\`,`..`, leading digit, illegal chars (reused). Integration test `scaffold_rejects_path_traversal_name` (`workbook_scaffold.rs:120-134`). | closed |
| T-96-05 | Elevation | code-mode/umya/SWC leaking into served tree via toolkit defaults | mitigate | `generate_cargo_toml` (`workbook_server.rs:75-102`) emits `default-features = false, features = ["workbook-embedded","http"]`. Test `emitted_cargo_toml_is_purity_safe` (l.322-353) asserts feature set + NO `code-mode`; integration `workbook_scaffold.rs:86-105` repeats over real binary output. | closed |
| T-96-06 | Tampering | embedded bundle drifting from committed golden | mitigate | `embedded_bundle_matches_committed_golden` (`workbook_server.rs:382-431`) byte-equality of every file vs in-package `EMBEDDED_BUNDLE`; `embedded_xlsx_matches_committed_source` (l.433-444). | closed |
| T-96-06b | Tampering | scaffold assets missing after `cargo publish` | mitigate | Assets committed under `cargo-pmcp/src/templates/workbook_bundle/`, embedded via `include_dir!`/`include_bytes!` (`workbook_server.rs:44-48`). Packaging smoke `embedded_assets_appear_in_cargo_package_list` (`workbook_scaffold.rs:201-230`). | closed |
| T-96-07 | Elevation | production bypass of provenance gate via test override | mitigate | `compile_workbook_with_fixture_override` is `#[cfg(test)]` (`lib.rs:394-395`). Production-refusal self-test `production_compile_refuses_authored_fixture` (`fixture_author.rs:947-961`): bare `compile_workbook` (Enforce) refuses authored bytes. | closed |
| T-96-08 | Spoofing | fabricated (umya) Excel identity passing freshness gate | mitigate | `author_xlsx` uses `rust_xlsxwriter` genuine identity (`fixture_author.rs:171-211`). `authored_xlsx_classifies_excel_trusted_directly` (l.904-918) asserts `classify() == ProvenanceClass::ExcelTrusted` directly via the production reader. | closed |
| T-96-08b | Tampering | committed fixtures silently mutated by a normal test run | mitigate | Committed `.xlsx` written ONLY by `#[ignore]`+env-gated `regenerate_fixtures` (`fixture_author.rs:369-435`, no-op unless `PMCP_REGEN_FIXTURES=1`). Self-tests author into `tempfile::TempDir`. | closed |
| T-96-09 | Tampering | leap-year quirk smuggling date functions into whitelist | accept-by-design | `SPIKE-1900-leap.md:3,49-92` disposition (A): leap quirk encoded as whitelisted `IF`+`>`+`+` over f64; "NO `DATE`/`DATEVALUE` added". WHITELIST = 13 fns (`pmcp-workbook-dialect/src/lib.rs:35-38`); WBEX-02 traceability table at SPIKE l.75-92. | closed |
| T-96-10 | Elevation | loan fixture passing production freshness gate | mitigate | `production_compile_refuses_loan_fixture` (`reemit_loan.rs:411-425`): bare `compile_workbook` (Enforce) on loan bytes asserts `is_err()`; override path is the only acceptor. | closed |
| T-96-11 | Tampering | per-workbook Rust shortcut faking generalization | mitigate | `reemit_loan.rs` reads SERVED schema from GENERIC toolkit fns `input_schema_for_manifest`/`output_schema_for_manifest`/`GetManifestHandler` (l.250-406). Disjointness proof `loan_and_tax_served_key_sets_are_disjoint` (l.333-353); `name_named_inputs` (`lib.rs:630`) generic over any `in_*` named range. | closed |
| T-96-12 | Info disclosure | customer/TowelRads material leaking into fixture | mitigate | `loan_calc_spec` (`fixture_author.rs:714-857`) is synthetic rate-tier toy data. Grep of `tests/fixtures/` for towelrads/customer/client/ssn/account → NONE. | closed |
| T-96-13 | Tampering | quirk fixture weakening freshness/reconcile gate | mitigate | Quirk fixtures use the `#[cfg(test)]` override (`quirks_reconcile.rs:121`). Production-refusal spot check `production_compile_refuses_a_quirk_fixture` (l.250-269). Reconcile grades every value through `within_tol` (l.214-227); gate code untouched. | closed |
| T-96-14 | Tampering | exact-float `==` smuggled into money assertion | mitigate | All reconcile assertions go through `within_tol(&computed,&oracle)` / `TOL=0.01` (`quirks_reconcile.rs:76,217,239,290,293,322`); no exact-float `==` on money. Float-boundary quirk (`0.1+0.2`) is the in-corpus witness. | closed |
| T-96-14b | Tampering | quirk test silently degrading to compile-success only | mitigate | `recompute_at_reconcile_key` (`quirks_reconcile.rs:183-206`) runs `run_executor`, RETRIEVES the computed `CellValue` + cached oracle; `a_wrong_oracle_does_not_reconcile_proving_the_value_is_graded` (l.233-243) proves a wrong value fails. | closed |
| T-96-15 | Elevation | leap quirk forcing date functions into whitelist | accept-by-design | Same disposition as T-96-09: SPIKE-1900-leap.md fixes it in Plan 03 with WBEX-02 traceability; no DATE/DATEVALUE added (WHITELIST = 13 fns, drift-guard bound). | closed |
| T-96-SC | Tampering | npm/pip/cargo installs | mitigate | Only new runtime dep is `include_dir = "0.7.4"` (`cargo-pmcp/Cargo.toml:61`, vetted). `tempfile`/`rust_xlsxwriter` are `[dev-dependencies]` (`pmcp-workbook-compiler/Cargo.toml:81-91`). Dep additions match the claim — no install-legitimacy checkpoint needed. | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-96-01 | T-96-02 | D-03 single-source-of-truth: dialect version comes solely from the workbook cell. No CLI/pmcp.toml override is provided by design, so a flag cannot spoof the declared version. Verified absent in code. | Phase 96 design (D-03) | 2026-06-15 |
| AR-96-02 | T-96-09, T-96-15 | The 1900 leap-year quirk is bounded to whitelisted ops over f64 (`IF`/`>`/`+`); supporting it would require date functions, which are deliberately NOT added to the 13-fn whitelist. Recorded as a known limitation with WBEX-02 traceability in `SPIKE-1900-leap.md`. | Phase 96 spike disposition (A) | 2026-06-15 |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-06-15 | 18 | 18 | 0 | gsd-security-auditor (opus) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-06-15

---

## Notes

- Implementation files were NOT modified (read-only audit).
- The two heavier scaffold smokes (`scaffold_crate_cargo_check_compiles`, `embedded_assets_appear_in_cargo_package_list`) are `#[ignore]`+env-gated by design; their assertions exist and the backing mitigation logic (byte-equality, purity, packaging) is additionally covered by always-run unit tests, so closures are not contingent on the ignored smokes running in CI.
