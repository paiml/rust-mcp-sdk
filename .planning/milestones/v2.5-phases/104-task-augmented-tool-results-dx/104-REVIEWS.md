---
phase: 104
reviewers: [codex]
reviewed_at: 2026-07-04T21:01:56Z
plans_reviewed: [104-01-PLAN.md, 104-02-PLAN.md, 104-03-PLAN.md, 104-04-PLAN.md, 104-05-PLAN.md]
---

# Cross-AI Plan Review — Phase 104

## Codex Review

## Summary

The plan set is strong overall: it decomposes Phase 104 cleanly across client API, dispatch semantics, tripwire safety, ergonomics, and documentation/acceptance gates. The strongest parts are the explicit rejection of implicit `CallToolResult` sniffing, the live HTTP acceptance test, and the focus on keeping `Server`/`ServerCore` behavior aligned. Main risks are around Plan 02’s response-middleware bypass semantics, Plan 01’s `wait_for_task` API not fully honoring `TaskMetadata` despite the requirement, and Plan 04’s `RequestHandlerExtra` interior-mutability plumbing adding subtle async/concurrency surface.

## Plan 104-01

**Strengths**
- Clean additive type model: `TaskMetadata` distinct from existing minimal `RelatedTaskMetadata`.
- Good minimal-shape compatibility with native `{taskId}` emission.
- Correctly uses existing `crate::runtime::sleep`, avoiding a new wasm timer dependency.
- Includes timeout behavior, which is essential to avoid unbounded polling.

**Concerns**
- **MEDIUM:** `wait_for_task(task_id, opts)` cannot directly “honor `TaskMetadata`” from `related_task()` unless callers manually copy fields into `WaitForTaskOptions`. The phase decision says `TaskMetadata` poll fields should be honored.
- **MEDIUM:** Timeout based on elapsed wall time needs to use a wasm-safe clock too. The plan names sleep but not the elapsed-time source.
- **LOW:** `assert_capability("tasks", "tasks/get")` then later `tasks_result()` also asserts capability. That is fine, but duplicate capability checks should not produce inconsistent method names/errors.
- **LOW:** Poll interval unit should be documented clearly as milliseconds, since `pollInterval` can otherwise be misread as seconds.

**Suggestions**
- Add `WaitForTaskOptions::from_metadata(&TaskMetadata)` or `Client::wait_for_related_task(meta, opts)` so the accessor and poller compose naturally.
- Use an existing platform-safe time source for timeout tracking and test wasm compile.
- Add a zero/very-small poll interval test or clamp behavior to avoid hot loops.

**Risk Assessment: LOW-MEDIUM**

Mostly straightforward client/API work. Risk is API ergonomics and timeout correctness, not architecture.

## Plan 104-02

**Strengths**
- Correctly makes `ToolOutput` additive via a default `handle_output()`.
- Explicitly preserves the Payload create-path gate.
- Good attention to drift between `Server` and `ServerCore`.
- Keeps workflow-internal `handle() -> Value` paths out of scope, which avoids accidental envelope flattening.

**Concerns**
- **HIGH:** The plan treats `ToolOutput::Result` bypassing response middleware as “locked,” but the original decisions only explicitly locked no text-wrap/no widget enrichment. Bypassing redaction/sanitization middleware is security-relevant and may surprise existing users.
- **HIGH:** If middleware is used for audit, policy enforcement, or output redaction, `tool_with_result` becomes an escape path. The plan documents it, but documentation may not be enough.
- **MEDIUM:** “Shared helper resolves ToolOutput into verbatim vs Payload” is good, but the plan also says `process_response` is bypassed before coercion. The exact control flow could get messy and deserves a small, carefully named helper with tests.
- **MEDIUM:** ServerCore already has a duplicated create-path gate and the plan leaves it out of scope. That is pragmatic, but it weakens the “shared seam” claim.

