---
phase: 56-tower-middleware-dns-rebinding-protection
plan: 02
subsystem: api
tags: [axum, tower, cors, dns-rebinding, security, middleware]

# Dependency graph
requires:
  - phase: 56-tower-middleware-dns-rebinding-protection (Plan 01)
    provides: AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer in tower_layers module
provides:
  - pmcp::axum::router() one-liner API for secure MCP server hosting
  - pmcp::axum::router_with_config() with explicit AllowedOrigins override
  - RouterConfig struct for production deployment customization
  - Origin-locked CORS in StreamableHttpServer (replaces wildcard *)
  - Crate-root re-exports for AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer
affects: [examples, documentation, deployment-guides, streamable-http-server]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "resolve_allowed_origins() helper for consistent CORS origin resolution"
    - "Origin-locked CORS: reflect request Origin when allowed, omit otherwise"
    - "build_mcp_router() + make_server_state() extracted for shared router construction"

key-files:
  created:
    - src/server/axum_router.rs
  modified:
    - src/server/streamable_http_server.rs
    - src/server/tower_layers/dns_rebinding.rs
    - src/server/mod.rs
    - src/lib.rs

key-decisions:
  - "Extracted build_mcp_router() and make_server_state() as pub(crate) from StreamableHttpServer::start() for shared use by axum_router"
  - "Made ServerState pub(crate) to allow axum_router.rs cross-module access"
  - "Origin-locked CORS: add_cors_headers reflects request Origin when allowed, omits Access-Control-Allow-Origin entirely for disallowed/missing origins"
  - "AllowedOrigins defaults to localhost() when config.allowed_origins is None"
  - "handle_options now takes State and HeaderMap to thread origin-locked CORS through preflight"

patterns-established:
  - "resolve_allowed_origins(): centralized helper resolving Optional AllowedOrigins from config with localhost fallback"
  - "pmcp::axum::router(server) one-liner pattern: returns Router with DNS rebinding + security headers + CORS layers"

requirements-completed: [AXUM-ADAPTER, DNS-REBINDING, TOWER-MIDDLEWARE]

# Metrics
duration: 12min
completed: 2026-03-21
---

# Phase 56 Plan 02: Axum Router Integration + Origin-Locked CORS Summary

**pmcp::axum::router() one-liner API with DNS rebinding + security headers + origin-locked CORS, replacing wildcard * in StreamableHttpServer**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-21T02:43:45Z
- **Completed:** 2026-03-21T02:55:56Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created pmcp::axum::router(server) one-liner that returns axum::Router with DnsRebindingLayer + SecurityHeadersLayer + CorsLayer
- Replaced wildcard Access-Control-Allow-Origin: * with origin-locked CORS throughout StreamableHttpServer (CVE-pattern fix)
- Added pmcp::axum re-export module and crate-root re-exports for AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer
- 790 tests passing with zero regressions; example 55 compiles unchanged

## Task Commits

Each task was committed atomically:

1. **Task 1: Create axum_router.rs with router() and router_with_config()** - `f550146` (feat)
2. **Task 2: Update StreamableHttpServer CORS + lib.rs re-exports + verify compatibility** - `14ff873` (feat)

## Files Created/Modified
- `src/server/axum_router.rs` - RouterConfig, router(), router_with_config() with Tower layer composition
- `src/server/streamable_http_server.rs` - Origin-locked add_cors_headers, allowed_origins config field, build_mcp_router/make_server_state extraction
- `src/server/tower_layers/dns_rebinding.rs` - Added AllowedOrigins::origins() public accessor
- `src/server/mod.rs` - Added axum_router module declaration
- `src/lib.rs` - Added pmcp::axum re-export module and crate-root tower_layers re-exports

## Decisions Made
- Extracted `build_mcp_router()` and `make_server_state()` as `pub(crate)` from `StreamableHttpServer::start()` so both the existing serving path and the new `axum_router` can share the same Router construction logic
- Made `ServerState` `pub(crate)` to allow cross-module access from `axum_router.rs`
- Origin-locked CORS reflects the request's `Origin` header when it appears in the allowed set; omits `Access-Control-Allow-Origin` entirely for disallowed or missing origins (browser blocks the response naturally)
- `handle_options` now takes `State(state)` and `HeaderMap` to properly thread origin-locked CORS through CORS preflight responses
- `AllowedOrigins` defaults to `localhost()` when `config.allowed_origins` is `None`, matching the safest default for development

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 56 complete: Tower middleware layers built (Plan 01) and integrated into both serving paths (Plan 02)
- DNS rebinding protection available via DnsRebindingLayer for custom Tower stacks
- One-liner API available via pmcp::axum::router(server) for default secure configuration
- Ready for documentation/examples showing the new API

---
*Phase: 56-tower-middleware-dns-rebinding-protection*
*Completed: 2026-03-21*
