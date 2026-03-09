# Phase 39: Add Deep Merge for UI Meta Key to Prevent Collision - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Prevent `_meta` data loss when multiple systems contribute to the same `_meta` map on `ToolInfo`. Currently, `build_meta_map()` creates a fresh map with only UI keys, and `metadata()` returns it as the entire `_meta`. If anything else needs to add keys to `_meta` (execution config, custom metadata), it overwrites UI keys or vice versa. Fix: make each `with_*()` builder method merge into `_meta` rather than replacing it. Also add `with_ui()` support to `TypedToolWithOutput` since deep merge makes UI + output schema coexistence natural.

</domain>

<decisions>
## Implementation Decisions

### Merge strategy
- Deep merge: recursively merge nested JSON objects so `with_ui()` adds `{ui: {resourceUri: ...}}`, `with_execution()` adds `{execution: ...}`, and both coexist
- Only recurse into JSON objects — arrays are replaced entirely by the overlay (not concatenated)
- Standalone function: `deep_merge(base: &mut Map, overlay: Map)` — mutates base in-place, avoids allocation
- Last-in wins at the leaf level when two contributors set the same nested key

### Collision rules
- Last-in wins — simple, predictable, matches `HashMap::insert` semantics
- User controls priority by builder call order
- Log collisions at `tracing::debug` level — useful for debugging, not noisy in production

### API surface
- Fix existing internal methods (with_ui, metadata) to use deep merge instead of replacing `_meta`
- Also add public `with_meta_entry(key: &str, value: Value)` on `ToolInfo` for custom `_meta` contributions — adds one key at a time, composable
- Keep existing `with_meta()` (replace-all) alongside the new method — different use cases, both valid, no deprecation

### Scope
- Fix `ToolInfo._meta` only — this is where the collision actually happens
- `CallToolResult._meta` and `GetPromptResult._meta` are set by single sources today — no collision problem
- Add `with_ui()` to `TypedToolWithOutput` as part of this phase — deep merge makes UI + output schema metadata coexistence natural

### Claude's Discretion
- Where to place the standalone deep_merge function (types/ui.rs, types/meta.rs, or a util module)
- Test structure and naming conventions
- Whether TypedToolWithOutput.with_ui() needs its own example

</decisions>

<specifics>
## Specific Ideas

- Phase 37 established `ui_resource_uri: Option<String>` with `_meta` built via `build_meta_map()` in `metadata()`
- Phase 38 caches `_meta` at registration time — merge only needs to happen once at builder time, not per-request
- `TypedToolWithOutput::metadata()` at `src/server/typed_tool.rs:683` currently sets `_meta: None`, losing any UI metadata — this is the primary collision point to fix
- The `with_meta()` method on `CallToolResult` already replaces entirely — but that's a separate concern (result-time, not registration-time)

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ToolUIMetadata::build_meta_map()` at `src/types/ui.rs:276` — builds `{ui: {resourceUri}, openai/outputTemplate}` map
- `ToolInfo::with_ui()` at `src/types/protocol.rs:327` — static constructor that sets `_meta` from `build_meta_map()`
- `TypedTool::with_ui()` at `src/server/typed_tool.rs:205` — stores `ui_resource_uri`, builds `_meta` in `metadata()`
- `TypedSyncTool::with_ui()` at `src/server/typed_tool.rs:374` — same pattern
- `WasmTypedTool::with_ui()` at `src/server/wasm_typed_tool.rs:86` — same pattern

### Established Patterns
- `_meta` is `Option<serde_json::Map<String, Value>>` on `ToolInfo`
- Builder methods take `mut self` and return `Self`
- `metadata()` implementations construct `ToolInfo` with `_meta` set from `ui_resource_uri` if present, or `None`
- `TypedToolWithOutput::metadata()` at `src/server/typed_tool.rs:683` always sets `_meta: None` — collision point

### Integration Points
- `TypedTool::metadata()` at line 229 — needs to merge `ui_resource_uri` meta with any other `_meta` entries
- `TypedSyncTool::metadata()` at line 393 — same
- `TypedToolWithOutput::metadata()` at line 683 — currently ignores UI, needs merge + with_ui() support
- `WasmTypedTool::info()` at line 95 — same pattern for WASM
- `ServerCoreBuilder::tool()/tool_arc()` at `src/server/builder.rs` — where cached `_meta` is stored

</code_context>

<deferred>
## Deferred Ideas

- Apply deep-merge to `CallToolResult._meta` and `GetPromptResult._meta` — if middleware collision becomes a problem
- Meta key constants module (phase 35) — extract string literals to named constants
- Deprecate `with_meta()` replace-all in favor of merge-only API — evaluate after adoption

</deferred>

---

*Phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision*
*Context gathered: 2026-03-06*
