---
phase: 90-openapi-built-in-server
plan: 05
subsystem: api
tags: [openapi, code-mode, script-tools, plan-executor, engine-parity, wiremock, js-runtime]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 03
    provides: "synthesize_from_config_with_http_connector single-call synthesizer + the explicit is_script_tool() seam; build_input_schema/build_annotations/apply_widget_meta helpers"
  - phase: 90-openapi-built-in-server
    plan: 04
    provides: "code_mode::HttpCodeExecutor (impl HttpExecutor, Clone, with_inbound_token) + the openapi-code-mode re-exports of ExecutionConfig/HttpExecutor/JsCodeExecutor"
  - phase: 90-openapi-built-in-server
    plan: 01
    provides: "http::HttpClient + http::auth::create_auth_provider/NoAuth/HttpAuthProvider; http feature"
provides:
  - "tools::ScriptToolHandler (gated openapi-code-mode) — admin-authored JS run over the SAME PlanCompiler+PlanExecutor+HttpCodeExecutor seam Code Mode uses, args bound to `args`, NO validate/token cycle"
  - "synthesize_from_config_with_http_connector_and_scripts(config, connector, http_exec, exec_config) — the OpenAPI Code Mode synthesizer that fills the is_script_tool() branch"
  - "synthesize_http_inner — shared single-call body taking a script-tool builder closure (single-call entry point keeps the typed seam error; Code Mode entry point builds a ScriptToolHandler)"
  - "tests/script_tool_engine_parity.rs — the first-class D-02 byte-equality proof (script tool vs execute_code, identical output + identical backend request sequence)"
