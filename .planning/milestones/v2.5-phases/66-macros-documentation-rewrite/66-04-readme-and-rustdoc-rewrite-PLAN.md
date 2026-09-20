---
phase: 66-macros-documentation-rewrite
plan: 04
type: execute
wave: 2
depends_on:
  - 66-01-poc-include-str-gate
  - 66-02-delete-deprecated-macros
files_modified:
  - pmcp-macros/README.md
  - pmcp-macros/src/lib.rs
  - examples/s23_mcp_tool_macro.rs
  - examples/s24_mcp_prompt_macro.rs
autonomous: true
requirements:
  - MACR-01
  - MACR-03
user_setup: []

must_haves:
  truths:
    - "pmcp-macros/README.md is a new ~200-300 line document targeted at new users landing on docs.rs/pmcp-macros"
    - "README documents all four macros: #[mcp_tool], #[mcp_server], #[mcp_prompt], #[mcp_resource]"
    - "Every README code block uses rust,no_run (never rust,ignore, never bare rust)"
    - "README installation section specifies pmcp = \"2.3\" with features = [\"macros\"] (not pmcp-macros as a direct dep)"
    - "README contains zero references to pmcp = \"1.*\" or any other stale version"
    - "README contains zero references to #[tool], #[tool_router], #[prompt] (the deleted macros)"
    - "The word \"migration\" does NOT appear in the README body (D-05 — migration lives in CHANGELOG)"
    - "cargo test -p pmcp-macros --doc passes after the README rewrite (proves every rust,no_run block compiles)"
    - "Per-macro /// doc comments on pub fn mcp_tool/mcp_server/mcp_prompt/mcp_resource are rewritten to reference s23/s24 renamed examples"
    - "Per-macro /// doc comments use rust,no_run (not rust,ignore)"
    - "examples/s23_mcp_tool_macro.rs:14 no longer says `cargo run --example 63_mcp_tool_macro`"
    - "examples/s24_mcp_prompt_macro.rs no longer says `cargo run --example 64_mcp_prompt_macro`"
    - "README `#[mcp_resource]` section explains URI template variable extraction (e.g. `{topic}` → `String` parameter)"
  artifacts:
    - path: "pmcp-macros/README.md"
      provides: "Rewritten canonical README documenting the four current macros"
      min_lines: 180
      contains: "#[mcp_tool]"
    - path: "pmcp-macros/src/lib.rs"
      provides: "Rewritten per-macro /// doc comments referencing renamed examples"
      contains: "s23_mcp_tool_macro"
    - path: "examples/s23_mcp_tool_macro.rs"
      provides: "Corrected example run-instructions header"
      contains: "cargo run --example s23_mcp_tool_macro"
    - path: "examples/s24_mcp_prompt_macro.rs"
      provides: "Corrected example run-instructions header"
      contains: "cargo run --example s24_mcp_prompt_macro"
  key_links:
    - from: "pmcp-macros/src/lib.rs include_str!"
      to: "pmcp-macros/README.md"
      via: "#![doc = include_str!(\"../README.md\")]"
      pattern: "include_str!\\(\"\\.\\./README\\.md\"\\)"
    - from: "README code block"
      to: "pmcp crate re-export"
      via: "use pmcp::{mcp_tool, mcp_server, mcp_prompt};"
      pattern: "use pmcp::\\{"
    - from: "README code block for #[mcp_resource]"
      to: "pmcp_macros direct import (re-export gap workaround)"
      via: "use pmcp_macros::mcp_resource;"
      pattern: "use pmcp_macros::mcp_resource"
---

