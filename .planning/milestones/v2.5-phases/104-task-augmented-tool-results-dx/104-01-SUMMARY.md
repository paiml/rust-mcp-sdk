---
phase: 104-task-augmented-tool-results-dx
plan: 01
subsystem: api
tags: [tasks, sep-1686, wasm, web-time, polling, call-tool-result, client]

# Dependency graph
requires:
  - phase: 101-tasks-as-tasks-dx / 102-http-task-dispatch
    provides: "tasks/* client methods (tasks_get/tasks_result), TaskStore server path, RELATED_TASK_META_KEY"
provides:
  - "TaskMetadata type (taskId + optional pollInterval ms / maxPollDurationSecs s) with new/builders"
  - "CallToolResult::with_related_task server-emit builder + related_task client accessor (twins keyed by RELATED_TASK_META_KEY)"
  - "Client::wait_for_task wasm-safe polling convenience (web_time::Instant timeout, clamped interval)"
  - "Client::wait_for_related_task + WaitForTaskOptions (from_metadata/From<TaskMetadata>/or_from_metadata) — zero-glue compose from related_task()"
affects: [104-02, task-augmented-tool-results, http-task-dispatch, wasm-client]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "web_time::Instant for wasm-safe elapsed-time (not std::time::Instant, which panics on wasm)"
    - "crate::runtime::sleep for platform-independent poll delays"
    - "non_exhaustive public struct + new()/with_*() builders for forward compatibility"
    - "fallible _meta accessor (from_value().ok()) for tamper tolerance (never panics)"

key-files:
  created:
    - "tests/task_augmented_result.rs"
  modified:
    - "src/types/tasks.rs"
    - "src/types/tools.rs"
    - "src/client/mod.rs"
    - "src/lib.rs"

key-decisions:
  - "TaskMetadata is #[non_exhaustive] (per plan) → added new()+builders so external test crates/doctests can construct it (Rule 3 blocking fix)"
  - "Interval floor of 50ms clamps zero/absent pollInterval to prevent hot-spin (T-104-01-02)"
  - "Timeout maps to Error::Timeout(max_secs*1000ms) via existing constructor"
  - "Re-exported WaitForTaskOptions at crate root alongside ToolCallResponse for ergonomics"

patterns-established:
  - "Server-emit builder + client accessor as twins keyed by a shared _meta const"
  - "Options struct composes directly from a metadata type (from_metadata/From/or_from_metadata) — no hand-copy"

requirements-completed: [TOUT-03, TOUT-01]

# Metrics
duration: ~25min
completed: 2026-07-04
---

# Phase 104 Plan 01: Task-Augmented Tool-Result Client DX Summary

**SEP-1686 client surface: typed `TaskMetadata`, `CallToolResult::with_related_task`/`related_task` twins, and a wasm-safe `Client::wait_for_task` poller that composes directly from `TaskMetadata` (clamped, timed out via `web_time::Instant`).**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-04
- **Tasks:** 3
- **Files modified:** 4 (+1 test file created)

## Accomplishments
- `TaskMetadata` type: `taskId` + optional `pollInterval` (ms) / `maxPollDurationSecs` (s), `non_exhaustive`, camelCase, unit-documented, minimal-`{taskId}`-shape tolerant. Additive — `RelatedTaskMetadata` untouched.
- `CallToolResult::with_related_task` (server builder) and `related_task` (client accessor) twins keyed by `RELATED_TASK_META_KEY`; accessor is fallible (None on absent/malformed, never panics).
- `Client::wait_for_task` polls `tasks/get` to terminal then returns `tasks/result`; wasm-safe elapsed clock (`web_time::Instant`), `crate::runtime::sleep` delays, interval clamped to a 50 ms floor, `max_poll_duration_secs` timeout returns `Error::Timeout`.
- `WaitForTaskOptions` with `from_metadata` / `From<TaskMetadata>` / `or_from_metadata`, and `wait_for_related_task` convenience — a `related_task()` handle drives the poller with zero glue.
- New integration test file with 9 tests (accessor round-trip/minimal/malformed, option composition, live lifecycle, related-task compose, timeout + clamp bounded-poll-count).

## Task Commits

Each task committed atomically:

