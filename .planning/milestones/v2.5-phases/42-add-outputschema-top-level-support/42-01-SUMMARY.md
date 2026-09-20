---
phase: 42-add-outputschema-top-level-support
plan: 01
subsystem: api
tags: [mcp-spec, output-schema, tool-info, annotations, macro-codegen]

requires:
  - phase: 41-chatgpt-mcp-apps-upgraded-version
    provides: stable ToolInfo struct with _meta and annotations
provides:
  - ToolInfo.output_schema top-level field (MCP spec 2025-06-18)
  - ToolInfo::with_output_schema() builder method
  - ToolAnnotations::with_output_type_name() for codegen extension
  - Macro codegen emitting top-level outputSchema
affects: [typed-tools, macro-codegen, tool-annotations, mcp-spec-compliance]

tech-stack:
  added: []
  patterns: [output-schema-on-toolinfo, output-type-name-in-annotations]

key-files:
  created: []
  modified:
    - src/types/protocol.rs
    - src/server/typed_tool.rs
    - src/server/wasm_server.rs
    - src/server/wasm_typed_tool.rs
    - pmcp-macros/src/tool.rs
    - tests/tool_annotations_test.rs

key-decisions:
  - "outputSchema is top-level on ToolInfo, pmcp:outputTypeName remains in annotations as PMCP codegen extension"
  - "ToolAnnotations no longer has output_schema field -- only output_type_name"

patterns-established:
  - "output_schema on ToolInfo: all ToolInfo struct literals must include output_schema field"
  - "with_output_schema() chains on ToolInfo, with_output_type_name() chains on ToolAnnotations"

requirements-completed: [OS-01, OS-02, OS-03, OS-04]

duration: 19min
completed: 2026-03-07
---

# Phase 42 Plan 01: Add outputSchema Top-Level Support Summary

**Migrated output_schema from ToolAnnotations to ToolInfo top-level field, aligning with MCP spec 2025-06-18 outputSchema placement**

## Performance

- **Duration:** 19 min
- **Started:** 2026-03-07T22:27:24Z
- **Completed:** 2026-03-07T22:46:10Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- output_schema field added to ToolInfo as top-level sibling to inputSchema
- ToolAnnotations cleaned: output_schema field removed, with_output_schema() replaced by with_output_type_name()
- TypedToolWithOutput::metadata() rewired to set output_schema on ToolInfo directly
- Macro codegen emits .with_output_schema() on ToolInfo and .with_output_type_name() on annotations
- All ToolInfo struct literal sites updated (typed_tool, wasm_server, wasm_typed_tool)
- Full workspace compiles and all tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate output_schema from ToolAnnotations to ToolInfo** - `2a8ec31` (feat)
2. **Task 2: Rewire TypedToolWithOutput, all struct literal sites, and macro codegen** - `460ad99` (feat)

## Files Created/Modified
- `src/types/protocol.rs` - Added output_schema field to ToolInfo, with_output_schema() builder, replaced ToolAnnotations::with_output_schema() with with_output_type_name()
- `src/server/typed_tool.rs` - Rewired TypedToolWithOutput::metadata() to top-level output_schema, updated TypedTool and TypedSyncTool metadata(), fixed test assertions
- `src/server/wasm_server.rs` - Added output_schema: None to SimpleTool ToolInfo literal
- `src/server/wasm_typed_tool.rs` - Added output_schema: None to WasmTypedTool and WasmTypedToolWithOutput ToolInfo literals
- `pmcp-macros/src/tool.rs` - Updated generate_definition_code() to emit .with_output_type_name() on annotations and .with_output_schema() on ToolInfo
- `tests/tool_annotations_test.rs` - Migrated all tests from annotations-based output_schema to top-level ToolInfo output_schema

## Decisions Made
- outputSchema is a top-level field on ToolInfo (MCP spec 2025-06-18 alignment), serialized as `outputSchema` via serde rename_all camelCase
- pmcp:outputTypeName remains in ToolAnnotations as a PMCP codegen extension (not part of MCP spec)
- ToolAnnotations no longer has an output_schema field or with_output_schema() method

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated tool_annotations_test.rs for new API**
- **Found during:** Task 2 (verification step)
- **Issue:** tests/tool_annotations_test.rs referenced ToolAnnotations::with_output_schema() and annotations.output_schema which no longer exist
- **Fix:** Rewrote all test cases to use ToolInfo::with_output_schema() and ToolAnnotations::with_output_type_name()
- **Files modified:** tests/tool_annotations_test.rs
- **Verification:** cargo test --workspace passes
- **Committed in:** 460ad99 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Test file update necessary for compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- output_schema is now correctly placed as a top-level ToolInfo field
- Ready for plan 02 (if any) to add further tests or documentation
- All existing tests pass with the new structure

---
*Phase: 42-add-outputschema-top-level-support*
*Completed: 2026-03-07*
