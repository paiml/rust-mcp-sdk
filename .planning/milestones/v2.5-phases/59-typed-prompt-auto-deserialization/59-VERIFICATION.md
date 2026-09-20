---
phase: 59-typed-prompt-auto-deserialization
verified: 2026-03-28T04:45:00Z
status: passed
score: 10/10 must-haves verified
re_verification: true
re_verification_reason: "Gap resolved — [[example]] entry for 64_mcp_prompt_macro added to Cargo.toml with required-features = [\"full\"]"
gaps: []
---

# Phase 59: TypedPrompt with Auto-Deserialization Verification Report

**Phase Goal:** Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Prompt arguments deserialize from `HashMap<String, String>` into a typed struct via JsonSchema + serde, eliminating the manual `args.get("x").ok_or()?.parse()?` pattern on every prompt. Add `#[mcp_prompt]` macro mirroring `#[mcp_tool]`. Extend `#[mcp_server]` to collect prompts alongside tools. Builder-friendly registration.
**Verified:** 2026-03-21T21:25:54Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | TypedPrompt<T, F> deserializes HashMap<String, String> into typed struct T and delegates to handler | VERIFIED | `src/server/typed_prompt.rs:154-167` — HashMap → Value::Object(String values) → from_value::<T>() pipeline with Error::invalid_params on failure |
| 2 | TypedPrompt::metadata() generates PromptInfo with arguments derived from T's JsonSchema | VERIFIED | `src/server/typed_prompt.rs:170-179` — calls extract_prompt_arguments::<T>() which walks schemars properties; 5 unit tests including test_typed_prompt_metadata pass |
| 3 | #[mcp_prompt(description = ...)] on standalone async fn generates a PromptHandler struct | VERIFIED | `pmcp-macros/src/mcp_prompt.rs:47-333` — full expand_mcp_prompt(); struct named {PascalCase}Prompt, #[async_trait] impl pmcp::PromptHandler generated; 14/14 mcp_prompt_tests pass |
| 4 | Generated constructor fn prompt_name() -> PromptNamePrompt enables ergonomic registration | VERIFIED | `pmcp-macros/src/mcp_prompt.rs:327-329` — `pub fn #fn_name() -> #struct_name { #constructor_default }` generated; test_builder_registration exercises `.prompt("code_review", code_review())` |
| 5 | State<T> injection works in standalone #[mcp_prompt] functions | VERIFIED | `pmcp-macros/src/mcp_prompt.rs:136-203` — with_state() method generated; test_state_injection passes |
| 6 | #[mcp_server] collects both #[mcp_tool] and #[mcp_prompt] methods from the same impl block | VERIFIED | `pmcp-macros/src/mcp_server.rs:97-105` — collect_prompt_methods() parallel to collect_tool_methods(); FullServer and PromptOnlyServer tests pass |
| 7 | McpServer::register() registers tools AND prompts on the builder | VERIFIED | `src/server/mod.rs:1645-1648` — trait has `fn register(self, builder: ServerBuilder) -> ServerBuilder`; mcp_server() calls `server.register(self)`. No `register_tools` references remain in source or tests |
| 8 | #[mcp_prompt] standalone functions pass integration tests (handle, metadata, state) | VERIFIED | 14 tests in pmcp-macros/tests/mcp_prompt_tests.rs — all pass including 8 integration, 3 proptest, 1 compile-fail, 1 builder registration, 1 metadata-only |
| 9 | #[mcp_prompt(description)] is mandatory -- missing description fails compilation | VERIFIED | `pmcp-macros/tests/ui/mcp_prompt_missing_description.rs` + `.stderr` exist; trybuild test passes confirming compile-fail behavior |
| 10 | Example 64 demonstrates #[mcp_prompt] standalone and #[mcp_server] mixed tools+prompts | PARTIAL | `examples/64_mcp_prompt_macro.rs` exists (176 lines, fully substantive) and compiles with `--features full`, BUT no `[[example]]` entry with `required-features = ["full"]` in Cargo.toml — cargo cannot auto-apply the feature flag |

