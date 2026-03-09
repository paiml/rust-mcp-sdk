---
phase: 41-chatgpt-mcp-apps-upgraded-version
plan: 03
subsystem: cli
tags: [mcp-apps, scaffold, chatgpt, mime-type, template]

# Dependency graph
requires:
  - phase: 41-01
    provides: Content::Resource meta field and HtmlMcpApp MIME type
  - phase: 41-02
    provides: Widget runtime bridge protocol aligned with ChatGPT spec
provides:
  - Updated cargo pmcp new --mcp-app scaffold template with ChatGPT-compatible code
affects: [cargo-pmcp, mcp-apps-examples]

# Tech tracking
tech-stack:
  added: []
  patterns: [TypedSyncTool with_ui for tool-to-widget linking in scaffold]

key-files:
  created: []
  modified:
    - cargo-pmcp/src/templates/mcp_app.rs
    - crates/mcp-tester/src/tester.rs

key-decisions:
  - "Used TypedSyncTool::new().with_ui() instead of tool_typed_sync_with_description() for scaffold tool registration to enable UI resource linking"

patterns-established:
  - "Scaffold template uses TypedSyncTool builder with .with_ui() for ChatGPT-compatible tool-to-widget binding"

requirements-completed: [P41-05]

# Metrics
duration: 4min
completed: 2026-03-07
---

# Phase 41 Plan 03: Scaffold Template Update Summary

**Updated cargo pmcp new --mcp-app scaffold to use HtmlMcpApp MIME type, TypedSyncTool with .with_ui(), and Content::Resource _meta emission**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-07T06:17:58Z
- **Completed:** 2026-03-07T06:21:44Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Replaced HtmlSkybridge with HtmlMcpApp MIME type in both read() and list() resource handlers
- Added .with_ui("ui://app/hello.html") on tool registration via TypedSyncTool builder pattern
- Added _meta emission with WidgetMeta on Content::Resource in read handler
- Updated test assertions to verify HtmlMcpApp, with_ui, and meta fields
- Fixed mcp-tester Content::Resource pattern matches for new meta field (blocking issue from 41-01)

## Task Commits

Each task was committed atomically:

1. **Task 1: Update scaffold template MIME type, with_ui, and resource _meta** - `3aecf2f` (feat)

## Files Created/Modified
- `cargo-pmcp/src/templates/mcp_app.rs` - Updated scaffold template with HtmlMcpApp, TypedSyncTool with_ui, and Content::Resource meta
- `crates/mcp-tester/src/tester.rs` - Fixed Content::Resource pattern matches to use `..` for new meta field

## Decisions Made
- Used TypedSyncTool::new().with_description().with_ui() instead of tool_typed_sync_with_description() to enable the .with_ui() call for UI resource linking in the scaffold template

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed mcp-tester Content::Resource pattern matches**
- **Found during:** Task 1 (verification)
- **Issue:** mcp-tester had two Content::Resource destructuring patterns missing the new `meta` field added in 41-01, causing compilation failure
- **Fix:** Added `..` to two Content::Resource match arms in crates/mcp-tester/src/tester.rs
- **Files modified:** crates/mcp-tester/src/tester.rs
- **Verification:** cargo test -p cargo-pmcp passes, cargo clippy clean
- **Committed in:** 3aecf2f (part of task commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Auto-fix necessary for compilation. No scope creep.

## Issues Encountered
None beyond the mcp-tester blocking issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All three plans in phase 41 complete
- Scaffold template generates ChatGPT-compatible MCP App code out of the box
- New projects use correct MIME type, tool-to-widget linking, and resource metadata

---
*Phase: 41-chatgpt-mcp-apps-upgraded-version*
*Completed: 2026-03-07*
