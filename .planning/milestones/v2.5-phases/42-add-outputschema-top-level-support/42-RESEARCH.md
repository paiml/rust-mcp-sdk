# Phase 42: Add outputSchema Top Level Support - Research

**Researched:** 2026-03-07
**Domain:** MCP spec alignment -- ToolInfo struct, macro codegen, serde serialization
**Confidence:** HIGH

## Summary

The MCP specification revision 2025-06-18 added `outputSchema` as a **top-level field on Tool**, sibling to `inputSchema`. This is confirmed by the official spec at modelcontextprotocol.io/specification/2025-06-18/server/tools, which shows the example JSON with `outputSchema` at the same level as `inputSchema`, not nested inside `annotations`.

The PMCP SDK currently stores output schema in `ToolAnnotations` as `pmcp:outputSchema` (a PMCP extension). This phase moves it to a top-level `Option<Value>` field on `ToolInfo`, removes the `output_schema` field and `with_output_schema()` method from `ToolAnnotations`, and keeps only `pmcp:outputTypeName` in annotations as a codegen-specific extension.

**Primary recommendation:** Add `output_schema: Option<Value>` to `ToolInfo` (serde handles `camelCase` rename automatically), remove `output_schema` from `ToolAnnotations`, update `TypedToolWithOutput::metadata()` to set the top-level field, update macro codegen to emit top-level field, and let compiler errors guide remaining consumer updates.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Clean break: remove `output_schema` field from `ToolAnnotations` struct entirely
- No dual-emit, no deprecation period -- small user base, version-locked
- Remove `with_output_schema()` method from `ToolAnnotations` entirely
- Keep `pmcp:outputTypeName` in annotations -- no standard equivalent, needed for codegen
- Add `with_output_type_name(name)` on `ToolAnnotations` for setting type name independently
- `output_schema` is a public `Option<Value>` field on `ToolInfo` -- settable inline (like `input_schema`)
- Add `ToolInfo::with_output_schema(schema)` builder method for chaining (symmetric with `input_schema`)
- `TypedToolWithOutput::metadata()` sets `output_schema` directly on `ToolInfo` (not via annotations)
- `#[tool]` macro sets top-level `ToolInfo.output_schema` directly, only puts `pmcp:outputTypeName` in annotations
- All consumers switch at once -- no fallback to annotations
- Compiler errors from removing annotations field will guide all call sites
- `cargo-pmcp/src/commands/schema.rs` local structs updated to mirror the change
- Example `48_structured_output_schema.rs` updated to use top-level field
- Docs updated: `OUTPUT_SCHEMA_ANNOTATIONS.md`, relevant course chapters
- Tests in `tests/tool_annotations_test.rs` rewritten to verify top-level field + JSON serialization

### Claude's Discretion
- Internal wiring details in TypedToolWithOutput and macro codegen
- How to handle the `with_output_type_name()` method signature
- Test structure and assertion patterns
- Course chapter update scope (minimal vs comprehensive rewrite)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Architecture Patterns

### Change Topology

This is a struct field migration with 6 layers of impact:

```
1. ToolAnnotations (remove field + method)
   |
2. ToolInfo (add field + builder method)
   |
3. TypedToolWithOutput::metadata() (rewire output_schema target)
   |
4. #[tool] macro codegen (emit to ToolInfo, not annotations)
   |
5. cargo-pmcp local structs (mirror changes)
   |
6. Tests, examples, docs (update assertions + prose)
```

### ToolInfo Struct Change

Current:
```rust
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub annotations: Option<ToolAnnotations>,
    pub _meta: Option<serde_json::Map<String, Value>>,
    pub execution: Option<Value>,
}
```

After:
```rust
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,       // NEW -- sibling to input_schema
    pub annotations: Option<ToolAnnotations>,
    pub _meta: Option<serde_json::Map<String, Value>>,
    pub execution: Option<Value>,
}
```

The `#[serde(rename_all = "camelCase")]` on `ToolInfo` automatically renames `output_schema` to `outputSchema` in JSON. No manual `#[serde(rename)]` needed.

### ToolAnnotations Change

Remove two items:
```rust
// REMOVE this field:
pub output_schema: Option<Value>,

// REMOVE this method:
pub fn with_output_schema(mut self, schema: Value, type_name: impl Into<String>) -> Self

// KEEP this field:
pub output_type_name: Option<String>,  // pmcp:outputTypeName

// ADD this method:
pub fn with_output_type_name(mut self, name: impl Into<String>) -> Self {
    self.output_type_name = Some(name.into());
    self
}
```

### TypedToolWithOutput::metadata() Rewire

