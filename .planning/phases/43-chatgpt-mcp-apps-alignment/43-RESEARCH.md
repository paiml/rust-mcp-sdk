# Phase 43: ChatGPT MCP Apps Alignment - Research

**Researched:** 2026-03-08
**Domain:** MCP protocol _meta propagation across tools/call, resources/list, resources/read
**Confidence:** HIGH

## Summary

This phase fixes 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets. The core problem is that ChatGPT expects identical `_meta` (4 descriptor keys) on `tools/list`, `resources/list`, and `resources/read` responses, with a filtered subset (2 invocation keys) on `tools/call`. Our SDK currently: (1) has no `_meta` field on `ResourceInfo`, (2) puts wrong keys on `resources/read` _meta, (3) passes all _meta keys unfiltered to `tools/call`, and (4) lacks a `title` field on `ToolInfo` (deferred by user decision).

The implementation is entirely within the SDK -- no new dependencies needed. All changes touch existing structs and functions in `src/types/protocol.rs`, `src/types/ui.rs`, `src/server/simple_resources.rs`, and `src/server/core.rs`. The existing `deep_merge()` function from Phase 39 and `ChatGptToolMeta`/`WidgetMeta` structs from Phase 34/40 provide the building blocks.

**Primary recommendation:** Tool `_meta` is the single source of truth. Build a URI-to-tool-meta index at registration time. Use it to auto-populate `ResourceInfo._meta` during `resources/list`, merge descriptor keys into `resources/read` Content _meta, and filter `tools/call` _meta to only `openai/toolInvocation/*` keys.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **ResourceInfo _meta propagation (Gap 1):** Add `_meta: Option<Map>` field to `ResourceInfo` struct. Auto-populate from the linked tool's `_meta`. Propagate all `openai/*` prefixed keys. Auto-set `mimeType` on ResourceInfo from the registered resource's MIME type. When multiple tools share the same widget resource URI, first tool registered wins for _meta resolution.
- **tools/call _meta filtering (Gap 3):** Modify `with_widget_enrichment()` to filter _meta before applying -- only `openai/toolInvocation/*` keys pass through. Strip everything else. Always filter regardless of host type. Filter happens inside `with_widget_enrichment()`.
- **resources/read _meta alignment (Gap 2):** Merge both key sets: ChatGPT descriptor keys (outputTemplate, invoking, invoked, widgetAccessible) PLUS existing display keys (widgetCSP, prefersBorder, etc.). Descriptor keys come from the linked tool's `_meta`. Merge happens at the handler level (core.rs or resource handler), not inside the ChatGptAdapter. Use `deep_merge` to combine.
- **title field on ToolInfo (Gap 4):** Skip for now -- test without top-level title first.
- **DRY principle:** Tool `_meta` is the single source of truth, auto-propagated everywhere, filtered per context.

