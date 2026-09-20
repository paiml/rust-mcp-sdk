---
phase: 82-builder-dx-prerequisites
plan: 01
subsystem: server-builder
tags:
  - builder-dx
  - arc-handlers
  - testing-pattern
  - additive-only
requirements:
  - BLDR-01
  - BLDR-02
  - BLDR-03
  - BLDR-04
dependency_graph:
  requires: []
  provides:
    - "pmcp::ServerBuilder::tool_arc"
    - "pmcp::ServerBuilder::prompt_arc"
    - "pmcp::ServerBuilder::resources_arc"
    - "pmcp::ServerBuilder::sampling_arc"
    - "pmcp::ServerBuilder::auth_provider_arc"
    - "pmcp::ServerBuilder::tool_authorizer_arc"
    - "pmcp::Server::get_tool"
  affects:
    - "Phase 83 pmcp-server-toolkit (consumes tool_arc / prompt_arc)"
    - "Phase 84 SQL connectors (transitively via Phase 83)"
    - "Phase 87 pmcp-config-helper (consumes documented testing pattern)"
    - "Phase 88 dogfood crates/pmcp-server (consumes testing pattern)"
tech_stack:
  added: []
  patterns:
    - "Pure-additive copy from private ServerCoreBuilder to public ServerBuilder"
    - "Donor `let name = name.into();` first-statement pattern for impl Into<String>"
    - "if is_none capability auto-enable to preserve explicit prior capabilities(...) overrides"
    - "Hidden-async-wrapper doctest pattern `# async fn example() -> pmcp::Result<()> { ... # Ok(()) # }`"
key_files:
  created: []
  modified:
    - src/server/mod.rs
key_decisions:
  - "D-04 enforced: existing `tool() / prompt() / resources() / sampling() / auth_provider() / tool_authorizer()` signatures and bodies are byte-for-byte unchanged"
  - "D-05 enforced: all six `_arc` siblings lifted in one plan, not just the two named in BLDR-01/-02"
  - "D-07 enforced: only `get_tool` added; no `list_tools` / `list_prompts` / `get_sampling_handler`"
  - "D-08 enforced: `get_tool` folded into BLDR-03, not assigned a separate REQ ID"
  - "tool_authorizer_arc mirrors public tool_authorizer (protection-clear + tracing::warn), NOT the donor; this is the only non-mechanical lift"
  - "Behavioral test added for the one semantic deviation; the five other lifts are mechanical and source-grep verified"
metrics:
  duration_minutes: ~45
  tasks_completed: 4
  commits_created: 4
  files_modified: 1
  test_pass_count: "1 new lib test + 2 new doctests + 17 existing builder tests + full quality-gate"
completed: 2026-05-17
---

# Phase 82 Plan 01: Builder DX — `_arc` Lifts and `get_tool` Accessor Summary

Lifts six `_arc` handler-registration methods from the private `ServerCoreBuilder` to the public `ServerBuilder`, adds a `Server::get_tool` accessor symmetric with `get_prompt`, documents the handler-level testing pattern via two D-03-compliant doctests, and proves the one non-mechanical lift (`tool_authorizer_arc` clearing semantics) with a crate-internal behavioral test.

## Objective Recap

