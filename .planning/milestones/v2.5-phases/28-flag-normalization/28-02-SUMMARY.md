---
phase: 28-flag-normalization
plan: 02
subsystem: cli
tags: [clap, flags, positional-url, server-flags, verbose-normalization, cargo-pmcp]

# Dependency graph
requires:
  - "28-01: Shared flag structs (FormatValue, OutputFlags, FormatFlags, ServerFlags) in flags.rs"
provides:
  - "Positional URL on test check, test apps, preview, connect (required) and test run, test generate, schema export (via ServerFlags flatten)"
  - "Schema diff url at positional index 2 (replaces --endpoint)"
  - "Global verbose: all test, validate, and deploy subcommands read global_flags.verbose (no local --verbose/--detailed)"
  - "Download format: FormatValue enum with json default (yaml removed)"
  - "test generate --output has -o short alias"
  - "GlobalFlags.verbose field active (allow(dead_code) removed)"
affects: [28-03-PLAN]

# Tech tracking
tech-stack:
  added: []
  patterns: ["#[command(flatten)] ServerFlags for optional positional URL + --server", "global_flags.verbose replaces local verbose/detailed params"]

key-files:
  created: []
  modified:
    - "cargo-pmcp/src/commands/test/mod.rs"
    - "cargo-pmcp/src/commands/test/check.rs"
    - "cargo-pmcp/src/commands/test/apps.rs"
    - "cargo-pmcp/src/commands/test/run.rs"
    - "cargo-pmcp/src/commands/test/generate.rs"
    - "cargo-pmcp/src/commands/test/download.rs"
    - "cargo-pmcp/src/commands/schema.rs"
    - "cargo-pmcp/src/commands/validate.rs"
    - "cargo-pmcp/src/commands/deploy/mod.rs"
    - "cargo-pmcp/src/commands/mod.rs"
    - "cargo-pmcp/src/main.rs"

key-decisions:
  - "Removed #[allow(dead_code)] from GlobalFlags.verbose since check.rs, apps.rs, run.rs, validate.rs, deploy now read it"
  - "Download format default changed from yaml to json (FormatValue enum enforces text/json only)"
  - "Schema diff url at index 2 (after schema path at index 1) -- positional not --endpoint"

patterns-established:
  - "Commands with optional URL + --server fallback use #[command(flatten)] ServerFlags"
  - "Commands with required URL use bare positional field (no #[arg] needed)"
  - "All verbose behavior reads global_flags.verbose -- no local --verbose or --detailed flags"

requirements-completed: [FLAG-01, FLAG-03, FLAG-05]

# Metrics
duration: 6min
completed: 2026-03-12
---

# Phase 28 Plan 02: Flag Normalization for Test/Schema/Preview/Connect/Validate/Deploy Summary

**Positional URLs on 8 commands (via ServerFlags flatten or bare positional), global verbose on 5 commands, FormatValue on download, -o alias on generate**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-12T22:53:04Z
- **Completed:** 2026-03-12T22:59:30Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Converted 8 commands to positional URL (test check, test apps, test run, test generate, schema export, schema diff, preview, connect)
- Eliminated all local --verbose and --detailed flags across test, validate, and deploy modules -- all read global_flags.verbose
- Normalized test download format from yaml/json Option<String> to FormatValue enum with json default
- Added -o short alias to test generate --output
- Removed #[allow(dead_code)] from GlobalFlags.verbose (now actively read by 5+ commands)

## Task Commits

Each task was committed atomically:

1. **Task 1: Normalize test module flags** - `f143daa` (feat)
2. **Task 2: Normalize schema, preview, connect, validate, deploy flags** - `b136d8d` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/test/mod.rs` - Apps/Check use positional URL; Run/Generate use ServerFlags flatten; Download uses FormatValue; Generate gets -o; verbose/detailed fields removed
- `cargo-pmcp/src/commands/test/check.rs` - Removed verbose param, uses global_flags.verbose; updated CLI hint messages
- `cargo-pmcp/src/commands/test/apps.rs` - Removed verbose param, uses global_flags.verbose
- `cargo-pmcp/src/commands/test/run.rs` - Removed detailed param, uses global_flags.verbose; updated error/hint messages
- `cargo-pmcp/src/commands/test/generate.rs` - Updated error message for positional URL
- `cargo-pmcp/src/commands/test/download.rs` - Format param changed from Option<String> to String (FormatValue always has value)
- `cargo-pmcp/src/commands/schema.rs` - Export uses ServerFlags flatten; Diff uses positional url at index 2; error/hint messages updated
- `cargo-pmcp/src/commands/validate.rs` - Removed local --verbose field; execute passes global_flags.verbose
- `cargo-pmcp/src/commands/deploy/mod.rs` - Removed verbose field from Test variant; match arm uses global_flags.verbose
- `cargo-pmcp/src/commands/mod.rs` - Removed #[allow(dead_code)] from verbose field
- `cargo-pmcp/src/main.rs` - Preview URL is bare positional; Connect URL is positional with default

## Decisions Made
- Removed #[allow(dead_code)] from GlobalFlags.verbose: Now that check.rs, apps.rs, run.rs, validate.rs, and deploy all read global_flags.verbose, the field is no longer dead code. This completes the deferred work from Plan 01.
- Download format default changed from yaml to json: Per CONTEXT.md decision "text and json only -- two values everywhere", yaml is removed entirely. JSON is the natural default for a machine-oriented download command.
- Schema diff url at index 2: The schema file path occupies index 1 (already positional), so the URL takes index 2. This follows the natural `cargo pmcp schema diff local.json https://server.com` order.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FLAG-01 (URL positional) now covers all test/*, schema, preview, and connect commands
- FLAG-03 (verbose normalization) is complete across entire codebase
- FLAG-05 (output -o alias) partially complete (test generate done; app, secret, landing remaining for Plan 03)
- FLAG-06 (format normalization) partially complete (test download done)
- Plan 03 can proceed with remaining commands: app, secret, loadtest, landing

## Self-Check: PASSED

All 11 modified files verified present. Both task commits (f143daa, b136d8d) verified in git log.

---
*Phase: 28-flag-normalization*
*Completed: 2026-03-12*
