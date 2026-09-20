---
phase: 55-tasks-with-polling
plan: 01
subsystem: api
tags: [tasks, protocol-types, serde, state-machine]

requires:
  - phase: 54.1-protocol-type-construction-dx
    provides: "Uniform protocol type pattern (#[non_exhaustive] + Default + ::new() + .with_*())"
provides:
  - "TaskStatus utility methods (is_terminal, can_transition_to) on SDK types"
  - "Display impl for TaskStatus"
  - "Spec-correct TTL serialization (null when None, not omitted)"
affects: [55-02, 55-03, pmcp-tasks]

tech-stack:
  added: []
  patterns: ["SDK types as canonical source of truth for MCP Tasks"]

key-files:
  created: []
  modified: ["src/types/tasks.rs"]

key-decisions:
  - "TTL serialization: removed skip_serializing_if from both Task.ttl and TaskCreationParams.ttl for MCP spec compliance (number | null)"
  - "TaskStatus utility methods replicate pmcp-tasks behavior exactly for type parity"

patterns-established:
  - "SDK task types mirror pmcp-tasks utility methods for canonical source of truth"

requirements-completed: [TASKS-POLLING, TASK-CAPABILITIES]

duration: 3min
completed: 2026-03-21
---

# Phase 55 Plan 01: SDK Task Type Reconciliation Summary

**TaskStatus gains is_terminal(), can_transition_to(), Display impl; Task.ttl serializes as null (not omitted) matching MCP spec**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T00:14:31Z
- **Completed:** 2026-03-21T00:17:52Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `is_terminal()` method to SDK `TaskStatus` matching pmcp-tasks behavior
- Added `can_transition_to()` method to SDK `TaskStatus` with full state machine validation
- Added `Display` impl for `TaskStatus` producing snake_case output matching serde
- Fixed `Task.ttl` and `TaskCreationParams.ttl` serialization to emit `null` instead of omitting field when `None`, matching MCP spec's `number | null` type
- Added 7 new tests (12 total), all passing

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Failing tests** - `b8305b0` (test)
2. **Task 1 (GREEN): Implementation** - `a48d482` (feat)

_TDD task: test-first, then implementation._

## Files Created/Modified
- `src/types/tasks.rs` - Added TaskStatus utility methods (is_terminal, can_transition_to), Display impl, fixed TTL serialization, added 7 new tests

## Decisions Made
- Removed `skip_serializing_if` from `Task.ttl` so `None` serializes as `"ttl": null` per MCP spec's `number | null` type
- Removed `skip_serializing_if` from `TaskCreationParams.ttl` for consistency
- Kept `skip_serializing_if` on `poll_interval` and `status_message` (those are truly optional per spec)
- Utility methods match pmcp-tasks implementation exactly for type parity per D-01

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SDK task types are now canonical with utility methods and spec-correct serialization
- Ready for 55-02 (TaskStore trait / handler integration) and 55-03 (capability negotiation)
- pmcp-tasks can reference SDK types for type parity

---
*Phase: 55-tasks-with-polling*
*Completed: 2026-03-21*
