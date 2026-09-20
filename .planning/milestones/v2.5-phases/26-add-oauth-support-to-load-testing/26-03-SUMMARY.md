---
phase: 26-add-oauth-support-to-load-testing
plan: 03
subsystem: auth
tags: [oauth, api-key, middleware, loadtest, cli]

# Dependency graph
requires:
  - phase: 26-add-oauth-support-to-load-testing
    plan: 01
    provides: "OAuthHelper, OAuthConfig, create_middleware_chain, HttpMiddlewareChain in pmcp crate"
provides:
  - "HttpMiddlewareChain threaded through McpClient, VU loop, and LoadTestEngine"
  - "OAuth/API-key CLI flags on `cargo pmcp loadtest run` (--oauth-client-id, --oauth-issuer, --oauth-scopes, --oauth-no-cache, --oauth-redirect-port, --api-key)"
  - "resolve_auth_middleware helper that acquires OAuth token once at startup (fail-fast)"
  - "with_http_middleware builder on LoadTestEngine"
affects: [26-04, cargo-pmcp-loadtest]

# Tech tracking
tech-stack:
  added: [pmcp (with oauth + streamable-http features)]
  patterns: [middleware chain threaded via Arc through engine/VU/client, fail-fast auth at startup]

key-files:
  created: []
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/loadtest/client.rs
    - cargo-pmcp/src/loadtest/vu.rs
    - cargo-pmcp/src/loadtest/engine.rs
    - cargo-pmcp/src/commands/loadtest/mod.rs
    - cargo-pmcp/src/commands/loadtest/run.rs
    - cargo-pmcp/src/commands/loadtest/init.rs

key-decisions:
  - "API key takes precedence over OAuth when both provided (simpler, no flow needed)"
  - "Middleware chain is Arc-wrapped and cloned to each VU (shared, not per-VU allocation)"
  - "Auth acquired ONCE at startup before VU spawn -- fail fast on bad config"

patterns-established:
  - "Optional middleware chain pattern: Option<Arc<HttpMiddlewareChain>> threaded from engine to VU to client"
  - "Two-path send_request: middleware path builds HttpRequest then applies chain; non-middleware path uses reqwest directly"

requirements-completed: [OAUTH-04, OAUTH-05]

# Metrics
duration: 6min
completed: 2026-03-01
---

# Phase 26 Plan 03: Wire OAuth Middleware into Loadtest Subsystem Summary

**OAuth and API-key auth wired through loadtest McpClient/VU/engine with CLI flags matching `cargo pmcp test` and fail-fast token acquisition at startup**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-01T02:20:40Z
- **Completed:** 2026-03-01T02:27:32Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Added pmcp dependency with oauth feature to cargo-pmcp, providing HttpMiddlewareChain and OAuthHelper types
- Threaded optional HttpMiddlewareChain (Arc-wrapped) through McpClient, VU loop, and LoadTestEngine with full backward compatibility
- McpClient.send_request applies middleware chain before HTTP POST when present (header injection for auth)
- Added all 6 auth CLI flags to `cargo pmcp loadtest run`: --api-key, --oauth-client-id, --oauth-issuer, --oauth-scopes, --oauth-no-cache, --oauth-redirect-port
- Auth middleware resolved once at startup via resolve_auth_middleware helper -- OAuth token acquired before VU spawn
- All 115 loadtest tests pass, clippy clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Add pmcp dependency and thread HttpMiddlewareChain through McpClient, VU, and engine** - `2795480` (feat)
2. **Task 2: Add OAuth/API-key CLI flags and middleware chain setup in run command** - `ef3bf7c` (feat)

## Files Created/Modified
- `cargo-pmcp/Cargo.toml` - Added pmcp dependency with oauth + streamable-http features
- `cargo-pmcp/src/loadtest/client.rs` - Added http_middleware_chain field, middleware path in send_request, new test
- `cargo-pmcp/src/loadtest/vu.rs` - Threaded middleware chain through vu_loop, vu_loop_inner, try_initialize
- `cargo-pmcp/src/loadtest/engine.rs` - Added middleware chain field, with_http_middleware builder, pass to all vu_loop calls
- `cargo-pmcp/src/commands/loadtest/mod.rs` - Added 6 auth CLI flags to Run variant, pass to execute_run
- `cargo-pmcp/src/commands/loadtest/run.rs` - Added resolve_auth_middleware helper, auth setup before engine, display auth mode
- `cargo-pmcp/src/commands/loadtest/init.rs` - Updated McpClient::new call for new 4th parameter

## Decisions Made
- API key takes precedence over OAuth when both --api-key and --oauth-client-id are provided (simpler path, no browser flow needed)
- Middleware chain is Arc-wrapped and cloned to each VU -- shared across all virtual users, not per-VU allocation
- Auth acquired ONCE at startup before VU spawn -- fail fast on bad auth config without wasting resources

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed McpClient::new call in init.rs**
- **Found during:** Task 2 (compile check)
- **Issue:** `cargo-pmcp/src/commands/loadtest/init.rs` calls `McpClient::new` with 3 args, but Task 1 changed the signature to 4 args
- **Fix:** Added `None` as 4th argument to the init.rs call
- **Files modified:** cargo-pmcp/src/commands/loadtest/init.rs
- **Verification:** cargo build -p cargo-pmcp succeeds
- **Committed in:** ef3bf7c (Task 2 commit)

**2. [Rule 1 - Bug] Added clippy::too_many_arguments allow on try_initialize**
- **Found during:** Task 2 (clippy check)
- **Issue:** Adding http_middleware_chain parameter pushed try_initialize to 9 args, triggering clippy::too_many_arguments
- **Fix:** Added `#[allow(clippy::too_many_arguments)]` attribute
- **Files modified:** cargo-pmcp/src/loadtest/vu.rs
- **Verification:** cargo clippy -p cargo-pmcp -- -D warnings passes
- **Committed in:** ef3bf7c (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for compilation/lint compliance. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- OAuth middleware is now fully wired into the loadtest subsystem
- `cargo pmcp loadtest run --oauth-client-id X --url Y` will acquire a token and inject it into all VU requests
- Ready for Plan 04 (integration tests / documentation)

## Self-Check: PASSED

All 7 modified files verified present. Both task commits (2795480, ef3bf7c) verified in git log.

---
*Phase: 26-add-oauth-support-to-load-testing*
*Completed: 2026-03-01*
