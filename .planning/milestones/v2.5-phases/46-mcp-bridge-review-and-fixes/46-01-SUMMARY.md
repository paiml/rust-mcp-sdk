---
phase: 46-mcp-bridge-review-and-fixes
plan: 01
subsystem: ui
tags: [mcp-apps, bridge, postmessage, widget-runtime, chatgpt, adapter]

requires:
  - phase: 45-extend-mcp-apps-support-to-claude-desktop
    provides: "Adapter bridge scripts, App class, AppBridge class"
provides:
  - "Dual method name support in all bridge scripts (short-form and long-form)"
  - "onToolResult/onToolInput/onToolCancelled callback API on McpApps mcpBridge"
  - "App class _normalizeMethod() for long-form to short-form mapping"
affects: [mcp-preview, widget-runtime, mcp-apps-examples]

tech-stack:
  added: []
  patterns: ["method name normalization map for spec version compatibility"]

key-files:
  created: []
  modified:
    - src/server/mcp_apps/adapter.rs
    - packages/widget-runtime/src/app.ts
    - crates/mcp-preview/assets/widget-runtime.mjs

key-decisions:
  - "Used static lookup map for method normalization (O(1), no string manipulation chains)"
  - "McpApps bridge stores callbacks as _onToolResult properties with getter/setter pairs for clean API"
  - "Normalization happens in both widget-runtime App class AND injected bridge scripts for defense-in-depth"

patterns-established:
  - "Method name aliasing: always support both ui/toolResult (short) and ui/notifications/tool-result (long) forms"

requirements-completed: [BRIDGE-01, BRIDGE-02, BRIDGE-03]

duration: 4min
completed: 2026-03-10
---

# Phase 46 Plan 01: MCP Bridge Review and Fixes Summary

**Fixed bridge protocol method name mismatch preventing tool results from reaching widgets, added onToolResult callback API to McpApps bridge**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-10T18:35:40Z
- **Completed:** 2026-03-10T18:39:46Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- App class now normalizes long-form spec method names (ui/notifications/tool-result) to short-form (ui/toolResult) before dispatch
- ChatGPT adapter bridge listens for both short and long form method names, preserving window.openai path
- McpApps adapter bridge exposes onToolResult/onToolInput/onToolCancelled callback setters on window.mcpBridge
- 3 new adapter tests confirming dual method form support and callback API
- widget-runtime.mjs rebuilt and copied to mcp-preview assets

## Task Commits

Each task was committed atomically:

1. **Task 1: Add method name normalization and fix adapter bridge scripts** - `a3ebbcb` (feat)
2. **Task 2: Update adapter tests and rebuild widget-runtime** - `a75adbe` (test)

## Files Created/Modified
- `src/server/mcp_apps/adapter.rs` - Fixed ChatGPT and McpApps bridge scripts with dual method form support and onToolResult callbacks
- `packages/widget-runtime/src/app.ts` - Added _normalizeMethod() and _METHOD_ALIASES map to App class
- `crates/mcp-preview/assets/widget-runtime.mjs` - Rebuilt from updated widget-runtime source

## Decisions Made
- Used static Record<string, string> lookup map for method normalization (O(1) per call, cleaner than string replacement chains)
- McpApps bridge callbacks stored as _onToolResult/etc properties with getter/setter pairs on mcpBridge object
- Applied normalization in both the widget-runtime App class AND the injected McpApps bridge script for defense-in-depth

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Adapter tests require `--features mcp-apps` flag since the module is behind a feature gate. Discovered during verification and used correct feature flag.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Bridge protocol alignment complete for both ChatGPT and standard MCP Apps hosts
- Widgets should now receive tool results regardless of which method name form the host sends
- Ready for end-to-end testing with Claude Desktop and other MCP hosts

---
*Phase: 46-mcp-bridge-review-and-fixes*
*Completed: 2026-03-10*
