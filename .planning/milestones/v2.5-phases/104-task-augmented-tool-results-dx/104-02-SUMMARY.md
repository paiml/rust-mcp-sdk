---
phase: 104-task-augmented-tool-results-dx
plan: 02
subsystem: api
tags: [tool-dispatch, tool-output, call-tool-result, middleware, tasks, pmcp-server]

# Dependency graph
requires:
  - phase: 101-tasks-as-tasks-dx
    provides: ToolCallOutcome + ServerCore tools/call dispatch
  - phase: 102-http-task-dispatch
    provides: task_dispatch::TaskDispatch shared unit + maybe_build_task_created create-path gate
provides:
  - "ToolOutput enum (Payload/Result) + ToolHandler::handle_output default method (native)"
  - "task_dispatch::resolve_tool_output + DispatchOutput — single shared Payload-vs-Result + response-middleware-bypass decision (D-05)"
  - "Both native dispatchers (Server + ServerCore) route handler output through handle_output; ToolOutput::Result lands on the wire VERBATIM"
  - "pmcp::ToolOutput crate-root re-export"
affects: [104-03-tripwire, 104-04-tool_with_result-ergonomics, 104-05-migration-guide]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Additive trait default method (handle_output) mirrors metadata() — existing impls untouched"
    - "Single shared dispatch-decision helper honored identically by both dispatchers (anti-drift)"

key-files:
  created:
    - tests/tool_output_passthrough.rs
  modified:
    - src/server/mod.rs
    - src/server/core.rs
    - src/server/task_dispatch.rs
    - src/lib.rs

key-decisions:
  - "ToolOutput::Result bypasses RESPONSE middleware only (D-04 + D-04a, USER-APPROVED + LOCKED); request middleware, handler-error path, token cleanup, and the Payload tail are unchanged"
  - "No implicit from_value::<CallToolResult> sniffing on the Payload path (D-02 rejected)"
  - "Workflow-internal tool-step path (execute_tool_with_middleware / prompt_handler fallback / builder_middleware_executor) kept on handle()/Value — out of scope"

patterns-established:
  - "Pattern 1: opt-in full-envelope tool output via a #[non_exhaustive] control enum with a loud bypass rustdoc on the trust-shifting variant"
  - "Pattern 2: one resolve_tool_output helper is the sole owner of the Payload-vs-Result + bypass rule; both dispatchers branch on DispatchOutput"

requirements-completed: [TOUT-01]

# Metrics
duration: ~40min
completed: 2026-07-04
---

# Phase 104 Plan 02: ToolOutput verbatim pass-through Summary

**A ToolHandler can now return `ToolOutput::Result(CallToolResult)` and land that envelope (its `_meta` included) on the wire verbatim through both `Server` and `ServerCore`, bypassing RESPONSE middleware by user-approved locked design while request middleware, handler-error routing, and the create-path gate stay unchanged.**

## Performance

- **Duration:** ~40 min
- **Completed:** 2026-07-04
- **Tasks:** 3
- **Files modified:** 4 (+1 created)

## Accomplishments
- Added the public `ToolOutput` enum (`Payload`/`Result`, `#[non_exhaustive]`) and a default `ToolHandler::handle_output` that delegates to `handle()` → `Payload`, so every existing `TypedTool`/`TypedToolWithOutput`/hand-written handler behaves exactly as before.
- Closed the `src/server/mod.rs` double-wrap junction: `ToolOutput::Result` reaches the wire verbatim (no text-wrap, no widget enrichment, no create-path routing) while `ToolOutput::Payload` keeps today's full tail.
- Encoded the Payload-vs-Result decision + the RESPONSE-middleware-bypass rule ONCE in `task_dispatch::resolve_tool_output`/`DispatchOutput`; both native dispatchers branch on it identically (D-05), asserted by a Server-vs-ServerCore parity test.
- Landed the full D-04a regression battery on BOTH dispatchers: response-middleware fires for Payload but is bypassed for a successful Result; request middleware still fires for the Result tool; a Result handler returning `Err` still routes through `handle_tool_error`.

## Task Commits

Each task was committed atomically:

1. **Task 1: ToolOutput enum + handle_output default (loud bypass rustdoc)** - `511b0f86` (feat)
2. **Task 2: shared pass-through branch in both native dispatchers** - `46732064` (feat)
3. **Task 3: D-04a middleware/error regression battery** - `f68d58d9` (test)

_Note: Rust's pre-commit build/format quality gate forbids committing a compile-failing RED test, so each TDD task shipped its implementation and tests together in one commit rather than a separate RED commit. RED/GREEN was still exercised locally (tests written and observed to drive the code)._

## Files Created/Modified
- `src/server/mod.rs` - Added `ToolOutput` enum + `ToolHandler::handle_output` default; restructured the native `handle_call_tool` tail to resolve a `DispatchOutput` (RESPONSE middleware runs only for the Payload/error arm) and emit `ToolOutput::Result` verbatim after unconditional token cleanup; added a default-delegation unit test (`tool_output_tests`).
- `src/server/task_dispatch.rs` - Added `DispatchOutput` + `resolve_tool_output` (the single shared decision helper) with the bypass-rule documentation.
- `src/server/core.rs` - Swapped the native `ServerCore` call site to `handle_output`; verbatim `ToolOutput::Result` early-returns `ToolCallOutcome::Result` (bypassing response middleware + create-path + wrap); wasm branch left on `handle()`/Value.
- `src/lib.rs` - Re-exported `pmcp::ToolOutput` next to `ToolHandler`.
- `tests/tool_output_passthrough.rs` - New integration gate: verbatim `_meta` survival, Payload text-wrap regression, task-shaped-Payload create-path precedence, Server-vs-ServerCore parity, and the D-04a battery (response-bypass + request-still-runs + error-path) on both dispatchers.

## Decisions Made
- Followed the plan's locked D-04/D-04a design exactly: only RESPONSE middleware is bypassed on the successful `ToolOutput::Result` arm; a `// Why:` comment at each branch documents this is deliberate.
- For core.rs the verbatim path early-returns `ToolCallOutcome::Result(_)` (the wasm dispatch tail stays on `Result<Value>`), whereas mod.rs resolves the enum out of the middleware block so the unconditional token cleanup still runs before the verbatim return — both call the same shared `resolve_tool_output`.

## Deviations from Plan

None - plan executed exactly as written. (The TDD RED/GREEN commit cadence was adapted to Rust's build-must-pass quality gate as noted above; no scope or behavior deviation.)

## Issues Encountered
- `Server::handle_request` is a private method (only `ServerCore` implements `ProtocolHandler`), so the high-level `Server` half of the parity/middleware tests is driven via `Server::run` + a real `pmcp::Client` over an in-process duplex transport (the plan's sanctioned alternative), while `ServerCore` is driven via a `ProtocolHandler` pump.
- Two clippy lints on the new test (`map_or` → `is_none_or`, complex tuple return type → `type Recorder`) were fixed before the Task 3 commit.

## Verification
- `cargo build --features full` — clean.
- `cargo test --features full --test tool_output_passthrough` — 8/8 pass (verbatim + parity + Payload regression + create-path precedence + D-04a battery).
- `cargo test --features full --test tool_as_task_lifecycle` — 7/7 pass (no Phase 101/102 regression).
- `cargo test --features full --lib server::` — 671 pass; `--lib task_dispatch` — 16 pass.
- `cargo clippy --features full --lib` and `--test tool_output_passthrough` — no warnings.
- `pmat analyze complexity --max-cognitive 25` — no new violation in task_dispatch/mod/core (shared helper is trivial).

## Next Phase Readiness
- `ToolOutput` + `handle_output` + the shared `resolve_tool_output` decision point are in place for Plan 03 (tripwire) and Plan 04 (`tool_with_result` ergonomics) to build on.
- The loud bypass rustdoc lives on the `ToolOutput::Result` variant; Plan 05's migration guide should link to it.

## Self-Check: PASSED

- Created files exist: `tests/tool_output_passthrough.rs`, `104-02-SUMMARY.md`.
- Task commits present: `511b0f86` (T1), `46732064` (T2), `f68d58d9` (T3).
- No unexpected file deletions in the plan's commit range; working tree clean.

---
*Phase: 104-task-augmented-tool-results-dx*
*Completed: 2026-07-04*
