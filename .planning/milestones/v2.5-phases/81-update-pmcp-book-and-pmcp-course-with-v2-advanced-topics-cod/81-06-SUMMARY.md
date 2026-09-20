---
phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod
plan: 06
subsystem: docs/course
tags: [docs, course, tasks, refresh, drift-correction]
requires: [pmcp-course/src/part8-advanced/ch21-tasks.md, pmcp-course/src/part8-advanced/ch21-01-lifecycle.md, pmcp-course/src/part8-advanced/ch21-02-capability-negotiation.md, pmcp-course/src/part8-advanced/ch21-exercises.md, crates/pmcp-tasks/src/lib.rs, src/server/task_store.rs]
provides:
  - Tasks course pages refreshed against current pmcp-tasks crate API surface
  - Tasks course quiz (ch21-tasks.toml) created with 5 minimal questions
affects: [pmcp-course]
tech-stack:
  added: []
  patterns: [drift-correction, structure-preserving-verify, R-5-behavioral-prose-audit]
key-files:
  created:
    - pmcp-course/src/quizzes/ch21-tasks.toml
  modified:
    - pmcp-course/src/part8-advanced/ch21-tasks.md
decisions:
  - "ch21-tasks.md: Replace fictional DynamoDbTaskStore/RedisTaskStore type names with real GenericTaskStore<DynamoDbBackend>/GenericTaskStore<RedisBackend> composition (and a matching constructor example) matching crates/pmcp-tasks public API."
  - "ch21-01-lifecycle.md, ch21-02-capability-negotiation.md, ch21-exercises.md: NO drift detected. Per discipline rules these tasks are no-ops; structure-preserving verify tolerates no-op."
  - "ch21-tasks.toml: Branch B (file did not exist pre-edit). Created with 5 questions (within the 4-6 minimal range)."
metrics:
  duration_min: 7
  completed_date: 2026-05-15
---

# Phase 81 Plan 06: Tasks Course Refresh Summary

Drift-only refresh of the four `pmcp-course/src/part8-advanced/ch21-*.md` Tasks chapter pages against the current `pmcp-tasks` and `pmcp` crate API surface, plus creation of a minimal `ch21-tasks.toml` quiz. The audit found a single type-name drift in `ch21-tasks.md`; the other three pages had zero drift in either the type-name/signature category or the R-5 behavioral-prose category.

## Per-File Drift Audit Results (Task 1)

The audit used two complementary methods:

1. **Type-name + method-signature audit** against `crates/pmcp-tasks/src/lib.rs`, `src/server/task_store.rs`, `src/types/tasks.rs`, `src/types/tools.rs`, `src/server/cancellation.rs`.
2. **R-5 12-term behavioral-prose audit** (search list: SSE, serverless, owner binding, experimental.tasks, TaskSupport::Required, TaskSupport::Optional, tasks/result, tasks/cancel, tasks/get, poll interval, pollInterval, CreateTaskResult).

### ch21-tasks.md — 1 drift item

**Drift item D1 (TYPE-NAME):** Lines 93-94 and the production-store table at lines 99-103 referenced `DynamoDbTaskStore` and `RedisTaskStore` as if they were exported types in `pmcp-tasks`. The actual exported types are `DynamoDbBackend` and `RedisBackend`, wrapped via `GenericTaskStore::new(backend)`.

| Before (drift) | After (fixed) |
|----------------|---------------|
| `// let store = Arc::new(DynamoDbTaskStore::new(table_name, client).await);` | `// let backend = DynamoDbBackend::new(client, table_name);`<br/>`// let store = Arc::new(GenericTaskStore::new(backend));` |
| `\| `DynamoDbTaskStore` \| `pmcp-tasks` \| ... \|` | `\| `GenericTaskStore<DynamoDbBackend>` \| `pmcp-tasks` \| ... \|` |
| `\| `RedisTaskStore` \| `pmcp-tasks` \| ... \|` | `\| `GenericTaskStore<RedisBackend>` \| `pmcp-tasks` \| ... \|` |