External toolkit authors (Phase 83's `pmcp-server-toolkit` and downstream) need to share `Arc<dyn Handler>` between `pmcp::ServerBuilder` and an in-process handler map without writing a 20-line delegating wrapper shim per handler type. Phase 82 Plan 01 closes the public-builder Arc-symmetry gap and adds the missing accessor that the documented handler-level testing pattern requires for tools (it already worked for prompts via `get_prompt`).

## What Shipped

### Seven new public methods on `src/server/mod.rs`

| Symbol | Signature | Position | Purpose |
|--------|-----------|----------|---------|
| `Server::get_tool` | `pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn ToolHandler>>` | mod.rs:515 | Symmetric accessor for the testing pattern (BLDR-03 D-07). |
| `ServerBuilder::tool_arc` | `pub fn tool_arc(mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) -> Self` | mod.rs ~1997 (after `tool()`) | BLDR-01 — share Arc, no shim. |
| `ServerBuilder::prompt_arc` | `pub fn prompt_arc(mut self, name: impl Into<String>, handler: Arc<dyn PromptHandler>) -> Self` | mod.rs ~2537 (after `prompt()`) | BLDR-02 — share Arc, no shim. |
| `ServerBuilder::resources_arc` | `pub fn resources_arc(mut self, handler: Arc<dyn ResourceHandler>) -> Self` | mod.rs ~2705 (after `resources()`) | BLDR-04. |
| `ServerBuilder::sampling_arc` | `pub fn sampling_arc(mut self, handler: Arc<dyn SamplingHandler>) -> Self` | mod.rs ~2870 (after `sampling()`) | BLDR-04 — uses donor's `if is_none` capability auto-enable. |
| `ServerBuilder::auth_provider_arc` | `pub fn auth_provider_arc(mut self, provider: Arc<dyn auth::AuthProvider>) -> Self` | mod.rs ~2940 (after `auth_provider()`) | BLDR-04 — verbatim donor lift. |
| `ServerBuilder::tool_authorizer_arc` | `pub fn tool_authorizer_arc(mut self, authorizer: Arc<dyn auth::ToolAuthorizer>) -> Self` | mod.rs ~2980 (after `tool_authorizer()`) | BLDR-04 — **mirrors public tool_authorizer() semantics, NOT donor** (the only non-mechanical lift). |

### One semantic deviation vs the donor

`tool_authorizer_arc`'s body does **not** match the donor at `src/server/builder.rs:458`. The donor's body is the bare two-liner

```rust
self.tool_authorizer = Some(authorizer);
self
```

because `ServerCoreBuilder` has no `tool_protections` field. The public `ServerBuilder` does have a `tool_protections: HashMap<String, Vec<String>>` field (mod.rs:1758), and at `.build()` time the builder returns `Err` if `tool_protections` is non-empty AND a custom `tool_authorizer` is set. The public sibling `tool_authorizer()` (mod.rs ~2928) handles this by clearing `tool_protections` and emitting a `tracing::warn!` under target `"mcp.auth"` first; the lifted `tool_authorizer_arc` mirrors that semantic verbatim:

```rust
if !self.tool_protections.is_empty() {
    tracing::warn!(target: "mcp.auth", "Setting a custom tool_authorizer clears any previous protect_tool() configurations");
    self.tool_protections.clear();
}
self.tool_authorizer = Some(authorizer);
self
```

This is the **only** of the six lifts where the donor body is incomplete for the public builder.

### One crate-internal behavioral test

Added inside the existing `#[cfg(test)] mod tests` block in `src/server/mod.rs` (~line 4441):

- **Test name:** `tool_authorizer_arc_clears_tool_protections_and_allows_build`
- **Type:** `#[tokio::test]` async unit test
- **What it proves:**
  1. After `builder.protect_tool("delete", ["admin"]).tool_authorizer_arc(Arc::new(NoopAuthorizer))`, the private field `builder.tool_protections.is_empty()` is `true` — proving the `.clear()` call fired.
  2. `builder.build().is_ok()` — proving the mixed-config rejection branch at `build()` does **not** fire, because `_arc()` cleared the protections.
- **Why crate-internal:** access to the private `tool_protections` field requires being inside the same module. The trait impl for the test's `NoopAuthorizer` uses the actual trait methods (`can_access_tool` + `required_scopes_for_tool`) — the plan's template had `authorize(...)` which does not match the trait; this was corrected during execution.

### Two doctest locations

Both doctests are on `impl Server` in `src/server/mod.rs`:

1. **`get_prompt` doctest** — uses `prompt_arc("greet", Arc::new(GreetingPrompt))` in the build chain (double-duty: documents the new Arc surface AND the testing pattern), retrieves via `get_prompt("greet")`, calls `.handle(args, RequestHandlerExtra::default()).await`, asserts on `result.messages.len()`.
2. **`get_tool` doctest** — uses `tool_arc("echo", Arc::new(EchoTool))` in the build chain, retrieves via `get_tool("echo")`, calls `.handle(json!({"msg": "hi"}), RequestHandlerExtra::default()).await`, asserts `result == json!({"echoed": {"msg": "hi"}})`.

Both bodies use the standard hidden-async-wrapper shape `# async fn example() -> pmcp::Result<()> { ... # Ok(()) # }` so top-level `.await` compiles under `cargo test --doc`. Both are tagged `\`\`\`rust` (executable; both compile AND run).

Both rustdoc blocks contain the verbatim D-03 "What this pattern skips" callout:

> "This pattern exercises handler logic only. The JSONRPC dispatch path (`Server::handle_request`) is bypassed, so `auth_provider`, `tool_authorizer`, and `tool_middleware` are **not** invoked. For full-pipeline tests that exercise the security pipeline, drive a real transport (stdio or streamable-http) with a `pmcp::Client`."

Verified literal substrings present (rg backward-context greps):
- `auth_provider` — FOUND in both
- `tool_authorizer` — FOUND in both
- `tool_middleware` — FOUND in both
- `pmcp::Client` — FOUND in both
- `stdio` — FOUND in both

### What this plan did NOT do (negative assertions)

- ✗ Did **not** modify any existing public method's signature or body. `tool() / prompt() / resources() / sampling() / auth_provider() / tool_authorizer()` are byte-for-byte unchanged. (D-04 hold.)
- ✗ Did **not** add `list_tools` / `list_prompts` / `get_sampling_handler` / `get_resources`. (D-07 hold — broader introspection is out of scope for Phase 82.)
- ✗ Did **not** add a `ToolAuthorizer::authorize(...)` invocation in either doctest. (Plan deferred-ideas hold — manual security-pipeline simulation is scope creep.)
- ✗ Did **not** bump the `pmcp` crate version. (Release workflow handles that; per executor instructions.)
- ✗ Did **not** modify any file outside `src/server/mod.rs`. The other workspace files in `git status` (CLAUDE.md, cargo-pmcp/*, pmcp-course/*, etc.) are pre-existing unrelated modifications in the working tree from prior work and are out of scope.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `NoopAuthorizer` trait impl method name mismatch in the plan's test template**

- **Found during:** Task 3 (running `cargo clippy --tests`)
- **Issue:** The plan's literal test template defined `async fn authorize(&self, _ctx: &AuthContext, _tool: &str)` on `NoopAuthorizer`, but the actual `ToolAuthorizer` trait at `src/server/auth/traits.rs:527` requires two methods: `can_access_tool(&self, auth: &AuthContext, tool_name: &str)` and `required_scopes_for_tool(&self, tool_name: &str)`.
- **Fix:** Replaced the single `authorize` method with the two correct trait methods in the test, both returning trivially passing values (`Ok(true)` and `Ok(vec![])`).
- **Files modified:** `src/server/mod.rs` (test body only)
- **Commit:** part of `56b0c298` (task(82-01-03)) — the trait-impl fix was inline before commit, not a separate retry.

### Documentation deviations

**1. Acceptance-criteria line-number ranges loosened by ~7 lines**

The plan's acceptance criteria asserted `prompt_arc` line number "between 2502 and 2530". The actual line ended up at ~2537 because the inserted `tool_arc` (24 lines including rustdoc) shifted downstream numbering more than the plan estimated. The plan's own copy explicitly noted "line range allows for the new tool_arc above shifting downstream numbering", so this is within the plan's authored intent. Placement constraint ("immediately after `prompt()`") is satisfied. No code change needed; this is a documentation note only.

### Auth gates / human-action checkpoints

None.

## Verification Results

| Step | Command | Result |
|------|---------|--------|
| Cargo check | `cargo check -p pmcp --features full` | exit 0 |
| Cargo build | `cargo build -p pmcp --features full` | exit 0 |
| Clippy (lib+tests) | `cargo clippy -p pmcp --features full --lib --tests -- -D warnings` | exit 0 — no issues |
| Behavioral test | `cargo test -p pmcp --features full --lib server::tests::tool_authorizer_arc_clears_tool_protections_and_allows_build` | 1 passed |
| Doctests | `cargo test --doc -p pmcp --features full -- get_tool get_prompt` | passed (executable doctests run) |
| Existing builder tests | `cargo test -p pmcp --features full --lib server::builder` | 17 passed, 0 failed |
| **Full quality-gate** | `make quality-gate` | exit 0 — `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` |
| Count of `_arc` methods | `grep -c '^\s*pub fn (tool|prompt|resources|sampling|auth_provider|tool_authorizer)_arc\b' src/server/mod.rs` | 6 |
| Count of `get_tool` | `grep -c '^\s*pub fn get_tool\b' src/server/mod.rs` | 1 |

## Commits

| # | Hash | Subject |
|---|------|---------|
| 1 | `8de9ad79` | `task(82-01-01): add Server::get_tool accessor symmetric with get_prompt` |
| 2 | `a6555c4a` | `task(82-01-02): lift tool_arc and prompt_arc to public ServerBuilder` |
| 3 | `56b0c298` | `task(82-01-03): lift remaining four _arc methods + behavioral test` |
| 4 | `f0dc4b60` | `task(82-01-04): comprehensive doctests on get_tool + get_prompt with D-03 callout` |

## Requirements Closed

- **BLDR-01** (full) — `tool_arc` on public `ServerBuilder`.
- **BLDR-02** (full) — `prompt_arc` on public `ServerBuilder`.
- **BLDR-03** (partial) — `Server::get_tool` accessor and the doctest portion of the documented handler-level testing pattern. The `tests/in_process_handler_pattern.rs` reference test and the book section are completed in Plans 02 + 03 respectively.
- **BLDR-04** (full) — the four extra `_arc` lifts (`resources_arc`, `sampling_arc`, `auth_provider_arc`, `tool_authorizer_arc`).

## Threat Flags

None. This plan introduces no new attack surface — the six `_arc` lifts are mechanical copies of code already in production via the private builder, the `get_tool` accessor is a structural twin of the existing `get_prompt`, and the doctest changes are documentation rather than runtime code. Threat register entries T-82-01 / T-82-02 / T-82-03 from the plan all hold (accept / accept / mitigate via D-03 callout).

## Self-Check: PASSED

- ✅ `src/server/mod.rs` modified (file exists, contains all seven new symbols).
- ✅ Commit `8de9ad79` exists in `git log` (task 01).
- ✅ Commit `a6555c4a` exists in `git log` (task 02).
- ✅ Commit `56b0c298` exists in `git log` (task 03).
- ✅ Commit `f0dc4b60` exists in `git log` (task 04).
- ✅ `make quality-gate` exit 0 with `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` banner.
- ✅ Behavioral test passes: `cargo test ... tool_authorizer_arc_clears_tool_protections_and_allows_build` → 1 passed.
- ✅ Doctests pass: `cargo test --doc ... get_tool get_prompt` → all pass.
- ✅ No new files created outside the plan scope. No files outside `src/server/mod.rs` modified.
