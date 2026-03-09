# Phase 39: Add Deep Merge for UI Meta Key to Prevent Collision - Research

**Researched:** 2026-03-06
**Domain:** Rust JSON deep-merge, serde_json Map manipulation, builder-pattern metadata composition
**Confidence:** HIGH

## Summary

This phase addresses a metadata collision problem in `ToolInfo._meta` where multiple builder methods (UI, execution, custom) each replace the entire `_meta` map rather than merging into it. The fix is straightforward: implement a `deep_merge(base, overlay)` function for `serde_json::Map` and update all `_meta`-producing code paths to merge instead of replace.

The codebase is well-structured for this change. Phase 38 established that `_meta` is cached at registration time (in `ServerCoreBuilder::tool()`), so the merge only happens once per tool during builder construction, not per-request. The primary collision point is `TypedToolWithOutput::metadata()` which currently hardcodes `_meta: None`, discarding any UI metadata.

**Primary recommendation:** Add a standalone `deep_merge` function in `src/types/ui.rs` (collocated with `build_meta_map`), update all `metadata()` implementations to use merge, add `with_ui()` to `TypedToolWithOutput`, and add `with_meta_entry()` to `ToolInfo` for custom `_meta` contributions.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Deep merge: recursively merge nested JSON objects so `with_ui()` adds `{ui: {resourceUri: ...}}`, `with_execution()` adds `{execution: ...}`, and both coexist
- Only recurse into JSON objects -- arrays are replaced entirely by the overlay (not concatenated)
- Standalone function: `deep_merge(base: &mut Map, overlay: Map)` -- mutates base in-place, avoids allocation
- Last-in wins at the leaf level when two contributors set the same nested key
- Last-in wins collision rule -- simple, predictable, matches `HashMap::insert` semantics
- User controls priority by builder call order
- Log collisions at `tracing::debug` level -- useful for debugging, not noisy in production
- Fix existing internal methods (with_ui, metadata) to use deep merge instead of replacing `_meta`
- Add public `with_meta_entry(key: &str, value: Value)` on `ToolInfo` for custom `_meta` contributions -- adds one key at a time, composable
- Keep existing `with_meta()` (replace-all) alongside the new method -- different use cases, both valid, no deprecation
- Fix `ToolInfo._meta` only -- this is where the collision actually happens
- `CallToolResult._meta` and `GetPromptResult._meta` are set by single sources today -- no collision problem
- Add `with_ui()` to `TypedToolWithOutput` as part of this phase -- deep merge makes UI + output schema metadata coexistence natural

### Claude's Discretion
- Where to place the standalone deep_merge function (types/ui.rs, types/meta.rs, or a util module)
- Test structure and naming conventions
- Whether TypedToolWithOutput.with_ui() needs its own example

### Deferred Ideas (OUT OF SCOPE)
- Apply deep-merge to `CallToolResult._meta` and `GetPromptResult._meta` -- if middleware collision becomes a problem
- Meta key constants module (phase 35) -- extract string literals to named constants
- Deprecate `with_meta()` replace-all in favor of merge-only API -- evaluate after adoption
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | (workspace) | JSON Map manipulation, Value types | Already used throughout; `serde_json::Map<String, Value>` is the `_meta` type |
| tracing | 0.1 | Debug-level collision logging | Already a dependency; `tracing::debug!` for collision reporting |

### Supporting
No additional dependencies needed. This is a pure internal refactoring using existing types.

## Architecture Patterns

### Recommended Function Placement

Place `deep_merge` in `src/types/ui.rs` alongside `build_meta_map`. Rationale:
- `build_meta_map` is the primary producer of `_meta` content
- The merge function is consumed by the same callers (`metadata()` impls, `ToolInfo::with_ui()`)
- Avoids creating a new module for a single function
- If a `types/meta.rs` module exists in the future (phase 35), it can be relocated then

```
src/types/
├── ui.rs             # deep_merge() here, next to build_meta_map()
├── protocol.rs       # ToolInfo::with_meta_entry() here
```

### Pattern 1: Deep Merge Function

**What:** A standalone recursive merge for `serde_json::Map`
**When to use:** Whenever multiple contributors need to add keys to the same `_meta` map

