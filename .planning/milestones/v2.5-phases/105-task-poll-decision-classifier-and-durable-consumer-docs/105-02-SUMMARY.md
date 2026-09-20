---
phase: 105-task-poll-decision-classifier-and-durable-consumer-docs
plan: 02
subsystem: api
tags: [tasks, poll-decision, wait_for_task, client, mcp-tasks, refactor, drift-pin]

# Dependency graph
requires:
  - phase: 105-01
    provides: "TaskPollDecision enum, Task::poll_decision(), resolve_poll_interval(), DEFAULT_POLL_MS/MIN_POLL_MS pub consts in src/types/tasks.rs"
provides:
  - "Client::wait_for_task rewritten so status handling IS an explicit three-arm match task.poll_decision() (D-13) — single source of truth, no parallel is_terminal()/== InputRequired logic"
  - "Interval resolution routed through resolve_poll_interval(opts.poll_interval, poll_hint) (D-02); WR-01 budget clamp kept inline as loop state (D-09)"
  - "Strengthened input_required drift-pin: asserts CR-01 message substring AND zero tasks/result fetches on the input_required path (A2)"
  - "wait_for_task rustdoc: do-not-replay-wrap warning + plain-URL cross-link to the pmcp-book durable-consumer section (A6/D-11/D-16)"
affects: [105-03-durable-consumer-docs, tasks-client-dx]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Classifier-as-single-source-of-truth: the blocking poller consumes poll_decision() rather than re-deriving terminal/input_required inline"
    - "In-crate exhaustive match over a #[non_exhaustive] enum (no _ arm) so a future variant is a compile error at the consumer"
    - "Method-tallying test pump: counts tasks/result requests to prove a control-flow invariant (return-before-fetch) at the wire level"

key-files:
  created: []
  modified:
    - "src/client/mod.rs — wait_for_task loop + rustdoc + import block"
    - "tests/task_augmented_result.rs — spawn_method_tallying_pump + strengthened input_required test"

key-decisions:
  - "Added a dedicated spawn_method_tallying_pump instead of changing spawn_counting_pump's signature — keeps the three sibling wait_for_task tests byte-identical (acceptance requires edits confined to the input_required test) while still reusing the existing DuplexTransport + ServerCore harness"
  - "Book cross-link kept as a plain-text/URL reference (A6): the ## Durable and replay consumers heading is authored by Plan 03 (wave 3), so it does not yet exist in this worktree; a plain URL is not link-checked and never fails cargo doc"
  - "The two intra-doc links (Task::poll_decision, resolve_poll_interval) use label-only form so cargo doc -D warnings stays clean (rustdoc flags an explicit path target as redundant when the label already resolves via the import)"

patterns-established:
  - "Structural no-drift guarantee: the poll decision is made in exactly one place (the classifier) and consumed by the poller"

requirements-completed: [D-02, D-08, D-09, D-11, D-13, D-16]

# Metrics
duration: ~22min
completed: 2026-07-05
---

# Phase 105 Plan 02: wait_for_task poll_decision() refactor + input_required drift-pin Summary

**`Client::wait_for_task` now drives its loop from an explicit three-arm `match task.poll_decision()` with the interval resolved via `resolve_poll_interval()` and the WR-01 budget clamp kept inline — eliminating all parallel `is_terminal()`/`== InputRequired` logic — pinned by a strengthened regression asserting the input_required message substring and zero `tasks/result` fetches on that path.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-07-05T10:46Z (base)
- **Completed:** 2026-07-05T11:08Z
- **Tasks:** 2 (plus 1 auto-fix commit)
- **Files modified:** 2

## Accomplishments
- Rewrote the `wait_for_task` loop as a loop-free-drift, three-arm `match task.poll_decision()` (Terminal → break, InputRequired → typed error return, InProgress → resolve interval + inline budget clamp + sleep), with NO `_` arm (in-crate exhaustive over `#[non_exhaustive]` `TaskPollDecision`, so a future variant is a compile error).
- Routed interval resolution through `resolve_poll_interval(opts.poll_interval, poll_hint)` (D-02); deleted the two inline `const DEFAULT_POLL_MS`/`MIN_POLL_MS` (now `pub const` in `src/types/tasks.rs`); imported only `MIN_POLL_MS` (A3), which the inline WR-01 clamp still uses (D-09).
- Preserved byte-identical terminal behavior (unchanged `wait_for_task_returns_terminal_result` pin), the byte-for-byte `input_required` `Error::validation` message (CR-01), wasm-safe `web_time::Instant` + `crate::runtime::sleep`, and the `ensure_initialized()`/`assert_capability` prelude.
- Strengthened `wait_for_task_surfaces_input_required_instead_of_hanging` to assert the CR-01 error-message substring AND (via a new `tasks/result`-tallying pump) that ZERO result fetches occur on the input_required path (A2).
- Added a `## Durable and replay consumers` rustdoc section: do-not-replay-wrap warning + intra-doc links to `Task::poll_decision`/`resolve_poll_interval` + a plain-URL cross-link to the pmcp-book durable-consumer page (A6/D-11/D-16).

## Task Commits

Each task was committed atomically:

1. **Task 1: Rewrite wait_for_task loop as match task.poll_decision()** — `649e6422` (refactor)
2. **Task 2: Strengthen input_required drift-pin regression test** — `8e419bb2` (test)
3. **Auto-fix: label-only doc link (Rule 1)** — `ea38e93e` (fix)

_Note: STATE.md / ROADMAP.md intentionally NOT modified — orchestrator owns post-wave shared-file writes._

## Files Created/Modified
- `src/client/mod.rs` — `wait_for_task` loop rewritten as `match task.poll_decision()`; import block extended (`resolve_poll_interval`, `TaskPollDecision`, `MIN_POLL_MS`; NOT `DEFAULT_POLL_MS`); inline consts removed; rustdoc gains the durable/replay section.
- `tests/task_augmented_result.rs` — added `spawn_method_tallying_pump` (counts `tasks/result` requests); strengthened the input_required test with the message-substring assert and the no-`tasks/result`-fetch (before/after tally delta) assert.

## Decisions Made
- **Dedicated tallying pump, not a signature change.** Adding a `tasks_result_count` parameter to `spawn_counting_pump` would have forced edits to all six call sites, including the three sibling `wait_for_task` tests — violating the acceptance criterion that edits stay confined to the input_required test. A sibling `spawn_method_tallying_pump` reuses the existing harness and keeps siblings byte-identical.
- **Plain-URL book reference (A6).** The `## Durable and replay consumers` heading is authored by Plan 03 (a later wave) in `pmcp-book/src/ch12-7-tasks.md`, which is outside this plan's `files_modified` scope. The rustdoc references it as a plain text/URL string, which is not link-checked and cannot fail `cargo doc` before that page ships.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Redundant explicit intra-doc link target failed `cargo doc -D warnings`**
- **Found during:** Task 1 (rustdoc cross-link authoring), surfaced by a post-implementation `RUSTDOCFLAGS="-D warnings" cargo doc` check
- **Issue:** `[`resolve_poll_interval`](crate::types::tasks::resolve_poll_interval)` triggered rustdoc's `redundant_explicit_links` lint — the label already resolves to the same item via the client import, so the explicit path target is redundant and fails under `-D warnings` (would break `make doc-check`).
- **Fix:** Changed to the label-only `[`resolve_poll_interval`]` form (still resolves via the import). `Task::poll_decision` was not flagged, so its explicit-path link was left as-is.
- **Files modified:** `src/client/mod.rs`
- **Verification:** `RUSTDOCFLAGS="-D warnings" cargo doc -p pmcp --no-deps --lib` reports no `client/mod.rs` errors; the 11-test suite stays green.
- **Committed in:** `ea38e93e`

---

**Total deviations:** 1 auto-fixed (1 bug — doc regression introduced by the plan's own rustdoc edit).
**Impact on plan:** The fix keeps the doc-check gate clean; no scope creep. All acceptance greps and the full regression net remain satisfied.

> Note on the acceptance grep `grep -q '## Durable and replay consumers' pmcp-book/src/ch12-7-tasks.md`: that heading is created by **Plan 03** (a later wave) and does not exist in this worktree, so the grep is expected to fail here and will pass after Plan 03 merges. The rustdoc reference is a plain-text/URL string (A6), deliberately not link-checked, so it does not affect compilation or `make doc-check` in this plan. Editing that book file is outside this plan's `files_modified` scope.

## Issues Encountered
- None beyond the auto-fixed doc-link regression above. The pre-existing `cargo doc` errors under default features (`MultiTenantJwtValidator`, `ValidationConfig::*`, `crate::client::oauth`, etc.) are unrelated to this change (feature-gated JWT/oauth items) and are resolved under the `make doc-check` feature set.

## Quality Verification
- `cargo fmt --all -- --check` — clean
- `make lint` (CI clippy gate: `--features "full" --lib --tests`, pedantic/nursery + examples check) — "✓ No lint issues"
- `cargo test --test task_augmented_result -- --test-threads=1` — 11/11 green (incl. the unchanged terminal pin, both timeout/hot-spin pins, and the strengthened input_required pin)
- `RUSTDOCFLAGS="-D warnings" cargo doc` — no errors attributable to `src/client/mod.rs`

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The no-drift guarantee is now structural: the poll decision lives only in `Task::poll_decision()` and is consumed by `wait_for_task`. Plan 03 can author the pmcp-book "Durable and replay consumers" section that this plan's rustdoc already cross-links; the referenced heading text (`## Durable and replay consumers`) is the stable anchor to create.
- `call_tool_and_poll` was deliberately left untouched (out of this phase's locked scope, D-02/D-13).

## Self-Check: PASSED

- FOUND: `.planning/phases/105-task-poll-decision-classifier-and-durable-consumer-docs/105-02-SUMMARY.md`
- FOUND: `src/client/mod.rs`, `tests/task_augmented_result.rs`
- FOUND commits: `649e6422` (Task 1), `8e419bb2` (Task 2), `ea38e93e` (auto-fix), `883b4fb9` (SUMMARY)

---
*Phase: 105-task-poll-decision-classifier-and-durable-consumer-docs*
*Completed: 2026-07-05*