### Claude's Discretion
- Internal implementation of the openai/* key filter (regex, prefix match, or hardcoded list)
- How to look up the linked tool's _meta from a resource URI during resources/list and resources/read
- Whether to cache the propagated _meta or compute it on each request
- Test strategy for verifying _meta content across all 3 protocol methods

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

## Architecture Patterns

### Current Data Flow (Before This Phase)

```
Developer registers:
  tool (with _meta containing openai/* keys)  -----> tool_infos HashMap
  ui_resource + contents                       -----> ResourceCollection.ui_resources

tools/list:   tool_infos.values()                          -> ToolInfo._meta (correct, all keys)
resources/list: ResourceCollection.list()                  -> ResourceInfo (NO _meta field)
resources/read: ResourceCollection.read() -> Content::Resource { meta: contents.meta }  (display keys only)
tools/call:   with_widget_enrichment(info, value)          -> clones ALL _meta (too many keys)
```

### Target Data Flow (After This Phase)

```
Registration:
  tool_infos HashMap  --------> uri_to_tool_meta: HashMap<String, Map>  (index built at registration)
  ResourceCollection.ui_resources

tools/list:   tool_infos.values()                          -> ToolInfo._meta (unchanged, correct)
resources/list: ResourceCollection.list()                  -> ResourceInfo._meta = openai/* keys from uri_to_tool_meta
resources/read: Content::Resource { meta: deep_merge(display_keys, descriptor_keys_from_uri_to_tool_meta) }
tools/call:   with_widget_enrichment()                     -> FILTERED: only openai/toolInvocation/* keys
```

### Pattern 1: URI-to-Tool-Meta Index

**What:** Build a `HashMap<String, serde_json::Map>` mapping resource URIs to their linked tool's `_meta` at registration time. Cache this alongside `tool_infos` in `ServerCore`.

**When to use:** During `resources/list` and `resources/read` to look up descriptor keys without traversing all tools.

**Implementation approach:**

```rust
// Built from tool_infos after registration
fn build_uri_to_tool_meta(
    tool_infos: &HashMap<String, ToolInfo>,
) -> HashMap<String, serde_json::Map<String, Value>> {
    let mut map = HashMap::new();
    for info in tool_infos.values() {
        if let Some(meta) = info.widget_meta() {
            if let Some(Value::String(uri)) = meta.get("openai/outputTemplate") {
                // First tool registered wins (user decision)
                map.entry(uri.clone()).or_insert_with(|| {
                    // Extract only openai/* keys for propagation
                    meta.iter()
                        .filter(|(k, _)| k.starts_with("openai/"))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                });
            }
        }
    }
    map
}
```

**Key detail:** The index extracts only `openai/*` prefixed keys from the tool's `_meta`, matching the user's decision to propagate `openai/*` keys (future-proof) without leaking internal `ui.*` keys.

### Pattern 2: _meta Filtering in with_widget_enrichment()

**What:** Filter the cloned _meta map to only retain `openai/toolInvocation/*` keys before setting it on `CallToolResult`.

**Current code** (`src/types/protocol.rs:594-601`):
```rust
pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
    if let Some(meta) = info.widget_meta() {
        self.with_structured_content(structured_value)
            .with_meta(meta.clone())  // <-- clones ALL keys
    } else {
        self
    }
}
```

**Target implementation:**
```rust
pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
    if let Some(meta) = info.widget_meta() {
        let filtered: serde_json::Map<String, Value> = meta
            .iter()
            .filter(|(k, _)| k.starts_with("openai/toolInvocation/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.with_structured_content(structured_value)
            .with_meta(filtered)
    } else {
        self
    }
}
```

**Filter approach:** Simple `starts_with("openai/toolInvocation/")` prefix match. This is the most maintainable option -- no regex overhead, and naturally includes any future `toolInvocation` sub-keys ChatGPT might add. A hardcoded list would be more restrictive but fragile to spec changes.

**Recommendation:** Use prefix match (`starts_with`). It matches exactly the 2 current keys (`openai/toolInvocation/invoking`, `openai/toolInvocation/invoked`) and is forward-compatible.

### Pattern 3: ResourceInfo _meta Field Addition

**What:** Add `_meta` field to `ResourceInfo` struct.

**Current struct** (`src/types/protocol.rs:898-909`):
```rust
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}
```

**Addition:**
```rust
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    /// Optional metadata (e.g., widget descriptor keys for ChatGPT MCP Apps)
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}
```

**Note:** The field is named `meta` with `#[serde(rename = "_meta")]` following the established pattern from Phase 41 (leading underscores not idiomatic Rust).

### Pattern 4: resources/read _meta Merging

**What:** When `resources/read` returns a UI resource, merge the tool's descriptor keys into the existing display keys.

**Current code** (`src/server/simple_resources.rs:309-317`):
```rust
if let Some((_resource, contents)) = self.ui_resources.get(uri) {
    return Ok(ReadResourceResult {
        contents: vec![Content::Resource {
            uri: contents.uri.clone(),
            text: contents.text.clone(),
            mime_type: Some(contents.mime_type.clone()),
            meta: contents.meta.clone(),  // <-- display keys only
        }],
    });
}
```

**The merge needs to happen:** The question is where. Two options:

1. **In `ResourceCollection::read()`** -- requires `ResourceCollection` to have access to the URI-to-tool-meta index. Would mean passing it in at construction or as a parameter.

2. **In `ServerCore::handle_read_resource()`** -- ServerCore already has `tool_infos` and can build/cache the URI-to-tool-meta index. After getting the `ReadResourceResult` from the handler, post-process it to merge descriptor keys.

**Recommendation:** Option 2 (ServerCore level). This keeps `ResourceCollection` unaware of tools, preserving separation of concerns. ServerCore already does similar post-processing for `tools/call` (the `with_widget_enrichment` call at line 355).

### Pattern 5: Populating ResourceInfo._meta in resources/list

**Where it happens:** `ResourceCollection::list()` builds `ResourceInfo` from `UIResource` data (line 275-281 of `simple_resources.rs`). But `ResourceCollection` has no access to tool metadata.

**Options:**
1. **Post-process in `ServerCore::handle_list_resources()`** -- after getting the result from the handler, iterate over resources and populate `_meta` from the URI-to-tool-meta index.
2. **Store propagated _meta in `ResourceCollection` at registration time** -- would require passing tool_infos to the resource collection builder.

**Recommendation:** Option 1 (post-process in ServerCore). Same pattern as resources/read. The resource handler returns raw data, ServerCore enriches it. This is cleaner because registration order between tools and resources shouldn't matter.

### Anti-Patterns to Avoid
- **Don't put the merge in ChatGptAdapter:** User explicitly decided this. Adapter stays focused on HTML transformation.
- **Don't use HostType for filtering:** User decided to always filter regardless of host type.
- **Don't cache propagated _meta on ResourceInfo at registration:** Registration order between tools and resources is not guaranteed. Post-process at request time using the cached index.

## Integration Points

### Files to Modify

| File | Change | Lines |
|------|--------|-------|
| `src/types/protocol.rs` | Add `meta` field to `ResourceInfo` | ~898-909 |
| `src/types/protocol.rs` | Filter _meta in `with_widget_enrichment()` | ~594-601 |
| `src/server/core.rs` | Add `uri_to_tool_meta` field to `ServerCore` | ~109-140 |
| `src/server/core.rs` | Build index in `ServerCore::new()` | ~183-210 |
| `src/server/core.rs` | Post-process `handle_list_resources()` result | ~401-424 |
| `src/server/core.rs` | Post-process `handle_read_resource()` result | ~427-446 |
| `src/server/simple_resources.rs` | Update `ResourceInfo` construction to include `meta: None` | ~276, ~286 |
| `src/server/wasm_server_tests.rs` | Update `ResourceInfo` literals to include `meta: None` | ~230, ~240 |
| `src/server/resource_watcher.rs` | Update `ResourceInfo` literals | ~509 |
| `src/server/core_tests.rs` | Update `ResourceInfo` literals | ~112 |
| `src/server/workflow/conversion.rs` | Different `ResourceInfo` struct (not protocol), check if affected |

### Existing Reusable Code

| Asset | Location | Use |
|-------|----------|-----|
| `deep_merge()` | `src/types/ui.rs:286` | Merge descriptor keys into resources/read _meta |
| `ToolInfo::widget_meta()` | `src/types/protocol.rs:407` | Detect widget tools, get _meta reference |
| `ChatGptToolMeta` | `src/types/mcp_apps.rs:453` | Struct documenting all openai/* keys |
| `WidgetMeta::to_meta_map()` | `src/types/mcp_apps.rs:341` | Produces display keys for resources/read |
| `build_meta_map()` | `src/types/ui.rs:365` | Produces the 3 resource URI keys |

## Common Pitfalls

### Pitfall 1: Field Addition Breaks Struct Literals
**What goes wrong:** Adding `meta` to `ResourceInfo` breaks every `ResourceInfo { ... }` literal that doesn't include the new field.
**Why it happens:** `ResourceInfo` is NOT `#[non_exhaustive]`, so struct literal syntax is used throughout tests and code.
**How to avoid:** Grep for all `ResourceInfo {` usages and add `meta: None` to each. Locations identified: `simple_resources.rs` (lines 276, 286), `wasm_server_tests.rs` (lines 230, 240), `resource_watcher.rs` (line 509), `core_tests.rs` (line 112), and any examples.
**Warning signs:** Compilation errors mentioning missing field.

### Pitfall 2: Workflow ResourceInfo Confusion
**What goes wrong:** There are TWO `ResourceInfo` types -- `crate::types::protocol::ResourceInfo` and `crate::server::workflow::conversion::ResourceInfo`. They are different structs.
**How to avoid:** Only modify `protocol::ResourceInfo`. The workflow one is unrelated (has different fields: uri, name as Option, mime_type).

### Pitfall 3: tools/call _meta Becomes Empty for Non-Widget Tools
**What goes wrong:** Filtering to only `openai/toolInvocation/*` on `with_widget_enrichment` might produce an empty map if the tool has widget_meta but no invocation keys.
**How to avoid:** The filtering is correct behavior -- empty map means no invocation keys, which is fine. But ensure `skip_serializing_if = "Option::is_none"` or check for empty maps before setting.

### Pitfall 4: Multiple Tools Sharing Same Resource URI
**What goes wrong:** Two tools (e.g., `search_images` and `search_advanced`) both point to `ui://widget/explorer.html`. Which tool's _meta wins?
**How to avoid:** User decided: first tool registered wins. Use `entry().or_insert()` pattern when building the URI-to-tool-meta index. Since `tool_infos` is a HashMap, iteration order is not guaranteed -- this is acceptable since all tools sharing a URI should have the same descriptor keys.

### Pitfall 5: resources/read Post-Processing Scope
**What goes wrong:** Only UI resources (`ui://` scheme) need descriptor key merging. Regular file resources should not get widget _meta injected.
**How to avoid:** In `handle_read_resource` post-processing, only merge when the content URI matches an entry in `uri_to_tool_meta`. The URI check naturally scopes it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON object merging | Custom merge loop | `deep_merge()` from `src/types/ui.rs` | Already handles nested objects, leaf collision, arrays |
| Widget detection | Custom _meta parsing | `ToolInfo::widget_meta()` | Checks all 3 key formats (nested, flat, openai/) |
| openai/* key emission | Manual map building | `ChatGptToolMeta::to_meta_map()` | Serde-driven, produces correct keys |

## Code Examples

### Building the URI-to-Tool-Meta Index

```rust
// In ServerCore or a helper function
fn build_uri_to_tool_meta(
    tool_infos: &HashMap<String, ToolInfo>,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for info in tool_infos.values() {
        if let Some(meta) = info.widget_meta() {
            if let Some(serde_json::Value::String(uri)) = meta.get("openai/outputTemplate") {
                map.entry(uri.clone()).or_insert_with(|| {
                    meta.iter()
                        .filter(|(k, _)| k.starts_with("openai/"))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                });
            }
        }
    }
    map
}
```

### Post-Processing resources/list

```rust
// In ServerCore::handle_list_resources, after getting result from handler
async fn handle_list_resources(&self, req: &ListResourcesParams, auth_context: Option<AuthContext>) -> Result<ListResourcesResult> {
    let mut result = match &self.resources {
        Some(handler) => { /* existing code */ },
        None => { /* existing code */ },
    };

    // Enrich ResourceInfo items with tool _meta for widget resources
    for resource in &mut result.resources {
        if let Some(tool_meta) = self.uri_to_tool_meta.get(&resource.uri) {
            resource.meta = Some(tool_meta.clone());
        }
    }

    Ok(result)
}
```

### Post-Processing resources/read

```rust
// In ServerCore::handle_read_resource, after getting result from handler
async fn handle_read_resource(&self, req: &ReadResourceParams, auth_context: Option<AuthContext>) -> Result<ReadResourceResult> {
    let mut result = /* existing handler call */;

    // Merge tool descriptor keys into content _meta for widget resources
    for content in &mut result.contents {
        if let Content::Resource { uri, meta, .. } = content {
            if let Some(tool_meta) = self.uri_to_tool_meta.get(uri.as_str()) {
                let content_meta = meta.get_or_insert_with(serde_json::Map::new);
                crate::types::ui::deep_merge(content_meta, tool_meta.clone());
            }
        }
    }

    Ok(result)
}
```

### Filtered with_widget_enrichment

```rust
pub fn with_widget_enrichment(self, info: &ToolInfo, structured_value: Value) -> Self {
    if let Some(meta) = info.widget_meta() {
        let filtered: serde_json::Map<String, Value> = meta
            .iter()
            .filter(|(k, _)| k.starts_with("openai/toolInvocation/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.with_structured_content(structured_value)
            .with_meta(filtered)
    } else {
        self
    }
}
```

## Open Questions

1. **Content mutability in ReadResourceResult**
   - What we know: `ReadResourceResult.contents` is `Vec<Content>`, and `Content::Resource` has a `meta` field. We need to mutate this after the handler returns.
   - What's unclear: `Content` is an enum -- mutating the `meta` field of a `Resource` variant requires pattern matching with `if let Content::Resource { ref mut meta, .. }`.
   - Recommendation: Use `for content in &mut result.contents { if let Content::Resource { meta, .. } = content { ... } }` -- standard Rust pattern.

2. **Should uri_to_tool_meta be in ServerCore or ServerCoreBuilder?**
   - What we know: `tool_infos` is built in `ServerCoreBuilder` and passed to `ServerCore`. The index should be built from `tool_infos`.
   - Recommendation: Build it in `ServerCore::new()` from the already-populated `tool_infos`, same as other derived state. Keep it as a field on `ServerCore`.

## Sources

### Primary (HIGH confidence)
- `src/types/protocol.rs` -- ResourceInfo struct (line 898), with_widget_enrichment (line 594), ToolInfo::widget_meta (line 407)
- `src/server/core.rs` -- ServerCore struct, handle_list_resources (line 402), handle_read_resource (line 427), tool_infos usage (line 354)
- `src/server/simple_resources.rs` -- ResourceCollection::list() (line 267), read() (line 300)
- `src/types/ui.rs` -- deep_merge (line 286), build_meta_map (line 365), emit_resource_uri_keys (line 319)
- `src/types/mcp_apps.rs` -- ChatGptToolMeta (line 453), WidgetMeta (line 246)
- Reference analysis doc: `CHATGPT_WIDGET_PROTOCOL_ANALYSIS.md`

### Secondary (MEDIUM confidence)
- Phase 39 decisions (deep_merge pattern) -- from STATE.md
- Phase 41 decisions (serde rename _meta pattern) -- from STATE.md

## Metadata

**Confidence breakdown:**
- Architecture: HIGH -- all code paths traced, structs identified, patterns verified in source
- Integration points: HIGH -- exact line numbers and function signatures known
- Pitfalls: HIGH -- identified from actual struct usage patterns in codebase

**Research date:** 2026-03-08
**Valid until:** 2026-04-08 (stable internal SDK code, no external dependency changes)
