---
phase: 59-typed-prompt-auto-deserialization
plan: 02
subsystem: macros
tags: [proc-macro, prompts, mcp-server, proptest, typed-prompt]

# Dependency graph
requires:
  - phase: 59-01
    provides: "#[mcp_prompt] standalone macro, TypedPrompt runtime type, McpPromptArgs"
  - phase: 58-02
    provides: "#[mcp_server] impl block macro, McpServer trait with register_tools"
provides:
  - "Extended #[mcp_server] collecting both #[mcp_tool] and #[mcp_prompt] methods"
  - "McpServer::register() (renamed from register_tools) registering tools AND prompts"
  - "PromptHandler structs generated for impl-block prompts with Arc<ServerType>"
  - "14 integration tests for standalone #[mcp_prompt] (handle, metadata, state, sync, extra)"
  - "3 proptest property tests for deserialization invariants"
  - "Compile-fail test for missing description"
  - "Example 64 demonstrating full prompt macro DX"
affects: [phase-60, documentation, mcp-server-macro-users]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Parallel PromptMethodInfo/ToolMethodInfo collection in mcp_server.rs"
    - "HashMap<String, String> -> serde_json::Value deserialization for prompts in impl blocks"
    - "strip_mcp_attrs removes both mcp_tool and mcp_prompt attributes"

key-files:
  created:
    - pmcp-macros/tests/mcp_prompt_tests.rs
    - pmcp-macros/tests/ui/mcp_prompt_missing_description.rs
    - pmcp-macros/tests/ui/mcp_prompt_missing_description.stderr
    - examples/64_mcp_prompt_macro.rs
  modified:
    - pmcp-macros/src/mcp_server.rs
    - src/server/mod.rs
    - pmcp-macros/tests/mcp_server_tests.rs

key-decisions:
  - "mcp_prompt inside #[mcp_server] does not require separate import -- handled internally by the server macro"
  - "register_tools renamed to register per D-15 -- breaking change for manual McpServer impls"
  - "PromptMethodInfo has no return_type or annotations fields -- prompts are simpler than tools"

patterns-established:
  - "Mixed tool+prompt #[mcp_server] impl block pattern with parallel collection and registration"
  - "Property tests for macro-generated code using proptest with tokio::runtime::Runtime"

requirements-completed: [TYPED-PROMPT, PROMPT-SCHEMA]

# Metrics
duration: 8min
completed: 2026-03-21
---

# Phase 59 Plan 02: Prompt Macro Server Integration Summary

**Extended #[mcp_server] to collect both #[mcp_tool] and #[mcp_prompt] methods, renamed register_tools to register, added 16 tests including 3 proptest property tests and compile-fail, plus example 64**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T21:06:20Z
- **Completed:** 2026-03-21T21:14:20Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Extended #[mcp_server] macro to collect both #[mcp_tool] and #[mcp_prompt] methods from the same impl block, generating per-prompt PromptHandler structs
- Renamed McpServer::register_tools() to McpServer::register() on both the trait and all generated code
- Created 14 standalone #[mcp_prompt] integration tests covering handle, metadata, no-args, optional args, name override, state injection, sync, extra, and builder registration
- Added 3 proptest property tests verifying: missing required args always errors, metadata mirrors struct fields, no panics on arbitrary input
- Created compile-fail test ensuring missing description is caught at compile time
- Created example 64 demonstrating standalone and impl-block prompt macro DX

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend #[mcp_server] for prompts, rename register_tools to register** - `eafc259` (feat)
2. **Task 2: Integration tests, property tests, compile-fail tests, and example 64** - `490f8da` (test)

## Files Created/Modified
- `pmcp-macros/src/mcp_server.rs` - Extended with PromptMethodInfo, collect_prompt_methods, prompt handler generation, rename strip_mcp_tool_attrs to strip_mcp_attrs, rename register_tools to register
- `src/server/mod.rs` - McpServer trait: register_tools renamed to register; mcp_server() updated to call register()
- `pmcp-macros/tests/mcp_server_tests.rs` - Renamed test_register_tools to test_register, added mixed tool+prompt test and prompt-only test
- `pmcp-macros/tests/mcp_prompt_tests.rs` - 14 tests: 8 integration + 3 proptest + compile-fail + builder registration
- `pmcp-macros/tests/ui/mcp_prompt_missing_description.rs` - Compile-fail test source
- `pmcp-macros/tests/ui/mcp_prompt_missing_description.stderr` - Expected compiler error output
- `examples/64_mcp_prompt_macro.rs` - Example with standalone prompts, state injection, and mixed #[mcp_server] impl block

## Decisions Made
- mcp_prompt inside #[mcp_server] does not require a separate `use pmcp_macros::mcp_prompt` import since the server macro processes and strips the attribute internally before rustc resolves it
- register_tools renamed to register per D-15 -- this is a breaking change for any manual McpServer trait implementations (none exist since the trait was introduced in Phase 58)
- PromptMethodInfo omits return_type and annotations fields that ToolMethodInfo has -- prompts return GetPromptResult directly and have no MCP standard annotations

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unused import warnings in tests and example**
- **Found during:** Task 2
- **Issue:** `mcp_prompt` import in mcp_server_tests.rs and `mcp_tool`/`json`/`Value` imports in example 64 were unused because #[mcp_server] handles attribute resolution internally
- **Fix:** Removed unused imports to eliminate compiler warnings
- **Files modified:** pmcp-macros/tests/mcp_server_tests.rs, examples/64_mcp_prompt_macro.rs
- **Verification:** `cargo check` produces zero warnings
- **Committed in:** 490f8da (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor -- unused import cleanup. No scope creep.

## Issues Encountered
None

## Known Stubs
None -- all prompts and tools are fully wired with real logic.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 59 is complete: both Plan 01 (TypedPrompt + #[mcp_prompt] standalone) and Plan 02 (#[mcp_server] extension + tests + example) are shipped
- Users can now use #[mcp_prompt] for standalone prompts and mix #[mcp_tool] + #[mcp_prompt] in #[mcp_server] impl blocks

## Self-Check: PASSED

All 8 files verified present. Both commits (eafc259, 490f8da) verified in git log. All 25 acceptance criteria pass.

---
*Phase: 59-typed-prompt-auto-deserialization*
*Completed: 2026-03-21*
