---
phase: 59-typed-prompt-auto-deserialization
plan: 01
subsystem: server
tags: [proc-macro, typed-prompt, schemars, darling, prompt-handler]

# Dependency graph
requires:
  - phase: 58-mcp-tool-proc-macro
    provides: "#[mcp_tool] pattern, mcp_common.rs shared helpers, State<T> injection"
provides:
  - "TypedPrompt<T, F> runtime type implementing PromptHandler with auto-deserialization"
  - "#[mcp_prompt] proc-macro attribute for standalone prompt functions"
  - "mcp_prompt re-exported from pmcp crate root"
affects: [59-02, mcp-server-impl-prompts, typed-prompt-examples]

# Tech tracking
tech-stack:
  added: []
  patterns: ["TypedPrompt mirrors TypedTool for prompts", "HashMap->Value::String->from_value deserialization for string-only MCP args"]

key-files:
  created:
    - src/server/typed_prompt.rs
    - pmcp-macros/src/mcp_prompt.rs
  modified:
    - src/server/mod.rs
    - pmcp-macros/src/lib.rs
    - src/lib.rs

key-decisions:
  - "TypedPrompt requires JsonSchema bound unconditionally (no feature flag guard) since schema IS the type's value proposition"
  - "String-only argument limitation documented prominently on TypedPrompt struct and generated macro docs"
  - "No annotations/ui fields on McpPromptArgs -- prompts are simpler than tools"
  - "Prompts return GetPromptResult directly with no serialization wrapper (unlike tools which serialize to Value)"

patterns-established:
  - "TypedPrompt<T,F> mirrors TypedTool<T,F> pattern: schema-at-construction, typed deserialization in handle()"
  - "#[mcp_prompt] mirrors #[mcp_tool] pattern: darling args, classify_param, renamed internal fn, constructor fn"

requirements-completed: [TYPED-PROMPT, PROMPT-SCHEMA]

# Metrics
duration: 5min
completed: 2026-03-21
---

# Phase 59 Plan 01: TypedPrompt Runtime Type and #[mcp_prompt] Macro Summary

**TypedPrompt<T,F> with HashMap-to-Value deserialization and #[mcp_prompt] proc-macro generating PromptHandler structs from annotated functions with JsonSchema argument extraction**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T20:57:43Z
- **Completed:** 2026-03-21T21:02:55Z
- **Tasks:** 1
- **Files modified:** 5

## Accomplishments
- Created TypedPrompt<T,F> implementing PromptHandler with automatic PromptArgument extraction from JsonSchema properties
- Created #[mcp_prompt] proc-macro with description (mandatory), name override, State<T> injection, and async/sync support
- HashMap<String,String> -> Value::Object(String values) -> from_value<T> deserialization pipeline for MCP string-only prompt arguments
- String-only limitation documented on both TypedPrompt struct doc and generated macro struct doc
- 5 unit tests validating metadata generation, successful handle, missing required field error, debug formatting, and no-args prompts

## Task Commits

Each task was committed atomically:

1. **Task 1: Create TypedPrompt runtime type and #[mcp_prompt] macro** - `1cdc9ef` (feat)

## Files Created/Modified
- `src/server/typed_prompt.rs` - TypedPrompt<T,F> runtime type with PromptHandler impl and schema-derived arguments
- `pmcp-macros/src/mcp_prompt.rs` - #[mcp_prompt] attribute macro expansion with McpPromptArgs darling struct
- `pmcp-macros/src/lib.rs` - Added mod mcp_prompt and proc_macro_attribute entry point
- `src/server/mod.rs` - Added pub mod typed_prompt
- `src/lib.rs` - Added TypedPrompt re-export and mcp_prompt macro re-export

## Decisions Made
- TypedPrompt requires `JsonSchema` bound unconditionally (no `#[cfg(feature = "schema-generation")]` guard) -- schema extraction is the core value proposition of the type; without it, use SimplePrompt instead
- String-only argument limitation documented prominently -- MCP prompt arguments are `HashMap<String, String>` so all struct fields must be `String` or `Option<String>` for correct deserialization
- No annotations or ui fields on McpPromptArgs (unlike McpToolArgs) -- prompts are simpler than tools per the MCP spec
- Prompts return `GetPromptResult` directly from handler (no `serde_json::to_value()` wrapper) unlike tools which serialize to `Value`
- Constructor function uses `{fn_name}Prompt` suffix (not `Tool`) per D-13

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functionality is fully wired and operational.

## Next Phase Readiness
- TypedPrompt and #[mcp_prompt] are ready for Plan 02 (compile tests and integration example)
- The macro generates structs registered via `server_builder.prompt("name", prompt_fn())`
- State injection pattern matches #[mcp_tool] exactly: `prompt_fn().with_state(arc_state)`

---
*Phase: 59-typed-prompt-auto-deserialization*
*Completed: 2026-03-21*
