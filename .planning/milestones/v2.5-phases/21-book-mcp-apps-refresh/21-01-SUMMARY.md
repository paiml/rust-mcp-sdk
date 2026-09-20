---
phase: 21-book-mcp-apps-refresh
plan: 01
subsystem: docs
tags: [book, mcp-apps, widgetdir, cargo-pmcp, widgets, hot-reload]

# Dependency graph
requires:
  - phase: 14-19 (v1.3 MCP Apps Developer Experience)
    provides: WidgetDir, ChatGptAdapter, cargo pmcp app/preview CLI, examples
provides:
  - Rewritten Ch 12.5 first half covering WidgetDir authoring, bridge API, and developer workflow
affects: [21-02 (adapter pattern and example walkthroughs)]

# Tech tracking
tech-stack:
  added: []
  patterns: [WidgetDir file-based widget authoring, ResourceHandler pattern, mcpBridge API]

key-files:
  created: []
  modified:
    - pmcp-book/src/ch12-5-mcp-apps.md

key-decisions:
  - "Organized chapter into 5 major sections matching the developer journey: intro, quick start, widget authoring, bridge communication, developer workflow"
  - "Documented full mcpBridge API including ChatGPT-only methods (setState, sendMessage, requestDisplayMode) with clear host-availability annotations"
  - "Used tables for all API references (WidgetEntry fields, mcpBridge methods, CLI flags) for scanability"

patterns-established:
  - "Book chapter structure: intro -> quick start -> core concepts -> tooling workflow -> continuation marker"
  - "Source-code-faithful documentation: all API details extracted from actual Rust and JS source"

requirements-completed: [BKAP-01, BKAP-02]

# Metrics
duration: 3min
completed: 2026-02-28
---

# Phase 21 Plan 01: MCP Apps Chapter Rewrite Summary

**Rewrote Ch 12.5 from UIResourceBuilder/inline HTML approach to WidgetDir file-based authoring with full cargo pmcp CLI workflow documentation**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-28T02:20:01Z
- **Completed:** 2026-02-28T02:23:48Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Replaced entire 843-line UIResourceBuilder-based chapter with 712-line WidgetDir-based chapter
- Documented WidgetDir API (new, discover, read_widget, inject_bridge_script) with all fields matching source
- Documented complete window.mcpBridge bridge API with tables for core ops, state management, communication, display modes, and environment context
- Documented full cargo pmcp app CLI: scaffolding (new), preview (7 flags), build/manifest/landing (all flags and defaults)
- Added ResourceHandler pattern showing the identical three-step pattern used across chess, map, and dataviz examples

## Task Commits

Each task was committed atomically:

1. **Task 1: Rewrite Ch 12.5 intro, WidgetDir authoring, and quick start** - `61478a2` (feat)
2. **Task 2: Write developer workflow section (cargo pmcp app new/build/preview)** - `97030a4` (feat)

## Files Created/Modified
- `pmcp-book/src/ch12-5-mcp-apps.md` - Complete chapter rewrite: intro, quick start, WidgetDir authoring, bridge communication, developer workflow

## Decisions Made
- Organized chapter into 5 major sections matching the developer journey from scaffolding to distribution
- Documented full mcpBridge API including ChatGPT-only methods with clear host-availability annotations in tables
- Used the actual scaffolded main.rs as the Quick Start code example rather than a simplified snippet, so readers see the real pattern
- Ended file with continuation comment for Plan 21-02 (adapter pattern and example walkthroughs)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Ch 12.5 first half is complete with continuation marker for Plan 21-02
- Plan 21-02 should add: adapter pattern section (ChatGptAdapter, McpAppsAdapter, McpUiAdapter), example walkthroughs (chess, map, dataviz), best practices, and security considerations

---
*Phase: 21-book-mcp-apps-refresh*
*Completed: 2026-02-28*
