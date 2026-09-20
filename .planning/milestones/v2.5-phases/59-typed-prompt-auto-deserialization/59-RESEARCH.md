# Phase 59: TypedPrompt with Auto-Deserialization - Research

**Researched:** 2026-03-21
**Domain:** Rust proc-macro codegen, serde deserialization, MCP prompt protocol
**Confidence:** HIGH

## Summary

This phase adds `TypedPrompt` (a runtime generic type) and `#[mcp_prompt]` (a proc-macro attribute) to mirror the existing `TypedTool`/`#[mcp_tool]` pattern for prompts. The core technical challenge is converting MCP's `HashMap<String, String>` prompt arguments into a typed Rust struct, and deriving `PromptArgument` metadata (name, description, required) from the struct's `JsonSchema`.

Phase 58 shipped a complete pattern in `pmcp-macros/src/mcp_tool.rs`, `mcp_server.rs`, and `mcp_common.rs` that is directly reusable. The `#[mcp_prompt]` macro mirrors `#[mcp_tool]` with three key differences: (1) the handler signature takes `HashMap<String, String>` instead of `serde_json::Value`, (2) it returns `GetPromptResult` instead of `Value`, and (3) metadata produces `PromptInfo` instead of `ToolInfo`. The `#[mcp_server]` macro must be extended to collect `#[mcp_prompt]` methods alongside `#[mcp_tool]` methods.

**Primary recommendation:** Follow the Phase 58 pattern exactly -- create `mcp_prompt.rs` in pmcp-macros mirroring `mcp_tool.rs`, extend `mcp_server.rs` to detect `#[mcp_prompt]` attrs, and rename `register_tools()` to `register()`. The `TypedPrompt` runtime type goes in `src/server/typed_prompt.rs` mirroring `typed_tool.rs`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: Mirror the `#[mcp_tool]` DX pattern exactly -- developers who learned tools should recognize prompts instantly
- D-02: Description mandatory at compile time -- LLMs need prompt descriptions
- D-03: `extra` (RequestHandlerExtra) is opt-in -- only declare in signature if needed
- D-04: Name defaults to function name, override with `name = "custom"`
- D-05: `TypedPrompt<T, F>` where T: `Deserialize + JsonSchema` -- deserializes `HashMap<String, String>` into typed struct T
- D-06: Deserialization strategy: convert `HashMap<String, String>` to `serde_json::Value` (object with string values), then `serde_json::from_value::<T>()` -- handles string-to-type coercion via serde
- D-07: Generates `PromptInfo` with argument schema from T's JsonSchema implementation -- clients can discover argument names, types, and descriptions
- D-08: Registration: `.prompt("name", TypedPrompt::new("name", handler).with_description("..."))`
- D-09: Use `#[mcp_prompt]` (not `#[prompt]`) -- consistent with `#[mcp_tool]` naming convention; existing `#[prompt]` stub is a no-op passthrough
- D-10: Standalone function pattern: `#[mcp_prompt(description = "...")]` on `async fn` returning `Result<GetPromptResult>`
- D-11: State injection via `State<T>` -- same pattern as `#[mcp_tool]`
- D-12: Generates a struct implementing `PromptHandler` with `handle()` + `metadata()`
- D-13: Constructor function: `fn prompt_name() -> PromptNamePrompt` for ergonomic registration
- D-14: `#[mcp_server]` collects both `#[mcp_tool]` AND `#[mcp_prompt]` methods from the same impl block
- D-15: `McpServer::register_tools()` renamed to `McpServer::register()` -- registers tools AND prompts
- D-16: Prompts use `&self` for state access in impl blocks -- identical to tools
- D-17: Generate `PromptArgument` entries from T's JsonSchema -- each struct field becomes a prompt argument with name, description (from doc comment or serde attr), and required flag (non-Option fields are required)
- D-18: `#[serde(default)]` fields are treated as optional arguments

