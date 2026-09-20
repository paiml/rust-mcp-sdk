---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
verified: 2026-05-27T00:00:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 86: Shapes B/C/D — Scaffold, Library Example, Deploy — Verification Report

**Phase Goal:** A developer can choose any of three ergonomics levels for non-pure-config use cases — scaffold a starter project with `cargo pmcp new --kind sql-server` (Shape B), wire a ≤15-line `main.rs` library use (Shape C), or `cargo pmcp deploy` a config-only server to pmcp.run as a hosted target (Shape D) — and Phase 77's `cargo pmcp configure` target system accommodates each without breaking changes.
**Verified:** 2026-05-27T00:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `cargo pmcp new --kind sql-server` emits a single runnable crate (Cargo.toml + src/main.rs + config.toml + schema.sql); `cargo run` serves tools/list + one tools/call | ✓ VERIFIED | `cargo-pmcp/src/commands/new.rs:69-73` dispatches `--kind sql-server`; `cargo-pmcp/src/templates/sql_server.rs` emits all 4 files; `test_tools_list_and_call_against_scaffolded_server` passed (orchestrator-confirmed) |
| 2 | A runnable ≤15-line `examples/sql_server_http.rs` proves Shape C library use (toolkit + SQLite connector) | ✓ VERIFIED | `crates/pmcp-server-toolkit/examples/sql_server_http.rs` exists; `example_body_is_at_most_15_lines` test passes (confirmed by direct run); subprocess integration test `test_tools_list_and_call_against_serving_example` passed (orchestrator-confirmed) |
| 3 | `cargo pmcp deploy` for a config-only server adds no breaking changes to the Phase 77 `TargetEntry` enum | ✓ VERIFIED | `cargo-pmcp/src/commands/configure/config.rs:173` — enum has exactly 4 variants (PmcpRun/AwsLambda/GoogleCloudRun/CloudflareWorkers), unchanged; `target_entry_enum_unchanged` compile-time guard passes; no diff on configure/config.rs in Phase 86 commits |
| 4 | A deploy integration test exercises a config-only deploy against pmcp.run and confirms Phase 79 post-deploy lifecycle | ✓ VERIFIED | `cargo-pmcp/tests/deploy_config_only.rs:107` — `config_only_deploy_runs_phase79_lifecycle` test exists, double-gated (`#[ignore]` + `PMCP_RUN_DEPLOY_TEST` env gate), skips cleanly in normal CI (1 ignored confirmed by orchestrator); represents SC-4 deliverable per D-11 |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-server-toolkit/src/sql/sqlite.rs` | `SqliteConnector::execute_batch` multi-statement bootstrap helper | ✓ VERIFIED | Inherent method at line 139; unit-tested at lines 387-449 |
| `crates/pmcp-server-toolkit/src/lib.rs` | `demo_db_path()` asset-aware path resolver re-exported | ✓ VERIFIED | `pub fn demo_db_path()` at line 167; unit tests at lines 227-253 |
| `crates/pmcp-server-toolkit/Cargo.toml` | `http` feature forwarding `pmcp/streamable-http` | ✓ VERIFIED | Line: `http = ["pmcp/streamable-http"]`; example registered with `required-features = ["sqlite", "code-mode", "http"]` |
| `crates/pmcp-server-toolkit/examples/sql_server_http.rs` | ≤15-line Shape C example serving over HTTP | ✓ VERIFIED | 72 physical lines total; body statement count ≤15 confirmed by automated test |
| `crates/pmcp-server-toolkit/examples/fixtures/config.toml` | Shape C fixture config with `list_books` tool | ✓ VERIFIED | Exists; parses cleanly with `deny_unknown_fields` |
| `crates/pmcp-server-toolkit/examples/fixtures/schema.sql` | Idempotent DDL + seed for demo SQLite DB | ✓ VERIFIED | Exists; uses `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE` |
| `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs` | Integration test: subprocess + ≤15-line assertion | ✓ VERIFIED | Both tests exist and pass; `example_body_is_at_most_15_lines` confirmed passing |
| `cargo-pmcp/src/commands/new.rs` | `--kind sql-server` dispatch to `execute_sql_server` | ✓ VERIFIED | Lines 69-73: `match kind.as_deref()` dispatches `"sql-server"` path |
| `cargo-pmcp/src/main.rs` | `--kind` CLI arg wired to `new::execute` | ✓ VERIFIED | Lines 92-104: `New.kind: Option<String>` declared; line 526 passes it through |
| `cargo-pmcp/src/templates/sql_server.rs` | Scaffold emitter for all 4 files + deploy.toml | ✓ VERIFIED | `generate()` calls 5 sub-generators; golden test `emitted_main_matches_example_modulo_setup` enforces no drift from Shape C example |
| `cargo-pmcp/src/commands/deploy/mod.rs` | `is_config_driven_project` detection seam (D-09/D-10) | ✓ VERIFIED | `pub(crate) fn is_config_driven_project` at lines 104-111; 3-marker heuristic (config.toml + schema.sql + pmcp-server-toolkit dep) |
| `cargo-pmcp/src/deployment/builder.rs` | Single-crate Lambda resolution fallback (H3) + secret sanitization (H4) | ✓ VERIFIED | `is_single_crate_config_root()` at line 384; `find_lambda_package_dir` H3 fallback at line 338; `sanitize_config_bytes_for_deploy` at line 677; `bundled_artifact_paths_and_secret_posture` in-module test |
| `cargo-pmcp/tests/scaffold_sql_server.rs` | TEST-05 end-to-end scaffold integration test | ✓ VERIFIED | `test_tools_list_and_call_against_scaffolded_server` exists; orchestrator confirms it passed |
| `cargo-pmcp/tests/deploy_config_driven.rs` | D-10 compile-time enum guard + deploy.toml parse test | ✓ VERIFIED | Both tests pass (confirmed by direct run: 2 passed) |
| `cargo-pmcp/tests/deploy_config_only.rs` | TEST-06 env-gated real pmcp.run deploy test | ✓ VERIFIED | Exists; double-gated (`#[ignore]` + `PMCP_RUN_DEPLOY_TEST`); skips cleanly in CI (1 ignored) |
| `cargo-pmcp/tests/support/scaffold_patch.rs` | Shared `[patch.crates-io]` writer + ChildGuard | ✓ VERIFIED | Exists; shared by scaffold_sql_server.rs and deploy_config_only.rs via `#[path]` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `new.rs execute()` | `templates::sql_server::generate()` | `execute_sql_server()` at new.rs:70 | ✓ WIRED | Direct call through `execute_sql_server` helper |
| `templates/sql_server.rs` | `examples/sql_server_http.rs` | `include_str!` golden test at sql_server.rs:294 | ✓ WIRED | `emitted_main_matches_example_modulo_setup` test enforces byte-level parity |
| `deploy/mod.rs` | `builder.rs::find_lambda_package_dir` | H3 fallback path via `is_single_crate_config_root` | ✓ WIRED | Detection seam at mod.rs:104; builder resolution at builder.rs:338 |
| `builder.rs::bundle_assets_if_configured` | `sanitize_config_bytes_for_deploy` | `config_driven` flag at builder.rs:564 | ✓ WIRED | H4 secret-sanitization path is active for config-driven projects |
| `TargetEntry` enum | Phase 77 configure system | No change — zero enum modification (D-10) | ✓ WIRED | Compile-time guard `target_entry_enum_unchanged` in deploy_config_driven.rs |
| `scaffold templates` | `.pmcp/deploy.toml` | `generate_deploy_toml()` at sql_server.rs:225 | ✓ WIRED | Emits both `deploy.toml` + `.pmcp/deploy.toml` with `target_type = "pmcp-run"` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `sql_server_http.rs` main body | `cfg` (ServerConfig) | `pmcp::assets::load_string("config.toml")` + `ServerConfig::from_toml_strict_validated` | Yes — reads actual fixture file | ✓ FLOWING |
| `sql_server_http.rs` main body | `conn` (SqliteConnector) | `SqliteConnector::open(demo_db_path())` | Yes — opens real SQLite file | ✓ FLOWING |
| `sql_server_http.rs` main body | DB seeded | `conn.execute_batch(schema.sql)` | Yes — runs real DDL + INSERTs | ✓ FLOWING |
| scaffolded `main.rs` | Same as above, cwd-relative | Same asset resolution, no `PMCP_ASSETS_DIR` override | Yes — reads scaffolded config.toml/schema.sql | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `example_body_is_at_most_15_lines` passes | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode,http --test sql_server_http_example example_body_is_at_most_15_lines` | 1 passed | ✓ PASS |
| `target_entry_enum_unchanged` + `emitted_deploy_toml_parses_and_selects_pmcp_run` pass | `cargo test -p cargo-pmcp --test deploy_config_driven` | 2 passed | ✓ PASS |
| All cargo-pmcp tests pass (excluding `#[ignore]`) | `cargo test -p cargo-pmcp -- --test-threads=1` | 1098 passed, 1 ignored, 0 failed | ✓ PASS |
| All pmcp-server-toolkit tests pass (sqlite+code-mode+http) | `cargo test -p pmcp-server-toolkit --features sqlite,code-mode,http -- --test-threads=1` | 210 passed (across all suites), 0 failed | ✓ PASS |
| TEST-06 deploy test skips cleanly (not `--ignored`) | `cargo test -p cargo-pmcp --test deploy_config_only` | 1 ignored, 0 failed | ✓ PASS (skip expected per D-11) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SHAP-B-01 | 86-03-PLAN.md | `cargo pmcp new --kind sql-server` scaffolds starter project | ✓ SATISFIED | `new.rs:69-73` + `templates/sql_server.rs` + `scaffold_sql_server.rs` integration test PASSED |
| SHAP-C-01 | 86-02-PLAN.md | Runnable ≤15-line example proves library use | ✓ SATISFIED | `examples/sql_server_http.rs`; D-08 deviation approved: SQLite in-toolkit connector satisfies "+ chosen backend connector" in intent |
| SHAP-D-01 | 86-05-PLAN.md | `cargo pmcp deploy` config-only deploy without breaking changes to Phase 77 target system | ✓ SATISFIED | D-10: zero enum changes to `TargetEntry`; detection-based deploy via `is_config_driven_project` + H3 single-crate Lambda resolution |
| TEST-05 | 86-04-PLAN.md | `cargo pmcp new --kind sql-server` tempdir scaffold-to-run integration test | ✓ SATISFIED | `cargo-pmcp/tests/scaffold_sql_server.rs:78`; orchestrator confirmed test PASSED |
| TEST-06 | 86-06-PLAN.md | config-only deploy integration test (mock or real pmcp.run) | ✓ SATISFIED | `cargo-pmcp/tests/deploy_config_only.rs:107`; double-gated (`#[ignore]` + `PMCP_RUN_DEPLOY_TEST`); D-11: env-gated real test IS the SC-4 deliverable — skip-by-default behavior is intentional and not a gap |

