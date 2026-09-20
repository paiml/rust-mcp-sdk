---
phase: 46-mcp-bridge-review-and-fixes
plan: 02
subsystem: ui
tags: [mcp-preview, postMessage, widget, bridge, mcp-apps]

requires:
  - phase: 45-extend-mcp-apps-support-to-claude-desktop
    provides: "AppBridge, widget-runtime.mjs, mcp-preview ChatGPT/standard mode infrastructure"
provides:
  - "Readiness-based tool result delivery replacing fragile setTimeout"
  - "Dual method name emission (ui/toolResult + ui/notifications/tool-result)"
affects: [mcp-apps, mcp-preview, widget-runtime]

tech-stack:
  added: []
  patterns: ["readiness signal via ui/notifications/initialized before tool result delivery"]

key-files:
  created: []
  modified: ["crates/mcp-preview/assets/index.html"]

key-decisions:
  - "Use ui/toolResult as primary short form with ui/notifications/tool-result as long form fallback"
  - "3-second fallback timeout for legacy widgets that may not send ui/notifications/initialized"
  - "wrapWidgetHtml ready signal fires synchronously after connect() instead of 100ms setTimeout"

patterns-established:
  - "Readiness signal pattern: listen for ui/notifications/initialized before delivering data to widget"
  - "Dual method emission: send both short-form and long-form method names for compatibility"

requirements-completed: [BRIDGE-04, BRIDGE-05]

duration: 1min
completed: 2026-03-10
---

# Phase 46 Plan 02: MCP Preview Tool Result Delivery Fix Summary

**Replaced fragile 300ms setTimeout with ui/notifications/initialized readiness signal and added dual-emit ui/toolResult + ui/notifications/tool-result for widget compatibility**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-10T18:35:40Z
- **Completed:** 2026-03-10T18:36:53Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Tool result delivery now waits for widget readiness signal (ui/notifications/initialized) with 3s fallback timeout instead of hardcoded 300ms setTimeout
- deliverToolResult sends both short-form (ui/toolResult) and long-form (ui/notifications/tool-result) method names for maximum compatibility with App class and legacy widgets
- wrapWidgetHtml ready signal fires synchronously after connect() handshake instead of 100ms delay
- ChatGPT mode AppBridge, window.openai injection, and widgetAlreadyLoaded fast path all unchanged

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace setTimeout with readiness signal and add dual method emission** - `fdaccac` (fix)
2. **Task 2: Verify mcp-preview builds and test** - verification only, no code changes

## Files Created/Modified
- `crates/mcp-preview/assets/index.html` - Updated deliverToolResult to emit dual method names, replaced setTimeout with readiness Promise, removed wrapWidgetHtml setTimeout

## Decisions Made
- Used ui/toolResult as primary (short form matches ext-apps SDK App class) with ui/notifications/tool-result as fallback for older widgets
- 3-second fallback timeout chosen for legacy widgets that may never send initialized signal
- wrapWidgetHtml ready signal made synchronous since connect() handshake already guarantees initialization

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- mcp-preview tool result delivery is now robust against race conditions on first widget load
- Both standard and ChatGPT mode widgets will receive tool results via their preferred method name

---
*Phase: 46-mcp-bridge-review-and-fixes*
*Completed: 2026-03-10*
