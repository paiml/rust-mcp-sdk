---
phase: 23-course-mcp-apps-refresh
plan: 01
subsystem: docs
tags: [mcp-apps, widgetdir, mcpbridge, adapter-pattern, cargo-pmcp, course]

requires:
  - phase: 21-book-mcp-apps-refresh
    provides: "Book Ch 12.5 with current WidgetDir/mcpBridge/adapter APIs as source material"
provides:
  - "Course Ch 20 parent chapter with WidgetDir/mcpBridge paradigm"
  - "Course Ch 20-01 Widget Authoring and Developer Workflow tutorial"
  - "Course Ch 20-02 Bridge Communication and Adapters tutorial"
  - "Updated SUMMARY.md with new Ch 20 sub-chapter structure"
affects: [24-quizzes, course-exercises]

tech-stack:
  added: []
  patterns: [widget-dir-convention, resource-handler-pattern, adapter-pattern, multi-platform-resource]

key-files:
  created: []
  modified:
    - pmcp-course/src/part8-advanced/ch20-mcp-apps.md
    - pmcp-course/src/part8-advanced/ch20-01-ui-resources.md
    - pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md
    - pmcp-course/src/SUMMARY.md

key-decisions:
  - "Kept existing filenames (ch20-01-ui-resources.md, ch20-02-tool-ui-association.md) with new content and titles"
  - "Used ChatGptAdapter as the recommended default adapter throughout tutorials"
  - "Structured Ch 20-01 as progressive hands-on (scaffold -> explore -> run -> extend) matching course pedagogy"

patterns-established:
  - "Course widget tutorials follow scaffold-first pedagogy: create project, explore code, run, then extend"
  - "Bridge API taught via core-four methods first, then platform-specific extras"

requirements-completed: [CRAP-01, CRAP-02]

duration: 6min
completed: 2026-02-28
---

# Phase 23 Plan 01: Course MCP Apps Refresh Summary

**Rewrote Ch 20 parent + two sub-chapters replacing UIResourceBuilder/postMessage with WidgetDir/mcpBridge/adapter paradigm in course teaching style**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-28T04:28:10Z
- **Completed:** 2026-02-28T04:33:57Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Rewrote Ch 20 parent chapter with WidgetDir architecture overview, quick start, and learning objectives (163 lines)
- Rewrote Ch 20-01 as hands-on widget authoring tutorial covering WidgetDir, cargo pmcp app workflow, ResourceHandler pattern, and hot-reload development (499 lines)
- Rewrote Ch 20-02 as bridge communication and adapter pattern tutorial covering mcpBridge API, communication flow, three adapters, and MultiPlatformResource (409 lines)
- Updated SUMMARY.md with new Ch 20 sub-chapter titles and file references

## Task Commits

Each task was committed atomically:

1. **Task 1: Rewrite Ch 20 parent chapter and update SUMMARY.md** - `1252af3` (feat)
2. **Task 2: Rewrite Ch 20-01 Widget Authoring and Developer Workflow** - `d5077b5` (feat)
3. **Task 3: Rewrite Ch 20-02 Bridge Communication and Adapters** - `1b03f24` (feat)

## Files Created/Modified

- `pmcp-course/src/part8-advanced/ch20-mcp-apps.md` - Parent chapter with WidgetDir intro, quick start, learning objectives, architecture diagram
- `pmcp-course/src/part8-advanced/ch20-01-ui-resources.md` - Widget authoring tutorial: WidgetDir convention, scaffolding, ResourceHandler, hot-reload, build
- `pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md` - Bridge communication: mcpBridge API, adapters, MultiPlatformResource
- `pmcp-course/src/SUMMARY.md` - Updated Ch 20 section with new titles and file references

## Decisions Made

- Kept existing filenames (ch20-01-ui-resources.md, ch20-02-tool-ui-association.md) with new content to avoid breaking any existing references
- Used ChatGptAdapter as the recommended starting adapter, consistent with all three shipped examples
- Structured the counter widget exercise in Ch 20-01 as a progressive extension of the scaffolded hello widget

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ch 20 parent and sub-chapters 20-01/20-02 complete with current WidgetDir/mcpBridge APIs
- Ch 20-03 (Example Walkthroughs) available for Plan 02
- Quiz content for Ch 20 ready for Phase 24

---
*Phase: 23-course-mcp-apps-refresh*
*Completed: 2026-02-28*
