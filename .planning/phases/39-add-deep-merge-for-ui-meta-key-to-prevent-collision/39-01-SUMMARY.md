---
phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision
plan: 01
subsystem: api
tags: [serde_json, deep-merge, builder-pattern, meta]

requires:
  - phase: 38-cache-toolinfo-at-registration-to-avoid-per-request-cloning
    provides: ToolInfo caching infrastructure
provides:
  - deep_merge() function for recursive JSON object merging
  - ToolInfo::with_meta_entry() composable builder method
affects: [39-02 migrate metadata implementations]

tech-stack:
  added: []
  patterns: [deep-merge for _meta composition, with_meta_entry builder]

key-files:
  created: []
  modified:
    - src/types/ui.rs
    - src/types/protocol.rs

key-decisions:
  - "deep_merge placed in ui.rs alongside build_meta_map for locality"
  - "Arrays replaced entirely (not concatenated) matching JSON Merge Patch semantics"
  - "tracing::debug on leaf collision for observability without noise"

patterns-established:
  - "deep_merge for composable _meta: multiple builder methods can contribute keys without collision"
  - "with_meta_entry for single-key addition vs with_meta for replace-all"

requirements-completed: [MERGE-01]

duration: 4min
completed: 2026-03-07
---

# Phase 39 Plan 01: Deep Merge for UI Meta Key Summary

**Recursive deep_merge function and ToolInfo::with_meta_entry builder for collision-free _meta composition**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-07T00:01:52Z
- **Completed:** 2026-03-07T00:06:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `deep_merge()` function that recursively merges nested JSON objects in-place with last-in-wins semantics
- Added `ToolInfo::with_meta_entry()` composable builder that deep-merges a single key-value pair into `_meta`
- 12 unit tests covering all merge scenarios (7 for deep_merge, 5 for with_meta_entry)
- Zero clippy warnings, all existing tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add deep_merge function with unit tests** - `4c3f905` (feat)
2. **Task 2: Add ToolInfo::with_meta_entry builder method with tests** - `5077bb7` (feat)

## Files Created/Modified
- `src/types/ui.rs` - Added `deep_merge()` pub function with 7 unit tests
- `src/types/protocol.rs` - Added `ToolInfo::with_meta_entry()` builder with 5 unit tests

## Decisions Made
- Placed `deep_merge` in `ui.rs` alongside `build_meta_map` for module locality (both deal with _meta map construction)
- Arrays are replaced entirely by overlay, matching JSON Merge Patch (RFC 7396) semantics
- Used `tracing::debug!` for leaf collision logging -- observable but not noisy

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `deep_merge` and `with_meta_entry` are ready for Plan 02 to migrate all `metadata()` implementations
- Existing `with_meta` (replace-all) is untouched -- backward compatible

---
*Phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision*
*Completed: 2026-03-07*
