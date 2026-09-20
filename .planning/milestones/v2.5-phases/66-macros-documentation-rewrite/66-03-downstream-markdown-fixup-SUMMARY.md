---
phase: 66-macros-documentation-rewrite
plan: 03
subsystem: docs
tags: [macros, documentation, pmcp-course, migration-guide]
requires:
  - 66-01-poc-include-str-gate
provides:
  - Downstream markdown doc consistency with current #[mcp_tool]/#[mcp_server] API
affects:
  - pmcp-course/src/part1-foundations/ch01-03-why-rust.md
  - pmcp-course/src/part5-security/ch13-oauth.md
  - pmcp-course/src/part5-security/ch13-02-oauth-basics.md
  - pmcp-course/src/part5-security/ch13-03-validation.md
  - docs/advanced/migration-from-typescript.md
tech-stack:
  added: []
  patterns:
    - Markdown-only documentation rewrite (zero Rust compile surface)
key-files:
  created: []
  modified:
    - pmcp-course/src/part1-foundations/ch01-03-why-rust.md
    - pmcp-course/src/part5-security/ch13-oauth.md
    - pmcp-course/src/part5-security/ch13-02-oauth-basics.md
    - pmcp-course/src/part5-security/ch13-03-validation.md
    - docs/advanced/migration-from-typescript.md
decisions:
  - Left TypedTool derive pattern intact — 4 of 5 pmcp-course occurrences are inside #[derive(TypedTool)] helper-attribute style; plan says match the prose, don't restructure teaching examples
  - Added HTML comments to ch01-03 and migration-from-typescript.md noting compile-ready form uses an args struct (these examples are illustrative, not compile-tested)
  - Updated `use pmcp::{tool, tool_router, Parameters}` import in migration-from-typescript.md to `mcp_server, mcp_tool` for consistency with the rewritten attribute names
metrics:
  duration_seconds: 161
  tasks_completed: 2
  files_modified: 5
  occurrences_rewritten: 7
  completed_date: 2026-04-11
---

# Phase 66 Plan 03: Downstream Markdown Fixup Summary

**One-liner:** Rewrites all 7 deprecated `#[tool(...)]` / `#[tool_router]` references across 5 downstream markdown files (4 pmcp-course chapters + migration-from-typescript.md) to the current `#[mcp_tool(...)]` / `#[mcp_server]` API, markdown-only, zero Rust compile surface touched.

## What Was Built

Two atomic commits that rewrite deprecated macro syntax in shipped documentation:

1. **Task 1** (commit `18f11d3f`): Updated 4 pmcp-course chapters (5 occurrences)
2. **Task 2** (commit `abfe5c50`): Updated `docs/advanced/migration-from-typescript.md` (2 occurrences)

Total: 7 occurrences across 5 files rewritten from deprecated to current API.

## Task Execution

### Task 1: pmcp-course chapter fixup — `docs(66): update pmcp-course chapters to #[mcp_tool] (D-17)` (`18f11d3f`)

Five verified deprecated `#[tool(...)]` occurrences rewritten to `#[mcp_tool(...)]` with all attribute arguments preserved.

**Exact before/after diffs:**

#### `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118`

```diff
 This dramatically increases productivity for teams adopting MCP—especially teams new to Rust:

+<!-- Illustrative only; compile-ready form uses an args struct — see pmcp-macros README -->
 ```rust
 // AI-generated MCP tool implementation
