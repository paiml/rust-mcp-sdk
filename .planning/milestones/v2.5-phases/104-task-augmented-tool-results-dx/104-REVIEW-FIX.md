---
phase: 104-task-augmented-tool-results-dx
fixed_at: 2026-07-04T00:00:00Z
review_path: .planning/phases/104-task-augmented-tool-results-dx/104-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 104: Code Review Fix Report

**Fixed at:** 2026-07-04
**Source review:** .planning/phases/104-task-augmented-tool-results-dx/104-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5 (fix_scope: critical_warning — CR-01, WR-01, WR-02, WR-03, WR-04)
- Fixed: 5
- Skipped: 0

**Verification (per fix and final):** `cargo fmt --all -- --check` clean;
`cargo clippy --features full --lib --tests -- -D warnings` clean; all four
phase-104 suites pass (42 tests across `task_augmented_result`,
`tool_output_passthrough`, `double_wrap_tripwire`, `tool_with_result`), plus
the D-14 `tool_output_result_http` suite (1 test) and the `tool_with_result*`
doctests (2) as extra safety.

## Fixed Issues

### CR-01: `Client::wait_for_task` hangs forever on `input_required` tasks with default options

**Files modified:** `src/client/mod.rs`, `tests/task_augmented_result.rs`, `pmcp-book/src/task-augmented-results.md`
**Commit:** 688de942
**Applied fix:** Implemented the review's primary suggestion: the poll loop now
returns `Error::Validation` when the task enters `TaskStatus::InputRequired`
(that state is not terminal and needs client-side elicitation the poller cannot
provide). Rustdoc on `wait_for_task` gained an explicit Errors bullet, the
`WaitForTaskOptions::max_poll_duration_secs` doc no longer claims unconditional
"polling continues until terminal", and the book chapter (which recommends the
default unbounded options) gained a callout note. Added a live duplex
regression test that transitions a store task `Working -> InputRequired`
server-side and asserts the poller errors promptly (with an outer 10 s CI
safety timeout that would catch a re-introduced hang).

### WR-01: `wait_for_task` timeout can overshoot the budget by an unbounded factor

**Files modified:** `src/client/mod.rs`, `tests/task_augmented_result.rs`
**Commit:** 8983f648
**Applied fix:** Applied the suggested clamp: the effective interval is
computed first, then the budget is enforced at millisecond precision
(`saturating_mul`/`saturating_sub`, `as_millis` with `u64::try_from` guard) and
each sleep is clamped to the remaining budget, floored at the existing 50 ms
hot-spin clamp. `Error::timeout` still reports `max_secs * 1000` ms, keeping
the existing `Error::Timeout(_)` test matching intact. Regression test: a 60 s
poll interval with a 1 s budget must report the timeout near the budget
(asserted < 5 s wall clock), not after the first full interval.

### WR-02: Tripwire `ContentArray` marker false-positives on common LLM/chat-message payloads

**Files modified:** `src/server/task_dispatch.rs`, `tests/double_wrap_tripwire.rs`
**Commit:** ee2122b4
**Applied fix:** `looks_like_call_tool_result` now requires the object to be a
`CallToolResult` *envelope* — all keys must be in
`["content", "isError", "structuredContent", "_meta"]` — before the non-empty
all-`Content` content-array marker fires. Chat-message payloads
(`role`/`model`/`stopReason` + content array) no longer trip; a hand-built
double-wrap (authored to BE a `CallToolResult`) still trips, so recall for the
actual bug class is unchanged. The `_meta[related-task]` marker is untouched
(it is unambiguous regardless of foreign keys). Precision rationale in the
rustdoc updated; regression tests added for both sides (chat message must not
fire; full envelope with `isError`/`structuredContent` must fire). The
existing 6 behavior cases and the proptest precision fuzz still pass.
Incidentally resolves IN-07 (the inverted "(a)"/"(b)" comment labels were
dropped while rewriting that block).

### WR-03: Design doc contradicts implementation on create-path precedence for `ToolOutput::Result`

**Files modified:** `docs/design/sep-1686-task-augmented-results.md`, `tests/tool_output_passthrough.rs`
**Commit:** 137995ce
**Applied fix:** Parts (1) and (2) of the suggested fix. (1) §4 rewritten: the
"What is NOT bypassed" list no longer claims the create-path gate "keeps
precedence"; a new "What IS bypassed" paragraph states the `Verbatim` arm
returns before the gate ever executes in BOTH dispatchers and documents the
client-visible consequence (task-augmented call to a verbatim tool gets a
plain `CallToolResult`, no task minted, even under `TaskSupport::Required`).
(2) Added the missing regression test
(`task_augmented_call_to_verbatim_tool_returns_plain_result`): a
`VerbatimTaskRequiredTool` (declares `TaskSupport::Required`, overrides
`handle_output`) on a `ServerCore` WITH a task store, driven via
`call_tool_with_task` — asserts the response is `ToolCallResponse::Result`
with the verbatim envelope, panics if a task was minted. Part (3) — a
build-time/dispatch-time warn for the `TaskSupport::Required` + verbatim
conflict — was phrased as "Consider" in the review and is NOT implemented; the
conflict is now documented in the design doc instead. Flag for a follow-up if
desired.

### WR-04: `tool_with_result` cannot set a tool description

**Files modified:** `src/server/mod.rs`, `tests/tool_with_result.rs`
**Commit:** c73d6ee4
**Applied fix:** Added
`ServerBuilder::tool_with_result_and_description(name, description, handler)`
(the review's preferred option), mirroring `tool_typed_with_description`:
constructs `TypedToolWithResult::new(...).with_description(...)`. Purely
additive — `tool_with_result` is unchanged, so the 2.11 minor line stays
backward compatible. The new method's rustdoc repeats the D-04a verbatim-wire
bypass warning and carries a compiled `no_run` example; `tool_with_result`'s
rustdoc now cross-references the description variant. Test asserts the
description lands in `tools/list` AND the verbatim `ToolOutput::Result`
semantics (top-level `_meta[related-task]`, un-stringified content) are
preserved through the new overload.

## Skipped Issues

None — all in-scope findings were fixed.

## Notes

- Info findings (IN-01 … IN-07) were out of scope (`fix_scope: critical_warning`).
  IN-07 was incidentally resolved by the WR-02 rewrite of the same code block.
- Commits were made directly on `release/pmcp-v2.11.0` per orchestrator
  instruction. STATE.md / ROADMAP.md / 104-VERIFICATION.md untouched.
- Commit hashes: 688de942 (CR-01), 8983f648 (WR-01), ee2122b4 (WR-02),
  137995ce (WR-03), c73d6ee4 (WR-04).

---

_Fixed: 2026-07-04_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
