---
phase: 56-tower-middleware-dns-rebinding-protection
plan: 03
subsystem: server
tags: [tower, cors, dns-rebinding, security-headers, axum, middleware]

# Dependency graph
requires:
  - phase: 56-01
    provides: DnsRebindingLayer, SecurityHeadersLayer, AllowedOrigins types
  - phase: 56-02
    provides: build_mcp_router(), make_server_state() extraction, axum_router module
provides:
  - Tower layer security stack in StreamableHttpServer::start()
  - Single CORS implementation via CorsLayer (no hand-rolled add_cors_headers)
  - Pre-resolved AllowedOrigins in ServerState (zero per-request allocation)
  - Simplified handler function signatures (no CORS parameters)
affects: [streamable-http, axum-router, examples, integration-tests]

# Tech tracking
tech-stack:
  added: []
  patterns: [tower-layer-stack-in-start, pre-resolved-origins-in-state]

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs
    - src/server/axum_router.rs
    - tests/streamable_http_server_tests.rs
    - tests/streamable_http_integration.rs
    - tests/streamable_http_oauth_integration.rs
    - tests/sse_middleware_integration.rs
    - tests/streamable_http_spec_compliance.rs
    - examples/23_streamable_http_server_stateless.rs

key-decisions:
  - "AllowedOrigins resolved once in make_server_state(), stored as field -- zero per-request allocation"
  - "CorsLayer handles all CORS including preflight OPTIONS -- no hand-rolled add_cors_headers"
  - "Handler signatures simplified by removing allowed_origins/request_origin params -- Tower layers handle CORS at middleware level"
  - "Net reduction of 166 lines (1589 -> 1423) in streamable_http_server.rs"

patterns-established:
  - "Tower layer stack pattern: both start() and router() apply identical SecurityHeadersLayer + DnsRebindingLayer + CorsLayer"
  - "Pre-resolved state pattern: AllowedOrigins resolved at ServerState construction, not per-request"

requirements-completed: [TOWER-MIDDLEWARE, DNS-REBINDING]

# Metrics
duration: 14min
completed: 2026-03-21
---

# Phase 56 Plan 03: Gap Closure Summary

**Unified Tower layer security in StreamableHttpServer::start() -- deleted hand-rolled CORS, pre-resolved AllowedOrigins, 166 lines removed**

## Performance

- **Duration:** 14 min
- **Started:** 2026-03-21T03:37:59Z
- **Completed:** 2026-03-21T03:52:00Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- StreamableHttpServer::start() now applies DnsRebindingLayer + SecurityHeadersLayer + CorsLayer -- same stack as pmcp::axum::router()
- Deleted add_cors_headers() function and all 13 call sites -- single CORS implementation via CorsLayer
- Deleted handle_options() handler and OPTIONS route -- CorsLayer handles preflight automatically
- Pre-resolved AllowedOrigins in ServerState eliminates per-request config clone (was 5 call sites)
- Simplified 7 function signatures by removing CORS parameters
- Net removal of 166 lines from streamable_http_server.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: Hoist AllowedOrigins into ServerState** - `ab3db92` (feat)
2. **Task 2: Apply Tower layers in start(), delete add_cors_headers** - `c7a08c1` (feat)

## Files Created/Modified
- `src/server/streamable_http_server.rs` - Added allowed_origins to ServerState, applied Tower layers in start(), deleted add_cors_headers/handle_options/resolve_allowed_origins, simplified all handler signatures
- `src/server/axum_router.rs` - Updated router_with_config() to pass resolved origins through server_config
- `tests/streamable_http_server_tests.rs` - Added missing allowed_origins: None field
- `tests/streamable_http_integration.rs` - Added missing allowed_origins: None field (3 sites)
- `tests/streamable_http_oauth_integration.rs` - Added missing allowed_origins: None field (5 sites)
- `tests/sse_middleware_integration.rs` - Added missing allowed_origins: None field (4 sites)
- `tests/streamable_http_spec_compliance.rs` - Added missing allowed_origins: None field
- `examples/23_streamable_http_server_stateless.rs` - Added missing allowed_origins: None field

## Decisions Made
- AllowedOrigins resolved once in make_server_state(), stored as field -- avoids per-request config.allowed_origins.clone().unwrap_or_else() overhead
- CorsLayer handles all CORS including preflight OPTIONS -- no hand-rolled add_cors_headers needed
- Handler signatures simplified by removing allowed_origins/request_origin params -- 7 functions simplified (create_error_response, validate_headers, build_response, process_init_session, validate_non_init_session, validate_protocol_version, extract_and_validate_auth)
- StreamableHttpServer::with_config() refactored to delegate to make_server_state() -- single construction path

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing allowed_origins field in test/example struct literals**
- **Found during:** Task 1 (after adding field to ServerState and compiling tests)
- **Issue:** Plan 56-02 added `allowed_origins: Option<AllowedOrigins>` to StreamableHttpServerConfig but did not update test/example files that construct struct literals without the field. 14 struct literal sites across 6 files failed to compile.
- **Fix:** Added `allowed_origins: None,` to all 14 struct literal sites
- **Files modified:** tests/streamable_http_server_tests.rs, tests/streamable_http_integration.rs, tests/streamable_http_oauth_integration.rs, tests/sse_middleware_integration.rs, tests/streamable_http_spec_compliance.rs, examples/23_streamable_http_server_stateless.rs
- **Verification:** cargo check --features streamable-http passes, all 790 lib tests pass
- **Committed in:** ab3db92 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Pre-existing gap from Plan 56-02 that was not caught because integration tests were not compiled under the streamable-http feature flag at that time. Fix is trivial and necessary.

## Issues Encountered
None -- plan executed as specified after the blocking deviation was resolved.

## User Setup Required
None -- no external service configuration required.

## Next Phase Readiness
- Phase 56 fully complete: all 3 plans shipped
- Tower middleware stack (DNS rebinding + security headers + CORS) applied to both the StreamableHttpServer path and the pmcp::axum::router() path
- 790 lib tests passing, example 55 compiles unchanged
- Ready for next milestone work

---
*Phase: 56-tower-middleware-dns-rebinding-protection*
*Completed: 2026-03-21*
