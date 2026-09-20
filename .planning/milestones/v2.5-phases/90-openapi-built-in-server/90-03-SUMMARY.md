---
phase: 90-openapi-built-in-server
plan: 03
subsystem: api
tags: [openapi, http, openapiv3, serde_yaml, synthesizer, single-call, tool-synthesis, proptest]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 01
    provides: "http::HttpConnector trait + Operation/Parameter/ParameterLocation request model + join_url + HttpConnectorError; openapiv3/serde_yaml http-gated deps"
  - phase: 90-openapi-built-in-server
    plan: 02
    provides: "ToolDecl path/method/base_url/script fields + is_script_tool() detection rule; ServerConfig.backend section"
  - phase: 83-toolkit-core-lift
    provides: "build_input_schema / build_annotations / apply_widget_meta synth helpers + SynthesizedTool tuple + ToolkitError::Synth"
provides:
  - "http::schema::OpenApiSchema parser (openapiv3 + serde_yaml fallback) — runtime-OPTIONAL (D-03)"
  - "AUTHORITATIVE Operation/Parameter/ParameterLocation in http::schema (re-exported from http::mod) — one stable type home"
  - "Operation.base_url additive field — per-tool base_url override carried, never dropped"
  - "tools::synthesize_from_config_with_http_connector (http feature) — single-call [[tools]] -> ToolInfo + HttpToolHandler"
  - "explicit script-tool seam (typed ToolkitError) for Plan 05 signature widening"
affects: [90-04-code-mode-executor, 90-05-script-tools, 90-06-binary-dispatch]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Authoritative type lives with its producer (Operation in http::schema, the parser) and is re-exported from http::mod so the trait signature path never churns"
    - "Single-call synthesizer mirrors the SQL synthesize_from_config_with_connector shape; reuses the Phase 83 build_input_schema/build_annotations helpers (no new struct-literal ToolInfo)"
    - "Per-tool base_url reflected onto the Operation (additive field) rather than threaded through a wider connector signature"
    - "Plan-anticipated seam: script-tool arm returns a typed error (not a silent skip, not todo!()) so the Plan 05 edit is localized"

key-files:
  created: []
  modified:
    - crates/pmcp-server-toolkit/src/http/schema.rs
    - crates/pmcp-server-toolkit/src/http/mod.rs
    - crates/pmcp-server-toolkit/src/http/client.rs
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/lib.rs
    - crates/pmcp-server-toolkit/tests/http_connector_props.rs

key-decisions:
  - "MOVED the canonical Operation/Parameter/ParameterLocation from http::mod into http::schema (the parser is their producer) and re-exported from http::mod via `pub use schema::{...}` — the type path crate::http::Operation stays stable, never moves again (Codex MEDIUM)"
  - "Added an additive Operation.base_url: Option<String> field so a per-tool [[tools]] base_url is reflected on the synthesized Operation, not dropped — the connector trait signature stayed unchanged (Codex MEDIUM)"
  - "Path params = the `{...}` template segments (always required); every other declared [[tools.parameters]] becomes a query param; POST/PUT/PATCH set has_request_body (reference create_tool_from_config mapping)"
  - "Script-tool arm returns ToolkitError::Synth referencing Plan 05 (explicit seam); missing path-or-method also returns a typed error (T-90-03-04 negative validation)"

patterns-established:
  - "Pattern: a recording mock HttpConnector (records the last Operation) lets the synthesizer's per-tool base_url / path-param / query-param mapping be asserted with no live HTTP backend, in both the lib synth_http tests and the integration proptest"

requirements-completed: [OAPI-04, OAPI-02a]

# Metrics
duration: 9min
completed: 2026-05-29
---

# Phase 90 Plan 03: OpenAPI Spec Parser + Single-Call Synthesizer Summary

**Filled `http/schema.rs` with the runtime-optional `openapiv3`+YAML `OpenApiSchema` parser (making `Operation` authoritative there and re-exporting it from `http::mod`), and added `synthesize_from_config_with_http_connector` — the single-call `[[tools]]` synthesizer that maps a `path`/`method` entry (+ per-tool `base_url`) to a `ToolInfo` + handler over `HttpConnector::execute`, with the `script` arm left as an explicit Plan 05 seam.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-05-29T18:10:42Z
- **Completed:** 2026-05-29T18:19Z
- **Tasks:** 2
- **Files modified:** 6 (0 created, 6 modified)

## Accomplishments