### Claude's Discretion
- Internal codegen details for HashMap<String, String> to typed struct conversion
- Error messages for deserialization failures (argument name, expected type)
- Whether to support sync prompts (fn vs async fn) -- likely yes for consistency
- How PromptArgument descriptions are extracted from JsonSchema
- Test strategy mirroring Phase 58 (integration tests, compile-fail tests)
- Re-export `#[mcp_prompt]` from pmcp crate alongside `#[mcp_tool]`

### Deferred Ideas (OUT OF SCOPE)
- `#[mcp_resource]` macro -- future phase
- Prompt template interpolation (filling placeholders in prompt text) -- different concern
- Prompt chaining / composition -- separate from typed arguments
- WASM target support for prompt macros
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TYPED-PROMPT | Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Arguments deserialize from `HashMap<String, String>` into typed struct via JsonSchema + serde. | TypedPrompt runtime type pattern from TypedTool; HashMap-to-Value-to-struct conversion via serde_json::from_value; PromptArgument generation from JsonSchema properties |
| PROMPT-SCHEMA | `#[mcp_prompt]` macro mirroring `#[mcp_tool]` and `#[mcp_server]` extension to collect prompts alongside tools | mcp_tool.rs pattern reuse for mcp_prompt.rs; mcp_server.rs extension for dual #[mcp_tool]/#[mcp_prompt] collection; register_tools() renamed to register() |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 | Deserialize HashMap<String,String> to typed structs | Already in workspace, `from_value` handles string-to-type coercion |
| serde_json | 1.0 | HashMap-to-Value conversion, `from_value::<T>()` | Already in workspace, D-06 specifies this approach |
| schemars | 1.0 | JsonSchema derive for struct fields -> PromptArgument metadata | Already in workspace via `schema-generation` feature |
| syn | 2.0 | Proc-macro parsing (full, extra-traits, visit-mut) | Already in pmcp-macros deps |
| quote | 1.0 | Token stream generation | Already in pmcp-macros deps |
| darling | 0.23 | Attribute parsing for `#[mcp_prompt(...)]` | Already in pmcp-macros deps |
| heck | 0.5 | Case conversion (fn_name -> PascalCase struct name) | Already in pmcp-macros deps |
| async-trait | 0.1 | `PromptHandler` trait is already `#[async_trait]` | Already in workspace |

No new dependencies needed. Everything required is already in the workspace.

## Architecture Patterns

### Recommended Project Structure (new/modified files)
```
pmcp-macros/src/
  lib.rs                    # ADD: #[mcp_prompt] entry point
  mcp_prompt.rs             # NEW: #[mcp_prompt] expansion (mirrors mcp_tool.rs)
  mcp_server.rs             # MODIFY: collect #[mcp_prompt] alongside #[mcp_tool]
  mcp_common.rs             # REUSE: classify_param, schema helpers (no changes needed)

src/server/
  mod.rs                    # MODIFY: re-export TypedPrompt; rename register_tools -> register on McpServer
  typed_prompt.rs           # NEW: TypedPrompt<T, F> runtime type (mirrors typed_tool.rs)

src/lib.rs                  # MODIFY: re-export mcp_prompt from pmcp_macros

pmcp-macros/tests/
  mcp_prompt_tests.rs       # NEW: integration tests (mirrors mcp_tool_tests.rs)
  ui/mcp_prompt_*.rs        # NEW: compile-fail tests
```

### Pattern 1: HashMap<String, String> to Typed Struct (D-06)

**What:** The core deserialization path for prompt arguments. MCP protocol sends prompt arguments as `HashMap<String, String>`. TypedPrompt must convert these to a typed struct T.

**When to use:** Every TypedPrompt invocation and every `#[mcp_prompt]` handler.

**Example:**
```rust
// Source: D-06 from CONTEXT.md
fn convert_args<T: DeserializeOwned>(args: HashMap<String, String>) -> Result<T> {
    // Step 1: Convert HashMap<String, String> to serde_json::Value
    // This creates a JSON object with all values as strings
    let value = serde_json::Value::Object(
        args.into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect()
    );
    // Step 2: Deserialize from Value to T
    // serde_json::from_value handles string-to-type coercion
    // BUT: numbers/bools as strings won't auto-coerce with default deserialize
    serde_json::from_value::<T>(value)
        .map_err(|e| Error::invalid_params(format!("Invalid prompt arguments: {}", e)))
}
```

