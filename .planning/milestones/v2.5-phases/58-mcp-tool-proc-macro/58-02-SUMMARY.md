---
phase: 58-mcp-tool-proc-macro
plan: 02
subsystem: macros
tags: [proc-macro, syn, quote, darling, impl-block, tool-handler, arc-sharing, generics]

# Dependency graph
requires:
  - phase: 58-01
    provides: State<T> wrapper, ParamRole enum, classify_param(), McpToolArgs/McpToolAnnotations, generate_tool_info_code
provides:
  - "#[mcp_server] impl-block attribute macro generating per-tool ToolHandler structs"
  - "McpServer trait with register_tools() for bulk tool registration"
  - "ServerBuilder::mcp_server() convenience method accepting any McpServer impl"
  - "Generic impl block support with type parameter propagation (D-25)"
affects: [58-03-integration-tests]

# Tech tracking
tech-stack:
  added: []
  patterns: [impl-block-macro, arc-wrapped-server, per-tool-handler-struct, trait-based-registration]

key-files:
  created:
    - pmcp-macros/src/mcp_server.rs
  modified:
    - pmcp-macros/src/mcp_tool.rs
    - pmcp-macros/src/lib.rs
    - src/server/mod.rs
    - src/lib.rs

key-decisions:
  - "Clone server_type before mutable strip to satisfy borrow checker (immutable borrow of self_ty + mutable strip_mcp_tool_attrs)"
  - "McpToolArgs/McpToolAnnotations fields made pub(crate) for cross-module reuse by mcp_server.rs"
  - "generate_tool_info_code made pub(crate) for shared ToolInfo construction code between mcp_tool and mcp_server"
  - "Per-tool handler structs are module-private (no pub) since only register_tools() creates them"

patterns-established:
  - "Per-tool handler struct pattern: {PascalCase}ToolHandler holding Arc<ServerType>, implementing ToolHandler"
  - "McpServer trait pattern: generated impl delegates to register_tools() which wraps self in Arc and registers all tools"
  - "Attribute stripping: #[mcp_tool] attrs removed from preserved impl block after collection to avoid double-expansion"

requirements-completed: [TOOL-MACRO, STATE-INJECTION]

# Metrics
duration: 5min
completed: 2026-03-21
---

# Phase 58 Plan 02: #[mcp_server] Impl-Block Macro and McpServer Trait Summary

**#[mcp_server] attribute macro processing impl blocks with #[mcp_tool] methods, generating per-tool Arc-wrapped ToolHandler structs, McpServer trait with register_tools(), and ServerBuilder::mcp_server() convenience for single-line bulk registration**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T18:40:13Z
- **Completed:** 2026-03-21T18:45:30Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Full #[mcp_server] impl-block macro in pmcp-macros/src/mcp_server.rs: collects #[mcp_tool] methods, generates per-tool ToolHandler structs with Arc<ServerType>, strips attributes from preserved block
- Generic impl block support (D-25): type parameters and trait bounds propagated through handler structs, ToolHandler impls, and McpServer impl
- McpServer trait in src/server/mod.rs with register_tools() contract, plus ServerBuilder::mcp_server() convenience method
- 8 unit tests covering method collection, attribute stripping, State<T> rejection, sync methods, custom names, extra params, error cases
- 764 existing pmcp lib tests pass unchanged, zero warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement #[mcp_server] impl-block macro** - `b73bfb2` (feat)
2. **Task 2: Add McpServer trait and mcp_server() builder convenience** - `a820063` (feat)

## Files Created/Modified

- `pmcp-macros/src/mcp_server.rs` - Full #[mcp_server] expansion: expand_mcp_server, collect_tool_methods, per-tool handler generation, McpServer impl generation, attribute stripping, 8 tests
- `pmcp-macros/src/mcp_tool.rs` - Made McpToolArgs/McpToolAnnotations fields and generate_tool_info_code pub(crate) for cross-module access
- `pmcp-macros/src/lib.rs` - Added mod mcp_server and #[proc_macro_attribute] mcp_server entry point
- `src/server/mod.rs` - McpServer trait definition and ServerBuilder::mcp_server() method
- `src/lib.rs` - Added McpServer to server re-export block