1. **Task 1: TaskMetadata type** - `b6b44a90` (feat)
2. **Task 2: CallToolResult::with_related_task + related_task** - `55566ab1` (feat)
3. **Task 3: Client::wait_for_task (wasm-safe, composes with TaskMetadata)** - `401ddd4f` (feat)

_Note: pre-commit build/doctest gates preclude a classic failing-test RED commit (a test referencing a not-yet-existing type won't compile); TDD was done test+impl together and committed as `feat`._

## Files Created/Modified
- `src/types/tasks.rs` - Added `TaskMetadata` struct + `new`/`with_poll_interval`/`with_max_poll_duration_secs` builders + 2 unit tests.
- `src/types/tools.rs` - Added `CallToolResult::with_related_task` / `related_task` methods (keyed by `RELATED_TASK_META_KEY`, `#[allow(clippy::used_underscore_binding)]`).
- `src/client/mod.rs` - Added `WaitForTaskOptions` (+ `from_metadata`/`From`/`or_from_metadata`), `Client::wait_for_task`, `Client::wait_for_related_task`; imported `TaskMetadata`.
- `src/lib.rs` - Re-exported `WaitForTaskOptions` at crate root.
- `tests/task_augmented_result.rs` - New: accessor twins + option composition + live duplex lifecycle/timeout/clamp tests.

## Decisions Made
- **TaskMetadata constructor:** The plan requires `#[non_exhaustive]` on `TaskMetadata`, which forbids struct-literal construction from external crates (the integration tests and doctests). Added `new()` + builders (mirroring `Task`) so both compile — a Rule 3 blocking fix. All doctests/tests use the builders.
- **Interval floor 50 ms:** clamps a zero/absent `pollInterval` so the loop cannot busy-spin (threat T-104-01-02); the timeout test asserts a bounded poll count (< 200 over a 1 s budget) to prove it.
- **Timeout representation:** reused `Error::timeout(ms)` → `Error::Timeout` (existing variant), with `max_secs * 1000`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `TaskMetadata::new` + builder methods**
- **Found during:** Task 2 (external test/doctest construction)
- **Issue:** `#[non_exhaustive]` (required by the plan's acceptance criteria) blocks struct-literal construction from the `tests/` integration crate and from doctests, so `TaskMetadata { .. }` failed to compile (E0639).
- **Fix:** Added `new(task_id)`, `with_poll_interval(ms)`, `with_max_poll_duration_secs(secs)` builders (same convention as `Task`); updated doctests and tests to use them.
- **Files modified:** src/types/tasks.rs, src/types/tools.rs (doctest), tests/task_augmented_result.rs
- **Verification:** `cargo test --features full --lib types::tasks`, doctests, and the integration suite all green.
- **Committed in:** `55566ab1` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The builder addition was required for the plan's own tests to compile under the mandated `#[non_exhaustive]`. No scope creep; the public surface is a superset of what the plan specified.

## Issues Encountered
None beyond the deviation above.

## Threat Flags
None — no new network endpoints, auth paths, or trust boundaries introduced. `wait_for_task` composes existing owner-scoped `tasks_get`/`tasks_result` (T-104-01-01), enforces a timeout + interval floor (T-104-01-02), and reads `_meta` fallibly (T-104-01-03), all as specified in the threat register.

## Verification Results
- `cargo test --features full --test task_augmented_result` → 9 passed, 0 failed.
- `cargo test --features full --lib types::tasks` → 14 passed.
- `cargo check --target wasm32-unknown-unknown --lib` → success (no errors).
- `cargo clippy --features full --lib --tests -- -D warnings` → exit 0.
- `cargo fmt --all -- --check` → clean.
- `git diff 930197be..HEAD -- Cargo.toml` → empty (zero new dependencies; `web-time` already in-tree).

## Next Phase Readiness
- Client-side SEP-1686 surface (TOUT-03) + server-emit twin builder (D-03.1 of TOUT-01) shipped and wasm-compatible.
- Ready for downstream plans (e.g. 104-02) to wire the server side / examples / docs.

## Self-Check: PASSED

All created/modified files present; all 4 commits (`b6b44a90`, `55566ab1`, `401ddd4f`, `f7cd5acd`) exist; key symbols verified (`TaskMetadata`, `related_task`, `wait_for_task`, `web_time::Instant`, `from_metadata`, interval clamp floor).

---
*Phase: 104-task-augmented-tool-results-dx*
*Completed: 2026-07-04*