<objective>
Rewrite `pmcp-macros/README.md` from scratch (~200-300 lines) as the definitive source of
documentation for the four `mcp_*` macros, plus rewrite the per-macro `///` doc comments on
`pub fn mcp_tool` / `mcp_server` / `mcp_prompt` / `mcp_resource` in `pmcp-macros/src/lib.rs` to
reference the renamed `s23_mcp_tool_macro.rs` / `s24_mcp_prompt_macro.rs` examples (D-11),
AND fix the two stale `cargo run --example 63_.../64_...` headers inside those example files
themselves (research addition #2).

Purpose: MACR-01 requires "pmcp-macros README rewritten to document #[mcp_tool], #[mcp_server],
#[mcp_prompt], #[mcp_resource] as primary APIs with working examples". MACR-03 requires the
README to be wired via `include_str!` so docs.rs renders it as the crate-level page. Wave 0
already wired the `include_str!` attribute and Wave 1 already deleted the obsolete surface;
this plan fills the page with the actual content.

The README is intentionally NOT a short "see main README" gateway (that would match
rmcp-macros' style but frustrate the target audience per D-05 — new users landing on
docs.rs/pmcp-macros directly). It is a self-contained ~200-300 line document with proportional
per-macro depth (D-07).

Output: A new `pmcp-macros/README.md` that compiles under `cargo test -p pmcp-macros --doc`,
rewrites per-macro `///` docs in `lib.rs` to match, and cleans up two stale `//!` headers in
the renamed example files.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md
@.planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md
@pmcp-macros/src/lib.rs
@pmcp-macros/README.md
@examples/s23_mcp_tool_macro.rs
@examples/s24_mcp_prompt_macro.rs
@pmcp-macros/src/mcp_tool.rs
@pmcp-macros/src/mcp_server.rs
@pmcp-macros/src/mcp_prompt.rs
@pmcp-macros/src/mcp_resource.rs

<interfaces>
<!-- Canonical API signatures the README must match. Extracted during planning. -->

Working examples on disk (authoritative for any code snippet the README shows):
- `examples/s23_mcp_tool_macro.rs` — `#[mcp_tool]` + `#[mcp_server]` showcase
- `examples/s24_mcp_prompt_macro.rs` — `#[mcp_prompt]` showcase

Key signature from `s23_mcp_tool_macro.rs`:
```rust
use pmcp::{ServerBuilder, ServerCapabilities, State, ToolHandler};
use pmcp_macros::{mcp_server, mcp_tool};   // ← note: example uses pmcp_macros:: directly
```

For the README (doctest context), use `use pmcp::{mcp_tool, mcp_server, mcp_prompt};` instead
because those three ARE re-exported from `pmcp::`. The example files can keep using
`pmcp_macros::` — they're standalone examples, not doctests.

`pmcp-macros/src/lib.rs` state at start of this plan (after Plan 02 completes):
- Line 1: `#![doc = include_str!("../README.md")]`  (from Wave 0, still present)
- Lines 3-4: `#[cfg(doctest)] pub struct ReadmeDoctests;` (from Wave 0)
- No `//!` comments at top of file (deleted in Plan 02)
- Lines ~9-17: `use` statements + `mod mcp_*;` declarations (clean)
- Four `pub fn mcp_*` declarations each with a `///` doc block that still references
  OLD example filenames like `63_mcp_tool_macro` or `64_mcp_prompt_macro` (stale leftover
  from Phase 65 rename). Each `///` doc block has a `rust,ignore` code fence. Both need
  updating.

The `#[mcp_resource]` re-export gap (Pitfall 4 in research):
- `pmcp/src/lib.rs:147` → `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};` (verified)
- `mcp_resource` is NOT re-exported. This is deferred per D-03.
- README `#[mcp_resource]` section MUST use `use pmcp_macros::mcp_resource;` with an
  explanatory inline comment per Pitfall 4 option 1 recommendation.
- Wave 0 POC Block B already empirically validated this import path compiles. If the POC
  failed (executor flipped `wave_0_complete: false` in 66-VALIDATION.md), fall back to
  Pitfall 4 option 3: document `#[mcp_resource]` without a full compiling example, using
  prose + an HTML comment showing the attribute usage pattern.

Stale example headers to fix (research addition #2):
- `examples/s23_mcp_tool_macro.rs:14` currently reads:
  `//! cargo run --example 63_mcp_tool_macro --features full`
  Should be:
  `//! cargo run --example s23_mcp_tool_macro --features full`
- `examples/s24_mcp_prompt_macro.rs` has a similar stale header:
  `//! cargo run --example 64_mcp_prompt_macro --features full`
  Should be:
  `//! cargo run --example s24_mcp_prompt_macro --features full`
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Rewrite pmcp-macros/README.md from scratch with all four macros documented</name>
  <files>pmcp-macros/README.md</files>

  <read_first>
    - pmcp-macros/README.md (CURRENT state — the POC placeholder from Wave 0 — needs complete overwrite)
    - pmcp-macros/src/mcp_tool.rs (skim — confirm attribute surface: `name`, `description`,
      `annotations(...)`, `ui = "..."`, State injection, async/sync auto-detect)
    - pmcp-macros/src/mcp_server.rs (skim — confirm `#[mcp_server]` on `impl` block)
    - pmcp-macros/src/mcp_prompt.rs (skim — confirm attribute surface)
    - pmcp-macros/src/mcp_resource.rs (skim — confirm the exact attributes:
      check if it's `uri = "..."`, `uri_template = "..."`, or some other name; exact
      `description` requirement; async vs sync; ALSO confirm the URI-template variable
      extraction behaviour described in the README content requirements below — grep the
      file for `template` / `{` parsing to see which variable-name rules apply)
    - examples/s23_mcp_tool_macro.rs (FULL read — authoritative example for showcase section)
    - examples/s24_mcp_prompt_macro.rs (FULL read — authoritative example for prompt section)
    - pmcp/src/lib.rs line 147 (verify the re-export line still
      reads `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};`)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md sections:
      `## Implementation Decisions` D-05 through D-11 (the README decision set)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md sections:
      "Pattern 1" (include_str! wiring),
      "Pattern 2" (rust,no_run code blocks),
      "Pitfall 4" (mcp_resource re-export asymmetry),
      "Pitfall 3" (no relative file links — use absolute GitHub URLs),
      "Code Examples" (verified working rust,no_run block template),
      "rmcp Benchmark" (what to match, what to diverge from)
    - .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md (for the `wave_0_complete` status — if false, use Pitfall 4 option 3 fallback)
  </read_first>

  <action>
    Overwrite `pmcp-macros/README.md` with a new document using EXACTLY the section structure
    below. The executor is free to edit prose for tone and clarity but MUST preserve:
    - Every heading listed below (same level, same text)
    - The code-block count indicated in each section
    - The `rust,no_run` fence on every code block
    - The `use pmcp::{...};` imports in sections 2-4 and `use pmcp_macros::mcp_resource;` in
      section 5 (or the fallback per Wave 0 POC result)
    - The installation pin `pmcp = "2.3"` with `features = ["macros"]`
    - Zero occurrences of the words "migration", "deprecated", "#[tool]", "#[tool_router]",
      "#[prompt]", "#[resource]", or version strings starting with "1."
    - All links to other files MUST use absolute GitHub URLs
      (e.g. `https://github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs`) NOT
      relative paths (Pitfall 3)
    - The `#[mcp_resource]` section MUST include a "URI template variables" sub-section (see
      Section 5 content requirements below) that explains how variables like `{topic}` inside
      a URI template are automatically extracted by the macro and bound to function parameters
      of the same name with type `String`. This is a key feature of `#[mcp_resource]` that the
      external plan review flagged as missing. The words "URI template" and a literal `{topic}`
      (or equivalent `{variable_name}` token) must appear in the rendered README.

    ---

    **MANDATORY SECTION STRUCTURE (copy headings verbatim, fill prose + examples):**

    ```
    # pmcp-macros

    <one paragraph summary, ~2-3 sentences: "procedural macros that eliminate boilerplate for
    MCP tools, prompts, resources, and server routers. Used by pmcp via the `macros` feature.
    All four macros integrate with compile-time schema generation and pmcp's handler registry.">

    ## Installation

    Add `pmcp` with the `macros` feature. This pulls `pmcp-macros` transitively — you don't
    need to add it as a direct dependency.

    \`\`\`toml
    [dependencies]
    pmcp = { version = "2.3", features = ["macros"] }
    schemars = "1.0"
    serde = { version = "1.0", features = ["derive"] }
    tokio = { version = "1.46", features = ["full"] }
    \`\`\`

    ## Overview

    pmcp-macros provides four attribute macros:

    | Macro            | Applied to      | Purpose                                       |
    | ---------------- | --------------- | --------------------------------------------- |
    | `#[mcp_tool]`    | `async fn` or `fn` | Define a tool handler with a typed arg struct and compile-time schema |
    | `#[mcp_server]`  | `impl` block    | Collect tools/prompts/resources on a type into a single registerable `McpServer` |
    | `#[mcp_prompt]`  | `async fn` or `fn` | Define a prompt template with typed arguments |
    | `#[mcp_resource]`| `async fn`      | Define a resource handler with URI pattern matching |

    Each macro generates the glue code and schema wiring that MCP servers need, so your
    handler code stays focused on behavior instead of protocol plumbing.

    ## `#[mcp_tool]`

    ### Purpose
    <1-2 sentences: what it does, why it exists, key wins over hand-rolled TypedTool>

    ### Attributes
    - `description = "..."` — required. The human-readable tool description.
    - `name = "..."` — optional. Defaults to the function name.
    - `annotations(...)` — optional. MCP tool annotations (read_only, destructive, idempotent,
      etc.).
    - (any other attributes the executor verifies from `mcp_tool.rs`)

    ### Example

    \`\`\`rust,no_run
    use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, JsonSchema)]
    struct AddArgs {
        /// First addend
        a: f64,
        /// Second addend
        b: f64,
    }

    #[derive(Debug, Serialize, JsonSchema)]
    struct AddResult {
        sum: f64,
    }

    #[mcp_tool(description = "Add two numbers")]
    async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult { sum: args.a + args.b })
    }

    #[tokio::main]
    async fn main() -> pmcp::Result<()> {
        let _server = ServerBuilder::new()
            .name("calculator")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .tool("add", add())
            .build()?;
        Ok(())
    }
    \`\`\`

    ### Shared state (State<T>)

    <Brief explanation of how State<T> injection works — 3-5 sentences. Then a second
    `rust,no_run` block showing a tool that takes `args: FooArgs, db: State<MyDatabase>`.>

    ### Full runnable example

    See `examples/s23_mcp_tool_macro.rs` on GitHub:
    https://github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs

    ## `#[mcp_server]`

    ### Purpose
    <2-3 sentences: turns an `impl` block into a server bundle, auto-registers every
    `#[mcp_tool]` / `#[mcp_prompt]` / `#[mcp_resource]` defined on the impl, wires
    `State<T>` across all of them.>

    ### Example

    \`\`\`rust,no_run
    use pmcp::{mcp_server, mcp_tool, ServerBuilder, ServerCapabilities, State};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;

    struct Calculator;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct AddArgs { a: f64, b: f64 }

    #[derive(Debug, Serialize, JsonSchema)]
    struct AddResult { sum: f64 }

    #[mcp_server]
    impl Calculator {
        #[mcp_tool(description = "Add two numbers")]
        async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
            Ok(AddResult { sum: args.a + args.b })
        }
    }

    #[tokio::main]
    async fn main() -> pmcp::Result<()> {
        let calculator = Arc::new(Calculator);
        let _server = ServerBuilder::new()
            .name("calculator")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .mcp_server(calculator)
            .build()?;
        Ok(())
    }
    \`\`\`

    ### Full runnable example

    See `examples/s23_mcp_tool_macro.rs` (same file — demonstrates `#[mcp_server]`
    alongside `#[mcp_tool]`):
    https://github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs

    ## `#[mcp_prompt]`

    ### Purpose
    <2-3 sentences: why it's better than raw PromptHandler — eliminates
    `HashMap::get("x").ok_or()?.parse()?` boilerplate>

    ### Attributes
    - `description = "..."` — required
    - `name = "..."` — optional
    - (any other attributes from mcp_prompt.rs)

    ### Example

    \`\`\`rust,no_run
    use pmcp::{mcp_prompt, types::GetPromptResult};
    // ... rest of the example, ~15 lines, following the s24 file's structure
    \`\`\`

    ### Full runnable example

    https://github.com/paiml/pmcp/blob/main/examples/s24_mcp_prompt_macro.rs

    ## `#[mcp_resource]`

    > **Note:** For technical reasons, `#[mcp_resource]` is currently imported directly from
    > `pmcp_macros` rather than re-exported via `pmcp`. A future `pmcp` release will add it
    > to the re-export alongside the other three macros, at which point this import can
    > become `use pmcp::mcp_resource;`.

    ### Purpose
    <2-3 sentences>

    ### Attributes
    <confirmed from mcp_resource.rs — list each supported attribute with required/optional>

    ### URI template variables

    `#[mcp_resource]` accepts a URI template on the attribute (for example
    `data://articles/{topic}`). Any `{variable_name}` placeholder inside the template is
    **automatically extracted at request time and passed to the decorated function as a
    parameter of type `String`**. The parameter name must match the template variable name
    exactly. You don't have to parse the URI yourself — the macro handles pattern-matching
    the incoming URI against the template and binding the captured substrings.

    For example, a resource declared as `#[mcp_resource(uri_template = "data://articles/{topic}", ...)]`
    on `async fn get_article(topic: String) -> pmcp::Result<String>` will receive the `topic`
    segment of a request URI like `data://articles/rust-macros` bound directly to the
    `topic: String` parameter.

    ### Example

    \`\`\`rust,no_run
    use pmcp_macros::mcp_resource;

    // <full example, ~10-15 lines, following whatever signature mcp_resource.rs requires.
    //  The example MUST use a URI template containing at least one `{variable_name}`
    //  placeholder and show the function signature receiving that variable as a `String`
    //  parameter with the same name.>
    \`\`\`

    ## Feature flags

    The `pmcp` `macros` feature flag exists to give users who don't need the proc-macro
    machinery a smaller compile surface. If you want any of the four macros, enable `macros`.

    ## License

    MIT — see https://github.com/paiml/pmcp/blob/main/LICENSE
    ```

    ---

    **Wave 0 POC fallback path:**

    If `66-VALIDATION.md` has `wave_0_complete: false` (meaning Block B in the POC failed to
    compile), replace the `#[mcp_resource]` section's code block with:

    ```
    ### Example (usage pattern)

    Apply `#[mcp_resource(...)]` to an async function that takes a URI parameter and
    returns `pmcp::Result<String>`. Because of a re-export gap that will be fixed in a
    future `pmcp` release, a compiling example requires importing from `pmcp_macros`
    directly. See `examples/` for a full runnable demo.
    ```

    and drop the `rust,no_run` block entirely for that section. This is Pitfall 4 option 3.
    The other three macro sections remain as specified above. IMPORTANT: even in the fallback
    path, the `### URI template variables` sub-section MUST still be present — it is prose
    explaining a feature, not a compiling demo, and it does not depend on the POC outcome.

    ---

    **Verification loop during authoring:**

    After writing the full README, run `cargo test -p pmcp-macros --doc` and watch for errors.
    Because this is edition 2021 (Pitfall 5), error messages may point to
    `pmcp-macros/src/lib.rs:1` even though the real issue is in README.md. Grep the README
    for the failing symbol when diagnosing. Iterate until `cargo test --doc` is green.

    ---

    **Critical DO-NOTs:**
    - Do NOT include a "Migration" section (D-05)
    - Do NOT reference `#[tool]`, `#[tool_router]`, `#[prompt]`, `#[resource]`
    - Do NOT use `rust,ignore` or bare `rust` — only `rust,no_run` for Rust blocks
    - Do NOT write relative file links (Pitfall 3) — use absolute GitHub URLs
    - Do NOT add `pmcp-macros` as a direct dep in the installation example (D-08)
    - Do NOT use `pmcp = "1.*"` or any v1 version string (Pitfall 2)
    - Do NOT invoke `include_str!` or `include_bytes!` inside a README code block (Pitfall 1)
    - Do NOT use GitHub-flavored admonitions like `> [!NOTE]` (Pitfall 3)
    - Do NOT use the word `migration` anywhere in the README body
    - Do NOT omit the URI template variables sub-section from the `#[mcp_resource]` section
      — external plan review flagged its absence as a gap

    Commit with subject:
    `docs(66): rewrite pmcp-macros/README.md for current macro API (MACR-01, MACR-03)`
  </action>

  <acceptance_criteria>
    - File `pmcp-macros/README.md` exists and is ≥ 180 lines (verify: `[ $(wc -l < pmcp-macros/README.md) -ge 180 ]`)
    - File contains heading `# pmcp-macros` at line 1 (verify: `head -1 pmcp-macros/README.md | grep -q '^# pmcp-macros'`)
    - File contains heading `## Installation` (verify: `grep -q '^## Installation' pmcp-macros/README.md`)
    - File contains heading `## \`#\[mcp_tool\]\`` (verify: `grep -q '## `#\[mcp_tool\]`' pmcp-macros/README.md`)
    - File contains heading `## \`#\[mcp_server\]\`` (verify: `grep -q '## `#\[mcp_server\]`' pmcp-macros/README.md`)
    - File contains heading `## \`#\[mcp_prompt\]\`` (verify: `grep -q '## `#\[mcp_prompt\]`' pmcp-macros/README.md`)
    - File contains heading `## \`#\[mcp_resource\]\`` (verify: `grep -q '## `#\[mcp_resource\]`' pmcp-macros/README.md`)
    - File contains at least 4 `rust,no_run` fenced blocks (one per macro minimum; `#[mcp_tool]` section may have 2)
      (verify: `[ $(grep -c '^```rust,no_run' pmcp-macros/README.md) -ge 4 ]`)
    - File contains zero `rust,ignore` blocks (verify: `! grep -q '^```rust,ignore' pmcp-macros/README.md`)
    - File contains zero bare `rust` blocks (verify: `! grep -qE '^```rust$' pmcp-macros/README.md`)
    - File contains `pmcp = "2.3"` or `pmcp = { version = "2.3"` (verify: `grep -qE 'pmcp *= *"2\.3"|pmcp *= *\{ *version *= *"2\.3"' pmcp-macros/README.md`)
    - File contains no `pmcp = "1.` or `pmcp = { version = "1.` patterns (verify: `! grep -qE 'pmcp *= *"1\.|pmcp *= *\{ *version *= *"1\.' pmcp-macros/README.md`)
    - File contains `use pmcp::{` import in at least one block (verify: `grep -q 'use pmcp::{' pmcp-macros/README.md`)
    - File contains no `#[tool(` pattern (verify: `! grep -qF '#[tool(' pmcp-macros/README.md`)
    - File contains no `#[tool_router` pattern (verify: `! grep -qF '#[tool_router' pmcp-macros/README.md`)
    - File contains no `#[prompt(` pattern (verify: `! grep -qF '#[prompt(' pmcp-macros/README.md`)
    - File contains no `#[resource(` pattern (verify: `! grep -qF '#[resource(' pmcp-macros/README.md`)
    - File contains zero occurrences of the word `migration` in body
      (verify: `! grep -qi 'migration' pmcp-macros/README.md`)
    - File contains zero occurrences of the word `deprecated`
      (verify: `! grep -qi 'deprecated' pmcp-macros/README.md`)
    - File contains absolute GitHub URL to s23_mcp_tool_macro.rs (verify:
      `grep -q 'github.com/paiml/pmcp/blob/main/examples/s23_mcp_tool_macro.rs' pmcp-macros/README.md`)
    - File contains absolute GitHub URL to s24_mcp_prompt_macro.rs (verify:
      `grep -q 'github.com/paiml/pmcp/blob/main/examples/s24_mcp_prompt_macro.rs' pmcp-macros/README.md`)
    - File `#[mcp_resource]` section explains URI template variable extraction. The README
      must contain the literal text `URI template` AND a `{variable_name}`-shaped token
      (verify: `grep -q 'URI template' pmcp-macros/README.md && grep -qE '\{[a-z_]+\}' pmcp-macros/README.md`)
    - `cargo test -p pmcp-macros --doc` exits 0 (every rust,no_run block compiles)
  </acceptance_criteria>

  <verify>
    <automated>[ $(wc -l < pmcp-macros/README.md) -ge 180 ] && head -1 pmcp-macros/README.md | grep -q '^# pmcp-macros' && grep -q '^## Installation' pmcp-macros/README.md && [ $(grep -c '^```rust,no_run' pmcp-macros/README.md) -ge 4 ] && ! grep -q '^```rust,ignore' pmcp-macros/README.md && ! grep -qE '^```rust$' pmcp-macros/README.md && ! grep -qE 'pmcp *= *"1\.' pmcp-macros/README.md && ! grep -qi 'migration' pmcp-macros/README.md && ! grep -qi 'deprecated' pmcp-macros/README.md && ! grep -qF '#[tool(' pmcp-macros/README.md && ! grep -qF '#[tool_router' pmcp-macros/README.md && grep -q 'URI template' pmcp-macros/README.md && grep -qE '\{[a-z_]+\}' pmcp-macros/README.md && cargo test -p pmcp-macros --doc 2>&1 | tail -20</automated>
  </verify>

  <done>
    `pmcp-macros/README.md` is a ~200-300 line document following the exact section structure
    above, with all four macros documented using `rust,no_run` blocks that compile under
    `cargo test -p pmcp-macros --doc`. The `#[mcp_resource]` section includes a "URI template
    variables" sub-section explaining automatic `{variable_name}` → `String` parameter
    extraction. All acceptance criteria pass.
  </done>
</task>

<task type="auto">
  <name>Task 2: Rewrite per-macro /// doc comments on pub fn mcp_tool/mcp_server/mcp_prompt/mcp_resource in lib.rs, fix stale example headers in s23/s24 files</name>
  <files>pmcp-macros/src/lib.rs, examples/s23_mcp_tool_macro.rs, examples/s24_mcp_prompt_macro.rs</files>

  <read_first>
    - pmcp-macros/src/lib.rs (full read — need to see current state of all four `///` blocks)
    - examples/s23_mcp_tool_macro.rs line 14 (verify stale `63_mcp_tool_macro` text)
    - examples/s24_mcp_prompt_macro.rs around line 14 (verify stale `64_mcp_prompt_macro` text)
    - pmcp-macros/README.md (the just-rewritten file — `///` docs reference it)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-11)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md "Open Question 1" (the
      stale-header fix)
  </read_first>

  <action>
    **Part A: lib.rs per-macro `///` doc rewrites (D-11)**

    For each of the four `pub fn mcp_*` declarations in `pmcp-macros/src/lib.rs`, locate its
    preceding `///` doc comment block and rewrite it to:
    1. Give a one-line summary of the macro's purpose
    2. List its attributes (matching the README)
    3. Include a SHORT `rust,no_run` example (NOT a full working main() — the README has the
       full version). The per-macro `///` example is the "see at a glance" form; 5-10 lines.
    4. Link to `examples/s23_mcp_tool_macro.rs` or `s24_mcp_prompt_macro.rs` as appropriate
       for the full runnable demo
    5. Use `rust,no_run` fences, never `rust,ignore`
    6. Use `use pmcp::{mcp_tool, ...};` imports for mcp_tool/mcp_server/mcp_prompt
    7. Use `use pmcp_macros::mcp_resource;` for mcp_resource (per Pitfall 4 workaround), with
       an inline comment `// Note: direct import until re-export gap is closed`
    8. Reference ONLY `s23_mcp_tool_macro` and `s24_mcp_prompt_macro` — NOT the old `63_`/`64_`
       prefixes

    Skeleton per macro (adapt and fill):

    ```rust
    /// Defines a tool handler with a typed argument struct and compile-time schema.
    ///
    /// # Attributes
    /// - `description = "..."` — required. Human-readable description.
    /// - `name = "..."` — optional. Defaults to the function name.
    /// - `annotations(...)` — optional. MCP tool annotations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use pmcp::mcp_tool;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, JsonSchema)]
    /// struct AddArgs { a: f64, b: f64 }
    ///
    /// #[derive(Debug, Serialize, JsonSchema)]
    /// struct AddResult { sum: f64 }
    ///
    /// #[mcp_tool(description = "Add two numbers")]
    /// async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    ///     Ok(AddResult { sum: args.a + args.b })
    /// }
    /// ```
    ///
    /// See `examples/s23_mcp_tool_macro.rs` for a complete runnable demo.
    #[proc_macro_attribute]
    pub fn mcp_tool(args: TokenStream, input: TokenStream) -> TokenStream {
        // ... body unchanged ...
    }
    ```

    Repeat for `pub fn mcp_server`, `pub fn mcp_prompt`, `pub fn mcp_resource`. The
    `mcp_resource` variant uses `use pmcp_macros::mcp_resource;` in the example block with
    the inline comment.

    Do NOT modify the `pub fn` signatures or bodies — only the `///` doc block preceding
    each one.

    Do NOT modify `#![doc = include_str!(...)]` or `#[cfg(doctest)] pub struct ReadmeDoctests;`.

    **Part B: s23 and s24 stale example header fixes (research addition #2)**

    Edit `examples/s23_mcp_tool_macro.rs`:
    - Line 14 currently reads: `//! cargo run --example 63_mcp_tool_macro --features full`
    - Replace with:          `//! cargo run --example s23_mcp_tool_macro --features full`

    Edit `examples/s24_mcp_prompt_macro.rs`:
    - Similar stale line with `64_mcp_prompt_macro` (read to confirm line number)
    - Replace with the `s24_mcp_prompt_macro` form

    Grep both files for any other occurrence of `63_mcp_tool_macro` or `64_mcp_prompt_macro`
    and fix them too. The intent is to leave zero references to the old example numbers.

    **Verification:**

    Run `cargo test -p pmcp-macros --doc` — all four per-macro `///` doctests must compile
    alongside the README doctests.

    Run `cargo build --example s23_mcp_tool_macro --features full` and
    `cargo build --example s24_mcp_prompt_macro --features full` to confirm the example
    files themselves still compile (no breakage from the header comment edit).

    Commit with subject:
    `docs(66): rewrite per-macro /// docs + fix stale example headers (D-11)`
  </action>

  <acceptance_criteria>
    - `pmcp-macros/src/lib.rs` has zero `rust,ignore` fences in `///` blocks
      (verify: `! grep -q 'rust,ignore' pmcp-macros/src/lib.rs`)
    - `pmcp-macros/src/lib.rs` has at least 4 `rust,no_run` fences
      (verify: `[ $(grep -c 'rust,no_run' pmcp-macros/src/lib.rs) -ge 4 ]`)
    - `pmcp-macros/src/lib.rs` references `s23_mcp_tool_macro` (not `63_mcp_tool_macro`)
      (verify: `grep -q 's23_mcp_tool_macro' pmcp-macros/src/lib.rs && ! grep -q '63_mcp_tool_macro' pmcp-macros/src/lib.rs`)
    - `pmcp-macros/src/lib.rs` references `s24_mcp_prompt_macro` (not `64_mcp_prompt_macro`)
      (verify: `grep -q 's24_mcp_prompt_macro' pmcp-macros/src/lib.rs && ! grep -q '64_mcp_prompt_macro' pmcp-macros/src/lib.rs`)
    - `examples/s23_mcp_tool_macro.rs` contains `s23_mcp_tool_macro` in the `//!` header
      (verify: `grep -q 'cargo run --example s23_mcp_tool_macro' examples/s23_mcp_tool_macro.rs`)
    - `examples/s23_mcp_tool_macro.rs` contains no `63_mcp_tool_macro` anywhere
      (verify: `! grep -q '63_mcp_tool_macro' examples/s23_mcp_tool_macro.rs`)
    - `examples/s24_mcp_prompt_macro.rs` contains `s24_mcp_prompt_macro` in the `//!` header
      (verify: `grep -q 'cargo run --example s24_mcp_prompt_macro' examples/s24_mcp_prompt_macro.rs`)
    - `examples/s24_mcp_prompt_macro.rs` contains no `64_mcp_prompt_macro` anywhere
      (verify: `! grep -q '64_mcp_prompt_macro' examples/s24_mcp_prompt_macro.rs`)
    - `cargo test -p pmcp-macros --doc` exits 0
    - `cargo build --example s23_mcp_tool_macro --features full` exits 0
    - `cargo build --example s24_mcp_prompt_macro --features full` exits 0
    - `cargo build -p pmcp-macros` exits 0
  </acceptance_criteria>

  <verify>
    <automated>! grep -q 'rust,ignore' pmcp-macros/src/lib.rs && [ $(grep -c 'rust,no_run' pmcp-macros/src/lib.rs) -ge 4 ] && grep -q 's23_mcp_tool_macro' pmcp-macros/src/lib.rs && ! grep -q '63_mcp_tool_macro' pmcp-macros/src/lib.rs && grep -q 's24_mcp_prompt_macro' pmcp-macros/src/lib.rs && ! grep -q '64_mcp_prompt_macro' pmcp-macros/src/lib.rs && grep -q 'cargo run --example s23_mcp_tool_macro' examples/s23_mcp_tool_macro.rs && ! grep -q '63_mcp_tool_macro' examples/s23_mcp_tool_macro.rs && grep -q 'cargo run --example s24_mcp_prompt_macro' examples/s24_mcp_prompt_macro.rs && ! grep -q '64_mcp_prompt_macro' examples/s24_mcp_prompt_macro.rs && cargo test -p pmcp-macros --doc 2>&1 | tail -10 && cargo build --example s23_mcp_tool_macro --features full 2>&1 | tail -3 && cargo build --example s24_mcp_prompt_macro --features full 2>&1 | tail -3</automated>
  </verify>

  <done>
    All four `pub fn mcp_*` declarations in `pmcp-macros/src/lib.rs` have rewritten `///`
    doc comments matching the README content style, using `rust,no_run` and referencing
    the renamed example files. The two stale example-name headers in `s23_mcp_tool_macro.rs`
    and `s24_mcp_prompt_macro.rs` are fixed. Both the macros crate and the two examples still
    compile. Combined with Task 1, the entire `cargo test -p pmcp-macros --doc` suite is green.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| None | Documentation rewrite + comment fix. No runtime surface, no input parsing, no auth. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-66-07 | Tampering | User copy-pastes README example that doesn't compile | mitigate | Every code block uses `rust,no_run` and runs under `cargo test -p pmcp-macros --doc`. API drift breaks doctests, not users. |
| T-66-08 | Information Disclosure | README exposes internal `pmcp_macros` import path for `mcp_resource` | accept | Documented in the "Note:" above the `#[mcp_resource]` section. Transparent about the re-export gap. |
| T-66-09 | Tampering (of user's understanding) | README references wrong pmcp version | mitigate | Acceptance criteria explicitly blocks `pmcp = "1.*"` patterns and requires `pmcp = "2.3"`. `cargo test --doc` catches version drift (D-08). |

Docs-only change — security surface is zero.
</threat_model>

<verification>
- `cargo test -p pmcp-macros --doc` passes (README + per-macro doctests all compile)
- `cargo build -p pmcp-macros` passes
- `cargo build --example s23_mcp_tool_macro --features full` passes
- `cargo build --example s24_mcp_prompt_macro --features full` passes
- README has all mandatory headings and meets length / content acceptance criteria
- README `#[mcp_resource]` section explains URI template variable extraction (`URI template`
  + `{variable_name}` tokens both present)
- lib.rs `///` docs reference `s23_mcp_tool_macro` / `s24_mcp_prompt_macro`
- Stale `63_` / `64_` references removed from example file headers
</verification>

<success_criteria>
- MACR-01: pmcp-macros README rewritten — all four macros documented with compiling `rust,no_run` examples
- MACR-03: pmcp-macros/src/lib.rs continues to use `include_str!("../README.md")` (wired in Wave 0, preserved here)
- D-05: No "migration" section in README
- D-07: Proportional macro depth — `#[mcp_tool]` is showcase, other three get focused sections
- D-08: Installation uses `pmcp = "2.3"` with `features = ["macros"]`
- D-09: Every code block uses `rust,no_run`; zero `rust,ignore`
- D-11: Per-macro `///` docs rewritten, reference s23/s24 renamed examples
- Research addition #2: Stale `63_`/`64_` references in example file `//!` headers fixed
- Gemini review fold-in: README `#[mcp_resource]` section explains URI template variable
  extraction (the `{variable_name}` → `String` parameter pattern)
</success_criteria>

<output>
After completion, create `.planning/phases/66-macros-documentation-rewrite/66-04-readme-and-rustdoc-rewrite-SUMMARY.md`
documenting: (a) commit SHA, (b) final README line count, (c) total rust,no_run block count
in README, (d) whether Wave 0 POC Block B fallback was needed (i.e. whether the mcp_resource
section got a compiling example or the Pitfall 4 option 3 fallback), (e) `cargo test --doc`
output confirmation, (f) any README prose decisions that deviated from the skeleton (e.g.
section order changes, additional sub-sections).
</output>
