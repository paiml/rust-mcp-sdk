---
phase: 43-chatgpt-mcp-apps-alignment
plan: 02
subsystem: api
tags: [mcp-apps, chatgpt, meta, resources, deep-merge, widget]

requires:
  - phase: 43-01
    provides: "URI-to-tool-meta index, ResourceInfo._meta field, with_widget_enrichment"
  - phase: 39
    provides: "deep_merge for recursive JSON object merging"
provides:
  - "resources/list auto-populates _meta on widget ResourceInfo items from linked tool metadata"
  - "resources/read merges tool descriptor keys into Content::Resource _meta via deep_merge"
affects: [mcp-apps, chatgpt-compatibility]

tech-stack:
  added: []
  patterns: ["post-process resource handler results for _meta propagation"]

key-files:
  created: []
  modified: ["src/server/core.rs"]

key-decisions:
  - "Post-process after handler returns rather than modifying handler trait"
  - "Use deep_merge for read (preserves display keys alongside descriptor keys) vs clone for list (no pre-existing meta)"

patterns-established:
  - "URI-to-tool-meta index lookup pattern for resource _meta enrichment"
  - "get_or_insert_with + deep_merge for safe meta merging on Content::Resource"

requirements-completed: []

duration: 11min
completed: 2026-03-08
---

# Phase 43 Plan 02: Resources _meta Propagation Summary

**Post-process resources/list and resources/read to auto-populate widget _meta from URI-to-tool-meta index using deep_merge**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-08T16:09:31Z
- **Completed:** 2026-03-08T16:20:20Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- resources/list responses now include openai/* _meta keys on widget ResourceInfo items (auto-populated from linked tool)
- resources/read responses now include both display keys (widgetCSP, prefersBorder) AND descriptor keys (outputTemplate, widgetAccessible) via deep_merge
- Non-widget resources remain completely unaffected by _meta propagation

## Task Commits

Each task was committed atomically:

1. **Task 1: Post-process handle_list_resources to populate ResourceInfo._meta** - `eb7a7a9` (feat)
2. **Task 2: Post-process handle_read_resource to merge descriptor keys into content _meta** - `4cb55a6` (feat)

## Files Created/Modified
- `src/server/core.rs` - Added _meta enrichment loop in handle_list_resources and deep_merge in handle_read_resource

## Decisions Made
- Used clone for list (ResourceInfo has no pre-existing _meta from handler) vs deep_merge for read (Content::Resource may have display keys from ChatGptAdapter)
- Used get_or_insert_with pattern for safe handling when content meta is None

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All four protocol entry points now have correct _meta behavior:
  - tools/list: unchanged (all _meta keys via ToolInfo)
  - tools/call: filtered to openai/toolInvocation/* only (Plan 01)
  - resources/list: openai/* keys from linked tool (this plan, Task 1)
  - resources/read: merged display + descriptor keys (this plan, Task 2)
- ChatGPT MCP Apps alignment phase is complete

---
*Phase: 43-chatgpt-mcp-apps-alignment*
*Completed: 2026-03-08*

## Self-Check: PASSED
