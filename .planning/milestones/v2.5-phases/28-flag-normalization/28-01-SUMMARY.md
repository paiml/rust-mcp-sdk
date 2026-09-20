---
phase: 28-flag-normalization
plan: 01
subsystem: cli
tags: [clap, flags, cli-dx, cargo-pmcp]

# Dependency graph
requires: []
provides:
  - "Shared flag structs: FormatValue, OutputFlags, FormatFlags, ServerFlags in flags.rs"
  - "Zero #[clap()] attributes remaining in cargo-pmcp/src/ (FLAG-07 complete)"
  - "Legacy dead execute() removed from test/mod.rs"
affects: [28-02-PLAN, 28-03-PLAN]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Shared flag structs via #[command(flatten)]", "#[arg()]/#[command()] attribute style"]

key-files:
  created: ["cargo-pmcp/src/commands/flags.rs"]
  modified: ["cargo-pmcp/src/commands/mod.rs", "cargo-pmcp/src/commands/deploy/mod.rs", "cargo-pmcp/src/commands/test/mod.rs"]

key-decisions:
  - "Retained #[allow(dead_code)] on GlobalFlags.verbose until Plans 02/03 add readers"
  - "ServerFlags makes both url and server optional for flexible flatten usage"

patterns-established:
  - "#[arg()] for field-level CLI attributes, #[command()] for subcommand/flatten attributes"
  - "Shared flag structs in flags.rs module for #[command(flatten)] reuse"

requirements-completed: [FLAG-01, FLAG-06, FLAG-07]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 28 Plan 01: Shared Flag Infrastructure Summary

**Shared FormatValue/OutputFlags/FormatFlags/ServerFlags structs in flags.rs plus full #[clap()]-to-#[arg()]/#[command()] conversion in deploy module**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T22:45:15Z
- **Completed:** 2026-03-12T22:50:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created flags.rs with four shared flag structs (FormatValue, OutputFlags, FormatFlags, ServerFlags) ready for #[command(flatten)] in Plans 02/03
- Converted all 33 #[clap()] attributes in deploy/mod.rs to #[arg()]/#[command()] -- zero remain in codebase
- Removed legacy dead execute() function from test/mod.rs (29 lines of dead code)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create flags.rs shared structs and clean up dead code** - `8873238` (feat)
2. **Task 2: Convert deploy #[clap()] to #[arg()]/#[command()]** - `5dd2909` (refactor)

## Files Created/Modified
- `cargo-pmcp/src/commands/flags.rs` - New shared flag structs: FormatValue, OutputFlags, FormatFlags, ServerFlags
- `cargo-pmcp/src/commands/mod.rs` - Added pub mod flags declaration
- `cargo-pmcp/src/commands/deploy/mod.rs` - Converted 33 #[clap()] to #[arg()]/#[command()]
- `cargo-pmcp/src/commands/test/mod.rs` - Removed legacy dead execute() function

## Decisions Made
- Retained #[allow(dead_code)] on GlobalFlags.verbose: The plan requested removal, but no code in cargo-pmcp currently reads global_flags.verbose (it is set but never accessed). Removing the attribute causes a dead_code warning that violates the zero-warnings quality gate. The attribute will be naturally removed when Plans 02/03 add verbose readers.
- ServerFlags.url uses #[arg(index = 1)] making it a positional argument, while ServerFlags.server uses #[arg(long)] for --server named flag.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Kept #[allow(dead_code)] on GlobalFlags.verbose**
- **Found during:** Task 1 (Create flags.rs shared structs and clean up dead code)
- **Issue:** Plan requested removing #[allow(dead_code)] from verbose field, but no code reads global_flags.verbose -- removal produces a dead_code warning that blocks the quality gate
- **Fix:** Retained #[allow(dead_code)] with updated doc comment explaining it will be removed when Plans 02/03 add readers
- **Files modified:** cargo-pmcp/src/commands/mod.rs
- **Verification:** cargo check -p cargo-pmcp passes with zero warnings
- **Committed in:** 8873238 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- the attribute removal is deferred to Plans 02/03 which will naturally remove it when adding verbose readers. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- flags.rs module with FormatValue, OutputFlags, FormatFlags, ServerFlags is ready for #[command(flatten)] usage in Plans 02 and 03
- All #[clap()] attributes eliminated -- Plans 02/03 only need to use #[arg()]/#[command()] style
- Legacy dead code cleaned up from test/mod.rs

---
*Phase: 28-flag-normalization*
*Completed: 2026-03-12*
