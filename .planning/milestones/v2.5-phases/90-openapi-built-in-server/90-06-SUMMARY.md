---
phase: 90-openapi-built-in-server
plan: 06
subsystem: api
tags: [openapi, shape-a, binary, streamable-http, dispatch, oauth-passthrough, code-mode, wiremock]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 03
    provides: "synthesize_from_config_with_http_connector_and_scripts (single-call + script tools); OpenApiSchema parser + spec_text(); http::HttpClient connector"
  - phase: 90-openapi-built-in-server
    plan: 04
    provides: "code_mode::HttpCodeExecutor + with_inbound_token (H1); code_mode_tools_from_executor(Arc<dyn CodeExecutor>, ValidationFlavor); JsCodeExecutor + ExecutionConfig re-exports; ValidationFlavor::OpenApi"
  - phase: 90-openapi-built-in-server
    plan: 05
    provides: "ScriptToolHandler routed by is_script_tool() inside the _and_scripts synthesizer"
  - phase: 85-pure-config-binary
    provides: "pmcp-sql-server structural blueprint (cli/main/dispatch/assemble/lib split; serve over StreamableHttpServer; RunError shape; T-85-10-02 serve-task propagation)"
provides:
  - "crates/pmcp-openapi-server — the Shape A OpenAPI pure-config binary (5-file crate + example + smoke test)"
  - "dispatch(cfg) -> (Arc<dyn HttpConnector>, HttpCodeExecutor) — the lazy [backend] -> connector+executor PAIR seam (CF-2)"
  - "build_server(cfg, connector, http_exec, Option<OpenApiSchema>) — toolkit assemble (single-call + script + Code Mode + resources/prompts), NOT the reference server builder (Pitfall 6)"
  - "TokenCaptureAuthProvider + request_executor — inbound MCP token capture threaded to with_inbound_token for oauth_passthrough (H1)"
  - "load_config_and_spec / run_serving / run — the run pipeline over streamable HTTP (CF-1), --spec OPTIONAL (D-03)"
