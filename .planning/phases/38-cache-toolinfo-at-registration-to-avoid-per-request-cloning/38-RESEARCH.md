# Phase 38: Cache ToolInfo at Registration to Avoid Per-Request Cloning - Research

**Researched:** 2026-03-06
**Domain:** Rust server internals -- caching metadata at registration time
**Confidence:** HIGH

## Summary

This phase eliminates per-request `metadata()` calls and their associated cloning (especially `input_schema: Value` deep-clones) by caching `ToolInfo` and `PromptInfo` at registration time in the builder. The change is purely internal -- no trait signatures change, no public API breaks.

The codebase has 4 call sites in `ServerCore` that call `handler.metadata()` per-request: `handle_list_tools` (line 245), `handle_call_tool` widget enrichment (line 357), `handle_list_prompts` (line 365), and task routing `tool_requires_task` (line 731-734). The WASM server (`WasmMcpServer`) calls `tool.info()` and `prompt.info()` per-request at lines 142 and 263. All of these become simple cache lookups.

**Primary recommendation:** Add `HashMap<String, ToolInfo>` and `HashMap<String, PromptInfo>` fields to both `ServerCoreBuilder` and `ServerCore` (and `WasmMcpServerBuilder`/`WasmMcpServer`). Populate at `add_tool()`/`add_prompt()` time. Replace all `handler.metadata()` calls in hot paths with cache lookups.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Cache at the **builder level** (ServerCoreBuilder and WasmMcpServerBuilder), NOT inside tool structs
- Builder calls `handler.metadata()` once at `add_tool()` time, stores result in a parallel `HashMap<String, ToolInfo>` (and `HashMap<String, PromptInfo>` for prompts)
- Tool structs keep their existing fields unchanged -- no struct-level refactor
- ServerCore receives both `tools` (handlers) and `tool_infos` (cached metadata)
- Immutable after registration -- no runtime mutation of cached metadata
- No cache invalidation logic needed
- Apply to ALL tool types: TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool, SimpleWasmTool
- Apply to BOTH server types: ServerCore (standard) and WasmMcpServer
- Also cache PromptInfo with the same pattern
- NO breaking trait changes -- ToolHandler::metadata() signature stays `fn metadata(&self) -> Option<ToolInfo>`
- Document the contract change: metadata() is now called once at registration, not per-request
- Trust the cache completely -- no fallback to handler.metadata() in core.rs hot paths
- Simplify tool struct metadata() implementations to remove lazy _meta rebuild from ui_resource_uri (since it's only called once now, the lazy pattern adds no value)

### Claude's Discretion
- Exact field name for the cache in ServerCore/WasmMcpServer structs
- Whether to use `IndexMap` vs `HashMap` for the cache (consistency with existing tool storage)
- Test strategy and naming conventions

### Deferred Ideas (OUT OF SCOPE)
- Cache the full serialized ListToolsResult JSON response for zero-cost repeated list calls
- Support dynamic tool registration with cache invalidation
- Change ToolHandler::metadata() to return `&ToolInfo` or `Arc<ToolInfo>` -- cleaner API but breaking change
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::collections::HashMap | stdlib | Cache storage type | Already used for `tools` and `prompts` fields in both ServerCore and WasmMcpServer |

No new dependencies needed. This is purely an internal refactor using existing types.

## Architecture Patterns

### Current Architecture (Before)

```
ServerCoreBuilder
  tools: HashMap<String, Arc<dyn ToolHandler>>
  prompts: HashMap<String, Arc<dyn PromptHandler>>

ServerCore
  tools: HashMap<String, Arc<dyn ToolHandler>>
  prompts: HashMap<String, Arc<dyn PromptHandler>>

handle_list_tools():  for (name, handler) in &self.tools { handler.metadata() }  // clones every field
handle_call_tool():   handler.metadata()  // clones for widget enrichment
handle_list_prompts(): for (name, handler) in &self.prompts { handler.metadata() }
task routing:         handler.metadata()  // clones for execution config
```

### Target Architecture (After)

```
ServerCoreBuilder
  tools: HashMap<String, Arc<dyn ToolHandler>>
  tool_infos: HashMap<String, ToolInfo>        // NEW: populated in add_tool()
  prompts: HashMap<String, Arc<dyn PromptHandler>>
  prompt_infos: HashMap<String, PromptInfo>    // NEW: populated in add_prompt()

ServerCore
  tools: HashMap<String, Arc<dyn ToolHandler>>
  tool_infos: HashMap<String, ToolInfo>        // NEW: received from builder
  prompts: HashMap<String, Arc<dyn PromptHandler>>
  prompt_infos: HashMap<String, PromptInfo>    // NEW: received from builder

handle_list_tools():   self.tool_infos.values().cloned().collect()  // still clones, but from cache
handle_call_tool():    self.tool_infos.get(&req.name).cloned()      // single lookup
handle_list_prompts(): self.prompt_infos.values().cloned().collect()
task routing:          self.tool_infos.get(&req.name).and_then(|m| m.execution.clone())
```

### Pattern: Cache at Registration

**What:** Call `metadata()` / `info()` once during `tool()` / `prompt()` builder methods, store the result alongside the handler.

**When to use:** When metadata is immutable after registration and called on every request.

**ServerCoreBuilder.tool() change:**
```rust
pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
    let name = name.into();
    let handler = Arc::new(handler) as Arc<dyn ToolHandler>;

    // Cache metadata at registration time
    let info = handler.metadata().unwrap_or_else(|| {
        ToolInfo::new(name.clone(), None, serde_json::json!({}))
    });
    // Ensure name matches registered name
    let mut info = info;
    info.name.clone_from(&name);

    self.tool_infos.insert(name.clone(), info);
    self.tools.insert(name, handler);

    // ... capability update unchanged ...
    self
}
```

**ServerCoreBuilder.tool_arc() change:** Same pattern -- call `handler.metadata()` before inserting.

**ServerCoreBuilder.prompt() change:** Same pattern with `handler.metadata()` -> `PromptInfo`.

**ServerCoreBuilder.build() change:** Pass `self.tool_infos` and `self.prompt_infos` to `ServerCore::new()`.

**ServerCore::new() change:** Accept and store `tool_infos: HashMap<String, ToolInfo>` and `prompt_infos: HashMap<String, PromptInfo>`.

### Pattern: WasmMcpServer Cache

**What:** Same cache pattern but uses `tool.info()` (non-optional return) for WasmTool and `prompt.info()` for WasmPrompt.

**WasmMcpServerBuilder.tool() change:**
```rust
pub fn tool<T: WasmTool + 'static>(mut self, name: impl Into<String>, tool: T) -> Self {
    let name = name.into();
    let info = tool.info();
    self.tool_infos.insert(name.clone(), info);
    self.tools.insert(name, Box::new(tool));
    self.capabilities.tools = Some(Default::default());
    self
}
```

**WasmMcpServer.handle_list_tools() change:**
```rust
fn handle_list_tools(&self, _params: ListToolsParams) -> Result<Value> {
    let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();
    let result = ListToolsResult { tools, next_cursor: None };
    serde_json::to_value(result).map_err(|e| Error::internal(&e.to_string()))
}
```

### Pattern: prompt_workflow() Already Calls metadata()

The `prompt_workflow()` method in `ServerCoreBuilder` (line 637) already calls `handler.metadata()` to build a tool registry for workflows. After this phase, it should use `self.tool_infos` instead:

```rust
// BEFORE (line 636-647)
for (name, handler) in &self.tools {
    if let Some(metadata) = handler.metadata() { ... }
}

// AFTER
for (name, info) in &self.tool_infos {
    tool_registry.insert(
        Arc::from(name.as_str()),
        workflow::conversion::ToolInfo {
            name: info.name.clone(),
            description: info.description.clone().unwrap_or_default(),
            input_schema: info.input_schema.clone(),
        },
    );
}
```

### Anti-Patterns to Avoid
- **Fallback to handler.metadata():** Do NOT add `if cache miss, call handler.metadata()` -- the cache is the source of truth, populated at registration. A cache miss means the tool was not registered.
- **Arc<ToolInfo> in cache:** Tempting for zero-cost sharing, but ToolInfo needs to be mutated (name override) during registration. Clone from cache is fine since it only happens once per list request, not once per tool per request.
- **Skipping the name override in cache:** The current code does `info.name.clone_from(name)` to ensure the registered name matches. This MUST happen at cache insertion time, not at list time.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cache invalidation | Runtime cache invalidation | Immutable-after-registration cache | User decided: no mutation, no invalidation |
| Lazy initialization | OnceCell/Lazy for metadata | Eager cache in builder | metadata() is cheap to call once; lazy adds complexity |

## Common Pitfalls

### Pitfall 1: Forgetting tool_arc() and prompt_arc() methods
**What goes wrong:** The `tool_arc()` and `prompt_arc()` methods also need cache population. They take `Arc<dyn ToolHandler>` directly.
**Why it happens:** Easy to update `tool()` but forget the `_arc` variants.
**How to avoid:** Search for ALL methods that insert into `self.tools` or `self.prompts` and ensure each populates the cache.
**Warning signs:** Tests pass for `tool()` but `tool_arc()` returns empty metadata.

### Pitfall 2: ServerCore::new() parameter count
**What goes wrong:** `ServerCore::new()` already has `#[allow(clippy::too_many_arguments)]`. Adding 2 more params makes it worse.
**Why it happens:** Flat constructor with many parameters.
**How to avoid:** Accept this -- the builder pattern exists to hide this complexity from users. The constructor is only called from `build()`.
**Warning signs:** None -- this is an acceptable tradeoff.

### Pitfall 3: WasmMcpServer handle_call_tool does NOT use metadata
**What goes wrong:** Attempting to add widget enrichment cache lookup to WASM `handle_call_tool`.
**Why it happens:** Assuming WASM mirrors ServerCore exactly.
**How to avoid:** Note that `WasmMcpServer::handle_call_tool()` (line 151-196) does NOT call `tool.info()` -- it only calls `tool.execute()`. Only `handle_list_tools` and `handle_list_prompts` need cache in WASM.
**Warning signs:** Compile errors on WASM target.

### Pitfall 4: prompt_workflow() tool registry must use cache
**What goes wrong:** `prompt_workflow()` at line 636 iterates `self.tools` and calls `handler.metadata()`. After caching, it should use `self.tool_infos` instead.
**Why it happens:** Not all call sites of `metadata()` are in core.rs.
**How to avoid:** Grep for ALL calls to `metadata()` and `info()` across the server module.
**Warning signs:** Inconsistency where some paths use cache and others still call metadata().

### Pitfall 5: WasmTypedTool extension methods on WasmMcpServerBuilder
**What goes wrong:** `WasmMcpServerBuilder` has extension methods in `wasm_typed_tool.rs` (line 219+) that add tools. These also need to populate the cache.
**Why it happens:** Extension methods in a separate file are easy to miss.
**How to avoid:** Check `src/server/wasm_typed_tool.rs` line 219+ for `impl WasmMcpServerBuilder` methods.
**Warning signs:** WasmTypedTool tests fail to list tools with metadata.

## Code Examples

### 4 Call Sites in core.rs to Replace

**1. handle_list_tools (line 245-259):**
```rust
// BEFORE
let tools = self.tools.iter().map(|(name, handler)| {
    if let Some(mut info) = handler.metadata() {
        info.name.clone_from(name);
        info
    } else {
        ToolInfo::new(name.clone(), None, serde_json::json!({}))
    }
}).collect();

// AFTER
let tools = self.tool_infos.values().cloned().collect();
```

**2. handle_call_tool widget enrichment (line 356-359):**
```rust
// BEFORE
if let Some(info) = handler.metadata() {
    call_result = call_result.with_widget_enrichment(info, value);
}

// AFTER
if let Some(info) = self.tool_infos.get(&req.name) {
    call_result = call_result.with_widget_enrichment(info.clone(), value);
}
```

**3. handle_list_prompts (line 365-405):**
```rust
// BEFORE
let prompts: Vec<PromptInfo> = self.prompts.iter().map(|(name, handler)| {
    if let Some(mut info) = handler.metadata() { ... } else { ... }
}).collect();

// AFTER
let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();
```

**4. Task routing (line 731-734):**
```rust
// BEFORE
let tool_execution = self.tools.get(&req.name)
    .and_then(|h| h.metadata())
    .and_then(|m| m.execution);

// AFTER
let tool_execution = self.tool_infos.get(&req.name)
    .and_then(|m| m.execution.clone());
```

### 2 Call Sites in wasm_server.rs to Replace

**5. handle_list_tools (line 141-142):**
```rust
// BEFORE
let tools: Vec<ToolInfo> = self.tools.values().map(|tool| tool.info()).collect();

// AFTER
let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();
```

**6. handle_list_prompts (line 263-264):**
```rust
// BEFORE
let prompts: Vec<PromptInfo> = self.prompts.values().map(|prompt| prompt.info()).collect();

// AFTER
let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();
```

### Builder Registration Change in builder.rs

**7. prompt_workflow() tool registry (line 636-647):**
```rust
// BEFORE
for (name, handler) in &self.tools {
    if let Some(metadata) = handler.metadata() {
        tool_registry.insert(Arc::from(name.as_str()), ...);
    }
}

// AFTER
for (name, info) in &self.tool_infos {
    tool_registry.insert(Arc::from(name.as_str()), workflow::conversion::ToolInfo {
        name: info.name.clone(),
        description: info.description.clone().unwrap_or_default(),
        input_schema: info.input_schema.clone(),
    });
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-request metadata() calls | Cached at registration | This phase | Eliminates O(N) clone per list request, especially expensive for input_schema (serde_json::Value deep clone) |

## Open Questions

1. **HashMap vs IndexMap for caches**
   - What we know: Existing `tools` and `prompts` fields use `HashMap`. The project uses `indexmap` elsewhere (in Cargo.toml dependencies).
   - What's unclear: Whether tool ordering in `ListToolsResult` matters to any client.
   - Recommendation: Use `HashMap` for consistency with existing tool/prompt storage. If ordering becomes important, it can be changed later without API impact since the cache is internal.

2. **Whether to simplify metadata() implementations in typed tools**
   - What we know: CONTEXT.md says to "simplify tool struct metadata() implementations to remove lazy _meta rebuild from ui_resource_uri." Currently `TypedTool::metadata()` (line 229-243) builds `_meta` from `ui_resource_uri` on every call.
   - What's unclear: Whether simplification means removing the `_meta` build entirely or just acknowledging it's called once.
   - Recommendation: Keep the current metadata() implementation as-is -- it correctly builds the _meta map from ui_resource_uri. Since it's now called only once at registration, the "lazy" overhead is gone automatically. No code change needed in the tool structs themselves.

## Files Requiring Changes

| File | What Changes | Scope |
|------|-------------|-------|
| `src/server/builder.rs` | Add `tool_infos` + `prompt_infos` fields; populate in `tool()`, `tool_arc()`, `prompt()`, `prompt_arc()`; pass to `ServerCore::new()`; update `prompt_workflow()` | Medium |
| `src/server/core.rs` | Add `tool_infos` + `prompt_infos` fields to `ServerCore`; update `new()` signature; replace 4 metadata() call sites | Medium |
| `src/server/wasm_server.rs` | Add `tool_infos` + `prompt_infos` fields to both builder and server; populate in builder; replace 2 call sites | Small |
| `src/server/wasm_typed_tool.rs` | Update `impl WasmMcpServerBuilder` extension methods to populate `tool_infos` cache | Small |

## Sources

### Primary (HIGH confidence)
- Direct code reading of `src/server/builder.rs` (1043 lines), `src/server/core.rs` (1149 lines), `src/server/wasm_server.rs`, `src/server/wasm_typed_tool.rs`
- `ToolInfo` struct at `src/types/protocol.rs:239`, `PromptInfo` at `src/types/protocol.rs:612`
- `ToolHandler` trait at `src/server/mod.rs:184`, `PromptHandler` at `src/server/mod.rs:198`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, just HashMap from stdlib
- Architecture: HIGH - straightforward cache pattern with clear before/after
- Pitfalls: HIGH - identified all call sites by direct code reading

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (stable internal refactor, no external dependencies)
