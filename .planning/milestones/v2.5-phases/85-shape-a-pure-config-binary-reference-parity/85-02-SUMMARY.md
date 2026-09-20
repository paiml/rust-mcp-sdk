---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 02
subsystem: api
tags: [code-mode, sql, validate_code, execute_code, hmac, sqlite, security]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift
    provides: validation_pipeline_from_config, build_cm_config, resolve_token_secret, register_code_mode_tools stub, code_mode re-exports
  - phase: 84-sql-connectors
    provides: 3-method SqlConnector trait (execute), SqliteConnector, Dialect
  - phase: 85-01
    provides: ${VAR} token_secret expansion in resolve_token_secret
provides:
  - SqlCodeExecutor adapter (CodeExecutor over the single-method SqlConnector with defense-in-depth re-validation)
  - LOCKED try_code_mode_from_config_with_connector (registers validate_code + execute_code)
  - code_mode_tools_from_executor (builder, config, Arc<SqlCodeExecutor>) two-tool registration helper
  - assemble_code_mode_prompt_with_schema (sync, file-based prompt seam, no live introspection)
affects: [85-04, 85-05, 85-06, 86-shapes-bcd]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Hand-built validate_code/execute_code ToolHandlers mirror #[derive(CodeMode)] without taking a proc-macro dependency"
    - "Single-method SqlConnector::execute collapses production's 2-method execute_query/execute_statement dispatch"
    - "Static [code_mode] policy via NoopPolicyEvaluator — config flags (allow_writes/deletes/ddl) ARE the authorization (D-13)"
    - "Sync file-based prompt seam (Dialect + &str) cannot trigger a live connector.schema_text() round-trip"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/code_mode_tools.rs
  modified:
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/src/builder_ext.rs
    - crates/pmcp-server-toolkit/src/lib.rs

key-decisions:
  - "Hand-built the two ToolHandlers in code_mode.rs rather than taking a pmcp-code-mode-derive dependency — only the PUBLIC API is locked, the registration mechanism is implementer's discretion (Plan 85-02 Task 2)"
  - "Repurposed the unused Phase 83 code_mode_tools_from_executor(Box<dyn CodeExecutor>, config) stub to the LOCKED (builder, config, Arc<SqlCodeExecutor>) signature — Rule 3, no external callers existed"
  - "execute_code success payload uses {\"rows\": <values>} mirroring production's observable shape; the single-method trait has no columns/rows_affected channel (REVIEW FIX Codex #6b)"
  - "validate_code uses a fixed config-derived ValidationContext (no live user/session in the pure-config binary); static policy enforcement happens inside validate_sql_query regardless of context"

patterns-established:
  - "SqlCodeExecutor re-validates SQL against [code_mode] policy BEFORE the connector (defense-in-depth, threat T-85-02-01)"
  - "# Database Schema header kept identical across prompt seam and Plan 05 resource surface for parity"

requirements-completed: [SHAP-A-01]

# Metrics
duration: 18min
completed: 2026-05-27
---

# Phase 85 Plan 02: Code-Mode Tool Registration + File-Based Prompt Seam Summary

**The LOCKED `try_code_mode_from_config_with_connector` now registers real `validate_code` + `execute_code` tools backed by an `SqlCodeExecutor` (defense-in-depth re-validation over the single-method `SqlConnector`), with static `[code_mode]` policy rejecting DELETE/DDL on a read-only config, plus a sync `--schema` file-based prompt seam that never touches a live connector.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-05-27T00:53:00Z (approx)
- **Completed:** 2026-05-27T01:11:00Z
- **Tasks:** 3
- **Files modified:** 4 (3 modified, 1 created)

## Accomplishments