**Critical pitfall:** `serde_json::from_value` does NOT auto-coerce string `"42"` to integer `42`. Since MCP sends ALL prompt arguments as strings, structs with `i32`, `f64`, `bool` fields will fail deserialization. The macro must generate a custom deserializer or use `serde_aux`/manual `#[serde(deserialize_with)]`. See Pitfall 1 below.

### Pattern 2: PromptArgument Generation from JsonSchema (D-17)

**What:** Extract field names, descriptions, and required status from a struct's JsonSchema to build `Vec<PromptArgument>`.

**When to use:** In `TypedPrompt::new()` and in generated `metadata()` methods.

**Example:**
```rust
// Source: Schema introspection pattern
fn generate_prompt_arguments<T: JsonSchema>() -> Vec<PromptArgument> {
    let schema = schemars::schema_for!(T);
    let json_schema = serde_json::to_value(&schema).unwrap();

    let properties = json_schema.get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    let required_fields: Vec<String> = json_schema.get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    properties.into_iter().map(|(name, prop)| {
        let description = prop.get("description")
            .and_then(|d| d.as_str())
            .map(String::from);
        let is_required = required_fields.contains(&name);

        let mut arg = PromptArgument::new(&name);
        if let Some(desc) = description {
            arg = arg.with_description(desc);
        }
        if is_required {
            arg = arg.required();
        }
        arg
    }).collect()
}
```

**Key insight:** schemars puts doc comments into the `"description"` field of each property. `#[serde(default)]` fields are NOT in the `"required"` array. `Option<T>` fields are also NOT in the `"required"` array. This is exactly the behavior D-17 and D-18 specify.

### Pattern 3: Struct Naming Convention (D-13)

**What:** Generate `{PascalCase}Prompt` struct from function name, matching the tool pattern.

```rust
// Input: #[mcp_prompt(description = "...")] async fn code_review(...)
// Output struct: CodeReviewPrompt
// Constructor: fn code_review() -> CodeReviewPrompt
let struct_name = format_ident!("{}Prompt", fn_name_str.to_upper_camel_case());
```

### Pattern 4: #[mcp_server] Extension for Dual Collection (D-14)

**What:** Extend `mcp_server.rs` to detect both `#[mcp_tool]` and `#[mcp_prompt]` attributes, generate per-tool/per-prompt handler structs, and register all in `register()`.

```rust
// Generated McpServer impl
impl McpServer for MyServer {
    fn register(self, mut builder: ServerBuilder) -> ServerBuilder {
        let shared = Arc::new(self);
        // Tool registrations
        builder = builder.tool("query", QueryToolHandler { server: shared.clone() });
        // Prompt registrations
        builder = builder.prompt("code_review", CodeReviewPromptHandler { server: shared.clone() });
        builder
    }
}
```

### Anti-Patterns to Avoid
- **Separate register_tools / register_prompts:** D-15 says single `register()` method for both. Don't split registration.
- **Custom serde Deserialize for all prompts:** Don't hand-roll deserializers. Use the HashMap->Value->from_value pattern (D-06). Address string coercion as a documented concern.
- **Requiring JsonSchema on GetPromptResult:** Prompts return `GetPromptResult` which is already a known type. Don't generate output schemas for prompts (unlike tools, prompts have fixed output structure).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON schema generation | Manual property enumeration | `schemars::schema_for!(T)` + normalize | Handles nested types, descriptions, required fields automatically |
| Proc-macro attribute parsing | Manual token parsing | `darling::FromMeta` | Type-safe, error messages, nested attributes |
| PascalCase conversion | String manipulation | `heck::ToUpperCamelCase` | Unicode-aware, battle-tested |
| Parameter classification | Custom type matching | `mcp_common::classify_param()` | Already handles State<T>, RequestHandlerExtra, &self |
| Schema normalization | Inline $ref manually | `schema_utils::normalize_schema()` | Already handles depth limits, metadata removal |

