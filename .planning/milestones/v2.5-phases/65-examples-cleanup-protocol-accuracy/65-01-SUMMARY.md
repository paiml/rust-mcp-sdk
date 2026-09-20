---
phase: 65-examples-cleanup-protocol-accuracy
plan: 01
subsystem: examples
tags: [cargo-toml, examples, readme, protocol-version, repo-hygiene]

# Dependency graph
requires: []
provides:
  - "All .rs files in examples/ registered as [[example]] in Cargo.toml with correct feature flags"
  - "README.md protocol version updated to 2025-11-25 in all 3 locations"
  - "No .disabled files remain in examples/"
affects: [65-02, 65-03]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Feature flag derivation from import analysis"]

key-files:
  created: []
  modified:
    - Cargo.toml
    - README.md

key-decisions:
  - "All 17 orphan examples compile successfully -- registered all of them (no deletions needed)"
  - "Feature flags derived from import analysis: workflow/auth/middleware/observability -> full; basic server/client -> none"
  - "No helper/module files found -- all .rs files in examples/ have fn main()"

patterns-established:
  - "Import-based feature flag derivation: workflow/auth/middleware/observability imports require 'full'"

requirements-completed: [EXMP-02, PROT-01]

# Metrics
duration: 4min
completed: 2026-04-10
---

# Phase 65 Plan 01: Orphan Example Registration and README Protocol Fix Summary

**Registered 17 orphan example files in Cargo.toml with import-derived feature flags, deleted the .disabled file, and updated README protocol version to 2025-11-25 in 3 locations**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-10T22:33:16Z
- **Completed:** 2026-04-10T22:38:00Z
- **Tasks:** 3 (1 discovery, 2 implementation)
- **Files modified:** 2

## Accomplishments
- Registered all 17 orphan .rs files in Cargo.toml with correct required-features derived from import analysis
- Deleted examples/21_macro_tools.rs.disabled and removed its commented-out Cargo.toml entry
- Updated README.md protocol version from 2025-03-26 to 2025-11-25 in badge, feature list, and compatibility table
- Verified all examples compile with `cargo check --examples --features full`

## Task Commits

Each task was committed atomically:

1. **Task 1: Pre-audit discovery** - No commit (discovery-only, no file changes)
2. **Task 2: Resolve all orphan/unnumbered example files** - `5fac9db2` (chore)
3. **Task 3: Fix README protocol version** - `1f7afb3a` (fix)

## Files Created/Modified
- `Cargo.toml` - Added 17 [[example]] entries, removed commented-out 21_macro_tools block
- `README.md` - Updated protocol version in 3 locations (badge, feature list, compatibility table)
- `examples/21_macro_tools.rs.disabled` - Deleted

## Files Registered (with feature flags)

| Example | Feature Flag | Rationale |
|---------|-------------|-----------|
| 08_server_resources | (none) | Basic server imports only |
| 11_progress_countdown | (none) | Basic server/tool imports |
| 12_prompt_workflow_progress | (none) | Basic server/prompt imports |
| 32_simd_parsing_performance | full | Uses pmcp::shared::simd_parsing |
| 40_middleware_demo | full | Uses client oauth/http middleware |
| 47_multiple_clients_parallel | full | Run docs specify --features full |
| 48_structured_output_schema | (none) | Basic re-exports only |
| 54_hybrid_workflow_execution | full | Uses pmcp::server::workflow |
| 58_oauth_transport_to_tools | full | Uses pmcp::server::auth + middleware |
| 59_dynamic_resource_workflow | full | Uses pmcp::server::workflow::dsl |
| 60_resource_only_steps | full | Uses pmcp::server::workflow::dsl |
| 61_observability_middleware | full | Uses pmcp::server::observability |
| client | (none) | Basic client imports |
| currency_server | (none) | Basic server/tool imports |
| refactored_server_example | full | Uses server adapters/builder/core |
| server | (none) | Basic server/tool imports |
| test_currency_server | (none) | Only uses serde_json |

## Files Deleted

| File | Reason |
|------|--------|
| examples/21_macro_tools.rs.disabled | Dead .disabled file (per Pitfall 12) |

## Helper/Module Files

None found. All .rs files in examples/ have a `fn main()` function and no files use `#[path]`, `mod`, or `include!` to reference other example files.

## Decisions Made
- All 17 orphan examples compile successfully with `--features full`, so none were deleted
- Feature flags were derived systematically from import analysis rather than ad-hoc guessing
- No helper modules exist -- all examples are standalone with fn main()

## Deviations from Plan

None - plan executed exactly as written. The plan anticipated some files might need fixing or deletion, but all compiled successfully as-is.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All examples are registered and compilable
- Plan 02 can proceed with renaming/renumbering (no helpers to exclude from renaming)
- Plan 03 can reference all examples by their Cargo.toml names

---
*Phase: 65-examples-cleanup-protocol-accuracy*
*Completed: 2026-04-10*
