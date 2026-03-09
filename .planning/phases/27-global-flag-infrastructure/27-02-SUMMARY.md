---
phase: 27-global-flag-infrastructure
plan: 02
subsystem: cli
tags: [quiet-mode, output-filtering, global-flags, ux]

# Dependency graph
requires:
  - phase: 27-global-flag-infrastructure (plan 01)
    provides: GlobalFlags struct wired through all 16 command modules with _global_flags convention
provides:
  - Quiet-aware output filtering across all 25 command files
  - Verbose-wins-over-quiet precedence logic
  - GlobalFlags helper methods (should_output, status, print) and macros (status!, qprintln!)
  - PMCP_QUIET=1 env var for subprocess quiet propagation
affects: [cargo-pmcp-commands, any-future-command-additions]

# Tech tracking
tech-stack:
  added: []
  patterns: [quiet-aware-output, env-var-quiet-propagation, should_output-guard-pattern]

key-files:
  created: []
  modified:
    - cargo-pmcp/src/commands/mod.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/loadtest/run.rs
    - cargo-pmcp/src/commands/loadtest/upload.rs
    - cargo-pmcp/src/commands/loadtest/init.rs
    - cargo-pmcp/src/commands/loadtest/mod.rs
    - cargo-pmcp/src/commands/test/check.rs
    - cargo-pmcp/src/commands/test/run.rs
    - cargo-pmcp/src/commands/test/generate.rs
    - cargo-pmcp/src/commands/test/list.rs
    - cargo-pmcp/src/commands/test/download.rs
    - cargo-pmcp/src/commands/test/upload.rs
    - cargo-pmcp/src/commands/test/mod.rs
    - cargo-pmcp/src/commands/deploy/mod.rs
    - cargo-pmcp/src/commands/secret/mod.rs
    - cargo-pmcp/src/commands/schema.rs
    - cargo-pmcp/src/commands/validate.rs
    - cargo-pmcp/src/commands/preview.rs
    - cargo-pmcp/src/commands/app.rs
    - cargo-pmcp/src/commands/connect.rs
    - cargo-pmcp/src/commands/dev.rs
    - cargo-pmcp/src/commands/new.rs
    - cargo-pmcp/src/commands/add.rs
    - cargo-pmcp/src/commands/landing/mod.rs
    - cargo-pmcp/src/commands/landing/init.rs
    - cargo-pmcp/src/commands/landing/dev.rs
    - cargo-pmcp/src/commands/landing/deploy.rs

key-decisions:
  - "Used should_output() guard pattern for files with direct global_flags access, PMCP_QUIET env var for deeply nested functions"
  - "Merged secret module local --quiet with global --quiet into effective_quiet parameter"
  - "Passed quiet: bool to schema sub-functions instead of full GlobalFlags to minimize signature changes"
  - "Kept verbose field on GlobalFlags with allow(dead_code) for forward-compatibility; used in precedence logic but not yet by individual commands"

patterns-established:
  - "should_output() pattern: wrap decorative output blocks in `if global_flags.should_output() { ... }`"
  - "PMCP_QUIET env var pattern: for functions that don't receive global_flags, check `std::env::var(\"PMCP_QUIET\").is_err()`"
  - "Output classification: decorative (suppress in quiet), requested data (always show), errors (always show)"

requirements-completed: [FLAG-09]

# Metrics
duration: 35min
completed: 2026-03-03
---

# Phase 27 Plan 02: Quiet-Aware Output Summary

**--quiet flag suppresses all decorative output across 25 command files with verbose-wins-over-quiet precedence and PMCP_QUIET env var propagation**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-03T18:45:00Z
- **Completed:** 2026-03-03T19:20:00Z
- **Tasks:** 2
- **Files modified:** 27

