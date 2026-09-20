---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 04
type: execute
wave: 3
depends_on:
  - 67-01
  - 67-03
files_modified:
  - src/client/http_logging_middleware.rs
  - src/server/http_middleware.rs
  - src/shared/http_utils.rs
  - src/server/auth/providers/mod.rs
  - src/server/task_store.rs
  - src/server/tasks.rs
  - src/server/core.rs
  - src/server/workflow/task_prompt_handler.rs
  - src/server/workflow/workflow_step.rs
  - src/server/axum_router.rs
  - src/server/streamable_http_server.rs
  - src/server/tower_layers/dns_rebinding.rs
  - src/server/workflow/handles.rs
  - src/server/workflow/newtypes.rs
  - src/types/ui.rs
  - src/lib.rs
autonomous: true
requirements:
  - DRSD-04
tags:
  - rust
  - rustdoc
  - warnings
must_haves:
  truths:
    - "Zero rustdoc warnings on `cargo doc --no-deps --features <D-16 list>`"
    - "Every category of warning (9 bracket-escape, 15 intra-doc-link, 3 private-link, 2 HTML-tag, 1 redundant-link — 30 raw reports aggregated to 29 baseline warnings per RESEARCH.md) is addressed"
    - "`cargo test --doc --features full` still passes (no doctest regression)"
    - "No new `#![allow(rustdoc::...)]` suppressions added"
    - "Fixes scoped to the pmcp crate (not pmcp-macros, mcp-tester, cargo-pmcp, etc.)"
  artifacts:
    - path: "src/client/http_logging_middleware.rs"
      provides: "Fixed bracket-escape warnings (9 of them: lines 9, 10, 11, 86 (x2), 159, 160 + server/http_middleware.rs:429 + shared/http_utils.rs:85)"
    - path: "src/server/workflow/handles.rs"
      provides: "Fixed unclosed HTML tag (Arc<str> in backticks)"
    - path: "src/server/workflow/newtypes.rs"
      provides: "Fixed unclosed HTML tag (Arc<str> in backticks)"
  key_links:
    - from: "RUSTDOCFLAGS=-D warnings cargo doc"
      to: "exit code 0"
      via: "zero warning output"
      pattern: "warning:"
---

<objective>
Fix all 29 rustdoc warnings reported by `cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` (the D-16 feature list). Categories: 9 unescaped-bracket pitfalls, 15 broken intra-doc links (cross-crate / renamed / stale), 3 public-doc-to-private-item links, 2 unclosed HTML `<str>` tags, 1 redundant explicit link target.

Purpose: Makes `make doc-check` (Plan 05) passable — without this plan the new CI gate fails immediately. This is the bulk of the work in Phase 67 and the only plan with significant file churn.

