---
phase: 105
reviewers: [codex]
reviewed_at: 2026-07-05T17:09:33Z
plans_reviewed: [105-01-PLAN.md, 105-02-PLAN.md, 105-03-PLAN.md]
supersedes: pre-planning discussion review (folded into 105-CONTEXT.md D-01..D-16); this file now reviews the three generated PLAN.md files
---

# Cross-AI Plan Review — Phase 105

> Reviews the three generated plans (105-01/02/03), not the pre-planning discussion.
> The earlier discussion-stage Codex review was folded into 105-CONTEXT.md decisions.

## Codex Review

## Summary

The three plans are strong overall: they keep the change wire-neutral, isolate the new public primitive in the types layer, make `wait_for_task` structurally consume that primitive, and give durable/replay consumers both API and documentation. The main risks are API surface permanence around the two new public constants, a few verification gaps where “byte-identical” behavior is asserted indirectly, and some possible example overreach from copying a test harness into an example.

## 105-01 Plan Review

### Strengths

- Clear placement in `src/types/tasks.rs`; this is the right layer for a pure `Task` classifier.
- Correctly separates `TaskStatus` exhaustiveness from `TaskPollDecision #[non_exhaustive]`.
- Good rustdoc requirements, especially the `Terminal` note that `tasks/result` remains separate.
- Resolver semantics are explicit and match the existing inline precedence.
- Unit + property coverage is appropriate for the classifier and interval floor.

### Concerns

- **MEDIUM:** Exposing `DEFAULT_POLL_MS` and `MIN_POLL_MS` as `pub const` makes these part of the long-term public contract. That may be fine, but the plan treats it as low-risk without discussing future tuning or naming stability.
- **LOW:** `TaskPollDecision` could reasonably derive `Copy`; all payloads are `Copy`. Omitting it is not wrong, but less ergonomic for a tiny decision enum.
- **LOW:** The requested proptest for “generates a Task with each TaskStatus” may add complexity without much value over the exhaustive table unless the generator is very small and deterministic.
- **LOW:** Acceptance checks using grep around derive lines may be brittle and can give false confidence. The real gate should be compilation, doctests, and focused tests.

### Suggestions

- Consider naming constants more specifically: `DEFAULT_TASK_POLL_INTERVAL_MS` and `MIN_TASK_POLL_INTERVAL_MS`, unless existing naming style strongly favors the shorter names.
- Explicitly decide whether the constants are stable public policy or merely documented defaults. If the latter, keep them private and expose only `resolve_poll_interval`.
- Add `Copy` to `TaskPollDecision` unless there is a local convention against it.
- Keep the proptest simple, or replace it with a table test plus one property for resolver floor; the classifier state space is only five statuses.

### Risk Assessment

**LOW to MEDIUM.** The implementation is simple and well scoped. The only medium concern is public API commitment around constants.

## 105-02 Plan Review

### Strengths

- Correctly makes no-drift structural: `wait_for_task` must `match task.poll_decision()`.
- Preserves the WR-01 budget clamp inside `wait_for_task`, which is the right boundary.
- Explicitly forbids touching `call_tool_and_poll`, avoiding scope creep.
- Strengthening the `input_required` test is a good targeted regression pin.
- Maintains wasm-safe timing and the existing sleep abstraction.

### Concerns

- **MEDIUM:** “Byte-identical terminal behavior” is mostly enforced by existing tests, but the plan does not require a direct test proving `tasks_result` is still called only after a terminal decision and not on `InputRequired`.
- **MEDIUM:** The `input_required` assertion pins only a substring. That is pragmatic, but weaker than the “byte-identical” wording.
- **LOW:** Importing both `DEFAULT_POLL_MS` and `MIN_POLL_MS` into `client/mod.rs` may only need `MIN_POLL_MS` after deleting the inline default. Unused imports will be caught by clippy, but the plan should avoid asking for unnecessary imports.
- **LOW:** Rustdoc cross-link to a book section from source rustdoc may become stale or not resolve under rustdoc. Plain URL/text is safer than intra-doc link syntax.

### Suggestions

- Require a regression assertion that the terminal path still returns the same `tasks/result` content, ideally by keeping the existing terminal test unchanged and explicitly calling it out as the terminal byte-behavior pin.
- For the `input_required` message, either assert the full current message or clearly downgrade the claim from “byte-identical” to “semantically/message-substring identical.”
- Import only the symbols actually used by `src/client/mod.rs`.
- Add an acceptance check that `tasks_result(task_id)` remains after the loop, not inside the `Terminal` match arm.

### Risk Assessment

**LOW.** The refactor is mechanical and the plan has good guardrails. The biggest risk is overstating “byte-identical” relative to what the tests actually assert.

## 105-03 Plan Review

### Strengths

- Correctly separates runnable example from durable-runtime prose; no fake replay harness.
- The example focuses on the intended consumer pattern: `tasks_get` → `poll_decision` → resolver → wait → separate `tasks_result`.
- Documentation obligations are precise and address the important replay determinism caveat.
- Good callout that `wait_for_task` should not be wrapped inside replay workflows.
- Cross-linking from task-augmented results is appropriate.

