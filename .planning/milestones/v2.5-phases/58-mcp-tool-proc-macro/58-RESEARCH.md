# Phase 58: #[mcp_tool] Proc Macro - Research

**Researched:** 2026-03-21
**Domain:** Rust procedural macros, MCP tool handler code generation, State injection pattern
**Confidence:** HIGH

## Summary

This phase expands the existing `pmcp-macros` crate with two new attribute macros: `#[mcp_tool]` for individual tool function decoration, and `#[mcp_server]` for impl-block companion macro that collects tools and generates bulk registration. The existing macro infrastructure (`syn 2.0`, `quote 1.0`, `proc-macro2 1.0`, `darling 0.23`, `heck 0.5`) is already in place, along with established patterns for attribute parsing, schema generation codegen, and compile-fail tests via `trybuild`.

The core technical challenges are: (1) parameter type detection to distinguish args struct vs `State<T>` vs `RequestHandlerExtra` vs `&self`, (2) generating code that implements the `ToolHandler` trait (the `handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value>` + `metadata() -> Option<ToolInfo>` version from `src/server/mod.rs`), (3) handling both sync and async functions with auto-detection, and (4) preserving generic type parameters and trait bounds for `#[mcp_server]` impl blocks. The existing `#[tool]` macro and `#[tool_router]` provide working reference implementations to build upon, though both have limitations that `#[mcp_tool]` and `#[mcp_server]` are designed to resolve.

**Primary recommendation:** Build `#[mcp_tool]` as a new module (`pmcp-macros/src/mcp_tool.rs`) and `#[mcp_server]` as another (`pmcp-macros/src/mcp_server.rs`), reusing utilities from `utils.rs`. Generate code that targets the existing `ToolHandler` trait from `src/server/mod.rs` (not `traits.rs`), producing structs compatible with `ServerCoreBuilder::tool()`. Add a `State<T>` wrapper type to the main `pmcp` crate for the extractor pattern. Test with `trybuild` compile-fail tests and integration tests.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: Use `#[mcp_tool]` (not `#[tool]`) to distinguish MCP protocol tools from generic agent tool patterns
- D-02: The existing `#[tool]` macro remains as-is for backward compatibility; `#[mcp_tool]` is the recommended path forward
- D-03: Add `#[mcp_server]` as the impl-block companion macro (replaces the incomplete `#[tool_router]`)
- D-04: Target audience is developers who find Rust intimidating -- minimize visible Rust-isms (Box::pin, Arc, move closures)
- D-05: Description is mandatory at compile time -- enforces good MCP practice (LLMs need descriptions)
- D-06: Tool name defaults to function name -- convention over configuration, override with `name = "custom"`
- D-07: `extra` (RequestHandlerExtra) is opt-in -- only declare in function signature if needed
- D-08: `State<T>` extractor pattern for standalone `#[mcp_tool]` functions -- familiar from Axum/Actix web frameworks
- D-09: `&self` access for `#[mcp_server]` impl-block tools -- natural Rust method pattern
- D-10: Both patterns supported -- standalone tools for simple cases, impl-block for multi-tool servers with shared state
- D-11: Macro inspects parameters by type, not position: first JsonSchema+Deserialize struct = args, `State<T>` = shared state, `RequestHandlerExtra` = extra
- D-12: No-arg tools are valid (e.g., health check, version info)
- D-13: Sync tools via auto-detection from `fn` vs `async fn` (D-26 supersedes the `sync` flag)
- D-14: If return type is `Result<T>` where T: Serialize + JsonSchema, auto-generate `outputSchema` in ToolInfo
- D-15: `Result<Value>` remains valid for untyped output (no outputSchema generated)
- D-16: Typed output encouraged by documentation and examples
- D-17: Each `#[mcp_tool]` function generates a constructor `fn tool_name() -> ToolNameTool` for registration
- D-18: Registration: `server_builder.tool("name", tool_name())` -- no name repetition in macro, one-liner at builder
- D-19: State provided at registration: `.tool("name", tool_name().with_state(shared_db))`
- D-20: `#[mcp_server]` generates `.mcp_server(instance)` for bulk registration of all impl-block tools
- D-21: Macro generates a struct implementing `ToolHandler` trait -- compatible with existing builder
- D-22: Input schema via `schemars::schema_for!` -- same as TypedTool/TypedToolWithOutput today
- D-23: MCP standard annotations supported: `annotations(read_only, destructive, idempotent, open_world)`
- D-24: `ui = "uri"` attribute on `#[mcp_tool]` for widget attachment -- MCP Apps servers cannot migrate without this
- D-25: Generic impl blocks on `#[mcp_server]` must work -- `impl<F: FoundationClient + 'static> Server<F>` is the natural pattern
- D-26: Auto-detect sync vs async from `fn` vs `async fn` -- drop the `sync` flag
- D-27: Auto-validate inputs if the args type implements `validator::Validate` -- call `.validate()` before passing args to handler
- D-28: Document the "thin wrapper" migration pattern for existing standalone async functions

