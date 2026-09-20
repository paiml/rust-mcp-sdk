---
phase: 105-task-poll-decision-classifier-and-durable-consumer-docs
plan: 01
subsystem: types
tags: [tasks, poll-decision, classifier, wasm-clean, additive-api]
requires: []
provides:
  - "TaskPollDecision enum (3-variant, #[non_exhaustive], no serde)"
  - "Task::poll_decision() pure classifier"
  - "resolve_poll_interval(caller, hint) -> u64 ms"
  - "pub const DEFAULT_POLL_MS / MIN_POLL_MS"
affects:
  - "src/client/mod.rs wait_for_task refactor (Plan 02 will consume these)"
tech-stack:
  added: []
  patterns:
    - "types-layer home (no cfg gate) keeps the wasm boundary clean"
    - "total match over exhaustive TaskStatus, no _ arm"
    - "single-source-of-truth decision fn consumed by every poller"
key-files:
  created: []
  modified:
    - src/types/tasks.rs
decisions:
  - "TaskPollDecision derives Debug/Clone/PartialEq/Eq only — deliberately NO serde (classifier value, not a wire type)"
  - "TaskPollDecision is #[non_exhaustive] (D-04); TaskStatus stays exhaustive (D-15) — kept as distinct claims in docs"
  - "resolve_poll_interval returns u64 ms, not Duration (D-12)"
  - "DEFAULT_POLL_MS/MIN_POLL_MS documented as STABLE, SUPPORTED public defaults (A4), not internal tunables"
metrics:
  duration: ~15m
  completed: 2026-07-05
requirements: [D-01, D-03, D-04, D-05, D-06, D-07, D-08, D-12, D-15, D-16]
---

# Phase 105 Plan 01: Task poll-decision classifier + interval resolver Summary

Added the pure, loop-free poll-decision primitive to the types layer — the
`TaskPollDecision` classifier enum, the `Task::poll_decision()` method,
the `resolve_poll_interval()` interval resolver, and the two `pub const`
interval floors — all in `src/types/tasks.rs`, so every future task poller
(blocking `wait_for_task` in Plan 02 and external durable/replay consumers)
consumes one non-drifting decision source.

## What was built

- **`TaskPollDecision` enum** (after the `TaskStatus` block): three variants
  `Terminal { status: TaskStatus }`, `InProgress { poll_hint: Option<u64> }`,
  and unit `InputRequired`. Derives `Debug, Clone, PartialEq, Eq` and is
  `#[non_exhaustive]` (D-04). Deliberately no `Serialize`/`Deserialize` and no
  `#[serde(...)]` — it is a returned classifier value, not a wire type. The
  `Terminal` variant rustdoc states the caller still issues a separate
  `tasks/result` call (D-06/D-16); the `InProgress` rustdoc states `poll_hint`
  is the raw server `pollInterval` verbatim (D-07). The enum rustdoc keeps
  "`TaskStatus` exhaustive today" and "`TaskPollDecision` `#[non_exhaustive]` for
  future-proofing" as distinct claims (D-15).

- **`Task::poll_decision(&self) -> TaskPollDecision`**: a total `match self.status`
  with three arms and NO `_` wildcard (TaskStatus is exhaustive). `Working` →
  `InProgress { poll_hint: self.poll_interval }`; `InputRequired` → unit variant;
  `Completed | Failed | Cancelled` → `Terminal { status: self.status }`. Rustdoc
  includes a runnable `# Examples` doctest mirroring `CallToolResult::related_task()`.

- **`pub const DEFAULT_POLL_MS: u64 = 1000` and `pub const MIN_POLL_MS: u64 = 50`**:
  documented as STABLE, SUPPORTED public defaults / documented poll-interval policy
  (A4), not internal tunables.

- **`resolve_poll_interval(caller_override, hint) -> u64`**: body
  `caller_override.or(hint).unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS)` — lifted
  verbatim from the inline consts+chain at `src/client/mod.rs:685-716` (D-08).
  Returns `u64` ms, not `Duration` (D-12). Rustdoc includes a runnable `# Examples`
  doctest asserting the precedence chain.

All symbols are re-exported for free via the existing `pub use super::tasks::*`
at `src/types/protocol/mod.rs:23`.

## Tests added (to the existing `#[cfg(test)] mod tests`)

- `poll_decision_maps_every_status` + `poll_decision_covers_all_statuses_exhaustively`
  — the full status→decision map (D-03/D-15 exhaustiveness).
- `poll_decision_matches_expected_map` proptest — every status × arbitrary
  `poll_interval` returns the mapped variant.
- `resolve_poll_interval_precedence` + `resolve_poll_interval_floors_zero` —
  all behavior-block cases (override wins, hint used, 1000 default, 50 floor).
- `resolve_poll_interval_never_below_floor` proptest — the 50 ms floor holds for
  ALL caller/hint inputs (T-105-01 invariant).

`cargo test --lib types::tasks -- --test-threads=1` → 20 passed.
`cargo test --doc types::tasks` → 4 doctests passed.

## Deviations from Plan

None functionally. One process note: the plan marks both tasks `tdd="true"`, but
this repo's pre-commit hook runs `make quality-gate` which requires a successful
build — a non-compiling RED-only commit (tests referencing not-yet-existing types)
would be blocked by the hook. Each task was therefore committed as a single
compiling, green commit containing both the test and the implementation, preserving
the test-first authoring intent (tests written against the target shape before the
impl was finalized) while respecting the mandatory quality gate. No behavior or API
surface differs from the plan.

## Verification

- `cargo test --lib types::tasks -- --test-threads=1` — green (20 tests)
- `cargo test --doc types::tasks` — green (4 doctests)
- `cargo build` (default) — compiles
- `cargo clippy --lib --all-features` — zero warnings
- `cargo fmt --all -- --check` — clean
- No `#[cfg(...)]` gate near the new items (wasm boundary stays clean, Pitfall 3)
- Acceptance greps: enum present + `#[non_exhaustive]` + no `Serialize`, method
  present, both consts present, resolver signature + body present, floor proptest present.

## TDD Gate Compliance

Both tasks are `tdd="true"`. Due to the build-blocking pre-commit hook (above),
tests and implementation ship together per task rather than as separate
`test(...)`/`feat(...)` commits. Tests were authored against the target shapes
first and drive the full status→decision map plus the resolver precedence/floor
invariants; both commits are `feat(105-01): ...`.

## Commits

- `9de6e44e` feat(105-01): add TaskPollDecision enum + Task::poll_decision() classifier
- `b2d4b193` feat(105-01): add resolve_poll_interval() + pub const poll floors

## Self-Check: PASSED

- src/types/tasks.rs — FOUND (modified, both commits)
- Commit 9de6e44e — FOUND
- Commit b2d4b193 — FOUND
