# Phase 58: #[mcp_tool] Proc Macro - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Expand pmcp-macros crate with `#[mcp_tool]` attribute macro that eliminates `Box::pin(async move {})` boilerplate on tool definitions. Accepts `async fn(input: T, extra: RequestHandlerExtra) -> Result<Output>` directly. Handles Arc state injection for composition scenarios (eliminates the foundation cloning ceremony). Auto-derives input/output JSON schema from types.

</domain>

<decisions>
## Implementation Decisions

### Macro naming
- **D-01:** Use `#[mcp_tool]` (not `#[tool]`) to distinguish MCP protocol tools from generic agent tool patterns
- **D-02:** The existing `#[tool]` macro remains as-is for backward compatibility; `#[mcp_tool]` is the recommended path forward
- **D-03:** Add `#[mcp_server]` as the impl-block companion macro (replaces the incomplete `#[tool_router]`)

### DX-first design philosophy
- **D-04:** Target audience is developers who find Rust intimidating — minimize visible Rust-isms (Box::pin, Arc, move closures)
- **D-05:** Description is mandatory at compile time — enforces good MCP practice (LLMs need descriptions)
- **D-06:** Tool name defaults to function name — convention over configuration, override with `name = "custom"`
- **D-07:** `extra` (RequestHandlerExtra) is opt-in — only declare in function signature if needed

### State injection
- **D-08:** `State<T>` extractor pattern for standalone `#[mcp_tool]` functions — familiar from Axum/Actix web frameworks
- **D-09:** `&self` access for `#[mcp_server]` impl-block tools — natural Rust method pattern
- **D-10:** Both patterns supported — standalone tools for simple cases, impl-block for multi-tool servers with shared state

### Parameter signature
- **D-11:** Macro inspects parameters by type, not position: first JsonSchema+Deserialize struct = args, `State<T>` = shared state, `RequestHandlerExtra` = extra
- **D-12:** No-arg tools are valid (e.g., health check, version info)
- **D-13:** Sync tools via `#[mcp_tool(sync)]` flag — async is the default

### Output typing
- **D-14:** If return type is `Result<T>` where T: Serialize + JsonSchema, auto-generate `outputSchema` in ToolInfo
- **D-15:** `Result<Value>` remains valid for untyped output (no outputSchema generated)
- **D-16:** Typed output encouraged by documentation and examples — enables server-to-server composition

### Registration ergonomics
- **D-17:** Each `#[mcp_tool]` function generates a constructor `fn tool_name() -> ToolNameTool` for registration
- **D-18:** Registration: `server_builder.tool("name", tool_name())` — no name repetition in macro, one-liner at builder
- **D-19:** State provided at registration: `.tool("name", tool_name().with_state(shared_db))`
- **D-20:** `#[mcp_server]` generates `.mcp_server(instance)` for bulk registration of all impl-block tools

### Generated code
- **D-21:** Macro generates a struct implementing `ToolHandler` trait — compatible with existing builder
- **D-22:** Input schema via `schemars::schema_for!` — same as TypedTool/TypedToolWithOutput today
- **D-23:** MCP standard annotations supported: `annotations(read_only, destructive, idempotent, open_world)`

### Team feedback — P1 (must-have)
- **D-24:** `ui = "uri"` attribute on `#[mcp_tool]` for widget attachment — e.g., `#[mcp_tool(description = "Add", ui = CALCULATOR_WIDGET_URI)]`. Without this, MCP Apps servers cannot migrate to the macro.
- **D-25:** Generic impl blocks on `#[mcp_server]` must work — `#[mcp_server] impl<F: FoundationClient + 'static> Server<F>` is the natural pattern for composition servers using generic foundation clients. Proc macro must preserve type parameters and trait bounds.

### Team feedback — P2/P3 (should-have)
- **D-26:** Auto-detect sync vs async from `fn` vs `async fn` — drop the `sync` flag. If the method is `fn`, it's sync. If `async fn`, it's async. Redundant flags are a DX anti-pattern.
- **D-28:** Document the "thin wrapper" migration pattern for existing standalone async functions — annotating existing functions directly isn't supported; wrap them in `#[mcp_server]` impl methods. This is the expected path, not a limitation.

