# Phase 37: Add with_ui Support to TypedSyncTool - Research

**Researched:** 2026-03-06
**Domain:** Rust builder pattern extension for MCP tool structs
**Confidence:** HIGH

## Summary

This phase adds `with_ui()` builder methods to `TypedSyncTool` and `WasmTypedTool` to achieve API parity with the existing `TypedTool` (async) implementation. The pattern is already established and battle-tested in `TypedTool` -- this is a mechanical copy of that pattern to two additional structs.

The work is entirely internal to two source files (`typed_tool.rs` and `wasm_typed_tool.rs`) with zero dependency changes. The canonical `ToolUIMetadata::build_meta_map()` function in `src/types/ui.rs` handles all `_meta` construction, so neither struct needs to know about the nested `ui.resourceUri` / `openai/outputTemplate` format details.

**Primary recommendation:** Mirror `TypedTool::with_ui()` exactly -- add `ui_resource_uri: Option<String>` field, `with_ui()` builder method, and `_meta` emission in `metadata()`/`info()` methods. Two files, four touch points per struct.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Add `with_ui()` to both `TypedSyncTool` AND `WasmTypedTool` for consistency
- Do NOT add annotation methods (`with_annotations`, `read_only`, etc.) to `WasmTypedTool` -- separate concern
- Do NOT add `SimpleWasmTool` UI support -- it's a convenience wrapper, not primary API
- Mirror `TypedTool::with_ui()` exactly: store `ui_resource_uri: Option<String>` field, build `_meta` via `ToolUIMetadata::build_meta_map()` in metadata/info method
- `TypedSyncTool::metadata()` should emit `_meta` when `ui_resource_uri` is set (currently always returns `_meta: None`)
- `WasmTypedTool::info()` should emit `_meta` when `ui_resource_uri` is set
- Metadata emission only for WASM -- do NOT add structured_content + _meta enrichment to WASM tool call results

### Claude's Discretion
- Whether to update existing examples to use `TypedSyncTool::with_ui()`
- Test structure and naming conventions

### Deferred Ideas (OUT OF SCOPE)
- Add annotation methods (`with_annotations`, `read_only`, etc.) to `WasmTypedTool` -- API parity phase
- Add structured_content + _meta enrichment to WASM server tool call results -- WASM MCP Apps phase
- Add `with_ui` to `SimpleWasmTool` -- if demand arises

</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_json | workspace | Build `_meta` Map via `ToolUIMetadata::build_meta_map()` | Already used throughout |

No new dependencies. All changes use existing crate-internal types.

## Architecture Patterns

### Pattern 1: UI Resource Field + Builder + Metadata Emission

This is the established pattern from `TypedTool` (lines 36, 205-208, 229-243 of `typed_tool.rs`):

**Step 1 -- Add field to struct:**
```rust
pub struct TypedSyncTool<T, F> {
    name: String,
    description: Option<String>,
    input_schema: Value,
    annotations: Option<ToolAnnotations>,
    ui_resource_uri: Option<String>,  // NEW
    handler: F,
    _phantom: PhantomData<T>,
}
```

**Step 2 -- Initialize field in constructors (`new`, `new_with_schema`):**
```rust
ui_resource_uri: None,
```

**Step 3 -- Add builder method:**
```rust
/// Associate this tool with a UI resource (MCP Apps Extension).
pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
    self.ui_resource_uri = Some(ui_resource_uri.into());
    self
}
```

