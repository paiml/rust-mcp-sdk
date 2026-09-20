---
phase: 27-global-flag-infrastructure
plan: 01
subsystem: cli
tags: [clap, colored, console, no-color, global-flags, cli-ux]

# Dependency graph
requires: []
provides:
  - "GlobalFlags struct (verbose, no_color, quiet) wired to every command handler"
  - "Global --no-color and --quiet CLI flags on Cli struct"
  - "Pre-dispatch color suppression (colored + console crates)"
  - "NO_COLOR env var and non-TTY auto-detection"
  - "PMCP_NO_COLOR and PMCP_QUIET env vars for subprocess consumption"
affects: [27-02-quiet-flag, 28-flag-normalization, 29-auth-propagation, 30-tester-integration, 31-new-commands, 32-help-polish]

# Tech tracking
tech-stack:
  added: []
  patterns: ["GlobalFlags parameter threading through command dispatch", "pre-dispatch color suppression pattern"]

key-files:
  created: []
  modified:
    - "cargo-pmcp/src/commands/mod.rs"
    - "cargo-pmcp/src/main.rs"
    - "cargo-pmcp/src/commands/loadtest/mod.rs"
    - "cargo-pmcp/src/commands/loadtest/run.rs"
    - "cargo-pmcp/src/loadtest/display.rs"

key-decisions:
  - "GlobalFlags defined in commands/mod.rs to avoid circular imports (not main.rs)"
  - "no_color field stores resolved value (CLI flag OR NO_COLOR env OR non-TTY) so downstream code needs no re-checking"
  - "Underscore-prefixed _global_flags parameters for handlers not yet using flags (Plan 02 activates quiet)"
  - "Loadtest local --no-color removed; engine.with_no_color() still uses global_flags.no_color"

patterns-established:
  - "GlobalFlags threading: every command handler receives &GlobalFlags as its last parameter"
  - "Pre-dispatch suppression: color overrides set once in main() before execute_command()"
  - "Env var propagation: PMCP_NO_COLOR=1 and PMCP_QUIET=1 for subprocesses"

requirements-completed: [FLAG-08]

# Metrics
duration: 8min
completed: 2026-03-04
---

# Phase 27 Plan 01: Global Flag Infrastructure Summary

**GlobalFlags struct wired through 16 command modules with --no-color/--quiet global flags and pre-dispatch color suppression via colored + console crates**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-04T02:46:03Z
- **Completed:** 2026-03-04T02:54:02Z
- **Tasks:** 2
- **Files modified:** 19

## Accomplishments
- Defined GlobalFlags struct with verbose, no_color, quiet fields in commands/mod.rs
- Wired GlobalFlags through all 16 command handler modules (loadtest, test, deploy, secret, schema, validate, app, landing, new, add, dev, connect, preview)
- Removed loadtest-local --no-color flag (replaced by global)
- Implemented global color suppression in main() covering colored crate, console crate, and NO_COLOR env var / non-TTY auto-detection

## Task Commits

Each task was committed atomically:

1. **Task 1: Define GlobalFlags struct and wire through all command dispatch** - `186ccf2` (feat)
2. **Task 2: Implement global color suppression** - `7b8f4e8` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/mod.rs` - Added GlobalFlags struct definition
- `cargo-pmcp/src/main.rs` - Added --no-color/--quiet CLI flags, pre-dispatch color suppression, GlobalFlags construction and threading
- `cargo-pmcp/src/commands/loadtest/mod.rs` - Removed local no_color field from Run variant, updated execute() signature
- `cargo-pmcp/src/commands/loadtest/run.rs` - Replaced local no_color param with GlobalFlags, removed local color override
- `cargo-pmcp/src/loadtest/display.rs` - Removed local colored::control::set_override call from LiveDisplay::new()
- `cargo-pmcp/src/commands/test/mod.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/deploy/mod.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/secret/mod.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/schema.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/validate.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/app.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/landing/mod.rs` - Added GlobalFlags to execute() signature
- `cargo-pmcp/src/commands/new.rs` - Added GlobalFlags parameter
- `cargo-pmcp/src/commands/add.rs` - Added GlobalFlags parameter to server/tool/workflow
- `cargo-pmcp/src/commands/dev.rs` - Added GlobalFlags parameter, passed to connect::execute
- `cargo-pmcp/src/commands/connect.rs` - Added GlobalFlags parameter
- `cargo-pmcp/src/commands/preview.rs` - Added GlobalFlags parameter

## Decisions Made
- GlobalFlags defined in commands/mod.rs (not main.rs) to avoid circular import issues -- subcommand modules import from their parent
- The no_color field stores the resolved effective value (CLI flag OR NO_COLOR env OR non-TTY), so downstream code can use it directly
- Handlers not yet consuming flags use _global_flags (underscore-prefixed) -- Plan 02 will activate quiet behavior
- The loadtest engine's with_no_color() still receives global_flags.no_color since it uses the value for progress bar draw target decisions

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed dev.rs internal call to connect::execute**
- **Found during:** Task 1 (GlobalFlags wiring)
- **Issue:** dev.rs calls connect::execute internally at line 64, which now requires GlobalFlags
- **Fix:** Passed _global_flags through from dev::execute to connect::execute
- **Files modified:** cargo-pmcp/src/commands/dev.rs
- **Verification:** cargo check passes
- **Committed in:** 186ccf2 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for internal function call chain. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- GlobalFlags infrastructure is complete and compiled clean
- Plan 02 can build on this to implement --quiet behavior
- All subsequent Phase 28-32 work has the GlobalFlags wiring available
- The _global_flags underscore convention signals which handlers will gain behavior in future plans

---
*Phase: 27-global-flag-infrastructure*
*Completed: 2026-03-04*