### Claude's Discretion
- Internal codegen details (how Box::pin is hidden, how State<T> is resolved)
- Error message quality for macro misuse (wrong parameter types, missing derives)
- Whether to generate `#[cfg(feature = "schema-generation")]` guards or always include schemas
- Exact trait bounds and lifetime handling for generic impl blocks
- Test strategy for the macro itself (trybuild, compile-fail tests)
- Whether `#[mcp_server]` generates a single `tools()` method or per-tool metadata
- How `ui` attribute value gets wired to `.with_ui()` in generated code
- Whether `validator` integration requires a feature flag or is always checked

### Deferred Ideas (OUT OF SCOPE)
- `#[mcp_prompt]` macro -- Phase 59 (TypedPrompt with auto-deserialization)
- `#[mcp_resource]` macro -- future phase
- WASM target support for macros
- Auto-discovery / compile-time tool inventory
- Hot-reload of tool definitions
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TOOL-MACRO | `#[mcp_tool]` attribute macro that eliminates Box::pin boilerplate, accepts async fn directly, generates ToolHandler impl with schema, supports annotations and ui attribute | Existing `#[tool]` macro provides reference implementation; ToolHandler trait from `src/server/mod.rs` is the target; TypedTool/TypedToolWithOutput show the code to generate; darling handles attribute parsing |
| STATE-INJECTION | State<T> extractor for standalone tools and &self for impl-block tools; eliminates Arc cloning ceremony for composition scenarios | State<T> wrapper type needed in pmcp crate; generated struct stores `Option<Arc<T>>` internally; `.with_state()` builder method on generated struct; &self in #[mcp_server] wraps instance in Arc for Send+Sync |
</phase_requirements>

## Standard Stack

### Core (existing -- no new dependencies needed for macros)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| syn | 2.0.117 | Parse Rust syntax (ItemFn, ItemImpl, Type, etc.) | Industry standard for proc macros; already in pmcp-macros with "full", "extra-traits", "visit-mut" features |
| quote | 1.0 | Rust code generation via quasi-quoting | Standard companion to syn |
| proc-macro2 | 1.0 | TokenStream bridging between proc-macro and syn/quote | Required for proc macro crate interop |
| darling | 0.23 | Attribute parsing into typed structs (FromMeta) | Already used for ToolArgs parsing; handles nested attributes cleanly |
| heck | 0.5 | Case conversion (snake_case to PascalCase) | Already in deps but unused; replaces custom `to_pascal_case` |

### Supporting (test infrastructure -- already in dev-dependencies)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| trybuild | 1.0 | Compile-fail tests for macro error messages | Test that missing description, wrong param types, etc. produce clear errors |
| insta | 1.43 | Snapshot testing for generated code verification | Verify macro expansion output matches expected code |
| proptest | 1.6 | Property-based testing for macro parameter parsing | Fuzz test parameter detection logic |

### New (may need to add to pmcp root crate)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| validator | 0.20.0 | Input validation trait | Optional -- only if D-27 auto-validate is gated behind a feature flag in pmcp |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| darling for attribute parsing | Raw syn NestedMeta parsing | darling is cleaner but adds a dependency (already present) |
| heck for case conversion | Custom to_pascal_case in utils.rs | heck handles edge cases (acronyms, numbers) better; already in Cargo.toml |
| trybuild for error tests | Manual compile_error! tests | trybuild captures actual rustc output; already in dev-deps |

**Installation:**
No new dependencies needed for pmcp-macros. The State<T> type goes in the pmcp root crate with zero additional deps.

For optional validator integration (D-27):
```bash
# Only if validator feature flag is added to pmcp
cargo add validator --optional  # In root Cargo.toml
```

## Architecture Patterns

### Recommended Project Structure
```
pmcp-macros/
├── src/
│   ├── lib.rs                 # Entry points: #[mcp_tool], #[mcp_server] (add alongside existing #[tool], #[tool_router])
│   ├── mcp_tool.rs            # NEW: #[mcp_tool] expansion logic
│   ├── mcp_server.rs          # NEW: #[mcp_server] expansion logic
│   ├── mcp_common.rs          # NEW: Shared codegen (ToolHandler impl, metadata, schema, annotations)
│   ├── tool.rs                # EXISTING: #[tool] (unchanged)
│   ├── tool_router.rs         # EXISTING: #[tool_router] (unchanged, deprecated)
│   └── utils.rs               # EXISTING: Shared utilities (extend as needed)
├── tests/
│   ├── mcp_tool_tests.rs      # NEW: Integration tests for #[mcp_tool]
│   ├── mcp_server_tests.rs    # NEW: Integration tests for #[mcp_server]
│   ├── tool_tests.rs          # EXISTING (unchanged)
│   ├── tool_router_tests.rs   # EXISTING (unchanged)
│   └── ui/
│       ├── mcp_tool_missing_description.rs   # NEW: compile-fail
│       ├── mcp_tool_wrong_return_type.rs     # NEW: compile-fail
│       ├── mcp_tool_invalid_state_type.rs    # NEW: compile-fail
│       └── tool_missing_description.rs       # EXISTING
src/
├── server/
│   ├── state.rs               # NEW: State<T> wrapper type definition
│   └── mod.rs                 # Add pub mod state; re-export State
├── lib.rs                     # Re-export State, #[mcp_tool], #[mcp_server]
```