**Key insight:** Phase 58 already built the hard infrastructure. Phase 59 reuses `mcp_common.rs` wholesale -- `classify_param`, `type_name_matches`, `extract_state_inner`, `generate_input_schema_code`, `add_async_trait_bounds` all work unchanged for prompts.

## Common Pitfalls

### Pitfall 1: String-Only Arguments Won't Auto-Coerce
**What goes wrong:** MCP sends all prompt arguments as `HashMap<String, String>`. If a user defines `struct Args { count: i32 }`, `serde_json::from_value` receives `{"count": "5"}` -- a JSON string, not a JSON number. Standard serde will reject this with `"invalid type: string, expected i32"`.
**Why it happens:** D-06 converts HashMap values to `serde_json::Value::String`, but serde_json's default `Deserialize` for i32 expects a JSON number.
**How to avoid:** Two approaches:
1. **Document the limitation:** Prompt argument structs should use `String` fields and parse manually. Simple and honest.
2. **Custom conversion:** Before calling `from_value`, inspect the schema to determine which fields need numeric/boolean conversion, and convert the Value strings to the appropriate JSON types. This is the approach the PromptArgumentType::parse_value already implements.
**Recommendation:** Use option 2 at the TypedPrompt/macro level: generate code that reads the schema, identifies non-string fields, and pre-converts the Value map entries using the existing `PromptArgumentType::parse_value` logic before calling `from_value`. This makes `struct Args { count: i32, verbose: bool }` "just work" with prompt arguments `{"count": "42", "verbose": "true"}`.
**Warning signs:** Tests with non-string argument types failing with serde errors.

### Pitfall 2: register_tools() Rename Backward Compatibility
**What goes wrong:** D-15 renames `register_tools()` to `register()` on the `McpServer` trait. This is a breaking change for any code implementing McpServer manually.
**Why it happens:** The trait is public, and renaming a trait method breaks all implementors.
**How to avoid:** Since McpServer is generated by `#[mcp_server]`, manual implementations are rare. Options: (1) rename and accept the breakage (minor version bump), (2) provide both methods with deprecated alias. Given the trait was just introduced in Phase 58, option 1 is acceptable with a note in CHANGELOG.
**Warning signs:** Existing tests referencing `register_tools()` will fail.

### Pitfall 3: #[prompt] Stub Collision
**What goes wrong:** `pmcp-macros/src/lib.rs` already has a `#[prompt]` attribute macro (no-op passthrough, line 240). Adding `#[mcp_prompt]` is safe (different name per D-09), but if the old `#[prompt]` is not updated, users might confuse the two.
**Why it happens:** The `#[prompt]` stub was added as a placeholder. It's never been functional.
**How to avoid:** Leave `#[prompt]` as-is (no-op passthrough). D-09 explicitly chose `#[mcp_prompt]` to avoid collision. Document that `#[prompt]` is deprecated/no-op in its doc comment.
**Warning signs:** Users using `#[prompt]` and wondering why nothing happens.

### Pitfall 4: mcp_server Must Strip Both Attribute Types
**What goes wrong:** `strip_mcp_tool_attrs()` in `mcp_server.rs` only strips `#[mcp_tool]`. After extending to collect `#[mcp_prompt]`, the stripping function must also remove `#[mcp_prompt]` attributes from the original impl block.
**Why it happens:** Oversight -- the strip function is currently hardcoded for `mcp_tool`.
**How to avoid:** Rename to `strip_mcp_attrs()` and strip both `mcp_tool` and `mcp_prompt`.
**Warning signs:** Compile errors about `mcp_prompt` not being a valid attribute on methods in an already-processed impl block.