```rust
/// Recursively merge `overlay` into `base`.
///
/// - JSON objects are merged recursively (keys from overlay added/overwritten in base)
/// - All other types (arrays, strings, numbers, bools, null) are replaced entirely
/// - Last-in wins at leaf level; collisions logged at debug level
pub fn deep_merge(base: &mut serde_json::Map<String, serde_json::Value>, overlay: serde_json::Map<String, serde_json::Value>) {
    for (key, overlay_value) in overlay {
        match base.get_mut(&key) {
            Some(serde_json::Value::Object(base_obj)) if overlay_value.is_object() => {
                // Both are objects: recurse
                if let serde_json::Value::Object(overlay_obj) = overlay_value {
                    deep_merge(base_obj, overlay_obj);
                }
            }
            Some(_existing) => {
                // Leaf collision: last-in wins
                tracing::debug!(key = %key, "deep_merge: overwriting existing _meta key");
                base.insert(key, overlay_value);
            }
            None => {
                // New key: just insert
                base.insert(key, overlay_value);
            }
        }
    }
}
```

### Pattern 2: TypedTool metadata() with Merge

**What:** Update `metadata()` to merge UI meta into any existing `_meta` instead of replacing
**Current code (TypedTool line 229-243):**
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let meta = self.ui_resource_uri.as_deref()
        .map(ToolUIMetadata::build_meta_map);
    Some(ToolInfo { ..., _meta: meta, ... })
}
```

**Updated pattern:**
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let mut meta = serde_json::Map::new();
    if let Some(uri) = &self.ui_resource_uri {
        let ui_meta = ToolUIMetadata::build_meta_map(uri);
        deep_merge(&mut meta, ui_meta);
    }
    Some(ToolInfo {
        ...,
        _meta: if meta.is_empty() { None } else { Some(meta) },
        ...
    })
}
```

### Pattern 3: ToolInfo::with_meta_entry

**What:** Public method to add a single key to `_meta`, merging with existing content
```rust
impl ToolInfo {
    /// Add a single key-value pair to `_meta`, merging with existing entries.
    ///
    /// If the key already exists and both values are objects, they are
    /// deep-merged. Otherwise the new value replaces the old (last-in wins).
    pub fn with_meta_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let meta = self._meta.get_or_insert_with(serde_json::Map::new);
        let mut overlay = serde_json::Map::with_capacity(1);
        overlay.insert(key.into(), value);
        deep_merge(meta, overlay);
        self
    }
}
```

### Pattern 4: TypedToolWithOutput with_ui Support

**What:** Add `ui_resource_uri: Option<String>` field and `with_ui()` builder method to `TypedToolWithOutput`, update its `metadata()` to merge UI meta.

```rust
// In TypedToolWithOutput struct:
ui_resource_uri: Option<String>,

// Builder method:
pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
    self.ui_resource_uri = Some(ui_resource_uri.into());
    self
}

// In metadata(): merge UI meta with output schema annotations
fn metadata(&self) -> Option<ToolInfo> {
    // ... existing annotation logic ...
    let mut meta = serde_json::Map::new();
    if let Some(uri) = &self.ui_resource_uri {
        let ui_meta = ToolUIMetadata::build_meta_map(uri);
        deep_merge(&mut meta, ui_meta);
    }
    Some(ToolInfo {
        ...,
        _meta: if meta.is_empty() { None } else { Some(meta) },
        ...
    })
}
```

### Anti-Patterns to Avoid
- **Replacing _meta entirely in metadata():** The whole point of this phase -- never `_meta: Some(new_map)`, always merge into existing or start from empty
- **Concatenating arrays during merge:** Decision is to replace arrays entirely, not concatenate -- avoids duplicate entries and undefined ordering
- **Logging at warn/info level:** Collisions are expected behavior (user controls with call order), `debug` level only

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON deep merge | Full RFC 7396 (JSON Merge Patch) implementation | Simple recursive object merge | RFC 7396 handles `null` deletion semantics which are unnecessary here; our use case is additive-only |
| Meta key validation | Schema validation for _meta keys | Simple string keys with `tracing::debug` on collision | `_meta` is unstructured by MCP spec -- validation would over-constrain |