- `http::schema::OpenApiSchema` parses an OpenAPI 3.0/3.1 doc from JSON (then YAML fallback via `serde_yaml`) into an indexed operation set; `operations()` / `operation_for(path, method)` (case-insensitive method) / `spec_text()` accessors; `parse` + `parse_path` (OAPI-04).
- **D-03 runtime-optional:** the parser is never called unless a spec is supplied; documented that the binary threads `Option<OpenApiSchema>` and a curated-only server boots with `None`.
- **`Operation` made authoritative in `http::schema`** (the parser is its producer) and re-exported from `http::mod` (`pub use schema::{OpenApiSchema, Operation, Parameter, ParameterLocation};`) so the `HttpConnector::execute` trait signature and Plans 04/05 reference one stable type path — no second "unify" churn (Codex MEDIUM).
- **`Operation.base_url` additive field** so a per-tool `[[tools]]` `base_url` override is carried on the synthesized operation (never silently dropped) without widening the connector trait signature.
- `tools::synthesize_from_config_with_http_connector(cfg, Arc<dyn HttpConnector>)` (http feature) mirrors the SQL `synthesize_from_config_with_connector` shape: for each single-call `[[tools]]` it builds an `Operation` (path `{...}` segments → path params, remaining declared params → query params, POST/PUT/PATCH → request body, per-tool `base_url` reflected), a `ToolInfo` via the existing `build_input_schema`/`build_annotations` helpers, and an `HttpToolHandler` that calls `HttpConnector::execute` and returns the JSON (OAPI-02a).
- **Explicit Plan 05 seam:** a `script` tool returns a typed `ToolkitError::Synth` referencing Plan 05 (not a silent skip, not `todo!()`); a `[[tools]]` missing `path` or `method` and without `script` is also rejected with a typed error (T-90-03-04 negative validation).
- Re-exported `synthesize_from_config_with_http_connector` at the crate root (gated `http`), mirroring the SQL re-export.
- 231 tests green under `--features http` (incl. 6 `schema_parse`, 6 `synth_http`, and the path-param-substitution proptest); default-features (no-http) build unaffected; clippy clean on lib + tests.

## Task Commits

1. **Task 1: OpenApiSchema parser (JSON+YAML) + authoritative Operation type** - `9b0601b6` (feat)
2. **Task 2: single-call HTTP tool synthesizer + handler (OAPI-02a)** - `e085360f` (feat)

_TDD note: this plan's frontmatter is `type: execute`; each task carried `tdd="true"`. Tests and implementation were committed together per task because every assertion targets net-new functions/types (the parser, the synthesizer, the additive `Operation.base_url` field) with no prior passing behaviour to protect. All `<behavior>` items have passing tests._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/http/schema.rs` - filled the Plan 01 stub: `OpenApiSchema` parser + the authoritative `Operation`/`Parameter`/`ParameterLocation` types (moved here from mod.rs) + 6 `schema_parse` tests (JSON, YAML, spec-text retention, case-insensitive lookup, malformed-no-panic, redaction discipline).
- `crates/pmcp-server-toolkit/src/http/mod.rs` - removed the type definitions; `pub use schema::{OpenApiSchema, Operation, Parameter, ParameterLocation};`; dropped the now-unused `serde` import and the relocated `test_operation_path_parameters`.
- `crates/pmcp-server-toolkit/src/http/client.rs` - added `base_url: None` to the two test `Operation` literals (additive field).
- `crates/pmcp-server-toolkit/src/tools.rs` - `synthesize_from_config_with_http_connector` + `build_operation` + `HttpToolHandler` (http feature) + the `synth_http_tests` module (single-call/POST/path+query/per-tool base_url/missing-method/script-seam).
- `crates/pmcp-server-toolkit/src/lib.rs` - `#[cfg(feature="http")] pub use crate::tools::synthesize_from_config_with_http_connector;`.
- `crates/pmcp-server-toolkit/tests/http_connector_props.rs` - extended with the `only_declared_path_segments_become_path_params` proptest (a recording mock connector asserts only `{...}` template segments become path params; undeclared/query keys never reach the path).

## Decisions Made

- **Operation home = http::schema (its producer), re-exported from http::mod.** Plan 01 placed a minimal `Operation` in mod.rs so the trait signature could name it; this plan makes the parser-side definition canonical and re-exports it, so the type never moves again (Codex MEDIUM). All Plan 01 call sites (client.rs) compile against the re-export.
- **Per-tool base_url via an additive `Operation.base_url` field**, not a wider connector signature — the synthesizer reflects `decl.base_url` onto the `Operation`. The Plan 01 connector currently targets its constructed `base_url`; honoring the per-operation override at the connector layer (if needed) is a localized future edit, but the acceptance contract (base_url reflected, not dropped) is satisfied at the synthesis layer.
- **Param-location mapping mirrors the reference `create_tool_from_config`:** `{...}` path-template segments → path params (always required); all other declared `[[tools.parameters]]` → query params; POST/PUT/PATCH set `has_request_body`.
- **Script arm is an explicit typed-error seam** (references Plan 05), so Plan 05's signature widening (adding `http_exec`/`exec_config` + `ScriptToolHandler`) is a localized, anticipated edit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test mock `execute` return type collided with the crate `Result` alias; `expect_err` required `Debug`**
- **Found during:** Task 2 (synth_http test compilation)
- **Issue:** The crate re-exports a 1-generic `Result<T>` alias into scope, so the mock connector's `async fn execute(...) -> Result<Value, HttpConnectorError>` was parsed as the alias (E0107) and produced an incompatible trait-method type (E0053). Separately, `Result::expect_err` requires the `Ok` type (`Vec<SynthesizedTool>`, containing `Arc<dyn ToolHandler>`) to be `Debug`, which it is not (E0277).
- **Fix:** Wrote the mock's `execute` return as the fully-qualified `std::result::Result<Value, HttpConnectorError>`; replaced the two `.expect_err(...)` calls with `.err().expect(...)` (which does not require `Debug` on the `Ok` variant).
- **Files modified:** crates/pmcp-server-toolkit/src/tools.rs
- **Verification:** `cargo test -p pmcp-server-toolkit --features http synth_http` 6/6 green.
- **Committed in:** `e085360f` (Task 2 commit)

