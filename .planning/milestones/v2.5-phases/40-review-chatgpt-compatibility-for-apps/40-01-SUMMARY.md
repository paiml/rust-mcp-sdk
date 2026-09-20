---
phase: 40-review-chatgpt-compatibility-for-apps
plan: 01
subsystem: ui
tags: [mcp-apps, chatgpt, backward-compatibility, meta-keys]

# Dependency graph
requires:
  - phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision
    provides: deep_merge function for recursive JSON object merging
provides:
  - Legacy flat key "ui/resourceUri" in build_meta_map() for older MCP hosts
affects: [mcp-apps, chatgpt-compatibility]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dual-emit pattern: nested + flat key for backward compatibility"

key-files:
  created: []
  modified:
    - src/types/ui.rs

key-decisions:
  - "Flat key inserted between nested ui object and openai/outputTemplate to match official ext-apps reference ordering"

patterns-established:
  - "build_meta_map emits 3 keys: nested ui.resourceUri, flat ui/resourceUri, openai/outputTemplate"

requirements-completed: [COMPAT-01]

# Metrics
duration: 4min
completed: 2026-03-07
---

# Phase 40 Plan 01: Review ChatGPT Compatibility Summary

**Added legacy flat "ui/resourceUri" key to build_meta_map() matching official ext-apps dual-emit behavior**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-07T03:15:39Z
- **Completed:** 2026-03-07T03:20:06Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added legacy flat `ui/resourceUri` key emission in `build_meta_map()` for backward compatibility with older MCP hosts
- Updated existing test assertion from "must not have flat key" to "must have flat key"
- Added `test_build_meta_map_emits_all_three_keys` verifying all 3 top-level keys
- Added `test_deep_merge_preserves_flat_key` verifying flat key survives deep merge operations

## Task Commits

Each task was committed atomically (TDD RED-GREEN):

1. **Task 1 (RED): Add failing tests for legacy flat key** - `0bbd4be` (test)
2. **Task 1 (GREEN): Add legacy flat key to build_meta_map** - `634ce30` (feat)

## Files Created/Modified
- `src/types/ui.rs` - Added flat key emission in `build_meta_map()`, updated capacity to 3, updated doc comment, added 2 new tests, updated 1 existing test assertion

## Decisions Made
- Flat key inserted between nested `ui` object and `openai/outputTemplate` to match the ordering in the official `@modelcontextprotocol/ext-apps` reference implementation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 40-02 can proceed (if it exists) for any remaining ChatGPT compatibility review tasks
- All ui.rs tests pass (18/18)
- Zero clippy warnings

---
*Phase: 40-review-chatgpt-compatibility-for-apps*
*Completed: 2026-03-07*
