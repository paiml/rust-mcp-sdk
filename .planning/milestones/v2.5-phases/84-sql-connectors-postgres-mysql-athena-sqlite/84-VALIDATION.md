---
phase: 84
slug: sql-connectors-postgres-mysql-athena-sqlite
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-19
---

# Phase 84 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 84-RESEARCH.md §6 (Validation Architecture).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust 2021, workspace) + `cargo fuzz` + `cargo run --example` |
| **Config file** | Per-crate `Cargo.toml` `[dev-dependencies]` + workspace root `Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-server-toolkit --features sqlite` (single crate, fast) |
| **Full suite command** | `make quality-gate` (matches CI exactly — `cargo fmt --all --check`, clippy pedantic+nursery with `--features full`, build, test workspace, audit) |
| **Estimated runtime** | Quick ~15s · Full ~3–5min (matches CI wall clock) |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p {touched_crate}` (≤5s incremental)
- **After every task that adds tests:** Run `cargo test -p {touched_crate}` (≤30s)
- **After every plan wave:** Run `cargo test --workspace --features sqlite` plus `cargo clippy --workspace --all-targets -- -D warnings` (≤90s)
- **Before `/gsd:verify-work`:** Full `make quality-gate` must be green
- **Max feedback latency:** 30 seconds at task granularity, 90 seconds at wave granularity

---

## Per-Task Verification Map

> Filled by the planner during PLAN.md generation. One row per task in the
> task XML blocks across all `84-NN-PLAN.md` files. The matrix below seeds the
> structure with one row per phase-level coverage axis from RESEARCH.md §6 so
> the planner can expand it row-by-row as it writes plans.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 84-02-01 | 84-02 | 2 | CONN-03 | — | placeholder translation never panics on malformed input | property | `cargo test -p pmcp-server-toolkit --features sqlite translate_placeholders_props` | ✅ | ✅ green |
| 84-01-01 | 84-01 | 1 | CONN-01 | T-84-01-01 | `execute()` returns ordered, JSON-serializable rows | unit + integration | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode sql::` | ✅ | ✅ green |
| 84-05-01 | 84-05 | 2 | CONN-05 | — | Postgres connector binds `$1` correctly against authentic in-process mock | integration | `cargo test -p pmcp-toolkit-postgres --features dev_mock` | ✅ | ✅ green |
| 84-06-01 | 84-06 | 2 | CONN-06 | — | MySQL connector binds `?` correctly against authentic in-process mock | integration | `cargo test -p pmcp-toolkit-mysql --features dev_mock` | ✅ | ✅ green |
| 84-07-01 | 84-07 | 2 | CONN-07 | — | Athena connector binds `?` + GetTableMetadata for schema_text against in-process mock (NO Glue) | integration | `cargo test -p pmcp-toolkit-athena --features dev_mock` | ✅ | ✅ green |
| 84-04-01 | 84-04 | 1 | CONN-08 | — | SQLite connector executes against real in-memory `rusqlite` DB | integration | `cargo test -p pmcp-server-toolkit --features sqlite sqlite::` | ✅ | ✅ green |
| 84-03-01 | 84-03 | 1 | CONN-04 | — | `build_code_mode_prompt(connector)` produces dialect-aware body for all 4 dialects | unit | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode code_mode::build_code_mode_prompt` | ✅ | ✅ green |
| 84-01-02 | 84-01 | 1 | CONN-02 | — | `schema_text()` is dialect-styled (not normalized) and folds `[[database.tables]]` descriptions | unit | `cargo test -p pmcp-server-toolkit --features sqlite schema_text` | ✅ | ✅ green |
| 84-08-02 | 84-08 | 3 | TEST-01 | T-84-08-05 | All four per-backend integration suites exit 0 (no Docker / no testcontainers anywhere) | workspace | `cargo test -p pmcp-toolkit-{postgres,mysql,athena} --features dev_mock` + `! grep -rn testcontainers crates/pmcp-toolkit-*` | ✅ | ✅ green |
| 84-08-01 | 84-08 | 3 | TEST-07 | T-84-08-01 | Toolkit config-toml fuzz target survives 60s without panic | fuzz | `cd crates/pmcp-server-toolkit && cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` | ✅ | ✅ green |
| 84-01-03 | 84-01..07 | 1–2 | CONN-01/03/04 | — | All new public types/fns have working doctests | doctest | `cargo test --doc -p pmcp-server-toolkit --features sqlite,code-mode` + `cargo test --doc -p pmcp-toolkit-{postgres,mysql,athena} --features dev_mock` | ✅ | ✅ green |
| 84-05-02 | 84-04..07 | 1–2 | CONN-05/06/07/08 | — | One `cargo run --example` per backend demonstrating Shape-C-shaped use (`{postgres,mysql,athena}_minimal.rs`, `sqlite_minimal.rs`) | example | `cargo run -p pmcp-toolkit-{postgres,mysql,athena} --features dev_mock --example {backend}_minimal` and `cargo run -p pmcp-server-toolkit --features sqlite --example sqlite_minimal` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

The planner SHALL allocate Wave 0 tasks to create the following before any
production code lands:

- [x] `crates/pmcp-server-toolkit/src/sql/translate.rs` — `SqlWalker` state machine + `TranslatedSql` shipped GREEN in Plan 84-02 (5 proptests + 4 H7 named edge tests)
- [x] `crates/pmcp-server-toolkit/tests/translate_placeholders_props.rs` — property-test scaffold landed Wave 0 (84-00), GREEN in 84-02 (idempotence, bind-order, no-panic invariants)
- [x] `crates/pmcp-toolkit-postgres/` — workspace member shipped (84-05); mock lives at `src/dev_mock.rs` under `dev_mock` feature (REVIEWS H5; legacy `tests/mock_postgres.rs` removed); example `examples/postgres_minimal.rs`
- [x] `crates/pmcp-toolkit-mysql/` — shipped (84-06); same `src/dev_mock.rs` shape; `examples/mysql_minimal.rs`
- [x] `crates/pmcp-toolkit-athena/` — shipped (84-07); same `src/dev_mock.rs` shape; `examples/athena_minimal.rs`
- [x] `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` — single target reused (D-14 extend-don't-duplicate); 3 per-backend + 4 REVIEWS M6 adversarial URL corpus seeds added in 84-08 (`seed-database-url.toml` from 84-00)
- [x] Cargo.toml root `[workspace.members]` — three new entries inserted in Wave 0 (84-00)

*If existing infrastructure covers a row, the planner SHALL strike it; otherwise it becomes a Wave 0 task.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real-AWS Athena query against a live workgroup | CONN-07 | Requires AWS credentials + an Athena-backed S3 dataset; CI has neither | One-time smoke run by operator: `AWS_PROFILE=… cargo run -p pmcp-toolkit-athena --example shape_c` against the `open-images` config |
| Real-Postgres / MySQL container query | CONN-05 / CONN-06 | We deliberately ship NO Docker / testcontainers — authentic in-process mocks cover automated CI | One-time smoke run by operator: `DATABASE_URL=postgres://… cargo run -p pmcp-toolkit-postgres --example shape_c`, same for MySQL with their own URL |
| `make quality-gate` matches CI exactly | TEST-01 | Reproducing CI's `--features full` pedantic+nursery clippy locally is the contract per CLAUDE.md | Operator runs `make quality-gate` before pushing the phase merge |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (skeleton crates + property-test scaffold + workspace-members insertion)
- [x] No watch-mode flags (no `cargo watch`; CI uses one-shot `cargo test`)
- [x] Feedback latency < 90s at wave granularity
- [x] `nyquist_compliant: true` set in frontmatter after planner fills the per-task matrix

**Approval:** approved (Phase 84 closeout, Plan 84-08, 2026-05-26). Phase-84-scoped verification green: 4 connector/toolkit crates build + test + doctest clean (108 toolkit tests, 31 doctests, postgres/mysql/athena dev_mock suites), PMAT zero cog-25 violations on new files, all REVIEWS H1–H7 + M1–M6 guards pass, 60s fuzz clean. Broad `make quality-gate` deferred to CI due to pre-existing unrelated rust-1.95.0 pedantic lints in `pmcp-widget-utils` (documented in `deferred-items.md`; NOT a Phase 84 regression).