**2. [Rule 3 - Blocking] `parking_lot` and `RequestHandlerExtra::new` were not the available test idioms**
- **Found during:** Task 2 (synth_http + proptest compilation)
- **Issue:** The first draft used `parking_lot::Mutex` (not a toolkit dependency) and `RequestHandlerExtra::new(id, Default::default())` (the second arg is a `CancellationToken`, not `Default`).
- **Fix:** Used `std::sync::Mutex` (with `.lock().unwrap()`) and the established `RequestHandlerExtra::default()` idiom (already used in `tests/synthesizer_structured_content.rs` / `tests/code_mode_tools.rs`).
- **Files modified:** crates/pmcp-server-toolkit/src/tools.rs, crates/pmcp-server-toolkit/tests/http_connector_props.rs
- **Verification:** full `--features http` suite + the proptest green.
- **Committed in:** `e085360f` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both blocking/Rule 3) — all compilation reconciliations against the real dependency/alias surface. No scope creep; public surface matches the plan's `artifacts` and `must_haves`.

## Issues Encountered

- The crate-level `Result<T>` alias shadows `std::result::Result` inside `tools.rs`, so a 2-generic `Result` in a test `impl` must be fully qualified (see Deviation 1). Documented here so Plan 05's `ScriptToolHandler` test additions avoid the same trap.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (not `type: tdd`); each task carried `tdd="true"`. Tests and implementation were committed together per task rather than as separate RED/GREEN commits, because every assertion targets net-new functions/types (the parser, the synthesizer, the additive `Operation.base_url` field) with no prior passing behaviour to protect. All `<behavior>` items in both tasks have passing tests (231 total under `--features http`).

## Known Stubs

None for this plan's scope. The `script` arm is an INTENTIONAL, documented seam (returns a typed `ToolkitError::Synth` referencing Plan 05), not a silent stub — Plan 05 widens the signature and fills the `ScriptToolHandler` branch. The `Operation.base_url` override is reflected by the synthesizer; whether the Plan 01 connector additionally re-targets per-operation (vs. its constructed base) is a localized future concern, but the per-tool value is carried, not dropped.

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. The spec parser reads admin-authored text (T-90-03-02 accepted); arg→path injection is bounded by the object-envelope schema + the declared-`{params}`-only substitution proptest (T-90-03-01); parser errors are redaction-disciplined (T-90-03-03); ill-formed tools are rejected (T-90-03-04).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 04 (code-mode executor)** can call `http::schema::OpenApiSchema::spec_text()` to serve the `api_schema` resource (D-03 surface (a)) and `operations()` for richer tool surfacing; the authoritative `Operation` type path is `crate::http::Operation`.
- **Plan 05 (script tools)** widens `synthesize_from_config_with_http_connector`'s signature (adding the shared `http_exec` + `exec_config`) and fills the explicit `is_script_tool()` seam with a `ScriptToolHandler` branch — a localized edit at `tools.rs:~378`.
- **Plan 06 (binary dispatch)** wires the synthesizer + the `Option<OpenApiSchema>` (parsed only when `--spec` supplied) into the server assembly.
- No blockers.

## Self-Check: PASSED

- All 6 modified files present on disk (no created files this plan).
- Both task commits (9b0601b6, e085360f) present in git history.
- Acceptance greps match: `pub use schema::Operation` (re-export from authoritative home); `synthesize_from_config_with_http_connector` in both tools.rs and lib.rs; no struct-literal `ToolInfo {` (the one match is a `-> ToolInfo {` return type); `.execute(&self.operation, &args)` key_link present.
- `cargo test -p pmcp-server-toolkit --features http schema_parse synth_http -- --test-threads=1` green; full `--features http` suite 231 green; default-features build unaffected; clippy clean on lib + tests.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
