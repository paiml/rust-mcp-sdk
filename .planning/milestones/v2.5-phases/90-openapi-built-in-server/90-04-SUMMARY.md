---
phase: 90-openapi-built-in-server
plan: 04
subsystem: api
tags: [openapi, code-mode, http-executor, oauth-passthrough, js-validation, generalization, wiremock]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 01
    provides: "http::join_url shared helper; http::auth::HttpAuthProvider::apply(inbound_token) + create_auth_provider/create_passthrough_auth_provider; openapi-code-mode umbrella feature (forwards pmcp-code-mode/js-runtime)"
  - phase: 90-openapi-built-in-server
    plan: 02
    provides: "[backend]/[backend.auth] config + AuthConfig re-export"
  - phase: 83-toolkit-core-lift
    provides: "code_mode_tools_from_executor + SqlCodeExecutor + ValidationPipeline wiring; builder_ext try_code_mode_from_config_with_connector (LOCKED ext method)"
provides:
  - "code_mode::HttpCodeExecutor (impl pmcp_code_mode::HttpExecutor) — the OpenAPI backend seam for Code Mode + script tools, gated openapi-code-mode (H2)"
  - "HttpCodeExecutor carries a per-request inbound MCP token (with_inbound_token) for oauth_passthrough (H1)"
  - "code_mode::ValidationFlavor { Sql, OpenApi } enum (compile-time, not stringly-typed)"
  - "backend-agnostic code_mode_tools_from_executor(Arc<dyn CodeExecutor>, ValidationFlavor) — ONE wiring fn for SQL + OpenAPI"
  - "OpenAPI validate_code really runs SWC-backed JS validation (validate_javascript_code)"
  - "code_mode re-exports ExecutionConfig/HttpExecutor/JsCodeExecutor (openapi-code-mode)"