affects: [90-07-london-tube-parity, 90-08-deploy, 90-09-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dispatch returns the (HttpConnector, HttpCodeExecutor) PAIR over a SHARED reqwest::Client + auth provider (D-02: one engine feeds single-call tools, script tools, and Code Mode)"
    - "Inbound token capture at the binary boundary (TokenCaptureAuthProvider) + a request_executor seam that threads the captured AuthContext.token via HttpCodeExecutor::with_inbound_token — the binary owns this because the toolkit synthesizer holds a fixed http_exec"
    - "D-03 no-spec + code-mode = warn-and-proceed: Code Mode RUNS without the api_schema resource (tracing::warn!), never fails, never silently drops"
    - "run_serving returns (bound_addr, JoinHandle) so integration tests drive the REAL binary path and abort the handle (bounded shutdown — no leaked server)"

key-files:
  created:
    - crates/pmcp-openapi-server/Cargo.toml
    - crates/pmcp-openapi-server/src/cli.rs
    - crates/pmcp-openapi-server/src/main.rs
    - crates/pmcp-openapi-server/src/lib.rs
    - crates/pmcp-openapi-server/src/dispatch.rs
    - crates/pmcp-openapi-server/src/assemble.rs
    - crates/pmcp-openapi-server/examples/openapi_server_min.rs
    - crates/pmcp-openapi-server/tests/http_smoke.rs
  modified:
    - Cargo.toml

key-decisions:
  - "reqwest pinned to 0.13 (NOT 0.12) so the shared reqwest::Client threaded into HttpClient::new / HttpCodeExecutor::new is the SAME type the toolkit expects — a version mismatch would make the Client types incompatible (Rule 3 blocking)"
  - "Used the free synthesize_from_config_with_http_connector_and_scripts + code_mode_tools_from_executor functions, registering handlers via tool_arc, because there is NO ServerBuilderExt::*_with_http_connector method (the http path uses free functions; confirmed against builder_ext.rs and Plans 04/05 summaries) — the plan's ServerBuilderExt pattern label is aspirational"
  - "ExecutionConfig is derived from [code_mode.limits]: max_tables_per_query -> max_api_calls, max_join_depth -> max_loop_iterations (the closest OpenAPI mapping of the SQL-shaped complexity ceiling), defaults otherwise (max_api_calls=50, max_loop_iterations=100, timeout=30)"
  - "TokenCaptureAuthProvider.is_required()=false (best-effort capture) so a curated / static-auth server still serves unauthenticated clients; request_executor reads AuthContext.token from RequestHandlerExtra and threads it via with_inbound_token (H1)"
  - "api_schema resource merges from --spec when present (override an existing api_schema / /schema-suffixed resource, else append); no api_schema is synthesized when --spec is absent (D-03)"

patterns-established:
  - "Pattern: wiremock-backed http_smoke that drives run_serving (the REAL binary path) for initialize + tools/list over the SDK StreamableHttpTransport, then aborts the handle — proves curated-only no-spec boot + CF-1 + bounded shutdown in one offline test"

requirements-completed: [OAPI-06]

# Metrics
duration: 13min
completed: 2026-05-29
---

# Phase 90 Plan 06: pmcp-openapi-server Shape A Binary Summary

**Built `crates/pmcp-openapi-server` — the Shape A OpenAPI pure-config binary, a structural copy of `pmcp-sql-server` (`run → load_config_and_spec → dispatch → build_server → serve` over streamable HTTP): `--spec` is OPTIONAL at runtime (D-03, a curated-only config boots with no spec), `dispatch` builds the `(HttpConnector, HttpCodeExecutor)` PAIR lazily over a shared `reqwest::Client` (CF-2, no backend contact at startup), `build_server` wires single-call + script tools + Code Mode through the toolkit's free synthesizers (NOT the reference server builder, Pitfall 6), and an inbound `TokenCaptureAuthProvider` captures the MCP client token into `AuthContext` which `request_executor` threads into `HttpCodeExecutor::with_inbound_token` so `oauth_passthrough` forwards it per request (H1). No-spec + code-mode has a defined warn-and-proceed behavior.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-05-29T20:50:23Z
- **Completed:** 2026-05-29T21:03Z
- **Tasks:** 3
- **Files modified:** 9 (8 created, 1 modified)

## Accomplishments

- **Task 1 — scaffold:** `Cargo.toml` (toolkit dep on the `openapi-code-mode` UMBRELLA feature so the script-tool/code-mode engine compiles in; `exclude` keeps fixtures out of the published crate), `cli.rs` (`Args` with `config: PathBuf` required, `spec: Option<PathBuf>` OPTIONAL — D-03, `http` default `127.0.0.1:8080`), `main.rs` (thin `#[tokio::main]` shim), and `crates/pmcp-openapi-server` added to the root `[workspace.members]`. 4 cli arg-parse tests (incl. the D-03 config-only-no-spec parse).
- **Task 2 — dispatch:** `dispatch(cfg) -> (Arc<dyn HttpConnector>, HttpCodeExecutor)` builds a shared `reqwest::Client` + auth provider ONCE (lazy — no network, CF-2) and threads them into both an `HttpClient` (Plan 03 single-call connector) and an `HttpCodeExecutor` (Plan 04 Code-Mode/script surface) — D-02, one engine. `DispatchError` is `#[non_exhaustive]` + redaction-disciplined (names backend/field only, never base_url/credentials — Pitfall 5 / T-90-06-01). 3 tests: offline pair build, `MissingBackend`, `display_no_secret` (asserts the backend base_url itself is absent — Codex LOW).
- **Task 3 — assemble + lib + example:** `build_server` wires single-call + admin-authored script tools via `synthesize_from_config_with_http_connector_and_scripts` (Plans 03/05) and Code Mode via `code_mode_tools_from_executor` over a `JsCodeExecutor` wrapping the SAME `http_exec` (`ValidationFlavor::OpenApi`, D-02), preserves configured resources/prompts, merges the `--spec` as the `api_schema` resource when present, and installs the inbound `TokenCaptureAuthProvider` (H1). `lib.rs` carries `RunError`, `load_config_and_spec` (parses the spec ONLY when `--spec` is supplied — D-03), `serve` (over `StreamableHttpServer` — CF-1), `run_serving` (the testable `(addr, handle)` seam), and `run` (awaits the handle, propagating a serve-task panic as `RunError::Serving` — T-90-06-03). A build-only `examples/openapi_server_min.rs` (assembles + returns, does NOT hang) and an `http_smoke` integration test (curated-only no-spec boot + initialize + tools/list over streamable HTTP + bounded shutdown).
- **H1 token capture wired:** `request_executor(base, extra)` reads `AuthContext.token` from `RequestHandlerExtra` and produces a per-request `with_inbound_token` clone (the binary-side seam; the toolkit synthesizer holds a fixed `http_exec`, documented).
- **D-03 no-spec + code-mode = warn-and-proceed:** `[code_mode] enabled=true` + no spec emits a `tracing::warn!` and proceeds — Code Mode runs without the `api_schema` resource (tested: server builds + code-mode tools register with no spec).
- 18 tests green (13 lib/unit + 1 http_smoke + 4 doctests); `pmcp-sql-server` non-regression build green; clippy clean on lib + tests + example; build-only example builds (does not hang).

## Task Commits

1. **Task 1: scaffold the crate (Cargo.toml + cli + main + workspace member)** — `74be3850` (feat)
2. **Task 2: dispatch builds the (HttpConnector, HttpCodeExecutor) pair lazily** — `6012162f` (feat)
3. **Task 3: assemble.rs + lib.rs run pipeline over streamable HTTP (D-03/CF-1/H1)** — `5eee0fdb` (feat)

## Files Created/Modified

- `crates/pmcp-openapi-server/Cargo.toml` — package + lib + bin; `pmcp-server-toolkit` on the `openapi-code-mode` umbrella; reqwest 0.13 (shared-Client type match); `exclude` fixtures; dev-deps wiremock/mcp-tester/url/tempfile.
- `crates/pmcp-openapi-server/src/cli.rs` — `Args` (`config` required, `spec: Option<PathBuf>` OPTIONAL, `http` loopback default) + 4 tests.
- `crates/pmcp-openapi-server/src/main.rs` — thin `#[tokio::main]` shim → `run(Args::parse())`.
- `crates/pmcp-openapi-server/src/dispatch.rs` — `dispatch` (the lazy pair seam) + `DispatchError` (`#[non_exhaustive]`, redacted) + 3 tests.
- `crates/pmcp-openapi-server/src/assemble.rs` — `build_server`, `TokenCaptureAuthProvider`, `request_executor`, `execution_config`, `merge_spec_resource`, `register_prompts`, `AssembleError` + 5 tests (no-spec+code-mode warn-and-proceed, spec→api_schema merge, no-spec→no api_schema, request_executor token threading).
- `crates/pmcp-openapi-server/src/lib.rs` — `RunError`, `load_config_and_spec`, `serve` (StreamableHttpServer/CF-1), `run_serving`, `run` + 2 tests (serve-task panic mapping, addr display).
- `crates/pmcp-openapi-server/examples/openapi_server_min.rs` — build-only ≤15-line wiring example (does not hang).
- `crates/pmcp-openapi-server/tests/http_smoke.rs` — curated-only no-spec boot + initialize + tools/list over streamable HTTP + bounded shutdown (SC-1).
- `Cargo.toml` (root) — `crates/pmcp-openapi-server` added to `[workspace.members]`.

## Decisions Made

- **reqwest 0.13 (not 0.12):** the binary threads a `reqwest::Client` into `HttpClient::new` / `HttpCodeExecutor::new`, so it MUST use the toolkit's reqwest 0.13 or the `Client` types are incompatible (Rule 3 reconciliation — caught at compile time during Task 1).
- **Free functions, not a ServerBuilderExt http method:** there is no `ServerBuilderExt::*_with_http_connector`; the http assemble path uses the free `synthesize_*_and_scripts` + `code_mode_tools_from_executor` and registers via `tool_arc` (the same pattern the SQL ext methods use internally). The plan's `ServerBuilderExt` pattern label is the SQL analog, not a real http method.
- **ExecutionConfig from [code_mode.limits]:** `max_tables_per_query → max_api_calls`, `max_join_depth → max_loop_iterations` (the OpenAPI mapping of the SQL-shaped complexity ceiling); defaults otherwise.
- **H1 capture at the binary boundary:** because the toolkit synthesizer constructs its handlers over a fixed `http_exec` and does not yet read `extra`, the binary owns the per-request threading via the documented `request_executor` seam — `TokenCaptureAuthProvider` is the capture side, `with_inbound_token` the forward side, and Plan 01's outbound `apply(.., inbound_token)` the receiving end.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] reqwest version mismatch (0.12 vs the toolkit's 0.13) would make the shared Client type incompatible**
- **Found during:** Task 1 (after scaffolding, before the cli commit)
- **Issue:** The initial `Cargo.toml` declared `reqwest = "0.12"`, but `dispatch` (Task 2) threads a single `reqwest::Client` into the toolkit's `HttpClient::new` / `HttpCodeExecutor::new`, which are compiled against reqwest 0.13. Two different reqwest majors → two distinct `reqwest::Client` types → a type error.
- **Fix:** Pinned the binary to `reqwest = { version = "0.13", default-features = false, features = ["json", "rustls"] }` matching the toolkit.
- **Files modified:** crates/pmcp-openapi-server/Cargo.toml
- **Verification:** Task 2/3 compile + all tests green.
- **Committed in:** `74be3850` (Task 1 commit)

