---
phase: 45-extend-mcp-apps-support-to-claude-desktop
plan: 01
subsystem: api
tags: [mcp-apps, metadata, host-layer, claude-desktop, chatgpt, serde_json]

# Dependency graph
requires:
  - phase: 43-chatgpt-mcp-apps-alignment
    provides: "URI-to-tool-meta index, _meta propagation to resources"
  - phase: 34-fix-mcp-apps-chatgpt-compatibility
    provides: "Triple-key metadata emission, WidgetMeta, ChatGptToolMeta"
provides:
  - "Standard-only build_meta_map and emit_resource_uri_keys"
  - "with_host_layer(HostType) builder method on ServerCoreBuilder"
  - "enrich_meta_for_host() enrichment pipeline for host-specific _meta keys"
  - "Standard-keyed build_uri_to_tool_meta index (ui.resourceUri, not openai/outputTemplate)"
affects: [45-02, 45-03, mcp-preview, widget-runtime, examples, cargo-pmcp]

# Tech tracking
tech-stack:
  added: []
  patterns: ["host-layer registration on builder", "build-time metadata enrichment pipeline", "standard-only default with opt-in host layers"]

key-files:
  created: []
  modified:
    - src/types/ui.rs
    - src/types/mcp_apps.rs
    - src/server/builder.rs
    - src/server/core.rs
    - src/server/typed_tool.rs
    - src/types/protocol.rs
    - src/server/mcp_apps/adapter.rs

key-decisions:
  - "Standard-only metadata emission by default: build_meta_map returns only ui.resourceUri nested key"
  - "Host layer enrichment at build time via enrich_meta_for_host(), not at request time"
  - "build_uri_to_tool_meta indexes by standard ui.resourceUri, not openai/outputTemplate"
  - "ChatGptAdapter always emits openai/outputTemplate from resource URI (adapter-level enrichment)"
  - "Resource propagation uses prefix-based filter for openai/* keys"

patterns-established:
  - "Host layer pattern: .with_host_layer(HostType::ChatGpt) on ServerCoreBuilder for opt-in host-specific keys"
  - "Build-time enrichment: host layers iterate tool_infos and inject host-specific _meta keys before ServerCore construction"
  - "Standard-first metadata: all _meta emission functions produce only MCP standard keys; host-specific keys are additive"

requirements-completed: [P45-STANDARD-DEFAULT, P45-HOST-LAYER, P45-URI-INDEX]

# Metrics
duration: 11min
completed: 2026-03-09
---

# Phase 45 Plan 01: Standard-Only Metadata with Host Layer System Summary

**Refactored SDK metadata emission to standard-only ui.resourceUri default with opt-in .with_host_layer(HostType::ChatGpt) for ChatGPT openai/* keys**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-09T19:09:57Z
- **Completed:** 2026-03-09T19:21:08Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Refactored build_meta_map and emit_resource_uri_keys to emit only standard nested ui.resourceUri key
- Added with_host_layer(HostType) to ServerCoreBuilder with build-time enrichment pipeline
- Refactored build_uri_to_tool_meta to index by standard ui.resourceUri instead of openai/outputTemplate
- All 800 tests passing with zero clippy warnings

## Task Commits

Each task was committed atomically (TDD: test then feat):

1. **Task 1: Refactor metadata emission to standard-only default**
   - `8ca158b` (test) - Failing tests for standard-only metadata
   - `b2193ac` (feat) - Implement standard-only emission
2. **Task 2: Add host layer system to ServerCoreBuilder and enrichment pipeline**
   - `95091c4` (test) - Failing tests for host layer system and URI index
   - `25df1d3` (feat) - Implement host layers, enrichment, and refactored URI index

## Files Created/Modified
- `src/types/ui.rs` - emit_resource_uri_keys now standard-only; build_meta_map returns 1 key
- `src/types/mcp_apps.rs` - WidgetMeta::to_meta_map emits standard-only ui.resourceUri
- `src/server/builder.rs` - Added host_layers field and with_host_layer() method; build-time enrichment
- `src/server/core.rs` - enrich_meta_for_host(); refactored build_uri_to_tool_meta to use standard key
- `src/server/typed_tool.rs` - Updated tests to expect standard-only metadata
- `src/types/protocol.rs` - Updated tests to expect standard-only metadata
- `src/server/mcp_apps/adapter.rs` - ChatGptAdapter always emits openai/outputTemplate from URI

## Decisions Made
- Standard-only metadata emission by default: `build_meta_map` returns only `{ui: {resourceUri: ...}}`
- Host layer enrichment happens at build time (in `ServerCoreBuilder::build()`), not at request time
- `build_uri_to_tool_meta` indexes by `meta.get("ui").get("resourceUri")` instead of `meta.get("openai/outputTemplate")`
- `ChatGptAdapter` always injects `openai/outputTemplate` from the resource URI (adapter-level responsibility)
- Resource propagation uses prefix-based filtering (`RESOURCE_PROPAGATION_PREFIXES`) instead of `CHATGPT_DESCRIPTOR_KEYS`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ChatGptAdapter missing openai/outputTemplate after standard-only refactor**
- **Found during:** Task 2 (full test suite run)
- **Issue:** ChatGptAdapter::transform() relied on WidgetMeta::to_meta_map() for openai/outputTemplate, which no longer emits it
- **Fix:** ChatGptAdapter now always inserts openai/outputTemplate from the resource URI passed to transform()
- **Files modified:** src/server/mcp_apps/adapter.rs
- **Verification:** All adapter tests pass
- **Committed in:** 25df1d3

**2. [Rule 1 - Bug] 10 existing tests expected triple-key format**
- **Found during:** Task 2 (full test suite run)
- **Issue:** Tests in protocol.rs, typed_tool.rs, and adapter.rs expected openai/outputTemplate and ui/resourceUri flat keys
- **Fix:** Updated all test assertions to expect standard-only metadata
- **Files modified:** src/types/protocol.rs, src/server/typed_tool.rs, src/server/mcp_apps/adapter.rs
- **Verification:** All 800 tests pass
- **Committed in:** 25df1d3

---

**Total deviations:** 2 auto-fixed (2 bugs from breaking change propagation)
**Impact on plan:** Both fixes necessary for correctness after the intentional breaking change. No scope creep.

## Issues Encountered
- Clippy flagged `for (_name, info) in &mut self.tool_infos` (should use `.values_mut()`) and single-arm match (should use `if`) -- fixed inline before commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Standard-only metadata is the default; Claude Desktop servers work out of the box
- ChatGPT users need to add `.with_host_layer(HostType::ChatGpt)` to their builder (breaking change)
- Ready for Plan 02: mcp-preview standard mode updates, widget-runtime bridge normalization, example verification

---
*Phase: 45-extend-mcp-apps-support-to-claude-desktop*
*Completed: 2026-03-09*
