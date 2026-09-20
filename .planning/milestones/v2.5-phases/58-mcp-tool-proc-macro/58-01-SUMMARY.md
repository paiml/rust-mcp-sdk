---
phase: 58-mcp-tool-proc-macro
plan: 01
subsystem: macros
tags: [proc-macro, darling, syn, quote, state-injection, tool-handler, schema-generation]

# Dependency graph
requires:
  - phase: 54-protocol-2025-11-25
    provides: ToolHandler trait, ToolInfo constructors, ToolAnnotations builder
provides:
  - State<T> wrapper type for shared state injection
  - ParamRole enum and classify_param for parameter type detection
  - "#[mcp_tool] attribute macro expansion generating ToolHandler impls"
  - Constructor function pattern for ergonomic tool registration
affects: [58-02-mcp-server-macro, 58-03-integration-tests]

# Tech tracking
tech-stack:
  added: []
  patterns: [state-extractor, parameter-classification, macro-expansion]

key-files:
  created:
    - src/server/state.rs
    - pmcp-macros/src/mcp_common.rs
    - pmcp-macros/src/mcp_tool.rs
  modified:
    - src/server/mod.rs
    - src/lib.rs
    - pmcp-macros/src/lib.rs

key-decisions:
  - "Manual Clone impl on State<T> to avoid requiring T: Clone (Arc<T> is always Clone)"
  - "allow(dead_code) on ParamRole::State.full_ty -- reserved for mcp_server expansion"
  - "Schema generation unconditional (no cfg(feature = schema-generation) guard) -- schema IS the macro value proposition"
  - "Branching ToolInfo constructors (with_annotations vs new) since ToolInfo has no set_annotations method"

patterns-established:
  - "State<T> extractor: wraps Arc<T>, auto-derefs, From<Arc<T>> and From<T>"
  - "Parameter classification: classify_param inspects type (not position) for Args/State/Extra/SelfRef"
  - "Macro expansion: generates {PascalCase}Tool struct + ToolHandler impl + constructor fn"
  - "param_order tracking: preserves user's parameter order for correct call-site argument passing"

requirements-completed: [TOOL-MACRO, STATE-INJECTION]

# Metrics
duration: 6min
completed: 2026-03-21
---

# Phase 58 Plan 01: State<T> Extractor, Parameter Classification, and #[mcp_tool] Macro Summary

**State<T> wrapper type with Deref/From/Clone, parameter classification infrastructure (ParamRole enum with 4 roles), and standalone #[mcp_tool] attribute macro generating ToolHandler structs with auto schema, state injection, annotations, and sync/async auto-detection**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-21T18:30:58Z
- **Completed:** 2026-03-21T18:37:19Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- State<T> wrapper type in src/server/state.rs with Deref, From<Arc<T>>, From<T>, manual Clone (no T: Clone bound), AsRef -- re-exported at pmcp::State
- Parameter classification in pmcp-macros/src/mcp_common.rs with ParamRole enum (Args/State/Extra/SelfRef), type detection, schema generation helpers
- Full #[mcp_tool] macro expansion in pmcp-macros/src/mcp_tool.rs generating ToolHandler struct, handle()/metadata() impls, with_state() builder, constructor function
- 18 unit tests passing (13 mcp_common + 5 State<T>)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create State<T> type and mcp_common parameter classification** - `ebaac3b` (feat)
2. **Task 2: Implement #[mcp_tool] standalone macro expansion** - `0828447` (feat)

## Files Created/Modified

- `src/server/state.rs` - State<T> wrapper type with Deref, From, Clone, AsRef + 5 unit tests
- `src/server/mod.rs` - Added `pub mod state;` module declaration
- `src/lib.rs` - Added `state::State` to server re-exports
- `pmcp-macros/src/mcp_common.rs` - ParamRole enum, classify_param, type detection, schema codegen + 13 unit tests
- `pmcp-macros/src/mcp_tool.rs` - McpToolArgs (darling), McpToolAnnotations, expand_mcp_tool, ToolInfo codegen
- `pmcp-macros/src/lib.rs` - Added mod mcp_common, mod mcp_tool, #[proc_macro_attribute] mcp_tool entry point

## Decisions Made

- **Manual Clone for State<T>:** Derive(Clone) on State<T> would require T: Clone, but Arc<T> is always Clone. Manual impl removes the unnecessary bound.
- **allow(dead_code) on full_ty:** ParamRole::State stores both inner_ty and full_ty. Only inner_ty is used by #[mcp_tool]; full_ty is reserved for #[mcp_server] in Plan 02/03.
- **Unconditional schema generation:** The plan specified schemas should be generated unconditionally since schema generation IS the macro's value proposition. No cfg(feature) guard.
- **Branching ToolInfo constructors:** ToolInfo has no set_annotations() method, so code branches: ToolInfo::with_annotations() when annotations are present, ToolInfo::new() when absent.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed State<T> Clone derive requiring T: Clone**
- **Found during:** Task 1 (State<T> unit tests)
- **Issue:** `#[derive(Clone)]` on `State<T>` generates a `T: Clone` bound, but `Arc<T>` is always `Clone` regardless of `T`. TestDb (not Clone) caused compilation failure.
- **Fix:** Replaced `#[derive(Debug, Clone)]` with `#[derive(Debug)]` + manual `Clone` impl that uses `Arc::clone(&self.0)`.
- **Files modified:** `src/server/state.rs`
- **Verification:** All 5 State<T> unit tests pass including clone test with non-Clone inner type.
- **Committed in:** `ebaac3b`

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential correctness fix. The manual Clone impl is the correct pattern for Arc wrappers.

## Issues Encountered

None beyond the Clone derive issue (documented above as deviation).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- State<T> and mcp_common are ready for #[mcp_server] impl-block macro (Plan 02)
- #[mcp_tool] is ready for integration testing (Plan 03)
- Both macro and runtime crates compile cleanly with no warnings

## Self-Check: PASSED

- All 3 created files verified present on disk
- Both task commits (ebaac3b, 0828447) verified in git log
- cargo build -p pmcp-macros: clean (0 warnings)
- cargo build -p pmcp --lib: clean
- cargo test -p pmcp-macros mcp_common: 13 passed
- cargo test -p pmcp --lib server::state: 5 passed

---
*Phase: 58-mcp-tool-proc-macro*
*Completed: 2026-03-21*