## Common Pitfalls

### Pitfall 1: Forgetting to Initialize _meta Before Merging
**What goes wrong:** Calling `deep_merge` on a `None` `_meta` panics or requires unwrapping
**Why it happens:** `_meta` is `Option<Map>`, not `Map`
**How to avoid:** Always use `get_or_insert_with(Map::new)` or start from an empty `Map::new()` and set to `None` if still empty after merge
**Warning signs:** `unwrap()` calls on `_meta`

### Pitfall 2: TypedToolWithOutput Losing Output Schema Annotations
**What goes wrong:** Adding `with_ui()` to `TypedToolWithOutput` could accidentally reset annotations
**Why it happens:** `metadata()` has complex annotation merging logic (output schema auto-annotation)
**How to avoid:** Only touch `_meta` in `metadata()`, leave annotation logic untouched
**Warning signs:** Tests showing `annotations.output_schema` is `None` after `with_ui()`

### Pitfall 3: Breaking Existing with_meta() Semantics on CallToolResult
**What goes wrong:** Accidentally changing `CallToolResult::with_meta()` to use deep merge
**Why it happens:** Scope creep -- the same name suggests same treatment
**How to avoid:** Phase scope is `ToolInfo._meta` only; `CallToolResult` and `GetPromptResult` `with_meta()` stay as replace-all
**Warning signs:** Changes to `protocol.rs` lines 530 or 788

### Pitfall 4: Infinite Recursion in deep_merge
**What goes wrong:** Theoretically impossible with owned values, but worth noting
**Why it happens:** Would only happen with circular references (impossible in serde_json::Value)
**How to avoid:** serde_json::Value is a tree, not a graph -- recursion always terminates
**Warning signs:** N/A -- just document this is safe

### Pitfall 5: Forgetting WasmTypedTool
**What goes wrong:** WasmTypedTool still uses old `_meta: meta` pattern, doesn't benefit from merge
**Why it happens:** Separate file (`wasm_typed_tool.rs`), easy to miss
**How to avoid:** Update all four tool types: TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool
**Warning signs:** `wasm_typed_tool.rs` not in the diff

## Code Examples

### All Files That Need Changes

```
src/types/ui.rs           -- Add deep_merge() function
src/types/protocol.rs     -- Add ToolInfo::with_meta_entry()
src/server/typed_tool.rs  -- Update TypedTool::metadata(), TypedSyncTool::metadata(),
                             TypedToolWithOutput (add ui_resource_uri field, with_ui(), update metadata())
src/server/wasm_typed_tool.rs -- Update WasmTypedTool::info() to use merge pattern
```

### Current TypedToolWithOutput::metadata() (line 683-718)
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let mut annotations = self.annotations.clone().unwrap_or_default();
    if let Some(schema) = &self.output_schema {
        let type_name = schema.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Output").to_string();
        annotations = annotations.with_output_schema(schema.clone(), type_name);
    }
    let has_annotations = /* ... */;
    Some(ToolInfo {
        name: self.name.clone(),
        description: self.description.clone(),
        input_schema: self.input_schema.clone(),
        annotations: if has_annotations { Some(annotations) } else { None },
        _meta: None,  // <-- THIS IS THE BUG: always None
        execution: None,
    })
}
```

### Deep Merge Test Cases
```rust
#[test]
fn test_deep_merge_disjoint_keys() {
    let mut base = serde_json::Map::new();
    base.insert("a".into(), json!(1));
    let mut overlay = serde_json::Map::new();
    overlay.insert("b".into(), json!(2));
    deep_merge(&mut base, overlay);
    assert_eq!(base.get("a"), Some(&json!(1)));
    assert_eq!(base.get("b"), Some(&json!(2)));
}

#[test]
fn test_deep_merge_nested_objects() {
    let mut base = serde_json::Map::new();
    base.insert("ui".into(), json!({"resourceUri": "ui://test"}));
    let mut overlay = serde_json::Map::new();
    overlay.insert("ui".into(), json!({"prefersBorder": true}));
    deep_merge(&mut base, overlay);
    let ui = base.get("ui").unwrap().as_object().unwrap();
    assert_eq!(ui.get("resourceUri"), Some(&json!("ui://test")));
    assert_eq!(ui.get("prefersBorder"), Some(&json!(true)));
}

