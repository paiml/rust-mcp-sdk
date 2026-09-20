---
phase: 85-shape-a-pure-config-binary-reference-parity
verified: 2026-05-27T06:00:00Z
status: passed
score: 4/4
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/4
  re_verified: 2026-05-27T10:00:00Z
  gaps_closed:
    - "SC-3 Gap 1: sql_require_limit mapping was discarded (_require_limit_gap); now cfg.sql_require_limit = section.require_limit; (code_mode.rs:551) + missing_limit rejection enforced in validation.rs:1085"
    - "SC-3 Gap 2: parity test gated only on result.success (masked by continue_on_failure exclusion); now gates per-step via result.step_results[i].success + REQUIRED_REJECTION_SCENARIOS presence guard"
    - "SC-3 Gap 3: start_code_mode prompt silently dropped code-mode://instructions + code-mode://policies include_resources; now synthesized via merged_resource_configs + synthesize_instructions_resource + synthesize_policies_resource in assemble.rs"
  gaps_remaining: []
  regressions: []
  history: >
    Initial verification (2026-05-27T06:00:00Z) passed 4/4 on automated checks.
    Code review (/code-review, extra-high recall) reopened SC-3 (reopen timestamp
    2026-05-27T07:30:00Z): the no-LIMIT rejection scenario actually FAILED its
    assertion (masked by continue_on_failure), require_limit was parsed but discarded
    (_require_limit_gap), and the prompt silently omitted two of five include_resources.
    Plans 85-07 through 85-10 closed all three gaps. Re-verification on 2026-05-27
    confirms all fixes empirically: 133+201+39 tests pass across the three affected
    crates with no regressions.
---

# Phase 85: Shape A Pure-Config Binary — Reference Parity Verification Report

