# Phase 59: TypedPrompt with Auto-Deserialization - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Prompt arguments deserialize from `HashMap<String, String>` into a typed struct via JsonSchema + serde, eliminating the manual `args.get("x").ok_or()?.parse()?` pattern on every prompt. Builder-friendly registration via `.prompt("name", TypedPrompt::new(handler))`. Add `#[mcp_prompt]` attribute macro mirroring `#[mcp_tool]` and extend `#[mcp_server]` to collect prompts.

</domain>

<decisions>
## Implementation Decisions

### DX-first philosophy (carries forward from Phase 58)
- **D-01:** Mirror the `#[mcp_tool]` DX pattern exactly — developers who learned tools should recognize prompts instantly
- **D-02:** Description mandatory at compile time — LLMs need prompt descriptions
- **D-03:** `extra` (RequestHandlerExtra) is opt-in — only declare in signature if needed
- **D-04:** Name defaults to function name, override with `name = "custom"`

### TypedPrompt runtime type
- **D-05:** `TypedPrompt<T, F>` where T: `Deserialize + JsonSchema` — deserializes `HashMap<String, String>` into typed struct T
- **D-06:** Deserialization strategy: convert `HashMap<String, String>` to `serde_json::Value` (object with string values), then `serde_json::from_value::<T>()` — handles string-to-type coercion via serde
- **D-07:** Generates `PromptInfo` with argument schema from T's JsonSchema implementation — clients can discover argument names, types, and descriptions
- **D-08:** Registration: `.prompt("name", TypedPrompt::new("name", handler).with_description("..."))`

### #[mcp_prompt] attribute macro
- **D-09:** Use `#[mcp_prompt]` (not `#[prompt]`) — consistent with `#[mcp_tool]` naming convention; existing `#[prompt]` stub is a no-op passthrough
- **D-10:** Standalone function pattern: `#[mcp_prompt(description = "...")]` on `async fn` returning `Result<GetPromptResult>`
- **D-11:** State injection via `State<T>` — same pattern as `#[mcp_tool]`
- **D-12:** Generates a struct implementing `PromptHandler` with `handle()` + `metadata()`
- **D-13:** Constructor function: `fn prompt_name() -> PromptNamePrompt` for ergonomic registration

### #[mcp_server] extension
- **D-14:** `#[mcp_server]` collects both `#[mcp_tool]` AND `#[mcp_prompt]` methods from the same impl block
- **D-15:** `McpServer::register_tools()` renamed to `McpServer::register()` — registers tools AND prompts
- **D-16:** Prompts use `&self` for state access in impl blocks — identical to tools

### Prompt argument schema
- **D-17:** Generate `PromptArgument` entries from T's JsonSchema — each struct field becomes a prompt argument with name, description (from doc comment or serde attr), and required flag (non-Option fields are required)
- **D-18:** `#[serde(default)]` fields are treated as optional arguments

### Claude's Discretion
- Internal codegen details for HashMap<String, String> → typed struct conversion
- Error messages for deserialization failures (argument name, expected type)
- Whether to support sync prompts (fn vs async fn) — likely yes for consistency
- How PromptArgument descriptions are extracted from JsonSchema
- Test strategy mirroring Phase 58 (integration tests, compile-fail tests)
- Re-export `#[mcp_prompt]` from pmcp crate alongside `#[mcp_tool]`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 58 implementation (pattern to follow)
- `pmcp-macros/src/mcp_tool.rs` — `#[mcp_tool]` expansion (mirror for `#[mcp_prompt]`)
- `pmcp-macros/src/mcp_server.rs` — `#[mcp_server]` expansion (extend for prompts)
- `pmcp-macros/src/mcp_common.rs` — Shared helpers (reuse for prompt param classification)
- `src/server/state.rs` — `State<T>` extractor (reuse for prompts)
- `src/server/mod.rs` — `McpServer` trait (extend for prompts)
- `docs/design/mcp-tool-macro-design.md` — Design RFC (reference for consistency)

### Current prompt infrastructure
- `src/server/mod.rs` line 213 — `PromptHandler` trait (`handle(args: HashMap<String, String>, extra)`)
- `src/server/builder.rs` — `ServerBuilder::prompt()` registration
- `src/server/simple_prompt.rs` — `SimplePrompt` (similar to `SimpleTool`)
- `src/types/prompts.rs` — `PromptInfo`, `PromptArgument`, `GetPromptResult`, `PromptMessage`
- `examples/17_completable_prompts.rs` — Current boilerplate (the pain to eliminate)
- `examples/06_server_prompts.rs` — Basic prompt example

### TypedTool reference (pattern source)
- `src/server/typed_tool.rs` — `TypedTool`, `TypedToolWithOutput` (the pattern TypedPrompt follows)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `mcp_common.rs` — `classify_param()`, `type_name_matches()`, schema generation helpers, `output_schema_tokens()`, `add_async_trait_bounds()` — all reusable for prompt macros
- `State<T>` — already exists, no changes needed
- `McpToolArgs` / `McpToolAnnotations` darling structs — pattern to follow for `McpPromptArgs`
- `mcp_tool.rs::generate_tool_info_code()` — pattern for `generate_prompt_info_code()`

### Established Patterns
- `PromptHandler` trait: `handle(args: HashMap<String, String>, extra) -> Result<GetPromptResult>`
- `TypedTool` pattern: closure taking (typed_args, extra) → Box::pin → Result
- `#[mcp_tool]` pattern: generates struct + ToolHandler impl + constructor function
- Builder: `.prompt("name", handler)` — TypedPrompt must be compatible

### Integration Points
- `pmcp-macros/src/lib.rs` — Add `#[mcp_prompt]` proc macro entry point
- `pmcp-macros/src/mcp_server.rs` — Extend to collect `#[mcp_prompt]` methods
- `src/server/mod.rs` — Add `TypedPrompt`, rename `register_tools` to `register`
- `src/lib.rs` — Re-export `#[mcp_prompt]` alongside `#[mcp_tool]`

</code_context>

<specifics>
## Specific Ideas

- MCP server team reported "2 Arc clones remain for prompts" in their arithmetics server — this phase eliminates those
- The `HashMap<String, String>` → typed struct conversion must handle MCP's string-only prompt arguments gracefully (numbers, bools, enums in string form)
- PromptArgument has `required: Option<bool>` — derive from whether the struct field is `Option<T>` or not

</specifics>

<deferred>
## Deferred Ideas

- `#[mcp_resource]` macro — future phase
- Prompt template interpolation (filling placeholders in prompt text) — different concern
- Prompt chaining / composition — separate from typed arguments
- WASM target support for prompt macros

</deferred>

---

*Phase: 59-typed-prompt-auto-deserialization*
*Context gathered: 2026-03-21*
