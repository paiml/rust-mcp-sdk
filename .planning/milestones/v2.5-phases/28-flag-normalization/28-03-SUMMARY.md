---
phase: 28-flag-normalization
plan: 03
subsystem: cli
tags: [clap, flags, cli-dx, cargo-pmcp, normalization]

# Dependency graph
requires:
  - phase: 28-01
    provides: "Shared flag structs (OutputFlags, ServerFlags) in flags.rs"
provides:
  - "App manifest/build accept URL as positional argument (FLAG-01 partial)"
  - "Landing deploy uses --server instead of --server-id (FLAG-02 complete)"
  - "Secret delete and loadtest init use --yes/-y instead of --force (FLAG-04 complete)"
  - "-o short alias on app manifest/landing/build, secret get, landing init (FLAG-05 partial)"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: ["Positional URL for app commands", "--yes/-y for confirmation skip", "-o short alias on --output flags"]

key-files:
  created: []
  modified:
    - "cargo-pmcp/src/commands/app.rs"
    - "cargo-pmcp/src/commands/secret/mod.rs"
    - "cargo-pmcp/src/commands/loadtest/mod.rs"
    - "cargo-pmcp/src/commands/loadtest/init.rs"
    - "cargo-pmcp/src/commands/landing/mod.rs"

key-decisions:
  - "Renamed landing deploy handler parameter from server_id to server in match arm; internal deploy function still receives the value positionally"
  - "Updated loadtest init error message from 'Use --force' to 'Use --yes' for consistency"

patterns-established:
  - "Positional URL (no #[arg(long)]) for commands requiring a server URL"
  - "--yes/-y for all confirmation-skip flags (not --force)"
  - "-o short alias on all --output flags"

requirements-completed: [FLAG-01, FLAG-02, FLAG-04, FLAG-05]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 28 Plan 03: App/Secret/Loadtest/Landing Flag Normalization Summary

**Positional URL for app manifest/build, --yes/-y replacing --force in secret/loadtest, --server replacing --server-id in landing, -o alias on five output flags**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T22:52:52Z
- **Completed:** 2026-03-12T22:58:33Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- App manifest and build commands now accept URL as a required positional argument instead of --url flag
- All three app output commands (manifest, landing, build) plus secret get and landing init have -o short alias for --output
- Secret delete and loadtest init use --yes/-y instead of --force for confirmation skip
- Landing deploy uses --server instead of --server-id for pmcp.run server references

## Task Commits

Each task was committed atomically:

1. **Task 1: Normalize app command flags (URL positional, -o alias)** - `bd648fa` (feat)
2. **Task 2: Normalize secret, loadtest, and landing flags (--yes, -o, --server)** - `2a34327` (feat)

## Files Created/Modified
- `cargo-pmcp/src/commands/app.rs` - URL made positional for Manifest/Build variants; -o added to all three output flags; preview hint updated
- `cargo-pmcp/src/commands/secret/mod.rs` - Delete --force renamed to --yes/-y; Get --output gets -o alias
- `cargo-pmcp/src/commands/loadtest/mod.rs` - Init --force renamed to --yes/-y
- `cargo-pmcp/src/commands/loadtest/init.rs` - Parameter renamed from force to yes; error message updated
- `cargo-pmcp/src/commands/landing/mod.rs` - Deploy --server-id renamed to --server; Init --output gets -o alias

## Decisions Made
- Renamed landing deploy handler parameter from server_id to server in match arm; the internal `deploy_landing_page()` function still receives the value positionally so its parameter name (server_id) was not changed -- this is an internal API boundary, not a CLI flag
- Updated loadtest init error message from "Use `--force` to overwrite" to "Use `--yes` to overwrite" for consistency with the new flag name

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing compilation errors from incomplete 28-02 plan changes exist in dirty working tree files (deploy/mod.rs, test/mod.rs, schema.rs, validate.rs, main.rs). These are NOT caused by plan 03 changes and do not affect committed code. Committed code compiles cleanly when dirty working tree is stashed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FLAG-01 (URL positional): Complete for app commands; test/preview/connect/schema covered by Plan 28-02
- FLAG-02 (--server): Complete
- FLAG-04 (--yes): Complete
- FLAG-05 (-o alias): Complete for app/secret/landing; test generate covered by Plan 28-02
- All plan 28-03 requirements delivered; phase 28 flag normalization can proceed to verification after 28-02 completes

## Self-Check: PASSED

All modified files exist. Both task commits verified (bd648fa, 2a34327). Committed code compiles cleanly.

---
*Phase: 28-flag-normalization*
*Completed: 2026-03-12*
