---
phase: 24-course-quizzes-exercises
plan: 02
subsystem: documentation
tags: [quiz, toml, course, summary, mdbook, mcp-apps, widgetdir]

requires:
  - phase: 23-course-mcp-apps-refresh
    provides: rewritten ch20 sub-chapters with WidgetDir, mcpBridge, adapter content
  - phase: 22-course-performance-testing
    provides: ch18-03 performance optimization chapter
provides:
  - refreshed ch20 quiz covering WidgetDir, cargo pmcp app, mcpBridge, adapter pattern
  - ch18 exercises landing page for load testing and dashboard exercises
  - updated SUMMARY.md with all Phase 22-23 content entries
affects: []

tech-stack:
  added: []
  patterns: [quiz-refresh-with-content-alignment]

key-files:
  created:
    - pmcp-course/src/part7-observability/ch18-exercises.md
  modified:
    - pmcp-course/src/quizzes/ch20-mcp-apps.toml
    - pmcp-course/src/SUMMARY.md

key-decisions:
  - "Kept 7 valid existing questions, replaced 1 outdated UIResourceBuilder question, added 6 new questions for 14 total"
  - "Created ch18-exercises.md with load testing and dashboard exercises to fill missing exercises gap"

patterns-established: []

requirements-completed: [CRQE-03, CRQE-04]

duration: 5min
completed: 2026-02-28
---

# Phase 24 Plan 02: Ch20 Quiz Refresh and SUMMARY.md Update Summary

**Refreshed ch20 quiz to 14 questions covering WidgetDir, cargo pmcp app new/build/preview, mcpBridge, and multi-platform adapter pattern; added ch18 exercises page and SUMMARY.md entry**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-28T04:47:50Z
- **Completed:** 2026-02-28T04:53:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Refreshed ch20-mcp-apps.toml from 12 to 14 questions aligned with rewritten chapter content
- Added 7 new questions covering WidgetDir, cargo pmcp app workflow, mcpBridge, adapter pattern, and 4-step architecture
- Retained 7 valid existing questions (experimental status, ui://, MIME type, sandboxed iframes, postMessage, graceful degradation, request IDs)
- Created ch18-exercises.md with load testing and dashboard setup exercises
- Updated SUMMARY.md with ch18 exercises entry

## Task Commits

Each task was committed atomically:

1. **Task 1: Refresh ch20-mcp-apps.toml quiz** - `c872b59` (feat)
2. **Task 2: Update SUMMARY.md for Phase 22-23 additions** - `78753fc` (feat)

## Files Created/Modified
- `pmcp-course/src/quizzes/ch20-mcp-apps.toml` - Refreshed quiz with 14 questions covering updated ch20 content
- `pmcp-course/src/part7-observability/ch18-exercises.md` - New exercises landing page for ch18 operations chapter
- `pmcp-course/src/SUMMARY.md` - Added ch18 exercises entry after ch18-03

## Decisions Made
- Kept 7 valid questions, replaced 1 outdated .with_ui() question with WidgetDir question, added 6 more new questions (14 total)
- Created ch18-exercises.md following ch17-exercises.md pattern since the file did not exist

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Created ch18-exercises.md**
- **Found during:** Task 2 (SUMMARY.md update)
- **Issue:** Plan instructed to add ch18 exercises entry to SUMMARY.md, but ch18-exercises.md did not exist on disk
- **Fix:** Created ch18-exercises.md following the ch17-exercises.md pattern with load testing and dashboard exercises
- **Files modified:** pmcp-course/src/part7-observability/ch18-exercises.md
- **Verification:** File exists, SUMMARY.md entry points to valid path
- **Committed in:** 78753fc (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Plan anticipated this case and instructed to create the file. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 24 plan 02 complete; all quizzes and SUMMARY updates for Phases 22-23 content are done
- Remaining: Phase 24 plan 01 (other chapter quizzes) if not yet completed

---
*Phase: 24-course-quizzes-exercises*
*Completed: 2026-02-28*
