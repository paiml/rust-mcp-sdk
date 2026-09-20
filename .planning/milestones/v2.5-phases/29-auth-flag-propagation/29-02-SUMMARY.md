---
phase: 29-auth-flag-propagation
plan: 02
subsystem: auth
tags: [clap, oauth, bearer-token, cli-flags, middleware, server-tester]

requires:
  - phase: 29-auth-flag-propagation
    plan: 01
    provides: AuthFlags struct, AuthMethod enum, resolve_auth_middleware() shared module
provides:
  - AuthFlags flattened into test check, run, generate, apps variants
  - check.rs and apps.rs wired with resolve_auth_middleware -> ServerTester
  - run.rs and generate.rs accept auth flags with degraded-support warning
  - Loadtest migrated from 6 inline fields to shared AuthFlags
  - Local resolve_auth_middleware() deleted from loadtest/run.rs
affects: [29-03, loadtest, test]

tech-stack:
  added: []
  patterns: [auth middleware wiring for ServerTester, auth warning for library functions without passthrough support]

key-files:
  created: []
  modified:
    - cargo-pmcp/src/commands/test/mod.rs
    - cargo-pmcp/src/commands/test/check.rs
    - cargo-pmcp/src/commands/test/apps.rs
    - cargo-pmcp/src/commands/test/run.rs
    - cargo-pmcp/src/commands/test/generate.rs
    - cargo-pmcp/src/commands/loadtest/mod.rs
    - cargo-pmcp/src/commands/loadtest/run.rs
    - cargo-pmcp/src/commands/flags.rs
    - cargo-pmcp/src/commands/auth.rs
    - cargo-pmcp/src/commands/connect.rs
    - cargo-pmcp/src/commands/dev.rs

key-decisions:
  - "Middleware-only auth for ServerTester (pass None for api_key param, middleware for http_middleware_chain) to avoid double auth headers"
  - "Warning approach for run/generate: accept flags but warn that library functions do not yet support auth passthrough"
  - "connect.rs and dev.rs fixed as blocking pre-existing issues (Rule 3) to enable compilation"

patterns-established:
  - "auth middleware wiring: auth_flags.resolve() -> auth::resolve_auth_middleware() -> ServerTester::new(None, middleware)"
  - "degraded auth support: resolve + warn when downstream library function lacks auth parameter"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03]

duration: 7min
completed: 2026-03-13
---

# Phase 29 Plan 02: Test/Loadtest Auth Propagation Summary

