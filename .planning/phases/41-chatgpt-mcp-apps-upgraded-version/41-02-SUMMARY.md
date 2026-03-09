---
phase: 41-chatgpt-mcp-apps-upgraded-version
plan: 02
subsystem: ui
tags: [mcp-apps, chatgpt, bridge-protocol, postmessage, widget-runtime]

requires:
  - phase: 41-01
    provides: "Research on ChatGPT MCP Apps protocol method names"
provides:
  - "Bridge protocol aligned with official ChatGPT MCP Apps spec"
  - "Backward-compatible method name handling in widget-runtime.mjs"
  - "adapter.rs inject_bridge sends ui/initialize"
affects: [mcp-preview, mcp-apps]

tech-stack:
  added: []
  patterns: ["fall-through switch cases for backward compat protocol method names"]

key-files:
  created: []
  modified:
    - crates/mcp-preview/assets/widget-runtime.mjs
    - src/server/mcp_apps/adapter.rs

key-decisions:
  - "AppBridge class lives in widget-runtime.mjs (not duplicated in index.html), so all protocol changes centralized there"
  - "Backward compat via fall-through switch cases -- old method names still accepted"
  - "ui/notifications/initialized sent via setTimeout(0) after ui/initialize response to avoid blocking the response"

patterns-established:
  - "Fall-through switch: new official name first, old name with // backward compat comment"

requirements-completed: [P41-04]

duration: 2min
completed: 2026-03-07
---

# Phase 41 Plan 02: Bridge Protocol Method Names Summary

**Aligned bridge protocol in widget-runtime.mjs and adapter.rs with official ChatGPT MCP Apps method names (ui/notifications/*, ui/message, ui/open-link, ui/resource-teardown) while keeping old names as fallbacks**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-07T06:10:01Z
- **Completed:** 2026-03-07T06:12:28Z
- **Tasks:** 2 (Task 2 was no-op since AppBridge lives in widget-runtime.mjs not index.html)
- **Files modified:** 2

## Accomplishments
- Widget App class now sends ui/message and ui/open-link (was ui/sendMessage, ui/openLink)
- Notification handler accepts both new (ui/notifications/tool-input, etc.) and old (ui/toolInput, etc.) method names
- AppBridge host side sends ui/notifications/tool-input, ui/notifications/tool-result, ui/notifications/context-changed, ui/resource-teardown
- AppBridge accepts both ui/message and ui/sendMessage, both ui/open-link and ui/openLink from widgets
- AppBridge sends ui/notifications/initialized after ui/initialize handshake response
- adapter.rs inject_bridge sends ui/initialize instead of ui/ready

## Task Commits

Each task was committed atomically:

1. **Task 1: Update widget-runtime.mjs and adapter.rs bridge script method names** - `ea0f766` (feat)
2. **Task 2: Update index.html AppBridge host-side method names** - no-op (AppBridge class is in widget-runtime.mjs, not duplicated in index.html; all changes already in Task 1)

## Files Created/Modified
- `crates/mcp-preview/assets/widget-runtime.mjs` - Updated App class outgoing methods, notification handler, and AppBridge outgoing/incoming method names
- `src/server/mcp_apps/adapter.rs` - Changed inject_bridge() from ui/ready to ui/initialize

## Decisions Made
- AppBridge class lives in widget-runtime.mjs (imported by index.html), so all protocol string changes centralized in one file
- Backward compat via fall-through switch cases -- old method names still accepted from widgets
- ui/notifications/initialized sent via setTimeout(0) after ui/initialize response to ensure the response is sent first

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 2 was no-op -- AppBridge not in index.html**
- **Found during:** Task 2
- **Issue:** Plan expected AppBridge protocol strings in index.html, but AppBridge is defined in widget-runtime.mjs and imported by index.html
- **Fix:** All protocol method changes were already made in Task 1 where the AppBridge class actually lives
- **Files modified:** None additional
- **Verification:** grep confirmed no protocol string literals in index.html; mcp-preview builds

---

**Total deviations:** 1 (task consolidation due to code architecture)
**Impact on plan:** No scope change -- same work, different file location than plan assumed.

## Issues Encountered
- Pre-existing build error in pmcp crate (missing `meta` field in conversion.rs) prevented running adapter tests with `--features full`. This is unrelated to our changes -- mcp-preview builds and passes clippy cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Bridge protocol now aligned with ChatGPT MCP Apps spec
- Ready for further protocol alignment or feature work

---
*Phase: 41-chatgpt-mcp-apps-upgraded-version*
*Completed: 2026-03-07*
