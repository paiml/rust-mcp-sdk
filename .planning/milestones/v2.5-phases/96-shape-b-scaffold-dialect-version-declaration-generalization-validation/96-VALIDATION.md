---
phase: 96
slug: shape-b-scaffold-dialect-version-declaration-generalization-validation
status: verified
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-14
updated: 2026-06-15
---

# Phase 96 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust) + `cargo nextest` if available |
| **Config file** | none — workspace `Cargo.toml` test harness |
| **Quick run command** | `cargo test -p pmcp-workbook-dialect -p cargo-pmcp` |
| **Full suite command** | `make quality-gate` (fmt + clippy + build + test + audit) + `make purity-check` |
| **Estimated runtime** | ~120–300 seconds |

---

## Sampling Rate

- **After every task commit:** Run the relevant crate's `cargo test -p <crate>`
- **After every plan wave:** Run `cargo test --workspace` for touched crates
- **Before `/gsd:verify-work`:** `make quality-gate` must be green AND `make purity-check` must pass (the explicit phase-final gate — Codex MEDIUM)
- **Max feedback latency:** ~300 seconds

---

## Cross-AI Review Disposition (Phase 96 `--reviews` replan, 2026-06-15)

Verified against the codebase before actioning (reviewer claims are hypotheses; some were false positives):

| Review item | Verdict | Action |
|-------------|---------|--------|
| WBEX-01 served-schema proof (96-04) | VALID | 96-04 T2 upgraded: assert served `input_schema_for_manifest`/`output_schema_for_manifest` + `GetManifestHandler` over the loaded loan bundle; 5 generic tool NAMES unchanged; loan fields present + tax fields absent. Reachable under the EXISTING `workbook` dev-dep (no new dep). |
| Fixture reproducibility + provenance assertion (96-03) | VALID | 96-03 T1 upgraded: `#[ignore]`+env-gated non-mutating generator; direct `classify() == ProvenanceClass::ExcelTrusted` assertion; production-refusal self-test. |
| Scaffold example visibility + post-publish packaging (96-02) | VALID | `templates` is bin-only (verified) → 96-02 expose a narrow lib seam + drive example/integration test through it; embed assets via `include_dir!`/`include_bytes!` under the package root + `cargo package --list` smoke. |
| Contract-first (`../provable-contracts/` + `pmat comply check`) | **FALSE POSITIVE** | VERIFIED: `../provable-contracts/` does NOT exist; Phase 95 already recorded contract-first does NOT apply to these crates (the analog `pmcp-sql-server` ships without it). NO contract task added; revisit if the directory is established later. |
| 96-01 parser policy + non-masking fuzz | VALID | 96-01 T2/T3: explicit grammar (MAJOR.MINOR[.PATCH], whitespace/leading-zero/u64/patch-ignored); PUBLIC parser path (fuzz crate is separate — `pub`, not `pub(crate)`); fuzz dir ALREADY EXISTS (drop "create if absent"); non-masking grep gate + manual nightly fuzz; integration test over 5 cases. |
| 96-01 typed error | VALID (keep + justify) | `CompileError::Lint` via stage-1 collect-all IS the idiomatic typed error; no separate structured finding type exists to prefer — noted in 96-01 T2. |
| 96-01 ALWAYS EXAMPLE (CLAUDE.md NO EXCEPTIONS) | VALID (revision 2026-06-15) | WBDL-02 adds a net-new PUBLIC parser feature; CLAUDE.md requires a runnable `cargo run --example` demo. 96-01 T2 folded in `examples/dialect_version_demo.rs` (no 4th task) calling the public `parse_dialect_version` + compat fn over absent->baseline / compatible->accepted / incompatible->typed-error, exit 0, no `.xlsx`, no `#[ignore]`. |
| 1900-leap WBEX-02 traceability (96-03/05) | VALID | 96-03 T2 requires a `## WBEX-02 Traceability` section; 96-05 carries a quirk→WBEX-02 map. |
| 96-05 quirk precision | VALID | Each quirk = {formula+context, oracle, runtime expected, reconcile key}; ≥1 reconcile per named quirk (or documented stand-in); production-refusal spot check. |
| Final gate `make quality-gate` + `make purity-check` | Already present | Sampling Rate above (phase-final) + Full suite command. |
| LOW: serialized lib.rs edits / LF-CRLF / over-serialized depends_on | Acknowledged | Waves serialize lib.rs edits (each plan adds exactly one `mod` decl, no duplicates); 96-02 drift-lock normalizes LF/CRLF; 96-05 `depends_on:[96-03,96-04]` kept (safe). |

