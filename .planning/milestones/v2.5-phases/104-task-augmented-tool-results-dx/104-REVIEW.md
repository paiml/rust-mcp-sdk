---
phase: 104-task-augmented-tool-results-dx
reviewed: 2026-07-04T00:00:00Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - docs/design/sep-1686-task-augmented-results.md
  - examples/s47_task_augmented_result.rs
  - pmcp-book/src/SUMMARY.md
  - pmcp-book/src/task-augmented-results.md
  - src/client/mod.rs
  - src/lib.rs
  - src/server/builder.rs
  - src/server/cancellation.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/task_dispatch.rs
  - src/server/typed_tool.rs
  - src/types/tasks.rs
  - src/types/tools.rs
  - tests/double_wrap_tripwire.rs
  - tests/task_augmented_result.rs
  - tests/tool_output_passthrough.rs
  - tests/tool_output_result_http.rs
  - tests/tool_with_result.rs
findings:
  critical: 1
  warning: 4
  info: 7
  total: 12
status: issues_found
---

# Phase 104: Code Review Report

**Reviewed:** 2026-07-04
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Narrative Findings (AI reviewer)

## Summary

Phase 104 adds the SEP-1686 task-augmented tool-result surface: `TaskMetadata` +
`CallToolResult::with_related_task`/`related_task`, the `ToolOutput` verbatim
pass-through (`handle_output`, `resolve_tool_output`, D-04a bypass), the TOUT-02
double-wrap tripwire with per-tool suppression, the `tool_with_result` /
`set_result_meta` sugar, the `Client::wait_for_task` poller, plus migration
docs, example s47, and five integration test suites.

The implementation is well-structured. The D-05 anti-drift design (single
`resolve_tool_output` decision function consumed by both dispatchers) is
verified correct in both `Server::handle_call_tool` (src/server/mod.rs) and
`ServerCore::handle_call_tool` (src/server/core.rs): request middleware fires
before the handler on every path, the response-middleware bypass is scoped to
the successful `Ok(ToolOutput::Result(_))` arm only, handler errors still route
through `handle_tool_error`, token cleanup in `Server` runs before the verbatim
early return, the tripwire runs before text-wrap and after the create-path
gate, and `merge_result_meta` runs after the tripwire (so `set_result_meta`
never false-trips it). cfg hygiene (`HashSet`, `Arc`, wasm gating of
`ToolOutput`/`task_dispatch`/`__test_support`) checks out; `web-time` and
`runtime::sleep` are genuinely wasm-safe. Poisoned-lock recovery in the
`result_meta` slot avoids handler-panic-poisons-dispatcher hazards.

However, the new client poller has one correctness defect (permanent hang on
`input_required` tasks — CR-01) and a timeout-budget overshoot (WR-01); the
tripwire's precision claim does not hold for a common payload class (WR-02);
and the design doc contradicts the implemented create-path/verbatim ordering
(WR-03).

## Critical Issues

### CR-01: `Client::wait_for_task` hangs forever on `input_required` tasks with default options

**File:** `src/client/mod.rs:685-704`
**Issue:** The loop breaks only on `task.status.is_terminal()`. `TaskStatus::InputRequired` is NOT terminal (`src/types/tasks.rs:523` asserts exactly that), and a task in `input_required` cannot progress without client-side action — which `wait_for_task` neither performs nor surfaces. With `WaitForTaskOptions::default()` (`max_poll_duration_secs: None`, the exact usage the book chapter recommends at `pmcp-book/src/task-augmented-results.md:56`), the poller loops forever: a guaranteed, silent hang under a protocol-legal server state that this SDK itself models (`Working -> InputRequired` is a valid transition per `src/types/tasks.rs:51`). Hand-rolled loops had this problem too, but this phase ships the loop as the blessed SDK API with an unbounded default.
**Fix:**
```rust
loop {
    let task = self.tasks_get(task_id).await?;
    if task.status.is_terminal() {
        break;
    }
    // input_required needs caller action this poller cannot provide —
    // surface it instead of spinning until (a possibly absent) timeout.
    if task.status == TaskStatus::InputRequired {
        return Err(Error::validation(format!(
            "task {task_id} is input_required; wait_for_task cannot provide input — \
             handle elicitation and resume polling"
        )));
    }
    // ... existing budget check + sleep ...
}
```
Alternatively (less breaking): add a `break_on_input_required: bool` (default `true`) to `WaitForTaskOptions`, or return the non-terminal `Task` in a richer result type. At minimum, the rustdoc "polling continues until the task is terminal" must call out the `input_required` deadlock.

## Warnings

### WR-01: `wait_for_task` timeout can overshoot the budget by an unbounded factor