Output: 15 source files edited (no new files). After this plan, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list>` exits 0. All 338 existing doctests continue to pass.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md

<interfaces>
<!-- Full baseline from RESEARCH.md (Rustdoc Warning Baseline section). 29 warnings. -->
<!-- Established by `cargo doc --no-deps --features <D-16 list>` on the pre-edit tree. -->

Category breakdown (from RESEARCH.md):

| Category | Count | Lint group |
|---|---|---|
| Unresolved intra-doc link — `[REDACTED]` brackets-as-links (pitfall 3) | 9 | `rustdoc::broken_intra_doc_links` |
| Unresolved intra-doc link — cross-crate types not re-exported (TaskStore, TaskRouter, IdentityProvider, CorsLayer, StreamableHttpServerConfig, WorkflowProgress, etc.) | 15 | `rustdoc::broken_intra_doc_links` |
| Public doc links to private item (PauseReason, StepStatus, insert_legacy_resource_uri_key) | 3 | `rustdoc::private_intra_doc_links` |
| Unclosed HTML tag `<str>` — raw `Arc<str>` in prose without backticks | 2 | `rustdoc::invalid_html_tags` |
| Redundant explicit link target at `src/lib.rs:102` | 1 | `rustdoc::redundant_explicit_links` |

File-by-file (from RESEARCH.md Rustdoc Warning Baseline table):

1–7. `src/client/http_logging_middleware.rs` lines 9, 10, 11, 86 (x2 at cols 53 and 77), 159, 160 — unescaped `[REDACTED]`
8. `src/server/http_middleware.rs:429` — unescaped `[REDACTED]`
9. `src/shared/http_utils.rs:85` — unescaped `[REDACTED]`
10. `src/server/auth/providers/mod.rs:3` — `[`IdentityProvider`]` (cross-crate to pmcp-tasks or similar)
11–15. `src/server/task_store.rs` (multiple lines in the module-level `//!` block) — `[`TaskStore`]`, `[`InMemoryTaskStore`]`, `[`Task`]` (module-level doc refs to types in pmcp-tasks)
16. `src/server/tasks.rs:5` — `[`TaskRouter`]`
17. `src/server/core.rs:853` — `[`TaskStore`]` and `[`TaskRouter`]` (two links on one line)
18. `src/server/core.rs` (nearby) — `[`ServerCoreBuilder`](super::builder::ServerCoreBuilder)` bad path
19. `src/server/tasks.rs:98:37` — `[`WorkflowProgress`]`
20–22. `src/server/workflow/task_prompt_handler.rs:10`, `:28` (two occurrences) — `[`PauseReason`]`, `[`StepStatus`]` (private types)
23. `src/server/workflow/workflow_step.rs:337` — `[`PauseReason::ToolError`]` (stale ref)
24–25. `src/server/axum_router.rs:3` — `[`router()`]` and `[`router_with_config()`]` (link to functions in same module — either drop the `()` or use plain backticks)
26. `src/server/streamable_http_server.rs:367` — `[`CorsLayer`]` (in tower-http, not re-exported)
27. `src/server/tower_layers/dns_rebinding.rs:99` — `[`StreamableHttpServerConfig::stateless()`]` (stale ref)
28. `src/server/workflow/handles.rs:3:50` — `Arc<str>` in prose (unclosed HTML `<str>`)
29. `src/server/workflow/newtypes.rs:4:16` — `Arc<str>` in prose (unclosed HTML `<str>`)
30. `src/types/ui.rs:385:40` — `[`insert_legacy_resource_uri_key`]` (private fn)
31. `src/lib.rs:102:66` — `[`RouterConfig`](axum::RouterConfig), and [`AllowedOrigins`](axum::AllowedOrigins)` — redundant explicit link target

Note: The 29→31 count discrepancy is explained in RESEARCH.md — rustdoc reports 29 "warning:" summary lines but the file:line enumeration totals 31 because some single-line warnings reference two items. The plan treats all 31 occurrences as distinct fixes; the ultimate gate is zero warnings.

**Baseline re-check command (run before any fix):**
```
cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | tee /tmp/doc-warnings-before.log | grep -c 'warning:'
```
Expected output: `29` (±2 — may vary slightly as the include_str! flip from Plan 03 might shift line 102 by a few lines). If the count is 0, the plan is a no-op (unexpected — raise an issue). If the count is 30+ with new warnings not in the list above, stop and investigate — possibly a new category introduced by Plan 03's include_str! content.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Fix 9 unescaped bracket pitfalls in http_logging_middleware.rs / http_middleware.rs / http_utils.rs</name>
  <files>src/client/http_logging_middleware.rs, src/server/http_middleware.rs, src/shared/http_utils.rs</files>
  <read_first>
    - src/client/http_logging_middleware.rs lines 1–170 (see all 7 occurrences)
    - src/server/http_middleware.rs around line 429
    - src/shared/http_utils.rs around line 85
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Pitfall 3 ("Non-escaped `[BRACKET]` inside doc strings") for the fix pattern
  </read_first>
  <action>