#[test]
fn test_deep_merge_leaf_collision_last_in_wins() {
    let mut base = serde_json::Map::new();
    base.insert("key".into(), json!("old"));
    let mut overlay = serde_json::Map::new();
    overlay.insert("key".into(), json!("new"));
    deep_merge(&mut base, overlay);
    assert_eq!(base.get("key"), Some(&json!("new")));
}

#[test]
fn test_deep_merge_array_replaced_not_concatenated() {
    let mut base = serde_json::Map::new();
    base.insert("tags".into(), json!(["a", "b"]));
    let mut overlay = serde_json::Map::new();
    overlay.insert("tags".into(), json!(["c"]));
    deep_merge(&mut base, overlay);
    assert_eq!(base.get("tags"), Some(&json!(["c"])));
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `_meta: Some(build_meta_map(uri))` | `deep_merge(&mut meta, build_meta_map(uri))` | Phase 39 | Multiple _meta contributors coexist |
| `TypedToolWithOutput` has no UI support | `with_ui()` builder method added | Phase 39 | UI tools can have typed output schemas |
| No public _meta manipulation on ToolInfo | `with_meta_entry(key, value)` added | Phase 39 | Custom _meta keys composable with UI |

## Open Questions

1. **Should deep_merge be pub(crate) or pub?**
   - What we know: Only internal callers need it today (`metadata()` impls, `with_meta_entry`)
   - What's unclear: Whether users might want to merge custom _meta maps
   - Recommendation: Start as `pub` -- it's a general utility on serde_json::Map, no harm in exposing

2. **Should with_meta_entry take &str or impl Into<String>?**
   - What we know: Builder methods in this codebase use `impl Into<String>`
   - Recommendation: Use `impl Into<String>` for consistency

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test --lib` |
| Full suite command | `make tests` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A | deep_merge disjoint keys | unit | `cargo test deep_merge_disjoint -p pmcp` | No -- Wave 0 |
| N/A | deep_merge nested objects | unit | `cargo test deep_merge_nested -p pmcp` | No -- Wave 0 |
| N/A | deep_merge leaf collision | unit | `cargo test deep_merge_leaf_collision -p pmcp` | No -- Wave 0 |
| N/A | deep_merge array replacement | unit | `cargo test deep_merge_array -p pmcp` | No -- Wave 0 |
| N/A | TypedToolWithOutput with_ui metadata | unit | `cargo test typed_tool_with_output.*ui -p pmcp` | No -- Wave 0 |
| N/A | TypedToolWithOutput with_ui + output_schema coexist | unit | `cargo test typed_tool_with_output.*coexist -p pmcp` | No -- Wave 0 |
| N/A | ToolInfo with_meta_entry merge | unit | `cargo test with_meta_entry -p pmcp` | No -- Wave 0 |
| N/A | Existing UI metadata tests still pass | unit | `cargo test tool_info_with_ui -p pmcp` | Yes |

### Sampling Rate
- **Per task commit:** `cargo test --lib -p pmcp`
- **Per wave merge:** `make tests`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `deep_merge` unit tests in `src/types/ui.rs`
- [ ] `with_meta_entry` tests in `src/types/protocol.rs`
- [ ] `TypedToolWithOutput` UI tests in `src/server/typed_tool.rs`

## Sources

### Primary (HIGH confidence)
- Direct code inspection of `src/types/ui.rs`, `src/types/protocol.rs`, `src/server/typed_tool.rs`, `src/server/wasm_typed_tool.rs`, `src/server/builder.rs`, `src/server/core.rs`
- Phase 38 implementation (cached ToolInfo at registration time)
- Phase 39 CONTEXT.md (locked decisions)

### Secondary (MEDIUM confidence)
- serde_json `Map` API -- well-known stable API, `insert`, `get_mut`, `get_or_insert_with` are standard

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all existing crate APIs
- Architecture: HIGH - clear pattern from existing code, straightforward refactor
- Pitfalls: HIGH - identified from direct code reading, specific line numbers

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (stable internal refactoring, no external API dependencies)
