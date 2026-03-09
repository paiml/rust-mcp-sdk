# Phase 38: Cache ToolInfo at Registration to Avoid Per-Request Cloning - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Cache ToolInfo (and PromptInfo) at registration time in the server builders so that `handle_list_tools`, `handle_call_tool`, `handle_list_prompts`, and task routing no longer rebuild metadata from scratch on every request. With streamable HTTP, clients re-initialize frequently, making list operations a hot path.

</domain>

<decisions>
## Implementation Decisions

### Cache location
- Cache at the **builder level** (ServerCoreBuilder and WasmMcpServerBuilder), NOT inside tool structs
- Builder calls `handler.metadata()` once at `add_tool()` time, stores result in a parallel `HashMap<String, ToolInfo>` (and `HashMap<String, PromptInfo>` for prompts)
- Tool structs keep their existing fields unchanged — no struct-level refactor
- ServerCore receives both `tools` (handlers) and `tool_infos` (cached metadata)

### Mutation support
- Immutable after registration — no runtime mutation of cached metadata
- No cache invalidation logic needed
- Streamable HTTP means clients get fresh state on each init — no stale-cache concerns

### Scope
- Apply to ALL tool types: TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool, SimpleWasmTool
- Apply to BOTH server types: ServerCore (standard) and WasmMcpServer
- Also cache PromptInfo with the same pattern (handle_list_prompts has same clone-per-request issue)

### Breaking changes
- NO breaking trait changes — ToolHandler::metadata() signature stays `fn metadata(&self) -> Option<ToolInfo>`
- Document the contract change: metadata() is now called once at registration, not per-request
- Trust the cache completely — no fallback to handler.metadata() in core.rs hot paths
- Simplify tool struct metadata() implementations to remove lazy _meta rebuild from ui_resource_uri (since it's only called once now, the lazy pattern adds no value)

### Claude's Discretion
- Exact field name for the cache in ServerCore/WasmMcpServer structs
- Whether to use `IndexMap` vs `HashMap` for the cache (consistency with existing tool storage)
- Test strategy and naming conventions

</decisions>

<specifics>
## Specific Ideas

- Phase 37 established `ui_resource_uri: Option<String>` with lazy `_meta` rebuild — this phase makes that irrelevant since metadata() is called once
- 4 call sites in core.rs invoke metadata(): handle_list_tools (line 251), handle_call_tool (line 357), handle_list_prompts (line 371), task routing (line 734) — all should use cached info
- `input_schema` clone is the most expensive per-request operation (potentially large JSON tree) — caching eliminates this entirely from hot paths

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ServerCoreBuilder` at `src/server/core.rs` — registration point for tools/prompts
- `WasmMcpServerBuilder` at `src/server/wasm_server.rs` — WASM registration point
- `ToolHandler::metadata()` trait method — already returns `Option<ToolInfo>`, called by builder
- `PromptHandler::metadata()` — same pattern for prompts

### Established Patterns
- Tools stored as `HashMap<String, Arc<dyn ToolHandler>>` in ServerCore
- Builder pattern: `ServerCoreBuilder::add_tool()` takes ownership and returns Self
- `handle_list_tools` iterates tools map, calls metadata() on each, collects into Vec<ToolInfo>
- `handle_call_tool` calls metadata() for widget enrichment via `with_widget_enrichment()`

### Integration Points
- `ServerCore::new()` — receives both tools and tool_infos from builder
- `handle_list_tools` (line 245) — switch from iterating tools+metadata() to iterating tool_infos
- `handle_call_tool` (line 268) — switch from handler.metadata() to tool_infos.get()
- `handle_list_prompts` (line 365) — same cache pattern for prompts
- Task routing (line 732) — uses metadata() for execution config

</code_context>

<deferred>
## Deferred Ideas

- Cache the full serialized ListToolsResult JSON response for zero-cost repeated list calls — evaluate if ToolInfo cache alone is sufficient
- Support dynamic tool registration with cache invalidation — if runtime add/remove tools is ever needed
- Change ToolHandler::metadata() to return `&ToolInfo` or `Arc<ToolInfo>` — cleaner API but breaking change

</deferred>

---

*Phase: 38-cache-toolinfo-at-registration-to-avoid-per-request-cloning*
*Context gathered: 2026-03-06*