- Closed RESEARCH Gap #2: `register_code_mode_tools` registered ZERO tools (Plan 08 was deferred). The new `code_mode_tools_from_executor` + `try_code_mode_from_config_with_connector` register BOTH `validate_code` and `execute_code` — the SC-3 / SHAP-A-01 security-parity surface now EXISTS.
- `SqlCodeExecutor` bridges the toolkit's single-method `SqlConnector::execute` to the code-mode flow, re-validating against `[code_mode]` policy BEFORE the connector is reached (collapses production's 2-method `execute_query`/`execute_statement` split).
- Static `[code_mode]` policy is enforced statically via `NoopPolicyEvaluator` + the validation pipeline — DELETE/DDL on a read-only config are rejected (no approval token issued), proving a config-driven server cannot bypass the write/DDL guards.
- Added `assemble_code_mode_prompt_with_schema` — a sync, connectorless prompt assembler so the `--schema` file becomes the prompt body without a live `schema_text()` round-trip (SC-1 prerequisite, D-04/D-05 redaction guarantee).

## Task Commits

Each task was committed atomically:

1. **Task 1: SqlCodeExecutor adapter over the single-method SqlConnector** - `509d2c5d` (feat)
2. **Task 2: LOCKED try_code_mode_from_config_with_connector (validate+execute)** - `a236e2e8` (feat)
3. **Task 3: assemble_code_mode_prompt_with_schema file-based prompt seam** - `d9bcc69e` (feat)

_TDD note: tasks combined RED+GREEN in a single commit (tests + implementation co-located in the same touched files; tests written first, then implementation, verified before commit)._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/code_mode.rs` - Added `SqlCodeExecutor` (struct + `CodeExecutor` impl + `revalidate` helper), repurposed `code_mode_tools_from_executor` to the locked 3-arg registration signature, hand-built `tool_handlers::{ValidateCodeHandler, ExecuteCodeHandler}`, added `assemble_code_mode_prompt_with_schema`, re-exported `ExecutionError`. 7 new unit tests (4 executor + 3 prompt).
- `crates/pmcp-server-toolkit/src/builder_ext.rs` - Added LOCKED `try_code_mode_from_config_with_connector` to the `ServerBuilderExt` trait + impl; documented `try_code_mode_from_config` as the connectorless validation-only path; dropped a needless `return`.
- `crates/pmcp-server-toolkit/tests/code_mode_tools.rs` - NEW: 6 integration tests (both tools registered; no-op when absent; connectorless registers neither; DELETE/DDL rejected; SELECT permitted).
- `crates/pmcp-server-toolkit/src/lib.rs` - Re-exported `assemble_code_mode_prompt_with_schema` from the crate root.

## Decisions Made

- **Hand-built ToolHandlers (no proc-macro dep):** The plan permits either `#[derive(CodeMode)]` or hand-built `ToolInfo`s. Chose hand-built handlers in `code_mode.rs` mirroring the derive macro's generated handler bodies, avoiding a `pmcp-code-mode-derive` dependency on the toolkit. Only the PUBLIC API is locked; this mechanism is implementer's discretion per the plan.
- **execute_code result shape (`{"rows": <values>}`):** Verified the production `mcp-sql-server-core::SqlCodeModeHandler::execute` returns `{"columns", "rows", "rows_affected"}` because its 2-method connector surfaces columns + affected counts separately. The toolkit's `SqlConnector::execute` returns `Vec<Value>` (one JSON object per row) with no separate columns/rows_affected channel, so the adapter mirrors production's OBSERVABLE `"rows"` key. The parity replay (Plan 06) only exercises `execute_code` with an INVALID token (asserts `failure`), so this success shape is not asserted by `generated.yaml`.
- **Static ValidationContext for the pure-config binary:** `validate_code` binds tokens to a fixed config-derived context (`code-mode-config` / `code-mode-session`) rather than a live user/session, because the pure-config Shape A binary has no per-request identity surface. Static `[code_mode]` policy (allow_writes/deletes/ddl) is enforced inside `validate_sql_query` regardless of the context value, so SC-3 holds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Repurposed the unused `code_mode_tools_from_executor` stub signature**
- **Found during:** Task 2
- **Issue:** Phase 83 shipped `code_mode_tools_from_executor(executor: Box<dyn CodeExecutor>, config: &ServerConfig) -> Result<Box<dyn CodeExecutor>>` as a shape-preserving stub. The plan's LOCKED API requires `code_mode_tools_from_executor(builder, config, executor: Arc<SqlCodeExecutor>) -> Result<builder>` — a different signature on the same name. The old stub had ZERO external callers (verified via grep across `crates/` + `src/`).
- **Fix:** Replaced the stub with the locked 3-arg registration signature that actually registers the two tools.
- **Files modified:** crates/pmcp-server-toolkit/src/code_mode.rs
- **Verification:** Full toolkit test suite (187 tests) green; no broken callers.
- **Committed in:** a236e2e8 (Task 2 commit)