## Decisions Made

- **Clone server_type before mutable strip:** Borrow checker requires exclusive mutable access for strip_mcp_tool_attrs, but server_type is borrowed from input. Cloning the Box<Type> resolves the conflict cleanly.
- **pub(crate) visibility for McpToolArgs fields:** The struct and its fields are pub but fields were private. Making them pub(crate) allows mcp_server.rs to reuse the same darling-parsed attribute struct without duplication.
- **generate_tool_info_code shared:** Both standalone #[mcp_tool] and impl-block #[mcp_server] need identical ToolInfo construction logic (branching on annotations). Making it pub(crate) avoids code duplication.
- **Private handler structs:** Per-tool handler structs (e.g., QueryToolHandler) are not pub since they are only instantiated by the generated register_tools() method. Users never interact with them directly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed borrow checker conflict with server_type and strip_mcp_tool_attrs**
- **Found during:** Task 1 (mcp_server.rs initial compilation)
- **Issue:** `server_type = &input.self_ty` created an immutable borrow of `input`, but `strip_mcp_tool_attrs(&mut input)` requires mutable access. The immutable borrow was still live in the `quote!` block after the mutable call.
- **Fix:** Changed `let server_type = &input.self_ty` to `let server_type = input.self_ty.clone()` to release the borrow.
- **Files modified:** `pmcp-macros/src/mcp_server.rs`
- **Verification:** cargo build -p pmcp-macros succeeds cleanly.
- **Committed in:** `b73bfb2`

**2. [Rule 1 - Bug] Removed unused FnArg import**
- **Found during:** Task 1 (compiler warning)
- **Issue:** `FnArg` was imported but not directly used (classify_param takes &FnArg internally).
- **Fix:** Removed unused import.
- **Files modified:** `pmcp-macros/src/mcp_server.rs`
- **Committed in:** `b73bfb2`

**3. [Rule 1 - Bug] Fixed Debug requirement on ToolMethodInfo in tests**
- **Found during:** Task 1 (test compilation)
- **Issue:** Tests used `result.unwrap_err()` which requires `T: Debug` on the Ok type. `ToolMethodInfo` contains `syn::Expr` and `Type` which make deriving Debug impractical.
- **Fix:** Replaced `unwrap_err()` with pattern matching (`match result { Err(e) => ..., Ok(_) => panic!(...) }`).
- **Files modified:** `pmcp-macros/src/mcp_server.rs`
- **Committed in:** `b73bfb2`

---

**Total deviations:** 3 auto-fixed (3 bug fixes)
**Impact on plan:** All fixes are standard Rust compilation issues. No scope change.

## Issues Encountered

None beyond the compilation issues documented above as deviations.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- #[mcp_server] macro and McpServer trait are ready for integration testing (Plan 03)
- Both standalone #[mcp_tool] (Plan 01) and impl-block #[mcp_server] (this plan) compile cleanly
- ServerBuilder::mcp_server() is available for ergonomic registration patterns
- 30 pmcp-macros unit tests pass (13 mcp_common + 8 mcp_server + 9 others)

## Self-Check: PASSED

- All 4 modified files verified present on disk
- Both task commits (b73bfb2, a820063) verified in git log
- expand_mcp_server function present in mcp_server.rs
- McpServer trait present in src/server/mod.rs
- mcp_server() method present on ServerBuilder
- McpServer in src/lib.rs re-exports
- mod mcp_server declared in pmcp-macros/src/lib.rs
- cargo build -p pmcp-macros: clean (0 warnings)
- cargo build -p pmcp --lib: clean
- cargo test -p pmcp --lib: 764 passed, 0 failed

---
*Phase: 58-mcp-tool-proc-macro*
*Completed: 2026-03-21*
