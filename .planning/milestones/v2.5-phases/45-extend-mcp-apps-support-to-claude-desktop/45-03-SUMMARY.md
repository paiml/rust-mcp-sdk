---
phase: 45-extend-mcp-apps-support-to-claude-desktop
plan: 03
subsystem: preview
tags: [mcp-preview, chatgpt, standard-mode, metadata-enrichment, protocol-validation]

# Dependency graph
requires:
  - phase: 45-extend-mcp-apps-support-to-claude-desktop
    plan: 01
    provides: "Standard-only metadata emission, host layer enrichment pipeline"
  - phase: 44-improving-mcp-preview-to-support-chatgpt-version
    provides: "PreviewMode enum, ChatGPT mode protocol validation"
provides:
  - "mcp-preview ChatGPT mode metadata enrichment for openai/* keys"
  - "Standard mode as default with correct protocol validation"
  - "Migration guide for ChatGPT-only servers to cross-client pattern"
affects: [cargo-pmcp, examples, documentation]

# Tech tracking
tech-stack:
  added: []
  patterns: ["preview-side ChatGPT enrichment from standard ui.resourceUri", "mode-aware API response enrichment"]

key-files:
  created: []
  modified:
    - crates/mcp-preview/src/handlers/api.rs
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/lib.rs
    - crates/mcp-preview/assets/index.html
    - .planning/phases/45-extend-mcp-apps-support-to-claude-desktop/45-RESEARCH.md

key-decisions:
  - "mcp-preview enriches tool/resource _meta with ChatGPT keys in ChatGPT mode rather than requiring SDK servers to emit them"
  - "enrich_meta_for_chatgpt derives openai/* keys from standard ui.resourceUri nested key"
  - "Pre-existing issues (chess reset, map search_cities) documented but not fixed -- out of scope for phase 45"

patterns-established:
  - "Preview-side enrichment: mcp-preview acts as host layer emulator, enriching standard metadata for ChatGPT validation"

requirements-completed: [P45-PREVIEW-STANDARD, P45-EXAMPLES-VERIFY]

# Metrics
duration: 15min
completed: 2026-03-09
---

# Phase 45 Plan 03: Standard Mode Default and Example Verification Summary

**mcp-preview defaults to standard mode with ChatGPT mode enriching tool _meta via preview-side openai/* key injection from ui.resourceUri**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-09T19:30:00Z
- **Completed:** 2026-03-09T19:45:00Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments
- mcp-preview defaults to Standard MCP Apps mode with correct banner and config response
- ChatGPT mode protocol validation fixed: openai/* keys now enriched preview-side from standard ui.resourceUri
- Migration guide documenting ChatGPT-only to cross-client pattern (Open Images case study)
- Human-verified examples render correctly in both standard and ChatGPT modes

## Task Commits

Each task was committed atomically:

1. **Task 1: Update mcp-preview standard mode validation and config** - `dfaddae` (feat)
2. **Task 2: Review Open Images migration and document DX simplifications** - `4e4aba2` (docs)
3. **Task 3: Fix ChatGPT mode missing openai/* keys regression** - `066f406` (fix)

## Files Created/Modified
- `crates/mcp-preview/src/handlers/api.rs` - Added enrich_meta_for_chatgpt() and mode-aware enrichment in list_tools, list_resources, call_tool
- `crates/mcp-preview/src/server.rs` - PreviewMode::Standard as default, mode-aware banner
- `crates/mcp-preview/src/lib.rs` - Re-export updates
- `crates/mcp-preview/assets/index.html` - Standard mode config handling, 3-tier widget URI fallback
- `.planning/phases/45-extend-mcp-apps-support-to-claude-desktop/45-RESEARCH.md` - Migration guide appendix

## Decisions Made
- mcp-preview enriches tool/resource _meta with ChatGPT keys locally rather than requiring SDK servers to emit them. This is the correct architecture because mcp-preview acts as a host emulator -- the MCP server should not need to know which host is connecting.
- enrich_meta_for_chatgpt() uses or_insert_with to avoid overwriting keys that the SDK server might already provide (e.g., if a server still uses with_host_layer).
- Pre-existing widget issues (chess board not resetting on new game, map missing search_cities schema) are documented as known issues but not fixed -- they are unrelated to phase 45 changes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ChatGPT mode missing openai/* keys in protocol tab**
- **Found during:** Task 3 (human-verify checkpoint, user reported)
- **Issue:** After 45-01 refactored SDK to standard-only metadata, mcp-preview ChatGPT mode no longer received openai/* keys in tool _meta. The protocol validation tab showed all keys as MISSING.
- **Fix:** Added enrich_meta_for_chatgpt() function in api.rs that derives openai/* keys from standard ui.resourceUri. Applied to list_tools, list_resources, and call_tool handlers when mode is ChatGPT.
- **Files modified:** crates/mcp-preview/src/handlers/api.rs
- **Verification:** cargo build -p mcp-preview, cargo clippy -p mcp-preview -- -D warnings, workspace builds
- **Committed in:** 066f406

---

**Total deviations:** 1 auto-fixed (1 bug from 45-01 breaking change)
**Impact on plan:** Fix was necessary for ChatGPT mode to function correctly. No scope creep.

## Known Pre-existing Issues (Out of Scope)

1. **Chess board not resetting on new game** - Widget does not re-render board when chess_start_game returns new state. Pre-existing widget state management issue.
2. **Map search_cities missing tool definition** - The search_cities tool lacks parameter schema in the map example. Pre-existing example gap.

## Issues Encountered
- Test command in plan (`cargo test --lib -p pmcp`) incorrect -- pmcp is not a workspace member name. Used `cargo test --lib -p mcp-preview` instead.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 45 is now complete: SDK, bridge, and preview all support standard-first MCP Apps
- ChatGPT mode works via opt-in host layers (SDK) and preview-side enrichment (mcp-preview)
- Claude Desktop support works out of the box with standard metadata

---
*Phase: 45-extend-mcp-apps-support-to-claude-desktop*
*Completed: 2026-03-09*