**AuthFlags flattened into all 4 test subcommands with check/apps fully wired via middleware, run/generate with degraded-support warnings, and loadtest migrated from 6 inline fields to shared AuthFlags**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-13T01:08:53Z
- **Completed:** 2026-03-13T01:16:00Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- AuthFlags flattened into Apps, Check, Run, Generate variants in test/mod.rs with full dispatch wiring
- check.rs and apps.rs resolve auth via shared middleware and pass to ServerTester (api_key=None, middleware=resolved)
- run.rs and generate.rs resolve auth and warn if configured (library functions lack auth passthrough)
- Loadtest Run variant migrated from 6 inline auth fields to `#[command(flatten)] AuthFlags`
- Local resolve_auth_middleware() deleted from loadtest/run.rs (66 lines removed)
- Removed allow(dead_code) from AuthMethod, AuthFlags::resolve(), resolve_auth_middleware() -- now consumed
- All 117 loadtest tests pass, cargo clippy clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Flatten AuthFlags into test subcommands and wire check/apps handlers** - `671924c` (feat)
2. **Task 2: Wire auth into run/generate handlers, migrate loadtest to shared AuthFlags** - `a4d7ce0` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/test/mod.rs` - AuthFlags flattened into 4 variants, dispatch updated to pass auth_flags
- `cargo-pmcp/src/commands/test/check.rs` - Auth resolution and middleware wiring for ServerTester
- `cargo-pmcp/src/commands/test/apps.rs` - Auth resolution and middleware wiring for ServerTester
- `cargo-pmcp/src/commands/test/run.rs` - Auth resolution with degraded-support warning
- `cargo-pmcp/src/commands/test/generate.rs` - Auth resolution with degraded-support warning
- `cargo-pmcp/src/commands/loadtest/mod.rs` - 6 inline fields replaced with #[command(flatten)] AuthFlags
- `cargo-pmcp/src/commands/loadtest/run.rs` - Shared auth::resolve_auth_middleware, local copy deleted
- `cargo-pmcp/src/commands/flags.rs` - Removed allow(dead_code) from AuthMethod and resolve()
- `cargo-pmcp/src/commands/auth.rs` - Removed allow(dead_code) from resolve_auth_middleware()
- `cargo-pmcp/src/commands/connect.rs` - Accept auth_flags parameter (blocking fix)
- `cargo-pmcp/src/commands/dev.rs` - Pass default AuthFlags to connect::execute (blocking fix)

## Decisions Made
- Used middleware-only approach for ServerTester (pass None for api_key, middleware for http_middleware_chain) to avoid Pitfall 1 (double auth headers)
- Used warning approach for run/generate: library functions (run_scenario_with_transport, generate_scenarios_with_transport) lack auth parameters, so we accept flags and warn users to use `cargo pmcp test check` for authenticated servers
- Connected connect.rs and dev.rs as blocking fixes since pre-existing main.rs dispatch already referenced auth_flags

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] connect.rs missing auth_flags parameter**
- **Found during:** Task 1 (cargo check)
- **Issue:** main.rs already passed auth_flags to connect::execute but the handler didn't accept it (pre-existing inconsistency from prior plan work)
- **Fix:** Added auth_flags parameter to connect::execute, applied linter-reformatted full auth wiring
- **Files modified:** cargo-pmcp/src/commands/connect.rs
- **Verification:** cargo check -p cargo-pmcp passes
- **Committed in:** 671924c (Task 1 commit)

**2. [Rule 3 - Blocking] dev.rs call to connect::execute missing auth argument**
- **Found during:** Task 1 (cargo check)
- **Issue:** dev.rs called connect::execute with 4 args but updated signature requires 5 (includes auth_flags)
- **Fix:** Construct default AuthFlags with all None/false/8080 values for dev connect (dev doesn't use auth)
- **Files modified:** cargo-pmcp/src/commands/dev.rs
- **Verification:** cargo check -p cargo-pmcp passes
- **Committed in:** 671924c (Task 1 commit)

**3. [Rule 3 - Blocking] run.rs and generate.rs already had auth_flags placeholder from prior work**
- **Found during:** Task 2
- **Issue:** Files already had `auth_flags: &AuthFlags` in signature with `let _ = auth_flags;` placeholder (from prior plan partial work)
- **Fix:** Replaced placeholder with proper auth resolution and warning logic
- **Files modified:** cargo-pmcp/src/commands/test/run.rs, cargo-pmcp/src/commands/test/generate.rs
- **Verification:** cargo check passes, auth warning prints when auth flags configured
- **Committed in:** a4d7ce0 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All fixes necessary for compilation due to pre-existing partial work from prior planning sessions. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plans 01 and 02 complete: auth infrastructure + test/loadtest wiring done
- Plan 03 remaining: preview, schema export, and connect commands need auth wiring
- All AuthFlags and shared auth module consumed and battle-tested
- resolve_api_key() still has allow(dead_code) -- will be consumed by Plan 03 if needed

## Self-Check: PASSED

- [x] test/mod.rs has AuthFlags in all 4 variants
- [x] check.rs resolves auth and passes middleware to ServerTester
- [x] apps.rs resolves auth and passes middleware to ServerTester
- [x] run.rs warns when auth configured but passthrough not supported
- [x] generate.rs warns when auth configured but passthrough not supported
- [x] loadtest/mod.rs uses #[command(flatten)] AuthFlags (no inline fields)
- [x] loadtest/run.rs uses shared auth::resolve_auth_middleware
- [x] Local resolve_auth_middleware deleted from loadtest/run.rs
- [x] All 117 loadtest tests pass
- [x] cargo clippy -p cargo-pmcp -- -D warnings clean
- [x] All 5 commands show auth flags in --help output
- [x] Commit 671924c verified
- [x] Commit a4d7ce0 verified

---
*Phase: 29-auth-flag-propagation*
*Completed: 2026-03-13*