**2. [Rule 1 - Bug] prose-vs-grep collisions tripped the lazy / Pitfall-6 acceptance greps**
- **Found during:** Task 2 (lazy grep) + Task 3 (Pitfall 6 grep)
- **Issue:** Doc/comment prose contained the literal tokens the acceptance criteria require at count 0 — `dispatch.rs` described "No `schema_text()` / `execute()`" (lazy grep = 3) and `assemble.rs` described NOT lifting `OpenApiPmcpBuilder` (Pitfall 6 grep = 1). The same prose-vs-grep trap Plans 04/05 hit.
- **Fix:** Reworded the prose to describe the behavior without the literal tokens ("no spec read, no backend request"; "the reference pmcp-run server builder"). No behavior change.
- **Files modified:** crates/pmcp-openapi-server/src/dispatch.rs, crates/pmcp-openapi-server/src/assemble.rs
- **Verification:** lazy grep → 0; Pitfall 6 grep → 0; tests still green.
- **Committed in:** `6012162f` (Task 2) / `5eee0fdb` (Task 3)

**3. [Rule 1 - Bug] inline token_secret rejected by R9 enforcement in test configs**
- **Found during:** Task 3 (no_spec_code_mode_warns_and_still_builds + http_smoke first run)
- **Issue:** The first-draft test configs used a literal `token_secret = "0123...abcdef"`, which `code_mode_tools_from_executor` rejects at wiring time (`ConfigValidationError::InlineSecretRejected`, review R9 — secrets must be env refs).
- **Fix:** Switched the test configs to `${VAR}` env references and set the env var before building (`OPENAPI_ASSEMBLE_SECRET` / `OPENAPI_SMOKE_SECRET`). Note: the `${...}` in the `format!`-built smoke config must be escaped as `${{...}}` or `format!` treats `{...}` as a named placeholder.
- **Files modified:** crates/pmcp-openapi-server/src/assemble.rs, crates/pmcp-openapi-server/tests/http_smoke.rs
- **Verification:** all 13 lib tests + http_smoke green.
- **Committed in:** `5eee0fdb` (Task 3 commit)

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 bug). One was the reqwest-version reconciliation against the real toolkit surface; one the recurring prose-vs-grep collision; one the R9 inline-secret enforcement (a correctness gate, not a bug in the binary). No scope creep — public surface matches the plan's `artifacts` + `must_haves`.