Verified against `crates/pmcp-tasks/src/lib.rs:55-60` (`pub use store::dynamodb::DynamoDbBackend; pub use store::generic::GenericTaskStore; pub use store::redis::RedisBackend;`) and `crates/pmcp-tasks/src/store/generic.rs:72` (`pub fn new(backend: B) -> Self`).

### ch21-01-lifecycle.md — 0 drift items

Cross-checked every API reference against live source:
- `pmcp::server::task_store::{InMemoryTaskStore, TaskStore}` — exists per `src/server/task_store.rs:170,242`.
- `pmcp::types::tasks::{Task, TaskStatus}` — exists per `src/types/tasks.rs:14,94`.
- `pmcp::types::{ToolExecution, TaskSupport}` (via `pmcp::types::tools::{ToolExecution, TaskSupport}` re-export through `types::protocol::*`) — exists per `src/types/tools.rs:141,163` and `src/types/protocol/mod.rs:24`.
- `Server::builder().task_store(store)` — exists per `src/server/builder.rs:786`.
- `extra.is_task_request()` — exists per `src/server/cancellation.rs:350`.
- `extra.auth_context()` returning `AuthContext` with `.subject` field — exists per `src/server/cancellation.rs:355`, `src/server/auth/traits.rs:62-64`.
- `store.create(&owner, Some(300_000))` — matches pmcp core's `TaskStore::create(owner_id, ttl)` 2-arg signature per `src/server/task_store.rs:174`. (Note: the `pmcp-tasks` crate has a 3-arg `create(owner_id, request_method, ttl)` for its `GenericTaskStore`, but the course's code consistently uses the core SDK's `pmcp::server::task_store::InMemoryTaskStore` 2-arg path, which is internally consistent.)
- `Error::internal`, `Error::not_found`, `Error::validation` — exist per `src/error/mod.rs:189,215,239`.
- State machine diagram at L213-234 — matches `TaskStatus::can_transition_to` per `src/types/tasks.rs:56-72`.
- R-5 behavioral terms (TaskSupport::Required×2, TaskSupport::Optional×6, tasks/cancel×1, pollInterval×1, CreateTaskResult×3) — every occurrence's surrounding prose still describes current `pmcp-tasks` / `pmcp` behavior accurately.

### ch21-02-capability-negotiation.md — 0 drift items

Per-request capability signaling via `req.task` matches the implementation: server core extracts `req.task` from the parsed `CallToolRequest` and passes it through `RequestHandlerExtra::with_task_request(req.task)` (`src/server/cancellation.rs:284`). The handler accesses it via `extra.is_task_request()`. The Three Client Profiles section accurately describes how the dual-path handler + `get_task_result` fallback covers task-native, tool-polling, and sync-only clients. R-5 behavioral terms (serverless×1, TaskSupport::Required×2, tasks/result×2, tasks/get×3, CreateTaskResult×8) all still match current behavior.

### ch21-exercises.md — 0 drift items

All exercise skeletons use current API names (`TypedTool::new`, `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional))`, `extra.is_task_request()`, `extra.with_task_request(Some(...))`). The Exercise 4 design-thinking table is unaffected by API drift. R-5 behavioral terms (TaskSupport::Optional×4, CreateTaskResult×2) accurate.

## Pre-Edit Baselines (from /tmp/81-06-baselines.txt, quoted here for durable audit trail)