Current logic (lines 688-722 of typed_tool.rs):
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let mut annotations = self.annotations.clone().unwrap_or_default();
    if let Some(schema) = &self.output_schema {
        let type_name = schema.get("title")...;
        annotations = annotations.with_output_schema(schema.clone(), type_name);
    }
    // ... builds ToolInfo with annotations containing output_schema
}
```

After:
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let mut annotations = self.annotations.clone().unwrap_or_default();
    if let Some(schema) = &self.output_schema {
        let type_name = schema.get("title")...;
        annotations = annotations.with_output_type_name(type_name);
    }
    // ... determine if annotations has meaningful content ...
    Some(ToolInfo {
        // ...
        output_schema: self.output_schema.clone(),  // top-level
        annotations: if has_annotations { Some(annotations) } else { None },
        // ...
    })
}
```

The `has_annotations` check must be updated: remove `annotations.output_schema.is_some()` from the condition (line 709), add `annotations.output_type_name.is_some()` if not already covered.

### Macro Codegen Change

In `pmcp-macros/src/tool.rs`, the `generate_definition_code` function (line 194) currently emits:
```rust
.with_output_schema(Self::output_schema(), #output_type_name)
```

After, it should emit something like:
```rust
// Build annotations without output_schema
let annotations = #(#annotation_chain)*
    .with_output_type_name(#output_type_name);

// Build ToolInfo with top-level output_schema
let mut info = pmcp::types::ToolInfo::with_annotations(
    #tool_name,
    Some(#description.to_string()),
    Self::input_schema(),
    annotations,
);
info.output_schema = Some(Self::output_schema());
info
```

Or use the new builder method:
```rust
pmcp::types::ToolInfo::with_annotations(...)
    .with_output_schema(Self::output_schema())
```

### ToolInfo Builder Method

Add symmetric builder:
```rust
impl ToolInfo {
    pub fn with_output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}
```

This mirrors the pattern of other `with_*` methods on `ToolInfo` (`with_meta`, `with_meta_entry`, `with_annotations`).

### ToolInfo Constructors Update

Both `ToolInfo::new()` and `ToolInfo::with_annotations()` must set `output_schema: None` in their struct literals. The `with_ui()` constructor also needs the field.

### cargo-pmcp Schema Structs

`cargo-pmcp/src/commands/schema.rs` has a local `ToolSchema` (line 112) and `ToolAnnotations` (line 137):

1. Add `output_schema` field to `ToolSchema` (top-level, with alias `outputSchema`)
2. Remove `output_schema` field from local `ToolAnnotations`
3. Keep `output_type_name` in local `ToolAnnotations`

### Expected JSON Output (MCP Spec Conformant)

```json
{
  "name": "get_weather_data",
  "description": "Get weather data",
  "inputSchema": { "type": "object", ... },
  "outputSchema": { "type": "object", ... },
  "annotations": {
    "readOnlyHint": true,
    "pmcp:outputTypeName": "WeatherData"
  }
}
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| camelCase JSON field naming | Manual `#[serde(rename)]` for output_schema | `#[serde(rename_all = "camelCase")]` already on ToolInfo | Consistency with existing pattern |
| Finding all affected call sites | Manual grep + hope | Compiler errors from removing ToolAnnotations field | Rust compiler catches every usage |

**Key insight:** Removing the field from `ToolAnnotations` is the forcing function -- the compiler will find every call site that references `output_schema` on annotations, making migration exhaustive and safe.

## Common Pitfalls

### Pitfall 1: Missing ToolInfo Field in Struct Literals
**What goes wrong:** Every place that constructs a `ToolInfo` with struct literal syntax will fail to compile because `output_schema` field is missing.
**Why it happens:** `ToolInfo` is `#[non_exhaustive]` but internal code uses struct literals.
**How to avoid:** Search for all `ToolInfo {` struct literals in the codebase and add `output_schema: None` (or the appropriate value). There are at least 5 sites: `TypedTool::metadata()`, `TypedSyncTool::metadata()`, `TypedToolWithOutput::metadata()`, `ToolInfo::new()`, `ToolInfo::with_annotations()`, `ToolInfo::with_ui()`.
**Warning signs:** Compile errors listing missing field.

### Pitfall 2: Test Assertions Still Checking Annotations for Output Schema
**What goes wrong:** Tests asserting `annotations.output_schema.is_some()` will fail at compile time since the field no longer exists.
**How to avoid:** Rewrite tests to check `tool_info.output_schema` at top level and `annotations.output_type_name` in annotations.

### Pitfall 3: has_annotations Check Drift
**What goes wrong:** `TypedToolWithOutput::metadata()` has a manual `has_annotations` check (line 705-709) that currently includes `annotations.output_schema.is_some()`. If not updated, tools with only output_type_name won't emit annotations.
**How to avoid:** Replace `output_schema.is_some()` with `output_type_name.is_some()` in the condition.

### Pitfall 4: Macro cfg Guards
**What goes wrong:** The macro codegen in `pmcp-macros/src/tool.rs` has `#[cfg(feature = "schema-generation")]` / `#[cfg(not(...))]` blocks (line 260-280). Both paths need updating.
**How to avoid:** Update both the schema-generation and non-schema-generation paths.