Each occurrence of a bare `[REDACTED]` inside a `///` or `//!` comment must become `` `[REDACTED]` `` (wrap in backticks — the rustdoc-preferred form per RESEARCH.md Pitfall 3). Backticks are safer than backslash-escaping because they render identically on GitHub markdown and rustdoc, and they carry code-style formatting that matches the "this is a literal string value" intent.

**File 1: `src/client/http_logging_middleware.rs`** — 7 occurrences to fix:

- Line 9 (approximately): `//! - \`authorization\`: Redacted as "Bearer [REDACTED]"` → `//! - \`authorization\`: Redacted as `` "Bearer `[REDACTED]`" ``
- Line 10: `//! - \`cookie\` / \`set-cookie\`: Redacted as "[REDACTED]"` → replace `"[REDACTED]"` with `` "`[REDACTED]`" ``
- Line 11: `//! - \`x-api-key\`, \`proxy-authorization\`, \`x-auth-token\`: Redacted as "[REDACTED]"` → same fix
- Line 86: `/// - **Show auth scheme**: true (logs "Bearer [REDACTED]" instead of "[REDACTED]")` — **two occurrences on one line**, both become `` `[REDACTED]` ``
- Line 159: `/// If true: "Bearer [REDACTED]"` → `/// If true: `` "Bearer `[REDACTED]`" ``
- Line 160: `/// If false: "[REDACTED]"` → `/// If false: `` "`[REDACTED]`" ``

**File 2: `src/server/http_middleware.rs`** — 1 occurrence at line 429:

- `/// - **Show auth scheme**: true (logs "Bearer [REDACTED]")` → `/// - **Show auth scheme**: true (logs `` "Bearer `[REDACTED]`" ``)`

**File 3: `src/shared/http_utils.rs`** — 1 occurrence at line 85:

- `/// Returns the URL with query parameters replaced by "[REDACTED]".` → `/// Returns the URL with query parameters replaced by `` "`[REDACTED]`" ``.`

**Alternative acceptable form (if backticks are awkward around the quotes):**
Replace `[REDACTED]` (bracket-enclosed text) with `\[REDACTED\]` (backslash-escaped brackets). Either form silences the warning. Prefer backticks because they also carry semantic "this is a literal" formatting.

**Do NOT change:**
- Any non-doc-comment `[REDACTED]` occurrences in actual string literals (the runtime redaction constants). Those are source code, not rustdoc prose.
- The logic or behavior of the redaction — this is a doc-only fix.

After editing, verify the fix by running:
```
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | grep -E 'src/(client/http_logging_middleware|server/http_middleware|shared/http_utils)' | grep warning
```
Expected: empty output (all 9 bracket warnings gone).
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | { ! grep -E 'src/(client/http_logging_middleware|server/http_middleware|shared/http_utils).*warning'; }</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -E 'http_logging_middleware.*warning' | wc -l` returns `0`
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -E 'http_middleware.*warning' | wc -l` returns `0`
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -E 'http_utils.*warning' | wc -l` returns `0`
    - `cargo test --doc --features full` exits 0 (no doctest regression)
    - No runtime string literal changed (grep confirms every runtime `"[REDACTED]"` string in non-doc code is untouched)
  </acceptance_criteria>
  <done>
All 9 bracket-escape warnings in the 3 files silenced. Runtime redaction logic unchanged.
  </done>
</task>

<task type="auto">
  <name>Task 2: Fix 2 unclosed `<str>` HTML tag warnings in workflow/handles.rs and workflow/newtypes.rs</name>
  <files>src/server/workflow/handles.rs, src/server/workflow/newtypes.rs</files>
  <read_first>
    - src/server/workflow/handles.rs lines 1–10 (line 3 contains `Arc<str>` in prose)
    - src/server/workflow/newtypes.rs lines 1–10 (line 4 contains `Arc<str>` in prose)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Pitfall 4 ("`Arc<str>` in prose without backticks") for the fix pattern
  </read_first>
  <action>
Wrap `Arc<str>` in backticks so rustdoc treats it as inline code, not HTML:

**File 1: `src/server/workflow/handles.rs`** — line 3 (approximately):

- `//! Handles are lightweight identifiers using Arc<str> for O(1) cloning.` → `` //! Handles are lightweight identifiers using `Arc<str>` for O(1) cloning. ``