### Pitfall 5: GetPromptResult Return Type Detection
**What goes wrong:** In `#[mcp_tool]`, the macro extracts the `Ok` type from `Result<T>` to generate `outputSchema`. For prompts, the return type is always `Result<GetPromptResult>` -- there is no equivalent "output schema" concept. But the macro code might try to extract the inner type and feed it to schema generation.
**Why it happens:** Copy-paste from mcp_tool.rs without removing the output schema path.
**How to avoid:** In `mcp_prompt.rs`, skip all output schema generation. Prompt metadata is `PromptInfo` (with arguments), not `ToolInfo` (with input/output schemas).
**Warning signs:** Compilation failures mentioning `JsonSchema` not implemented for `GetPromptResult`.

### Pitfall 6: PromptHandler Handler Struct Must Use Arc
**What goes wrong:** In `#[mcp_server]` impl block prompts, the handler struct must hold `Arc<ServerType>` (same as tool handlers). If you forget the Arc wrapper, &self access won't work across async boundaries.
**Why it happens:** Prompts are registered as `Arc<dyn PromptHandler>` in ServerBuilder. The generated handler struct needs to be Send + Sync.
**How to avoid:** Mirror the tool handler struct pattern exactly: `struct CodeReviewPromptHandler { server: Arc<ServerType> }`.

## Code Examples

Verified patterns from existing codebase:

### Current Boilerplate (the pain to eliminate)
```rust
// Source: examples/17_completable_prompts.rs
struct DatabaseQueryPrompt;

#[async_trait]
impl PromptHandler for DatabaseQueryPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> PmcpResult<GetPromptResult> {
        let database = args.get("database").unwrap_or(&"main".to_string()).clone();
        let table = args.get("table").unwrap_or(&"users".to_string()).clone();
        // ... manual extraction for every argument
    }
}
```

### Target DX: Standalone #[mcp_prompt]
```rust
// AFTER: What this phase enables
#[derive(Debug, Deserialize, JsonSchema)]
struct DatabaseQueryArgs {
    /// Target database name
    database: String,
    /// Table to query
    table: String,
    /// Query operation type
    #[serde(default = "default_operation")]
    operation: String,
}

#[mcp_prompt(description = "Generate database queries")]
async fn database_query(args: DatabaseQueryArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![
            PromptMessage::system(Content::text(
                format!("Execute on {}.{}", args.database, args.table)
            )),
            PromptMessage::user(Content::text(format!("Operation: {}", args.operation))),
        ],
        Some("Database query prompt".to_string()),
    ))
}

// Registration: server_builder.prompt("database_query", database_query())
```

### Target DX: #[mcp_server] with Mixed Tools and Prompts
```rust
// AFTER: Tools and prompts in same impl block
#[mcp_server]
impl MyServer {
    #[mcp_tool(description = "Execute query")]
    async fn execute(&self, args: ExecuteArgs) -> pmcp::Result<serde_json::Value> {
        // ...
    }

    #[mcp_prompt(description = "Generate a query prompt")]
    async fn query_builder(&self, args: QueryBuilderArgs) -> pmcp::Result<GetPromptResult> {
        // ...
    }
}

// Registration: builder.mcp_server(my_server)  -- registers BOTH tools and prompts
```

### TypedPrompt Runtime Type (mirrors TypedTool)
```rust
// Source: pattern from src/server/typed_tool.rs adapted for prompts
pub struct TypedPrompt<T, F>
where
    T: DeserializeOwned + Send + Sync + 'static,
    F: Fn(T, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send + Sync,
{
    name: String,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
    handler: F,
    _phantom: PhantomData<T>,
}

#[async_trait]
impl<T, F> PromptHandler for TypedPrompt<T, F>
where /* bounds */
{
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Convert HashMap<String, String> to Value (D-06)
        let value = serde_json::Value::Object(
            args.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect()
        );
        let typed_args: T = serde_json::from_value(value)
            .map_err(|e| Error::invalid_params(
                format!("Invalid arguments for prompt '{}': {}", self.name, e)
            ))?;
        (self.handler)(typed_args, extra).await
    }

    fn metadata(&self) -> Option<PromptInfo> {
        let mut info = PromptInfo::new(&self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if !self.arguments.is_empty() {
            info = info.with_arguments(self.arguments.clone());
        }
        Some(info)
    }
}
```