affects: [90-05-script-tools, 90-06-binary-dispatch]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Low-level HttpExecutor seam (execute_request) wrapped by JsCodeExecutor for Code Mode + called directly by script tools — DIFFERENT layer from the high-level CodeExecutor (SqlCodeExecutor's layer)"
    - "Per-request clone-with-token builder (with_inbound_token) so one shared executor instance threads the captured MCP token per request (H1)"
    - "ValidationFlavor enum selects BOTH the CodeModeToolBuilder format string AND the ValidationPipeline method (validate_sql_query vs validate_javascript_code) — feature-gated OpenApi arm"
    - "Type-erased Arc<dyn CodeExecutor> + flavor param keeps ONE wiring fn + ONE execute_code handler body for every backend"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/http_executor.rs
    - crates/pmcp-server-toolkit/tests/code_mode_openapi.rs
  modified:
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/src/builder_ext.rs

key-decisions:
  - "HttpCodeExecutor query params appended via url::Url::query_pairs_mut (reqwest 0.13 gates RequestBuilder::query behind an off-by-default `query` feature — same Plan 01 Rule 1 reconciliation)"
  - "ExecutionError redaction: auth/transport/status/parse failures map to RuntimeError naming operation/status only — NEVER the URL or token (Pitfall 5 / T-90-04-01)"
  - "OpenApi validation arm is feature-gated on openapi-code-mode (validate_javascript_code lives behind pmcp-code-mode/openapi-code-mode, transitively enabled by js-runtime); the not(openapi-code-mode) arm returns a typed error so the enum still compiles under bare code-mode"
  - "assemble.rs (pmcp-sql-server) left UNTOUCHED — the LOCKED ext method signature absorbs the flavor (passes ValidationFlavor::Sql internally), so the SQL binary stays byte-stable; non-regression proven by cargo test -p pmcp-sql-server"
  - "SQL-flavor test in code_mode_openapi.rs is #[cfg(feature=sqlite)]-gated (needs SqliteConnector); the plan verify (openapi-code-mode only) still exercises the openapi flavor + real-behavior tests, and the existing tests/code_mode_tools.rs (code-mode+sqlite) covers SQL wiring through the generalized fn"

patterns-established:
  - "Pattern: integration test file + fn-prefix named for the verify filter (tests/http_executor.rs with http_executor_-prefixed fns; tests/code_mode_openapi.rs with code_mode_/openapi_-prefixed fns) so the positional cargo filter resolves (Plan 01 verify-filter lesson)"

requirements-completed: [OAPI-05, OAPI-10]

# Metrics
duration: 18min
completed: 2026-05-29
---

# Phase 90 Plan 04: Code-Mode HTTP Executor + Backend-Agnostic Wiring Summary

**Lifted the `HttpCodeExecutor` seam (impl `pmcp_code_mode::HttpExecutor`, OAPI-05) carrying a per-request inbound MCP token for `oauth_passthrough` (H1) and gated under `openapi-code-mode` (H2), then performed the phase's one real refactor: generalized `code_mode_tools_from_executor` from `Arc<SqlCodeExecutor>` + a hardcoded `"sql"` flavor to `Arc<dyn CodeExecutor>` + a `ValidationFlavor` enum (OAPI-10 / D-02) so SQL and OpenAPI share ONE wiring function and the OpenAPI `validate_code` really runs JS validation — with the SQL path staying green end-to-end.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-05-29T18:48:53Z
- **Completed:** 2026-05-29T19:07Z
- **Tasks:** 2
- **Files modified:** 4 (2 created, 2 modified)

## Accomplishments

- **`HttpCodeExecutor`** (`#[cfg(feature = "openapi-code-mode")]`) implements the low-level `pmcp_code_mode::HttpExecutor` (`execute_request(method, path, body)`): `{placeholder}` path substitution from body keys, base+path concat via the shared `crate::http::join_url` (Pitfall 2 — preserves an API-Gateway stage prefix), auth via `HttpAuthProvider::apply` threading the per-request inbound token, GET-like body→query / else→JSON body, and a redacted `ExecutionError` (OAPI-05).
- **Per-request passthrough (H1):** `with_inbound_token(Option<String>)` is the cheap clone-with-token builder the binary calls per request so an `OAuthPassthroughAuth` provider forwards the captured MCP client token (`Authorization: Bearer client-tok`) while static providers ignore it. Proven by a wiremock header matcher.
- **H2 feature gate:** `HttpCodeExecutor` + impl + tests are gated `openapi-code-mode` (the Plan 01 umbrella that forwards `pmcp-code-mode/js-runtime`), NOT the `all(http, code-mode)` gate that would omit `HttpExecutor` (the bug the plan-checker fixed for 01/05/06).
- **`ValidationFlavor { Sql, OpenApi }`** enum (compile-time, not `&str`) selects both the `CodeModeToolBuilder` format string and the `ValidationPipeline` method.
- **Backend-agnostic `code_mode_tools_from_executor`:** `executor: Arc<dyn CodeExecutor>` + `flavor: ValidationFlavor`; `ExecuteCodeHandler.executor` widened to `Arc<dyn CodeExecutor>` (the `handle` body unchanged — already dispatched through the trait). Validation parameterized: `Sql → validate_sql_query`, `OpenApi → validate_javascript_code` (OAPI-10 / D-02).
- **OpenAPI `validate_code` really runs:** a valid read-only JS script passes; `eval(...)` is rejected; an unbounded `while` loop is rejected — real SWC-backed JS validation, not a stub (Codex HIGH).
- **SQL non-regression:** the toolkit `builder_ext` bridge coerces `SqlCodeExecutor` to `Arc<dyn CodeExecutor>` + passes `ValidationFlavor::Sql`; `assemble.rs` / `pmcp-sql-server` untouched. `cargo test -p pmcp-sql-server` (exit 0) + toolkit SQL-only `code-mode` tests (exit 0) gate the refactor.
- Full `--features openapi-code-mode` toolkit suite green (exit 0, 0 FAILED).

## Task Commits

1. **Task 1: HttpCodeExecutor — impl HttpExecutor + per-request passthrough token (OAPI-05/H1/H2)** - `b25d1b39` (feat)
2. **Task 2: generalize code_mode_tools_from_executor to Arc<dyn CodeExecutor> + ValidationFlavor (OAPI-10)** - `99d39c5d` (feat)

_TDD note: this plan's frontmatter is `type: execute`; each task carried `tdd="true"`. Tests and implementation were committed together per task because every assertion targets net-new types/behavior (the `HttpExecutor` impl, the `ValidationFlavor` enum, the OpenApi validation arm) with no prior passing behaviour to protect. All `<behavior>` items have passing tests._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/code_mode.rs` — `HttpCodeExecutor` (struct + `HttpExecutor` impl + `new`/`with_inbound_token`/`resolve_path`/`scalar_str`); `ValidationFlavor` enum + `code_format`; widened `code_mode_tools_from_executor` signature; widened `ExecuteCodeHandler.executor` + both handlers carry `flavor`; `run_flavored_validation` helper (feature-gated OpenApi arm); openapi-code-mode re-exports of `ExecutionConfig`/`HttpExecutor`/`JsCodeExecutor`.
- `crates/pmcp-server-toolkit/src/builder_ext.rs` — `try_code_mode_from_config_with_connector` coerces the SQL executor to `Arc<dyn CodeExecutor>` and passes `ValidationFlavor::Sql` (ext-method signature unchanged → assemble.rs untouched).
- `crates/pmcp-server-toolkit/tests/http_executor.rs` — wiremock GET path-subst, POST+static bearer, per-request passthrough header (H1), redacted connect failure, Clone assertion.
- `crates/pmcp-server-toolkit/tests/code_mode_openapi.rs` — SQL-flavor still-works (sqlite-gated), OpenApi flavor registers with the `openapi` format enum, and 3 OpenAPI `validate_code` real-behavior tests.

## Decisions Made

- **`url::Url` query append (Plan 01 Rule 1 again):** reqwest 0.13's `RequestBuilder::query` is gated behind the `query` feature the toolkit deliberately leaves off, so query params (auth + GET-like body fields) are appended via `url::Url::query_pairs_mut`.
- **assemble.rs untouched:** the plan listed it in `files_modified` "to guarantee the non-regression test compiles," but the LOCKED ext-method signature was unchanged (flavor absorbed internally), so the SQL binary stays byte-stable — the cleaner outcome the plan preferred. `cargo test -p pmcp-sql-server` is the OAPI-10 non-regression gate.
- **Feature-gated OpenApi validation arm:** `validate_javascript_code` lives behind `pmcp-code-mode/openapi-code-mode`; the `ValidationFlavor` enum is available under bare `code-mode`, so the `OpenApi` match arm is `#[cfg(feature = "openapi-code-mode")]` with a typed-error fallback under `not(...)` — the enum + wiring fn compile under SQL-only builds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] reqwest 0.13 gates `RequestBuilder::query` behind a `query` feature**
- **Found during:** Task 1 (HttpCodeExecutor compilation)
- **Issue:** The lifted reference body calls `request.query(&query_params)`, but reqwest 0.13 puts `RequestBuilder::query` behind the off-by-default `query` feature the toolkit does not enable (the identical Plan 01 finding). It failed to compile.
- **Fix:** Append query params to the URL via `url::Url::query_pairs_mut().append_pair(...)` (the Plan 01 reconciliation). `url` is in scope under the `http` feature that `openapi-code-mode` enables.
- **Files modified:** crates/pmcp-server-toolkit/src/code_mode.rs
- **Verification:** all 5 `http_executor` wiremock tests green (GET path-subst, POST+auth, passthrough header, redaction).
- **Committed in:** `b25d1b39` (Task 1 commit)

