# Phase 42: Add outputSchema Top Level Support - Context

**Gathered:** 2026-03-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `output_schema` as a top-level field on `ToolInfo` (serializes as `outputSchema`), aligning with MCP spec 2025-06-18. Remove `pmcp:outputSchema` from `ToolAnnotations` entirely (clean break). Keep `pmcp:outputTypeName` in annotations as a PMCP codegen extension. Update all internal consumers, macro codegen, examples, docs, and tests.

</domain>

<decisions>
## Implementation Decisions

### Deprecation strategy
- Clean break: remove `output_schema` field from `ToolAnnotations` struct entirely
- No dual-emit, no deprecation period — small user base, version-locked
- Remove `with_output_schema()` method from `ToolAnnotations` entirely
- Keep `pmcp:outputTypeName` in annotations — no standard equivalent, needed for codegen
- Add `with_output_type_name(name)` on `ToolAnnotations` for setting type name independently

### Builder API surface
- `output_schema` is a public `Option<Value>` field on `ToolInfo` — settable inline (like `input_schema`)
- Add `ToolInfo::with_output_schema(schema)` builder method for chaining (symmetric with `input_schema`)
- `TypedToolWithOutput::metadata()` sets `output_schema` directly on `ToolInfo` (not via annotations)
- `#[tool]` macro sets top-level `ToolInfo.output_schema` directly, only puts `pmcp:outputTypeName` in annotations

### Consumer migration
- All consumers switch at once — no fallback to annotations
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

</decisions>

<specifics>
## Specific Ideas

- Output JSON should match MCP spec 2025-06-18 exactly: `outputSchema` as sibling to `inputSchema`
- `pmcp:outputTypeName` stays in annotations — it's a PMCP-specific codegen concern with no spec equivalent
- Symmetry between `input_schema` and `output_schema` on `ToolInfo` is important for API consistency

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ToolInfo` struct at `src/types/protocol.rs:239` — add field here
- `ToolAnnotations` at `src/types/protocol.rs:97` — remove `output_schema` field and `with_output_schema()` method
- `TypedToolWithOutput` at `src/server/typed_tool.rs` — already stores `output_schema` internally, currently injects into annotations
- `#[tool]` macro at `pmcp-macros/src/tool.rs:228` — generates `output_schema()` method and annotation chain

### Established Patterns
- Builder pattern: `with_*()` methods on ToolInfo (e.g., `with_meta()`, `with_meta_entry()`, `with_annotations()`)
- `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields
- `rename_all = "camelCase"` on ToolInfo handles `output_schema` -> `outputSchema` automatically

### Integration Points
- `src/server/typed_tool.rs` — `metadata()` method builds ToolInfo with annotations
- `pmcp-macros/src/tool.rs` — macro codegen for `#[tool(output_type = "...")]`
- `cargo-pmcp/src/commands/schema.rs` — local ToolAnnotations/ToolInfo structs for deserialization
- `tests/tool_annotations_test.rs` — annotation serialization tests
- `examples/48_structured_output_schema.rs` — example using output schema
- `docs/OUTPUT_SCHEMA_ANNOTATIONS.md` — documentation
- `pmcp-course/src/part2-design/ch05-02-output-schemas.md` and `ch05-03-annotations.md` — course content

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 42-add-outputschema-top-level-support*
*Context gathered: 2026-03-07*
