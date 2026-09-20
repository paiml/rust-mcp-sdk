---
phase: 94
slug: cli-subcommands-pmcp-toml
status: verified
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-15
---

# Phase 94 — Validation Strategy

> Per-phase validation contract. Reconstructed retroactively (State B) from PLAN/SUMMARY/VERIFICATION artifacts; every automated command was executed live and confirmed green.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust) — workspace harness, no external config |
| **Config file** | none — workspace `Cargo.toml` |
| **Quick run command** | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::` |
| **Full suite command** | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook:: && cargo test -p cargo-pmcp --test workbook_cli_integration && cargo test -p pmcp-workbook-compiler` |
| **Estimated runtime** | ~60–120 seconds |

> **Test-target gotcha (Genchi Genbutsu, recorded in VERIFICATION.md):** `cargo-pmcp`'s `commands::*` modules compile into the **bin target only** (`lib.rs` excludes them). Use `--bin cargo-pmcp`, never `--lib` (which reports a false `0 passed`).

---

## Sampling Rate

- **After every task commit:** `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::<mod>`
- **After every plan wave:** full suite command above
- **Before `/gsd:verify-work`:** full suite green; `make purity-check` (the `#[ignore]`d integration test) confirms served trees stay reader-free
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------------|-----------|-------------------|-------------|--------|
| 94-00 | 00 | 0 | WBCL-01 / WBCL-03 (seam) | gated-update candidate facade `prepare_candidate` + hash-covered `write_gate_marker`/`read_gate_marker` (D-08 tamper-evident); gate-before-write | unit | `cargo test -p pmcp-workbook-compiler prepare_candidate && cargo test -p pmcp-workbook-compiler gate_marker && cargo test -p pmcp-workbook-compiler gate` | ✅ | ✅ green |
| 94-01 | 01 | 1 | WBCL-04 | `PmcpToml`/`WorkbookEntry` containment guard (rejects absolute + `..`-escape); absent → `Ok(None)` (optional) | unit | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::config` | ✅ | ✅ green |
| 94-02 | 02 | 1 | WBCL-02 | lint standalone: errors → non-zero, warnings pass (D-10); text/json report | unit | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::lint` | ✅ | ✅ green |
| 94-03 | 03 | 2 | WBCL-01 / WBCL-04 | compile seed lane (`compile_workbook`) + gated lane (`prepare_candidate`→`gate`→block/`promote`); gate-block → distinct exit 2, writes nothing; version from Excel (no `--version` flag) | unit | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::compile` | ✅ | ✅ green |
| 94-04 | 04 | 2 | WBCL-03 / WBCL-04 | emit: zero `gate::gate` calls; `gated:false` marker + loud UNGATED banner; CR-02 `@<version>` non-overwrite; `--approver` not required | unit | `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::emit` | ✅ | ✅ green |
| 94-05 | 05 | 3 | WBCL-01 / WBCL-02 / WBCL-03 / WBCL-04 | end-to-end CLI: versionless-fixture refusal, lint clean exit 0, json parseable, compile-all over 2-entry toml; reader-free served-tree guard; non-vacuous compiler edge; ALWAYS example | integration + example | `cargo test -p cargo-pmcp --test workbook_cli_integration && cargo run -p cargo-pmcp --example workbook_cli_demo` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. The Wave-0 plan (94-00) delivered the compiler-crate library seam (`prepare_candidate`, `Candidate`, `write_gate_marker`/`read_gate_marker`, gate consts) that the CLI Waves 1–3 consume — its tests live in `crates/pmcp-workbook-compiler/` (`prepare_candidate_tests.rs` + gate suite) and run green.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `make purity-check` over the workbook offline/served cones | WBCL-01..04 (served-contract guard) | The purity-check integration test is `#[ignore]`d to keep the default `cargo test` fast (runs `cargo tree` per crate/feature) | `cargo test -p cargo-pmcp --test workbook_cli_integration -- --ignored` or `make purity-check` |
| Gate-BLOCK happy path + accepted-version two-version E2E | WBCL-01 | Requires a genuine two-version workbook fixture that cannot be constructed from the sole Phase-93 `tax-calc.xlsx`; documented residual risk in 94-05-SUMMARY | unit-covered in 94-00/94-03/94-04; full E2E needs a versioned fixture pair (deferred) |

*The gate-block/two-version path is unit-covered; only the cross-fixture E2E is deferred (residual risk, not a delivery gap — recorded in VERIFICATION.md).*

---

## Validation Sign-Off

- [x] All tasks have automated verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (existing infra + 94-00 seam)
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** validated 2026-06-15

---

## Validation Audit 2026-06-15

State B retroactive reconstruction. Every Per-Task Map command executed against the live tree and confirmed green:

| Metric | Count |
|--------|-------|
| Tasks audited | 6 |
| Resolved (COVERED, green) | 6 |
| Escalated | 0 |
| Gaps found | 0 |

Live evidence:
- `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::` → 50 passed (incl. bounded proptests)
- `cargo test -p cargo-pmcp --test workbook_cli_integration` → 10 passed, 1 ignored (`#[ignore]`d purity-check)
- `cargo run -p cargo-pmcp --example workbook_cli_demo` → exit 0
- `cargo test -p pmcp-workbook-compiler prepare_candidate` → 5 passed; `gate_marker` → 4 passed; `gate` → 46 passed

No new test files generated — phase shipped with complete automated coverage. `nyquist_compliant: true` confirmed by execution.
