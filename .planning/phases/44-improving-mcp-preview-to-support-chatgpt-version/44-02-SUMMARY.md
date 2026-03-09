---
phase: 44-improving-mcp-preview-to-support-chatgpt-version
plan: 02
subsystem: ui
tags: [mcp-preview, chatgpt, protocol-validation, postmessage, devtools]

requires:
  - phase: 44-01
    provides: "PreviewMode enum, --mode CLI flag, ConfigResponse with descriptor/invocation keys"
provides:
  - "Protocol diagnostics tab in DevTools with pass/fail validation for _meta keys"
  - "ChatGPT mode postMessage emulation with JSON-RPC ui/notifications/tool-result envelope"
  - "window.openai stub injection for ChatGPT host detection"
  - "Mode badge in browser header (Standard green / ChatGPT red)"
affects: [mcp-preview, widget-runtime]

tech-stack:
  added: []
  patterns:
    - "Protocol validation as warn-only diagnostics (never blocking)"
    - "Widget reload skip when same URI already loaded"
    - "ChatGPT postMessage as supplemental delivery alongside AppBridge"

key-files:
  created: []
  modified:
    - "crates/mcp-preview/assets/index.html"

key-decisions:
  - "AppBridge remains active in ChatGPT mode for ui/initialize handshake; postMessage is supplemental"
  - "Skip widget iframe reload when same resource URI is already displayed to preserve widget state"
  - "Protocol validation is warn-only with color-coded severity (red in ChatGPT, yellow in Standard)"

patterns-established:
  - "deliverToolResult helper for unified AppBridge + postMessage delivery"
  - "widgetAlreadyLoaded guard to avoid unnecessary iframe reloads"

requirements-completed: [P44-PROTOCOL-TAB, P44-CHATGPT-EMULATION, P44-BADGE]

duration: 25min
completed: 2026-03-08
---

# Phase 44 Plan 02: Browser-Side Protocol Tab and ChatGPT Emulation Summary

**Protocol diagnostics tab with pass/fail _meta key validation, ChatGPT postMessage emulation with window.openai stub, and widget reload skip for stable UI updates**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-08T23:00:00Z
- **Completed:** 2026-03-08T23:25:00Z
- **Tasks:** 3 (2 auto + 1 checkpoint with bug fix)
- **Files modified:** 1

## Accomplishments
- Protocol tab in DevTools validates _meta keys against descriptor/invocation key sets from config
- ChatGPT mode injects window.openai stub and delivers tool results via postMessage with JSON-RPC envelope
- Mode badge in browser header shows "Standard" (green) or "ChatGPT Strict" (red)
- Fixed widget UI not updating on subsequent tool calls by skipping iframe reload when same URI is loaded

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Protocol tab UI and validation logic** - `5d52c1f` (feat)
2. **Task 2: ChatGPT mode postMessage emulation and window.openai stub** - `7c8bcf7` (feat)
3. **Task 3: Fix widget UI not updating on tool response** - `d7ebc92` (fix)

## Files Created/Modified
- `crates/mcp-preview/assets/index.html` - Protocol tab UI, validation logic, mode badge, ChatGPT postMessage delivery, window.openai stub injection, widget reload skip

## Decisions Made
- AppBridge remains active in ChatGPT mode for ui/initialize handshake; ChatGPT postMessage is supplemental, not a replacement
- Skip widget iframe reload when the same resource URI is already displayed, preventing destruction of widget state on subsequent tool calls
- Protocol validation is warn-only with expandable key diffs; red severity in ChatGPT mode, informational yellow in Standard mode

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Widget UI not updating on tool response**
- **Found during:** Task 3 (checkpoint human-verify)
- **Issue:** Every tool response triggered a full iframe reload via loadResourceWidget, destroying the existing AppBridge and widget state. The 300ms setTimeout raced with widget initialization, so sendToolResult often fired before the widget was ready.
- **Fix:** Added widgetAlreadyLoaded guard that checks if the same URI is already displayed. When true, delivers tool result data immediately via the existing AppBridge without reloading the iframe. First load or different URI still triggers full loadResourceWidget flow.
- **Files modified:** crates/mcp-preview/assets/index.html
- **Verification:** User confirmed widget updates correctly on subsequent tool calls
- **Committed in:** d7ebc92

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for widget data delivery. No scope creep.

## Issues Encountered
None beyond the widget reload bug reported during checkpoint verification.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 44 is complete -- mcp-preview supports both Standard and ChatGPT modes
- Protocol diagnostics tab provides developer feedback on _meta key compliance
- Ready to proceed with Phase 27 (Global Flag Infrastructure) or other planned work

---
*Phase: 44-improving-mcp-preview-to-support-chatgpt-version*
*Completed: 2026-03-08*