**File:** `src/client/mod.rs:691-703`
**Issue:** The budget check (`start.elapsed().as_secs() >= max_secs`) runs before the sleep, but the sleep itself is not clamped to the remaining budget. The effective interval comes from `opts.poll_interval.or(task.poll_interval)` — i.e. the **server** controls it when the caller left it unset. With a server-reported `pollInterval` of 60000 ms and `max_poll_duration_secs: Some(5)`, the first check passes at t≈0, the client sleeps 60 s, and the timeout is reported at t≈60 s — 12x the caller's budget, growing without bound as the server's hint grows. A caller-specified budget should be honored to within one clamp floor, not one arbitrary server-chosen interval.
**Fix:** Clamp the sleep to the remaining budget:
```rust
let mut interval = opts.poll_interval.or(task.poll_interval)
    .unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS);
if let Some(max_secs) = opts.max_poll_duration_secs {
    let remaining_ms = max_secs
        .saturating_mul(1000)
        .saturating_sub(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX));
    if remaining_ms == 0 {
        return Err(Error::timeout(max_secs.saturating_mul(1000)));
    }
    interval = interval.min(remaining_ms.max(MIN_POLL_MS));
}
```
(Also switch the budget comparison to millisecond precision while here; `as_secs()` truncation makes a 1-second budget behave like ~1–2 s.)

### WR-02: Tripwire `ContentArray` marker false-positives on common LLM/chat-message payloads; `debug_assert!` aborts dispatch in debug builds

**File:** `src/server/task_dispatch.rs:242-252, 298-304`
**Issue:** `looks_like_call_tool_result` fires on ANY object carrying a non-empty `content` array of `Content`-parsable elements, regardless of what else the object contains. The "near-zero false positives" rationale (line 220) does not hold for a payload class that is ubiquitous in MCP tooling: Anthropic/OpenAI-style chat messages — `{"role": "assistant", "content": [{"type": "text", "text": "..."}]}` — and any tool that proxies sampling/LLM APIs returns exactly this shape. In debug/CI builds the `debug_assert!(false)` at line 298 then panics mid-dispatch, aborting the request-handling task for a perfectly legitimate payload. D-02/D-06/D-08 are locked decisions and the suppression opt-out exists, but the detector's precision can be raised without weakening recall for the actual bug class.
**Fix:** Require the object to look like a result *envelope*, not merely contain a content array — e.g. reject when foreign keys are present:
```rust
// Only CallToolResult envelope keys may accompany the content array.
const RESULT_KEYS: [&str; 4] = ["content", "isError", "structuredContent", "_meta"];
if obj.keys().all(|k| RESULT_KEYS.contains(&k.as_str())) && /* existing array check */ { ... }
```
A hand-built double-wrap always has this shape (it was authored to BE a `CallToolResult`), while a chat message carries `role`/`model`/`stopReason` etc. and would no longer trip.

### WR-03: Design doc contradicts implementation on create-path precedence for `ToolOutput::Result`; verbatim + task-augmented request is untested

**File:** `docs/design/sep-1686-task-augmented-results.md:203-206` (vs `src/server/mod.rs` Verbatim arm, `src/server/core.rs:565-567`)
**Issue:** §4 claims: "What is NOT bypassed: … the Phase 102 task create-path gate keeps precedence (a `Result` output is not task-shaped, so the gate naturally passes)". This is false: in BOTH dispatchers the `DispatchOutput::Verbatim` arm returns before the create-path gate ever executes — the gate is structurally bypassed, never consulted (the code comments in both dispatchers and the `ToolOutput::Result` rustdoc say exactly this: "the create-path gate … ALL bypassed"). Consequence the doc obscures: a client sending a task-augmented `tools/call` to a `tool_with_result` / `handle_output`-overriding tool silently receives a plain `CallToolResult` and no task is minted — even if the tool declares `TaskSupport::Required`. No test in `tool_output_passthrough.rs` covers the verbatim-tool + `call_tool_with_task` combination (the create-path precedence test at line 310 uses a Payload tool only).
**Fix:** (1) Correct §4 to state the gate is bypassed on the verbatim path and document the client-visible consequence for task-augmented requests. (2) Add a regression test: task-augmented call to a `ToolOutput::Result` tool → assert the response shape (plain `CallToolResult`) is the intended, documented behavior. (3) Consider a build-time or dispatch-time warn when a `TaskSupport::Required` tool overrides `handle_output` to return `Result`, since the two contracts conflict.

### WR-04: `tool_with_result` cannot set a tool description — the flagship migration sugar registers description-less tools