**Phase Goal:** A non-developer can take any of the existing pmcp-run/built-in/sql-api/servers/*/config.toml files unchanged, run `pmcp-sql-server --config <file> --schema <file>`, and get a live MCP server with the same tools, same code-mode policy, and same observable behavior as the production pmcp-run server — proving the toolkit lift end-to-end.

**Verified:** 2026-05-27T06:00:00Z (initial); 2026-05-27T10:00:00Z (re-verification after gap closure)
**Status:** passed
**Re-verification:** Yes — after SC-3 reopen and gap closure (plans 85-07 through 85-10)

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Running `pmcp-sql-server --config <reference config> --schema <schema-file>` produces a running MCP server with ZERO Rust written by the user | VERIFIED | `parity_chinook.rs` drives the REAL `run_serving` binary path (`Args { config: temp_toml, schema: temp_ddl, http: "127.0.0.1:0" }`) — no connector injection. `cargo test -p pmcp-sql-server --test parity_chinook`: `1 passed`. |
| 2 | The toolkit's config.toml schema is a SUPERSET of the existing pmcp-run sql-api server configs — any of the three reference servers' configs parse cleanly, additive new keys allowed, renames NOT | VERIFIED | `config_superset.rs` covers all four reference configs (open-images/imdb/msr-vtt Athena + Chinook SQLite). `deny_unknown_fields` count is 21 (all additive). `renames_rejected` test asserts a `filepath` typo is rejected. All 5 toolkit suites pass (201 passed). |
| 3 | The reproduced server responds to tools/list, tools/call for every [[tools]] entry, AND the code-mode pair (validate_code/execute_code) with policy enforcement matching production behavior | VERIFIED (re-verified after gap closure) | Gap 1 closed: `cfg.sql_require_limit = section.require_limit;` at `code_mode.rs:551`; `missing_limit` rejection enforced in `validation.rs:1085`. Gap 2 closed: `parity_chinook.rs` now gates per-step (`result.step_results[i].success`) + REQUIRED_REJECTION_SCENARIOS presence guard — the no-LIMIT `SELECT * FROM Artist` is individually gating. Gap 3 closed: `synthesize_instructions_resource` + `synthesize_policies_resource` in `assemble.rs` produce `code-mode://instructions` and `code-mode://policies`; `prompt_body_carries_synthesized_instructions_and_policies` async test asserts the served body contains the instructions marker, dialect name (SQLite), policy fields (require_limit, max_limit, allow_writes, Email), and no secret leak. Full parity test: `1 passed` with CODE_MODE_SECRET set. |
| 4 | Replaying a representative subset of reference scenarios against the Shape A reproduction yields result parity on the asserted scenarios | VERIFIED | `cargo test -p pmcp-sql-server --no-default-features --features sqlite --test parity_chinook -- --test-threads=1` passes. `generated.yaml` has 29 named scenarios. All 29 step-level assertions pass, including the 5 policy-rejection scenarios now gated individually. Data-value assertions on "Rock"/"AC/DC"/"Angus Young" prove chinook.db is data-bearing. |

**Score:** 4/4 truths verified

---

### Deferred Items

None.

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-server-toolkit/src/config.rs` | Additive superset fields `file_path` / `is_reference` / `[shared_policy_store]` | VERIFIED | `file_path` at line 341, `is_reference` at line 266, `SharedPolicyStoreSection` at line 466. All `#[serde(default)]`. |
| `crates/pmcp-server-toolkit/tests/config_superset.rs` | REF-01 regression gate (all 4 configs + renames_rejected + ${VAR}-verbatim) | VERIFIED | 6 tests, all pass. |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | `build_cm_config` maps `require_limit -> sql_require_limit` live (no _require_limit_gap discard) | VERIFIED | `cfg.sql_require_limit = section.require_limit;` at line 551. `grep _require_limit_gap` returns 0 matches. |
| `crates/pmcp-code-mode/src/validation.rs` | `check_sql_config_authorization` enforces missing-LIMIT rejection for read-only SELECT when `sql_require_limit=true` | VERIFIED | Enforcement at line 1085: `if self.config.sql_require_limit && !info.has_limit { return ... missing_limit ... }`. 5 require_limit unit tests (lines 1666–1730). |
| `crates/pmcp-sql-server/tests/parity_chinook.rs` | Per-step gating (not masked by continue_on_failure) + REQUIRED_REJECTION_SCENARIOS presence guard | VERIFIED | `REQUIRED_REJECTION_SCENARIOS` const at line 105 (5 rejection names). Per-step gate at lines 239–264: `result.step_results.iter().filter(!success)` must be empty. `result.success` is NOT used as the gate. WHY doc-comment at lines 73–110 explains the masking and the DO-NOT-SIMPLIFY warning. |
| `crates/pmcp-sql-server/src/assemble.rs` | `synthesize_instructions_resource` + `synthesize_policies_resource` + `merged_resource_configs` | VERIFIED | `synthesize_instructions_resource` at line 171; `synthesize_policies_resource` at line 208; `merged_resource_configs` at line 315; `build_resource_handler` calls `merged_resource_configs` at line 344. `prompt_body_carries_synthesized_instructions_and_policies` async test at line 596 asserts the complete served body. |
| `crates/pmcp-sql-server/tests/fixtures/generated.yaml` | 29-scenario parity contract including 5 rejection scenarios | VERIFIED | `grep -c "^- name:"` = 29. "Validate: SELECT without LIMIT should be rejected" at line 251. |
| `crates/pmcp-sql-server/src/lib.rs` | `RunError::Serving(JoinError)` propagated from `run()` | VERIFIED | Plan 85-10 Task 2 commit `25c58962`. Non-zero exit on serve-task panic. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `CodeModeSection.require_limit` | `CodeModeConfig.sql_require_limit` | `build_cm_config` in `code_mode.rs:551` | WIRED | Single line `cfg.sql_require_limit = section.require_limit;` — no discard. |
| `CodeModeConfig.sql_require_limit` | `missing_limit` PolicyViolation on bare SELECT | `check_sql_config_authorization` Select arm at `validation.rs:1085` | WIRED | `if self.config.sql_require_limit && !info.has_limit { return ValidationResult::failure(...) }` |
| `parity_chinook.rs` per-step gate | every StepResult including `continue_on_failure` steps | `result.step_results.iter().filter(!s.success)` | WIRED | The per-step gate fires independently of the aggregate `result.success`. |
| `merged_resource_configs` | `code-mode://instructions` + `code-mode://policies` synthesized resources | `synthesize_instructions_resource` + `synthesize_policies_resource` in `assemble.rs` | WIRED | Appended to the merged set only when not already declared by config (operator override wins, T-85-09-03). |
| `build_resource_handler` | `StaticResourceHandler` used by both prompt resolution and server resource surface | `merged_resource_configs(cfg, schema_ddl)` | WIRED | `build_resource_handler` at line 340 calls `merged_resource_configs`; both `register_prompts` and `builder.resources_arc` receive this handler. |
| `lib::run_serving` | `StreamableHttpServer::with_config` | parse → dispatch → build_server → serve pipeline | WIRED | Full pipeline unchanged from initial verification. |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `parity_chinook.rs` / curated tools | Tool call results containing "Rock", "AC/DC", "For Those About To Rock..." | `tests/fixtures/chinook.db` (984 KB, data-bearing) | Yes — data-value assertions pass | FLOWING |
| `assemble.rs merge_schema_resource` | `docs://chinook/schema` resource content | `--schema` file (chinook.ddl, 11 CREATE TABLE statements) | Yes — DDL text replaces configured content | FLOWING |
| `synthesize_instructions_resource` | `code-mode://instructions` body | `cfg.database.backend_type` → `dialect_label()` + template | Yes — dialect is "SQLite" (from config), instructions text is deterministic | FLOWING |
| `synthesize_policies_resource` | `code-mode://policies` body | `cfg.code_mode` fields (non-secret only) | Yes — policy fields (require_limit: true, max_limit: 1000, sensitive_columns: Email/Phone/Password etc.) | FLOWING |
| `SqlCodeExecutor::execute` | `execute_code` result | `connector.execute(code, &params)` with variables bound (85-10 WR-02) | Yes — valid queries return rows; invalid-token scenario asserts failure | FLOWING |

---

### Behavioral Spot-Checks (Re-Verification)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 29 parity scenarios pass, rejection scenarios individually gating | `CODE_MODE_SECRET=parity-chinook-code-mode-secret-32b cargo test -p pmcp-sql-server --no-default-features --features sqlite --test parity_chinook -- --test-threads=1` | `1 passed (1 suite, 0.66s)` | PASS |
| Full sql-server test suite (39 tests, 9 suites) | `CODE_MODE_SECRET=... cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1` | `39 passed (9 suites, 2.30s)` | PASS |
| pmcp-code-mode with require_limit tests | `cargo test -p pmcp-code-mode --features sql-code-mode -- --test-threads=1` | `133 passed, 5 ignored (4 suites, 2.04s)` | PASS |
| pmcp-server-toolkit with code-mode + sqlite | `cargo test -p pmcp-server-toolkit --features "code-mode sqlite" -- --test-threads=1` | `201 passed (11 suites, 11.91s)` | PASS |
| No _require_limit_gap discard remains | `grep _require_limit_gap crates/pmcp-server-toolkit/src/code_mode.rs` | 0 matches | PASS |
| Live mapping confirmed | `grep "sql_require_limit = section.require_limit" crates/pmcp-server-toolkit/src/code_mode.rs` | 1 match at line 551 | PASS |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SHAP-A-01 | Plans 02, 04, 05, 06, 07, 08, 09, 10 | Shape A pure-config binary, zero Rust | SATISFIED | Binary crate `run_serving` wires config+connector. 39 tests pass (incl. parity_chinook). All three SC-3 gaps closed. |
| REF-01 | Plans 01, 05 | config.toml schema is superset of pmcp-run sql-api server configs; renames rejected | SATISFIED | `config_superset.rs` + `superset_parse.rs` cover all 4 reference configs. `renames_rejected` green. 201 toolkit tests pass. |
| REF-02 | Plans 03, 06, 09 | At least one reference server reproduced end-to-end, result parity via scenario replay | SATISFIED | 29 scenarios replayed through real binary path. Per-step gate confirms all 5 rejection scenarios are individually gating. |

---

### Anti-Patterns Found

The pre-existing warnings (WR-01, WR-02) from 85-REVIEW.md that were outstanding after initial verification are now fully resolved by gap-closure plans:

- **WR-01 (require_limit unenforced):** RESOLVED by Plan 85-07. `_require_limit_gap` discard removed; `missing_limit` rejection in `validation.rs:1085`.
- **WR-02 (execute_code variables silently dropped):** RESOLVED by Plan 85-10. `variables_to_params()` binds named params; `None`/non-object → empty slice (parity scenario unaffected).

No new anti-patterns introduced by the gap-closure plans. The pre-existing `clippy::field_reassign_with_default` lint in `build_cm_config` (rust-1.95.0 toolchain mismatch) remains deferred per `deferred-items.md` — it pre-dates this phase and is explicitly excluded from its scope.

---

### Human Verification Required

None. All four success criteria are verifiable programmatically and the parity test passes empirically with per-step gating.

---

## Gaps Summary

**Status: passed (4/4).** All three SC-3 gaps reopened after code review are now closed and empirically confirmed.

### Gap 1 — SC-3: `require_limit` policy unenforced — CLOSED (Plan 85-07, commits 74871aae + 05c3f55e)

`build_cm_config` previously discarded `require_limit` into `let _require_limit_gap = section.require_limit;`. Fixed: `cfg.sql_require_limit = section.require_limit;` at `code_mode.rs:551`. Enforcement added to the `Select` arm of `check_sql_config_authorization` (`validation.rs:1085`): a bare SELECT (no LIMIT clause) under `require_limit=true` returns a `missing_limit` `PolicyViolation`. 5 unit tests cover: rejects bare SELECT, accepts LIMITed SELECT, default (false) no regression, write unaffected, serde round-trip. Empirical: `validate_code("SELECT * FROM Artist")` under `require_limit=true` is now rejected with `missing_limit`.

### Gap 2 — SC-3/SC-4: parity test masked failed rejection scenarios — CLOSED (Plan 85-08, commit eec2941a)

The old `assert!(result.success)` excluded `continue_on_failure` steps, so the no-LIMIT failure assertion was silently dropped. Fixed in `parity_chinook.rs`: the test now gates per-step (`result.step_results.iter().filter(!s.success)` must be empty) plus a `REQUIRED_REJECTION_SCENARIOS` presence guard (5 named scenarios that MUST appear in the suite). The two-sided regression proof (documented in 85-08-SUMMARY.md) shows: with Gap 1 reverted, `result.success` still reads PASSED but the per-step gate catches the `Validate: SELECT without LIMIT should be rejected` step failure and fails the test.

### Gap 3 — SC-1/parity: assembled `start_code_mode` prompt dropped policy + instructions content — CLOSED (Plan 85-09, commit 0f8b0f58)

`StaticPromptHandler::resolve_body` warn-logged and skipped `code-mode://instructions` and `code-mode://policies` because they were not declared as `[[resources]]`. Fixed: `synthesize_instructions_resource` and `synthesize_policies_resource` added to `assemble.rs`; `merged_resource_configs` appends them (dedup-by-URI, operator override wins). `build_resource_handler` now uses `merged_resource_configs`. Async test `prompt_body_carries_synthesized_instructions_and_policies` asserts the REAL served body (via `PromptHandler::handle`) contains: instructions marker, "SQLite" dialect, `require_limit`/`max_limit`/`allow_writes`/`Email` policy fields, and NO `CODE_MODE_SECRET` or `token_secret` leak.

### Secondary robustness fixes (Plan 85-10, commits d962051e + 25c58962) — no parity regression

Six lower-severity issues closed: `execute_code` variables now bound as named params (not silently dropped), `SqlCodeExecutor` caches its pipeline at construction (token_secret resolved once), explicit JSON `null` applies declared default (no `LIMIT NULL`), set-but-empty `token_secret`/`AWS_REGION` treated as unset, serve-task `JoinError` propagated as `RunError::Serving` (non-zero exit), `database = ":memory:"` SQLite form accepted. Full sql-server suite: 39 passed (was 30 before secondary fixes; 9 new tests added).

---

_Initial Verified: 2026-05-27T06:00:00Z_
_SC-3 Reopened: 2026-05-27T07:30:00Z (code review disproved initial SC-3 PASS)_
_Gap Closure Plans: 85-07, 85-08, 85-09, 85-10_
_Re-Verified: 2026-05-27T10:00:00Z_
_Verifier: Claude (gsd-verifier)_