**2. [Rule 3 - Blocking] doc-prose tripped the `Url::join` / `CodeModeToolBuilder::new("sql")` negative greps**
- **Found during:** Task 1 + Task 2 (acceptance grep checks)
- **Issue:** Explanatory doc-comments contained the literal `Url::join` (Task 1) and `CodeModeToolBuilder::new("sql")` (Task 2 `ValidationFlavor` docs), which the acceptance criteria require at count 0 (the same prose-vs-grep collision Plan 01 hit).
- **Fix:** Reworded the prose to describe the behavior without the literal tokens, preserving the explanatory intent.
- **Files modified:** crates/pmcp-server-toolkit/src/code_mode.rs
- **Verification:** `grep -c "Url::join"` → 0; `grep -c 'CodeModeToolBuilder::new("sql")'` → 0.
- **Committed in:** `b25d1b39` / `99d39c5d`

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking). No scope creep — public surface matches the plan's `artifacts` + `must_haves`.

## Issues Encountered

- The `rtk` test-proxy strips the per-test harness lines from `cargo test` output, so individual test PASS lines are not visible; verification relied on the authoritative cargo exit code (0) and the aggregate pass counts.
- The plan's verify filter `code_mode` (substring) resolves against test fn names; the new openapi tests are named with `code_mode_`/`openapi_` prefixes so they're picked up. The SQL-flavor test in the new file is `sqlite`-gated, so the bare `--features openapi-code-mode` verify run does not execute it — SQL wiring through the generalized fn is additionally covered by `tests/code_mode_tools.rs` and `cargo test -p pmcp-sql-server`.

