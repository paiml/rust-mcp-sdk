---
phase: 90-openapi-built-in-server
plan: 07
subsystem: api
tags: [openapi, parity, wiremock, london-tube, api-key, secret-expansion, mcp-tester, offline]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 06
    provides: "pmcp-openapi-server Shape A binary; run_serving(args) -> (bound_addr, JoinHandle) testable seam; dispatch builds (HttpConnector, HttpCodeExecutor) over a shared reqwest::Client + auth provider"
  - phase: 90-openapi-built-in-server
    plan: 05
    provides: "ScriptToolHandler over the PlanCompiler+PlanExecutor+HttpCodeExecutor engine; engine-accurate JS subset (template-literal api.get paths, const-before-return, args.maxLines)"
  - phase: 90-openapi-built-in-server
    plan: 03
    provides: "OpenApiSchema::parse + operation_for; synthesize_from_config_with_http_connector single-call synthesizer"
  - phase: 90-openapi-built-in-server
    plan: 01
    provides: "http::auth::AuthConfig (api_key query_params/headers) + create_auth_provider + ApiKeyAuth provider; HttpAuthProvider::apply(.., inbound_token)"
  - phase: 85-pure-config-binary
    provides: "parity_chinook.rs harness blueprint (run_serving testable seam, ServerTester + ScenarioExecutor replay, per-step StepResult.success gating, bounded shutdown via handle.abort())"
provides:
  - "crates/pmcp-openapi-server/tests/parity_replay.rs — wiremock-backed london-tube parity (OAPI-08/D-04): fixture-validity + real-binary-path replay + ${TFL_APP_KEY}->app_key=dummy secret-expansion proof + #[ignore] env-gated live TfL replay"
  - "crates/pmcp-openapi-server/tests/fixtures/{london-tube.toml, london-tube-api.yaml, london-tube-scenarios.yaml} — self-contained, publish-excluded, reference-provenanced parity fixtures"
  - "create_auth_provider now RESOLVES ${VAR}/env:VAR references in api_key query_params/headers (resolve_api_key_value + expand_api_key_map) — the outbound secret-expansion path the london-tube api_key relies on"