### Pattern 1: Parameter Type Detection (mcp_common.rs)
**What:** Classify function parameters by their type to determine their role.
**When to use:** Both `#[mcp_tool]` (standalone) and `#[mcp_server]` (impl method) must inspect parameters.
**Implementation approach:**

```rust
// Source: Design decision D-11 — match by type, not position
enum ParamRole {
    /// First struct implementing JsonSchema + Deserialize = tool input
    Args(Type),
    /// State<T> = shared state injection
    State(Type),  // The inner T
    /// RequestHandlerExtra = cancellation/progress/auth
    Extra,
    /// &self in impl block = server instance
    SelfRef,
}

fn classify_param(param: &FnArg) -> syn::Result<ParamRole> {
    match param {
        FnArg::Receiver(_) => Ok(ParamRole::SelfRef),
        FnArg::Typed(pat_type) => {
            let ty = &*pat_type.ty;
            if is_state_type(ty) {
                Ok(ParamRole::State(extract_state_inner(ty)?))
            } else if is_request_handler_extra(ty) {
                Ok(ParamRole::Extra)
            } else {
                // Assume first non-special type is the args struct
                Ok(ParamRole::Args(ty.clone()))
            }
        }
    }
}

fn is_state_type(ty: &Type) -> bool {
    // Check for State<T> or pmcp::State<T> or pmcp::server::State<T>
    if let Type::Path(path) = ty {
        path.path.segments.last()
            .map(|s| s.ident == "State")
            .unwrap_or(false)
    } else {
        false
    }
}

fn is_request_handler_extra(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        path.path.segments.last()
            .map(|s| s.ident == "RequestHandlerExtra")
            .unwrap_or(false)
    } else {
        false
    }
}
```

### Pattern 2: Generated ToolHandler Impl (mcp_tool.rs)
**What:** For each `#[mcp_tool]` function, generate a struct + ToolHandler impl.
**When to use:** Standalone tool functions (not inside an impl block).
**Target trait:** `ToolHandler` from `src/server/mod.rs` (NOT `src/server/traits.rs`).

The target trait signature:
```rust
#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value>;
    fn metadata(&self) -> Option<ToolInfo> { None }
}
```

Generated code pattern for a function like:
```rust
#[mcp_tool(description = "Perform arithmetic")]
async fn calculator(args: CalculatorArgs, db: State<Database>) -> Result<CalculatorResult> {
    // ...
}
```

Should expand to approximately:
```rust
// Original function preserved (but may be made private or renamed)
async fn calculator(args: CalculatorArgs, db: State<Database>) -> Result<CalculatorResult> {
    // ...
}

/// Generated tool handler for `calculator`
pub struct CalculatorTool {
    state: Option<std::sync::Arc<Database>>,
}

impl CalculatorTool {
    /// Attach shared state for this tool
    pub fn with_state(mut self, state: impl Into<std::sync::Arc<Database>>) -> Self {
        self.state = Some(state.into());
        self
    }
}

/// Constructor function for ergonomic registration
pub fn calculator() -> CalculatorTool {
    CalculatorTool { state: None }
}

#[async_trait::async_trait]
impl pmcp::ToolHandler for CalculatorTool {
    async fn handle(
        &self,
        args: serde_json::Value,
        extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        // Deserialize args
        let typed_args: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::Validation(
                format!("Invalid arguments for tool 'calculator': {}", e)
            ))?;

        // Resolve state
        let db = pmcp::State(
            self.state.as_ref()
                .expect("State<Database> not provided — call .with_state() at registration")
                .clone()
        );

        // Call handler
        let result = calculator(typed_args, db).await?;

        // Serialize typed output
        serde_json::to_value(result)
            .map_err(|e| pmcp::Error::Internal(format!("Failed to serialize result: {}", e)))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        let input_schema = {
            let schema = schemars::schema_for!(CalculatorArgs);
            let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
                serde_json::json!({"type": "object", "properties": {}})
            });
            pmcp::server::schema_utils::normalize_schema(json_schema)
        };

        let output_schema = {
            let schema = schemars::schema_for!(CalculatorResult);
            Some(serde_json::to_value(&schema).unwrap_or_else(|_| {
                serde_json::json!({"type": "object"})
            }))
        };

        let mut info = pmcp::types::ToolInfo::new(
            "calculator",
            Some("Perform arithmetic".to_string()),
            input_schema,
        );
        info.output_schema = output_schema;
        // annotations, ui, etc. set from macro attributes
        Some(info)
    }
}
```