**Suggestions**
- Reclassify middleware bypass as a decision needing explicit approval, or add a minimal `process_call_tool_result` hook for `CallToolResult`.
- If bypass remains, make it highly visible in rustdoc for `ToolOutput::Result` and `tool_with_result`, not only migration docs.
- Add a test proving request middleware still runs before `ToolOutput::Result`.
- Add a regression test that handler errors from `handle_output()` still go through existing error handling.

**Risk Assessment: MEDIUM-HIGH**

The core mechanism is sound, but response-middleware bypass is the biggest architectural/security risk in the whole phase.

## Plan 104-03

**Strengths**
- Detector uses high-precision markers instead of unsafe full deserialization.
- Good false-positive controls: exact `_meta` key, valid `Content`, non-empty array.
- Includes release-mode verification that `debug_assert!` does not panic.
- Per-tool suppression is appropriate and reviewable.

**Concerns**
- **MEDIUM:** Capturing/asserting `tracing::warn!` can be brittle unless the repo already has a stable tracing test pattern.
- **MEDIUM:** A debug-asserting integration test can be awkward because debug assertions abort the specific call path. It needs `#[should_panic]` or isolated helper-level testing.
- **LOW:** `serde_json::from_value::<Content>(e.clone())` clones every content item. Usually fine, but large arrays could make the tripwire moderately expensive.
- **LOW:** Suppression by tool name needs consistent behavior across aliases/registration variants.

**Suggestions**
- Test `looks_like_call_tool_result` directly for debug panic behavior separately from full dispatch.
- Consider a borrowed-deserialize path if available, though not worth major complexity.
- Ensure suppress configuration is included in any builder clone/ServerCore conversion path.
- Add rustdoc or builder docs saying suppression should be rare and reviewed.

**Risk Assessment: MEDIUM**

The logic is well-scoped. Main risk is test brittleness and suppression plumbing drift.

## Plan 104-04

**Strengths**
- `tool_with_result` is the right ergonomic layer and mirrors existing typed-tool APIs.
- Correctly identifies the moved-by-value `extra` problem.
- Merge semantics for `set_result_meta` are explicit.
- Keeps `set_result_meta` Payload-only, avoiding conflict with handler-owned `ToolOutput::Result`.

**Concerns**
- **MEDIUM:** `Arc<Mutex<Option<Map>>>` in `RequestHandlerExtra` is workable but subtle. Mutex choice matters in async code; standard `std::sync::Mutex` is fine only if never held across await.
- **MEDIUM:** Merge conflict behavior is underspecified. “Merges rather than dropping keys” does not say whether handler-set keys override widget/native keys or vice versa.
- **MEDIUM:** `tool_with_result<TIn>` appears to require `JsonSchema`, so it may need feature gating consistent with existing typed helpers. The plan mentions this, but it should be explicit in acceptance.
- **LOW:** No test is mentioned for repeated `set_result_meta` calls merging into the same slot.

**Suggestions**
- Specify collision behavior: preferably `set_result_meta` overwrites only same keys it sets, while preserving existing unrelated `_meta`.
- Add tests for repeated calls and collision with widget/native `_meta`.
- Keep slot access behind small methods like `result_meta_slot()` / `take_result_meta()` so dispatch code does not know lock details.
- Add rustdoc warning that `set_result_meta` is ignored on `ToolOutput::Result`.

**Risk Assessment: MEDIUM**

Useful ergonomics, but the interior-mutable slot is easy to get subtly wrong. Tests should be very targeted.

## Plan 104-05

**Strengths**
- Live HTTP acceptance gate is exactly the right validation for the historical failure mode.
- Good reliability choices: ephemeral port, no fixed readiness sleep, abort shutdown.
- Runnable `s47` example doubles as migration documentation.
- Docs explicitly capture the native `_meta[related-task]` compatibility proof.