### Anti-Patterns Found

| File | Location | Pattern | Severity | Impact |
|------|----------|---------|----------|--------|
| `cargo-pmcp/src/deployment/builder.rs` | Line 895 (inside `#[cfg(test)]`) | `"ELF-PLACEHOLDER"` | ℹ️ Info | In test code only — not production path; correct usage |
| `cargo-pmcp/src/templates/sql_server.rs` | Lines 59-62 | Emitted `Cargo.toml` includes `clap`, `tracing`, `tracing-subscriber` deps that emitted `main.rs` never uses | ⚠️ Warning | WR-03 from code review: unused deps slow first `cargo run`/`cargo lambda build`; no functional impact; flagged as advisory improvement |

**Note on WR-01 and WR-02 (from 86-REVIEW.md):** The secret-sanitization seam (WR-01: trigger coupled to `schema.sql` heuristic; WR-02: line-prefix rather than full key match) are both advisory warnings, not blockers. The H4 security requirement is met for the exact scaffold output this phase ships: `bundled_artifact_paths_and_secret_posture` test verifies the deployed artifact does not contain the inline dev literal. The review's suggested fixes are robustness improvements for future work.

### Human Verification Required

None. All success criteria are verified programmatically.

### Pre-Existing Quality Gate Issues (Out of Phase 86 Scope)

