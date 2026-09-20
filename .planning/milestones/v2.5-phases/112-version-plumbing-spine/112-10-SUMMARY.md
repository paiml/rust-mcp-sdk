---
phase: 112-version-plumbing-spine
plan: 10
subsystem: api
tags: [mcp, protocol-context, server-discover, streamable-http, version-plumbing, internal-dispatch]

# Dependency graph
requires:
  - phase: 112-version-plumbing-spine (Plan 09)
    provides: protocol_context threaded into prompt/resource handlers; method-aware HTTP header-gate; classify_v2_request matrix; run_v2_header_gate
  - phase: 112-version-plumbing-spine (Plans 03/05)
    provides: crate-private InternalClientRequest + classify_internal_method + parse_request_or_internal seam; discover_result_from_capabilities + inject_v2_result_envelope projection
provides:
  - server/discover reachable in PRODUCTION on the HTTP transport for v2 (capability + extensions projection)
  - crate-LOCAL HttpIngress::{Public,Discover} classify-then-continue in both POST parse entrypoints (TransportMessage public variants untouched)
  - run_v2_header_gate_raw — raw-_meta counterpart running the SAME classify_v2_request matrix; Server::resolve_discover_protocol_context
  - ONE shared build_discover_response free fn (ServerCore wrappers deleted, no #[allow(dead_code)] on the discover path)
  - v1/non-opted-in server/discover → -32601@200 with original id (deliberate benign D-10 change)
affects: [113-stateless-http, 114-tasks-extension, conformance]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Classify-then-continue: an internally-routed method is classified at ingress then flows through EVERY normal pipeline stage; routed only at the final per-path response-assembly step (no early-return bypass)"
    - "A crate-LOCAL ingress enum (HttpIngress) routes an internal method without adding a variant to the semver-sensitive public TransportMessage"
    - "A raw-_meta v2 header-gate counterpart reuses the SAME classify_v2_request matrix as the parsed-request gate (one matrix, two callers)"
    - "One shared projection free fn (build_discover_response) consumed by the production caller AND the migrated tests — no dead-code wrapper"

key-files:
  created: []
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/streamable_http_server.rs
    - src/types/protocol/mod.rs
    - tests/v2_required_headers.rs

key-decisions:
  - "server/discover uses classify-then-continue (NOT the previously-proposed early-return interception): the Discover arm is reached only AFTER session resolution, the v2 header matrix, legacy-version validation, and auth — proven by auth-provider 401 + response-middleware e2e"
  - "HttpIngress is crate-LOCAL so TransportMessage's ~30 public match sites are untouched; cargo-semver-checks stays MINOR (223 pass, no update required)"
  - "v1 / non-opted-in server/discover returns JSON-RPC -32601 at HTTP 200 with the original id — a DELIBERATE, documented, benign change from the pre-112 incidental PARSE_ERROR 400 (id:null), reconciled against the milestone byte-identity invariant because no conforming v1 client sends server/discover (D-10)"
  - "The two ServerCore wrappers (dispatch_internal_client_request/handle_discover) are DELETED and consolidated into one build_discover_response free fn, so no #[allow(dead_code)] survives clippy -D warnings"
  - "Scope: HTTP transport only (VERS-04 SC#3 = at least one real transport); stdio/adapters/WASI discover is a separate semver-reviewed follow-up (would need a TransportMessage-level internal variant)"

patterns-established:
  - "A future internally-routed method is a compile-time tripwire in classify_http_ingress (inner match exhaustive over InternalClientRequest)"
  - "Box::pin the middleware dispatch future when a new per-path arm grows it past clippy's large_future threshold (mirrors the fast-path handler)"

requirements-completed: [VERS-04]

# Metrics
duration: 50min
completed: 2026-07-23
---

# Phase 112 Plan 10: server/discover Live on HTTP (Gap A Closure) Summary

**Made `server/discover` (VERS-04, ROADMAP SC#3) reachable in production on the streamable-HTTP transport via a classify-then-continue design: a crate-LOCAL `HttpIngress::{Public,Discover}` classification in both POST parse entrypoints lets a `server/discover` request flow through the SAME pipeline (session → the SAME `classify_v2_request` matrix via a raw-_meta gate → legacy-version → auth → dispatch → event store → per-path assembly), returning the capability + extensions projection on v2 and `-32601@200` with the original id on v1/non-opted-in — never an early-return bypass.**

## Performance

- **Duration:** ~50 min
- **Completed:** 2026-07-23
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Closed verification Gap A: `server/discover` was 100% unreachable in production (every transport called `parse_request` → `-32601`); it now answers on a v2 HTTP connection, exposing the `extensions` map.
- Consolidated the discover projection into ONE shared `build_discover_response` free fn; DELETED both `ServerCore::handle_discover` and `ServerCore::dispatch_internal_client_request` wrappers (removing their `#[allow(dead_code)]`), and added the production `Server::handle_discover` delegate.
- Introduced a crate-LOCAL `HttpIngress::{Public(TransportMessage), Discover{id, raw_meta}}` classification in BOTH POST parse entrypoints (fast + middleware), leaving `TransportMessage`'s public variants untouched (semver MINOR).
- Added `run_v2_header_gate_raw` — a raw-_meta counterpart running the SAME `classify_v2_request` matrix as `run_v2_header_gate`, with `body_method` fixed to `"server/discover"`; non-opted-in servers short-circuit to Passthrough BEFORE inspecting the v2 `_meta` (D-04). Added `Server::resolve_discover_protocol_context`.
- Wired per-path response assembly (`assemble_discover_response_fast` / `assemble_discover_response_with_middleware`) reusing `store_response_event` + the existing response builders + `apply_v2_outbound_headers`, preserving the original request id.
- Cleaned the stale Plan 07/08 comments and the `dispatch_internal_client_request` doc reference in `protocol/mod.rs`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Consolidate discover projection into build_discover_response; delete ServerCore wrappers; migrate tests; clean stale docs** — `78b49421` (refactor)
2. **Task 2: Classify-then-continue HTTP wiring (HttpIngress, run_v2_header_gate_raw, per-path assembly)** — `d06c2653` (feat)
3. **Task 3: Live e2e discover tests (v2 projection+extensions, rejection matrix, auth/middleware bypass proofs)** — `18ab1387` (test)

Plus a `style` fixup after the quality gate: `fcf12203` (Box::pin the middleware dispatch future past clippy's large_future threshold, fmt, doc backticks, redundant match guard).

## Files Created/Modified
- `src/server/core.rs` — deleted the two ServerCore wrappers; added the shared `pub(crate) fn build_discover_response`; migrated the `#[cfg(test)]` callers to exercise it directly (v2 projection incl. extensions, v1 -32601, no init-state mutation).
- `src/server/mod.rs` — added `Server::handle_discover` (delegate to `build_discover_response`) and `Server::resolve_discover_protocol_context` (raw-_meta era resolution with the D-04 non-opted-in short-circuit).
- `src/server/streamable_http_server.rs` — `HttpIngress` enum + `classify_http_ingress` + `run_v2_header_gate_raw` + `assemble_discover_response_fast`/`_with_middleware`; both POST parse entrypoints return `HttpIngress`; both orchestrators branch the v2 gate and final dispatch on the ingress variant; `Box::pin` on the grown middleware dispatch future; unit tests for the raw gate cells + ingress classification + raw-body no-panic proptest.
- `src/types/protocol/mod.rs` — updated the stale `InternalClientRequest` doc comment to name the new live seam (`classify_internal_method` → `IngressRequest::Internal` → `HttpIngress::Discover` → `Server::handle_discover` → `build_discover_response`).
- `tests/v2_required_headers.rs` — v2-opted-in `build_server` pre-seeds a `capabilities.extensions` entry; 10 live-HTTP discover tests (v2 projection+extensions+id, four reject cells, v1 -32601@200, non-opted-in -32601, auth-provider 401, valid-token served, response-middleware pass-through).

## Decisions Made
- Classify-then-continue replaces the cross-AI-rejected early-return interception: the Discover arm runs only after session/matrix/legacy-version/auth, closing the auth-bypass and session-bypass gaps (findings #1/#3/#4).
- `HttpIngress` is crate-local; `TransportMessage`'s public variants are untouched, keeping the milestone additive (semver MINOR — 223 checks pass, no update required).
- v1/non-opted-in discover standardizes on `-32601@200` with the original id (D-10), a deliberate benign change from the pre-112 incidental `PARSE_ERROR` 400 (`id:null`), documented in code at the assembly point.
- Extensions in the e2e test are seeded via `.capabilities(..)` (a manual extensions map) rather than `.skills(..)` — the `skills` feature is NOT in `full`, and the verify command runs `--features full`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Box::pin the middleware dispatch future**
- **Found during:** `make quality-gate` (clippy `-D warnings`) after Task 2/3.
- **Issue:** Adding the discover per-path arm grew the `dispatch_message_with_middleware` future to 17096 bytes, tripping `clippy::large_futures`.
- **Fix:** `Box::pin(dispatch_message_with_middleware(..))` in `handle_post_with_middleware`, mirroring the existing fast-path `Box::pin` precedent. No behavior change.
- **Files modified:** src/server/streamable_http_server.rs
- **Commit:** fcf12203

**2. [Rule 3 - Blocking] Extensions seeded via `.capabilities(..)` not `.skills(..)`**
- **Found during:** Task 3 compile.
- **Issue:** The plan suggested `.with_extension`/`.skills` to populate the discover extensions map, but the high-level `ServerBuilder` has no `with_extension`, and `.skills`/`Skill`/`Skills` are gated behind the `skills` feature — NOT part of `full` (the verify feature set).
- **Fix:** Added an `extensions_capabilities()` helper building a `ServerCapabilities` with a manual `io.example/experimental` extension, applied via `.capabilities(..)` BEFORE the handlers (which layer their sub-capabilities on top), so the extensions survive.
- **Files modified:** tests/v2_required_headers.rs

Minor clippy fixups (redundant match guard, doc backticks) were corrected in-place before the `fcf12203` commit — lint fixes on new code, not behavior changes.

## Issues Encountered
- The STATE.md `Current Position` plan counter was out of sync with the phase's real progress before this plan (said "Plan 2 of 10" while 9 plans were complete); `state advance-plan` incremented that stale counter, so the Position/frontmatter were corrected manually to reflect all 10 plans complete.

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- Phase 112 (version-plumbing spine) gap-closure is DONE: all three verification gaps (A: discover reachability; B: prompt/resource plumbing; C: resources/read header gate) are closed. `server/discover` is the stateless replacement for `initialize`'s capability exchange (D-09) and now answers on a real v2 HTTP connection.
- All changes are crate-private/local (no public API surface changed); `cargo semver-checks` classifies MINOR (223 pass, no update required); wasm `cargo build --lib --target wasm32-unknown-unknown` green; `make quality-gate` green (fmt, clippy pedantic+nursery, build, tests, examples).
- Non-HTTP discover (stdio actor / ProtocolHandler adapter / WASI) remains a documented follow-up requiring a semver-reviewed `TransportMessage`-level internal variant — out of scope for VERS-04 SC#3 (which requires at least one real transport).

## Self-Check: PASSED

All 5 modified files present; all task commits (78b49421, d06c2653, 18ab1387) and the style fixup (fcf12203) exist in git history.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-23*
