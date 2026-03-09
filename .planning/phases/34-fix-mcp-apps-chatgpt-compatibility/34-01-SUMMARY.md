---
phase: 34-fix-mcp-apps-chatgpt-compatibility
plan: 01
subsystem: ui
tags: [mcp-apps, chatgpt, openai, mime-type, metadata]

requires:
  - phase: 14-mcp-apps
    provides: MCP Apps type system and adapter architecture
provides:
  - Nested _meta.ui.resourceUri format across all tool metadata paths
  - openai/outputTemplate ChatGPT alias in tool _meta
  - HtmlMcpApp MIME variant (text/html;profile=mcp-app) in both UIMimeType and ExtendedUIMimeType
  - Dual-emit WidgetMeta for prefersBorder (both flat openai/* and nested ui)
  - Backward-compatible ToolUIMetadata parsing (reads both nested and legacy flat formats)
affects: [mcp-apps-examples, mcp-preview]

tech-stack:
  added: []
  patterns:
    - "Dual-emit metadata: both flat openai/* keys and nested ui object for cross-platform compatibility"
    - "Backward-compatible parsing: from_metadata() reads both nested and legacy flat key formats"

key-files:
  created: []
  modified:
    - src/types/protocol.rs
    - src/server/typed_tool.rs
    - src/types/ui.rs
    - src/types/mcp_apps.rs

key-decisions:
  - "HtmlMcpApp variant marks is_chatgpt()=true since ChatGPT uses this profile-based MIME type"
  - "ToolUIMetadata switched from serde-based struct to manual from_metadata/to_metadata for nested format control"
  - "WidgetMeta dual-emits prefersBorder only (not domain/csp/description which are ChatGPT-specific)"

patterns-established:
  - "Dual-emit pattern: MCP standard nested ui object plus flat openai/* keys for ChatGPT compatibility"

requirements-completed: [CHATGPT-01, CHATGPT-02, CHATGPT-03, CHATGPT-04, CHATGPT-05]

duration: 11min
completed: 2026-03-06
---

# Phase 34 Plan 01: Fix MCP Apps ChatGPT Compatibility Summary

**Nested _meta.ui.resourceUri format with openai/outputTemplate alias and HtmlMcpApp MIME type for ChatGPT MCP Apps compatibility**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-06T20:33:21Z
- **Completed:** 2026-03-06T20:44:34Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Unified tool _meta format: both ToolInfo::with_ui() and TypedTool::metadata() now produce identical nested {"ui": {"resourceUri": ...}, "openai/outputTemplate": ...} format
- Added HtmlMcpApp MIME variant to both UIMimeType and ExtendedUIMimeType for "text/html;profile=mcp-app"
- WidgetMeta dual-emits prefersBorder in both flat "openai/widgetPrefersBorder" and nested "ui.prefersBorder" formats
- ToolUIMetadata reads both nested and legacy flat key formats for backward compatibility

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix tool _meta format and add MIME type variants** - `eb6607a` (feat)
2. **Task 2: Fix TypedTool metadata and WidgetMeta dual-emit** - `c702463` (feat)

## Files Created/Modified
- `src/types/protocol.rs` - ToolInfo::with_ui() now produces nested _meta with openai/outputTemplate
- `src/server/typed_tool.rs` - TypedTool::metadata() adds openai/outputTemplate alongside nested ui.resourceUri
- `src/types/ui.rs` - HtmlMcpApp MIME variant, ToolUIMetadata uses nested format with backward-compatible parsing
- `src/types/mcp_apps.rs` - HtmlMcpApp in ExtendedUIMimeType, WidgetMeta dual-emits prefersBorder

## Decisions Made
- HtmlMcpApp variant marks is_chatgpt()=true since ChatGPT uses this profile-based MIME type
- ToolUIMetadata switched from serde-based struct to manual from_metadata/to_metadata for nested format control
- WidgetMeta dual-emits prefersBorder only (not domain/csp/description which are ChatGPT-specific)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Pre-existing doctest failures in server/preset.rs and shared/middleware_presets.rs (3 failures) - unrelated to this plan's changes, not in files we modified.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- SDK now produces ChatGPT-compatible metadata format across all tool metadata paths
- mcp-preview wildcard route fix is planned for a separate plan (34-02)

---
*Phase: 34-fix-mcp-apps-chatgpt-compatibility*
*Completed: 2026-03-06*
