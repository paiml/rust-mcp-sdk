---
phase: 112-version-plumbing-spine
plan: 09
subsystem: api
tags: [mcp, protocol-context, dispatch, streamable-http, prompts, resources, version-plumbing]

# Dependency graph
requires:
  - phase: 112-version-plumbing-spine (Plans 01-06)
    provides: ProtocolContext resolver, RequestHandlerExtra era/client_info/trace_context accessors, inject_v2_result_envelope, v2 HTTP header gate
provides:
  - extract_request_meta_value reads _meta from GetPrompt + ReadResource (not only CallTool)
  - protocol_context + request_meta threaded into RequestHandlerExtra in prompt & resource handlers at both native dispatch sites (core.rs + mod.rs)
  - method-aware HTTP header-gate logical-name extraction (resources/read from params.uri)
  - live HTTP prompts/get + resources/read v2 acceptance matrix + v1 RAW-byte golden identity
affects: [113-stateless-http, 114-tasks-extension, conformance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-request _meta/ProtocolContext threaded uniformly into every name/uri-bearing handler (CallTool, GetPrompt, ReadResource)"
    - "Method-aware wire logical-name resolution at the HTTP smuggling boundary (params.uri for resources/read, params.name otherwise)"
    - "v1 wire byte-identity proven by RAW-byte golden-fixture comparison, not two-key absence"

key-files:
  created: []
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/streamable_http_server.rs
    - tests/v2_required_headers.rs

key-decisions:
  - "extract_request_meta_value carries a documented go-forward policy + exhaustive-variant tripwire test guarding future _meta-bearing variants"
  - "resources/read logical name resolved from params.uri method-awarely — no synthetic params.name fallback (which would mask the bug)"
  - "protocol_context moves across mutually-exclusive dispatch arms — no cloning needed (compiler-verified)"
  - "handler-visibility tests enter through the REAL dispatch entrypoints (handle_request_internal / process_client_request), never leaf handlers, and assert trace_context() so a dropped with_request_meta regresses"

patterns-established:
  - "Every ClientRequest _meta-bearing variant must be wired into extract_request_meta_value AND the tripwire test"
  - "v1 byte-identity guarded by RAW-string capture + golden serde_json::Value equality"

requirements-completed: [VERS-01, VERS-03, VERS-05, VERS-07, VERS-09]

# Metrics
duration: 40min
completed: 2026-07-23
---

# Phase 112 Plan 09: Version-Plumbing Spine Gap-Closure (Prompts + Resources) Summary

**Generalized the per-request `_meta`/`ProtocolContext` spine from `tools/call`-only to also cover `prompts/get` and `resources/read` at both native dispatch sites, and made the v2 HTTP header gate resolve the `resources/read` logical name method-awarely from `params.uri` so a standards-shaped v2 request is accepted instead of rejected 400.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-23
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- `extract_request_meta_value` now reads `_meta` from `CallTool`, `GetPrompt`, and `ReadResource` with a documented go-forward policy + exhaustive-variant tripwire test
- `handle_get_prompt`/`handle_read_resource` thread `protocol_context` + `request_meta` into `RequestHandlerExtra` at BOTH `core.rs` (ServerCore) and `mod.rs` (high-level Server) dispatch sites — `era()`/`client_info()`/`trace_context()` are now live inside prompt and resource handlers on a v2 connection
- HTTP header gate `extract_body_method_and_name` resolves the `resources/read` logical name from `params.uri` (review finding #2), so a standards-shaped v2 `resources/read` (uri only, no synthetic `params.name`) passes `cross_check_name`
- `inject_v2_result_envelope` (already generic) now demonstrably fires `resultType:"complete"` + `serverInfo` for v2 `prompts/get` + `resources/read`
- v1 wire output proven byte-for-byte unchanged for both methods via RAW-byte golden fixtures over the real HTTP transport

## Task Commits

Each task was committed atomically:

1. **Task 1: Generalize extract_request_meta_value + thread protocol_context/request_meta into prompt & resource handlers at both native sites** - `89d7ef88` (feat)
2. **Task 2: Method-aware HTTP header-gate logical-name extraction (resources/read from params.uri)** - `73ba4976` (fix)
3. **Task 3: Live HTTP prompts/get + resources/read v2 matrix + v1 RAW-byte golden identity** - `eb3c0361` (test)

_Note: Task 1 is `tdd="true"`; test + implementation landed in a single atomic feat commit (RED-verified locally: extract/dispatch tests fail on assertions before the wiring, pass after)._

## Files Created/Modified
- `src/server/core.rs` - extract_request_meta_value GetPrompt+ReadResource arms + doc policy; handle_get_prompt/handle_read_resource gain a protocol_context param and thread request_meta + protocol_context into RequestHandlerExtra; dispatch arms pass protocol_context; unit/proptest/tripwire/real-dispatch handler-visibility tests
- `src/server/mod.rs` - twin-site handle_get_prompt/handle_read_resource thread request_meta + protocol_context (parity with handle_call_tool); process_client_request arms pass protocol_context; real-dispatch handler-visibility test via process_client_request
- `src/server/streamable_http_server.rs` - extract_body_method_and_name method-aware (params.uri for resources/read); unit tests for uri-based extraction + cross_check_name acceptance
- `tests/v2_required_headers.rs` - greeting prompt + mem://greeting resource registered; Resp gains raw:String; prompt_body/resource_body typed builders; 5 new live-HTTP cells (v2 accept + envelope for both methods, fail-closed prompts/get, v1 byte-identity for both)

## Decisions Made
- `protocol_context` is moved (not cloned) across the mutually-exclusive `CallTool`/`GetPrompt`/`ReadResource` dispatch arms — the borrow checker confirms only one arm executes per request
- No synthetic `params.name` fallback for `resources/read`; the fix reads the true wire key (`uri`) so the smuggling-relevant WAF view is honored (D-06)
- Handler-visibility tests enter through the real dispatch entrypoints and assert `trace_context()` (not just `era`), so dropping either `.with_request_meta` or `.with_protocol_context` regresses the tests

## Deviations from Plan

None - plan executed exactly as written. (One clippy `needless_pass_by_value` on a test helper was corrected in-place before the Task 3 commit — a lint fix on new test code, not a behavior change.)

## Issues Encountered
- `ListResourcesResult` and other request/result types are `#[non_exhaustive]`; the integration test uses the public `::new` constructors instead of struct literals. `GetPromptRequest`/`ReadResourceRequest` are NOT non_exhaustive, so their bodies are built by literal + typed serde round-trip.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The phase headline claim ("ProtocolContext resolved once at ingress is threaded through dispatch to EVERY name/uri-bearing handler") is now TRUE for all three documented methods (tools/call, prompts/get, resources/read). Verification Gaps B and C are closed.
- All changes are crate-private/private (no public API surface changed) — the additive-only, semver-MINOR discipline holds.
- `make quality-gate` green (fmt, clippy pedantic+nursery, build, tests, examples).

## Self-Check: PASSED

All modified files present; all task commits (89d7ef88, 73ba4976, eb3c0361) and the plan-metadata commit (65fa4623) exist in git history.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-23*