**File 2: `src/server/workflow/newtypes.rs`** — line 4 (approximately):

- `//! All use Arc<str> for O(1) cloning.` → `` //! All use `Arc<str>` for O(1) cloning. ``

This is a purely textual fix — only the doc comment body changes. Do not modify any type definitions, use statements, or runtime code.

After editing, verify:
```
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | grep 'unclosed HTML tag'
```
Expected: empty.
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | { ! grep 'unclosed HTML tag'; }</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -c 'unclosed HTML tag'` returns `0`
    - `grep -c '`Arc<str>`' src/server/workflow/handles.rs` returns at least `1` (backtick-wrapped)
    - `grep -c '`Arc<str>`' src/server/workflow/newtypes.rs` returns at least `1`
    - `cargo test --doc --features full` exits 0
  </acceptance_criteria>
  <done>
Both `<str>` unclosed-HTML warnings gone. Backtick-wrapped `Arc<str>` in prose.
  </done>
</task>

<task type="auto">
  <name>Task 3: Fix redundant explicit link target at src/lib.rs line ~102 (axum module doc)</name>
  <files>src/lib.rs</files>
  <read_first>
    - src/lib.rs around the `pub mod axum { ... }` block (after Plan 03's include_str! flip, this block is approximately at lines 40–55 of the new lib.rs — use grep to find it, not absolute line numbers)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Pitfall 7 ("Redundant explicit link target") for the fix pattern
  </read_first>
  <action>
Find the `pub mod axum` module doc comment that currently contains:
```rust
/// Axum Router convenience API for secure MCP server hosting.
///
/// Re-exports [`router()`](axum::router), [`router_with_config()`](axum::router_with_config),
/// [`RouterConfig`](axum::RouterConfig), and [`AllowedOrigins`](axum::AllowedOrigins)
/// for ergonomic usage: `pmcp::axum::router(server)`.
```

Replace it with the non-redundant form (drop the explicit `(axum::Foo)` link targets — the `[``Foo``]` label already resolves to the same item):

```rust
/// Axum Router convenience API for secure MCP server hosting.
///
/// Re-exports [`router`], [`router_with_config`],
/// [`RouterConfig`], and [`AllowedOrigins`]
/// for ergonomic usage: `pmcp::axum::router(server)`.
```

Note the changes:
- `[`router()`](axum::router)` → `[`router`]` (also drops the `()` — function links don't need parens inside the label)
- `[`router_with_config()`](axum::router_with_config)` → `[`router_with_config`]`
- `[`RouterConfig`](axum::RouterConfig)` → `[`RouterConfig`]`
- `[`AllowedOrigins`](axum::AllowedOrigins)` → `[`AllowedOrigins`]`

**Note:** Rustdoc's intra-doc-link resolver will now resolve `[`RouterConfig`]` against the module's `pub use crate::server::axum_router::{router, router_with_config, AllowedOrigins, RouterConfig};` — which is exactly what the redundant form was pointing at. The links still work; the warning disappears.

**Do NOT:**
- Touch the `#[cfg(feature = "streamable-http")]` gate above this block (Plan 02 already deleted the adjacent `#[cfg_attr(docsrs, doc(cfg(...)))]` annotation; do not restore it).
- Touch anything outside this one doc comment.