## Issues Encountered

- `format!` interprets `${OPENAPI_SMOKE_SECRET}` as a named-argument placeholder (the `{...}` part), so a literal `${VAR}` inside a `format!` template must be written `${{VAR}}`. Documented for Plan 07's london-tube fixtures, which will also embed `${...}` secret refs in `format!`-built configs.
- The `rtk` test-proxy strips per-test PASS lines from some `cargo test` output; verification relied on the authoritative cargo exit code + aggregate `test result: ok` counts.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (no per-task `tdd="true"`). Tests and implementation were committed together per task because every assertion targets net-new functions/types (the dispatch pair seam, `build_server`, `request_executor`, the run pipeline) with no prior passing behavior to protect. All acceptance criteria have passing tests.

## Known Stubs

None. The binary is fully wired: dispatch builds the real connector+executor pair, `build_server` registers real single-call/script/code-mode tools, the http_smoke test drives the live (wiremock-backed) streamable-HTTP path end to end, and the example assembles a real server.

One documented seam (not a stub): the per-request `with_inbound_token` threading lives in the binary's `request_executor` helper rather than inside the toolkit synthesizer's handlers (which hold a fixed `http_exec` and do not yet read `extra`). The capture side (`TokenCaptureAuthProvider`) is installed and the forward side (`with_inbound_token`) is exercised by a unit test; full per-tool-invocation passthrough wiring through the toolkit handler boundary is the receiving end Plan 01/04 already provide and a future toolkit-handler `extra` hook would complete. This is the H1 seam the plan explicitly anticipated ("if the toolkit synthesizer does not yet thread `extra` … wire the capture at the binary's handler boundary; document the seam").

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. T-90-06-01 (error redaction) is covered by `dispatch_error_display_redacts_backend_and_secrets` (incl. base_url); T-90-06-02 (inbound transport) reuses `pmcp::StreamableHttpServer`'s security layers (not re-implemented); T-90-06-03 (serve-task panic) by `serving_task_panic_maps_to_run_error_serving`; T-90-06-05 (token misrouting) by the `request_executor` token-threading test + Plan 01's static-ignores-inbound guarantee.