## TDD Gate Compliance

`type: execute` plan with `tdd="true"` tasks; tests + implementation committed together per task (net-new types/behavior with no prior passing state to protect). All `<behavior>` items in both tasks have passing tests.

## Known Stubs

None. The `HttpCodeExecutor` is fully wired through reqwest + auth; the OpenApi `validate_code` path really runs JS validation. The script-tool consumer of `HttpCodeExecutor` is Plan 05 (an anticipated, documented seam, not a stub in this plan's scope).

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. T-90-04-01 (redaction) is covered by `http_executor_connect_failure_is_redacted`; T-90-04-02 (LLM JS) by the real-behavior OpenApi validate tests (the validate→token→execute gate is generalized, NOT removed); T-90-04-04 (SQL regression) by the sql-server + SQL-only toolkit gates; T-90-04-05 (passthrough misrouting) by Plan 01's static-ignores-inbound proof reused here.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 05 (script tools)** wires `HttpCodeExecutor` (directly, not via `JsCodeExecutor`) into a `ScriptToolHandler`, and may reuse `with_inbound_token` for per-request passthrough; the `ValidationFlavor::OpenApi` path is the validation surface.
- **Plan 06 (binary dispatch)** constructs `HttpCodeExecutor::new(...)`, wraps it in `JsCodeExecutor`, and calls `code_mode_tools_from_executor(builder, cfg, Arc::new(js_exec), ValidationFlavor::OpenApi)`; per request it captures the inbound MCP token (`TokenCaptureAuthProvider`) into `AuthContext` and threads it via `with_inbound_token`.
- No blockers.

## Self-Check: PASSED

- All 4 task files present on disk (2 created, 2 modified).
- Both task commits present in git history: `b25d1b39`, `99d39c5d`.
- Acceptance greps: `impl pmcp_code_mode::HttpExecutor for HttpCodeExecutor` (1); wrong `all(http,code-mode)` gate (0); `Url::join` (0); `executor: Arc<dyn ..CodeExecutor>` (2); `enum ValidationFlavor` (1); `CodeModeToolBuilder::new("sql")` (0); `Arc<SqlCodeExecutor>` in wiring (0).
- `cargo test -p pmcp-server-toolkit --features openapi-code-mode` exit 0 (0 FAILED); `cargo test -p pmcp-sql-server` exit 0; toolkit SQL-only `code-mode` exit 0; clippy clean on lib + tests.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