**File:** `src/server/mod.rs:2826-2911` (`tool_with_result`), `src/server/typed_tool.rs:909-921`
**Issue:** `ServerBuilder::tool_with_result` constructs `TypedToolWithResult::new(...)` and immediately inserts it; `TypedToolWithResult::with_description` exists but is unreachable through the sugar, so `metadata()` returns `description: None` and `tools/list` advertises the tool with no description. Every sibling sugar (`tool_typed_with_description`, `tool_typed_with_output` + description overload) offers a description path. Since the migration guide and the book steer users to `tool_with_result` as THE pattern, the default outcome is degraded LLM tool selection.
**Fix:** Add a `tool_with_result_and_description(name, description, handler)` overload (mirroring `tool_typed_with_description`), or have `tool_with_result` accept `impl Into<ToolRegistration>`; document constructing `TypedToolWithResult::new(...).with_description(...)` + `.tool(...)` as the workaround meanwhile.

## Info

### IN-01: Stray closing code fence at end of design doc

**File:** `docs/design/sep-1686-task-augmented-results.md:222`
**Issue:** The file's last line is a bare ``` ``` ``` with no matching opening fence; renderers will treat it as opening an unterminated code block or display a stray fence.
**Fix:** Delete the trailing fence line.

### IN-02: `wait_for_task` rustdoc example demonstrates `wait_for_related_task`

**File:** `src/client/mod.rs:658-669`
**Issue:** The `# Example` block on `wait_for_task` calls `client.wait_for_related_task(&meta, ...)` — the sibling method — so the method's own doc never shows its own call shape (`ignore`d, so never compiled/caught).
**Fix:** Change the example to `client.wait_for_task(&meta.task_id, WaitForTaskOptions::from_metadata(&meta))` or move the snippet to `wait_for_related_task`.

### IN-03: Non-empty `set_result_meta` slot is silently dropped on the verbatim, create-path, and error arms

**File:** `src/server/mod.rs` (verbatim/create-path early returns), `src/server/core.rs:565-567, 649-658`
**Issue:** All early-return arms drop `result_meta_handle` undrained. The Result-path drop is documented; the create-path drop is not user-visible anywhere (a Pattern-C handler on a task-capable tool loses its `_meta` on task-augmented calls with zero signal), and middleware that stamps audit keys via `set_result_meta` in `on_request` loses them on every verbatim tool.
**Fix:** On the early-return arms, `if result_meta_handle.take_result_meta().is_some() { tracing::debug!(tool = %req.name, "handler-set result _meta ignored on non-Payload path"); }` — one line per arm, keeps the contract, removes the silence.

### IN-04: Global panic-hook swap in debug tripwire test can swallow diagnostics of concurrently failing tests

**File:** `tests/double_wrap_tripwire.rs:169-179`
**Issue:** `debug_panics_on_tripping_unsuppressed` replaces the process-wide panic hook with a silent one for the duration of `catch_unwind`. Under default parallel test execution, any other test in this binary that fails during that window has its panic message suppressed (the failure is still recorded, but with no output). CI's `--test-threads=1` masks this; local runs don't.
**Fix:** Use `std::panic::catch_unwind` without hook replacement and tolerate the one-line expected panic output, or serialize via a shared `Mutex` guard around hook manipulation.

### IN-05: Example's "resolvable via tasks/get" claim doesn't hold for its own client (owner mismatch)

**File:** `examples/s47_task_augmented_result.rs:87-93`
**Issue:** The store task is minted with owner `"s47-owner"`, but the anonymous HTTP client would resolve a different owner (IDOR mitigation, T-102-01), so the doc-comment claim that "`tasks/get`/`tasks/result` would resolve it" is untrue for the example's own client. The example never polls, so it runs green — but a reader extending it with `wait_for_related_task` (the natural next step) hits an owner-mismatch error.
**Fix:** Mint with the owner the anonymous HTTP session resolves to, or adjust the comment to state that the id is resolvable *by its owner* and that this demo's client is not that owner.

### IN-06: Design doc hardcodes source line numbers that are already stale

**File:** `docs/design/sep-1686-task-augmented-results.md:153, 165-166`
**Issue:** References like "`src/client/mod.rs`, `parse_task_payload`, ~lines 629-640" are already wrong within this very phase — lines 632-707 of that file are now `wait_for_task`/`wait_for_related_task`. Line-number references in long-lived design docs rot immediately.
**Fix:** Reference symbols/test names only (e.g. "`parse_task_payload` in `src/client/mod.rs`"), drop the line ranges.

### IN-07: Check-ordering comment labels reversed in `looks_like_call_tool_result`

**File:** `src/server/task_dispatch.rs:234, 242`
**Issue:** The first (meta-key) check is labeled "(b)" and the second (content-array) check "(a)", inverted relative to reading order; the doc rationale above lists them without labels, so the letters map to nothing.
**Fix:** Renumber the inline comments or drop the letters.

---

_Reviewed: 2026-07-04_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