**2. [Rule 1 - Bug] Dropped a needless `return` in `try_code_mode_from_config`**
- **Found during:** Task 2 (touching builder_ext.rs)
- **Issue:** A pre-existing `return crate::code_mode::register_code_mode_tools(self, config);` inside a cfg block triggered a `clippy::needless_return` warning surfaced while editing the file.
- **Fix:** Removed the `return` (expression-position call). Scoped to a file I was already modifying for the new method.
- **Files modified:** crates/pmcp-server-toolkit/src/builder_ext.rs
- **Verification:** clippy clean on the touched method.
- **Committed in:** a236e2e8 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking signature change, 1 bug/lint).
**Impact on plan:** The signature repurpose was required to ship the LOCKED API on the planned name; no scope creep. The lint fix is incidental to a file already being edited.

## Issues Encountered

- The `pmcp::ToolHandler` trait the builder uses (`async fn handle(args, extra)` + `fn metadata()`, from `src/server/mod.rs`) is distinct from the `traits.rs` `ToolHandler` (`call_tool`). The derive macro and `tool_arc`/`get_tool` all use the `mod.rs` one — the hand-built handlers target that trait. Confirmed via grep before implementing.

## Known Stubs

None. The fixed config-derived `ValidationContext` in `validate_code` is an intentional design choice for the pure-config binary (documented inline), not a stub — static `[code_mode]` policy enforcement is fully functional regardless of context value.

## Deferred Issues

- Pre-existing `clippy::field_reassign_with_default` warning in `build_cm_config` (code_mode.rs:460-461) is from the rust-1.95.0-vs-CI pedantic toolchain drift logged in STATE.md / deferred-items.md. NOT in scope for this plan; left untouched.

## Threat Flags

None — no new network endpoints, auth paths, or trust-boundary surface beyond the plan's `<threat_model>` (T-85-02-01..05 all addressed: defense-in-depth re-validation, static policy enforcement, HMAC token binding via the pipeline, sanitized connector errors, file-based prompt seam).

## Next Phase Readiness

- Plan 85-04 (Wave 2) can call `try_code_mode_from_config_with_connector` explicitly when filling `lib::run()` — no A/B decision deferred (Codex HIGH #4 resolved).
- Plan 85-05 can use `assemble_code_mode_prompt_with_schema` for the `--schema`-driven prompt + reuse the identical `# Database Schema` header for the resource surface.
- Plan 85-06 parity replay: `validate_code` + `execute_code` tools now exist for the 8 validate scenarios + 1 execute scenario.

## Self-Check: PASSED

- Created files verified on disk: `crates/pmcp-server-toolkit/tests/code_mode_tools.rs`, `85-02-SUMMARY.md`
- Commits verified in git log: `509d2c5d`, `a236e2e8`, `d9bcc69e`
- Source assertions verified: `struct SqlCodeExecutor`, `fn try_code_mode_from_config_with_connector`, `pub fn assemble_code_mode_prompt_with_schema`
- Full toolkit suite: 187 tests passing (`--features "code-mode sqlite"`)

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*