**Step 4 -- Emit `_meta` in metadata/info method:**
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let meta = self.ui_resource_uri.as_ref().map(|uri| {
        crate::types::ui::ToolUIMetadata::build_meta_map(uri)
    });

    Some(ToolInfo {
        name: self.name.clone(),
        description: self.description.clone(),
        input_schema: self.input_schema.clone(),
        annotations: self.annotations.clone(),
        _meta: meta,
        execution: None,
    })
}
```

### Pattern 2: WasmTypedTool (same pattern, different trait method)

`WasmTypedTool` implements `WasmTool::info()` (returns `ToolInfo` directly, not `Option<ToolInfo>`). Same field/builder/emission pattern, but the method is `info(&self) -> ToolInfo` instead of `metadata(&self) -> Option<ToolInfo>`.

### Anti-Patterns to Avoid
- **Building _meta manually:** Always use `ToolUIMetadata::build_meta_map()` -- it handles nested `ui.resourceUri` + `openai/outputTemplate` dual-emit correctly.
- **Adding annotations to WasmTypedTool:** Explicitly out of scope per user decision.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| _meta map construction | Manual HashMap building | `ToolUIMetadata::build_meta_map(uri)` | Handles nested format + ChatGPT alias consistently |

## Common Pitfalls

### Pitfall 1: Forgetting Constructor Initialization
**What goes wrong:** New field added to struct but not initialized in all constructors, causing compile error.
**How to avoid:** `TypedSyncTool` has `new()` (cfg schema-generation) and `new_with_schema()` -- both need `ui_resource_uri: None`. `WasmTypedTool` has `new()` (cfg schema-generation) and `new_with_schema()` -- both need it too.
**Warning signs:** Compile error on struct literal missing field.

### Pitfall 2: WasmTypedTool Info vs Metadata
**What goes wrong:** Confusing `WasmTool::info()` (returns `ToolInfo`) with `ToolHandler::metadata()` (returns `Option<ToolInfo>`).
**How to avoid:** `WasmTypedTool` implements `WasmTool`, not `ToolHandler`. The method is `info(&self) -> ToolInfo`.

## Code Examples

### TypedSyncTool with_ui (exact target code)
```rust
// Source: Mirrored from TypedTool::with_ui() at src/server/typed_tool.rs:205
/// Associate this tool with a UI resource (MCP Apps Extension).
///
/// This sets the nested `_meta.ui.resourceUri` field and the `openai/outputTemplate`
/// alias in the tool's `_meta`, allowing both MCP and ChatGPT hosts to display
/// an interactive UI when this tool is invoked.
///
/// # Example
///
/// ```rust
/// use pmcp::server::typed_tool::TypedSyncTool;
/// use serde::Deserialize;
/// use schemars::JsonSchema;
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct LookupArgs {
///     query: String,
/// }
///
/// let tool = TypedSyncTool::new("lookup", |args: LookupArgs, _extra| {
///     Ok(serde_json::json!({"result": args.query}))
/// })
/// .with_description("Look up data")
/// .with_ui("ui://widgets/results");
/// ```
pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
    self.ui_resource_uri = Some(ui_resource_uri.into());
    self
}
```

### WasmTypedTool with_ui (exact target code)
```rust
// Source: Same pattern, adapted for WasmTypedTool at src/server/wasm_typed_tool.rs
/// Associate this tool with a UI resource (MCP Apps Extension).
///
/// Sets `_meta.ui.resourceUri` and `openai/outputTemplate` in the tool's
/// `ToolInfo` metadata for MCP and ChatGPT host compatibility.
pub fn with_ui(mut self, ui_resource_uri: impl Into<String>) -> Self {
    self.ui_resource_uri = Some(ui_resource_uri.into());
    self
}
```

### WasmTypedTool info() with _meta emission
```rust
fn info(&self) -> ToolInfo {
    let meta = self.ui_resource_uri.as_ref().map(|uri| {
        crate::types::ui::ToolUIMetadata::build_meta_map(uri)
    });

    ToolInfo {
        name: self.name.clone(),
        description: self.description.clone(),
        input_schema: self.input_schema.clone(),
        annotations: None,
        _meta: meta,
        execution: None,
    }
}
```

### Test pattern (from existing TypedTool tests)
```rust
// Source: src/server/typed_tool.rs:695-715
#[test]
fn test_typed_sync_tool_metadata_with_ui_has_openai_output_template() {
    let tool = TypedSyncTool::new_with_schema(
        "test_tool",
        json!({"type": "object"}),
        |_args: serde_json::Value, _extra| Ok(json!({})),
    )
    .with_ui("ui://widgets/chart.html");

    let info = tool.metadata().unwrap();
    let meta = info._meta.as_ref().expect("_meta should be present");

    // Must have nested ui.resourceUri
    let ui_obj = meta.get("ui").expect("must have nested 'ui' key");
    assert_eq!(ui_obj["resourceUri"], "ui://widgets/chart.html");

    // Must have openai/outputTemplate
    assert_eq!(
        meta.get("openai/outputTemplate").unwrap(),
        &serde_json::Value::String("ui://widgets/chart.html".to_string())
    );
}

