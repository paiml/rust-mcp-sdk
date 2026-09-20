---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 03
type: execute
wave: 2
depends_on:
  - 67-02
files_modified:
  - CRATE-README.md
  - src/lib.rs
autonomous: true
requirements:
  - DRSD-03
  - DOCD-02
tags:
  - rust
  - rustdoc
  - docs-rs
  - documentation
must_haves:
  truths:
    - "CRATE-README.md exists at repo root (~150–250 lines)"
    - "src/lib.rs top uses `#![doc = include_str!(\"../CRATE-README.md\")]` instead of the inline `//!` module doc"
    - "Lines 63–77 of src/lib.rs (lint block) are preserved intact"
    - "CRATE-README.md contains a Quick Start section with Client + Server examples as `rust,no_run` doctests (verbatim move from former src/lib.rs:14-61)"
    - "CRATE-README.md contains a 3-column Cargo Features table with 18 rows (2 meta + 16 individual features alphabetized, including logging)"
    - "`cargo test --doc --features full` passes (doctests still compile)"
    - "`cargo package --list --allow-dirty` includes CRATE-README.md at repo root"
  artifacts:
    - path: "CRATE-README.md"
      provides: "Crate-level rustdoc landing page for docs.rs"
      min_lines: 150
    - path: "src/lib.rs"
      provides: "Crate root with include_str! doc attribute and preserved lint block"
      contains: "include_str!"
  key_links:
    - from: "src/lib.rs"
      to: "CRATE-README.md"
      via: "include_str!(\"../CRATE-README.md\")"
      pattern: "include_str!\\(\"\\.\\./CRATE-README\\.md\"\\)"
---

