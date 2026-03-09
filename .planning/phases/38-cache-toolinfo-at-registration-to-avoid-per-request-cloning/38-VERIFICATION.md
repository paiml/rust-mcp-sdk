---
phase: 38-cache-toolinfo-at-registration-to-avoid-per-request-cloning
verified: 2026-03-06T23:30:00Z
status: passed
score: 6/6 must-haves verified
re_verification: false
---

# Phase 38: Cache ToolInfo at Registration Verification Report

**Phase Goal:** Cache ToolInfo and PromptInfo at builder registration time so handle_list_tools, handle_call_tool, handle_list_prompts, and task routing use cached metadata instead of calling handler.metadata() per request
**Verified:** 2026-03-06T23:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                              | Status     | Evidence                                                                                     |
|----|----------------------------------------------------------------------------------------------------|------------|----------------------------------------------------------------------------------------------|
| 1  | handle_list_tools returns cached ToolInfo without calling handler.metadata()                       | VERIFIED   | core.rs:256 — `self.tool_infos.values().cloned().collect()` replaces per-handler iteration  |
| 2  | handle_call_tool uses cached ToolInfo for widget enrichment without calling handler.metadata()     | VERIFIED   | core.rs:354 — `self.tool_infos.get(&req.name)` replaces `handler.metadata()`               |
| 3  | handle_list_prompts returns cached PromptInfo without calling handler.metadata()                   | VERIFIED   | core.rs:363 — `self.prompt_infos.values().cloned().collect()` replaces per-handler iteration|
| 4  | Task routing uses cached ToolInfo execution config without calling handler.metadata()              | VERIFIED   | core.rs:690 — `self.tool_infos.get(&req.name).and_then(|m| m.execution.clone())`           |
| 5  | WasmMcpServer list_tools and list_prompts use cached info without calling tool.info()/prompt.info()| VERIFIED   | wasm_server.rs:146 and :268 — both use `self.tool_infos/prompt_infos.values().cloned()`    |
| 6  | prompt_workflow() uses cached tool_infos instead of calling handler.metadata()                    | VERIFIED   | builder.rs:674 — iterates `&self.tool_infos` map, no handler.metadata() calls              |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact                      | Expected                                                                        | Status    | Details                                                                                                        |
|-------------------------------|---------------------------------------------------------------------------------|-----------|----------------------------------------------------------------------------------------------------------------|
| `src/server/builder.rs`       | tool_infos and prompt_infos cache fields in ServerCoreBuilder, populated at registration | VERIFIED  | Lines 62-64: fields declared. Lines 94-95: initialized in new(). Lines 140-225: populated in tool/prompt methods. Line 674: used in prompt_workflow(). Lines 785-786: passed to ServerCore::new(). |
| `src/server/core.rs`          | tool_infos and prompt_infos fields in ServerCore, used in all 4 hot-path call sites | VERIFIED  | Lines 122-126: fields declared. Lines 186-187: new() params. Lines 202-203: stored in Self. Lines 256, 354, 363, 690: all 4 hot paths use cache. |
| `src/server/wasm_server.rs`   | tool_infos and prompt_infos fields in WasmMcpServerBuilder and WasmMcpServer   | VERIFIED  | Lines 62-64 (WasmMcpServer). Lines 298-301 (WasmMcpServerBuilder). Lines 314-315: initialized. Lines 341/363: populated at registration. Lines 381-382: passed through build(). Lines 146/268: used in handlers. |

### Key Link Verification

| From                                    | To                             | Via                                                              | Status   | Details                                                                        |
|-----------------------------------------|--------------------------------|------------------------------------------------------------------|----------|--------------------------------------------------------------------------------|
| `src/server/builder.rs`                 | `src/server/core.rs`           | build() passes tool_infos and prompt_infos to ServerCore::new() | WIRED    | builder.rs:785-786 passes self.tool_infos, self.prompt_infos to ServerCore::new() |
| `src/server/builder.rs tool()`          | `src/server/builder.rs tool_infos` | handler.metadata() called once at registration, stored          | WIRED    | builder.rs:140-144: metadata() called, inserted into tool_infos before tools  |
| `src/server/core.rs handle_list_tools`  | `src/server/core.rs tool_infos`| self.tool_infos.values().cloned().collect()                      | WIRED    | core.rs:256: confirmed exact pattern from plan                                 |

### Requirements Coverage

| Requirement | Source Plan | Description                                       | Status     | Evidence                                                                 |
|-------------|-------------|---------------------------------------------------|------------|--------------------------------------------------------------------------|
| CACHE-01    | 38-01-PLAN.md | Cache ToolInfo/PromptInfo at registration time, eliminate per-request metadata() calls in hot paths | SATISFIED  | All 6 per-request call sites replaced with cache lookups. Zero remaining handler.metadata() calls in production hot paths of core.rs and wasm_server.rs. |

**Note on CACHE-01:** This requirement ID appears in ROADMAP.md (**Requirements**: CACHE-01) and the PLAN frontmatter, but it is not defined as a standalone entry in `.planning/REQUIREMENTS.md`. The REQUIREMENTS.md file does not contain a `CACHE-01` definition. The requirement is effectively defined inline in the ROADMAP phase description. This is an informational gap in the planning documentation only — the implementation itself fully satisfies the stated intent.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/core.rs` | 906-916 | `handler.metadata()` call inside `build_tool_infos()` | Info | This is a `#[cfg(test)]` helper function that mirrors builder logic for test setup. It is NOT a hot-path request handler. No impact on production behavior. |

No blockers or warnings found. The single informational item is intentional test scaffolding.

### Human Verification Required

None. All must-haves are verifiable programmatically, and the build passes cleanly.

### Gaps Summary

No gaps. All 6 observable truths are satisfied by the actual codebase. The implementation exactly matches the plan's intent:

- `ServerCoreBuilder` and `WasmMcpServerBuilder` both have `tool_infos` and `prompt_infos` fields initialized as empty `HashMap`s in `new()`.
- All 5 builder registration methods (`tool`, `tool_arc`, `prompt`, `prompt_arc`, `prompt_workflow`) populate the caches at registration time by calling `handler.metadata()` or `tool.info()` once.
- `ServerCore::new()` and `WasmMcpServer::build()` accept and store the cache fields, then pass them through.
- All 6 identified per-request hot-path call sites have been replaced with cache lookups: `handle_list_tools` (core + wasm), `handle_call_tool` widget enrichment, `handle_list_prompts` (core + wasm), and task routing.
- `prompt_workflow()` iterates `self.tool_infos` directly instead of calling `handler.metadata()` per tool.
- `cargo build` succeeds with zero errors.
- `cargo clippy -- -D warnings` passes with zero warnings.

---

_Verified: 2026-03-06T23:30:00Z_
_Verifier: Claude (gsd-verifier)_