affects: [90-08-deploy, 90-09-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Parity replay drives the REAL binary path (run_serving) against a wiremock backend (NOT an injected connector) — the same proof shape as the SQL parity_chinook.rs, adapted from a vendored DB to a mocked REST API (D-04, no Docker, no live network)"
    - "Outbound api_key secret expansion at provider-construction time: ${VAR}/env:VAR -> resolved value; unset required=false ref -> OMITTED (never sent as empty/literal), mirroring the token_secret env-ref discipline"
    - "Secret-expansion proven two-sided: wiremock query_param(\"app_key\",\"dummy\") matchers REQUIRE the resolved value to serve any response, AND post-replay received_requests inspection asserts the literal ${TFL_APP_KEY} never reached the wire"
    - "Live network replay double-gated (#[ignore] + PMCP_OPENAPI_LIVE_TEST=1 + real TFL_APP_KEY env early-return), offline wiremock is the default-CI proof (Phase 84/86 double-gate convention)"

key-files:
  created:
    - crates/pmcp-openapi-server/tests/parity_replay.rs
    - crates/pmcp-openapi-server/tests/fixtures/london-tube.toml
    - crates/pmcp-openapi-server/tests/fixtures/london-tube-api.yaml
    - crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml
  modified:
    - crates/pmcp-server-toolkit/src/http/auth.rs

key-decisions:
  - "[Rule 2] The ${TFL_APP_KEY} reference in [backend.auth] query_params was NOT expanded anywhere in the pipeline — create_auth_provider cloned the literal into ApiKeyAuth, so the literal `${TFL_APP_KEY}` would have reached the backend (100% auth failure in production). Added resolve_api_key_value + expand_api_key_map to create_auth_provider (the single correct home) so the RESOLVED value is applied; unset required=false refs are omitted. The SQL side had scoped ${VAR} expansion to token_secret only (Plan 85-01); this extends the same discipline to the outbound api_key the OpenAPI london-tube fixture needs."
  - "Tool-surface parity asserted BEHAVIORALLY (tool_call) not via a tools-array string-contains. mcp-tester's `contains` matches string array ELEMENTS, but list_tools returns tool OBJECTS — a name-contains on the tools array is always false. Invoking each tool (get-tube-status, disrupted-lines-with-detail, validate_code) proves presence + behavior in one step."
  - "Live test gates only on capability-discovery steps (tool surface), NOT tool-output value assertions — real-time TfL status legitimately varies, so 'Victoria'/'Severe delays' value matches are wiremock-only."
  - "Backend base_url overridden via string-replace of the vendored fixture (REFERENCE_BASE_URL const) at test time, keeping auth/tools/code_mode byte-identical — same technique as parity_chinook.rs's file_path override."

patterns-established:
  - "Pattern: a wiremock matcher that REQUIRES a resolved secret query param (query_param(\"app_key\",\"dummy\")) doubles as the secret-expansion proof — if expansion regressed to the literal ${...}, no matcher would fire and the replay would fail, so the parity gate IS the expansion gate."

requirements-completed: [OAPI-08]

# Metrics
duration: 12min
completed: 2026-05-29
---

# Phase 90 Plan 07: London-Tube Reference Parity Summary

**Reproduced the london-tube (TfL) reference instance OFFLINE: vendored three self-contained, reference-provenanced fixtures (`london-tube.toml` with `api_key` query-param auth + one single-call + one script tool, a minimal OpenAPI spec, and an `mcp-tester` scenario contract), and proved through the REAL binary pipeline (`run_serving`) against a `wiremock` backend that the binary serves the same tools + behavior as the reference (OAPI-08/D-04) — with the new `api_key` query-parameter outgoing-auth path exercised end-to-end and `${TFL_APP_KEY}` RESOLVED to `app_key=dummy` in every backend request (never the literal `${...}`). A live `api.tfl.gov.uk` replay exists but is double-gated. The parity also surfaced and fixed a real gap: the outbound api_key `${VAR}` reference was never expanded, so the literal `${TFL_APP_KEY}` would have hit the backend in production — `create_auth_provider` now resolves it (Rule 2).**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-05-29T21:10:21Z
- **Completed:** 2026-05-29T21:22Z
- **Tasks:** 2
- **Files modified:** 5 (4 created, 1 modified)

## Accomplishments

- **Task 1 — vendor the fixtures (`f52434a6`):**
  - `london-tube.toml` — adapted to the toolkit `[backend]`/`[[tools.parameters]]` config shape (NOT the reference's `shared/mcp-server-common` shape): `[backend.auth] type="api_key"` with `query_params = { app_key = "${TFL_APP_KEY}" }` + `required = false`, `[backend.http]`, one single-call tool (`get-tube-status` → `GET /Line/Mode/tube/Status`), one script tool (`disrupted-lines-with-detail`, engine-accurate JS subset reading `args.maxLines`), and `[code_mode]` with a DEV-only inline `token_secret` guarded by `allow_inline_token_secret_for_dev = true` (CF-4). Provenance header records the reference path + pmcp-run commit `4f5b4a47` (2026-02-07).
  - `london-tube-api.yaml` — minimal OpenAPI 3.0 spec covering exactly the two tool operations (`GET /Line/Mode/tube/Status`, `GET /Line/{lineId}/Disruption`).
  - `london-tube-scenarios.yaml` — `mcp-tester` `TestScenario` capturing the reference tool surface + outputs.
  - `london_tube_fixture` test — parses the config via `ServerConfig::from_toml_strict_validated` + the spec via `OpenApiSchema::parse`, asserts the tool names, the api_key auth shape, and the script-tool detection. Publish-exclude verified (`cargo package --list` ships no `tests/fixtures/*`).
- **Task 2 — wiremock parity replay + secret-expansion proof + live gate (`7de53102`):**
  - `london_tube_parity_through_real_binary_path` — sets `TFL_APP_KEY=dummy`, stands up a `wiremock` backend whose matchers REQUIRE `app_key=dummy`, overrides the fixture `base_url` to the mock, drives `run_serving` (the REAL binary path, NO `--spec` — the reference ships none), and replays `london-tube-scenarios.yaml` via `ServerTester` + `ScenarioExecutor` gating each `StepResult.success`. Then inspects `received_requests()` to assert EVERY backend request carried `app_key=dummy` and NONE carried the literal `${TFL_APP_KEY}` (T-90-07-04).
  - `parity_live_tfl` — `#[ignore]` + a `PMCP_OPENAPI_LIVE_TEST=1` + real-`TFL_APP_KEY` double-gate; runs the same scenarios against `https://api.tfl.gov.uk`, gating on the deterministic capability-discovery steps only.
- **[Rule 2] outbound api_key secret expansion** — `create_auth_provider` now expands `${VAR}`/`env:VAR` references in the api_key `query_params`/`headers` before building `ApiKeyAuth`; unset `required=false` references are omitted. 3 toolkit unit tests added (`test_api_key_query_param_expands_braced_env_ref`, `test_api_key_query_param_unset_ref_is_omitted`, `test_resolve_api_key_value_forms`).
- 2 offline parity tests + 1 fixture test green, 1 live test correctly ignored; 163 toolkit `--features openapi-code-mode` tests green (auth change non-regression); full `pmcp-openapi-server` suite green; clippy + fmt clean on both crates.

## Task Commits

1. **Task 1: vendor london-tube parity fixtures + fixture-validity test** — `f52434a6` (test)
2. **Task 2: wiremock parity replay + ${TFL_APP_KEY} secret-expansion proof + env-gated live test (incl. Rule 2 auth expansion)** — `7de53102` (test)

## Files Created/Modified

- `crates/pmcp-openapi-server/tests/parity_replay.rs` — fixture-validity + wiremock parity (real binary path, per-step gate, secret-expansion proof) + `#[ignore]` live replay.
- `crates/pmcp-openapi-server/tests/fixtures/london-tube.toml` — vendored config (api_key query-param auth, single-call + script tools, DEV inline token_secret), reference provenance.
- `crates/pmcp-openapi-server/tests/fixtures/london-tube-api.yaml` — minimal OpenAPI spec for the two tool operations.
- `crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml` — mcp-tester parity contract (tool surface + outputs).
- `crates/pmcp-server-toolkit/src/http/auth.rs` — `resolve_api_key_value` + `expand_api_key_map` + wiring into `create_auth_provider`'s `ApiKey` arm + 3 unit tests (Rule 2).

## Decisions Made

- **[Rule 2 — outbound api_key expansion was missing.]** The plan's must_have ("Outbound auth receives the RESOLVED `${TFL_APP_KEY}` value … not the literal `${...}`") assumed upstream resolution ("Plan 02/Phase 83 resolves it"). It did NOT: `create_auth_provider` cloned the api_key value verbatim into `ApiKeyAuth`, which inserts it literally into the outgoing query. Only `token_secret` was env-expanded (Plan 85-01 scoped expansion to it). Without a fix, a real london-tube deployment would send `app_key=${TFL_APP_KEY}` to TfL and 100% of authenticated calls would fail. This is a correctness requirement (Rule 2), so I added the expansion to the toolkit auth provider (its natural home — `dispatch` builds the provider via this function; the test-only files cannot fix the runtime path). The fix touches `crates/pmcp-server-toolkit/src/http/auth.rs`, outside this plan's declared `files_modified`, but does NOT overlap the sibling 90-08 (`cargo-pmcp/` only), so the sequential constraint is preserved.
- **Behavioral tool-surface parity.** `mcp-tester`'s `contains` assertion matches string array elements; `list_tools` returns tool OBJECTS, so a name-contains on the `tools` array is structurally always false. Tool presence is asserted by INVOKING each tool — a passing call proves the tool is in the synthesized surface AND behaves correctly.
- **Live test gates discovery only.** Real-time TfL status varies, so the live replay asserts the tool surface (capability discovery) but not the wiremock-specific output values.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Outbound api_key `${VAR}` reference was never expanded**
- **Found during:** Task 2 (writing the secret-expansion assertion — the wiremock matcher requiring `app_key=dummy` never fired because the literal `${TFL_APP_KEY}` reached the query).
- **Issue:** `create_auth_provider`'s `ApiKey` arm cloned `query_params`/`headers` verbatim into `ApiKeyAuth`, which inserts them literally. No `${VAR}`/`env:VAR` expansion existed for outbound api_key (only `token_secret` was expanded, Plan 85-01). The plan's must_have (resolved value, not literal) was therefore unmet by the runtime pipeline.
- **Fix:** Added `resolve_api_key_value` (`${VAR}`/`env:VAR` → env value; unset/empty → empty; plain literal → verbatim) + `expand_api_key_map` (expand all entries, drop empties) and wired them into the `ApiKey` arm. Unset `required=false` references are omitted, never sent empty/literal.
- **Files modified:** `crates/pmcp-server-toolkit/src/http/auth.rs`
- **Verification:** 3 new toolkit unit tests + the parity test's two-sided proof (wiremock `app_key=dummy` matchers + `received_requests` literal-absence assertion); 163 toolkit `openapi-code-mode` tests non-regression green.
- **Committed in:** `7de53102` (Task 2 commit)

**2. [Rule 1 - Bug] Brittle tool-name `contains` assertions on the `tools` array always failed**
- **Found during:** Task 2 (first parity run — the three `Tools include …` steps failed while the tool-CALL steps passed).
- **Issue:** `mcp-tester`'s `contains` assertion on an array path matches string ELEMENTS; `list_tools` returns an array of tool OBJECTS, so `contains tools value:"get-tube-status"` is always false even though the tool is present.
- **Fix:** Replaced the three name-contains steps with behavioral `tool_call` steps (get-tube-status, disrupted-lines-with-detail already invoked; added a `validate_code` call) — a passing call proves presence + behavior.
- **Files modified:** `crates/pmcp-openapi-server/tests/fixtures/london-tube-scenarios.yaml`
- **Verification:** parity test green (6/6 steps).
- **Committed in:** `7de53102` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 Rule 2 missing-functionality on the runtime auth path, 1 Rule 1 test-assertion bug). The Rule 2 fix is the substantive one — the plan's headline must_have could not be honored end-to-end without it. No scope creep; public surface additions are limited to two private helpers in the toolkit auth module.

## Issues Encountered

- The reference london-tube config uses a DIFFERENT shape (`[tools.inputs.parameters.X]`, `[tools.outputs]`, `[secrets]`, `[observability]`) than the toolkit (`[[tools.parameters]]`, `[backend]`). The vendored fixture is a faithful but ADAPTED reduction, not a byte copy — provenance comments record this so a reviewer diffs intent, not syntax.
- `mcp-tester` `contains` on object arrays (see Deviation 2) — documented for Plan 09 docs / any future OpenAPI scenario authoring.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (no per-task `tdd="true"`). The fixtures + tests were committed together because every assertion targets net-new behavior (the vendored parity surface + the secret-expansion path) with no prior passing behavior to protect. All acceptance criteria have passing tests.

## Known Stubs

None. The parity test drives the live (wiremock-backed) streamable-HTTP path through the real binary end to end; the api_key expansion is real (resolves the env var and applies the value); the fixtures are self-contained and parse-valid. The live test is intentionally `#[ignore]` (not a stub — it is a real test gated to an opt-in operator environment, the plan's explicit design).

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. The Rule 2 api_key expansion narrows (not widens) the surface: it ensures the resolved secret — never the literal placeholder — reaches the wire, directly mitigating T-90-07-04. Threat coverage: T-90-07-01 (no real key in fixtures — `${TFL_APP_KEY}` placeholder + a dummy in tests); T-90-07-02 (live replay double-gated, default CI is wiremock-only); T-90-07-03 (per-step `StepResult.success` gate + fixtures carry provenance); T-90-07-04 (the `received_requests` literal-absence assertion).

## User Setup Required

None for development (offline parity runs with no secrets). To run the LIVE replay, an operator sets `PMCP_OPENAPI_LIVE_TEST=1` + a real `TFL_APP_KEY` and runs the `#[ignore]` test with `--ignored`.

## Next Phase Readiness

- **Plan 08 (deploy)** wires the binary into the deploy targets; the api_key `${VAR}` expansion now resolves correctly at runtime, so a deployed london-tube instance with `TFL_APP_KEY` in its env authenticates correctly (the deploy secret-rewrite posture is Plan 08's scope).
- **Plan 09 (docs)** can reference the london-tube fixture as the canonical OpenAPI Shape-A demo and the `mcp-tester` scenario as the parity contract; note the `contains`-on-object-arrays caveat when authoring scenario docs.
- No blockers.

## Self-Check: PASSED

- All 4 created files + the 1 modified `auth.rs` present on disk.
- Both task commits present in git history: `f52434a6`, `7de53102`.
- Acceptance greps: `type = "api_key"` (1), `script = """` (1), `vendored from` (1) in london-tube.toml; `app_key`/`app_key=dummy` (16) + `PMCP_OPENAPI_LIVE_TEST` (6) + `run_serving` (7) in parity_replay.rs; `cargo package --list` ships no `tests/fixtures/*` (exclude works).
- `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` green (london_tube_fixture + london_tube_parity_through_real_binary_path pass, parity_live_tfl ignored); full `pmcp-openapi-server` suite green; toolkit `--features openapi-code-mode` 163 tests green (auth non-regression); clippy + fmt clean on both crates.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
