---
phase: 104-task-augmented-tool-results-dx
plan: 04
subsystem: api
tags: [tool-dispatch, tool-output, call-tool-result, result-meta, tasks, pmcp-server, ergonomics]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-01 CallToolResult::with_related_task/related_task + TaskMetadata"
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-02 ToolOutput enum + ToolHandler::handle_output + resolve_tool_output/DispatchOutput"
  - phase: 104-task-augmented-tool-results-dx
    provides: "104-03 double_wrap_tripwire + suppress_double_wrap"
provides:
  - "ServerBuilder::tool_with_result (schema-generation gated) — closure returning a full CallToolResult, emitted verbatim via ToolOutput::Result"
  - "TypedToolWithResult wrapper tool (handle_output override → ToolOutput::Result; handle fallback serializes the envelope)"
  - "RequestHandlerExtra::set_result_meta — one-call Payload-path _meta retrofit through an encapsulated Arc<std::sync::Mutex> slot with defined merge precedence"
  - "ResultMetaHandle::take_result_meta + merge_result_meta — dispatcher drain + handler-key-wins merge (both native dispatchers)"
affects: [104-05-migration-guide]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Interior-mutable Arc<std::sync::Mutex<Option<Map>>> slot cloned before a by-value move, drained after return (survives Clone like the peer field — Pitfall 4)"
    - "Verbatim-envelope sugar wrapper mirrors tool_typed_with_output but overrides handle_output to return ToolOutput::Result"
    - "Slot access encapsulated behind set_result_meta / take_result_meta / merge_result_meta so dispatch code never touches the lock"

key-files:
  created:
    - tests/tool_with_result.rs
  modified:
    - src/server/typed_tool.rs
    - src/server/mod.rs
    - src/server/cancellation.rs
    - src/server/core.rs

key-decisions:
  - "std::sync::Mutex (NOT tokio) for the result-meta slot: locked, merged, and guard dropped synchronously — never held across an await (Codex MEDIUM); poisoning recovered via PoisonError::into_inner"
  - "set_result_meta MERGES with handler-key-wins precedence (never whole-map replace); repeated calls accumulate; unrelated widget/native keys preserved"
  - "set_result_meta affects the Payload path ONLY and is IGNORED on the ToolOutput::Result path (handler owns its full envelope) — documented + tested"
  - "tool_with_result carries the SAME #[cfg(feature = \"schema-generation\")] gate as its sibling tool_typed_with_output, with the loud response-middleware-bypass rustdoc (D-04a #1)"

patterns-established:
  - "Pattern: a moved-by-value RequestHandlerExtra can still write back to the dispatcher via a shared interior-mutable Arc slot cloned before the move and drained after"
  - "Pattern: closure-authored full-envelope tools need no hand-written ToolHandler — a wrapper's handle_output override is the real path, handle() is a serialize fallback"

requirements-completed: [TOUT-01]

# Metrics
duration: ~35min
completed: 2026-07-04
---

# Phase 104 Plan 04: tool_with_result + set_result_meta ergonomics Summary

**Closure authors can now register a full-`CallToolResult` tool in one call (`ServerBuilder::tool_with_result`, emitted verbatim), and existing hand-written Payload-path handlers can retrofit task-augmented `_meta` with one call (`RequestHandlerExtra::set_result_meta`) that round-trips through an encapsulated `std::sync::Mutex` slot with defined handler-key-wins merge precedence — no manual `handle_output` impl required.**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-04
- **Tasks:** 2
- **Files modified:** 4 (+1 created)