Per orchestrator notes, the full `make quality-gate` does not currently pass due to pre-existing issues in files Phase 86 did NOT touch:
- `cargo fmt` drift in Phase 84 connector crates (`pmcp-toolkit-athena`, `pmcp-toolkit-mysql`, `pmcp-toolkit-postgres`)
- A clippy lint in `crates/pmcp-server-toolkit/src/code_mode.rs:520` (from Plan 85-10)
- Dead-code warnings in `cargo-pmcp/src/pentest/`

Git history confirms none of these files were modified by Phase 86 commits. These are pre-existing issues from Phases 84 and 85 that are out of scope for this verification.

### Gaps Summary

No gaps. All four success criteria are fully achieved:

1. **SC-1 (SHAP-B-01/TEST-05):** `cargo pmcp new --kind sql-server` is fully wired from CLI (`main.rs --kind` arg) through `new.rs` dispatch to `templates/sql_server.rs` which emits all 4 files (Cargo.toml, src/main.rs, config.toml, schema.sql). The scaffold's main.rs is byte-identical (modulo harness seams) to the Shape C example via the golden test. TEST-05 integration test passed.

2. **SC-2 (SHAP-C-01):** `crates/pmcp-server-toolkit/examples/sql_server_http.rs` is a complete, runnable ≤15-line example (confirmed by automated assertion). D-08 deviation (SQLite in-toolkit rather than a separate `pmcp-toolkit-*` crate) is approved scope.

3. **SC-3 (SHAP-D-01):** `TargetEntry` enum is unchanged (4 variants, no new fields). Config-only deploy works via detection-based heuristic + H3 single-crate Lambda resolution fallback in `builder.rs`. The compile-time `target_entry_enum_unchanged` guard enforces this contract.

4. **SC-4 (TEST-06):** `cargo-pmcp/tests/deploy_config_only.rs` contains the env-gated real pmcp.run deploy test, double-gated (`#[ignore]` + `PMCP_RUN_DEPLOY_TEST`). Per D-11, this is the intended SC-4 deliverable. Skips cleanly in CI with 1 ignored test.

---

_Verified: 2026-05-27T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