<objective>
Create `CRATE-README.md` at repo root (new file) and replace the inline `//!`-prefixed module doc at `src/lib.rs:1-61` with a single `#![doc = include_str!("../CRATE-README.md")]` attribute. This is the same pattern Phase 66 landed for `pmcp-macros` (see `pmcp-macros/src/lib.rs:6`). Structure of `CRATE-README.md` follows D-07 (crate intro + Quick Start + Cargo Features table + short pointers). The Quick Start code blocks are moved **verbatim** from the current `src/lib.rs:14-61` — no TypedToolWithOutput refactor (that's PLSH-01 / Phase 68).

Purpose: Single source of truth for crate-level docs. The same file renders on docs.rs and is the file contributors edit — no drift between "the README" and "what docs.rs shows." Adds the feature flag table that D-11/D-12/D-13 specify (18 rows = 2 meta + 16 individual, 3 columns: Feature / Description / Enables). This plan also pulls deferred requirement DOCD-02 into scope as an explicit consequence of D-04/D-06.

Output: New `CRATE-README.md` at repo root. `src/lib.rs` lines 1–61 replaced by `#![doc = include_str!("../CRATE-README.md")]` (keeping lines 63–77 lint block intact). `cargo test --doc --features full` passes. `cargo package --list --allow-dirty` includes the new file.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@src/lib.rs
@pmcp-macros/src/lib.rs
@pmcp-macros/README.md
@Cargo.toml

<interfaces>
<!-- Verbatim text the Client + Server Quick Start blocks to move from src/lib.rs:14-61. -->
<!-- Copy EXACTLY — do NOT touch types, imports, or body. Any rewrite to TypedToolWithOutput is out of scope (PLSH-01). -->

Current `src/lib.rs:14-31` (Client Example):
```rust
//! ```rust
//! use pmcp::{Client, StdioTransport, ClientCapabilities};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client with stdio transport
//! let transport = StdioTransport::new();
//! let mut client = Client::new(transport);
//!
//! // Initialize the connection
//! let server_info = client.initialize(ClientCapabilities::default()).await?;
//!
//! // List available tools
//! let tools = client.list_tools(None).await?;
//! # Ok(())
//! # }
//! ```
```

Current `src/lib.rs:35-61` (Server Example):
```rust
//! ```rust
//! use pmcp::{Server, ServerCapabilities, ToolHandler};
//! use async_trait::async_trait;
//! use serde_json::Value;
//!
//! struct MyTool;
//!
//! #[async_trait]
//! impl ToolHandler for MyTool {
//!     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
//!         Ok(serde_json::json!({"result": "success"}))
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Server::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .capabilities(ServerCapabilities::default())
//!     .tool("my-tool", MyTool)
//!     .build()?;
//!
//! // Run with stdio transport
//! server.run_stdio().await?;
//! # Ok(())
//! # }
//! ```
```

In the `CRATE-README.md` file, strip the `//! ` prefix and change the fence from ```` ```rust ```` to ```` ```rust,no_run ```` (D-09: `rust,no_run` is default, `rust,ignore` is forbidden).

<!-- Feature transitive deps (from RESEARCH.md Feature Flag Expansion table) -->
<!-- Used to populate the "Enables" column of the Cargo Features table. -->

| Feature | Direct deps / transitive features | Enables column |
|---|---|---|
| `default` | `["logging"]` | `tracing-subscriber` |
| `full` | `["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros"]` | Everything below — single switch |
| `composition` | `["streamable-http"]` | (via streamable-http) |
| `http` | `["dep:hyper", "dep:hyper-util", "dep:bytes"]` | `hyper`, `hyper-util`, `bytes` |
| `http-client` | `["dep:reqwest"]` | `reqwest` |
| `jwt-auth` | `["http-client", "dep:jsonwebtoken"]` | `jsonwebtoken` + `http-client` |
| `logging` | `["dep:tracing-subscriber"]` | `tracing-subscriber` |
| `macros` | `["dep:pmcp-macros", "schema-generation"]` | `pmcp-macros`, `schemars` |
| `mcp-apps` | `[]` | UI types only (code-gate, no deps) |
| `oauth` | `["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]` | `webbrowser`, `dirs`, `rand` + `http-client` |
| `rayon` | `["dep:rayon"]` | `rayon` |
| `resource-watcher` | `["dep:notify", "dep:glob-match"]` | `notify`, `glob-match` |
| `schema-generation` | `["dep:schemars"]` | `schemars` |
| `simd` | `[]` | SIMD JSON parsing (code-gate, no deps) |
| `sse` | `["http-client", "dep:bytes"]` | `bytes` + `http-client` |
| `streamable-http` | `["dep:hyper", "dep:hyper-util", "dep:hyper-rustls", "dep:rustls", "dep:futures-util", "dep:bytes", "dep:axum", "dep:tower", "dep:tower-http"]` | `hyper`, `hyper-util`, `hyper-rustls`, `rustls`, `axum`, `tower`, `tower-http`, `futures-util`, `bytes` |
| `validation` | `["dep:jsonschema", "dep:garde"]` | `jsonschema`, `garde` |
| `websocket` | `["dep:tokio-tungstenite"]` | `tokio-tungstenite` |
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
  <name>Task 1: Create CRATE-README.md at repo root</name>
  <files>CRATE-README.md</files>
  <read_first>
    - src/lib.rs lines 1–61 (to copy the Quick Start code blocks verbatim)
    - pmcp-macros/src/lib.rs (include_str! pattern reference)
    - pmcp-macros/README.md (structural template — note pmcp-macros/README has no feature table because pmcp-macros has no features)
    - Cargo.toml lines 150–184 (feature definitions — source of truth for the feature table rows)
    - Cargo.toml lines 15–45 (exclude list — verify CRATE-README.md is NOT matched by any glob; should be safe since none of the entries match `*.md` or `CRATE-README.md`)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-05 file path, D-07 structure, D-08 target length, D-09 doctests, D-11/D-12/D-13/D-14/D-15 feature table)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md (Code Examples → Example 5 verbatim Quick Start; Feature Flag Expansion table for "Enables" column content; Open Questions (RESOLVED) section on logging-row decision — answer: include logging as its own row, giving 18 total rows)
  </read_first>
  <action>