## Accomplishments
- Added `TypedToolWithResult<TIn, F>` in `typed_tool.rs`: its `handle_output` override returns `ToolOutput::Result(...)` so the closure's `CallToolResult` reaches the wire VERBATIM; `handle()` is a serialize fallback for non-dispatch callers (workflow-internal tool steps). A private `run()` helper shares the deserialize-and-invoke logic between the two.
- Added `ServerBuilder::tool_with_result<TIn>` mirroring `tool_typed_with_output` (same `#[cfg(feature = "schema-generation")]` gate + `TIn: DeserializeOwned + JsonSchema` bound), carrying the loud response-middleware-bypass rustdoc (D-04a #1) — closure authors attach `with_related_task(...)`/custom `_meta`/structured content in one call.
- Added the interior-mutable result-meta slot to `RequestHandlerExtra`: `result_meta: Arc<std::sync::Mutex<Option<Map>>>`, initialized in `new()` and `default()`, shared across the by-value Clone/move exactly like the `peer` field (Pitfall 4 / Open Q1).
- Encapsulated all slot access: `set_result_meta(&self, map)` merges (handler-key-wins, accumulating across repeated calls); `ResultMetaHandle::take_result_meta()` drains the cheap `Arc` clone the dispatcher retained before the move; `merge_result_meta(&mut CallToolResult, map)` applies handler-key-wins precedence onto the built envelope. Dispatch code never touches `.lock()`.
- Wired BOTH native dispatchers (`Server::handle_call_tool` in `mod.rs`, `ServerCore` tool dispatch in `core.rs`): clone the slot handle before the `handle_output` move, drain + merge onto the Payload-built `CallToolResult` ONLY (the verbatim `ToolOutput::Result` arm, create-path, and error arms all return earlier and are untouched).

## Task Commits

Each task was committed atomically:

1. **Task 1: ServerBuilder::tool_with_result + verbatim wrapper tool** - `fdfb7dcc` (feat)
2. **Task 2: RequestHandlerExtra::set_result_meta with encapsulated slot** - `02083541` (feat)

_Note (matches Plan 02): Rust's pre-commit build/format quality gate forbids committing a compile-failing RED test, so each TDD task shipped its implementation and tests together in one commit rather than a separate RED commit. RED/GREEN was still exercised locally (tests written and observed to drive the code)._

## Files Created/Modified
- `src/server/typed_tool.rs` - Added `TypedToolWithResult<TIn, F>` (struct + Debug + inherent `new`/`new_with_schema`/`with_description`/`run` + `ToolHandler` impl whose `handle_output` returns `ToolOutput::Result`); imported `CallToolResult` and `ToolOutput`.
- `src/server/mod.rs` - Added `ServerBuilder::tool_with_result` under `#[cfg(feature = "schema-generation")]` with the bypass rustdoc; cloned the result-meta handle before the `handle_output` move and merged drained handler `_meta` onto the Payload-built `CallToolResult` before serialization.
- `src/server/cancellation.rs` - Added the `result_meta` slot + `set_result_meta`/`result_meta_handle` on `RequestHandlerExtra`, the `ResultMetaHandle` drain type (`take_result_meta`), and the `merge_result_meta` free fn; slot initialized in `new()`/`default()`.
- `src/server/core.rs` - Cloned the result-meta handle before the `handle_output` move; drained + merged onto the Payload envelope via a non-wasm shadow-rebind (so the wasm branch needs no `mut`).
- `tests/tool_with_result.rs` - New gate (7 tests): verbatim `_meta` wire result + typed-input deserialization for `tool_with_result`; `set_result_meta` merge round-trip, widget collision (handler-key overwrites same-name widget key, unrelated widget key preserved), repeated-call accumulation (later colliding key wins), no-op-when-never-called, and ignored-on-`ToolOutput::Result`.

## Decisions Made
- Used `std::sync::Mutex` (not tokio): the slot is locked, mutated, and its guard dropped synchronously inside `set_result_meta`/`take_result_meta` — never held across an await (T-104-04-03). Lock poisoning is recovered with `PoisonError::into_inner` rather than panicking (handler-owned data, same trust level as the returned value).
- Merge, never whole-map replace (T-104-04-04): a handler-set key overwrites the same-name key (whether from an earlier `set_result_meta` call or from widget/native enrichment); all unrelated keys survive. The widget-collision test proves an unrelated `openai/toolInvocation/*` key is preserved while the colliding one is overwritten.
- Kept the wasm `RequestHandlerExtra` unit stub untouched: the slot, its methods, `ResultMetaHandle`, and `merge_result_meta` are all `#[cfg(not(target_arch = "wasm32"))]`, and the wasm dispatch tail (which uses `handle()`, not `handle_output`) never references them. `core.rs` uses a non-wasm shadow-rebind to add the merge without introducing an `unused_mut` on wasm.

## Deviations from Plan

None - plan executed exactly as written. (The TDD RED/GREEN commit cadence was adapted to Rust's build-must-pass quality gate as noted above; no scope or behavior deviation.)

An out-of-scope reformat of `tests/double_wrap_tripwire.rs` (a Wave-1 file) produced by `cargo fmt --all` was reverted twice so it stayed outside this plan's commits.

## Verification
- `cargo test --features full --test tool_with_result` — 7/7 pass (tool_with_result verbatim + typed-input; set_result_meta merge/collision/repeated/no-op/ignored-on-Result).
- `cargo test --features full --test tool_output_passthrough --test double_wrap_tripwire --test tool_as_task_lifecycle` — 12 + 7 + 8 pass (no Plan 02/03 or Phase 101/102 regression).
- `cargo check --target wasm32-unknown-unknown --lib` — clean (wasm stub compiles; no new warnings from this plan's code).
- `cargo clippy --features full --lib --test tool_with_result` — no warnings.
- `cargo test --features full --doc server::ServerBuilder` — 27 pass / 2 ignored (the new `tool_with_result` `no_run` doctest compiles); `--lib cancellation::` — 9 pass.
- `pmat analyze complexity --max-cognitive 25` — 0 violations across the touched files (cancellation/typed_tool/mod.rs/core.rs).

## Acceptance Criteria
- `src/server/mod.rs` contains `fn tool_with_result` under `#[cfg(feature = "schema-generation")]` with a `TIn: ... + JsonSchema` bound and the loud bypass rustdoc. ✓
- `src/server/typed_tool.rs` contains a wrapper whose `handle_output` returns `ToolOutput::Result`. ✓
- `src/server/cancellation.rs` contains `fn set_result_meta` + `fn take_result_meta` + a `result_meta: Arc<std::sync::Mutex<Option<..>>>` slot (std::sync::Mutex, not tokio). ✓
- Dispatch code (mod.rs/core.rs) clones the slot handle before the `handle_output` move and drains via `take_result_meta` after on the Payload path only — no direct `.lock()` outside cancellation.rs. ✓
- Method rustdoc states merge precedence, Payload-path-only, and ignored-on-`ToolOutput::Result`. ✓
- wire test asserts top-level `_meta[related-task].taskId == "t1"` and un-stringified content; set_result_meta merge/collision/repeated/no-op green; wasm `--lib` check green. ✓

## Next Phase Readiness
- All three D-03 sugar layers now ship (`with_related_task` in Plan 01; `tool_with_result` + `set_result_meta` here). Plan 05's migration guide can point existing hand-written handlers at `set_result_meta` (one-call retrofit) and closure authors at `tool_with_result`, and should link the loud bypass rustdoc on both `ToolOutput::Result` and `tool_with_result`.

## Self-Check: PASSED

- Created file exists: `tests/tool_with_result.rs`; modified files present: `typed_tool.rs`, `mod.rs`, `cancellation.rs`, `core.rs`.
- Task commits present: `fdfb7dcc` (T1), `02083541` (T2).
- Acceptance greps confirmed: `fn tool_with_result` ×1 in mod.rs; `fn set_result_meta`/`fn take_result_meta` ×2 in cancellation.rs; `result_meta: Arc<std::sync::Mutex<Option<` ×1 in cancellation.rs.
- No unexpected file deletions in the plan's commit range.

---
*Phase: 104-task-augmented-tool-results-dx*
*Completed: 2026-07-04*