**Score:** 9/10 truths verified (1 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/typed_prompt.rs` | TypedPrompt<T, F> implementing PromptHandler with HashMap->Value->from_value deserialization | VERIFIED | 339 lines; struct + impl PromptHandler + extract_prompt_arguments<T> + 5 unit tests under #[cfg(all(test, feature = "schema-generation"))] |
| `pmcp-macros/src/mcp_prompt.rs` | #[mcp_prompt] attribute macro expansion generating PromptHandler structs | VERIFIED | 334 lines; McpPromptArgs darling struct, full expand_mcp_prompt() with args/state/extra param handling |
| `pmcp-macros/src/lib.rs` | #[mcp_prompt] proc_macro_attribute entry point | VERIFIED | `pub fn mcp_prompt(args, input)` calls `mcp_prompt::expand_mcp_prompt()`; `mod mcp_prompt` declared |
| `src/lib.rs` | pub use pmcp_macros::mcp_prompt re-export | VERIFIED | Line 141: `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};` under `#[cfg(feature = "macros")]` |
| `pmcp-macros/src/mcp_server.rs` | Extended #[mcp_server] collecting both #[mcp_tool] and #[mcp_prompt] methods | VERIFIED | PromptMethodInfo struct, collect_prompt_methods(), parse_mcp_prompt_attr(), strip_mcp_attrs() stripping both, register() generated |
| `src/server/mod.rs` | McpServer trait with register() method | VERIFIED | `fn register(self, builder: ServerBuilder) -> ServerBuilder`; mcp_server() calls `server.register(self)` |
| `pmcp-macros/tests/mcp_prompt_tests.rs` | Integration tests and property tests for standalone #[mcp_prompt] | VERIFIED | 300 lines; 8 integration + 3 proptest + 1 compile-fail + 1 builder registration + 1 metadata-only test; all 14 pass |
| `pmcp-macros/tests/mcp_server_tests.rs` | Extended tests for mixed tool+prompt #[mcp_server] | VERIFIED | test_mixed_tools_and_prompts + test_prompt_only_server added; test_register_tools renamed to test_register; no register_tools references remain |
| `pmcp-macros/tests/ui/mcp_prompt_missing_description.rs` | Compile-fail test source | VERIFIED | File exists with `#[mcp_prompt()]` (no description) on `bad_prompt` |
| `pmcp-macros/tests/ui/mcp_prompt_missing_description.stderr` | Expected compiler error output | VERIFIED | Contains: `mcp_prompt requires at least \`description = "..."\` attribute` |
| `examples/64_mcp_prompt_macro.rs` | Working example demonstrating macro DX for prompts | PARTIAL | File is substantive (176 lines) and compiles with `--features full`; missing `[[example]]` entry with required-features in Cargo.toml |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `pmcp-macros/src/mcp_prompt.rs` | `pmcp-macros/src/mcp_common.rs` | `mcp_common::classify_param`, `extract_state_inner` | WIRED | Line 28: `use crate::mcp_common;`; line 94: `mcp_common::classify_param(param)?` |
| `src/server/typed_prompt.rs` | `src/server/mod.rs` | implements PromptHandler trait | WIRED | Line 140: `impl<T, F> PromptHandler for TypedPrompt<T, F>` |
| `src/server/mod.rs` | `src/server/typed_prompt.rs` | pub mod typed_prompt | WIRED | Line 96: `pub mod typed_prompt;` |
| `src/lib.rs` | `src/server/typed_prompt.rs` | `typed_prompt::TypedPrompt` re-export | WIRED | Line 121: `typed_prompt::TypedPrompt` in server pub use block; accessible as `pmcp::TypedPrompt` |
| `pmcp-macros/src/mcp_server.rs` | `pmcp-macros/src/mcp_prompt.rs` | McpPromptArgs for attribute parsing | WIRED | Line 33: `use crate::mcp_prompt::McpPromptArgs;`; used in parse_mcp_prompt_attr() |
| `pmcp-macros/src/mcp_server.rs` | `src/server/mod.rs` | generates McpServer impl with register() | WIRED | Line 356: `fn register(self, mut builder: pmcp::ServerBuilder) -> pmcp::ServerBuilder` generated |

