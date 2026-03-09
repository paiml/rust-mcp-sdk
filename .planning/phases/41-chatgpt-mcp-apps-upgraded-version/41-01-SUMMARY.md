---
phase: 41-chatgpt-mcp-apps-upgraded-version
plan: 01
subsystem: ui
tags: [mcp-apps, chatgpt, mime-type, resource-metadata, serde]

requires:
  - phase: 34-fix-mcp-apps-chatgpt-compatibility
    provides: ChatGPT adapter and HtmlSkybridge/HtmlMcpApp MIME types
  - phase: 40-review-chatgpt-compatibility-for-apps
    provides: Dual-emit metadata pattern for ChatGPT compatibility
provides:
  - Content::Resource with optional _meta field for resource-level widget metadata
  - UIResourceContents with optional meta field
  - ChatGptAdapter preferring HtmlMcpApp MIME type (text/html;profile=mcp-app)
affects: [41-02, 41-03, mcp-apps, chatgpt-adapter]

tech-stack:
  added: []
  patterns: [resource-level _meta for widget metadata, serde rename for _meta field]

key-files:
  created: []
  modified:
    - src/types/protocol.rs
    - src/types/ui.rs
    - src/types/mcp_apps.rs
    - src/server/mcp_apps/adapter.rs
    - src/server/mcp_apps/builder.rs
    - src/server/simple_resources.rs
    - src/server/workflow/conversion.rs
    - src/composition/mcp_client.rs
    - src/lib.rs

key-decisions:
  - "Used field name `meta` with serde rename to `_meta` since leading underscores are not idiomatic Rust identifiers"
  - "HtmlSkybridge kept in codebase with deprecation doc comment rather than removed, for backward compatibility"
  - "supports_mime_type still accepts both HtmlSkybridge and HtmlMcpApp for ChatGpt host type"

patterns-established:
  - "Resource-level _meta: Optional serde_json::Map on Content::Resource for widget metadata passthrough"

requirements-completed: [P41-01, P41-02, P41-03]

duration: 6min
completed: 2026-03-07
---

# Phase 41 Plan 01: Resource _meta and ChatGPT MIME Type Summary

**Added _meta field to Content::Resource and UIResourceContents; switched ChatGptAdapter from HtmlSkybridge to HtmlMcpApp (text/html;profile=mcp-app)**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-07T06:10:24Z
- **Completed:** 2026-03-07T06:16:03Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Content::Resource enum variant now carries optional `_meta` field for resource-level widget metadata (widgetDescription, CSP, domain)
- UIResourceContents struct has matching `meta` field that flows through to Content::Resource on resource reads
- ChatGptAdapter MIME type changed from `text/html+skybridge` to `text/html;profile=mcp-app` in both HostType::preferred_mime_type() and UIAdapter::mime_type()
- All 15+ construction and destructuring sites updated to compile cleanly
- Full serialization round-trip tests for _meta field

## Task Commits

Each task was committed atomically:

1. **Task 1: Add _meta field to Content::Resource and UIResourceContents** - `5ce9db9` (feat)
2. **Task 2: Fix ChatGptAdapter MIME type to HtmlMcpApp** - `63815ea` (feat)

## Files Created/Modified
- `src/types/protocol.rs` - Added optional `meta` field to Content::Resource with serde rename to `_meta`; added 4 serialization/deserialization tests
- `src/types/ui.rs` - Added optional `meta` field to UIResourceContents; updated html() constructor
- `src/types/mcp_apps.rs` - Changed ChatGpt preferred_mime_type to HtmlMcpApp; added deprecation doc on HtmlSkybridge; updated test assertions
- `src/server/mcp_apps/adapter.rs` - Changed ChatGptAdapter mime_type() to HtmlMcpApp; updated test assertion and doc comment
- `src/server/mcp_apps/builder.rs` - Updated builder test to expect HtmlMcpApp
- `src/server/simple_resources.rs` - Added `meta: None` to 2 construction sites, `meta: contents.meta.clone()` to 1 UI read site; updated 5 test destructurings with `..`
- `src/server/workflow/conversion.rs` - Added `meta: None` to 2 MessageContent::Resource constructions
- `src/composition/mcp_client.rs` - Added `..` to 2 Content::Resource destructuring match arms
- `src/lib.rs` - Added `meta: None` to doc example Content::Resource

## Decisions Made
- Used field name `meta` with `#[serde(rename = "_meta")]` since `_meta` with leading underscore is not idiomatic in Rust
- Kept HtmlSkybridge variant with deprecation doc comment rather than removing it, for backward compatibility
- ChatGpt `supports_mime_type` still accepts both HtmlSkybridge and HtmlMcpApp

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Resource-level _meta field ready for 41-02 to populate with widget metadata from WidgetMeta
- ChatGptAdapter MIME type correct for ChatGPT MCP Apps protocol
- All 854 lib tests pass, zero clippy warnings

---
*Phase: 41-chatgpt-mcp-apps-upgraded-version*
*Completed: 2026-03-07*