### McpPromptArgs Darling Struct (mirrors McpToolArgs)
```rust
// Source: pattern from mcp_tool.rs
#[derive(Debug, FromMeta)]
pub struct McpPromptArgs {
    /// Prompt description (mandatory per D-02).
    pub(crate) description: String,
    /// Override prompt name (defaults to function name per D-04).
    #[darling(default)]
    pub(crate) name: Option<String>,
}
```

Note: Prompts have NO annotations equivalent (no destructive/read_only/etc), NO ui resource URI, and NO output schema. The McpPromptArgs struct is simpler than McpToolArgs.

### Generated PromptHandler impl (what #[mcp_prompt] expands to)
```rust
// For: #[mcp_prompt(description = "Generate queries")]
//      async fn database_query(args: DbArgs) -> Result<GetPromptResult> { ... }

// Internal implementation function (renamed to avoid constructor conflict)
async fn __database_query_impl(args: DbArgs) -> Result<GetPromptResult> { ... }

#[derive(Clone)]
pub struct DatabaseQueryPrompt {
    // state: Option<Arc<T>> if State<T> param present
}

#[pmcp::async_trait]
impl pmcp::PromptHandler for DatabaseQueryPrompt {
    async fn handle(
        &self,
        args: std::collections::HashMap<String, String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<pmcp::types::GetPromptResult> {
        // Convert HashMap to Value
        let value = serde_json::Value::Object(
            args.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect()
        );
        // Deserialize typed args
        let typed_args: DbArgs = serde_json::from_value(value)
            .map_err(|e| pmcp::Error::invalid_params(
                format!("Invalid arguments for prompt 'database_query': {}", e)
            ))?;
        // Call implementation
        __database_query_impl(typed_args).await
    }

    fn metadata(&self) -> Option<pmcp::types::PromptInfo> {
        // Generate PromptArgument list from JsonSchema
        let schema = schemars::schema_for!(DbArgs);
        let json_schema = serde_json::to_value(&schema).unwrap();
        let arguments = extract_prompt_arguments(&json_schema);
        let mut info = pmcp::types::PromptInfo::new("database_query")
            .with_description("Generate queries");
        if !arguments.is_empty() {
            info = info.with_arguments(arguments);
        }
        Some(info)
    }
}

pub fn database_query() -> DatabaseQueryPrompt {
    DatabaseQueryPrompt {}
}
```

### PromptArgument Extraction from Schema (generated inline)
```rust
// This code is generated by the macro in the metadata() method
fn extract_prompt_arguments(schema: &serde_json::Value) -> Vec<pmcp::types::PromptArgument> {
    let properties = schema.get("properties")
        .and_then(|p| p.as_object());
    let required = schema.get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let Some(props) = properties else { return vec![]; };

    props.iter().map(|(name, prop)| {
        let mut arg = pmcp::types::PromptArgument::new(name);
        if let Some(desc) = prop.get("description").and_then(|d| d.as_str()) {
            arg = arg.with_description(desc);
        }
        if required.contains(&name.as_str()) {
            arg = arg.required();
        }
        arg
    }).collect()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual `HashMap::get("x").ok_or()?.parse()?` | `TypedPrompt` auto-deserializes | Phase 59 (this phase) | Eliminates 5-10 lines per prompt argument |
| Separate `SimplePrompt::with_argument()` calls | Arguments derived from `JsonSchema` struct | Phase 59 (this phase) | Single source of truth for prompt schema |
| `#[mcp_server]` only collects `#[mcp_tool]` | Collects both `#[mcp_tool]` and `#[mcp_prompt]` | Phase 59 (this phase) | Unified registration with `register()` |
| `register_tools()` on McpServer | `register()` on McpServer | Phase 59 (this phase) | Reflects that it registers both tools and prompts |

**Deprecated/outdated:**
- `#[prompt]` attribute macro: no-op stub since initial release, replaced by `#[mcp_prompt]` in this phase
- `SimplePrompt::with_argument()` chain: still works but `TypedPrompt` is the preferred pattern for new code