### Pattern 3: State<T> Wrapper Type (src/server/state.rs)
**What:** A newtype wrapper around `Arc<T>` that auto-derefs to `&T`.
**When to use:** Standalone `#[mcp_tool]` functions needing shared state.

```rust
use std::ops::Deref;
use std::sync::Arc;

/// State extractor for MCP tools.
///
/// Provides shared state injection for standalone `#[mcp_tool]` functions,
/// similar to Axum's `State<T>` extractor. The state is wrapped in `Arc<T>`
/// and auto-derefs to `&T`.
///
/// # Example
///
/// ```rust,ignore
/// #[mcp_tool(description = "Query database")]
/// async fn query(args: QueryArgs, db: State<Database>) -> Result<Value> {
///     db.execute(&args.sql).await  // auto-deref to &Database
/// }
/// ```
#[derive(Debug, Clone)]
pub struct State<T>(pub Arc<T>);

impl<T> Deref for State<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> From<Arc<T>> for State<T> {
    fn from(arc: Arc<T>) -> Self {
        State(arc)
    }
}

impl<T> From<T> for State<T> {
    fn from(val: T) -> Self {
        State(Arc::new(val))
    }
}
```

### Pattern 4: #[mcp_server] Impl Block (mcp_server.rs)
**What:** Processes an impl block, finds all `#[mcp_tool]` methods, generates per-tool ToolHandler impls wrapping the struct instance in Arc, plus a registration helper.
**When to use:** Multi-tool servers with shared state via `&self`.

Key codegen: The struct is wrapped in `Arc<Self>` and each tool handler holds a clone of that Arc. The `#[mcp_server]` generates a trait or extension on the builder:

```rust
// For:
// #[mcp_server]
// impl MyServer { ... }

// Generates:
impl MyServer {
    /// Register all tools from this server on the builder
    pub fn register_tools(
        self,
        mut builder: pmcp::ServerBuilder,
    ) -> pmcp::ServerBuilder {
        let shared = std::sync::Arc::new(self);
        // For each #[mcp_tool] method:
        builder = builder.tool("query", QueryToolHandler { server: shared.clone() });
        builder = builder.tool("clear_cache", ClearCacheToolHandler { server: shared.clone() });
        builder
    }
}
```

### Pattern 5: Generic Impl Blocks (D-25)
**What:** `#[mcp_server]` must preserve type parameters and trait bounds.
**When to use:** Composition servers like `impl<F: FoundationClient + 'static> ArithmeticsServer<F>`.

The macro must:
1. Extract generics from `ItemImpl` via `impl_block.generics`
2. Propagate them to generated handler structs
3. Add `Send + Sync + 'static` bounds for async_trait compatibility
4. Use the existing `utils::add_async_trait_bounds` helper

```rust
// Input:
#[mcp_server]
impl<F: FoundationClient + 'static> ArithmeticsServer<F> {
    #[mcp_tool(description = "Calculate")]
    async fn calculate(&self, args: CalcArgs) -> Result<CalcResult> { ... }
}

// Generated handler struct must be generic too:
struct CalculateToolHandler<F: FoundationClient + 'static + Send + Sync> {
    server: Arc<ArithmeticsServer<F>>,
}
```

### Pattern 6: Sync/Async Auto-Detection (D-26)
**What:** Check `input.sig.asyncness.is_some()` to determine if the function is async.
**When to use:** Every `#[mcp_tool]` expansion.

```rust
let is_async = func.sig.asyncness.is_some();
// For async: generate Box::pin(async move { ... }).await in handle()
// For sync: call function directly, wrap in immediate future for ToolHandler
```

For sync tools, the ToolHandler `handle` method (which is async) wraps the sync call:
```rust
async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
    // Deserialize, validate...
    let result = my_sync_fn(typed_args)?;  // No .await
    serde_json::to_value(result).map_err(...)
}
```

### Pattern 7: Validator Auto-Check (D-27)
**What:** If args type implements `validator::Validate`, call `.validate()` before handler.
**When to use:** Optional enhancement; uses a trait-bound check at compile time.

The macro cannot know at expansion time whether a type implements `Validate`. Use a trait-based conditional:

```rust
// Generated code uses a trait trick:
trait MaybeValidate {
    fn maybe_validate(&self) -> std::result::Result<(), String> { Ok(()) }
}
impl<T> MaybeValidate for T {}

// Blanket impl for types that DO implement Validate:
trait DoValidate {
    fn maybe_validate(&self) -> std::result::Result<(), String>;
}
impl<T: validator::Validate> DoValidate for T {
    fn maybe_validate(&self) -> std::result::Result<(), String> {
        self.validate().map_err(|e| format!("Validation failed: {}", e))
    }
}
```

Alternative (simpler, recommended): Gate behind a feature flag `validation` and use `#[cfg(feature = "validation")]` in generated code. The pmcp crate already has a `validation` feature flag.

### Anti-Patterns to Avoid
- **Generating code that references `pmcp::` directly without considering the proc macro compile context:** Proc macro crate cannot link against pmcp at compile time. Generated code runs in the user's crate context where `pmcp` is a dependency. This is correct -- use `pmcp::` paths in quote! output.
- **Extracting args individually instead of as a single struct:** The existing `#[tool]` macro extracts individual params from JSON, which doesn't support schemars schema generation. `#[mcp_tool]` must take a single struct (or no args) to enable `schema_for!`.
- **Using `_extra` parameter name in generated code:** This causes unused variable warnings. Generated code should use `_extra` only when the user didn't declare extra in their signature, and `extra` when they did.
- **Hardcoding schema generation without cfg guards:** The `schema-generation` feature flag exists for a reason. Generated schema code should be behind `#[cfg(feature = "schema-generation")]` with a fallback. HOWEVER, per Claude's discretion, this can be simplified -- always require schemars for `#[mcp_tool]` users since the macro's value proposition depends on schema generation.
- **Modifying the original function body:** The macro should preserve the user's function as-is and generate wrapper code alongside it, not transform the function in-place.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Attribute parsing | Custom token parsing | darling::FromMeta | Handles nested attributes, defaults, validation; already in deps |
| Case conversion | Custom to_pascal_case | heck::ToUpperCamelCase | Handles edge cases (acronyms, numbers, double underscores); already in Cargo.toml |
| JSON Schema generation | Custom schema builder | schemars::schema_for! | Type-safe, handles enums, options, nested types; already the SDK standard |
| Schema normalization | Custom $ref inlining | pmcp::server::schema_utils::normalize_schema | Handles circular refs, size limits, metadata stripping; already tested |
| Async trait impl | Manual Pin<Box<...>> | async_trait::async_trait in generated code | Users expect this pattern; matches existing TypedTool/ToolHandler |
| ToolInfo construction | Manual struct literal | ToolInfo::new() / ToolInfo::with_annotations() / ToolInfo::with_ui() | Constructors handle _meta, annotations, output_schema correctly |
| UI metadata | Manual _meta JSON building | crate::types::ui::build_ui_meta() | Handles nested ui.resourceUri format correctly |

**Key insight:** The macro generates code that uses the existing SDK APIs (TypedTool, ToolInfo, schema_utils). It should never duplicate SDK logic -- it should generate calls to SDK functions. The macro is sugar, not a replacement.

## Common Pitfalls

### Pitfall 1: Proc Macro Crate Cannot Import Runtime Types
**What goes wrong:** Attempting to use `pmcp::ToolHandler` or `pmcp::types::ToolInfo` in the proc macro crate itself (at expansion time).
**Why it happens:** Proc macro crates compile as compiler plugins; they can only depend on other proc-macro-compatible crates. They generate code that references runtime types, but cannot link against them.
**How to avoid:** All `pmcp::` references in `quote!` blocks are output tokens, not used at compile time. The proc macro crate only uses `syn`, `quote`, `proc-macro2`, and `darling`. The generated code references `pmcp::` paths which resolve in the user's crate context.
**Warning signs:** Build errors like "can't find crate `pmcp`" in the proc macro crate.

### Pitfall 2: State<T> Missing at Runtime
**What goes wrong:** User registers a tool that expects `State<T>` but forgets `.with_state()`.
**Why it happens:** The macro generates a struct with `Option<Arc<T>>` for state; if not set, it panics at first tool call.
**How to avoid:** Two options: (a) panic with a clear message at registration time via a validation method called by the builder, or (b) return a compile-time error. Since the macro cannot enforce this at compile time (state is provided at registration, not at macro expansion), use a clear runtime panic: `"State<Database> not provided for tool 'query' -- call .with_state() during registration"`.
**Warning signs:** `Option::unwrap()` panic in handle().

### Pitfall 3: Multiple Args Structs in Signature
**What goes wrong:** User passes two struct parameters, and the macro misidentifies which is the args struct.
**Why it happens:** Parameter classification by type only detects `State<T>` and `RequestHandlerExtra` -- everything else is assumed to be args.
**How to avoid:** Emit a compile error if more than one non-special parameter is found: `"#[mcp_tool] functions accept at most one args parameter (a struct implementing Deserialize + JsonSchema)"`.
**Warning signs:** Mysterious deserialization failures at runtime.