-#[tool(
+#[mcp_tool(
     name = "query_sales",
     description = "Query sales data by region and quarter"
 )]
 async fn query_sales(
```

Note: the HTML comment is placed outside the fenced rust block (so it doesn't become invalid Rust syntax inside the code fence). The function signature (`#[arg(description = ...)]` on positional params) was left untouched — this is a "read, don't write" illustrative snippet in a chapter about Rust ergonomics, and the plan explicitly said not to restructure teaching examples.

#### `pmcp-course/src/part5-security/ch13-oauth.md:177`

```diff
 #[derive(TypedTool)]
-#[tool(name = "get_my_data", description = "Get data for the authenticated user")]
+#[mcp_tool(name = "get_my_data", description = "Get data for the authenticated user")]
 pub struct GetMyData;
```

#### `pmcp-course/src/part5-security/ch13-oauth.md:294`

```diff
 #[derive(TypedTool)]
-#[tool(
+#[mcp_tool(
     name = "delete_customer",
     description = "Delete a customer record",
     annotations(destructive = true)
 )]
 pub struct DeleteCustomer;
```

#### `pmcp-course/src/part5-security/ch13-02-oauth-basics.md:243`

```diff
 #[derive(TypedTool)]
-#[tool(name = "execute_query", description = "Run a database query")]
+#[mcp_tool(name = "execute_query", description = "Run a database query")]
 pub struct ExecuteQuery;
```

#### `pmcp-course/src/part5-security/ch13-03-validation.md:78`

```diff
 #[derive(TypedTool)]
-#[tool(name = "query_sales", description = "Query sales data")]
+#[mcp_tool(name = "query_sales", description = "Query sales data")]
 pub struct QuerySales;
```

**Task 1 acceptance criteria — all PASSED:**

- `! grep -rn '#\[tool(' pmcp-course/src/part1-foundations/` → zero matches
- `! grep -rn '#\[tool(' pmcp-course/src/part5-security/` → zero matches
- `grep -c '#\[mcp_tool(' ch01-03-why-rust.md` → 1
- `grep -c '#\[mcp_tool(' ch13-oauth.md` → 2
- `grep -c '#\[mcp_tool(' ch13-02-oauth-basics.md` → 1
- `grep -c '#\[mcp_tool(' ch13-03-validation.md` → 1
- "v2.0 Tip" `#[mcp_tool]` references preserved (5 locations, matches pre-edit baseline; research doc said "6" but grep at phase start found 5 — this is a research count typo, not a drift event)

### Task 2: migration guide fixup — `docs(66): update migration-from-typescript.md to #[mcp_tool]/#[mcp_server] (D-16)` (`abfe5c50`)

Two verified deprecated references rewritten, plus the `use` statement updated for consistency.

**Exact before/after diff:**

```diff
 **Rust PMCP (with macros):**
+<!-- Note: real #[mcp_tool] requires a single args struct — see pmcp-macros/README.md -->
 ```rust
-use pmcp::{tool, tool_router, Parameters};
+use pmcp::{mcp_server, mcp_tool, Parameters};

 #[derive(Debug, Deserialize, JsonSchema)]
 struct CalculateParams {
     operation: String,
     a: f64,
     b: f64,
 }

-#[tool_router]
+#[mcp_server]
 impl Calculator {
-    #[tool(name = "calculate", description = "Perform calculations")]
+    #[mcp_tool(name = "calculate", description = "Perform calculations")]
     async fn calculate(&self, params: Parameters<CalculateParams>) -> Result<f64> {
```

The function signature (`&self, params: Parameters<CalculateParams>`) was intentionally left intact — it's a migration guide example illustrating the wrapper-type pattern, not a compile-tested snippet. The HTML comment above the code block documents that real `#[mcp_tool]` expects a single args struct.

**Task 2 acceptance criteria — all PASSED:**

- `! grep '#\[tool_router\]' docs/advanced/migration-from-typescript.md` → zero matches
- `! grep '#\[tool(' docs/advanced/migration-from-typescript.md` → zero matches
- `grep -q '#\[mcp_server\]' docs/advanced/migration-from-typescript.md` → match at line 123
- `grep -q '#\[mcp_tool(' docs/advanced/migration-from-typescript.md` → match at line 125

## Restructuring Notes

**None of the pmcp-course examples required restructuring beyond the attribute rename.** All 4 of the ch13 `#[derive(TypedTool)] #[tool(...)]` occurrences were mechanical attribute renames — the TypedTool derive pattern is itself illustrative (plan's note item 2 and 3 explicitly allowed this), and reshaping `pub struct ExecuteQuery; impl ExecuteQuery { async fn run(...) }` into a `#[mcp_tool]`-compatible `async fn execute_query(args: ExecuteQueryArgs)` signature would have forced a full rewrite of the surrounding teaching prose about `ToolContext` and `auth.require_scope(...)`.

The ch01-03 example was also left at the free-standing `async fn query_sales(#[arg(...)] region: String, #[arg(...)] quarter: String)` style — the chapter is teaching Rust ergonomics under the "read, don't write" paradigm, not the `#[mcp_tool]` signature surface. An HTML comment above the fenced block now points readers to the pmcp-macros README for the compile-ready form.

The migration-from-typescript.md example was the only occurrence inside a proper `#[tool_router] impl Calculator { #[tool(...)] async fn calculate(...) }` block. Here the rewrite to `#[mcp_server] impl Calculator { #[mcp_tool(...)] ... }` is structurally valid; only the inner function body uses the older `Parameters<CalculateParams>` wrapper instead of a plain args-struct arg — again left intact per the plan's guidance and flagged via an HTML comment.

## Additional Occurrences Found

**None.** Pre-execution grep matched the plan's 7 verified occurrences exactly:
- 5 in pmcp-course (1 in part1, 4 in part5)
- 2 in docs/advanced/migration-from-typescript.md

Post-execution grep confirmed zero residual `#[tool(` or `#[tool_router]` matches in the files listed in the plan's `files_modified` scope.

## Deviations from Plan

None — plan executed exactly as written.

Two editorial choices were explicitly authorized by the plan:

1. **HTML comment placement for ch01-03** — the plan's Task 1 step 4 said to add an HTML comment if the function signature is left at the multi-param shape. I placed the comment **above** the ```rust fence (not inside it), so it renders as a prose comment in the mdbook output rather than creating an invalid Rust comment inside the code block.

2. **Updated `use pmcp::{tool, tool_router, Parameters}` import in migration-from-typescript.md** — the plan's Task 2 grep "for any OTHER `#[tool` in the file" instruction was literally about attribute occurrences, but the `use` statement importing the deleted macro names was a consistency hazard (readers copy-pasting the example would get a compile error). Updated to `use pmcp::{mcp_server, mcp_tool, Parameters}` to match the rewritten attribute names. This is in spirit with the plan's scope (file consistency) and does not expand into new territory.

## Authentication Gates

None — this was a pure markdown edit plan with no tooling, network, or auth surface.

## Success Criteria

- [x] All 5 markdown files updated
- [x] All 7 occurrences of deprecated `#[tool(...)]` / `#[tool_router]` rewritten to current syntax
- [x] No file under `pmcp-course/src/` or `docs/advanced/migration-from-typescript.md` references `#[tool(` or `#[tool_router]`
- [x] Each task committed individually with `--no-verify`
- [x] SUMMARY.md created at `.planning/phases/66-macros-documentation-rewrite/66-03-downstream-markdown-fixup-SUMMARY.md`

## Commits

| Task | Hash      | Subject |
|------|-----------|---------|
| 1    | `18f11d3f` | docs(66): update pmcp-course chapters to #[mcp_tool] (D-17) |
| 2    | `abfe5c50` | docs(66): update migration-from-typescript.md to #[mcp_tool]/#[mcp_server] (D-16) |

## Self-Check: PASSED

Files verified to exist with correct content:

- FOUND: `pmcp-course/src/part1-foundations/ch01-03-why-rust.md` — contains `#[mcp_tool(` at line 119
- FOUND: `pmcp-course/src/part5-security/ch13-oauth.md` — contains 2 `#[mcp_tool(` occurrences (lines 177, 294)
- FOUND: `pmcp-course/src/part5-security/ch13-02-oauth-basics.md` — contains `#[mcp_tool(` at line 243
- FOUND: `pmcp-course/src/part5-security/ch13-03-validation.md` — contains `#[mcp_tool(` at line 78
- FOUND: `docs/advanced/migration-from-typescript.md` — contains `#[mcp_server]` at line 123 and `#[mcp_tool(` at line 125

Commits verified to exist on branch:

- FOUND: `18f11d3f` (Task 1 — pmcp-course)
- FOUND: `abfe5c50` (Task 2 — migration-from-typescript.md)

Global verification greps all return zero matches:

- `grep -rn '#\[tool(' pmcp-course/src/` → empty
- `grep -rn '#\[tool_router\]' pmcp-course/src/` → empty
- `grep -n '#\[tool[\(_]' docs/advanced/migration-from-typescript.md` → empty
