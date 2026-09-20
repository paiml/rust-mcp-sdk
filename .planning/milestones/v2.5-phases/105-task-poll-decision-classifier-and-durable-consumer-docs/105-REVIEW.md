---
phase: 105-task-poll-decision-classifier-and-durable-consumer-docs
reviewed: 2026-07-05T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - src/types/tasks.rs
  - src/client/mod.rs
  - tests/task_augmented_result.rs
  - examples/s48_durable_poll_decision.rs
findings:
  critical: 0
  warning: 1
  info: 2
  total: 3
status: issues_found
---

# Phase 105: Code Review Report

**Reviewed:** 2026-07-05T00:00:00Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Phase 105 factors the terminal/pollable/input-required poll decision out of
`Client::wait_for_task`'s loop into a shared, loop-free classifier
(`TaskPollDecision` + `Task::poll_decision()` + `resolve_poll_interval`) in
`src/types/tasks.rs`, and adds the `s48_durable_poll_decision` example plus
live-harness tests.

**Scope-fence verification (LOCKED constraints — all held):**

- **Semantic equivalence of the refactored loop is byte-identical.**
  `resolve_poll_interval(opts.poll_interval, poll_hint)` expands to
  `opts.poll_interval.or(poll_hint).unwrap_or(1000).max(50)`, and for a
  `Working` task `poll_hint == task.poll_interval`, exactly reproducing the
  prior `opts.poll_interval.or(task.poll_interval).unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS)`.
  The budget clamp (`saturating_mul(1000)` → `try_from(...).unwrap_or(u64::MAX)`
  → `saturating_sub` → `remaining_ms.max(MIN_POLL_MS)`), the timeout return
  (`Error::timeout(budget_ms)`), and the poll-interval precedence are unchanged
  (confirmed via `git diff beb7e7c..HEAD -- src/client/mod.rs`).
- **`input_required` typed-error message is byte-for-byte preserved**
  (`"task {task_id} is input_required; wait_for_task cannot provide input —
  handle the elicitation, then resume polling"`), and is still returned BEFORE
  any `tasks/result` fetch. The A2/D-13 drift-pin test asserts both the message
  substring and the zero-`tasks/result` invariant.
- **`Task::poll_decision()` is a pure, total function** — matches all five
  `TaskStatus` variants with no `_` wildcard, no I/O, no panics, no arithmetic.
  Exhaustiveness is table- and property-tested.
- **`resolve_poll_interval` floor/precedence is correct for all `Option<u64>`
  inputs** (`caller.or(hint).unwrap_or(1000).max(50)`), proptest-verified never
  below `MIN_POLL_MS`. No overflow risk (`.max()` only).
- **No wire changes, no new `TaskStatus` variants.**

The one substantive finding is a robustness gap in the s48 example (WR-01):
its background "worker" swallows both store operations and the main poll loop
is unbounded, so a setup regression manifests as an indefinite CI hang rather
than the clean failure the example's own "HARD assertion" contract promises.
The remaining items are duplicated harness code and a minor doc inconsistency.

Note: `src/types/tasks.rs` is unchanged within the `beb7e7c..HEAD` diff window
(the classifier landed in commits `9de6e44e`/`b2d4b193`, which are ancestors of
the diff base). It was reviewed in full per the explicit file list and phase
context; no defects were found in it.

## Warnings

### WR-01: s48 example swallows worker store errors under an unbounded poll loop — a setup regression hangs CI instead of failing

**File:** `examples/s48_durable_poll_decision.rs:208-217` (worker) and `224-263` (main poll loop)
**Issue:**
The background worker discards the result of both store mutations:

```rust
let _ = worker_store.set_result(&worker_task_id, "local", terminal).await;
let _ = worker_store
    .update_status(&worker_task_id, "local", TaskStatus::Completed, None)
    .await;
```

The main `loop { ... }` has no `max_poll_duration` / iteration bound — it only
exits on `Terminal` or `InputRequired`. If either store call fails (e.g. the
`"local"` owner assumption drifts, or the task id is wrong), the task stays
`Working` forever and the poll loop spins on 50–60 ms sleeps **indefinitely**.

This directly contradicts the example's own stated contract in the module doc:
"Every claim below is a HARD assertion (returns `Err` on failure), not just
printed output, so this example doubles as a regression harness." The two
worker mutations — the very setup the terminal assertion depends on — are the
only steps that are NOT asserted. Under `make test-examples` a genuine
regression (owner-resolution change, store API change) would surface as a
wedged CI job with no diagnostic, not as the clean `Err` the harness promises.
The whole example rests on the undocumented-here assumption that `"local"` is
the owner an unauthenticated duplex session resolves to (the same assumption
`tests/task_augmented_result.rs:471` calls out); if that assumption breaks,
this harness hides it as a hang.

**Fix:** Assert the worker's store results (propagate failure out of the spawned
task via a channel/`JoinHandle`, or at minimum log on `Err`), and bound the
poll loop with a wall-clock ceiling so a stuck task fails fast:

```rust
// worker: surface failure instead of swallowing it
worker_store.set_result(&worker_task_id, "local", terminal).await
    .expect("worker: set_result must persist the terminal result");
worker_store.update_status(&worker_task_id, "local", TaskStatus::Completed, None).await
    .expect("worker: Working -> Completed must succeed");

// main loop: fail fast instead of hanging on a stuck task
let deadline = std::time::Instant::now() + Duration::from_secs(10);
loop {
    if std::time::Instant::now() >= deadline {
        return Err(Error::internal("task never reached terminal within 10s"));
    }
    // ... existing tasks_get + poll_decision match ...
}
```

## Info

### IN-01: DuplexTransport harness duplicated between the test and the example

**File:** `examples/s48_durable_poll_decision.rs:68-140` and `tests/task_augmented_result.rs:135-243`
**Issue:** The `DuplexTransport` struct, its `Transport` impl, `pair()`, and the
spawn-pump helper are copied nearly verbatim into the example. The example's own
comment ("Copied down to the SMALLEST single-server pieces…") acknowledges this.
Duplication means a transport-contract change must be edited in two places and
they can drift silently.
**Fix:** Low priority and partly unavoidable — a `cargo run --example` binary
cannot import a `tests/`-private harness. If this duplex harness is reused again
(it now appears in at least two places), promote it to a small
`#[cfg(any(test, feature = "test-harness"))]` module under `src/` or an
`examples/common/` module so both the test and the example share one definition.

### IN-02: Example module doc claims "wasm-safe" sleep while the file is cfg-excluded from wasm

**File:** `examples/s48_durable_poll_decision.rs:15-17` (doc) vs `41` (`#![cfg(not(target_arch = "wasm32"))]`)
**Issue:** The header prose emphasizes "a single wasm-safe [`pmcp::runtime::sleep`]",
but line 41 compiles the entire example out on `wasm32`, so the wasm-safety of
the sleep is never exercised by this binary. The claim is true of the API but
potentially misleading in an example that never targets wasm.
**Fix:** Reword to "a single `pmcp::runtime::sleep` (the wasm-safe runtime shim
this native example uses for parity with the browser poll loop)", or drop the
"wasm-safe" emphasis here since the example is native-only.

---

_Reviewed: 2026-07-05T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
