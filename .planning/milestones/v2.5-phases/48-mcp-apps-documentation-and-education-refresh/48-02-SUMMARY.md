---
phase: 48-mcp-apps-documentation-and-education-refresh
plan: 02
subsystem: docs
tags: [mcp-apps, course, education, ext-apps, widget, mcp-tester]

# Dependency graph
requires:
  - phase: 48-mcp-apps-documentation-and-education-refresh
    provides: Updated book ch12-5 and ch15 with standard-first GUIDE.md content
provides:
  - Updated ch20-mcp-apps.md with standard-first paradigm (with_host_layer, ToolInfo::with_ui)
  - Updated ch20-01-ui-resources.md with UIResource::html_mcp_app and WidgetCSP
  - Updated ch20-02-tool-ui-association.md with ToolInfo::with_ui, structuredContent, with_host_layer
  - Updated ch20-03-postmessage.md with ext-apps App class, required handlers, Vite bundling
  - Updated ch20-exercises.md with current API exercises and mcp-tester apps validation
  - App Metadata Validation section in ch11-02-mcp-tester.md
affects: [course-content, mcp-apps-documentation]

# Tech tracking
tech-stack:
  added: []
  patterns: [teaching-oriented-standard-first, ext-apps-lifecycle-teaching]

key-files:
  created: []
  modified:
    - pmcp-course/src/part8-advanced/ch20-mcp-apps.md
    - pmcp-course/src/part8-advanced/ch20-01-ui-resources.md
    - pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md
    - pmcp-course/src/part8-advanced/ch20-03-postmessage.md
    - pmcp-course/src/part8-advanced/ch20-exercises.md
    - pmcp-course/src/part4-testing/ch11-02-mcp-tester.md

key-decisions:
  - "Eliminated all ChatGptAdapter, WidgetDir, and window.mcpBridge-first content from course -- standard SDK APIs (with_host_layer, ToolInfo::with_ui, ext-apps App class) are now primary"
  - "Restructured ch20 sections: ch20-01 focuses on resource registration, ch20-02 on tool-UI association, ch20-03 on ext-apps widget development"
  - "Added prominent warning block for required protocol handlers (onteardown, etc.) in ch20-03 since this is the #1 deployment issue"

patterns-established:
  - "Course teaches standard-first: with_host_layer and standard SDK APIs appear before any ChatGPT-specific content"
  - "ext-apps lifecycle is the primary widget pattern: create App, register handlers, connect, read hostContext"

requirements-completed: [DOCS-04]

# Metrics
duration: 6min
completed: 2026-03-12
---

# Phase 48 Plan 02: Course MCP Apps Chapter Update Summary

**Updated 5 course ch20 files and ch11-02 mcp-tester lesson to teach standard-first MCP Apps APIs (ToolInfo::with_ui, ext-apps App class, with_host_layer, UIResource::html_mcp_app) with teaching-oriented learning objectives, code examples, and mcp-tester apps validation**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-12T19:55:37Z
- **Completed:** 2026-03-12T20:01:40Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Course ch20 intro chapter rewritten with standard-first paradigm, architecture diagram, and developer tooling overview
- ch20-01 now teaches UIResource::html_mcp_app(), UIResourceContents::html(), WidgetCSP, ResourceCollection, and the three-step read pattern
- ch20-02 now teaches ToolInfo::with_ui(), with_structured_content(), with_host_layer(), WidgetMeta, and with_output_schema()
- ch20-03 now teaches ext-apps App class lifecycle, required protocol handlers with prominent warning, Vite bundling, React hooks, and fetch-blob pattern
- ch20-exercises updated with current API exercises including mcp-tester apps validation
- ch11-02 mcp-tester lesson has new App Metadata Validation section with CLI examples for all modes

## Task Commits

Each task was committed atomically:

1. **Task 1: Update course ch20 MCP Apps chapters** - `86a9611` (docs)
2. **Task 2: Add App Metadata Testing to course mcp-tester lesson** - `bf314a8` (docs)

## Files Created/Modified
- `pmcp-course/src/part8-advanced/ch20-mcp-apps.md` - Rewritten intro with standard-first paradigm, architecture diagram, developer tooling
- `pmcp-course/src/part8-advanced/ch20-01-ui-resources.md` - UIResource::html_mcp_app, WidgetCSP, ResourceCollection, MIME type
- `pmcp-course/src/part8-advanced/ch20-02-tool-ui-association.md` - ToolInfo::with_ui, structuredContent, with_host_layer, WidgetMeta, outputSchema
- `pmcp-course/src/part8-advanced/ch20-03-postmessage.md` - ext-apps App class, required handlers, Vite bundling, React, fetch-blob
- `pmcp-course/src/part8-advanced/ch20-exercises.md` - Updated exercises with current APIs and mcp-tester apps
- `pmcp-course/src/part4-testing/ch11-02-mcp-tester.md` - Added App Metadata Validation section

## Decisions Made
- Eliminated ChatGptAdapter, WidgetDir, and window.mcpBridge content entirely from course chapters -- these were the old paradigm from Phase 21/23
- Restructured ch20 section titles to match content: "UI Resources and Widget Registration", "Tool-UI Association and Data Flow", "Widget Communication with ext-apps"
- Added prominent warning block (not just inline text) for required protocol handlers since this is the #1 deployment failure
- Course content aligns with updated book ch12-5 from plan 48-01 but uses teaching-oriented format (learning objectives, try-it callouts, knowledge checks)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All phase 48 content is now complete (plans 01, 02, and 03 all executed)
- Course and book documentation are aligned with the current SDK APIs through Phase 47

## Self-Check: PASSED

All files verified, all commits confirmed, all verification criteria met.

---
*Phase: 48-mcp-apps-documentation-and-education-refresh*
*Completed: 2026-03-12*
