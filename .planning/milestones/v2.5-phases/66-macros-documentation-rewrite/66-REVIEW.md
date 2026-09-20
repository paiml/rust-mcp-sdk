---
phase: 66-macros-documentation-rewrite
reviewed: 2026-04-11T00:00:00Z
depth: standard
files_reviewed: 13
files_reviewed_list:
  - CHANGELOG.md
  - Cargo.toml
  - docs/advanced/migration-from-typescript.md
  - examples/s23_mcp_tool_macro.rs
  - examples/s24_mcp_prompt_macro.rs
  - pmcp-course/src/part1-foundations/ch01-03-why-rust.md
  - pmcp-course/src/part5-security/ch13-02-oauth-basics.md
  - pmcp-course/src/part5-security/ch13-03-validation.md
  - pmcp-course/src/part5-security/ch13-oauth.md
  - pmcp-macros/CHANGELOG.md
  - pmcp-macros/Cargo.toml
  - pmcp-macros/README.md
  - pmcp-macros/src/lib.rs
findings:
  critical: 0
  warning: 4
  info: 3
  total: 7
status: issues_found
---

# Phase 66: Code Review Report

**Reviewed:** 2026-04-11
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

Phase 66 rewrites the pmcp-macros surface to drop the deprecated `#[tool]` /
`#[tool_router]` / `#[prompt]` / `#[resource]` macros and bump `pmcp-macros`
0.4.1 → 0.5.0 plus `pmcp` 2.2.0 → 2.3.0. The real Rust code — `pmcp-macros/src/lib.rs`,
`pmcp-macros/README.md`, and `examples/s23_mcp_tool_macro.rs` / `s24_mcp_prompt_macro.rs`
— is clean, internally consistent, and correctly wired to the new `#[mcp_tool]` /
`#[mcp_server]` / `#[mcp_prompt]` API. Version bumps across `Cargo.toml` (pmcp 2.3.0,
pmcp-macros dep pinned to 0.5.0) and both CHANGELOGs are consistent with one another
and with the CLAUDE.md release rules.

However, the downstream Wave 1 markdown cleanup advertised in the 2.3.0 CHANGELOG
(`docs/advanced/migration-from-typescript.md` and four pmcp-course chapters updated
to `#[mcp_tool]` / `#[mcp_server]` syntax) is **incomplete**. Three `pmcp-course`
chapters in `part5-security/` still ship `#[derive(TypedTool)]` — a derive macro
that does not exist in pmcp OR pmcp-macros (TypedTool is a builder struct, not a
derive) — combined with `#[mcp_tool]` attribute macros, producing a hybrid pattern
that cannot compile. `docs/advanced/migration-from-typescript.md` additionally
references `pmcp::Parameters`, which is not a re-exported type. These are the
highest-impact issues: they contradict the phase's own CHANGELOG entry, and they
will surface as stale snippets to anyone copy-pasting from the course.

## Warnings

### WR-01: Course chapters ship non-existent `#[derive(TypedTool)]` combined with `#[mcp_tool]`

**Files:**
- `pmcp-course/src/part5-security/ch13-02-oauth-basics.md:242-244`
- `pmcp-course/src/part5-security/ch13-03-validation.md:77-79`
- `pmcp-course/src/part5-security/ch13-oauth.md:176-178`
- `pmcp-course/src/part5-security/ch13-oauth.md:293-296`

**Issue:** Four code snippets declare tools using a hybrid `#[derive(TypedTool)]` +
`#[mcp_tool(...)]` pattern:

```rust
#[derive(TypedTool)]
#[mcp_tool(name = "execute_query", description = "Run a database query")]
pub struct ExecuteQuery;
```

`TypedTool` is a **struct** in `src/server/typed_tool.rs` (`pub struct TypedTool<T, F>`),
built via `TypedTool::new(...)`. There is no `proc_macro_derive(TypedTool)` anywhere
in the workspace (verified by grep of `pmcp-macros/src/` and `src/`). Additionally,
`#[mcp_tool]` targets functions (`parse_macro_input!(input as ItemFn)` in
`pmcp-macros/src/lib.rs:85`), not unit structs — applying it to `pub struct
ExecuteQuery;` would fail with a parse error.