Create a new file at the repo root called `CRATE-README.md`. Target length: 150–250 lines. Must match the following structure exactly (every code block uses ```` ```rust,no_run ````, never ```` ```rust ```` or ```` ```ignore ````):

```markdown
# pmcp

High-quality Rust SDK for the Model Context Protocol (MCP), providing both
client and server implementations with full TypeScript SDK compatibility,
multiple transport options (stdio, HTTP streaming, WebSocket), and built-in
authentication support.

## Quick Start

### Client Example

` ``rust,no_run
use pmcp::{Client, StdioTransport, ClientCapabilities};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
// Create a client with stdio transport
let transport = StdioTransport::new();
let mut client = Client::new(transport);

// Initialize the connection
let server_info = client.initialize(ClientCapabilities::default()).await?;

// List available tools
let tools = client.list_tools(None).await?;
# Ok(())
# }
` ``

### Server Example

` ``rust,no_run
use pmcp::{Server, ServerCapabilities, ToolHandler};
use async_trait::async_trait;
use serde_json::Value;

struct MyTool;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
        Ok(serde_json::json!({"result": "success"}))
    }
}

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities::default())
    .tool("my-tool", MyTool)
    .build()?;

// Run with stdio transport
server.run_stdio().await?;
# Ok(())
# }
` ``

## Cargo Features

<!-- update when Cargo.toml [features] changes -->

| Feature | Description | Enables |
|---------|-------------|---------|
| `default` | Enabled by default; structured logging via `tracing-subscriber` | `tracing-subscriber` |
| `full` | Everything below — single switch | All individual features listed below |
| `composition` | Compose multiple MCP servers into one streamable-HTTP endpoint | (via `streamable-http`) |
| `http` | HTTP transport primitives (Hyper server) | `hyper`, `hyper-util`, `bytes` |
| `http-client` | Async HTTP client (reqwest, rustls backend) | `reqwest` |
| `jwt-auth` | JWT-based authentication helpers | `jsonwebtoken` + `http-client` |
| `logging` | Structured logging via the `tracing` ecosystem | `tracing-subscriber` |
| `macros` | Attribute proc macros (`#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`) | `pmcp-macros`, `schemars` |
| `mcp-apps` | ChatGPT Apps / MCP-UI / SEP-1865 interactive UI types | UI types (code-only, no extra deps) |
| `oauth` | OAuth 2.0 CLI helper for local token flows | `webbrowser`, `dirs`, `rand` + `http-client` |
| `rayon` | Parallel iterator support for batch operations | `rayon` |
| `resource-watcher` | File-system watcher for MCP resource notifications | `notify`, `glob-match` |
| `schema-generation` | Generate JSON Schema from Rust types | `schemars` |
| `simd` | SIMD-optimized JSON parsing (code-only, uses target-feature detection) | SIMD JSON parsing (no extra deps) |
| `sse` | Server-Sent Events streaming transport | `bytes` + `http-client` |
| `streamable-http` | HTTP streaming transport with SSE and Axum integration | `hyper`, `hyper-util`, `hyper-rustls`, `rustls`, `axum`, `tower`, `tower-http`, `futures-util`, `bytes` |
| `validation` | JSON Schema and struct-level validation | `jsonschema`, `garde` |
| `websocket` | WebSocket transport via `tokio-tungstenite` | `tokio-tungstenite` |

## Learn More

- **API docs:** <https://docs.rs/pmcp>
- **Book:** <https://paiml.github.io/pmcp/book/>
- **Course:** <https://paiml.github.io/pmcp/course/>
- **Repository:** <https://github.com/paiml/pmcp>

## License

MIT
```

**Important formatting notes:**

1. The 4 sections above **MUST appear in this exact order**: H1 title + intro paragraph → `## Quick Start` (with Client Example and Server Example subsections) → `## Cargo Features` (with the HTML comment directly above the table, then the table) → `## Learn More` → `## License`. This matches D-07 (structure) and D-14 (table placement immediately after Quick Start).
2. Every fenced code block uses ` ```rust,no_run` — NOT ` ```rust` and NEVER ` ```ignore`. D-09 forbids `ignore`; `no_run` is the project default.
3. In the real file, replace every ` `` ` sequence above with the actual triple backticks — the outer fence in this action description is escaped to avoid markdown-in-markdown confusion.
4. The Cargo Features table has **exactly 20 lines** inside the table block: 1 header row + 1 separator row + 18 data rows (2 meta: `default` + `full`, and 16 individual features alphabetized). The feature rows must appear in exactly this alphabetical order after the two meta rows: composition, http, http-client, jwt-auth, logging, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket. **Note:** `logging` gets its own row even though it's enabled by `default` — the meta row describes what `default` enables; the individual `logging` row describes what `logging` does if you turn off default features and re-enable it explicitly (per RESEARCH.md Open Questions answer).
5. The HTML comment ` <!-- update when Cargo.toml [features] changes --> ` must appear directly above the table (D-15 single-source-of-truth maintenance signal).
6. No GitHub-specific chrome (no `![badge]` images, no "Build Status", no "Coverage", no quality-gate badges). This is a crate-focused landing, not a GitHub project landing.
7. Target total line count: 150–250 lines (D-08). The file skeleton above is roughly 100 lines — pad the "Learn More" / intro / prose sections to stay above 150. Do NOT pad by adding more code blocks, or by describing types in detail (D-08 says detail belongs on per-type `///` docs, not here).
8. The intro paragraph must be 1–2 sentences (D-07 item 1).

Do NOT make changes to `src/lib.rs` in this task — that's Task 2. This task only creates the standalone file.
  </action>
  <verify>
    <automated>test -f CRATE-README.md && awk '/^# pmcp$/{h++} /^## Quick Start$/{q++} /^## Cargo Features$/{c++} /^## Learn More$/{l++} END{exit (h==1 && q==1 && c==1 && l==1)?0:1}' CRATE-README.md && [ "$(wc -l < CRATE-README.md)" -ge 150 ] && [ "$(wc -l < CRATE-README.md)" -le 250 ] && grep -c '^| `' CRATE-README.md | awk '{exit ($1==18)?0:1}'</automated>
  </verify>
  <acceptance_criteria>
    - `test -f CRATE-README.md` succeeds (file exists at repo root, parallel to README.md and Cargo.toml)
    - `grep -c '^# pmcp$' CRATE-README.md` returns exactly `1`
    - `grep -c '^## Quick Start$' CRATE-README.md` returns exactly `1`
    - `grep -c '^### Client Example$' CRATE-README.md` returns exactly `1`
    - `grep -c '^### Server Example$' CRATE-README.md` returns exactly `1`
    - `grep -c '^## Cargo Features$' CRATE-README.md` returns exactly `1`
    - `grep -c '^## Learn More$' CRATE-README.md` returns exactly `1`
    - `grep -c '^```rust,no_run$' CRATE-README.md` returns exactly `2` (Client + Server)
    - `grep -c '^```rust$' CRATE-README.md` returns `0` (no bare rust fences)
    - `grep -c '^```ignore' CRATE-README.md` returns `0` (D-09: ignore forbidden)
    - `grep -c '<!-- update when Cargo.toml \[features\] changes -->' CRATE-README.md` returns exactly `1`
    - `grep -c '^| `' CRATE-README.md` returns exactly `18` (data rows: 2 meta + 16 individual)
    - For each of `composition http http-client jwt-auth logging macros mcp-apps oauth rayon resource-watcher schema-generation simd sse streamable-http validation websocket`: `grep -q '`<name>`' CRATE-README.md` succeeds
    - `grep -q '`default`' CRATE-README.md` succeeds (meta row)
    - `grep -q '`full`' CRATE-README.md` succeeds (meta row)
    - Row order in the Cargo Features table (assert individual feature rows are alphabetized after the two meta rows — verify by `awk '/^## Cargo Features/,/^## Learn More/' CRATE-README.md | grep -oP '(?<=^\| \`)[a-z-]+(?=\`)'` producing the sequence `default full composition http http-client jwt-auth logging macros mcp-apps oauth rayon resource-watcher schema-generation simd sse streamable-http validation websocket`
    - `wc -l CRATE-README.md` in range `150..250` (strict enforcement of D-08 target; verified via `[ $(wc -l < CRATE-README.md) -ge 150 ] && [ $(wc -l < CRATE-README.md) -le 250 ]`)
    - `grep -c '!\[' CRATE-README.md` returns `0` (no badge images)
    - No Build Status / Coverage / Quality Gate chrome: `grep -ciE '(build status|coverage|quality gate)' CRATE-README.md` returns `0`
  </acceptance_criteria>
  <done>
`CRATE-README.md` exists at repo root with all four required sections, 18 feature-table rows in correct order, 2 `rust,no_run` code blocks verbatim from `src/lib.rs:14-61`, and no GitHub-only chrome.
  </done>
</task>

<task type="auto" tdd="false">
  <name>Task 2: Flip src/lib.rs module doc from inline //! to #![doc = include_str!(...)]</name>
  <files>src/lib.rs</files>
  <read_first>
    - src/lib.rs (full — confirm current state: lines 1–61 are `//! `-prefixed module doc, lines 63–77 are lint block, line 70 is `#![cfg_attr(docsrs, feature(doc_cfg))]`)
    - CRATE-README.md (must exist from Task 1 — `include_str!` will fail compile if missing)
    - pmcp-macros/src/lib.rs (reference implementation: lines 1–6 show the exact pattern)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-04 pattern; D-05 path; D-10 preserve lint block)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md (Code Examples → Example 1 for expected post-phase src/lib.rs top)
  </read_first>
  <action>
Replace `src/lib.rs` lines 1–61 (the entire `//! `-prefixed module doc block) with a single `#![doc = include_str!("../CRATE-README.md")]` attribute plus an optional 3-line comment preamble matching the pmcp-macros precedent.

**Before** (current `src/lib.rs:1-62`):
```rust
//! # MCP SDK for Rust
//!
//! A high-quality Rust implementation of the Model Context Protocol (MCP) SDK.
//!
//! This crate provides both client and server implementations of MCP with:
//! - Full protocol compatibility with the TypeScript SDK
//! - Zero-copy parsing where possible
//! - Comprehensive type safety
//! - Multiple transport options (stdio, HTTP/SSE, WebSocket)
//! - Built-in authentication support
//!
//! ## Quick Start
//!
//! ### Client Example
//!
//! ```rust
//! use pmcp::{Client, StdioTransport, ClientCapabilities};
//! ... [all 61 `//! `-prefixed lines] ...
//! ```
<blank line 62>
```

**After** (new `src/lib.rs:1-8`):
```rust
// Crate-level rustdoc is sourced from CRATE-README.md via include_str! so that
// docs.rs and GitHub render from a single authoritative source. Every
// `rust,no_run` code block inside CRATE-README.md is compiled as a doctest
// under `cargo test --doc`, which catches API drift automatically.
#![doc = include_str!("../CRATE-README.md")]

```

Then lines 63–77 (the lint block) follow immediately, preserved exactly as-is:
```rust
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Allow certain clippy lints that are too pedantic for this codebase
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
// _meta is a protocol field name mandated by the MCP spec; suppress underscore lint
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::result_large_err)]
```

**Rules enforced:**

1. The `#![cfg_attr(docsrs, feature(doc_cfg))]` line must remain — **DO NOT** change it to `feature(doc_auto_cfg)` (that feature name was removed in Rust 1.92.0, per RESEARCH.md Upstream Dependency Changes — will fail with E0557).
2. The `#![warn(...)]`, `#![deny(unsafe_code)]`, and five `#![allow(clippy::...)]` lines must remain exactly as-is (D-10 preserves the lint block).
3. The `#[macro_use] #[allow(unused_macros)] mod generated_contracts;` block and everything from `pub mod assets;` onward (currently lines 79+) must remain **unchanged** (this plan only rewrites the top `//!` block and leaves the rest of the file intact).
4. Note: Plan 02 has already deleted the 6 manual `doc(cfg(...))` annotations (including the 2 in this file at former lines 86 and 105). Do NOT re-add them. Do NOT restore any deleted annotation. If Plan 02 has not yet landed when this task runs, the executor must stop and request wave ordering be fixed — this plan's `depends_on: [67-02]` makes this explicit.
5. The `pub mod axum { ... }` block at former `src/lib.rs:99-110` (which contains the ``/// Axum Router convenience API...`` doc comment with the `[AllowedOrigins](axum::AllowedOrigins)` redundant link at former line 102) must remain **unchanged by this task**. That warning is fixed in Plan 04 Task 04-E; do not attempt to fix it here.

After editing, run:
```
cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
cargo test --doc --features full
```
The first must compile; the second must pass all existing doctests (baseline is 338 passing per RESEARCH.md).
  </action>
  <verify>
    <automated>grep -q '^#!\[doc = include_str!("\.\./CRATE-README\.md")\]$' src/lib.rs && [ "$(grep -c '^//!' src/lib.rs)" = "0" ] && grep -q '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs && grep -q '^#!\[deny(unsafe_code)\]$' src/lib.rs && ! grep -q 'doc_auto_cfg' src/lib.rs && cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket && cargo test --doc --features full</automated>
  </verify>
  <acceptance_criteria>
    - `grep -c '^#!\[doc = include_str!("../CRATE-README.md")\]$' src/lib.rs` returns exactly `1`
    - `grep -c '^//!' src/lib.rs` returns `0` (all `//!`-prefixed lines removed from the top of the file)
    - `grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs` returns exactly `1` (unchanged)
    - `grep -c 'doc_auto_cfg' src/lib.rs` returns `0`
    - `grep -c '^#!\[deny(unsafe_code)\]$' src/lib.rs` returns exactly `1`
    - `grep -c '^#!\[warn($' src/lib.rs` returns exactly `1` (the warn block opener)
    - `grep -c 'clippy::missing_errors_doc' src/lib.rs` returns exactly `1` (lint block preserved)
    - `grep -c 'clippy::result_large_err' src/lib.rs` returns exactly `1` (lint block preserved)
    - `grep -c '^mod generated_contracts;' src/lib.rs` returns exactly `1` (unchanged)
    - `grep -c '^pub mod assets;' src/lib.rs` returns exactly `1` (unchanged)
    - `grep -c '^pub mod composition;' src/lib.rs` returns exactly `1` (the `pub mod composition;` line following the `#[cfg(feature = "composition")]` gate; Plan 02 already deleted the adjacent annotation)
    - `grep -c '#\[cfg_attr(docsrs, doc(cfg' src/lib.rs` returns `0` (Plan 02 already deleted these; this plan must not re-introduce)
    - `cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` exits 0
    - `cargo test --doc --features full` exits 0 with all doctests passing (should include the Client + Server Quick Start doctests from CRATE-README.md; baseline 338 doctests, new count should be 338 or 340 — i.e., not decrease)
    - `cargo package --list --allow-dirty 2>/dev/null | grep -q '^CRATE-README.md$'` succeeds (the file ships with the crate; `include_str!` will resolve on crates.io)
  </acceptance_criteria>
  <done>
`src/lib.rs` lines 1–61 replaced by comment preamble + `#![doc = include_str!("../CRATE-README.md")]`. Lint block intact. `feature(doc_cfg)` line intact. `cargo check` and `cargo test --doc --features full` both pass. `cargo package --list` includes CRATE-README.md.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Published crate → docs.rs / crates.io consumers | `CRATE-README.md` is bundled with the published crate and rendered on docs.rs. Its content is author-written markdown with no executable user-input-driven code paths. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-03-01 | Tampering | `src/lib.rs:70` (`feature(doc_cfg)` line) | mitigate | Task 2 action and acceptance_criteria explicitly forbid editing line 70 or typing `doc_auto_cfg`. `grep -c 'doc_auto_cfg' src/lib.rs == 0` enforces this. |
| T-67-03-02 | Tampering | `src/lib.rs:63-77` lint block | mitigate | Task 2 acceptance_criteria asserts `#[deny(unsafe_code)]`, `#[warn(...)]`, and all 5 `clippy::...` allows are still present. D-10 compliance. |
| T-67-03-03 | Supply chain | `CRATE-README.md` bundled with published crate | accept | Content is author-written markdown; `cargo test --doc` validates doctest blocks compile but does not execute them under untrusted input. No executable code beyond the static doctests. No CVE vectors. |
| T-67-03-04 | Denial of service | `include_str!("../CRATE-README.md")` at crate root | mitigate | Task 2 `depends_on: [67-02]` + acceptance_criteria require `cargo check` to succeed. If CRATE-README.md is missing at build time, the macro expansion fails loudly. `cargo package --list` check in acceptance_criteria confirms the file ships with the crate (no stale exclude pattern matches it). |

No new runtime attack surface. Changes are documentation and build-metadata only.
</threat_model>

<verification>
**Wave placement:** This plan is Wave 2 (not Wave 1) because both this plan and Plan 02 modify `src/lib.rs`. Files-modified overlap → sequential wave per planner rules. Plan 02 deletes the 6 manual annotations; this plan rewrites the top doc block. Plan 02 must land first so this plan's acceptance criterion "`grep -c '#\[cfg_attr(docsrs, doc(cfg' src/lib.rs == 0`" is already satisfied by the prior commit and this plan does not re-introduce any annotations.

**Integration check:** After both tasks land, `cargo test --doc --features full` runs the Client + Server Quick Start doctests from CRATE-README.md and exits 0. This is the dispositive test that the include_str! wiring is working.
</verification>

<success_criteria>
- `CRATE-README.md` exists at repo root with all 4 required sections, 18-row Cargo Features table, 2 verbatim `rust,no_run` Quick Start code blocks, no GitHub chrome
- `src/lib.rs:1-61` inline module doc replaced by `#![doc = include_str!("../CRATE-README.md")]` (with optional preamble comment)
- `src/lib.rs:63-77` lint block intact (D-10)
- `src/lib.rs:70` `feature(doc_cfg)` unchanged (D-01 amended)
- `cargo check --features <D-16 list>` exits 0
- `cargo test --doc --features full` exits 0 (baseline 338 doctests, does not regress)
- `cargo package --list --allow-dirty` includes `CRATE-README.md`
- No new `doc(cfg(...))` annotations introduced (Plan 02's deletions preserved)
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-03-SUMMARY.md` with:
- `CRATE-README.md` line count and section list
- The old `src/lib.rs:1-61` diff range → new 5-line replacement
- Confirmation `cargo test --doc --features full` passes (doctest count)
- Confirmation `cargo package --list --allow-dirty | grep CRATE-README.md` shows the file
</output>