### Pitfall 4: Output Schema for Result<Value>
**What goes wrong:** Macro generates outputSchema for `Value` type, which produces a schema that accepts anything.
**Why it happens:** `Result<Value>` looks like a typed output to naive type analysis.
**How to avoid:** Special-case `Value` and `serde_json::Value` -- when detected as the Ok type, skip outputSchema generation (D-15). Check the last segment of the type path.
**Warning signs:** outputSchema of `{}` or `{"additionalProperties": true}` in tool metadata.

### Pitfall 5: Generic Impl Block Lifetime Inference
**What goes wrong:** Generated handler struct for a generic impl block fails to compile due to missing Send/Sync/'static bounds.
**Why it happens:** The async_trait macro requires Send + Sync + 'static for all type parameters in async trait impls.
**How to avoid:** Use `utils::add_async_trait_bounds` to append `Send + Sync + 'static` to all generic type params in generated handler structs. Also ensure the `server: Arc<T>` field uses the fully-bounded generic.
**Warning signs:** Error messages like "the trait `Send` is not implemented for `F`".

### Pitfall 6: Name Collision Between Generated Types
**What goes wrong:** Two `#[mcp_tool]` functions with similar names generate structs that collide.
**Why it happens:** `calculator` generates `CalculatorTool` -- if another crate or module also has `CalculatorTool`, collision occurs.
**How to avoid:** Generate struct names with a consistent suffix pattern (`{PascalName}Tool`) and document the naming convention. For `#[mcp_server]`, use `{PascalName}ToolHandler` to distinguish from standalone tool structs.
**Warning signs:** "duplicate definition" compile errors.

### Pitfall 7: schemars 1.x vs 0.8.x API Differences
**What goes wrong:** Generated code uses schemars 0.8.x APIs that don't exist in 1.x.
**Why it happens:** Training data may reference old schemars APIs.
**How to avoid:** The project uses schemars 1.1.0. The `schema_for!` macro is the same across versions. The main difference: schemars 1.x uses `schemars::SchemaObject` not `RootSchema` in some APIs. Generated code should use `schemars::schema_for!` which works in both.
**Warning signs:** "cannot find value `schema_for` in module `schemars`" errors.

## Code Examples

Verified patterns from the existing codebase:

### ToolHandler Trait (the target for generated code)
```rust
// Source: src/server/mod.rs:195-204
#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn handle(&self, args: Value, extra: cancellation::RequestHandlerExtra) -> Result<Value>;
    fn metadata(&self) -> Option<crate::types::ToolInfo> { None }
}
```

### Builder Registration (the API generated tools must be compatible with)
```rust
// Source: src/server/builder.rs:146-166
pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
    let name = name.into();
    let handler = Arc::new(handler) as Arc<dyn ToolHandler>;
    let mut info = handler.metadata()
        .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
    info.name.clone_from(&name);
    self.tool_infos.insert(name.clone(), info);
    self.tools.insert(name, handler);
    // capabilities auto-set...
    self
}
```

### TypedToolWithOutput ToolHandler impl (the pattern to match)
```rust
// Source: src/server/typed_tool.rs:673-729
#[async_trait]
impl<TIn, TOut, F> ToolHandler for TypedToolWithOutput<TIn, TOut, F>
where
    TIn: DeserializeOwned + Send + Sync + 'static,
    TOut: Serialize + Send + Sync + 'static,
    F: Fn(TIn, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<TOut>> + Send>>
        + Send + Sync,
{
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let typed_args: TIn = serde_json::from_value(args)
            .map_err(|e| Error::Validation(format!("Invalid arguments: {}", e)))?;
        let result = (self.handler)(typed_args, extra).await?;
        serde_json::to_value(result)
            .map_err(|e| Error::Internal(format!("Failed to serialize result: {}", e)))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        // Build annotations, merge output_type_name...
        Some(ToolInfo {
            name: self.name.clone(),
            title: None,
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            annotations: ...,
            icons: None,
            _meta: crate::types::ui::build_ui_meta(self.ui_resource_uri.as_deref()),
            execution: None,
        })
    }
}
```

### Schema Generation (generate_schema from typed_tool.rs)
```rust
// Source: src/server/typed_tool.rs:410-425
#[cfg(feature = "schema-generation")]
fn generate_schema<T: JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);
    let json_schema = serde_json::to_value(&schema).unwrap_or_else(|_| {
        serde_json::json!({"type": "object", "properties": {}, "additionalProperties": true})
    });
    crate::server::schema_utils::normalize_schema(json_schema)
}
```