The Phase 66 CHANGELOG (root `CHANGELOG.md:31`) explicitly advertises that
"`docs/advanced/migration-from-typescript.md` and four pmcp-course chapters updated
to `#[mcp_tool]` / `#[mcp_server]` syntax (Phase 66 Wave 1 cleanup of downstream
consumers)" — these three chapters were part of the advertised cleanup but were
not fully migrated. The stale hybrid will mislead every reader of part5-security.

**Fix:** Convert each snippet to either the function-with-args-struct `#[mcp_tool]`
pattern shown in `pmcp-macros/README.md` or, if the chapters want to keep the struct
shape (e.g., to attach a `.database` field for the OAuth examples), use the
`#[mcp_server] impl` block pattern:

```rust
// Option A: standalone function (matches README.md example)
use pmcp::{mcp_tool, State};

#[derive(Debug, Deserialize, JsonSchema)]
struct ExecuteQueryArgs { sql: String }

#[mcp_tool(description = "Run a database query")]
async fn execute_query(
    args: ExecuteQueryArgs,
    ctx: State<QueryContext>,
) -> pmcp::Result<QueryResult> {
    ctx.auth.require_scope("execute:tools")?;
    ctx.database.execute(&args.sql).await
}

// Option B: impl-block (preserves the `struct ExecuteQuery { database }` shape)
#[mcp_server]
impl ExecuteQuery {
    #[mcp_tool(description = "Run a database query")]
    async fn execute_query(
        &self,
        args: ExecuteQueryArgs,
        ctx: State<AuthContext>,
    ) -> pmcp::Result<QueryResult> {
        ctx.require_scope("execute:tools")?;
        self.database.execute(&args.sql).await
    }
}
```

### WR-02: `docs/advanced/migration-from-typescript.md` imports non-existent `pmcp::Parameters`

**File:** `docs/advanced/migration-from-typescript.md:113-133`

**Issue:** The Rust PMCP example reads:

```rust
use pmcp::{mcp_server, mcp_tool, Parameters};
// ...
async fn calculate(&self, params: Parameters<CalculateParams>) -> Result<f64> {
    match params.0.operation.as_str() { ... }
}
```

`pmcp::Parameters` does not exist in the re-export list at `src/lib.rs:120-131` or
anywhere else in the public API (grep confirms zero matches on `Parameters` in
`src/lib.rs`). The `#[mcp_tool]` macro expects the argument-struct to be passed as
a plain parameter (e.g., `args: CalculateParams`), not wrapped in a `Parameters<T>`
newtype — this is visible in both the README (`async fn add(args: AddArgs) ->
pmcp::Result<AddResult>`) and `examples/s23_mcp_tool_macro.rs:50`.

An HTML comment on line 112 (`<!-- Note: real #[mcp_tool] requires a single args
struct — see pmcp-macros/README.md -->`) acknowledges the snippet is illustrative,
but the body still imports a symbol that does not exist, so readers grepping for
`Parameters` in the SDK will not find it. This is the kind of silent API drift that
the Phase 66 include-str doctest wiring was designed to catch — but only catches
for README.md, not for `docs/advanced/*.md`.

The snippet also references `Error::InvalidParams` (line 130), but the real error
type uses `pmcp::Error::InvalidRequest { .. }` / `pmcp::Error::Validation`; confirm
or remove the stale constructor.

**Fix:** Replace the `Parameters<T>` wrapping with the flat arg-struct pattern used
by the README and the runnable example:

```rust
use pmcp::{mcp_server, mcp_tool};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct CalculateParams {
    operation: String,
    a: f64,
    b: f64,
}

struct Calculator;

#[mcp_server]
impl Calculator {
    #[mcp_tool(description = "Perform calculations")]
    async fn calculate(&self, args: CalculateParams) -> pmcp::Result<f64> {
        match args.operation.as_str() {
            "add" => Ok(args.a + args.b),
            "subtract" => Ok(args.a - args.b),
            _ => Err(pmcp::Error::validation("Unknown operation")),
        }
    }
}
```

Consider also deleting the trailing `// Register with server
server.register_tool_handler(Calculator::new());` line — there is no
`register_tool_handler` method on the public `Server` / `ServerBuilder` surface
either; `.mcp_server(Calculator)` is the real API.

### WR-03: `migration-from-typescript.md` also ships stale `#[resource]` and `#[prompt]` stub syntax