```
FILE=pmcp-course/src/part8-advanced/ch21-tasks.md
LINES=127
H1=# MCP Tasks: Long-Running Operations
H2_COUNT=6
R5_TERM_COUNT[SSE]=4
R5_TERM_COUNT[serverless]=6
R5_TERM_COUNT[owner binding]=0
R5_TERM_COUNT[experimental.tasks]=0
R5_TERM_COUNT[TaskSupport::Required]=0
R5_TERM_COUNT[TaskSupport::Optional]=1
R5_TERM_COUNT[tasks/result]=1
R5_TERM_COUNT[tasks/cancel]=0
R5_TERM_COUNT[tasks/get]=3
R5_TERM_COUNT[poll interval]=0
R5_TERM_COUNT[pollInterval]=0
R5_TERM_COUNT[CreateTaskResult]=1

FILE=pmcp-course/src/part8-advanced/ch21-01-lifecycle.md
LINES=372
H1=# Task Lifecycle and Polling
H2_COUNT=8
R5_TERM_COUNT[SSE]=0
R5_TERM_COUNT[serverless]=0
R5_TERM_COUNT[owner binding]=0
R5_TERM_COUNT[experimental.tasks]=0
R5_TERM_COUNT[TaskSupport::Required]=2
R5_TERM_COUNT[TaskSupport::Optional]=6
R5_TERM_COUNT[tasks/result]=0
R5_TERM_COUNT[tasks/cancel]=1
R5_TERM_COUNT[tasks/get]=0
R5_TERM_COUNT[poll interval]=0
R5_TERM_COUNT[pollInterval]=1
R5_TERM_COUNT[CreateTaskResult]=3

FILE=pmcp-course/src/part8-advanced/ch21-02-capability-negotiation.md
LINES=265
H1=# Capability Negotiation
H2_COUNT=8
R5_TERM_COUNT[SSE]=0
R5_TERM_COUNT[serverless]=1
R5_TERM_COUNT[owner binding]=0
R5_TERM_COUNT[experimental.tasks]=0
R5_TERM_COUNT[TaskSupport::Required]=2
R5_TERM_COUNT[TaskSupport::Optional]=0
R5_TERM_COUNT[tasks/result]=2
R5_TERM_COUNT[tasks/cancel]=0
R5_TERM_COUNT[tasks/get]=3
R5_TERM_COUNT[poll interval]=0
R5_TERM_COUNT[pollInterval]=0
R5_TERM_COUNT[CreateTaskResult]=8

FILE=pmcp-course/src/part8-advanced/ch21-exercises.md
LINES=210
H1=# Chapter 21 Exercises
H2_COUNT=6
EXERCISE_COUNT=4
R5_TERM_COUNT[SSE]=0
R5_TERM_COUNT[serverless]=0
R5_TERM_COUNT[owner binding]=0
R5_TERM_COUNT[experimental.tasks]=0
R5_TERM_COUNT[TaskSupport::Required]=0
R5_TERM_COUNT[TaskSupport::Optional]=4
R5_TERM_COUNT[tasks/result]=0
R5_TERM_COUNT[tasks/cancel]=0
R5_TERM_COUNT[tasks/get]=0
R5_TERM_COUNT[poll interval]=0
R5_TERM_COUNT[pollInterval]=0
R5_TERM_COUNT[CreateTaskResult]=2
```

## Post-Edit Measurements + Deltas vs Baselines

| File | Pre LINES | Post LINES | Delta | Pre H1 | Post H1 | H1 match? | Pre EX | Post EX |
|------|-----------|------------|-------|--------|---------|-----------|--------|---------|
| ch21-tasks.md | 127 | 128 | +1 | `# MCP Tasks: Long-Running Operations` | `# MCP Tasks: Long-Running Operations` | yes | n/a | n/a |
| ch21-01-lifecycle.md | 372 | 372 | 0 | `# Task Lifecycle and Polling` | `# Task Lifecycle and Polling` | yes | n/a | n/a |
| ch21-02-capability-negotiation.md | 265 | 265 | 0 | `# Capability Negotiation` | `# Capability Negotiation` | yes | n/a | n/a |
| ch21-exercises.md | 210 | 210 | 0 | `# Chapter 21 Exercises` | `# Chapter 21 Exercises` | yes | 4 | 4 |
| ch21-tasks.toml | (did not exist) | 86 | n/a | n/a | n/a | n/a | n/a | n/a |

Post-edit R-5 term counts re-checked: all identical to pre-edit (i.e., no behavioral-prose changes were required — the original prose still matches current crate behavior). Confirms the audit's "0 drift" finding is conservative-correct.

## Task 5 Branch Selection

**Branch B (create minimal)** — pre-edit `ls pmcp-course/src/quizzes/` confirmed only `ch20-mcp-apps.toml` existed for Part VIII; `ch21-tasks.toml` did not exist. Created with 5 questions:

| # | Type | Topic | Source of answer |
|---|------|-------|------------------|
| 1 | ShortAnswer | `Server::builder().task_store(...)` configures task support | `src/server/builder.rs:786`, ch21-01-lifecycle.md L34-50 |
| 2 | MultipleChoice | Why polling over SSE for serverless | ch21-tasks.md L46-78 |
| 3 | MultipleChoice | TaskSupport::Optional semantics | `src/types/tools.rs:163-170`, ch21-01-lifecycle.md L77-83 |
| 4 | ShortAnswer | `RequestHandlerExtra::is_task_request()` | `src/server/cancellation.rs:350`, ch21-02-capability-negotiation.md L42 |
| 5 | MultipleChoice | TaskStore-enforced state machine | `src/types/tasks.rs:56-72`, ch21-tasks.md L105, ch21-01-lifecycle.md L209-234 |

**TOML parse verification (revision R-3):** `python3 -c 'import tomllib; tomllib.loads(open("pmcp-course/src/quizzes/ch21-tasks.toml").read())'` exited 0. `python3 -c "import tomllib; data = tomllib.load(open(..., 'rb')); ..."` parsed cleanly into 5 questions with id, title, lesson_id, pass_threshold fields plus the questions array.

## Final Line Counts

| File | Lines |
|------|-------|
| pmcp-course/src/part8-advanced/ch21-tasks.md | 128 |
| pmcp-course/src/part8-advanced/ch21-01-lifecycle.md | 372 |
| pmcp-course/src/part8-advanced/ch21-02-capability-negotiation.md | 265 |
| pmcp-course/src/part8-advanced/ch21-exercises.md | 210 |
| pmcp-course/src/quizzes/ch21-tasks.toml | 86 |

## Per-Task Status

| Task | Outcome | Files changed | Commit |
|------|---------|---------------|--------|
| 1: Audit + baselines | done; baselines written to /tmp/81-06-baselines.txt | none (audit artifact in /tmp, quoted above) | (no commit — no tracked changes) |
| 2: ch21-tasks.md drift correction | 1 drift item fixed (DynamoDbTaskStore/RedisTaskStore → real types) | pmcp-course/src/part8-advanced/ch21-tasks.md (+4/-3) | ee4e6da8 |
| 3: ch21-01-lifecycle.md + ch21-02-capability-negotiation.md | 0 drift; no-op (verify tolerates) | none | (no commit — no tracked changes) |
| 4: ch21-exercises.md | 0 drift; no-op (structure-preserving verify tolerates) | none | (no commit — no tracked changes) |
| 5: ch21-tasks.toml | Branch B: created minimal 5-question quiz | pmcp-course/src/quizzes/ch21-tasks.toml (+86) | ad08e5de |

## Deviations from Plan

None. Plan executed exactly as written. The drift count came in at the low end of the expected 5-30 range (1 type-name drift item, 0 behavioral-prose drift items) — within the plan's "Zero is possible" budget. No Rules 1-3 auto-fixes needed; no Rule 4 architectural questions raised.

## "Needs Rewrite" Sections

Zero. The audit found no chapter sections requiring rewrite. All three Tasks chapter pages plus the exercises page accurately describe current `pmcp-tasks` + `pmcp` behavior with the single localized type-name fix above.

## Verification

All five verify-automated blocks passed end-to-end (`bc`-based line-delta checks, H1 bytewise comparison, EXERCISE_COUNT equality, `python3 -c 'import tomllib; tomllib.loads(...)'` exit code 0, `[[questions]]` count ≥ 4, TOML line count ≥ 40). Re-run aggregate at end of execution produced `ALL VERIFY CHECKS PASS`.

## Self-Check: PASSED

- File exists: `pmcp-course/src/part8-advanced/ch21-tasks.md` — FOUND
- File exists: `pmcp-course/src/quizzes/ch21-tasks.toml` — FOUND
- Commit `ee4e6da8` in git log — FOUND
- Commit `ad08e5de` in git log — FOUND
- TOML parses under Python tomllib — verified (exit 0)
- All structure-preserving verify checks pass — verified
