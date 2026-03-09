---
phase: 44-improving-mcp-preview-to-support-chatgpt-version
plan: 01
subsystem: ui
tags: [mcp-preview, chatgpt, cli, axum, preview-mode]

requires:
  - phase: 43-chatgpt-mcp-apps-alignment
    provides: "_meta propagation via deep_merge in Server"
provides:
  - "PreviewMode enum (Standard/ChatGpt) exported from mcp-preview"
  - "PreviewConfig.mode field defaulting to Standard"
  - "--mode standard|chatgpt CLI flag on cargo pmcp preview"
  - "GET /api/config returns mode, descriptor_keys, invocation_keys"
  - "ResourceInfo and ResourceContentItem _meta passthrough from MCP server"
  - "Terminal banner displays active mode"
affects: [44-02-browser-side-mode-awareness]

tech-stack:
  added: []
  patterns: [mode-enum-with-display-for-api-serialization]

key-files:
  created: []
  modified:
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/proxy.rs
    - crates/mcp-preview/src/lib.rs
    - crates/mcp-preview/src/handlers/api.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/preview.rs

key-decisions:
  - "Hard-coded ChatGPT descriptor/invocation keys in api.rs since mcp-preview does not depend on pmcp crate"
  - "Used derive(Default) with #[default] attribute on Standard variant per clippy recommendation"
  - "Used serde rename _meta with Rust field name meta (leading underscore not idiomatic Rust)"

patterns-established:
  - "PreviewMode enum with Display impl for API serialization"

requirements-completed: [P44-MODE, P44-CONFIG, P44-RESOURCEMETA]

duration: 3min
completed: 2026-03-08
---

# Phase 44 Plan 01: Improving mcp-preview to Support ChatGPT Version Summary

**PreviewMode enum with Standard/ChatGpt variants, --mode CLI flag, ConfigResponse with descriptor keys, and _meta passthrough on proxy resource structs**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-08T22:32:00Z
- **Completed:** 2026-03-08T22:35:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- PreviewMode enum with Standard (default) and ChatGpt variants, Display impl for API serialization
- CLI --mode flag threads mode through to PreviewConfig and displays it in both CLI and terminal banners
- ConfigResponse extended with mode, 4 descriptor_keys, and 2 invocation_keys for browser consumption
- ResourceInfo, ResourceContentItem, and ResourceReadResult all passthrough _meta from MCP server

## Task Commits

Each task was committed atomically:

1. **Task 1: Add PreviewMode enum, extend PreviewConfig and proxy structs** - `c49dbfe` (feat)
2. **Task 2: Add --mode CLI flag and extend ConfigResponse with mode and keys** - `e5380e6` (feat)

## Files Created/Modified
- `crates/mcp-preview/src/server.rs` - PreviewMode enum, mode field on PreviewConfig, mode in banner
- `crates/mcp-preview/src/proxy.rs` - _meta fields on ResourceInfo, ResourceContentItem, ResourceReadResult
- `crates/mcp-preview/src/lib.rs` - Export PreviewMode
- `crates/mcp-preview/src/handlers/api.rs` - Extended ConfigResponse, _meta in resource handlers
- `cargo-pmcp/src/main.rs` - --mode CLI argument on Preview command
- `cargo-pmcp/src/commands/preview.rs` - Mode parsing, threading to PreviewConfig, display in output

## Decisions Made
- Hard-coded ChatGPT descriptor and invocation keys in api.rs since mcp-preview does not depend on the pmcp crate directly (added comment noting mirror)
- Used derive(Default) with #[default] attribute on Standard variant per clippy recommendation
- Used serde rename `_meta` with Rust field name `meta` (leading underscore not idiomatic Rust, consistent with Phase 41 decision)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy warning on manual Default impl for PreviewMode**
- **Found during:** Task 2 (clippy verification)
- **Issue:** Clippy requires derive(Default) with #[default] attribute instead of manual impl
- **Fix:** Replaced manual Default impl with derive macro and #[default] attribute
- **Files modified:** crates/mcp-preview/src/server.rs
- **Verification:** cargo clippy -p mcp-preview -p cargo-pmcp -- -D warnings passes clean
- **Committed in:** e5380e6 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor style fix required by clippy. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Rust-side plumbing complete for Plan 02 (browser-side mode awareness)
- Browser can read mode and validation keys from GET /api/config
- Resources properly forward _meta from MCP server through proxy

---
*Phase: 44-improving-mcp-preview-to-support-chatgpt-version*
*Completed: 2026-03-08*