**File:** `docs/advanced/migration-from-typescript.md:171-254`

**Issue:** The Resource Handling section uses `#[resource(uri_pattern = "file:///{path}")]`
(line 174) and the Prompt Management section uses `#[prompt(name = "summarize", description = "Summarize content")]`
(line 235). Per `pmcp-macros/CHANGELOG.md` lines 20-23, both are **removed in 0.5.0**:

> - **`#[prompt]` zero-op stub removed.** This macro was a placeholder identity
>   function that generated no code. Use `#[mcp_prompt]` for the functional
>   equivalent.
> - **`#[resource]` zero-op stub removed.** ... Use `#[mcp_resource]` for the
>   functional equivalent.

The root `CHANGELOG.md` 2.3.0 entry (line 31) advertises this file as updated for
Phase 66 — but the Resource and Prompt subsections were left on the deleted macros.
The `uri_pattern = "..."` attribute name also never existed in the deleted stub; the
real attribute on `#[mcp_resource]` is `uri = "..."` (confirmed in README line 305
and `src/lib.rs:194-195`). Readers following this migration guide will be blocked
immediately.

**Fix:** Rewrite both subsections to use `#[mcp_resource]` and `#[mcp_prompt]` with
their current attribute names. Note that per the README (line 282-285) and
`src/lib.rs:146-147`, `mcp_resource` is **not yet re-exported** from `pmcp` — it
must be imported as `use pmcp_macros::mcp_resource;`. The migration guide should
call this out or readers will hit an import error:

```rust
use pmcp::types::{Content, GetPromptResult, PromptMessage};
use pmcp::{mcp_prompt, ServerBuilder};
// Note: mcp_resource is not yet re-exported from pmcp — import directly.
use pmcp_macros::mcp_resource;

#[mcp_resource(uri = "file:///{path}", description = "Read a file from disk")]
async fn read_file(path: String) -> pmcp::Result<String> {
    Ok(tokio::fs::read_to_string(&format!("/{path}")).await?)
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SummarizeArgs { content: String }

#[mcp_prompt(description = "Summarize content")]
async fn summarize(args: SummarizeArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(
            format!("Please summarize: {}", args.content),
        ))],
        None,
    ))
}
```

### WR-04: `pmcp-macros/README.md` installation snippet pins pmcp "2.3" but the feature `full` is required for macros to link in most example contexts

**File:** `pmcp-macros/README.md:13-19`

**Issue:** The Installation block says:

```toml
pmcp = { version = "2.3", features = ["macros"] }
```

This is technically correct — `macros = ["dep:pmcp-macros", "schema-generation"]`
in `Cargo.toml:167` — and the feature chain pulls in `schemars` transitively. The
subsequent `rust,no_run` doctest examples, however, call `ServerBuilder::new()`
with a chained `.tool(...)` / `.mcp_server(...)` that resolves against the native
(non-wasm) `ServerBuilder` at `src/server/mod.rs:1876-1915` — which is only compiled
outside of the `target_arch = "wasm32"` branch. Users who copy the minimal `features
= ["macros"]` line from the README but then run the doctest on a wasm target will
get a different `ServerBuilder` type that doesn't expose the same chain. This is
not a bug per se — the README is aimed at native users — but it is a subtle trap
worth a one-line note.

The runnable examples `s23_mcp_tool_macro.rs` and `s24_mcp_prompt_macro.rs` both
declare `required-features = ["full"]` in `Cargo.toml:492,497`, which is the
correct contract for the tokio-backed `main` they use. The README's minimal
`features = ["macros"]` line will compile the doctest (because doctests are native)
but will not match what the "Full runnable example" links point at.

**Fix:** Add a one-line note after the installation block clarifying the target
and the runnable-example feature set:

```markdown
> The `macros` feature is the minimal surface needed for `rust,no_run` doctests
> on native targets. The linked runnable examples below use `--features full`
> because they spin up a tokio runtime; compile them with
> `cargo run --example s23_mcp_tool_macro --features full`.
```

Alternatively, upgrade the installation block to `features = ["full"]` to match
the example invocations shown in `s23_mcp_tool_macro.rs:13` and
`s24_mcp_prompt_macro.rs:13`.

## Info

### IN-01: Phase 66 CHANGELOG line-count claim drifts from sub-changelog