## Accomplishments
- Implemented verbose-wins-over-quiet precedence: `quiet = cli.quiet && !cli.verbose`
- Converted decorative output in all 25 command files to quiet-aware output
- Added GlobalFlags helper methods (should_output, status, print, status_fmt, print_fmt) and macros (status!, qprintln!)
- Set PMCP_QUIET=1 environment variable when quiet is active for subprocess propagation
- All 233 tests pass with zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add quiet-aware output helpers and verbose-wins-over-quiet** - `8a6c9b2` (feat)
2. **Task 2: Convert decorative output across all commands to quiet-aware** - `bed59b2` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/mod.rs` - Added GlobalFlags helper methods, macros, moved allow(dead_code) to verbose field
- `cargo-pmcp/src/main.rs` - Added verbose-wins-over-quiet precedence, PMCP_QUIET env var setting
- `cargo-pmcp/src/commands/loadtest/run.rs` - Wrapped decorative output, updated apply_overrides signature with GlobalFlags
- `cargo-pmcp/src/commands/loadtest/upload.rs` - Wrapped banners, progress, troubleshooting output
- `cargo-pmcp/src/commands/loadtest/init.rs` - Wrapped discovery and success messages
- `cargo-pmcp/src/commands/loadtest/mod.rs` - Threaded global_flags to init/upload
- `cargo-pmcp/src/commands/test/check.rs` - Wrapped banners/tips, kept test results visible
- `cargo-pmcp/src/commands/test/run.rs` - Wrapped banners, prereqs, summary
- `cargo-pmcp/src/commands/test/generate.rs` - Wrapped progress, next-steps, tips
- `cargo-pmcp/src/commands/test/list.rs` - Wrapped commands/hints, kept scenario listing
- `cargo-pmcp/src/commands/test/download.rs` - Wrapped banners, success, next-steps
- `cargo-pmcp/src/commands/test/upload.rs` - Wrapped progress, success, next-steps
- `cargo-pmcp/src/commands/test/mod.rs` - Threaded global_flags through all sub-commands
- `cargo-pmcp/src/commands/deploy/mod.rs` - Wrapped destroy, status, deploy, OAuth sections
- `cargo-pmcp/src/commands/secret/mod.rs` - Merged global quiet with local quiet
- `cargo-pmcp/src/commands/schema.rs` - Passed quiet bool to export/validate/diff
- `cargo-pmcp/src/commands/validate.rs` - Renamed _global_flags
- `cargo-pmcp/src/commands/preview.rs` - Wrapped banner output
- `cargo-pmcp/src/commands/app.rs` - Wrapped create_app, run_manifest, create_landing, build_all output
- `cargo-pmcp/src/commands/connect.rs` - Wrapped all three client connection functions
- `cargo-pmcp/src/commands/dev.rs` - Wrapped step banners, server info, connect output
- `cargo-pmcp/src/commands/new.rs` - Wrapped workspace creation and next-steps output
- `cargo-pmcp/src/commands/add.rs` - Wrapped server/tool/workflow scaffolding output
- `cargo-pmcp/src/commands/landing/mod.rs` - Renamed _global_flags, wrapped Build stub
- `cargo-pmcp/src/commands/landing/init.rs` - Wrapped init progress and next-steps
- `cargo-pmcp/src/commands/landing/dev.rs` - Wrapped server config and startup output
- `cargo-pmcp/src/commands/landing/deploy.rs` - Wrapped deploy progress, upload, build output

## Decisions Made
- Used `global_flags.should_output()` as the primary guard for files that receive global_flags directly, and `std::env::var("PMCP_QUIET").is_err()` for deeply nested functions (like connect sub-functions, app sub-functions, landing sub-modules) that don't receive global_flags
- Merged secret module's own `--quiet` flag with global `--quiet` via `let effective_quiet = self.quiet || global_flags.quiet` to avoid breaking the secret module's existing quiet behavior
- Passed `quiet: bool` parameter to schema sub-functions (export, validate, diff) rather than threading full GlobalFlags to keep signature changes minimal
- Kept `verbose` field with `#[allow(dead_code)]` since it's used in precedence logic (main.rs) but not yet consumed by individual commands

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed apply_overrides test signatures in loadtest/run.rs**
- **Found during:** Task 2 (loadtest/run.rs conversion)
- **Issue:** After adding `global_flags: &GlobalFlags` to `apply_overrides`, 4 unit tests failed to compile
- **Fix:** Created default GlobalFlags in each test and passed as reference
- **Files modified:** cargo-pmcp/src/commands/loadtest/run.rs
- **Verification:** All tests pass
- **Committed in:** bed59b2

**2. [Rule 3 - Blocking] Fixed legacy test execute function in test/mod.rs**
- **Found during:** Task 2 (test module conversion)
- **Issue:** After updating test sub-command signatures, the legacy `execute()` function still called them with old signatures
- **Fix:** Created default GlobalFlags and threaded through all sub-command calls
- **Files modified:** cargo-pmcp/src/commands/test/mod.rs
- **Verification:** Compiles and tests pass
- **Committed in:** bed59b2

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes were necessary compilation fixes caused by the signature changes. No scope creep.

## Issues Encountered
None - plan executed as specified.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All three global flags (--verbose, --no-color, --quiet) are fully implemented
- Phase 27 is complete: global flag infrastructure is production-ready
- Future commands added to cargo-pmcp should follow the established patterns (should_output() guard or PMCP_QUIET env var check)

## Self-Check: PASSED

All 22 modified files verified present on disk. Both task commits (8a6c9b2, bed59b2) verified in git log. SUMMARY.md created.

---
*Phase: 27-global-flag-infrastructure*
*Completed: 2026-03-03*