## Key Technical Decisions (Claude's Discretion Resolution)

### Sync Prompts: YES
Support sync `fn` (not just `async fn`) in `#[mcp_prompt]`, matching the tool pattern exactly. The generated `PromptHandler` impl wraps sync calls in the async trait method. Phase 58 already solved this for tools.

### PromptArgument Description Extraction
Use `schemars`' `"description"` field from each property in the generated JSON schema. schemars populates this from `/// doc comments` on struct fields when using `#[derive(JsonSchema)]`. If no doc comment, the description will be `None`. This is the standard schemars behavior and requires no special handling.

### Error Messages
Format: `"Invalid arguments for prompt '{name}': {serde_error}"`. Mirror the tool pattern exactly (`"Invalid arguments for tool '{name}': {serde_error}"`).

### Re-exports
Add `pub use pmcp_macros::mcp_prompt;` in `src/lib.rs` alongside the existing `mcp_tool` and `mcp_server` re-exports.

### String Coercion Strategy
For the initial implementation, keep it simple: convert HashMap values to `Value::String` and rely on serde's default deserializer. This means prompt argument structs should primarily use `String` fields. Document this as a known limitation. A future enhancement could pre-process the Value map using JsonSchema type information to convert string values to their proper JSON types before calling `from_value`.

**Rationale:** This matches MCP's design intent -- prompt arguments ARE strings. The MCP spec defines `GetPromptRequest.arguments` as `HashMap<String, String>`. Expecting non-string types in a prompt argument struct is an edge case that users can handle with `String` + manual parsing if needed.

## Open Questions

1. **String-to-number/bool coercion at TypedPrompt level**
   - What we know: D-06 says use `from_value`, but Value::String("42") won't deserialize as i32
   - What's unclear: Whether to add pre-conversion using schema type info, or document String-only limitation
   - Recommendation: Start with String-only, document the limitation, add coercion in a follow-up if users request it. This keeps Phase 59 focused and shippable.

2. **Backward compatibility of register_tools() rename**
   - What we know: McpServer trait was introduced in Phase 58, so very few users have implemented it manually
   - What's unclear: Whether any external code depends on `register_tools()` method name
   - Recommendation: Rename to `register()` per D-15. Add a deprecated `register_tools()` default method that calls `register()` for one version cycle.

## Sources

### Primary (HIGH confidence)
- `pmcp-macros/src/mcp_tool.rs` -- Full #[mcp_tool] expansion pattern to mirror
- `pmcp-macros/src/mcp_server.rs` -- #[mcp_server] expansion to extend
- `pmcp-macros/src/mcp_common.rs` -- Shared helpers to reuse
- `src/server/typed_tool.rs` -- TypedTool<T, F> pattern for TypedPrompt
- `src/server/simple_prompt.rs` -- SimplePrompt/SyncPrompt (current prompt implementations)
- `src/types/prompts.rs` -- PromptInfo, PromptArgument, GetPromptResult types
- `src/server/mod.rs` -- PromptHandler trait, McpServer trait, ServerBuilder
- `examples/06_server_prompts.rs` -- Current prompt boilerplate (the pain)
- `examples/17_completable_prompts.rs` -- Current prompt boilerplate (the pain)
- `pmcp-macros/tests/mcp_tool_tests.rs` -- Test pattern to mirror
- `pmcp-macros/tests/mcp_server_tests.rs` -- Test pattern to mirror

### Secondary (MEDIUM confidence)
- `docs/design/mcp-tool-macro-design.md` -- RFC for tool macro design decisions
- schemars documentation on `description` field population from doc comments

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies already in workspace, no new additions
- Architecture: HIGH - Direct mirror of Phase 58 pattern, verified by reading all source files
- Pitfalls: HIGH - Identified from direct code analysis, especially the string coercion issue
- Code examples: HIGH - Derived from actual codebase patterns with verified types and APIs

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable codebase, no external dependencies changing)