**File:** `pmcp-macros/CHANGELOG.md:30-31`

**Issue:** The 0.5.0 entry says "The `lib.rs` crate root shrank from 374 to 226
lines after deleting the four deprecated `pub fn` exports and their documentation."
The root `CHANGELOG.md:25` states "`lib.rs` crate root shrank from 374 to 226 lines."
Both match each other, but `pmcp-macros/src/lib.rs` as currently on disk is 227
lines (the `wc -l`-equivalent count of the Read output is 227). This is an off-by-one
between the CHANGELOG narrative and the file as shipped — likely because a
trailing newline or blank-line convention was tweaked after the line count was
recorded in the summary phase.

**Fix:** Low priority. Update both CHANGELOGs to say "227 lines" on the next
documentation pass, or drop the exact number (the "898 lines of deprecated/stub
source removed" figure is the meaningful one, not the current size). Not worth
blocking the release.

### IN-02: `pmcp-macros/Cargo.toml` dev-dependency pin uses `>=1.20.0` while this release bumps pmcp to 2.3.0

**File:** `pmcp-macros/Cargo.toml:27`

**Issue:** `pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }` was
a reasonable pin when `pmcp-macros` was being developed alongside pmcp 1.20 — but
with pmcp now at 2.3.0 and the macros crate at 0.5.0 containing v2.x-only
references (e.g., the README examples use `ServerBuilder::new().name(...).version(...)`
which is the v2.x uniform-constructor DX introduced in 2.0), the `>=1.20.0` floor
is misleading. On crates.io a user who pins `pmcp = "1.20"` and `pmcp-macros =
"0.5"` directly (instead of routing through the feature flag as the README
recommends) will get a compile failure against the macro-generated code, because
v1.20 predates the current `ServerBuilder` builder method set.

This is a dev-dependency, so it only affects `cargo test -p pmcp-macros` inside
the workspace (which uses `path = ".."` anyway and therefore resolves to the
in-tree 2.3.0). It is not a runtime hazard, but it under-documents the minimum
supported pmcp version.

**Fix:** Tighten the dev-dep floor to `>=2.3.0` (or `=2.3.0`) during the next
pmcp-macros bump. Since `path = ".."` takes precedence inside the workspace, this
is purely a documentation/metadata fix — it does not change what `cargo test -p
pmcp-macros` actually compiles against. Out of scope for this release if it would
trigger additional CI runs.

### IN-03: `pmcp-course/src/part1-foundations/ch01-03-why-rust.md` snippet keeps obsolete `#[arg(description = ...)]` parameter attributes

**File:** `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:116-133`

**Issue:** The "AI-generated MCP tool implementation" snippet uses per-parameter
`#[arg(description = "...")]` attributes:

```rust
#[mcp_tool(name = "query_sales", description = "Query sales data by region and quarter")]
async fn query_sales(
    #[arg(description = "Sales region (NA, EMEA, APAC)")]
    region: String,
    #[arg(description = "Quarter (Q1, Q2, Q3, Q4)")]
    quarter: String,
) -> Result<SalesReport, ToolError> { ... }
```

The real `#[mcp_tool]` contract (see `pmcp-macros/src/lib.rs:63-79` and README
line 56-78) expects a **single args struct** with the parameter descriptions
derived from struct-field doc comments via `JsonSchema`, not `#[arg(description = ...)]`
parameter attributes. An HTML comment on line 116 (`<!-- Illustrative only;
compile-ready form uses an args struct — see pmcp-macros README -->`) already
warns the reader, so this is consciously-marked illustrative code rather than
accidental drift.

Because the comment explicitly disclaims compilability, this is Info-severity,
not a Warning. The rest of the chapter is conceptual and the misleading syntax
does not appear anywhere else.

**Fix:** Optional. If the chapter is rewritten in a future phase, replace the
`#[arg(...)]` form with the real args-struct pattern, which is only marginally
longer:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct QuerySalesArgs {
    /// Sales region (NA, EMEA, APAC)
    region: String,
    /// Quarter (Q1, Q2, Q3, Q4)
    quarter: String,
}

#[mcp_tool(description = "Query sales data by region and quarter")]
async fn query_sales(args: QuerySalesArgs) -> pmcp::Result<SalesReport> {
    // ...
}
```

---

_Reviewed: 2026-04-11_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