After editing, verify:
```
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | grep 'redundant explicit link'
```
Expected: empty.
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | { ! grep 'redundant explicit link'; }</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -c 'redundant explicit link'` returns `0`
    - `grep -c '\[`RouterConfig`\](axum::RouterConfig)' src/lib.rs` returns `0` (old form gone)
    - `grep -c '\[`AllowedOrigins`\](axum::AllowedOrigins)' src/lib.rs` returns `0` (old form gone)
    - `grep -c '\[`RouterConfig`\]' src/lib.rs` returns at least `1` (new form present)
    - `grep -c 'pub mod axum {' src/lib.rs` returns `1` (module declaration intact)
    - `cargo test --doc --features full` exits 0
  </acceptance_criteria>
  <done>
`src/lib.rs` axum module doc no longer contains the redundant `(axum::Foo)` form. Warning gone.
  </done>
</task>

<task type="auto">
  <name>Task 4: Fix 15 broken intra-doc links across task_store.rs, tasks.rs, core.rs, auth/providers/mod.rs, axum_router.rs, streamable_http_server.rs, dns_rebinding.rs, workflow_step.rs</name>
  <files>src/server/task_store.rs, src/server/tasks.rs, src/server/core.rs, src/server/auth/providers/mod.rs, src/server/axum_router.rs, src/server/streamable_http_server.rs, src/server/tower_layers/dns_rebinding.rs, src/server/workflow/workflow_step.rs</files>
  <read_first>
    - src/server/task_store.rs lines 1–50 (module-level doc block — multiple broken links)
    - src/server/tasks.rs lines 1–10 and around line 98
    - src/server/core.rs around line 853
    - src/server/auth/providers/mod.rs line 3
    - src/server/axum_router.rs line 3
    - src/server/streamable_http_server.rs around line 367
    - src/server/tower_layers/dns_rebinding.rs around line 99
    - src/server/workflow/workflow_step.rs around line 337
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Pitfall 5 ("Intra-doc links to types in unlisted crates") for the fix pattern
  </read_first>
  <action>
For each of the 15 broken intra-doc links below, apply Pitfall 5 remediation: **replace `[`TypeName`]` (an intra-doc link) with `` `TypeName` `` (plain code formatting, no link)**. This drops the clickability but silences the warning without resurrecting dead links or adding explicit URLs (which would create a separate maintenance burden).

Exception: if the target type is actually in the same crate but was renamed/moved, use the correct path. None of the 15 below fall in this exception per RESEARCH.md analysis — all are either cross-crate (pmcp-tasks, axum, tower-http) or refer to types that never existed in pmcp's namespace.

**File-by-file list:**

1. `src/server/auth/providers/mod.rs` line 3 (approximately):
   `//! This module provides concrete implementations of the [`IdentityProvider`] trait`
   → `//! This module provides concrete implementations of the `` `IdentityProvider` `` trait`

2–5. `src/server/task_store.rs` module-level `//!` block (lines 3, 16, 17, 47 per RESEARCH.md grep):
   Every `[`TaskStore`]`, `[`InMemoryTaskStore`]`, `[`Task`]` in the module-level doc block becomes the plain backticked form. Specifically:
   - Line 3: `//! This module provides [`TaskStore`], the core trait for task lifecycle` → `//! This module provides `` `TaskStore` ``, the core trait for task lifecycle`
   - Line 16: `//! The SDK [`TaskStore`] trait is intentionally simplified` → `//! The SDK `` `TaskStore` `` trait is intentionally simplified`
   - Line 17 or nearby: any `[`Task`]` occurrence → `` `Task` ``
   - Line 47 or nearby: any `[`InMemoryTaskStore`]` occurrence → `` `InMemoryTaskStore` ``

6. `src/server/tasks.rs:5` (approximately):
   `//! The `pmcp-tasks` crate implements [`TaskRouter`]` → `` //! The `pmcp-tasks` crate implements `TaskRouter` ``

7. `src/server/tasks.rs:98` (approximately):
   `* `progress` - Serialized [`WorkflowProgress`] to store in task variables.` → `* `progress` - Serialized `` `WorkflowProgress` `` to store in task variables.`

8–9. `src/server/core.rs:853` (approximately): one line contains two broken links:
   `/// When only a [`TaskStore`] is configured (no [`TaskRouter`]), derives` → `` /// When only a `TaskStore` is configured (no `TaskRouter`), derives ``

