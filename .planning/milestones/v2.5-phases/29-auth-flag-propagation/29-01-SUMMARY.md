---
phase: 29-auth-flag-propagation
plan: 01
subsystem: auth
tags: [clap, oauth, bearer-token, cli-flags, middleware]

requires:
  - phase: 28-flag-normalization
    provides: SharedFlags pattern (ServerFlags, FormatValue, GlobalFlags)
provides:
  - AuthFlags struct with resolve() -> AuthMethod enum
  - AuthMethod enum (None, ApiKey, OAuth variants)
  - resolve_auth_middleware() shared function
  - resolve_api_key() helper function
affects: [29-02, 29-03, loadtest, test, preview, schema, connect]

tech-stack:
  added: []
  patterns: [AuthFlags flatten pattern, AuthMethod enum dispatch, shared auth middleware resolution]

key-files:
  created:
    - cargo-pmcp/src/commands/auth.rs
  modified:
    - cargo-pmcp/src/commands/flags.rs
    - cargo-pmcp/src/commands/mod.rs

key-decisions:
  - "allow(dead_code) on AuthMethod, resolve(), resolve_auth_middleware(), resolve_api_key() until Plans 02/03 add consumers (matching Phase 28 precedent for GlobalFlags.verbose)"
  - "Parser import scoped to #[cfg(test)] module to avoid unused import warning in production code"
  - "AuthMethod derives PartialEq for ergonomic assert_eq! in tests"

patterns-established:
  - "AuthFlags::resolve() -> AuthMethod: typed enum dispatch instead of raw field inspection"
  - "resolve_auth_middleware(url, &AuthMethod) -> Option<Arc<HttpMiddlewareChain>>: single auth wiring point for all commands"
  - "resolve_api_key(&AuthMethod) -> Option<&str>: extraction helper for ServerTester api_key parameter"

requirements-completed: []

duration: 4min
completed: 2026-03-13
---

# Phase 29 Plan 01: Auth Flag Infrastructure Summary

**AuthFlags struct with resolve() enum dispatch, shared resolve_auth_middleware() in auth.rs, and 7 unit tests covering all input combinations**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-13T01:01:16Z
- **Completed:** 2026-03-13T01:05:40Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- AuthFlags struct defined in flags.rs with 6 fields, env var bindings, and clap conflicts_with mutual exclusion
- AuthMethod enum with None, ApiKey, OAuth variants and resolve() dispatch logic
- Shared auth.rs module with resolve_auth_middleware() extracted from loadtest/run.rs pattern
- resolve_api_key() helper for ServerTester consumers
- 7 passing unit tests covering None, ApiKey, OAuth (with/without scopes, no_cache), and clap conflict rejection

## Task Commits

Each task was committed atomically:

1. **Task 1: Define AuthFlags, AuthMethod, and resolve() in flags.rs** - `1a82dbc` (test: TDD RED), `7629ca0` (feat: TDD GREEN+REFACTOR)
2. **Task 2: Create shared auth.rs module with resolve_auth_middleware()** - `9a46e84` (feat)

_Note: Task 1 followed TDD with separate RED and GREEN+REFACTOR commits_

## Files Created/Modified
- `cargo-pmcp/src/commands/flags.rs` - AuthFlags struct, AuthMethod enum, resolve() method, 7 unit tests
- `cargo-pmcp/src/commands/auth.rs` - resolve_auth_middleware() and resolve_api_key() shared functions
- `cargo-pmcp/src/commands/mod.rs` - pub mod auth declaration

## Decisions Made
- Used `#[allow(dead_code)]` on new types/functions following Phase 28 precedent (GlobalFlags.verbose had same pattern until consumers were added in later plans)
- AuthMethod derives PartialEq for ergonomic test assertions with assert_eq!
- Parser import scoped to test module only to avoid production unused import warning
- No unit tests for resolve_auth_middleware() itself -- it's a thin wrapper around OAuthHelper/BearerToken which are already tested, and real auth requires a running OAuth provider

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added allow(dead_code) annotations for clippy -D warnings**
- **Found during:** Task 2 (auth.rs creation)
- **Issue:** New public types/functions triggered dead_code warnings, which become errors under clippy -D warnings
- **Fix:** Added targeted #[allow(dead_code)] with comments explaining they'll be consumed by Plans 02/03
- **Files modified:** cargo-pmcp/src/commands/flags.rs, cargo-pmcp/src/commands/auth.rs
- **Verification:** cargo clippy -p cargo-pmcp -- -D warnings passes clean
- **Committed in:** 9a46e84 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for clippy compliance. No scope creep. Annotations will be removed when Plans 02/03 add consumers.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- AuthFlags and AuthMethod are ready for Plans 02 and 03 to flatten into command variants
- resolve_auth_middleware() is ready for all handlers to call
- resolve_api_key() is ready for ServerTester-based handlers
- All verification gates pass: tests, check, clippy

## Self-Check: PASSED

- [x] flags.rs exists with AuthFlags, AuthMethod, resolve(), 7 tests
- [x] auth.rs exists with resolve_auth_middleware(), resolve_api_key()
- [x] mod.rs has pub mod auth declaration
- [x] Commit 1a82dbc (test RED phase) verified
- [x] Commit 7629ca0 (feat GREEN+REFACTOR) verified
- [x] Commit 9a46e84 (feat auth.rs) verified
- [x] cargo test flags::tests -- 7 passed
- [x] cargo check -p cargo-pmcp -- clean
- [x] cargo clippy -p cargo-pmcp -- -D warnings clean

---
*Phase: 29-auth-flag-propagation*
*Completed: 2026-03-13*