### Darling Attribute Parsing (existing pattern in tool.rs)
```rust
// Source: pmcp-macros/src/tool.rs:42-54
#[derive(Debug, FromMeta)]
struct ToolArgs {
    #[darling(default)]
    name: Option<String>,
    description: String,
    #[darling(default)]
    annotations: Option<ToolAnnotations>,
}
```

### Compile-Fail Test Pattern (existing)
```rust
// Source: pmcp-macros/tests/ui/tool_missing_description.rs
use pmcp_macros::tool;

#[tool] // Missing description - should fail
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[tool]` macro with individual params | `#[mcp_tool]` with struct args | Phase 58 (new) | Enables schema generation, typed output |
| `#[tool_router]` for impl blocks | `#[mcp_server]` companion macro | Phase 58 (new) | Proper ToolHandler integration, generic support |
| Manual `Box::pin(async move { ... })` | Macro hides pinning | Phase 58 (new) | DX improvement for non-Rust experts |
| Manual `Arc::clone()` + `move` closures | `State<T>` extractor | Phase 58 (new) | Eliminates Arc ceremony |
| schemars 0.8 | schemars 1.x | Already in project (1.1.0) | `schema_for!` macro unchanged; internal types differ |
| `pmcp-macros` not integrated with pmcp | `pmcp-macros` dependency commented out | Current state | Must uncomment and wire up the `macros` feature flag |

**Current integration state:**
- `pmcp-macros` exists as workspace member but is **not** a dependency of the `pmcp` crate (line 40 of root Cargo.toml: `# pmcp-macros = { ... }` is commented out)
- The `macros` feature flag (line 153: `# macros = ["dep:pmcp-macros", "dep:schemars"]`) is also commented out
- Phase 58 must uncomment both and wire the re-exports

## Open Questions

1. **Schema generation feature gating**
   - What we know: The `schema-generation` feature flag exists. TypedTool gates `::new()` behind it.
   - What's unclear: Should `#[mcp_tool]` always require `schemars` (and error if the feature is off), or should it degrade gracefully with an empty schema?
   - Recommendation: Always require `schema-generation` for `#[mcp_tool]` users. The macro's primary value is DX + schema, and schema without schemars is an empty `{}`. Emit a compile error if `schemars` is not available. The `macros` feature should imply `schema-generation`.

2. **Validator integration mechanism**
   - What we know: `validator` 0.20.0 is the current version. The project doesn't depend on it yet. D-27 says auto-validate if type implements `Validate`.
   - What's unclear: Whether to use a trait trick (specialization-like) or a feature flag.
   - Recommendation: Gate behind a `validation` feature flag. When enabled, generated code calls `typed_args.validate().map_err(...)` before handler invocation. When disabled, skip the call. This avoids the complexity of trait-based auto-detection and is explicit.

3. **Where the `mcp_server` registration method lives**
   - What we know: D-20 says `.mcp_server(instance)` on the builder. The builder is in `src/server/builder.rs`.
   - What's unclear: Whether this is a trait method the macro generates an impl for, or a new builder method.
   - Recommendation: Generate a `register_tools(self, builder: ServerCoreBuilder) -> ServerCoreBuilder` method on the user's struct. The `.mcp_server()` convenience can be added to the builder as a generic method: `pub fn mcp_server<T: McpServerRegistration>(self, server: T) -> Self`. The `McpServerRegistration` trait is generated by the macro for each `#[mcp_server]`-annotated type.

4. **How to handle `Result<T>` with different error types**
   - What we know: The design document shows `Result<T>` (implied `pmcp::Result<T>`). TypedToolWithOutput uses `Result<TOut>` which is `pmcp::Result<TOut>`.
   - What's unclear: What if user writes `Result<T, MyError>` where `MyError: Into<pmcp::Error>`?
   - Recommendation: Accept `Result<T, E>` where `E: Into<pmcp::Error>`. In generated code, convert via `.map_err(Into::into)`. For single-type-param `Result<T>`, assume `pmcp::Result<T>`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test + trybuild 1.0 + insta 1.43 + proptest 1.6 |