### Concerns

- **MEDIUM:** Copying the duplex test harness into an example can bloat the example and expose internal-style plumbing to users. It may distract from the classifier pattern.
- **MEDIUM:** The example’s `InputRequired` arm “breaks” in the sketched loop. If it then fetches `tasks_result`, that would be wrong for an input-required task. The plan should require a `saw_terminal` flag or return early on `InputRequired`.
- **LOW:** “Use `pmcp::runtime::sleep` never tokio sleep” is fine, but the example is native-only and already `cfg(not wasm32)`. This is consistent but slightly over-specified.
- **LOW:** Markdown-only verification via grep is weak; `make doc-check` is listed later, but should be part of the task verification, not only phase verification.

### Suggestions

- Keep the example minimal. If the full duplex harness is lengthy, consider extracting only the smallest standalone server needed to produce a task.
- Make the example’s control flow explicit:
  - `Terminal` sets a terminal flag and exits the loop.
  - `InputRequired` returns an explanatory error or exits before `tasks_result`.
  - `tasks_result` is called only after terminal.
- Add `cargo test --examples --features full` or equivalent if the repo already uses that gate.
- Run `make doc-check` after the book edits, not only at the phase end.

### Risk Assessment

**MEDIUM.** The docs are well designed, but the example has a real control-flow trap around `InputRequired` and could become too large if it copies the test harness wholesale.

## Overall Assessment

These plans do achieve the phase goal: they make `InputRequired` actionable for durable/replay consumers without changing wire protocol or `wait_for_task` blocking behavior. The sequencing is sound: Plan 01 creates the primitive, while Plans 02 and 03 consume it independently in Wave 2.

The highest-value improvements are:

- Tighten the `InputRequired` branch in the example so it never falls through to `tasks_result`.
- Reconsider or more deliberately name the public constants.
- Align “byte-identical” claims with actual test assertions, or strengthen tests accordingly.
- Keep `wait_for_task` as the only refactored poller; the plans correctly avoid `call_tool_and_poll`.

Overall risk: **LOW to MEDIUM**. The code change is small and well constrained; the main risks are public API permanence and documentation/example precision.

---

## Consensus Summary

Only one external reviewer (Codex) was invoked this run, so "consensus" reflects
Codex's ranked findings against the plans as written.

### Agreed Strengths
- Wire-neutral, additive design with the pure classifier isolated in the types layer (`src/types/tasks.rs`) — correct architectural home.
- `wait_for_task` structurally consumes `task.poll_decision()` (no-drift discipline), budget clamp correctly stays in the client layer, and `call_tool_and_poll` is explicitly fenced out of scope.
- Docs half is well designed: runnable example separated from durable-runtime prose, replay-determinism caveat and "don't wrap `wait_for_task` in replay workflows" callout present.
- Sequencing is sound (Plan 01 primitive in Wave 1; Plans 02/03 consume it in parallel in Wave 2).

### Agreed Concerns (highest priority first)
1. **[MEDIUM] Example `InputRequired` control-flow trap (105-03):** the sketched loop "breaks" on `InputRequired`; if it then falls through to `tasks_result`, that is wrong for an input-required task. Require an explicit `saw_terminal` flag / early-return so `tasks_result` is only called after a `Terminal` decision.
2. **[MEDIUM] "Byte-identical" claim vs. what the tests assert (105-02):** the `input_required` regression pins only a message substring, and no direct test proves `tasks_result` is called only after a terminal decision (never on `InputRequired`). Either strengthen the assertions or downgrade the wording from "byte-identical" to "message-substring/semantically identical."
3. **[MEDIUM] Public-const API permanence (105-01):** exposing `DEFAULT_POLL_MS`/`MIN_POLL_MS` as `pub const` commits them to the long-term public contract. Decide deliberately: stable public policy (keep, maybe rename to `DEFAULT_TASK_POLL_INTERVAL_MS` / `MIN_TASK_POLL_INTERVAL_MS`) vs. documented defaults (keep private, expose only `resolve_poll_interval`).
4. **[LOW] Example may over-copy the duplex test harness (105-03):** risks bloat and exposing internal-style plumbing; extract only the smallest standalone server needed to produce a task.
5. **[LOW] Minor tightening:** import only the symbols `src/client/mod.rs` actually uses (likely just `MIN_POLL_MS` after deleting the inline default); prefer plain URL over intra-doc link for the source→book cross-link; run `make doc-check` at the doc task, not only phase end; consider deriving `Copy` on `TaskPollDecision`; grep-on-derive-line acceptance checks are brittle — compilation + doctests + focused tests are the real gate.

### Divergent Views
None — single reviewer this run.

### Overall Risk (Codex)
**LOW to MEDIUM.** The change is small and well constrained; the main risks are public-API permanence around the two constants and documentation/example precision (the `InputRequired` fall-through in particular).
