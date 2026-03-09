# Phase 37: Add with_ui Support to TypedSyncTool - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Add the `with_ui()` builder method to `TypedSyncTool` and `WasmTypedTool` so all typed tool variants can associate with UI resources for MCP Apps. Mirror the existing pattern from `TypedTool` (async).

</domain>

<decisions>
## Implementation Decisions

### Scope
- Add `with_ui()` to both `TypedSyncTool` AND `WasmTypedTool` for consistency
- Do NOT add annotation methods (`with_annotations`, `read_only`, etc.) to `WasmTypedTool` — separate concern
- Do NOT add `SimpleWasmTool` UI support — it's a convenience wrapper, not primary API

### with_ui implementation
- Mirror `TypedTool::with_ui()` exactly: store `ui_resource_uri: Option<String>` field, build `_meta` via `ToolUIMetadata::build_meta_map()` in metadata/info method
- `TypedSyncTool::metadata()` should emit `_meta` when `ui_resource_uri` is set (currently always returns `_meta: None`)
- `WasmTypedTool::info()` should emit `_meta` when `ui_resource_uri` is set

### WASM enrichment depth
- Metadata emission only — add `_meta` to `WasmTypedTool::info()` ToolInfo
- Do NOT add structured_content + _meta enrichment to WASM tool call results — that's a separate concern for the WASM server

### Claude's Discretion
- Whether to update existing examples to use `TypedSyncTool::with_ui()`
- Test structure and naming conventions

</decisions>

<specifics>
## Specific Ideas

- Phase 34 established that `_meta` uses nested `ui.resourceUri` format + `openai/outputTemplate` alias
- Phase 36 added `From`/`TryFrom` bridge between `UIMimeType` and `ExtendedUIMimeType` which this phase may need
- `ToolUIMetadata::build_meta_map()` is the canonical builder — both `TypedTool` and `ToolInfo::with_ui()` already use it

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `TypedTool::with_ui()` at `src/server/typed_tool.rs:205` — exact pattern to mirror
- `ToolUIMetadata::build_meta_map()` at `src/types/ui.rs:274` — canonical _meta builder
- `ToolInfo::with_ui()` at `src/types/protocol.rs:327` — alternative static constructor

### Established Patterns
- `ui_resource_uri: Option<String>` field on struct, `with_ui()` builder method sets it
- `metadata()` builds `_meta` via `build_meta_map()` when `ui_resource_uri` is `Some`
- Builder pattern: all `with_*` methods take `mut self` and return `Self`

### Integration Points
- `TypedSyncTool` struct at `src/server/typed_tool.rs:247` — needs `ui_resource_uri` field
- `TypedSyncTool::metadata()` at `src/server/typed_tool.rs:369` — needs `_meta` emission
- `WasmTypedTool` struct at `src/server/wasm_typed_tool.rs:22` — needs `ui_resource_uri` field
- `WasmTypedTool::info()` at `src/server/wasm_typed_tool.rs:95` — needs `_meta` emission

</code_context>

<deferred>
## Deferred Ideas

- Add annotation methods (`with_annotations`, `read_only`, etc.) to `WasmTypedTool` — API parity phase
- Add structured_content + _meta enrichment to WASM server tool call results — WASM MCP Apps phase
- Add `with_ui` to `SimpleWasmTool` — if demand arises

</deferred>

---

*Phase: 37-add-with-ui-support-to-typedsynctool*
*Context gathered: 2026-03-06*
