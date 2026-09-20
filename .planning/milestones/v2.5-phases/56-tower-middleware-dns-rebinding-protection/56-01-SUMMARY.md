---
phase: 56-tower-middleware-dns-rebinding-protection
plan: 01
subsystem: security
tags: [tower, middleware, dns-rebinding, cors, owasp, cve-2025-66414]

# Dependency graph
requires:
  - phase: 54-protocol-2025-11-25-support
    provides: "Protocol types and streamable-http feature infrastructure"
provides:
  - "AllowedOrigins config with auto-detect for localhost aliases"
  - "DnsRebindingLayer for Host/Origin header validation (403 rejection)"
  - "SecurityHeadersLayer for OWASP response headers (nosniff, DENY, no-store)"
  - "to_cors_allow_origin() bridge to tower_http AllowOrigin"
affects: [56-02-PLAN, streamable-http-server, axum-router]

# Tech tracking
tech-stack:
  added: [tower 0.5, tower-http 0.6 (cors + set-header features)]
  patterns: [tower Layer/Service pattern with clone-and-swap, AllowedOrigins as single source of truth]

key-files:
  created:
    - src/server/tower_layers/mod.rs
    - src/server/tower_layers/dns_rebinding.rs
    - src/server/tower_layers/security_headers.rs
  modified:
    - Cargo.toml
    - src/server/mod.rs

key-decisions:
  - "AllowedOrigins auto-detects localhost/127.0.0.1/[::1] for loopback and unspecified bind addresses"
  - "Missing Origin header is permitted (non-browser clients like curl omit it)"
  - "No HSTS header per D-12 (transport-layer concern for reverse proxies)"
  - "tower and tower-http gated behind streamable-http feature (not new feature flags)"

patterns-established:
  - "Tower clone-and-swap pattern: clone inner, swap into self, move into async block"
  - "AllowedOrigins as single config consumed by both DnsRebindingLayer and to_cors_allow_origin()"

requirements-completed: [TOWER-MIDDLEWARE, DNS-REBINDING]

# Metrics
duration: 6min
completed: 2026-03-21
---

# Phase 56 Plan 01: Tower Middleware DNS Rebinding Protection Summary

**AllowedOrigins config with auto-detected localhost aliases, DnsRebindingLayer validating Host/Origin headers with 403 rejection, and SecurityHeadersLayer adding OWASP response headers (nosniff, DENY, no-store)**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-21T02:34:34Z
- **Completed:** 2026-03-21T02:40:25Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- AllowedOrigins with from_bind_addr/explicit/localhost constructors and hostname extraction for Host validation
- DnsRebindingLayer validates Host (always) and Origin (when present), returns 403 on mismatch per CVE-2025-66414
- SecurityHeadersLayer adds X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store with per-header opt-out
- to_cors_allow_origin() bridges AllowedOrigins to tower_http AllowOrigin for CORS layer consumption
- 21 unit tests covering all validation paths and service integration
- 975 existing tests pass with zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add tower deps, create AllowedOrigins and DnsRebindingLayer** - `9b702b1` (feat)
2. **Task 2: Create SecurityHeadersLayer** - `bcd487b` (feat)

**Plan metadata:** (pending) (docs: complete plan)

## Files Created/Modified
- `Cargo.toml` - Added tower 0.5 and tower-http 0.6 as optional deps behind streamable-http feature
- `src/server/mod.rs` - Added tower_layers module declaration gated behind streamable-http feature
- `src/server/tower_layers/mod.rs` - Module declarations and re-exports for dns_rebinding and security_headers
- `src/server/tower_layers/dns_rebinding.rs` - AllowedOrigins config, DnsRebindingLayer/Service, 15 tests
- `src/server/tower_layers/security_headers.rs` - SecurityHeadersLayer/Service with OWASP defaults, 6 tests

## Decisions Made
- AllowedOrigins auto-detects localhost/127.0.0.1/[::1] for loopback and unspecified bind addresses
- Missing Origin header is permitted (non-browser clients like curl omit it)
- No HSTS header per D-12 (transport-layer concern handled by reverse proxy)
- tower and tower-http gated behind existing streamable-http feature (no new feature flags needed)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tower layers are building blocks ready for Plan 02's `pmcp::axum::router()` integration
- AllowedOrigins::to_cors_allow_origin() ready for CorsLayer construction
- DnsRebindingLayer and SecurityHeadersLayer ready to compose into Axum router middleware stack

## Self-Check: PASSED

All created files verified on disk. All commit hashes verified in git log.

---
*Phase: 56-tower-middleware-dns-rebinding-protection*
*Completed: 2026-03-21*