#[test]
fn test_typed_sync_tool_metadata_without_ui_has_no_meta() {
    let tool = TypedSyncTool::new_with_schema(
        "test_tool",
        json!({"type": "object"}),
        |_args: serde_json::Value, _extra| Ok(json!({})),
    );

    let info = tool.metadata().unwrap();
    assert!(info._meta.is_none(), "_meta should be None without UI");
}
```

## Touch Points Summary

| File | Struct | Changes |
|------|--------|---------|
| `src/server/typed_tool.rs` | `TypedSyncTool` | Add `ui_resource_uri` field, init in 2 constructors, add `with_ui()`, update `metadata()` |
| `src/server/wasm_typed_tool.rs` | `WasmTypedTool` | Add `ui_resource_uri` field, init in 2 constructors, add `with_ui()`, update `info()` |
| `src/server/typed_tool.rs` | tests | Add 2 tests for TypedSyncTool (with/without UI) |
| `src/server/wasm_typed_tool.rs` | tests | Add 2 tests for WasmTypedTool (with/without UI) |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test --lib -- typed_tool` |
| Full suite command | `cargo test --test-threads=1` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P37-01 | TypedSyncTool with_ui sets _meta correctly | unit | `cargo test --lib -- typed_tool::tests::test_typed_sync_tool_metadata_with_ui` | Wave 0 |
| P37-02 | TypedSyncTool without UI has no _meta | unit | `cargo test --lib -- typed_tool::tests::test_typed_sync_tool_metadata_without_ui` | Wave 0 |
| P37-03 | WasmTypedTool with_ui sets _meta correctly | unit | `cargo test --lib -- wasm_typed_tool::tests::test_wasm_typed_tool_info_with_ui` | Wave 0 |
| P37-04 | WasmTypedTool without UI has no _meta | unit | `cargo test --lib -- wasm_typed_tool::tests::test_wasm_typed_tool_info_without_ui` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib -- typed_tool wasm_typed_tool`
- **Per wave merge:** `cargo test --test-threads=1`
- **Phase gate:** Full suite green + `cargo clippy -- -D warnings`

### Wave 0 Gaps
None -- tests will be created as part of the implementation (same file, `mod tests` block already exists in both files).

## Sources

### Primary (HIGH confidence)
- `src/server/typed_tool.rs` -- `TypedTool::with_ui()` reference implementation (lines 205-208)
- `src/server/typed_tool.rs` -- `TypedSyncTool` struct and `metadata()` (lines 247-379)
- `src/server/wasm_typed_tool.rs` -- `WasmTypedTool` struct and `info()` (lines 22-105)
- `src/types/ui.rs` -- `ToolUIMetadata::build_meta_map()` (lines 276-289)
- `src/types/protocol.rs` -- `ToolInfo::with_ui()` (lines 327-344)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all internal
- Architecture: HIGH - exact pattern exists in `TypedTool`, mechanical copy
- Pitfalls: HIGH - code is fully visible, all touch points identified

**Research date:** 2026-03-06
**Valid until:** 2026-04-06 (stable internal pattern, no external dependencies)
