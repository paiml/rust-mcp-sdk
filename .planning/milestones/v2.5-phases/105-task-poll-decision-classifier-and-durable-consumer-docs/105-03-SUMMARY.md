---
phase: 105-task-poll-decision-classifier-and-durable-consumer-docs
plan: 03
subsystem: docs
tags: [tasks, poll-decision, durable, replay, mdbook, example]

# Dependency graph
requires:
  - phase: 105-01
    provides: "TaskPollDecision enum, Task::poll_decision(), resolve_poll_interval() in src/types/tasks.rs"
provides:
  - "examples/s48_durable_poll_decision.rs — runnable plain-loop classifier example (D-10)"
  - "'Durable and replay consumers' section in the Tasks chapter (D-11/D-14/D-15/D-16)"
  - "Cross-link from task-augmented-results.md into the new durable-consumer section"
affects: [pmcp.run durable poller, tasks documentation, task-augmented-results docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Loop-free per-poll classifier consumption: tasks_get -> poll_decision -> resolve_poll_interval -> wasm-safe sleep, with the terminal result fetched via a separate consumer-owned tasks/result call"
    - "Minimal single-server duplex example harness (A5): mpsc DuplexTransport + simple pump + one stay-working task tool, driven to terminal out-of-band via the shared store"

key-files:
  created:
    - examples/s48_durable_poll_decision.rs
  modified:
    - pmcp-book/src/ch12-7-tasks.md
    - pmcp-book/src/task-augmented-results.md

key-decisions:
  - "The s48 example guards tasks_result behind a `terminal` boolean so the input_required path can never fetch a result (A1 / D-06 / D-16 / T-105-06)"
  - "TaskPollDecision is #[non_exhaustive], so the external example carries a defensive wildcard arm (keep-polling), documented as a semver affordance, NOT runtime unknown-status handling (D-15)"
  - "Rustdoc-style intra-doc links were rendered as plain code spans in the book (mdbook does not resolve rustdoc links)"
  - "The 'Run with' doc line drops the surrounding backtick so the acceptance grep matches contiguously; the wait_for_task token is deliberately absent from the example source (grep returns 0)"

patterns-established:
  - "Durable/replay consumer pattern: memoize tasks/get (ctx.step) + suspend via ctx.wait, classify the already-deserialized Task with the pure poll_decision(), fetch the terminal payload as its own memoized tasks/result step"

requirements-completed: [D-10, D-11, D-14, D-15, D-16]

# Metrics
duration: ~25min
completed: 2026-07-05
---

# Phase 105 Plan 03: Durable-consumer example + Tasks-chapter section Summary

**Shipped the consumer-facing half of the poll-decision classifier: a runnable `s48` example that drives a plain classifier loop (no `wait_for_task`, wasm-safe sleep, terminal-guarded result fetch) plus a "Durable and replay consumers" Tasks-chapter section teaching the `ctx.step`/`ctx.wait` pattern with replay-determinism, semver-honesty, and the "do not wrap the blocking waiter in a replay workflow" warning.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-05
- **Tasks:** 2/2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

### Task 1 — `examples/s48_durable_poll_decision.rs` (D-10)
- Built the smallest standalone in-process duplex harness (A5): an mpsc-backed `DuplexTransport`, a simple request pump, and one `stay_working` task tool over an `InMemoryTaskStore`. The full duplex test harness was NOT lifted.
- The consumer drives a plain poll loop: `tasks_get` → `Task::poll_decision()` → on `InProgress`, `resolve_poll_interval(None, poll_hint)` + `pmcp::runtime::sleep` (wasm-safe; zero `tokio::time::sleep`).
- **A1 guard:** a background worker sets the result then flips Working→Completed via the shared store; the `tasks_result` fetch sits inside `if terminal { … }`, so the `input_required` arm can never reach it. `wait_for_task` does not appear in the source (grep = 0).
- HARD assertion on the terminal content (`"durable job finished"`) makes the example a compile+run regression harness. `cargo run --example s48_durable_poll_decision --features full` exits 0; clippy `-D warnings` clean; `cargo fmt --check` clean.

### Task 2 — "Durable and replay consumers" section (D-11/D-14/D-15/D-16)
- Appended a new `## Durable and replay consumers` section to `ch12-7-tasks.md` (slug `#durable-and-replay-consumers`), after the Task Status State Machine section.
- Content: the `ctx.step`/`ctx.wait` typed-accessors-without-the-loop pattern (illustrative `rust,ignore` snippet, no pmcp.run internals named); **D-14** replay-determinism scoped to an already-deserialized Task (network `tasks/get` + serde decode inside the memoized step; unknown status fails at deserialization before classification); **D-15** two distinct claims (TaskStatus exhaustive today vs TaskPollDecision `#[non_exhaustive]` future-proofing, not runtime unknown-status handling); **D-06/D-16** the separate `tasks/result` step and the "never on input_required" rule; **D-16** an explicit warning callout against wrapping the blocking waiter in a replay workflow. Points readers at `s48`.
- Added a cross-link from `task-augmented-results.md` (near "## Try it") to `ch12-7-tasks.md#durable-and-replay-consumers` with a slug-matched anchor (W1).
- `make book` (mdbook build) exits 0; `make doc-check` (rustdoc `-D warnings`) exits 0.

## Deviations from Plan

**None functionally — plan executed as written.** Two minor phrasing adjustments were made to satisfy the literal acceptance greps without changing intent:
- The example's doc header omits the `wait_for_task` identifier (referring to "the blocking poll-to-terminal helper" instead) so `grep -c 'wait_for_task' examples/s48_durable_poll_decision.rs` returns 0 while still teaching the "don't use it" point in prose.
- The `Run with:` line drops the surrounding backtick so the contiguous acceptance grep matches.
- `TaskPollDecision` being `#[non_exhaustive]` required a wildcard arm in the example's `match` (external crate); it is documented as the semver affordance (defensive keep-polling), consistent with the D-15 book claim.

## Threat Model Coverage

- **T-105-01 (DoS / hot-spin):** the s48 loop waits via `resolve_poll_interval` (50 ms floor); the book teaches the resolver, not hand-rolled sleeps. ✅
- **T-105-05 (misinformation on unknown statuses):** D-15 keeps the two claims distinct; no claim of runtime graceful handling of unknown statuses. ✅
- **T-105-06 (result fetch on non-terminal task):** the `if terminal { … }` guard makes `tasks_result` unreachable on the input_required path (source-reviewed + grep). ✅

## Verification

- `cargo run --example s48_durable_poll_decision --features full` → exits 0, prints the classifier loop then `OK: … reached Terminal and fetched the owned result`.
- `cargo clippy --example s48_durable_poll_decision --features full -- -D warnings` → clean.
- Book greps: heading present; `replay`, `wait_for_task` warning, both semver claims, `tasks/result`, and the `#durable-and-replay-consumers` cross-link all present.
- `make book` → exits 0. `make doc-check` → exits 0.

## Self-Check: PASSED

- FOUND: examples/s48_durable_poll_decision.rs
- FOUND: pmcp-book/src/ch12-7-tasks.md (## Durable and replay consumers)
- FOUND: pmcp-book/src/task-augmented-results.md (#durable-and-replay-consumers cross-link)
- FOUND commit: 14ff91db (feat s48 example)
- FOUND commit: cb686f98 (docs durable-consumer section)