affects: [90-06-binary-dispatch, 90-07-london-tube-parity]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ScriptToolHandler::handle replicates compile_and_execute byte-for-byte: PlanCompiler::with_config(cfg).compile_code(script) → PlanExecutor::new(http_exec.clone(), cfg.clone()) → set_variable(\"args\", args) → execute(&plan) → result.value — which is exactly what makes the JsCodeExecutor path produce identical output (the D-02 parity hinge)"
    - "Shared synthesize_http_inner + a build_script_tool closure: the single-call-only entry point passes a closure returning the typed seam error; the openapi-code-mode entry point passes a ScriptToolHandler-constructing closure — one loop body, one cog budget, the light build still compiles single-call-only"
    - "Engine-parity asserted by running ONE script through two surfaces against two identical wiremock servers, comparing both the serde_json::Value output AND the sorted (method, path) request sequence"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/script_tool.rs
    - crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs
  modified:
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/lib.rs

key-decisions:
  - "Did NOT widen the existing 2-arg synthesize_from_config_with_http_connector signature (it is gated `http`, where HttpCodeExecutor does not exist and the existing http_connector_props caller runs under openapi-code-mode too); instead added an additive openapi-code-mode-gated synthesize_from_config_with_http_connector_and_scripts(config, connector, http_exec, exec_config) and routed both through a shared synthesize_http_inner taking a script-tool builder closure — the single-call entry point's is_script_tool() arm now returns a typed error pointing at the openapi-code-mode script path"
  - "ScriptToolHandler stays crate-private (like SynthesizedToolHandler/HttpToolHandler); tests reach it through the public synthesizer, so the public surface is exactly the new synthesize fn + the existing re-exports"
  - "ScriptToolHandler::handle binds args to `args` via PlanExecutor::set_variable and returns ExecutionResult.value — byte-identical to compile_and_execute's set_variable(\"args\", vars.clone()), which is what the engine_parity test then proves"

patterns-established:
  - "Integration test file + fn-prefix named for the verify filter (tests/script_tool.rs with script_tool_-prefixed fns; tests/script_tool_engine_parity.rs with an engine_parity-prefixed fn) so the positional cargo filter resolves (Plan 01/04 verify-filter lesson)"

requirements-completed: [OAPI-02b, OAPI-10]

# Metrics
duration: 11min
completed: 2026-05-29
---

# Phase 90 Plan 05: Script Tools + Engine-Parity Proof Summary

**Implemented script tools (OAPI-02b / D-01): a `ScriptToolHandler` (gated `openapi-code-mode`) compiles admin-authored embedded JS with `PlanCompiler` and runs it through a fresh `PlanExecutor` over the SAME shared `HttpCodeExecutor` instance Code Mode uses — binding the schema-validated client args to `args` with NO validate/token cycle (Pitfall 7), bounded only by `ExecutionConfig` — and filled the `is_script_tool()` seam via a new `synthesize_from_config_with_http_connector_and_scripts` entry point; then proved D-02 / OAPI-10 with a first-class offline engine-parity test that runs ONE script through both surfaces (script tool vs `execute_code`) and asserts byte-equal output PLUS an identical backend request sequence.**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-05-29T19:15:42Z
- **Completed:** 2026-05-29T19:27Z
- **Tasks:** 2
- **Files modified:** 4 (2 created, 2 modified)

## Accomplishments

- **`ScriptToolHandler`** (`#[cfg(feature = "openapi-code-mode")]`): holds the admin `script`, the shared `HttpCodeExecutor`, the `ExecutionConfig` bounds, and the synthesized `tool_info` (object-envelope schema from `[[tools.parameters]]`, `additionalProperties:false`). `handle` compiles the JS (`PlanCompiler::with_config(&exec_config).compile_code(&script)` → `Internal` on compile failure), builds `PlanExecutor::new(http_exec.clone(), exec_config.clone())`, `set_variable("args", args)`, `execute(&plan)` (→ `Internal` on exec failure), and returns `result.value`. No `validate`/HMAC-token machinery (Pitfall 7 / T-90-05-01). `metadata()` returns `Some(tool_info)`.
- **Filled the `is_script_tool()` seam:** `synthesize_from_config_with_http_connector` (single-call only, `http`-gated) now delegates to a shared `synthesize_http_inner` and passes a closure that returns a typed `ToolkitError::Synth` pointing at the `openapi-code-mode` script path; the new `synthesize_from_config_with_http_connector_and_scripts` (`openapi-code-mode`-gated) passes a closure that constructs a `ScriptToolHandler` — threading the shared `http_exec` + `exec_config` the binary (Plan 06) will supply (OAPI-02b / D-01 / D-02).
- **D-02 hinge:** `ScriptToolHandler::handle` is byte-for-byte the same sequence as `pmcp_code_mode::compile_and_execute` (the `JsCodeExecutor` path): `PlanCompiler` → `PlanExecutor` over the SAME `HttpCodeExecutor` → `set_variable("args", …)` → `execute`. That is what makes the parity proof hold.
- **`tests/script_tool.rs`** (wiremock, `openapi-code-mode`): (a) a single-step script returns the mock JSON, (b) the london-tube multi-call chain (`filter` disrupted lines → per-line `api.get`) returns the chained result, (c) the client's `args.maxLines` is bound into the script, (d) a `max_api_calls = 2` cap aborts an over-budget script (T-90-05-02 DoS bound) — no infinite loop. All 4 green.
- **`tests/script_tool_engine_parity.rs`** (wiremock, `openapi-code-mode`): `engine_parity_script_tool_equals_execute_code` runs ONE script `S` through Path A (a synthesized `ScriptToolHandler`) and Path B (`JsCodeExecutor` over the SAME `HttpCodeExecutor`, the `execute_code` surface), asserts `serde_json::Value` byte-equality of the outputs AND an identical sorted `(method, path)` backend request sequence — the D-02 / OAPI-10 proof. Green.
- **Light build preserved:** `cargo build -p pmcp-server-toolkit --features "http code-mode"` (no `openapi-code-mode`) still compiles — the script-tool path is cfg'd out, single-call only (RESEARCH Pitfall 4).
- clippy clean on lib + tests under `openapi-code-mode`; the `http`-only suite (`synth_http` lib tests + `http_connector_props`) green against the refactored inner signature.

## Task Commits

1. **Task 1: ScriptToolHandler + is_script_tool() synthesis branch (OAPI-02b/D-01/D-02)** — `6d87f0be` (feat)
2. **Task 2: engine-parity proof — script tool vs execute_code byte-equality (D-02/OAPI-10)** — `ce7f9d5b` (test)

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/tools.rs` — `ScriptToolHandler` struct + `new` + `#[pmcp_code_mode::async_trait] impl ToolHandler`; `synthesize_from_config_with_http_connector_and_scripts`; refactored `synthesize_from_config_with_http_connector` + the new `synthesize_http_inner` (shared body taking a `build_script_tool` closure); updated the `synth_http_tests` script-seam test (now `synth_http_script_tool_without_engine_is_rejected`, asserting the `openapi-code-mode` seam message); openapi-code-mode imports of `HttpCodeExecutor` + `ExecutionConfig`.
- `crates/pmcp-server-toolkit/src/lib.rs` — `#[cfg(feature = "openapi-code-mode")] pub use crate::tools::synthesize_from_config_with_http_connector_and_scripts;`.
- `crates/pmcp-server-toolkit/tests/script_tool.rs` — 4 wiremock script-tool tests (single-call, multi-call chain, args binding, max_api_calls bound).
- `crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs` — the `engine_parity` D-02 byte-equality + request-sequence parity proof.

## Decisions Made

- **Additive `_and_scripts` entry point, not a widened signature.** The plan suggested "thread `http_exec` + `exec_config` into the synthesizer signature." Widening the existing `synthesize_from_config_with_http_connector` would break it: it is gated `http`, where `HttpCodeExecutor` is not in scope, and its existing caller (`tests/http_connector_props.rs`, `#![cfg(feature = "http")]`) also compiles under `openapi-code-mode` in the full suite. So the existing 2-arg fn is unchanged (single-call only) and a new `openapi-code-mode`-gated 4-arg `synthesize_from_config_with_http_connector_and_scripts` carries the engine; both route through a shared `synthesize_http_inner(config, connector, build_script_tool)` so the `is_script_tool()` arm is filled in exactly one place. Plan 06 calls the 4-arg variant.
- **`ScriptToolHandler` stays crate-private** (matching `SynthesizedToolHandler` / `HttpToolHandler`); tests reach it through the public synthesizer. The only new public surface is the one new synthesize fn (re-exported at the crate root).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The RESEARCH reference script used string `+` path concatenation + a bare `return await api.get(...)`, which this engine version rejects**
- **Found during:** Task 1 (first `script_tool` test run)
- **Issue:** Two engine-subset mismatches surfaced. (i) The compiler's `extract_path_template` accepts only a string literal or a template literal — `'/Line/' + line.id + '/Disruption'` (the RESEARCH "Code Examples" form) fails with "Invalid path template: Path must be a string or template literal". (ii) A top-level `return await api.get(...)` routes the `ApiCall` to the evaluator ("API calls should be handled by executor, not evaluator") — the ApiCall must be bound to a `const` first.
- **Fix:** Used a template literal `` `/Line/${line.id}/Disruption` `` in the chain, and `const status = await api.get(...); return status;` in the single-call test. These match the engine's accepted JS subset (the bind-then-use shape the reference SQL/Code-Mode scripts also use). The handler itself is correct — only the test scripts needed the engine-accurate form.
- **Files modified:** crates/pmcp-server-toolkit/tests/script_tool.rs
- **Committed in:** `6d87f0be` (Task 1 commit)

**2. [Rule 1 - Bug] `.slice(0, args.maxLines)` does NOT consume a runtime variable bound in this engine**
- **Found during:** Task 1 (the `args.maxLines` binding test)
- **Issue:** The compiler's `slice` only reads NUMERIC LITERAL args (`extract_number_arg`); `.slice(0, args.maxLines)` resolves `end = None` (slice-to-end) and the `BoundedLoop` `max_iterations` falls back to the compile-time default. So `args.maxLines` cannot bound the loop at runtime — the RESEARCH example over-claims engine support. The binding itself (`args.maxLines`) IS still available to the script.
- **Fix:** Reworked the binding test (c) to prove the `args` binding directly — a script that returns `args.maxLines` after a backend call observes the exact caller-supplied value (`maxLines=7` → `received: 7`). The multi-call chain test (b) keeps the `slice(0, args.maxLines)` shape but uses a single-disrupted-line fixture (so the count is deterministic regardless of slice bounding), still proving filter + per-line chaining.
- **Files modified:** crates/pmcp-server-toolkit/tests/script_tool.rs
- **Committed in:** `6d87f0be` (Task 1 commit)

**3. [Rule 1 - Bug] doc-prose tripped the `validate_code` / clippy doc-overindent checks**
- **Found during:** Task 1 (acceptance grep + clippy)
- **Issue:** The `ScriptToolHandler` doc-comment used the literal `validate_code` while describing its ABSENCE (the acceptance criterion requires count 0 in the handler region); and the `script_tool.rs` module doc had over-indented list continuation lines (clippy `doc_overindented_list_items`).
- **Fix:** Reworded the doc to "skips the Code Mode validation + HMAC-token gate" (no literal token) and de-indented the doc list. No behavior change.
- **Files modified:** crates/pmcp-server-toolkit/src/tools.rs, crates/pmcp-server-toolkit/tests/script_tool.rs
- **Committed in:** `6d87f0be` (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 1). Two were the reference doc over-stating the engine's accepted JS subset (string-concat paths, runtime slice bounds); one was a prose-vs-grep/clippy collision. No scope creep — public surface matches the plan's `artifacts` + `must_haves`.

## Issues Encountered

- The 90-RESEARCH.md "Code Examples" script is illustrative, not engine-verified: it uses `'/' + x + '/'` path concatenation and `.slice(0, args.maxLines)` with a runtime variable, neither of which the `pmcp-code-mode` `PlanCompiler` honors (paths need a template literal; `slice` bounds need numeric literals). Plan 07 (london-tube parity) and the Plan 06 binary's bundled config should use the engine-accurate forms (template-literal paths; bind-then-use ApiCalls; literal slice bounds or `for`-of without a variable slice cap).
- The `rtk` test-proxy strips per-test PASS lines from some `cargo test` output; verification relied on the authoritative cargo exit code + the aggregate `test result: ok` counts.

## TDD Gate Compliance

This plan's frontmatter is `type: execute`; each task carried `tdd="true"`. Tests and implementation were committed together per task because every assertion targets net-new types/behavior (the `ScriptToolHandler`, the `_and_scripts` synthesizer, the engine-parity proof) with no prior passing behaviour to protect. All `<behavior>` items in both tasks have passing tests.

## Known Stubs

None. `ScriptToolHandler` is fully wired through `PlanCompiler` + `PlanExecutor` + the real `HttpCodeExecutor`; the engine-parity test exercises the live (wiremock-backed) path on both surfaces.

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. T-90-05-01 (no token cycle, BY DESIGN) is upheld (no `validate`/HMAC machinery in `ScriptToolHandler`); T-90-05-02 (DoS) is covered by the `max_api_calls` over-budget abort test; T-90-05-03 (arg tampering) by the object-envelope schema validated before the script runs; T-90-05-04 (divergence) by the `engine_parity` byte-equality + request-sequence proof.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 06 (binary dispatch)** calls `synthesize_from_config_with_http_connector_and_scripts(cfg, connector, http_exec, exec_config)` — constructing ONE `HttpCodeExecutor` over the resolved backend `base_url` + auth, and ONE `ExecutionConfig` (from `[code_mode.limits]` / defaults), and passing the SAME `http_exec` instance into both the script-tool synthesizer and the `JsCodeExecutor` it wraps for Code Mode (D-02). Per request it may attach the inbound token via `HttpCodeExecutor::with_inbound_token`.
- **Plan 07 (london-tube parity)** can reuse the wiremock fixtures + the engine-accurate script forms documented above.
- No blockers.

## Self-Check: PASSED

- Both created test files present on disk; both modified source files present.
- Both task commits present in git history: `6d87f0be` (Task 1), `ce7f9d5b` (Task 2).
- Acceptance greps: `struct ScriptToolHandler` (1); `validate_code`/`approval token` in tools.rs (0); `cfg(feature = "openapi-code-mode")` on the handler (matches); wrong `all(http, code-mode)` gate (0); `tfl.gov.uk`/`reqwest::get` in both test files (0); `engine_parity` + `path_a_output, path_b_output` Value-equality assert present.
- `cargo test -p pmcp-server-toolkit --features openapi-code-mode script_tool -- --test-threads=1` → 4 passed; `... engine_parity ...` → 1 passed; light build (`http code-mode`) compiles; clippy clean on lib + tests.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