### Pitfall 5: Calculator Template in cargo-pmcp
**What goes wrong:** `cargo-pmcp/src/templates/calculator.rs` references `pmcp:outputSchema` in doc comments. Not a compile error but misleading docs.
**How to avoid:** Update doc comments to reference top-level `outputSchema`.

## Complete File Inventory

All files requiring changes (confirmed by grep + code analysis):

### Core SDK (compile-critical)
1. `src/types/protocol.rs` -- ToolAnnotations (remove field/method), ToolInfo (add field + builder + constructors)
2. `src/server/typed_tool.rs` -- TypedToolWithOutput::metadata() rewire, has_annotations check

### Macro (compile-critical)
3. `pmcp-macros/src/tool.rs` -- generate_definition_code() to emit top-level output_schema

### CLI (compile-critical)
4. `cargo-pmcp/src/commands/schema.rs` -- local ToolSchema + ToolAnnotations structs

### Tests (compile-critical)
5. `tests/tool_annotations_test.rs` -- rewrite output_schema assertions

### Templates (compile-affecting)
6. `cargo-pmcp/src/templates/calculator.rs` -- doc comments referencing pmcp:outputSchema

### Examples
7. `examples/48_structured_output_schema.rs` -- update if it constructs ToolInfo manually (current version does NOT use output_schema directly, but doc comments should be updated)

### Documentation
8. `docs/OUTPUT_SCHEMA_ANNOTATIONS.md` -- update to reflect top-level field
9. `pmcp-course/src/part2-design/ch05-02-output-schemas.md` -- update annotation references
10. `pmcp-course/src/part2-design/ch05-03-annotations.md` -- remove output_schema from annotations discussion

### Changelog
11. `CHANGELOG.md` -- document breaking change

## Code Examples

### New ToolAnnotations with_output_type_name
```rust
// Source: Designed per CONTEXT.md decision
impl ToolAnnotations {
    pub fn with_output_type_name(mut self, name: impl Into<String>) -> Self {
        self.output_type_name = Some(name.into());
        self
    }
}
```

### New ToolInfo::with_output_schema Builder
```rust
// Source: Symmetric with existing ToolInfo builder pattern
impl ToolInfo {
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}
```

### Updated TypedToolWithOutput::metadata()
```rust
fn metadata(&self) -> Option<ToolInfo> {
    let mut annotations = self.annotations.clone().unwrap_or_default();

    // Put only type name in annotations (PMCP extension for codegen)
    if let Some(schema) = &self.output_schema {
        let type_name = schema
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Output")
            .to_string();
        annotations = annotations.with_output_type_name(type_name);
    }

    let has_annotations = annotations.read_only_hint.is_some()
        || annotations.destructive_hint.is_some()
        || annotations.idempotent_hint.is_some()
        || annotations.open_world_hint.is_some()
        || annotations.output_type_name.is_some();

    Some(ToolInfo {
        name: self.name.clone(),
        description: self.description.clone(),
        input_schema: self.input_schema.clone(),
        output_schema: self.output_schema.clone(),  // top-level
        annotations: if has_annotations { Some(annotations) } else { None },
        _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
        execution: None,
    })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No output schema in spec | `outputSchema` as top-level Tool field | MCP 2025-06-18 | SDK must align |
| PMCP extension: `pmcp:outputSchema` in annotations | Top-level `outputSchema` on Tool | This phase | Breaking change for annotation consumers |

## Open Questions

1. **Should `ToolInfo::with_annotations()` constructor accept output_schema?**
   - What we know: Current constructor only takes name, description, input_schema, annotations
   - Recommendation: No -- keep the constructor unchanged, use builder chaining: `ToolInfo::with_annotations(...).with_output_schema(schema)`. This avoids an explosion of constructor variants.

2. **Should example 48 be updated to actually use output_schema on ToolInfo?**
   - What we know: Current example registers tools via `.tool("name", handler)` without explicit ToolInfo construction. It does not use TypedToolWithOutput or the #[tool] macro.
   - Recommendation: Update doc comments only. The example demonstrates structured content responses, not output schema declaration. Consider adding a TypedToolWithOutput variant to show the feature.

## Sources

### Primary (HIGH confidence)
- [MCP Spec 2025-06-18 Tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools) -- confirms `outputSchema` is top-level sibling to `inputSchema`
- Direct code analysis of `src/types/protocol.rs`, `src/server/typed_tool.rs`, `pmcp-macros/src/tool.rs`, `cargo-pmcp/src/commands/schema.rs`, `tests/tool_annotations_test.rs`

### Secondary (MEDIUM confidence)
- [MCP Schema Reference](https://modelcontextprotocol.io/specification/2025-06-18/schema) -- JSON Schema definition

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new libraries, pure struct field migration
- Architecture: HIGH -- all affected code read and analyzed, pattern is straightforward
- Pitfalls: HIGH -- compiler-driven migration makes it hard to miss sites

**Research date:** 2026-03-07
**Valid until:** 2026-04-07 (stable -- struct migration doesn't age)
