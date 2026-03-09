---
phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision
verified: 2026-03-07T00:30:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 39: Add deep-merge for ui meta key to prevent collision — Verification Report

**Phase Goal:** Add deep_merge function for serde_json::Map and update all metadata() implementations to merge _meta instead of replacing, preventing data loss when multiple builder methods contribute to _meta. Also add with_ui() to TypedToolWithOutput and with_meta_entry() to ToolInfo.
**Verified:** 2026-03-07T00:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                               | Status     | Evidence                                                                                                           |
|----|-----------------------------------------------------------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------------|
| 1  | deep_merge recursively merges nested JSON objects in-place                                          | VERIFIED   | `src/types/ui.rs` line 281: `pub fn deep_merge(...)` with recursive call on line 296; 7 unit tests all pass       |
| 2  | Arrays are replaced entirely by overlay, not concatenated                                           | VERIFIED   | Logic in ui.rs lines 286-307: only `Object+Object` recurses; all other types (including arrays) do insert/replace  |
| 3  | Last-in wins at leaf level with tracing::debug on collision                                         | VERIFIED   | ui.rs line 300: `tracing::debug!(key = %key, "deep_merge: overwriting existing _meta key")` before insert         |
| 4  | ToolInfo::with_meta_entry adds a single key composably via deep merge                               | VERIFIED   | `src/types/protocol.rs` line 365: `pub fn with_meta_entry(...)` calls `crate::types::ui::deep_merge`              |
| 5  | TypedTool metadata() uses deep_merge for _meta instead of replacing                                 | VERIFIED   | `src/server/typed_tool.rs` lines 229-244: builds empty map, calls `deep_merge`, returns `None` if empty            |
| 6  | TypedSyncTool metadata() uses deep_merge for _meta instead of replacing                             | VERIFIED   | `src/server/typed_tool.rs` lines 397-412: identical deep_merge pattern applied                                     |
| 7  | TypedToolWithOutput has with_ui() builder and metadata() merges UI meta with output schema          | VERIFIED   | Lines 672-741: `with_ui()` sets `ui_resource_uri`; `metadata()` calls `deep_merge`; output_schema in annotations  |
| 8  | WasmTypedTool info() uses deep_merge for _meta instead of replacing                                 | VERIFIED   | `src/server/wasm_typed_tool.rs` lines 107-122: deep_merge pattern used, compiles cleanly                          |
| 9  | TypedToolWithOutput with_ui + output_schema coexist without data loss                               | VERIFIED   | Test `test_typed_tool_with_output_with_ui_and_output_schema_coexist` passes (line 846)                             |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact                          | Expected                                                                            | Status     | Details                                                                          |
|-----------------------------------|-------------------------------------------------------------------------------------|------------|----------------------------------------------------------------------------------|
| `src/types/ui.rs`                 | deep_merge standalone function                                                      | VERIFIED   | `pub fn deep_merge` at line 281; 7 unit tests in `mod tests`                     |
| `src/types/protocol.rs`           | ToolInfo::with_meta_entry builder method                                            | VERIFIED   | `pub fn with_meta_entry` at line 365; 4 unit tests; calls `crate::types::ui::deep_merge` |
| `src/server/typed_tool.rs`        | Updated metadata() for TypedTool, TypedSyncTool, TypedToolWithOutput; with_ui on TypedToolWithOutput | VERIFIED   | deep_merge called in all three metadata() implementations; `with_ui` on TypedToolWithOutput at line 672; 3 new tests |
| `src/server/wasm_typed_tool.rs`   | Updated info() for WasmTypedTool using deep_merge                                  | VERIFIED   | `crate::types::ui::deep_merge` called at line 111 inside `fn info()`             |

### Key Link Verification

| From                            | To                  | Via                              | Status  | Details                                                                             |
|---------------------------------|---------------------|----------------------------------|---------|-------------------------------------------------------------------------------------|
| `src/types/protocol.rs`         | `src/types/ui.rs`   | with_meta_entry calls deep_merge | WIRED   | Line 369: `crate::types::ui::deep_merge(meta, overlay);`                            |
| `src/server/typed_tool.rs`      | `src/types/ui.rs`   | metadata() calls deep_merge      | WIRED   | Lines 233, 401, 726: `crate::types::ui::deep_merge(&mut meta, ui_meta);` in all three |
| `src/server/wasm_typed_tool.rs` | `src/types/ui.rs`   | info() calls deep_merge          | WIRED   | Line 111: `crate::types::ui::deep_merge(&mut meta, ui_meta);`                       |

### Requirements Coverage

| Requirement | Source Plan | Description                                                                           | Status      | Evidence                                                                           |
|-------------|-------------|----------------------------------------------------------------------------------------|-------------|------------------------------------------------------------------------------------|
| MERGE-01    | 39-01-PLAN  | deep_merge function and ToolInfo::with_meta_entry builder                              | SATISFIED   | `pub fn deep_merge` in ui.rs; `pub fn with_meta_entry` in protocol.rs; 12 tests pass |
| MERGE-02    | 39-02-PLAN  | All four tool types use deep_merge; TypedToolWithOutput gains with_ui()                | SATISFIED   | All 4 tool types call `crate::types::ui::deep_merge`; TypedToolWithOutput.with_ui() at line 672 |

**Note on REQUIREMENTS.md:** MERGE-01 and MERGE-02 are referenced in ROADMAP.md (line 272) but are not formally defined in `.planning/REQUIREMENTS.md`. The requirements file covers v1.5 and v1.6 CLI/flag requirements (FLAG-*, AUTH-*, TEST-*, CMD-*, HELP-*). The MERGE requirements appear to be SDK-internal requirements tracked only in the roadmap. This is not a gap — the requirements are accounted for in ROADMAP.md and were satisfied in the plans and summaries. No orphaned requirements were found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | —    | —       | —        | —      |

No TODOs, FIXMEs, placeholders, empty implementations, or stub handlers found in any of the four modified files.

### Human Verification Required

None. All behavioral properties are verified programmatically via unit tests and code inspection:
- Recursive merge logic is verified by 7 passing test cases (disjoint, nested, collision, arrays, empty cases, 3-level deep)
- Builder composability verified by 4 passing with_meta_entry tests
- TypedToolWithOutput UI coexistence verified by 3 dedicated tests
- Full library suite: 720 tests pass, zero clippy warnings

### Test Results Summary

| Test Filter                        | Passed | Failed |
|------------------------------------|--------|--------|
| `deep_merge` (pmcp --lib)          | 8      | 0      |
| `with_meta_entry` (pmcp --lib)     | 4      | 0      |
| `typed_tool_with_output` (pmcp --lib) | 3   | 0      |
| Full library suite (pmcp --lib)    | 720    | 0      |
| `cargo clippy -p pmcp -D warnings` | pass   | —      |

### Scope Guard Check

Verified that `CallToolResult::with_meta` and `GetPromptResult::with_meta` are not modified by this phase — the scope guard is clean.

---

_Verified: 2026-03-07T00:30:00Z_
_Verifier: Claude (gsd-verifier)_
