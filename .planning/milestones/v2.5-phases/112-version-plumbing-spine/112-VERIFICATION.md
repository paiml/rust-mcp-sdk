---
phase: 112-version-plumbing-spine
verified: 2026-07-23T02:44:20Z
status: passed
score: 9/9 requirements verified; 5/5 roadmap success criteria verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 2/5 roadmap success criteria verified (2/9 requirements BLOCKED, 1 additional requirement partially blocked)
  gaps_closed:
    - "Gap A: server/discover was 100% unreachable in production (VERS-04, ROADMAP SC#3) — closed by Plan 112-10 (classify-then-continue HttpIngress wiring on the real HTTP POST pipeline)"
    - "Gap B: per-request _meta/ProtocolContext spine wired for tools/call ONLY, not GetPrompt/ReadResource (VERS-01/03/09) — closed by Plan 112-09 Task 1"
    - "Gap C: HTTP header gate's logical-name extraction was not method-aware for resources/read, rejecting standards-shaped v2 requests (VERS-05) — closed by Plan 112-09 Task 2"
  gaps_remaining: []
  regressions: []
deferred: []
human_verification: []
---

# Phase 112: Version Plumbing Spine Verification Report

**Phase Goal:** pmcp resolves a per-request protocol era once at transport ingress and threads it explicitly through dispatch, so one binary understands both 2025-11-25 and 2026-07-28 clients — with v2 strictly opt-in, no v1 behavior change, and the whole milestone kept additive (2.x minor).
**Verified:** 2026-07-23T02:44:20Z
**Status:** passed
**Re-verification:** Yes — after gap closure (Plans 112-09, 112-10)

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | v2-opt-in server resolves `ProtocolContext` once at ingress from per-request `_meta`; a handler reads it via typed accessors on `RequestHandlerExtra`; v2 results carry `serverInfo` (VERS-01, VERS-03) | ✓ VERIFIED | Previously TRUE only for `tools/call`. Now `extract_request_meta_value` (`src/server/core.rs:1747`) matches `ClientRequest::CallTool`, `ClientRequest::GetPrompt`, AND `ClientRequest::ReadResource`. `handle_get_prompt`/`handle_read_resource` at BOTH native dispatch sites (`core.rs:802/881`, `mod.rs:1902/2028`) now call `.with_request_meta(..)` + `.with_protocol_context(..)`. Proven via real-dispatch-entrypoint tests (not leaf-handler calls): `core.rs` test `prompt_resource_protocol_context_via_dispatch_core` enters through `handle_request_internal`; `mod.rs` test `prompt_resource_protocol_context_via_dispatch_server` enters through `process_client_request`. Both distinguish `era==None` (non-opted-in) / `Some(Era::V1)` (opted-in v1 fallback) / `Some(Era::V2)`, and assert `client_info()`/`trace_context()` are populated on v2 — all pass (`cargo test --lib --features full`: 1229 passed, 0 failed). Live HTTP e2e (`v2_prompts_get_accepts_and_envelopes`, `v2_resources_read_accepts_and_envelopes`) confirm `serverInfo` + `resultType:"complete"` on real v2 responses over the real HTTP transport. |
| 2 | An existing v1 client negotiates exactly as before — `LATEST_PROTOCOL_VERSION` stays pinned to `2025-11-25`; `2026-07-28` reached only via explicit opt-in (VERS-02) | ✓ VERIFIED | Regression-checked: `src/types/protocol/version.rs:4` unchanged (`LATEST_PROTOCOL_VERSION = "2025-11-25"`); `SUPPORTED_PROTOCOL_VERSIONS` (4 entries) still excludes 2026-07-28. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` (independently re-run) → `223 checks: 223 pass, 30 skip`, "no semver update required." |
| 3 | A v2 client calling `server/discover` receives a read-only projection of already-computed `ServerCore` capabilities, including the `extensions` map (VERS-04, VERS-08) | ✓ VERIFIED | `server/discover` is now reachable in production over the real HTTP POST pipeline (previously 100% unreachable — every request returned `-32601`). Traced the full classify-then-continue wiring in `src/server/streamable_http_server.rs`: a crate-local `HttpIngress::{Public,Discover}` enum classifies the raw body (`classify_http_ingress`, line 655) at BOTH POST parse entrypoints (`parse_transport_message_fast:1541`, `parse_transport_message_with_middleware`); the real production router (`handle_post_request` → `handle_post_fast_path`/`handle_post_with_middleware`, both bound to `.route("/", post(handle_post_request))` at line 289) runs the `Discover` ingress through session resolution → the SAME `classify_v2_request` matrix via a raw-`_meta` counterpart (`run_v2_header_gate_raw`) → legacy-version validation → auth → dispatch, exactly mirroring every other method — confirmed by reading `handle_post_fast_path` end-to-end (lines 1708-1829). The `ServerCore` wrapper methods `dispatch_internal_client_request`/`handle_discover` are DELETED (`grep -rn 'dispatch_internal_client_request' src/` returns nothing); logic consolidated into one `build_discover_response` free fn (`core.rs:1201`) with no `#[allow(dead_code)]`. Live e2e proof (`tests/v2_required_headers.rs`, all pass): `server_discover_v2_returns_capability_projection_with_extensions` asserts the registered extension id IS present in the response `capabilities.extensions` map plus `serverInfo`+`resultType:"complete"`+preserved id; `server_discover_requires_auth_when_provider_installed` proves a real `AuthProvider` (`RejectingAuth`) still gates discover (401 when unauthenticated) — no auth bypass; `server_discover_runs_response_middleware` proves a real `ServerHttpMiddlewareChain` observes the discover response — no middleware bypass; 4 reject tests prove discover is subject to the SAME v2 header-classification matrix as `tools/call`; v1/non-opted-in tests confirm the documented D-10 `-32601@200` behavior with the original id preserved. |
| 4 | On the v2 HTTP path, required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound (VERS-05) | ✓ VERIFIED | Previously broken for 2 of 3 documented name-bearing methods. `extract_body_method_and_name` (`streamable_http_server.rs:537`) is now method-aware: `resources/read` resolves its logical name from `params.uri` (the field the real `ReadResourceRequest` actually carries — it has NO `name` field), `prompts/get`/`tools/call` unchanged at `params.name`. Unit tests `extract_body_method_and_name_uses_uri_for_resources_read` and `cross_check_name_accepts_resources_read_uri` pass. Live e2e `v2_resources_read_accepts_and_envelopes` sends a standards-shaped `ReadResourceRequest` body (uri only, no synthetic `params.name`) with `Mcp-Name` = the URI and gets HTTP 200 (previously would 400). `server/discover` (not name-bearing) is also enforced via the same matrix (`run_v2_header_gate_raw`), including a `server_discover_rejects_missing_mcp_name` presence-only test. All 25 tests in `tests/v2_required_headers.rs` pass, tools/call cells untouched. |
| 5 | Every result carries the `resultType` envelope discriminator defaulting to `complete`; W3C trace-context keys are surfaced via typed accessors and propagated through dispatch; all error codes resolve from one centralized version-gated table (VERS-06, VERS-07, VERS-09) | ✓ VERIFIED | VERS-06 (error-code centralization) was already verified and is unaffected by this closure (regression-checked: `error_codes.rs` has 12 `pub const` covering the full standard + pmcp `-320xx` family + frozen `V1_TASK_PENDING`; `error::ErrorCode`'s 11 consts still delegate via `Self(error_codes::NAME)`, confirmed by `grep -c` = 11). VERS-07 (`resultType`)/VERS-09 (trace-context) previously failed outside `tools/call` — root cause was the same Gap B defect, now closed: `inject_v2_result_envelope` (`core.rs:1153`) is confirmed generic (era-gated only, no method-specific logic) and now genuinely fires for `prompts/get`/`resources/read` too, per the dispatch-entrypoint tests in Truth #1 (which explicitly assert `trace_context()` reflects the injected W3C `traceparent`, proving `.with_request_meta` threading) and the live e2e envelope assertions. |

**Score:** 5/5 roadmap success criteria fully verified. All three previously-open gaps (A, B, C) are closed with direct code + live-HTTP-test evidence, not SUMMARY narrative.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| VERS-01 | 01, 02, 04, 09 | ProtocolContext resolved once at ingress, threaded through dispatch, handler-readable | ✓ SATISFIED | Threading now covers `tools/call`, `prompts/get`, `resources/read` at both native dispatch sites (Truth #1) |
| VERS-02 | 01, 04 | 2026-07-28 explicit opt-in; LATEST stays pinned | ✓ SATISFIED | Regression-checked (Truth #2) |
| VERS-03 | 01, 02, 04, 05, 09 | v2 self-describes via per-request `_meta`; v2 results carry serverInfo | ✓ SATISFIED | `_meta` extraction now covers `CallTool`+`GetPrompt`+`ReadResource` with an exhaustive-variant tripwire test (Truth #1) |
| VERS-04 | 03, 05, 10 | `server/discover` read-only capability projection | ✓ SATISFIED | Reachable in production over the real HTTP POST pipeline; classify-then-continue proven via auth + middleware bypass tests (Truth #3) |
| VERS-05 | 06, 09 | Required v2 headers enforced inbound/outbound | ✓ SATISFIED | Fixed for `prompts/get`/`resources/read`; method-aware logical-name extraction (Truth #4) |
| VERS-06 | 03, 06, 07, 08 | Centralized version-gated error-code table | ✓ SATISFIED | Regression-checked, unaffected by this closure (Truth #5) |
| VERS-07 | 05, 09 | `resultType` envelope, default `complete` | ✓ SATISFIED | Now fires for `prompts/get`/`resources/read` v2 responses too, not just `tools/call` (Truth #5) |
| VERS-08 | 04, 06, 10 | `extensions` capability map supported in negotiation | ✓ SATISFIED | Surfaced via `initialize` (pre-existing) AND now via the live `server/discover` projection (Truth #3, extension id confirmed present in the wire response) |
| VERS-09 | 01, 02, 09 | W3C trace-context surfaced via typed accessors, propagated through dispatch | ✓ SATISFIED | `trace_context()` now populated inside prompt/resource handlers on a v2 connection, proven via the dispatch-entrypoint tests asserting the injected `traceparent` (Truth #1/#5) |

No orphaned requirements — all 9 VERS-01..09 IDs appear in the closure plans' `requirements:` frontmatter (112-09: VERS-01/03/05/07/09; 112-10: VERS-04) and are traced to Phase 112 in REQUIREMENTS.md, which marks all 9 as `[x]` Complete. This verification independently confirms all 9 as SATISFIED against the live codebase (previously 6 of 9 were BLOCKED).

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/server/core.rs` — `extract_request_meta_value` | Matches `CallTool`, `GetPrompt`, `ReadResource` | ✓ VERIFIED | Lines 1747-1769; doc comment states the go-forward policy; exhaustive-variant tripwire test `all_meta_bearing_client_requests_are_extracted` passes |
| `src/server/core.rs` — `handle_get_prompt`/`handle_read_resource` | Thread `protocol_context`+`request_meta` into `RequestHandlerExtra` | ✓ VERIFIED | Lines 802-836 (prompt), 881+ (resource); both call `.with_request_meta(..)`+`.with_protocol_context(..)` |
| `src/server/mod.rs` — `handle_get_prompt`/`handle_read_resource` twins | Same threading at the high-level `Server` dispatch site | ✓ VERIFIED | Lines 1902-1975 (prompt), 2028-2111 (resource); both call `.with_request_meta(..)`+`.with_protocol_context(..)`, mirroring `handle_call_tool` |
| `src/server/streamable_http_server.rs` — `extract_body_method_and_name` | Method-aware logical-name extraction (`resources/read` → `params.uri`) | ✓ VERIFIED | Lines 537-558; branches on method, `"uri"` key for `resources/read`, `"name"` otherwise |
| `src/server/streamable_http_server.rs` — `HttpIngress`/`classify_http_ingress`/`run_v2_header_gate_raw`/`assemble_discover_response_*` | Classify-then-continue live `server/discover` wiring | ✓ VERIFIED | `HttpIngress` enum (line 633), `classify_http_ingress` (655), `run_v2_header_gate_raw` (685), `assemble_discover_response_fast`/`_with_middleware` (1661/1939); wired into the real router via `handle_post_request` → `handle_post_fast_path`/`handle_post_with_middleware` |
| `src/server/core.rs` — `build_discover_response` | Single shared discover projection, no dead-code wrapper | ✓ VERIFIED | `core.rs:1201`, `pub(crate) fn build_discover_response`; `grep -c 'fn dispatch_internal_client_request' src/server/core.rs` = 0; `grep -c 'fn handle_discover' src/server/core.rs` = 0 (only `mod.rs`'s thin delegate remains) |
| `src/server/mod.rs` — `Server::handle_discover` | Production discover caller delegating to `build_discover_response` | ✓ VERIFIED | `mod.rs:1309`, delegates to `crate::server::core::build_discover_response` |
| `src/types/protocol/mod.rs` | Stale `dispatch_internal_client_request` doc reference removed | ✓ VERIFIED | Doc comment now names `classify_internal_method` → `IngressRequest::Internal` → `Server::handle_discover` → `build_discover_response` (lines 607-613); `grep -rn 'dispatch_internal_client_request' src/` returns nothing |
| `tests/v2_required_headers.rs` | Live HTTP prompts/get + resources/read + server/discover matrix, v1 byte-identity | ✓ VERIFIED | 25 tests, all pass (`cargo test --test v2_required_headers --features full`) |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `src/server/core.rs` (`handle_get_prompt`/`handle_read_resource`) | `src/server/cancellation.rs` (`with_protocol_context`) | Threading `protocol_context` into prompt/resource handlers | ✓ WIRED | Confirmed by grep + real-dispatch-entrypoint test `prompt_resource_protocol_context_via_dispatch_core` |
| `src/server/mod.rs` (`handle_get_prompt`/`handle_read_resource`) | `src/server/cancellation.rs` (`with_protocol_context`) | Twin threading on the high-level `Server` | ✓ WIRED | Confirmed by grep + real-dispatch-entrypoint test `prompt_resource_protocol_context_via_dispatch_server` (enters through `process_client_request`) |
| `src/server/streamable_http_server.rs` | `src/types/resources.rs` | `resources/read` logical name from `params.uri` | ✓ WIRED | `extract_body_method_and_name` method-aware branch; live e2e `v2_resources_read_accepts_and_envelopes` sends a uri-only body and gets 200 |
| `src/server/streamable_http_server.rs` (`HttpIngress`) | `src/shared/protocol_helpers.rs` (`parse_request_or_internal`) | Live transport routing for `server/discover` | ✓ WIRED | `classify_http_ingress` calls `parse_request_or_internal`; a non-test production caller exists (both POST parse entrypoints); confirmed reachable via the real axum router |
| `src/server/streamable_http_server.rs` (both POST orchestrators) | `src/server/mod.rs` (`Server::handle_discover`) | Per-path response assembly reached AFTER session/v2-gate/legacy-version/auth | ✓ WIRED | Traced `handle_post_fast_path` end-to-end: `Discover` arm is matched only after `resolve_session_for_request`, `run_v2_header_gate_raw`, `validate_protocol_version`, and `extract_and_validate_auth` all run; proven by `server_discover_requires_auth_when_provider_installed` (401 without auth) and `server_discover_runs_response_middleware` (middleware observes the response) |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `server_discover_v2_returns_capability_projection_with_extensions` result | `capabilities.extensions[DISCOVER_EXTENSION_KEY]` | `Server::handle_discover` → `build_discover_response` → `discover_result_from_capabilities(&self.capabilities, ..)` reading the actually-registered `.capabilities(extensions_capabilities())` on the test server | Yes — asserted `== json!(true)` against a specific registered key, not an empty/static map | ✓ FLOWING |
| `v2_prompts_get_accepts_and_envelopes` / `v2_resources_read_accepts_and_envelopes` result envelope | `resultType`/`serverInfo` | `inject_v2_result_envelope`, gated on live-resolved `protocol_context.era == V2` | Yes — asserted present only on v2 dispatch, absent on v1 (byte-identity tests) | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Library builds (native, full features) | `cargo build --lib --features full` | Clean, 0 warnings | ✓ PASS |
| Library builds (wasm32) | `cargo build --lib --target wasm32-unknown-unknown` | Exit 0 (pre-existing unrelated warnings only) | ✓ PASS |
| Clippy (native, full features, `-D warnings`) | `cargo clippy --lib --features full -- -D warnings` | Clean | ✓ PASS |
| Full lib test suite | `cargo test --lib --features full` | 1229 passed, 0 failed | ✓ PASS |
| Gap B/C targeted unit + dispatch-entrypoint tests | `cargo test --lib --features full -- extract_request_meta_value prompt_resource_protocol_context_via_dispatch all_meta_bearing_client_requests_are_extracted extract_body_method_and_name cross_check_name run_v2_header_gate_raw classify_http_ingress build_discover` | 13 passed, 0 failed | ✓ PASS |
| Gap A live HTTP e2e (real socket, real `StreamableHttpServer`, real `reqwest`) | `cargo test --test v2_required_headers --features full` | 25 passed, 0 failed (incl. 10 `server_discover_*` tests, 5 prompts/resources v2+v1 tests) | ✓ PASS |
| `cargo semver-checks check-release` (independent re-run) | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 checks: 223 pass, 30 skip` — no semver update required | ✓ PASS |
| No production caller of deleted `dispatch_internal_client_request` remains | `grep -rn 'dispatch_internal_client_request' src/` | 0 matches (function fully deleted, not just unreferenced) | ✓ PASS |
| `extract_request_meta_value` handles `GetPrompt`/`ReadResource` | `grep -A18 'fn extract_request_meta_value' src/server/core.rs` | Both arms present, distinct from CallTool arm | ✓ PASS |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| — | — | No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` found in any of the 5 files modified by Plans 112-09/112-10 | — | None — zero-SATD discipline held |
| `src/server/core.rs` | 203, 1119, 1122 | `#[allow(dead_code)]` on the pre-existing `ServerCore` struct and on `ResponseDisposition::InputRequired`/`::Task` | ℹ️ Info | Unrelated to the Gap A/B/C closure — these are intentionally-retained mechanism for Phases 113/114, each with a `// Why:` comment explaining the forward-looking retention. Not a regression of the previously-flagged discover-path dead code (that code is now fully deleted). |

## Cross-Check of Gap Closure Claims (as requested)

**Gap A (VERS-04, server/discover unreachable):** CONFIRMED CLOSED. Read `handle_post_fast_path` end-to-end and traced that a `server/discover` POST is classified as `HttpIngress::Discover` at the same parse step as every other request, then flows through session resolution, the raw-`_meta` v2 header-classification matrix (`run_v2_header_gate_raw`, which calls the SAME `classify_v2_request` function `run_v2_header_gate` uses), legacy-version validation, and `extract_and_validate_auth` before the `Discover` arm is ever reached — this is not an early-return shortcut. Live proof beyond unit tests: `server_discover_requires_auth_when_provider_installed` (a real `AuthProvider` that rejects unauthenticated requests) returns 401 for an unauthenticated v2 discover, and `server_discover_runs_response_middleware` (a real `ServerHttpMiddlewareChain`) observes the discover response — both would fail if discover bypassed the pipeline. The old `#[allow(dead_code)]` `ServerCore::dispatch_internal_client_request`/`handle_discover` wrapper methods are completely deleted from the codebase (`grep -rn` returns zero matches), consolidated into one `build_discover_response` free fn with no dead-code annotation.

**Gap B (VERS-01/03/09, spine wired for tools/call only):** CONFIRMED CLOSED. `extract_request_meta_value` now matches `ClientRequest::GetPrompt`/`ClientRequest::ReadResource` in addition to `CallTool` (core.rs:1747). `handle_get_prompt`/`handle_read_resource` in BOTH `src/server/core.rs` (lines 802, 881) AND `src/server/mod.rs` (lines 1902, 2028) now call `.with_protocol_context(..)`/`.with_request_meta(..)`. This was verified not by grep alone but by two dedicated tests that enter through the REAL dispatch entrypoints — `prompt_resource_protocol_context_via_dispatch_core` (via `core.rs::handle_request_internal`) and `prompt_resource_protocol_context_via_dispatch_server` (via `mod.rs::process_client_request`) — each of which registers a capturing `PromptHandler`/`ResourceHandler`, dispatches a v2 request with a `traceparent` in `_meta`, and asserts `era==Some(Era::V2)`, `client_info().is_some()`, AND `trace_context()` reflects the exact injected `traceparent` string (a dropped `.with_request_meta` call would fail this last assertion even if `.with_protocol_context` alone were present). Both tests also cover the `Some(Era::V1)` and `None` cases to prove they're distinguished, not collapsed. All pass.

**Gap C (HTTP header gate not method-aware for resources/read):** CONFIRMED CLOSED. `extract_body_method_and_name` (streamable_http_server.rs:537) now derives the logical name from `params.uri` specifically for `resources/read` (the field `ReadResourceRequest` actually has — it has no `name` field at all), while `prompts/get`/`tools/call` remain on `params.name`. The regression guard the review demanded is present: the live e2e test `v2_resources_read_accepts_and_envelopes` builds its request body from the real typed `ReadResourceRequest` struct (uri only, no synthetic `params.name` injected) and confirms it is ACCEPTED (200) with `Mcp-Name` set to the URI — this is the exact standards-shaped request that would previously 400.

## Human Verification Required

None — all findings in this report are directly observable and were confirmed by reading source, running the full test suite (native lib + wasm build + live-HTTP integration tests), and re-running the semver gate; no visual/real-time/external-service behavior is in question.

## Gaps Summary

None. All three gaps from the initial verification (`server/discover` production unreachability; the per-request `_meta`/`ProtocolContext` spine being wired for `tools/call` only; the HTTP header gate's non-method-aware logical-name extraction for `resources/read`) are closed with direct, independently-reproduced evidence: source reads at the exact defect sites named in the prior verification, targeted unit tests, real-dispatch-entrypoint tests (not leaf-handler shortcuts), and live HTTP integration tests against a real `StreamableHttpServer` instance including auth-bypass and middleware-bypass proofs for the previously-unreachable `server/discover` path. The full native test suite (1229 tests), the wasm32 build, `cargo clippy -- -D warnings`, and `cargo semver-checks` (still MINOR, no update required) all pass with zero regressions against the previously-verified truths (VERS-02, VERS-06, VERS-08).

REQUIREMENTS.md's `[x]` Complete marks for all 9 VERS-01..09 IDs are now independently confirmed accurate against the live codebase.

---
*Verified: 2026-07-23T02:44:20Z*
*Verifier: Claude (gsd-verifier)*
