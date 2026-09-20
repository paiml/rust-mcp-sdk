---
phase: 23-course-mcp-apps-refresh
plan: 02
subsystem: documentation
tags: [course, mcp-apps, chess, map, dataviz, widgets, mcpBridge, WidgetDir]

requires:
  - phase: 21-book-mcp-apps-refresh
    provides: Book Ch 12.5 with WidgetDir/adapter reference docs and example walkthroughs

provides:
  - Course Ch 20-03 rewritten as hands-on example walkthroughs (chess, map, dataviz)
  - Common 4-step architecture pattern documented for course learners
  - Comparison table and practice ideas for self-guided learning

affects: [24-course-quizzes]

tech-stack:
  added: []
  patterns: [example-walkthrough-teaching, 4-step-widget-architecture]

key-files:
  created: []
  modified:
    - pmcp-course/src/part8-advanced/ch20-03-postmessage.md

key-decisions:
  - "Wrote complete chapter (both tasks) in single pass since Task 2 appends to Task 1 output"
  - "Kept Best Practices 'never use postMessage' mention as negative instruction (correct teaching)"

patterns-established:
  - "Example walkthrough pattern: Architecture diagram, Run It, Explore Tools, Explore Widget/Server, Key Takeaway"

requirements-completed: [CRAP-03]

duration: 4min
completed: 2026-02-28
---

# Phase 23 Plan 02: Ch 20-03 Example Walkthroughs Summary

**Course Ch 20-03 rewritten as hands-on walkthroughs of chess (stateless), map (context-aware), and dataviz (SQL dashboard) examples with common 4-step architecture pattern**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-28T04:28:12Z
- **Completed:** 2026-02-28T04:31:45Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Replaced old PostMessage/JSON-RPC content with modern WidgetDir/mcpBridge walkthroughs
- Chess walkthrough: stateless game pattern with architecture, tools, widget, and server exploration
- Map walkthrough: context-aware queries with MapState, Haversine distance, Leaflet.js
- Dataviz walkthrough: SQL dashboard with Chart.js, Chinook database, input validation
- Common 4-step pattern distilled as reusable architecture
- Comparison table, best practices, chapter summary, and 5 practice ideas
- 575 lines of course content in teaching voice

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Ch 20-03 -- Chess and Map example walkthroughs** - `ebeb293` (feat)
2. **Task 2: Write Ch 20-03 -- Dataviz, common pattern, and practice ideas** - included in `ebeb293` (single file, written in one pass)

## Files Created/Modified

- `pmcp-course/src/part8-advanced/ch20-03-postmessage.md` - Complete rewrite as example walkthroughs (575 lines)

## Decisions Made

- Wrote both tasks in a single pass since Task 2 content depends directly on Task 1 output (same file)
- The best practices section mentions `window.parent.postMessage` as a "don't do this" instruction, which is correct teaching guidance

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ch 20 (MCP Apps) course content complete across all three sub-chapters
- Phase 23 needs 23-01 (Ch 20-01 and 20-02) to also be complete
- Phase 24 (quizzes) can proceed once both 23-01 and 23-02 are done

---
*Phase: 23-course-mcp-apps-refresh*
*Completed: 2026-02-28*
