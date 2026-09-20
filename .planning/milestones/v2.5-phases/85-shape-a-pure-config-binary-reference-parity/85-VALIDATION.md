---
phase: 85
slug: shape-a-pure-config-binary-reference-parity
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-26
---

# Phase 85 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + proptest + cargo doctest; mcp-tester library for scenario replay |
| **Config file** | none — workspace Cargo.toml drives test discovery |
| **Quick run command** | `cargo test -p pmcp-sql-server` |
| **Full suite command** | `make quality-gate` (fmt --all, clippy pedantic+nursery, build, test, audit) |
| **Estimated runtime** | ~90 seconds (quick) / ~5–8 min (full quality gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-sql-server` (or the touched crate)
- **After every plan wave:** Run `cargo test --workspace` for the affected crates
- **Before `/gsd:verify-work`:** `make quality-gate` must be green
- **Max feedback latency:** 90 seconds (quick test loop)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 85-01-xx | 01 | 1 | REF-01 | — | Chinook + 3 server configs parse cleanly; renames still rejected | unit | `cargo test -p pmcp-server-toolkit config_superset` | ❌ W0 | ⬜ pending |
| 85-01-xx | 01 | 1 | REF-01 | T-85-01 | `${VAR}` token-secret expands from env; missing var errors, never panics | unit+prop | `cargo test -p pmcp-server-toolkit env_expansion` | ❌ W0 | ⬜ pending |
| 85-02-xx | 02 | 1 | SHAP-A-01 | T-85-02 | `validate_code`/`execute_code` registered + enforce `[code_mode]` policy statically | integration | `cargo test -p pmcp-server-toolkit code_mode_tools` | ❌ W0 | ⬜ pending |
| 85-03-xx | 03 | 1 | REF-02 | — | Chinook DDL fixture committed; matches `SqliteConnector::schema_text()` shape | unit | `cargo test -p pmcp-sql-server schema_fixture` | ❌ W0 | ⬜ pending |
| 85-04-xx | 04 | 2 | SHAP-A-01 | T-85-03 | binary parses `--config`/`--schema`; dispatch on `[database] type`; compiled-out backend errors clearly | unit | `cargo test -p pmcp-sql-server dispatch` | ❌ W0 | ⬜ pending |
| 85-04-xx | 04 | 2 | SHAP-A-01 | — | `--schema` DDL → code-mode prompt + registered as MCP schema resource | integration | `cargo test -p pmcp-sql-server schema_resource` | ❌ W0 | ⬜ pending |
| 85-05-xx | 05 | 2 | SHAP-A-01 | T-85-04 | binary serves streamable HTTP; non-SQLite configs build connector + serve tools/list lazily (no creds) | integration | `cargo test -p pmcp-sql-server http_lazy_startup` | ❌ W0 | ⬜ pending |
| 85-06-xx | 06 | 3 | REF-02 | — | spawn binary on local HTTP port, replay `generated.yaml` via mcp-tester, all 31 scenarios pass (SC-3+SC-4) | integration | `cargo test -p pmcp-sql-server parity -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 85-06-xx | 06 | 3 | SHAP-A-01 | — | Shape C ≤15-line example builds + runs; doctests on every public item | doctest+example | `cargo test --doc -p pmcp-sql-server && cargo run --example sql_server_min` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-server-toolkit/tests/config_superset.rs` — parse-probe all 4 reference configs (REF-01); assert Chinook now parses + renames still rejected
- [ ] `crates/pmcp-server-toolkit/tests/env_expansion.rs` — `${VAR}` expansion unit + proptest (no-panic invariant)
- [ ] `crates/pmcp-server-toolkit/tests/code_mode_tools.rs` — `validate_code`/`execute_code` registration + static policy enforcement
- [ ] `crates/pmcp-sql-server/tests/` — dispatch, schema_resource, http_lazy_startup, parity integration tests
- [ ] `crates/pmcp-sql-server/tests/fixtures/chinook.ddl` + vendored `generated.yaml` (open question #1 — vendor from pmcp-run)
- [ ] `crates/pmcp-sql-server/examples/sql_server_min.rs` — Shape C ≤15-line example

*Existing config-parser fuzz target (Phase 77/84) covers REF-01 malformed-input fuzzing — extend, do not duplicate.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Athena/MySQL/Postgres query execution against real cloud backends | SHAP-A-01 (out of automated scope per D-02) | Needs cloud creds + S3/Glue/live DB; explicitly deferred | Run `pmcp-sql-server --config servers/open-images/config.toml --schema <ddl> --http 127.0.0.1:9000` with AWS creds set; confirm a real `tools/call` returns rows |

*All Phase-85-scoped behaviors (SQLite parity, parse, lazy startup) have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (fixtures, test files)
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