No deferred-by-design scope pulled in (row iteration, capability cells, validation lists, registry store, pmcp-code-mode all remain out).

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| T1 | 96-01 | 1 | WBDL-02 | T-96-03 | version consts bound to spec by drift guard; grammar documented | unit | `cargo test -p pmcp-workbook-dialect` | ✅ | ✅ green |
| T2 | 96-01 | 1 | WBDL-02 | T-96-01 | fail-closed typed CompileError on incompatible/malformed version; PUBLIC parser; explicit grammar matrix; ALWAYS example demonstrates absent/compatible/incompatible | unit + property + example | `cargo test -p pmcp-workbook-compiler dialect_version && cargo run -p pmcp-workbook-compiler --example dialect_version_demo` | ✅ | ✅ green |
| T3 | 96-01 | 1 | WBDL-02 | T-96-01 | wired stage-1 check + 5-case integration test; absent→baseline preserved; registered fuzz target (non-masking grep gate) | unit + integration + fuzz | `cargo test -p pmcp-workbook-compiler && grep -q 'name = "dialect_version_parse"' crates/pmcp-workbook-compiler/fuzz/Cargo.toml && grep -q 'dialect_version::parse_dialect_version' crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dialect_version_parse.rs` | ✅ | ✅ green |
| T1 | 96-02 | 1 | WBCL-05 | T-96-05 | scaffold Cargo.toml default-features=false (purity-safe); EMBEDDED publish-safe assets; lib-callable generate | unit | `cargo build -p cargo-pmcp && cargo build -p cargo-pmcp --lib` | ✅ | ✅ green |
| T2 | 96-02 | 1 | WBCL-05 | T-96-04 / T-96-06 / T-96-06b | path-traversal guard; bundle-bytes drift lock (LF/CRLF-safe); hardcoded-version drift guard | unit | `cargo test -p cargo-pmcp workbook_server` | ✅ | ✅ green |
| T3 | 96-02 | 1 | WBCL-05 | T-96-06b | scaffold via CLI/lib seam + scaffold-build smoke + packaging smoke; ALWAYS example through lib-public seam | example + integration | `cargo run -p cargo-pmcp --example workbook_server_scaffold && cargo test -p cargo-pmcp --test workbook_scaffold` | ✅ | ✅ green |
| T1 | 96-03 | 2 | WBEX-01 / WBEX-02 | T-96-07 / T-96-08 / T-96-08b | direct `classify()==ExcelTrusted` assertion; production-refusal self-test; non-mutating env-gated generator | integration | `cargo test -p pmcp-workbook-compiler fixture_author` | ✅ | ✅ green |
| T2 | 96-03 | 2 | WBEX-02 | T-96-09 | 1900-leap disposition + WBEX-02 traceability recorded; no DATE functions added | integration + doc | `test -f crates/pmcp-workbook-compiler/SPIKE-1900-leap.md && grep -q "Disposition" crates/pmcp-workbook-compiler/SPIKE-1900-leap.md && grep -q "WBEX-02 Traceability" crates/pmcp-workbook-compiler/SPIKE-1900-leap.md && cargo test -p pmcp-workbook-compiler` | ✅ | ✅ green |
| T1 | 96-04 | 3 | WBEX-01 | T-96-12 | synthetic loan fixture (no customer/TowelRads material); multiple unit-carrying `out_*` outputs | integration | `test -f crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx && test -f crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.provenance-override.json && cargo test -p pmcp-workbook-compiler fixture_author` | ✅ | ✅ green |
| T2 | 96-04 | 3 | WBEX-01 | T-96-10 / T-96-11 | loan serves OWN get_manifest/tools/list schema (loan fields present, tax fields absent) behind UNCHANGED 5 generic names; production-refusal counter-test | integration | `cargo test -p pmcp-workbook-compiler reemit_loan` | ✅ | ✅ green |
| T1 | 96-05 | 4 | WBEX-02 | T-96-15 | per-quirk scalar_eval assertions ({formula+context,oracle,expected}) against excel_round (no naive round) | unit | `cargo test -p pmcp-workbook-runtime scalar_eval` | ✅ | ✅ green |
| T2 | 96-05 | 4 | WBEX-02 | T-96-13 / T-96-14 / T-96-14b | penny reconcile via within_tol (computed-vs-oracle, not compile-success); ≥1 reconcile per named quirk; production-refusal spot check; WBEX-02 traceability map | integration | `cargo test -p pmcp-workbook-compiler quirks` | ✅ | ✅ green |

