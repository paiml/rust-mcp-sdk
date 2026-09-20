---
phase: 55-tasks-with-polling
plan: 03
subsystem: server
tags: [tasks, polling, builder, dispatch, capability-negotiation, task-store]

# Dependency graph
requires:
  - phase: 55-01
    provides: "TaskStatus utility methods (is_terminal, can_transition_to, Display)"
  - phase: 55-02
    provides: "TaskStore trait, InMemoryTaskStore, StoreConfig, TaskStoreError"
provides:
  - "Builder task_store() method with auto-capability negotiation"
  - "Core dispatch for tasks/get, tasks/list, tasks/cancel through TaskStore"
  - "Crate-root re-exports: TaskStore, InMemoryTaskStore, StoreConfig, TaskStoreError"
  - "TaskRouter backward compat as fallback path"
affects: [pmcp-tasks, examples, documentation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TaskStore-first dispatch with TaskRouter fallback for backward compat"
    - "Builder auto-sets ServerCapabilities.tasks (standard path, not experimental)"

key-files:
  created: []
  modified:
    - src/server/builder.rs
    - src/server/core.rs
    - src/server/adapters.rs
    - src/lib.rs

key-decisions:
  - "TaskStore checked before TaskRouter in dispatch for tasks/get, tasks/list, tasks/cancel"
  - "tasks/result remains TaskRouter-only (PMCP extension not in SDK TaskStore)"
  - "ServerCapabilities.tasks (standard field) used instead of experimental.tasks for TaskStore path"

patterns-established:
  - "TaskStore dispatch pattern: check task_store first, fall back to task_router, else error"

requirements-completed: [TASKS-POLLING, TASK-STORE, TASK-CAPABILITIES]

# Metrics
duration: 7min
completed: 2026-03-21
---

# Phase 55 Plan 03: Server Builder Integration Summary

**TaskStore wired into builder (auto-capability negotiation) and core dispatch (tasks/get, tasks/list, tasks/cancel) with TaskRouter backward compat fallback**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-21T00:24:58Z
- **Completed:** 2026-03-21T00:32:06Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Builder.task_store() registers Arc<dyn TaskStore> and auto-sets ServerCapabilities.tasks (list, cancel, requests.tools.call)
- Core dispatches tasks/get, tasks/list, tasks/cancel through TaskStore when available, falling back to TaskRouter for backward compat
- Re-exports from crate root: TaskStore, InMemoryTaskStore, StoreConfig, TaskStoreError
- All 759 tests pass, workspace compiles clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Add task_store field to builder and wire capability negotiation** - `2a6cc97` (feat)
2. **Task 2: Update core dispatch to use TaskStore and add re-exports** - `1c1f47c` (feat)

## Files Created/Modified
- `src/server/builder.rs` - Added task_store field, task_store() builder method, auto-capability negotiation, build() wiring, 2 new tests
- `src/server/core.rs` - Added task_store field to ServerCore, new() parameter, TaskStore-first dispatch for TasksGet/TasksList/TasksCancel
- `src/server/adapters.rs` - Updated test helper ServerCore::new() call with task_store parameter
- `src/lib.rs` - Added crate-root re-exports for TaskStore, InMemoryTaskStore, StoreConfig, TaskStoreError

## Decisions Made
- TaskStore is checked before TaskRouter in dispatch for tasks/get, tasks/list, tasks/cancel -- gives new standard path priority over legacy experimental path
- tasks/result stays TaskRouter-only since it is a PMCP extension not covered by the SDK TaskStore trait
- ServerCapabilities.tasks (standard MCP field) is used for the TaskStore path, while TaskRouter continues to use experimental.tasks -- both can coexist

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 55 complete: TaskStatus utilities (Plan 01), TaskStore trait + InMemoryTaskStore (Plan 02), server builder/dispatch integration (Plan 03)
- Ready for downstream usage: task-enabled servers can use `Server::builder().task_store(Arc::new(InMemoryTaskStore::new()))`
- pmcp-tasks crate backward compat preserved via TaskRouter fallback

---
*Phase: 55-tasks-with-polling*
*Completed: 2026-03-21*