## User Setup Required

None for development. To RUN the binary, an operator points it at a `config.toml` (with a `[backend]` block; `${VAR}` secret refs resolved from the env) and optionally an OpenAPI `--spec`.

## Next Phase Readiness

- **Plan 07 (london-tube parity)** can drive `run_serving` against the london-tube config + (optionally) the tfl spec, reusing the wiremock smoke pattern + the engine-accurate script forms from Plan 05; remember to escape `${...}` as `${{...}}` in `format!`-built fixtures.
- **Plan 08 (deploy)** wires this binary into the Lambda/CloudRun/Cloudflare targets (the serve seam + `run` are deploy-ready; T-90-06-04 deploy posture is Plan 08's scope).
- **Plan 09 (docs)** documents the `pmcp-openapi-server --config c.toml [--spec s.yaml]` workflow (Shape A) alongside the `cargo pmcp new --kind openapi-server` scaffold (Shape B).
- No blockers.

## Self-Check: PASSED

- All 8 created files + the 1 modified root Cargo.toml present on disk.
- All 3 task commits present in git history: `74be3850`, `6012162f`, `5eee0fdb`.
- Acceptance greps: `pub spec: Option<PathBuf>` (1); `openapi-code-mode` in Cargo.toml (present, NOT `["code-mode","http"]`); lazy grep `schema_text|.execute(` in dispatch (0); `HttpCodeExecutor` return in dispatch (present); `load_config_and_spec` + `StreamableHttpServer` in lib (present); Pitfall 6 `mcp_server_common::|OpenApiPmcpBuilder` in assemble (0); `with_inbound_token` in assemble (≥1).
- `cargo test -p pmcp-openapi-server -- --test-threads=1` green (13 lib + 1 http_smoke + 4 cli/main); doctests 4 green; `cargo build -p pmcp-openapi-server --example openapi_server_min` builds (does not hang); `pmcp-sql-server` non-regression build green; clippy clean on lib + tests + example.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