| Config file | pmcp-macros/Cargo.toml (dev-dependencies already configured) |
| Quick run command | `cargo test -p pmcp-macros` |
| Full suite command | `cargo test -p pmcp-macros && cargo test --workspace` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TOOL-MACRO-01 | #[mcp_tool] compiles for async fn with args | integration | `cargo test -p pmcp-macros test_mcp_tool_async_with_args` | Wave 0 |
| TOOL-MACRO-02 | #[mcp_tool] compiles for sync fn | integration | `cargo test -p pmcp-macros test_mcp_tool_sync` | Wave 0 |
| TOOL-MACRO-03 | #[mcp_tool] compiles for no-arg tool | integration | `cargo test -p pmcp-macros test_mcp_tool_no_args` | Wave 0 |
| TOOL-MACRO-04 | Missing description produces compile error | compile-fail | `cargo test -p pmcp-macros trybuild` | Wave 0 |
| TOOL-MACRO-05 | Generated struct implements ToolHandler | integration | `cargo test -p pmcp-macros test_mcp_tool_handler_impl` | Wave 0 |
| TOOL-MACRO-06 | Typed output generates outputSchema | integration | `cargo test -p pmcp-macros test_mcp_tool_typed_output` | Wave 0 |
| TOOL-MACRO-07 | Result<Value> skips outputSchema | integration | `cargo test -p pmcp-macros test_mcp_tool_value_output` | Wave 0 |
| TOOL-MACRO-08 | Annotations propagate to ToolInfo | integration | `cargo test -p pmcp-macros test_mcp_tool_annotations` | Wave 0 |
| TOOL-MACRO-09 | ui attribute generates _meta | integration | `cargo test -p pmcp-macros test_mcp_tool_ui_attr` | Wave 0 |
| STATE-INJ-01 | State<T> detected in fn signature | unit | `cargo test -p pmcp-macros test_classify_state_param` | Wave 0 |
| STATE-INJ-02 | .with_state() sets Arc<T> | integration | `cargo test -p pmcp-macros test_with_state` | Wave 0 |
| STATE-INJ-03 | Missing state panics with clear message | integration | `cargo test -p pmcp-macros test_missing_state_panic` | Wave 0 |
| SERVER-01 | #[mcp_server] collects multiple tools | integration | `cargo test -p pmcp-macros test_mcp_server_multi_tool` | Wave 0 |
| SERVER-02 | Generic impl blocks preserved | integration | `cargo test -p pmcp-macros test_mcp_server_generic` | Wave 0 |
| SERVER-03 | register_tools works with builder | integration | `cargo test -p pmcp-macros test_mcp_server_registration` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-macros`
- **Per wave merge:** `make quality-gate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `pmcp-macros/tests/mcp_tool_tests.rs` -- covers TOOL-MACRO-01 through TOOL-MACRO-09
- [ ] `pmcp-macros/tests/mcp_server_tests.rs` -- covers SERVER-01 through SERVER-03
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description.rs` -- compile-fail for TOOL-MACRO-04
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` -- expected error output
- [ ] `pmcp-macros/src/mcp_tool.rs` -- main macro expansion module
- [ ] `pmcp-macros/src/mcp_server.rs` -- impl block companion macro
- [ ] `pmcp-macros/src/mcp_common.rs` -- shared codegen utilities
- [ ] `src/server/state.rs` -- State<T> wrapper type
- [ ] Example file: `examples/XX_mcp_tool_macro.rs` -- demonstrates before/after

## Sources

### Primary (HIGH confidence)
- Existing codebase: `pmcp-macros/src/tool.rs` -- current #[tool] implementation, reference for attribute parsing and codegen
- Existing codebase: `pmcp-macros/src/tool_router.rs` -- current #[tool_router], reference for impl-block scanning
- Existing codebase: `src/server/mod.rs:195-204` -- ToolHandler trait definition (target for generated code)
- Existing codebase: `src/server/typed_tool.rs` -- TypedTool, TypedToolWithOutput implementations (pattern to match)
- Existing codebase: `src/server/builder.rs:146-166` -- ServerCoreBuilder::tool() (compatibility target)
- Design document: `docs/design/mcp-tool-macro-design.md` -- Full RFC with API surface, parameter rules, team feedback
- Context document: `.planning/phases/58-mcp-tool-proc-macro/58-CONTEXT.md` -- Locked decisions D-01 through D-28

### Secondary (MEDIUM confidence)
- [Trybuild documentation](https://docs.rs/trybuild/latest/trybuild/) -- Compile-fail test patterns
- [Darling crate](https://github.com/TedDriggs/darling) -- Attribute parsing best practices
- [Ferrous Systems proc macro testing](https://ferrous-systems.com/blog/testing-proc-macros/) -- Testing strategies for proc macros
- [Axum State extractor docs](https://docs.rs/axum/latest/axum/extract/struct.State.html) -- State<T> pattern reference
- crates.io: darling 0.23.0, syn 2.0.117, validator 0.20.0, schemars 1.2.1 (latest), trybuild 1.0.116

### Tertiary (LOW confidence)
- None -- all findings verified against existing codebase or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in pmcp-macros Cargo.toml; no new deps needed for core functionality
- Architecture: HIGH -- design document is comprehensive, existing macro code provides clear reference implementations, ToolHandler trait is stable
- Pitfalls: HIGH -- derived from direct codebase analysis (proc macro crate isolation, State<T> runtime validation, schemars version)
- Codegen patterns: HIGH -- derived from actual TypedTool/TypedToolWithOutput implementations in the codebase

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain -- proc macro APIs, syn/quote, and ToolHandler trait are stable)
