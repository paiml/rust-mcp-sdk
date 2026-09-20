---
phase: 86
slug: shapes-b-c-d-scaffold-library-example-deploy
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-26
---

# Phase 86 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 86-RESEARCH.md §Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` (cargo test) + `proptest 1` + `mcp-tester` `ServerTester` harness |
| **Config file** | none (cargo convention); per-crate `[dev-dependencies]` + `[[example]]` in `Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode -- --test-threads=1` |
| **Full suite command** | `cargo test -p pmcp-server-toolkit -p pmcp-sql-server -p cargo-pmcp --features sqlite,code-mode -- --test-threads=1` then `make quality-gate` |
| **Estimated runtime** | ~120 seconds (full suite, single-threaded) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched-crate> --features sqlite,code-mode -- --test-threads=1`
- **After every plan wave:** Run full suite across `pmcp-server-toolkit`, `pmcp-sql-server`, `cargo-pmcp` + `cargo run --example` for the Shape C example
- **Before `/gsd:verify-work`:** `make quality-gate` green (fmt + clippy pedantic/nursery + build + test + audit); PMAT cog ≤25 in CI
- **Max feedback latency:** ~120 seconds

---

## Per-Requirement Verification Map

> Plan/task-level rows are refined by the Nyquist auditor once PLAN.md files exist.
> This map fixes the requirement → observable-proof contract.

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| SHAP-B-01 | `new --kind sql-server` emits 4 files (`Cargo.toml`, `main.rs`, `config.toml`, `schema.sql`); emitted project `cargo run` serves `tools/list` + 1 `tools/call` | integration (tempdir) | `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` | ❌ W0 |
| SHAP-C-01 | ≤15-line example serves HTTP; `tools/list` + `tools/call` succeed | integration + runnable example | `cargo run -p pmcp-server-toolkit --example sql_server_http --features sqlite,code-mode` + spawn-poll test | ❌ W0 |
| SHAP-C-01 | example body is ≤15 lines | lint/assert | line-count assertion in the example test | ❌ W0 |
| SHAP-D-01 | config-driven project detection + `[assets]` bundling + `PmcpRun` reuse with NO `TargetEntry` enum change | unit (detection) + integration (bundle) + regression assert | `cargo test -p cargo-pmcp deploy_config_driven -- --test-threads=1` | ❌ W0 |
| TEST-05 | tempdir scaffold → spawn → poll readiness → `tools/list` + 1 `tools/call` | integration | `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` | ❌ W0 |
| TEST-06 | env-gated real pmcp.run deploy → Phase 79 `check` + `conformance` + `apps` clean | integration (gated) | `PMCP_RUN_DEPLOY_TEST=1 cargo test -p cargo-pmcp --test deploy_config_only -- --test-threads=1` | ❌ W0 |
| ALWAYS | config emission / parse round-trip | property | `cargo test -p pmcp-server-toolkit proptest -- --test-threads=1` | ❌ W0 |
| ALWAYS | config parser fuzz (reuse Phase 84 corpus) | fuzz | `cargo fuzz run pmcp_server_toolkit_config_parser` | ✅ corpus exists |
| ALWAYS | doctests on new public toolkit helper(s) (e.g. `bootstrap_from_sql`) | doctest | `cargo test --doc -p pmcp-server-toolkit --features sqlite,code-mode` | ❌ W0 (if helper added) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cargo-pmcp/tests/scaffold_sql_server.rs` — TEST-05 tempdir scaffold→spawn→poll→tools/list+call (with `[patch.crates-io]`/path override — RESEARCH Pitfall §1: toolkit crate is unpublished)
- [ ] `cargo-pmcp/tests/deploy_config_only.rs` — TEST-06 env-gated real-pmcp.run deploy + Phase 79 lifecycle assertion
- [ ] `crates/pmcp-server-toolkit/examples/sql_server_http.rs` (or upgraded `sqlite_minimal.rs`) — the runnable ≤15-line serving example (SHAP-C-01)
- [ ] `[[example]]` registration in `crates/pmcp-server-toolkit/Cargo.toml` with `required-features = ["sqlite", "code-mode"]`
- [ ] `cargo-pmcp/src/templates/sql_server.rs` (new template module) + `pub mod sql_server;` in `templates/mod.rs`
- [ ] `--kind` arg on the CLI `New {}` variant + threading through `new::execute`
- [ ] (Recommended) `SqliteConnector::execute_batch` / toolkit `bootstrap_from_sql` helper (RESEARCH Pitfall §3 — multi-statement `schema.sql` bootstrap) + its doctest
- [ ] Property test for config-emission/parse round-trip; reuse the existing config-parser fuzz corpus
- [ ] Detection-seam unit test asserting `TargetEntry` enum is unchanged (D-10 regression guard)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real pmcp.run deploy round-trip | TEST-06 / SHAP-D-01 | Requires live pmcp.run creds; gated out of CI (D-11) | Set the deploy creds + `PMCP_RUN_DEPLOY_TEST=1`, run `cargo test -p cargo-pmcp --test deploy_config_only -- --test-threads=1`; confirm `check`/`conformance`/`apps` all pass |

*All other phase behaviors have automated, CI-runnable verification (SQLite is zero-creds).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
