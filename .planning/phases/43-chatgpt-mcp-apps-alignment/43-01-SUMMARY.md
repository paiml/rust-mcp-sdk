---
phase: 43-chatgpt-mcp-apps-alignment
plan: 01
subsystem: api
tags: [mcp-apps, chatgpt, widget, meta, resourceinfo, protocol]

# Dependency graph
requires:
  - phase: 41-chatgpt-mcp-apps-upgraded-version
    provides: "serde rename _meta pattern, widget_meta() on ToolInfo"
  - phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision
    provides: "deep_merge utility, with_meta_entry on ToolInfo"
provides:
  - "ResourceInfo._meta field for widget descriptor key propagation"
  - "uri_to_tool_meta index on ServerCore for resource-to-tool lookup"
  - "Filtered with_widget_enrichment for tools/call (openai/toolInvocation/* only)"
affects: [43-02-PLAN, chatgpt-mcp-apps, resources-list, resources-read, tools-call]

# Tech tracking
tech-stack:
  added: []
  patterns: [uri-to-tool-meta-index, meta-key-filtering-by-prefix]

key-files:
  created: []
  modified:
    - src/types/protocol.rs
    - src/server/core.rs
    - src/server/simple_resources.rs
    - src/server/core_tests.rs
    - src/server/resource_watcher.rs
    - src/server/wasm_server_tests.rs
    - src/server/mod.rs
    - tests/server_subscriptions.rs
    - tests/typescript_interop.rs
    - tests/test_batch_requests.rs
    - examples/04_server_resources.rs
    - examples/18_resource_watcher.rs
    - examples/54_hybrid_workflow_execution.rs
    - examples/59_dynamic_resource_workflow.rs
    - examples/60_resource_only_steps.rs
    - examples/27-course-server-minimal/src/main.rs
    - examples/mcp-apps-dataviz/src/main.rs
    - examples/mcp-apps-chess/src/main.rs
    - examples/mcp-apps-map/src/main.rs
    - cargo-pmcp/src/templates/mcp_app.rs

key-decisions:
  - "Used starts_with prefix match for openai/toolInvocation/* filtering (forward-compatible)"
  - "URI-to-tool-meta index built at ServerCore construction time from tool_infos"
  - "First tool registered wins for URI collision resolution (entry().or_insert)"

patterns-established:
  - "URI-to-tool-meta index: build_uri_to_tool_meta() maps resource URIs to linked tool openai/* keys"
  - "Meta key filtering by prefix in with_widget_enrichment for context-specific _meta"

requirements-completed: []

# Metrics
duration: 9min
completed: 2026-03-08
---

# Phase 43 Plan 01: ChatGPT MCP Apps Alignment - Foundation Summary

**ResourceInfo._meta field, URI-to-tool-meta index on ServerCore, and filtered with_widget_enrichment for openai/toolInvocation/* keys only**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-08T15:58:16Z
- **Completed:** 2026-03-08T16:07:00Z
- **Tasks:** 2
- **Files modified:** 20

## Accomplishments
- Added `meta` field to `ResourceInfo` struct with `#[serde(rename = "_meta")]` for ChatGPT widget descriptor propagation
- Built `uri_to_tool_meta` index on `ServerCore` mapping resource URIs to linked tool `openai/*` meta keys
- Filtered `with_widget_enrichment()` to only pass `openai/toolInvocation/*` keys to `tools/call` responses
- Updated all 20 files containing `ResourceInfo` struct literals to include `meta: None`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add _meta field to ResourceInfo, filter with_widget_enrichment, build URI-to-tool-meta index** - `a995b61` (feat)
2. **Task 2: Update all ResourceInfo struct literals across codebase** - `9287455` (chore)

## Files Created/Modified
- `src/types/protocol.rs` - Added `meta` field to `ResourceInfo`, filtered `with_widget_enrichment()` to `openai/toolInvocation/*` prefix
- `src/server/core.rs` - Added `build_uri_to_tool_meta()` helper, `uri_to_tool_meta` field on `ServerCore`, index built in `new()`
- `src/server/simple_resources.rs` - Added `meta: None` to 3 `ResourceInfo` literals
- `src/server/core_tests.rs` - Added `meta: None` to mock resource handler
- `src/server/resource_watcher.rs` - Added `meta: None` to test resource
- `src/server/wasm_server_tests.rs` - Added `meta: None` to 2 paginated resource literals
- `src/server/mod.rs` - Added `meta: None` to doctest `ResourceInfo` literal
- `tests/server_subscriptions.rs` - Added `meta: None`
- `tests/typescript_interop.rs` - Added `meta: None`
- `tests/test_batch_requests.rs` - Added `meta: None` to 2 literals
- `examples/04_server_resources.rs` - Added `meta: None` to 3 literals
- `examples/18_resource_watcher.rs` - Added `meta: None`
- `examples/54_hybrid_workflow_execution.rs` - Added `meta: None`
- `examples/59_dynamic_resource_workflow.rs` - Added `meta: None` to 2 literals
- `examples/60_resource_only_steps.rs` - Added `meta: None` to 3 literals
- `examples/27-course-server-minimal/src/main.rs` - Added `meta: None`
- `examples/mcp-apps-dataviz/src/main.rs` - Added `meta: None`
- `examples/mcp-apps-chess/src/main.rs` - Added `meta: None`
- `examples/mcp-apps-map/src/main.rs` - Added `meta: None`
- `cargo-pmcp/src/templates/mcp_app.rs` - Added `meta: None` to template

## Decisions Made
- Used `starts_with("openai/toolInvocation/")` prefix match for filtering (forward-compatible with future ChatGPT sub-keys)
- Built URI-to-tool-meta index at `ServerCore::new()` time from `tool_infos` (cached, immutable)
- First tool registered wins for URI collision via `entry().or_insert_with()` (HashMap iteration order is non-deterministic but acceptable since tools sharing a URI should have identical descriptor keys)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated mod.rs doctest ResourceInfo literal**
- **Found during:** Task 2
- **Issue:** `src/server/mod.rs` line 2419 has a `rust,no_run` doctest with a `ResourceInfo` literal missing the new `meta` field -- would cause doctest compilation failure
- **Fix:** Added `meta: None` to the doctest literal
- **Files modified:** `src/server/mod.rs`
- **Verification:** `cargo test --doc` passes for this doctest
- **Committed in:** `9287455` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor addition to Task 2 scope -- one extra file. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ResourceInfo._meta field ready for auto-population from uri_to_tool_meta in Plan 02
- uri_to_tool_meta index ready for use in resources/list and resources/read post-processing (Plan 02)
- with_widget_enrichment filtering complete -- tools/call _meta now only contains openai/toolInvocation/* keys

---
*Phase: 43-chatgpt-mcp-apps-alignment*
*Completed: 2026-03-08*
