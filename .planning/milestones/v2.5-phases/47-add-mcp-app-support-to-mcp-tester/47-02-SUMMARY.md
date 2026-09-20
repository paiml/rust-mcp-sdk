---
phase: 47-add-mcp-app-support-to-mcp-tester
plan: 02
subsystem: testing
tags: [mcp-apps, cargo-pmcp, cli, validation]

requires:
  - phase: 47-add-mcp-app-support-to-mcp-tester
    provides: AppValidator module and AppValidationMode re-exported from mcp-tester
provides:
  - cargo pmcp test apps subcommand for App metadata validation
  - App-capable tool detection hint in cargo pmcp test check
affects: []

tech-stack:
  added: []
  patterns: [cargo-pmcp apps subcommand delegates to mcp_tester AppValidator]

key-files:
  created:
    - cargo-pmcp/src/commands/test/apps.rs
  modified:
    - cargo-pmcp/src/commands/test/mod.rs
    - cargo-pmcp/src/commands/test/check.rs

key-decisions:
  - "Apps subcommand follows check.rs pattern with header, connectivity test, then validation"
  - "Empty resources list gracefully handled (Vec::new) so validation continues even if resources/list fails"

patterns-established:
  - "Apps variant placed alphabetically first in TestCommand enum"
  - "App-capable hint in check.rs after tool listing, guarded by should_output()"

requirements-completed: [APP-VAL-01, APP-VAL-04]

duration: 3min
completed: 2026-03-12
---

# Phase 47 Plan 02: cargo pmcp test apps Subcommand Summary

**`cargo pmcp test apps --url <url>` subcommand wired to AppValidator with --mode/--tool/--strict flags, plus App-capable tool hint in check command**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-12T00:33:23Z
- **Completed:** 2026-03-12T00:36:30Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `cargo pmcp test apps` subcommand with full flag parity (--mode, --tool, --strict, --transport, --verbose, --timeout)
- App-capable tool detection hint in `cargo pmcp test check` output suggesting `cargo pmcp test apps` command
- Zero clippy warnings across both mcp-tester and cargo-pmcp crates

## Task Commits

Each task was committed atomically:

1. **Task 1: Create cargo-pmcp test apps subcommand** - `960e096` (feat)
2. **Task 2: Add App-capable tools hint to check command** - `40b07b7` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/test/apps.rs` - Apps validation handler with mode parsing, connectivity check, tool/resource discovery, AppValidator delegation, and report printing
- `cargo-pmcp/src/commands/test/mod.rs` - Added `mod apps`, Apps variant in TestCommand enum with all flags, match arm in execute()
- `cargo-pmcp/src/commands/test/check.rs` - Added AppValidator import and App-capable tool count hint after tools listing

## Decisions Made
- Apps subcommand follows check.rs pattern (header, connectivity, then validation) for consistent UX
- Resources listing failure is non-fatal (returns empty vec) since resource cross-reference is advisory

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 47 is complete: AppValidator module (47-01) and cargo-pmcp integration (47-02) both shipped
- `mcp-tester apps` and `cargo pmcp test apps` provide full App metadata validation

---
*Phase: 47-add-mcp-app-support-to-mcp-tester*
*Completed: 2026-03-12*