**Concerns**
- **MEDIUM:** The BEFORE example intentionally demonstrating a bad/wrapped shape can trip the new debug assertion from Plan 03 unless suppressed or isolated.
- **MEDIUM:** The example says “native `with_task_store()` + `ToolOutput`,” but `ToolOutput::Result` with a hardcoded task id is only a related-task hint unless an actual task exists in the store.
- **LOW:** README/book/doc updates can easily drift if the exact public API names change during implementation.
- **LOW:** `make doc-check` may catch rustdoc/book links, but not necessarily mdBook rendering unless already wired.

**Suggestions**
- Suppress the tripwire for the BEFORE demonstration or avoid executing the intentionally bad path in debug mode.
- Make the AFTER example create or reference a real store-minted task where feasible, so the migration story does not teach hand-minted related-task ids.
- Add a final grep/test that `s47` is included in any examples index if the repo maintains one.
- In docs, explicitly call out middleware bypass semantics if Plan 02 keeps that behavior.

**Risk Assessment: MEDIUM**

The acceptance strategy is excellent. Main risk is the example accidentally conflicting with the tripwire or teaching a half-native pattern.

## Overall Risk Assessment

**Overall risk: MEDIUM**

The phase goals are achievable and the plans are unusually thorough. The largest unresolved design risk is whether `ToolOutput::Result` should bypass response middleware. That decision affects security expectations and should be explicitly approved or mitigated with a result-aware middleware hook. The second risk is API coherence between `TaskMetadata`, `related_task()`, and `wait_for_task`; add a direct composition path so users do not fall back to hand-rolled polling. The test strategy is strong enough to catch the original bug class if the middleware and example concerns are tightened.

---

## Consensus Summary

Single external reviewer (Codex, codex-cli 0.142.5). Consensus is drawn between Codex and the internal gsd-plan-checker findings from the planning verification loop.

### Agreed Strengths
- Explicit rejection of implicit `CallToolResult` sniffing (D-02) — both Codex and the plan checker call this the right call.
- Live HTTP acceptance gate (Plan 05) targets the exact historical failure mode (stringified `_meta` over the wire).
- Additive `ToolOutput`/`handle_output` design keeps existing handlers untouched; `Server`/`ServerCore` anti-drift parity testing.
- Reuse of `crate::runtime::sleep` avoids a new wasm timer dependency.

### Agreed Concerns
- **Response-middleware bypass for `ToolOutput::Result` (Plan 02) — HIGH.** The plan checker flagged the original compile-incompatibility and required the bypass be made an explicit locked decision with a regression test; Codex goes further: the bypass itself is security-relevant (redaction/audit escape path) and was NOT explicitly locked in the original CONTEXT.md decisions (only no-text-wrap/no-widget-enrichment were). Codex recommends explicit user approval, louder rustdoc on `ToolOutput::Result`/`tool_with_result`, a request-middleware-still-runs test, and a handler-error-path regression test — or alternatively a minimal result-aware `process_call_tool_result` hook.
- **`set_result_meta` interior-mutability slot (Plan 04) — MEDIUM.** Both reviews call this the subtlest mechanism: Mutex discipline (never hold across await), underspecified merge/collision semantics, repeated-call behavior untested.
- **ServerCore's duplicated create-path gate stays out of scope — MEDIUM.** Both note it weakens the "single shared seam" claim; pragmatic but should stay visible.

### Divergent Views / Codex-Only Findings
- **`wait_for_task` should compose directly with `TaskMetadata`** (Plan 01, MEDIUM): callers shouldn't hand-copy poll fields into `WaitForTaskOptions`; also the elapsed-time source must be wasm-safe, not just the sleep.
- **Tripwire test brittleness** (Plan 03, MEDIUM): capturing `tracing::warn!` and `debug_assert` abort behavior need isolated helper-level tests, not full-dispatch assertions.
- **s47 BEFORE example may trip the Plan 03 debug assertion** (Plan 05, MEDIUM): suppress the tripwire for the intentional bad-shape demo, and make the AFTER example use a real store-minted task instead of a hand-minted task id.

**Overall Codex risk: MEDIUM** (Plan 02 middleware bypass rated MEDIUM-HIGH).