*Filled by planner; Status: ✅ green · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `.xlsx` fixture authoring recipe (rust_xlsxwriter with Excel identity + `compile_workbook_with_fixture_override` / `FreshnessPolicy::TrustedFixture`) proven reusable for the loan workbook (WBEX-01) and the quirk corpus (WBEX-02), with a direct `classify()==ExcelTrusted` assertion + a non-mutating env-gated generator — established by 96-03 Task 1 (`fixture_author.rs`)
- [x] Doc↔const binding test harness extended to cover the dialect-version surface (WBDL-02) — established by 96-01 Task 1 (`SUPPORTED_DIALECT_VERSION` / `BASELINE_DIALECT_VERSION` consts + spec-doc drift guard)
- [x] `dialect_version.rs` reader + PUBLIC explicit-grammar parser + semver-compat decision + ALWAYS `examples/dialect_version_demo.rs` created (WBDL-02) — 96-01 Task 2
- [x] Compiler fuzz harness for the version-string parse REGISTERED in the existing `fuzz/` crate + 5-case integration test (WBDL-02 ALWAYS fuzz) — 96-01 Task 3 (`fuzz/fuzz_targets/dialect_version_parse.rs`)
- [x] `workbook_server.rs` scaffold template (EMBEDDED publish-safe assets) + lib seam + drift-lock/bundle-bytes/version-drift/packaging golden tests (WBCL-05) — 96-02 Tasks 1–3
- [x] 1900-leap-year disposition + WBEX-02 traceability decided so the quirk corpus has a path (WBEX-02) — 96-03 Task 2 (`SPIKE-1900-leap.md`)
- [x] `reemit_loan.rs` in-crate compile-and-SERVE-SCHEMA proof (WBEX-01) — 96-04 Task 2
- [x] `quirks_reconcile.rs` mini-reconcile harness (computed-vs-oracle) + production-refusal spot check + per-quirk scalar_eval tests + WBEX-02 traceability map (WBEX-02) — 96-05 Tasks 1–2

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo pmcp new --kind workbook-server` produces a crate that `cargo run`s and serves | WBCL-05 | end-to-end scaffold serve smoke is best confirmed by a one-time manual run | scaffold into a tmp dir, `cargo run`, hit `tools/list` |
| `cargo +nightly fuzz run dialect_version_parse` clean | WBDL-02 | the fuzz crate is workspace-excluded + nightly-only; CI cannot run it on stable | run on nightly, record clean in 96-01 SUMMARY |

*Automated coverage preferred; the scaffold round-trip is additionally covered by the `workbook_server_scaffold` example + the `workbook_scaffold` integration test (CLI/lib seam + scaffold-build + packaging smoke) + the drift-lock/bundle-bytes/file-presence golden tests (96-02), so the manual serve run is a confirmatory smoke only. The fuzz target is registered + grep-gated on stable (non-masking); the nightly run is the ALWAYS-fuzz execution. The WBDL-02 ALWAYS example (`dialect_version_demo`) is automated on stable — `cargo run -p pmcp-workbook-compiler --example dialect_version_demo` exits 0 — so it is NOT a manual-only item.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 300s
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Cross-AI review items dispositioned (verified vs false-positive)
- [x] ALWAYS EXAMPLE present for the WBDL-02 feature (`dialect_version_demo`, automated on stable)

**Approval:** validated 2026-06-15

---

## Validation Audit 2026-06-15

State A retroactive audit of the completed phase. Every Per-Task Map automated command was executed against the live tree (genchi genbutsu) and confirmed green — not inferred from SUMMARY/VERIFICATION claims:

| Metric | Count |
|--------|-------|
| Tasks audited | 11 |
| Resolved (COVERED, green) | 11 |
| Escalated | 0 |
| Gaps found | 0 |

Evidence (live runs):
- `cargo test -p pmcp-workbook-dialect` → 6 passed (incl. `doc_versions_match_consts` drift guard)
- `cargo test -p pmcp-workbook-compiler dialect_version` → 30 passed
- `cargo test -p cargo-pmcp workbook_server` → 8 passed; `--test workbook_scaffold` → 2 passed, 2 ignored (env-gated smokes)
- `cargo test -p pmcp-workbook-compiler fixture_author` → 6 passed, 1 ignored
- `cargo test -p pmcp-workbook-compiler reemit_loan` → 9 passed
- `cargo test -p pmcp-workbook-compiler quirks` → 5 passed
- `cargo test -p pmcp-workbook-runtime scalar_eval` → 12 passed
- `cargo run -p pmcp-workbook-compiler --example dialect_version_demo` → exit 0 (absent/compatible/incompatible)
- `cargo run -p cargo-pmcp --example workbook_server_scaffold` → exit 0 (11-file tree)
- fuzz grep-gate (`dialect_version_parse` bin + public-parser call), SPIKE-1900-leap.md (Disposition + WBEX-02 Traceability), and loan fixture + override marker all present.

No new test files generated — phase shipped with complete automated coverage. `nyquist_compliant: true` confirmed by execution.
