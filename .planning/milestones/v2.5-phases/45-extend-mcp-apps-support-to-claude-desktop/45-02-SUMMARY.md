---
phase: 45-extend-mcp-apps-support-to-claude-desktop
plan: 02
subsystem: ui
tags: [widget-runtime, typescript, extensions-namespace, bridge-api, chatgpt-compat]

requires:
  - phase: 45-01
    provides: Standard-only metadata emission with host layer enrichment system
provides:
  - McpBridge interface with standard methods at root and ChatGPT under extensions.chatgpt
  - ChatGptExtensions and McpBridgeExtensions type definitions
  - Compat layer that populates extensions.chatgpt only when window.openai detected
  - Runtime routing all ChatGPT accessors through extensions.chatgpt
  - Rebuilt widget-runtime.mjs with extensions namespace
affects: [mcp-preview, widget-runtime, mcp-apps-examples]

tech-stack:
  added: []
  patterns: [extensions-namespace for host-specific bridge capabilities]

key-files:
  created: []
  modified:
    - packages/widget-runtime/src/types.ts
    - packages/widget-runtime/src/compat.ts
    - packages/widget-runtime/src/runtime.ts
    - packages/widget-runtime/src/index.ts
    - packages/widget-runtime/src/browser.ts
    - crates/mcp-preview/assets/widget-runtime.mjs

key-decisions:
  - "ChatGptExtensions interface isolates all ChatGPT-specific methods and readonly properties"
  - "McpBridgeExtensions.claude reserved as Record<string, never> for future use"
  - "Window declaration uses intersection type to preserve legacy flat method access for backward compat"
  - "compat.ts buildChatGptExtensions() delegates directly to window.openai when available"
  - "Legacy flat methods kept on compat bridge with deprecation warnings for existing widgets"

patterns-established:
  - "Extensions namespace pattern: host-specific capabilities under mcpBridge.extensions.[host]"
  - "Helper accessor pattern: _getChatGptExtensions() in runtime for clean ChatGPT access"

requirements-completed: [P45-BRIDGE-NORMALIZE, P45-EXTENSIONS-NS]

duration: 5min
completed: 2026-03-09
---

# Phase 45 Plan 02: Bridge Normalization Summary

**Host-agnostic McpBridge with ChatGPT extensions namespace -- standard methods at root, ChatGPT-specific under extensions.chatgpt**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-09T19:23:30Z
- **Completed:** 2026-03-09T19:28:06Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- McpBridge interface refactored: standard MCP methods (callTool, readResource, getPrompt, openExternal, notify, sendIntent, openLink) at root level
- ChatGPT-specific methods/properties isolated under extensions.chatgpt namespace via ChatGptExtensions interface
- Runtime class routes all ChatGPT accessors through _getChatGptExtensions() helper
- Compat layer populates extensions.chatgpt only when window.openai detected; undefined on standard hosts
- Rebuilt widget-runtime.mjs deployed to mcp-preview assets with extensions namespace

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor McpBridge interface with extensions namespace** - `064c006` (feat)
2. **Task 2: Update compat layer and rebuild widget-runtime.mjs** - `7930366` (feat)

## Files Created/Modified
- `packages/widget-runtime/src/types.ts` - ChatGptExtensions, McpBridgeExtensions interfaces; McpBridge reduced to standard methods
- `packages/widget-runtime/src/compat.ts` - buildChatGptExtensions(); extensions namespace on bridge facade
- `packages/widget-runtime/src/runtime.ts` - _getChatGptExtensions() helper; all ChatGPT methods route through extensions
- `packages/widget-runtime/src/index.ts` - Export ChatGptExtensions, McpBridgeExtensions types
- `packages/widget-runtime/src/browser.ts` - Export ChatGptExtensions, McpBridgeExtensions types
- `crates/mcp-preview/assets/widget-runtime.mjs` - Rebuilt bundle with extensions namespace

## Decisions Made
- Window declaration uses intersection type (`McpBridge & { legacy methods }`) to preserve backward compat for existing widgets that access ChatGPT methods directly on window.mcpBridge
- buildChatGptExtensions() in compat.ts creates the ChatGPT extensions object by delegating to window.openai -- this keeps the native ChatGPT API as the source of truth
- Legacy flat methods preserved on compat bridge with deprecation warnings so existing widgets don't break
- McpBridgeExtensions.claude defined as `Record<string, never>` (empty) to reserve the namespace

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Widget-runtime bridge is now host-agnostic with proper namespace isolation
- Standard JSON-RPC postMessage protocol unchanged for Claude Desktop, VS Code, and other hosts
- ChatGPT-specific features properly namespaced under extensions.chatgpt
- Ready for example verification and end-to-end testing

---
*Phase: 45-extend-mcp-apps-support-to-claude-desktop*
*Completed: 2026-03-09*
