---
phase: 38-cache-toolinfo-at-registration-to-avoid-per-request-cloning
plan: 01
subsystem: api
tags: [performance, caching, metadata, toolinfo, promptinfo]

# Dependency graph
requires:
  - phase: 37-add-with-ui-support-to-typedsynctool
    provides: TypedSyncTool and WasmTypedTool with_ui() support
provides:
  - tool_infos and prompt_infos cache fields in ServerCoreBuilder, ServerCore, WasmMcpServerBuilder, WasmMcpServer
  - Zero per-request handler.metadata() calls in hot paths
affects: [39-add-deep-merge-for-ui-meta-key-to-prevent-collision]

# Tech tracking
tech-stack:
  added: []
  patterns: [registration-time-caching, cache-over-compute]

key-files:
  created: []
  modified:
    - src/server/builder.rs
    - src/server/core.rs
    - src/server/wasm_server.rs
    - src/server/adapters.rs

key-decisions:
  - "Cache is source of truth -- no fallback to handler.metadata() at request time"
  - "prompt_workflow() direct inserts also cache metadata for consistency"
  - "Test helper build_tool_infos() mirrors builder logic for direct ServerCore construction"

patterns-established:
  - "Registration-time caching: metadata captured once at add_tool/add_prompt, never re-queried"

requirements-completed: [CACHE-01]

# Metrics
duration: 10min
completed: 2026-03-06
---

# Phase 38 Plan 01: Cache ToolInfo/PromptInfo Summary

**Cached ToolInfo and PromptInfo at registration time in all builders, eliminating 6 per-request handler.metadata() call sites across ServerCore and WasmMcpServer**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-06T23:07:19Z
- **Completed:** 2026-03-06T23:17:30Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Added tool_infos/prompt_infos HashMap fields to ServerCoreBuilder, ServerCore, WasmMcpServerBuilder, WasmMcpServer
- All 5 builder registration methods (tool, tool_arc, prompt, prompt_arc, prompt_workflow) populate caches at registration
- Replaced all 6 per-request metadata() call sites with cache lookups (handle_list_tools, handle_call_tool widget enrichment, handle_list_prompts, task routing, WASM list_tools, WASM list_prompts)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add cache fields to builders and populate at registration** - `b373b55` (feat)
2. **Task 2: Replace all per-request metadata() calls with cache lookups** - `98ca179` (feat)

## Files Created/Modified
- `src/server/builder.rs` - Added tool_infos/prompt_infos fields, populate at registration in tool/prompt methods and prompt_workflow
- `src/server/core.rs` - Added cache fields to ServerCore, replaced 4 hot-path metadata() calls with cache lookups, fixed tests
- `src/server/wasm_server.rs` - Added cache fields to WasmMcpServer/Builder, replaced 2 hot-path info() calls with cache lookups
- `src/server/adapters.rs` - Fixed test to pass new tool_infos/prompt_infos parameters to ServerCore::new()

## Decisions Made
- Cache is the sole source of truth for metadata in request handlers -- no fallback to handler.metadata()
- prompt_workflow() caches metadata for both TaskWorkflowPromptHandler and WorkflowPromptHandler (direct inserts, not through self.prompt())
- Added build_tool_infos() test helper function to construct cache from tools HashMap, mirroring builder logic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed test compilation: ServerCore::new() signature change**
- **Found during:** Task 2
- **Issue:** 5 test call sites in core.rs and adapters.rs construct ServerCore::new() directly and needed the new tool_infos/prompt_infos parameters
- **Fix:** Added tool_infos and prompt_infos parameters to all test ServerCore::new() calls, created build_tool_infos() helper for tests that need populated caches
- **Files modified:** src/server/core.rs, src/server/adapters.rs
- **Verification:** cargo test --lib passes all 705 tests
- **Committed in:** 98ca179 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for compilation. No scope creep.

## Issues Encountered
- 3 pre-existing doctest failures related to streamable-http feature gate (not related to this change)

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Cache infrastructure complete, ready for Phase 39 (deep merge for UI meta key)
- No blockers or concerns

---
*Phase: 38-cache-toolinfo-at-registration-to-avoid-per-request-cloning*
*Completed: 2026-03-06*