### Claude's Discretion
- Internal codegen details (how Box::pin is hidden, how State<T> is resolved)
- Error message quality for macro misuse (wrong parameter types, missing derives)
- Whether to generate `#[cfg(feature = "schema-generation")]` guards or always include schemas
- Exact trait bounds and lifetime handling for generic impl blocks
- Test strategy for the macro itself (trybuild, compile-fail tests)
- Whether `#[mcp_server]` generates a single `tools()` method or per-tool metadata
- How `ui` attribute value gets wired to `.with_ui()` in generated code

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design document
- `docs/design/mcp-tool-macro-design.md` — Full RFC with before/after examples, API surface, parameter rules, migration path, and team feedback questions

### Existing macro infrastructure
- `pmcp-macros/src/lib.rs` — Current macro entry points (#[tool], #[tool_router], #[prompt], #[resource])
- `pmcp-macros/src/tool.rs` — Existing #[tool] implementation (ToolArgs, schema generation, handler codegen)
- `pmcp-macros/src/tool_router.rs` — Incomplete #[tool_router] (reference for #[mcp_server] design)
- `pmcp-macros/src/utils.rs` — Shared utilities (to_pascal_case, extract_option_inner, generate_schema_for_type)
- `pmcp-macros/Cargo.toml` — Dependencies: syn 2.0, quote, proc-macro2, darling 0.23

### Tool handler trait and typed tools
- `src/server/traits.rs` — ToolHandler trait definition
- `src/server/typed_tool.rs` — TypedTool, TypedToolWithOutput, TypedSyncTool implementations (the code macro should generate)
- `src/server/builder.rs` — ServerCoreBuilder::tool() registration (line 146)

### RequestHandlerExtra
- `src/server/cancellation.rs` — Full native RequestHandlerExtra (cancellation, progress, auth, metadata)
- `src/shared/cancellation.rs` — WASM RequestHandlerExtra (simplified)

### Current usage examples
- `examples/32_typed_tools.rs` — TypedTool and TypedSyncTool with Box::pin pattern (the boilerplate to eliminate)
- `examples/02_server_basic.rs` — Manual ToolHandler impl pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pmcp-macros/src/tool.rs` — ToolArgs parsing, schema generation, handler wrapping logic — adapt for #[mcp_tool]
- `pmcp-macros/src/utils.rs` — to_pascal_case, extract_option_inner, generate_schema_for_type
- `src/server/typed_tool.rs` — TypedTool/TypedToolWithOutput impls that the macro should generate code against
- `darling` crate already in deps — use for attribute parsing

### Established Patterns
- `#[derive(Deserialize, JsonSchema)]` on input types — macro relies on this, doesn't replace it
- `schemars::schema_for!` for schema generation — cached at registration time
- `ToolHandler` trait with `handle()` and `metadata()` — target trait for generated code
- Builder pattern: `.tool("name", handler)` — generated tools must be compatible

### Integration Points
- `pmcp-macros/src/lib.rs` — Add `#[mcp_tool]` and `#[mcp_server]` proc macro entry points
- `src/server/mod.rs` — Add McpServer trait and `.mcp_server()` method on ServerBuilder
- `src/lib.rs` — Re-export `State<T>` type and `McpServer` trait

</code_context>

<specifics>
## Specific Ideas

- KPI is developer experience — minimize DRY violations, potential errors, and Rust-specific complexities
- Target developers who find Rust scary — the macro should make MCP tool definition feel approachable
- Opinionated about best practices: Streamable HTTP transport, prompts not just tools, stateless design, Tasks for long-running processes, strong input validation
- Design document (`docs/design/mcp-tool-macro-design.md`) created for team review — includes before/after comparisons, all API patterns, and 5 feedback questions

</specifics>

<deferred>
## Deferred Ideas

- `#[mcp_prompt]` macro — Phase 59 (TypedPrompt with auto-deserialization)
- `#[mcp_resource]` macro — future phase
- WASM target support for macros
- Auto-discovery / compile-time tool inventory
- Hot-reload of tool definitions
- **D-27:** Auto-validate inputs if args type implements `validator::Validate` — P3 nice-to-have, deferred to avoid scope creep. Can be added as feature-flagged enhancement in a future phase.

</deferred>

---

*Phase: 58-mcp-tool-proc-macro*
*Context gathered: 2026-03-21*