10. `src/server/core.rs` nearby (look for `[`ServerCoreBuilder`](super::builder::ServerCoreBuilder)`):
    The `super::builder::` path does not resolve. Replace with plain backticks: `` `ServerCoreBuilder` ``

11–12. `src/server/axum_router.rs:3` — two occurrences on one module-doc line:
    `//! Provides [`router()`] and [`router_with_config()`] that return a` → `//! Provides `` `router()` `` and `` `router_with_config()` `` that return a`
    (OR equivalently: drop the `()` and use `[`router`]` / `[`router_with_config`]` since they DO resolve in the same module — rustdoc's same-module resolver can find them. Try the bare-link form first; if it still warns because of the `()` parsing, fall back to plain backticks.)

13. `src/server/streamable_http_server.rs:367`:
    `/// - [`CorsLayer`] -- origin-locked CORS (no wildcard `*`)` → `` /// - `CorsLayer` -- origin-locked CORS (no wildcard `*`) ``

14. `src/server/tower_layers/dns_rebinding.rs:99`:
    `/// [`StreamableHttpServerConfig::stateless()`] uses this by default.` → `` /// `StreamableHttpServerConfig::stateless()` uses this by default. ``

15. `src/server/workflow/workflow_step.rs:337`:
    `/// the [`PauseReason::ToolError`] variant so clients know they can` → `` /// the `PauseReason::ToolError` variant so clients know they can ``

**Process:**

1. For each file, open and locate the exact line via grep (line numbers may have shifted from Plan 03's include_str! flip and from Task 1's REDACTED edits — use content-based search, not absolute line numbers).
2. Apply the `[`Foo`]` → `` `Foo` `` transformation (or the route variant for case 11–12 if the bare-link form works).
3. After all 15 edits, run the baseline command:
   ```
   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | grep -E 'unresolved link'
   ```
   Expected: empty. If the command still reports warnings in a file not listed above, inspect the new warning — it may be a cascade (fixing one link exposed another). Keep iterating until `grep -c 'unresolved link'` returns 0.

**Do NOT:**
- Add new `pub use` re-exports just to make the links resolve. That expands the public API surface for a doc fix — out of scope for Phase 67 and potentially regressive.
- Add `[`Foo`](https://docs.rs/...)` explicit URL forms. Those create drift risk (URL-pinned versions go stale) and are harder to review.
- Modify the text content beyond the bracket→backtick transformation. Prose meaning stays identical.
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | { ! grep 'unresolved link'; }</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -c 'unresolved link'` returns `0`
    - `grep -c '\[`TaskStore`\]' src/server/task_store.rs` returns `0` (old intra-doc-link form gone)
    - `grep -c '\[`IdentityProvider`\]' src/server/auth/providers/mod.rs` returns `0`
    - `grep -c '\[`CorsLayer`\]' src/server/streamable_http_server.rs` returns `0`
    - `grep -c '\[`StreamableHttpServerConfig' src/server/tower_layers/dns_rebinding.rs` returns `0`
    - `grep -c '\[`WorkflowProgress`\]' src/server/tasks.rs` returns `0`
    - No new `pub use` statements added (git diff confirms; count of `pub use` lines in src/ does not increase)
    - `cargo test --doc --features full` exits 0
  </acceptance_criteria>
  <done>
All 15 cross-crate/renamed/stale intra-doc-link warnings silenced by converting to plain backtick form. No public-API re-exports added.
  </done>
</task>

<task type="auto">
  <name>Task 5: Fix 3 public-doc-links-to-private-item warnings (PauseReason, StepStatus, insert_legacy_resource_uri_key)</name>
  <files>src/server/workflow/task_prompt_handler.rs, src/types/ui.rs</files>
  <read_first>
    - src/server/workflow/task_prompt_handler.rs lines 1–40 (two occurrences: PauseReason at line 10 and 28, StepStatus at line 28)
    - src/types/ui.rs around line 385 (insert_legacy_resource_uri_key)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Pitfall 6 ("Public docs referencing private items")
  </read_first>
  <action>
Three private-intra-doc-link warnings. Apply the "drop the link, keep the backticks" form (same as Task 4):

**File 1: `src/server/workflow/task_prompt_handler.rs`** — 3 occurrences in 2 lines:

- Line 10 (approximately): `//! 4. Classifies failures into typed [`PauseReason`] variants` → `` //! 4. Classifies failures into typed `PauseReason` variants ``
- Line 28 (approximately): `//! The typed [`PauseReason`], [`StepStatus`], and workflow progress types` → `` //! The typed `PauseReason`, `StepStatus`, and workflow progress types ``
  (Both `PauseReason` and `StepStatus` on the same line — two edits in one line.)

**File 2: `src/types/ui.rs`** — 1 occurrence at line 385 (approximately):

- `/// (`ui/resourceUri`) is emitted by [`insert_legacy_resource_uri_key`], which` → `` /// (`ui/resourceUri`) is emitted by `insert_legacy_resource_uri_key`, which ``

**Rationale:** These types (`PauseReason`, `StepStatus`, `insert_legacy_resource_uri_key`) are intentionally private — they are implementation details. Making them public to satisfy the link would expand the API surface unnecessarily. Dropping the link (while keeping the backtick formatting so readers still see "this is a name you'd see in the source") is the correct fix.

**Do NOT:**
- Change visibility of `PauseReason`, `StepStatus`, or `insert_legacy_resource_uri_key`. They stay private.
- Add `pub use` re-exports. They stay internal.
- Modify the prose meaning.

After editing, verify:
```
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | grep 'private intra-doc'
```
Expected: empty.
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | { ! grep -E 'private (intra-)?doc'; }</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -c 'private intra-doc'` returns `0`
    - `grep -c '\[`PauseReason`\]' src/server/workflow/task_prompt_handler.rs` returns `0`
    - `grep -c '\[`StepStatus`\]' src/server/workflow/task_prompt_handler.rs` returns `0`
    - `grep -c '\[`insert_legacy_resource_uri_key`\]' src/types/ui.rs` returns `0`
    - Visibility of `PauseReason`, `StepStatus`, `insert_legacy_resource_uri_key` unchanged: `grep -c 'pub enum PauseReason' src/server/workflow/*.rs` returns `0` AND `grep -c 'pub fn insert_legacy_resource_uri_key' src/types/ui.rs` returns `0` AND `grep -c 'pub struct StepStatus\|pub enum StepStatus' src/server/workflow/*.rs` returns `0`
    - `cargo test --doc --features full` exits 0
  </acceptance_criteria>
  <done>
All 3 public-to-private link warnings silenced. Visibility of the 3 items unchanged.
  </done>
</task>

<task type="auto">
  <name>Task 6: Aggregate gate — make doc-check (pre-created equivalent) exits with zero rustdoc warnings</name>
  <files></files>
  <read_first>
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md Phase Requirements → Test Map (DRSD-04 row) and Phase-Gate Checklist item 7 (the final command)
  </read_first>
  <action>
Run the aggregate command equivalent to `make doc-check` (which doesn't exist yet — Plan 05 creates it). This is the final integration check for Plan 04:

```
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
```

Expected exit code: **0**.
Expected stderr warning count: **0**.

If non-zero, iterate:
1. Read the specific warning line (file:line:col).
2. Apply the matching Pitfall fix from Tasks 1–5.
3. Rerun until zero.

If a NEW category of warning appears that wasn't in the Tasks 1–5 list (e.g., doctest-level warning, new broken-link from Plan 03's CRATE-README.md content):
1. Diagnose whether it's a CRATE-README.md doctest issue (likely in Client Example or Server Example — check that `rust,no_run` didn't introduce a new lint) or a pre-existing warning that was masked.
2. Apply minimal fix in the same file.
3. DO NOT add `#![allow(rustdoc::...)]` suppressions — the whole point of this phase is zero suppressions.

Also run the doctest gate:
```
cargo test --doc --features full
```
Expected: all doctests pass. Baseline is 338; after this plan the count should be 338 or 340 (Plan 03's include_str! may add the 2 Quick Start doctests as new passing tests).

Commit only after BOTH commands exit 0.
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket && cargo test --doc --features full</automated>
  </verify>
  <acceptance_criteria>
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` exits 0
    - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list> 2>&1 | grep -c 'warning:'` returns `0`
    - `cargo test --doc --features full` exits 0
    - `grep -rc '#!\[allow(rustdoc::' src/` returns `0` (no new rustdoc suppressions added)
    - `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c` returns nothing (Plan 02's cleanup preserved)
  </acceptance_criteria>
  <done>
`cargo doc` runs with zero warnings on the D-16 feature set. `cargo test --doc --features full` passes. No rustdoc suppressions in `src/`.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

None introduced — this plan edits doc comments (prose only). No runtime code paths, authentication, validation, or data handling are touched.

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-04-01 | Information disclosure | `src/client/http_logging_middleware.rs` redaction behavior | accept | Plan edits only doc comments describing the redaction. Runtime redaction constants (the actual `"[REDACTED]"` string literal values used at runtime) are explicitly out of scope per Task 1 action: "Do NOT change any non-doc-comment `[REDACTED]` occurrences in actual string literals." `grep`-based acceptance_criteria ensures no runtime string literal is modified. |
| T-67-04-02 | Tampering | Visibility of `PauseReason`, `StepStatus`, `insert_legacy_resource_uri_key` | mitigate | Task 5 explicitly forbids visibility changes and `pub use` additions; acceptance_criteria verifies with grep. Preserves current private-item boundary. |
| T-67-04-03 | Denial of service | Rustdoc suppression creep | mitigate | Task 6 acceptance_criteria: `grep -rc '#!\[allow(rustdoc::' src/` returns 0. Prevents "just suppress the warning" shortcut. |

No new runtime attack surface. Plan only edits doc comments and their internal links.
</threat_model>

<verification>
**Wave 3 placement:** This plan depends on Plan 01 (Cargo.toml feature list determines which warnings fire) and Plan 03 (include_str! flip of src/lib.rs changes line 102 position for the redundant-link warning). Plan 02 (which deletes manual annotations) is a transitive dependency but does not affect warning count directly.

**Inter-task order within this plan:** Tasks 1–5 are category-scoped and can run in any order; Task 6 is the final aggregate gate and must be the last commit.

**Cascade handling:** If fixing one link exposes another (e.g., the task_store.rs block has overlapping broken links), iterate within Task 4 until the `grep 'unresolved link'` count stabilizes at 0.

**Total warning count invariant:** Pre-plan 29 → post-plan 0. Intermediate commits may show partial reductions; no commit is allowed to increase the count.
</verification>

<success_criteria>
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list>` exits 0
- Warning count from pre-baseline 29 → 0
- `cargo test --doc --features full` exits 0 (338+ doctests still pass)
- No `#![allow(rustdoc::...)]` suppressions added anywhere in `src/`
- No `pub use` or visibility changes (grep confirms)
- Runtime redaction behavior unchanged (grep confirms non-doc `"[REDACTED]"` strings untouched)
- `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs` unchanged (Plan 02's invariant)
- No new manual `doc(cfg(...))` annotations (Plan 02's cleanup preserved)
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-04-SUMMARY.md` with:
- Pre-baseline warning count (from `/tmp/doc-warnings-before.log` or a fresh run)
- Post-plan warning count (0)
- Per-category breakdown (bracket: 9→0, intra-doc: 15→0, private: 3→0, HTML: 2→0, redundant: 1→0)
- Doctest count before/after (should be 338 or 338→340)
- Total files edited (should be ~15)
- Confirmation no suppressions added (`grep` count)
</output>