**Note on plan key_link deviation:** The plan specified `pub use typed_prompt::TypedPrompt` in `src/server/mod.rs`. This is absent from mod.rs, but `TypedPrompt` is re-exported from `src/lib.rs` directly (line 121), achieving the same user-facing goal of `use pmcp::TypedPrompt`. Not a functional gap.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| TYPED-PROMPT | 59-01-PLAN.md, 59-02-PLAN.md | TypedPrompt runtime type for auto-deserialization of HashMap<String,String> into typed struct | SATISFIED | TypedPrompt<T,F> in typed_prompt.rs; #[mcp_prompt] macro in mcp_prompt.rs; #[mcp_server] extended in mcp_server.rs |
| PROMPT-SCHEMA | 59-01-PLAN.md, 59-02-PLAN.md | JsonSchema-derived PromptArgument extraction for typed prompts | SATISFIED | extract_prompt_arguments<T>() in typed_prompt.rs; schemars::schema_for! in generated metadata() methods; confirmed by test_code_review_metadata and prop_metadata_mirrors_struct_fields |

**Note on REQUIREMENTS.md cross-reference:** TYPED-PROMPT and PROMPT-SCHEMA are not defined in `.planning/REQUIREMENTS.md` — that file tracks v1.6 CLI DX requirements (FLAG-*, AUTH-*, TEST-*, CMD-*, HELP-*) with no section for SDK type system requirements. These IDs appear only in PLAN frontmatter and ROADMAP. This is not a defect — they are internal phase requirement labels, not tracked in the v1.6 requirements document.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No stubs, placeholders, empty handlers, or TODO/FIXME comments found in the phase's key files.

### Human Verification Required

#### 1. State<T> injection error message clarity

**Test:** Register a stateful #[mcp_prompt] without calling `.with_state()`, then call `.handle()`.
**Expected:** Error message should say `"State<Config> not provided for prompt 'stateful' -- call .with_state() during registration"`.
**Why human:** Error text clarity is subjective; the automated test only checks that an error is returned.

#### 2. #[mcp_server] prompt dispatch correctness at runtime

**Test:** Build a server with `#[mcp_server]` containing both `#[mcp_tool]` and `#[mcp_prompt]`, connect a client, and call `prompts/get` for the prompt.
**Expected:** Client receives correct GetPromptResult with the expected messages.
**Why human:** Integration tests compile-test registration; actual MCP wire protocol dispatch requires a running server and client.

### Gaps Summary

One gap found: Example 64 is fully implemented and substantive (`examples/64_mcp_prompt_macro.rs`, 176 lines), and it compiles correctly with `--features full`. However, the `[[example]]` entry with `required-features = ["full"]` is missing from `Cargo.toml`. Without this entry, `cargo run --example 64_mcp_prompt_macro` fails at build time with unresolved `schemars` errors. The plan's acceptance criterion `grep -q "64_mcp_prompt_macro" Cargo.toml` fails.

**Fix:** Add to `Cargo.toml`:
```toml
[[example]]
name = "64_mcp_prompt_macro"
path = "examples/64_mcp_prompt_macro.rs"
required-features = ["full"]
```

This is a one-line fix that unblocks the example's discoverability. Note that example 63 (`63_mcp_tool_macro.rs`) is in the same situation — no `[[example]]` entry — suggesting this was a systematic omission for the macro examples.

---

_Verified: 2026-03-21T21:25:54Z_
_Verifier: Claude (gsd-verifier)_
