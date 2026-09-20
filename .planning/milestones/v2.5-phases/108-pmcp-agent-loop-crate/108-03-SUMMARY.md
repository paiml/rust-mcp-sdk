---
phase: 108-pmcp-agent-loop-crate
plan: 03
subsystem: api
tags: [iteration-engine, pure-decisions, replay-safety, checkpoint-ordering, retry-as-data, proptest, fuzz]

# Dependency graph
requires:
  - phase: 108-pmcp-agent-loop-crate (plan 02)
    provides: "Three seams (CompletionSource/ToolInvoker/ConversationStore), RetryClass, ToolCall/ToolCallResult, RunState/RunPhase, InMemoryStore, ResolvedAgentConfig"
provides:
  - "iteration::decide — pure, replay-deterministic decision fns (is_end_turn, check_limits, classify_retry, extract_tool_calls, assistant_turn, extract_token_usage, digest_tool_results, evaluate_submit_result via jsonschema, parse_completion/parse_tool_result)"
  - "iteration::result — TurnMessage, IterationResult, RunOutcome{Completed,LimitReached,RetryRequired{class},Failed} (AGNT-02 retry-as-data), LimitDecision"
  - "iteration::engine::AgentEngine — thin async loop that loads/resumes state, checkpoints PendingTools BEFORE dispatch and final state before return, enforces limits from counters, returns RunOutcome"
  - "trace — EffectTrace (recorded effect results) + DecisionTrace (ordered decisions, PartialEq) + ReplaySource/ReplayInvoker replay seams"
  - "AGNT-03 replay-safety property (identical EffectTrace ⇒ identical DecisionTrace) + digestion/raw-parse fuzz + two golden fixtures"
affects: [plan-108-04-sources, plan-108-05-invoker-config, plan-108-06-adapter, AGNT-02, AGNT-03]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure-decisions-between-awaits: engine awaits ONLY load/create_message/invoke_batch/save; every between-await computation is a synchronous side-effect-free fn in decide.rs (no wall-clock, no RNG, counters only)"
    - "Classification-as-data: local exhaustive ErrorSignal enum → classify_retry → RetryClass surfaced via RunOutcome::RetryRequired (mirrors Task::poll_decision)"
    - "Crash-safe checkpoint ordering: save RunPhase::PendingTools BEFORE invoke_batch, save final state before returning; resume dispatches saved pending calls instead of re-running completion"
    - "Replay substrate: EffectTrace (inputs) + DecisionTrace (decisions) + ReplaySource/ReplayInvoker feed a recorded trace through the SAME engine for AGNT-03"

key-files:
  created:
    - crates/pmcp-agent/src/iteration/result.rs
    - crates/pmcp-agent/src/iteration/decide.rs
    - crates/pmcp-agent/src/iteration/engine.rs
    - crates/pmcp-agent/tests/replay_safety.rs
    - crates/pmcp-agent/tests/digest_fuzz.rs
    - crates/pmcp-agent/tests/fixtures/golden_trace_end_turn.json
    - crates/pmcp-agent/tests/fixtures/golden_trace_tool_loop.json
  modified:
    - crates/pmcp-agent/src/iteration/mod.rs
    - crates/pmcp-agent/src/trace.rs
    - crates/pmcp-agent/src/lib.rs

requirements: [AGNT-02, AGNT-03]

# Verification
verification:
  - "cargo test -p pmcp-agent -- --test-threads=1: 31 lib + 6 digest_fuzz + 2 object_safety + 3 replay_safety = 42 tests pass"
  - "cargo clippy -p pmcp-agent --all-targets --all-features: clean"
  - "cargo fmt -p pmcp-agent --check: clean"
  - "pmat analyze complexity --max-cognitive 25 on iteration/: no functions exceed cog 25"
  - "no std::time / Instant / SystemTime / rand in decide.rs or result.rs; fixtures contain no floats"
---

# Phase 108 Plan 03: Pure Decision Core + Thin Async Iteration Engine + Replay Substrate Summary

Built the heart of `pmcp-agent` (AGNT-02, AGNT-03): a pure, replay-deterministic decision
core (`iteration::decide`), a thin async `AgentEngine` that orchestrates the three seams with
crash-safe checkpoint ordering and returns retry classification as data, and the
`EffectTrace`/`DecisionTrace` replay substrate proven by an AGNT-03 replay-safety proptest
(comparing decision SEQUENCES) plus a two-target digestion fuzz.

## Tasks

1. **Pure decision fns + IterationResult/RunOutcome data types** (`f9453332`) — `result.rs`
   defines `TurnMessage`, `IterationResult`, `#[non_exhaustive] RunOutcome{Completed,
   LimitReached, RetryRequired{class}, Failed}` (retry-as-data, AGNT-02), and `LimitDecision`.
   `decide.rs` provides small, separate, PMAT-cog-≤25 pure functions: `is_end_turn`,
   `check_limits` (counter-based), `classify_retry` (exhaustive over a local `ErrorSignal`),
   `extract_tool_calls`, `assistant_turn`, `extract_token_usage`, `digest_tool_results`,
   `evaluate_submit_result` (real `jsonschema::validator_for`, no `stop_reason` param), and the
   `parse_completion`/`parse_tool_result` raw-parse boundary. No `std::time`/RNG. 16 unit tests.
2. **Thin async engine + EffectTrace/DecisionTrace substrate** (`99cf9e42`) — `engine.rs`'s
   `AgentEngine` loads state (`ConversationStore::load`), resumes a `PendingTools` checkpoint by
   dispatching the already-saved calls (no completion re-run, Codex HIGH #1), saves
   `RunPhase::PendingTools` BEFORE `invoke_batch` and the final state before returning (Codex
   HIGH #2), enforces limits from counters, and returns `RunOutcome` with retry class. It awaits
   ONLY the four seam calls; all logic delegates to `decide.rs`. `trace.rs` fills `EffectTrace`,
   `DecisionTrace` (+ `DecisionStep`/`OutcomeTag`, `PartialEq` for replay), and
   `ReplaySource`/`ReplayInvoker`. 8 engine + 3 trace unit tests.
3. **Replay-safety property + digestion fuzz + golden fixtures** (`1699220a`) —
   `tests/replay_safety.rs` (AGNT-03) generates consistent `EffectTrace`s, runs the engine twice
   via the replay seams, and asserts byte-identical `DecisionTrace`s (decision sequences, not
   just final results); two golden fixtures pin the end-turn and tool-loop outcomes.
   `tests/digest_fuzz.rs` has a STRUCTURAL proptest (arbitrary `Value` through digest/extract, no
   panic) and a RAW-BYTES proptest (arbitrary bytes through the parser boundary → `Err`, never a
   panic). 3 replay + 6 fuzz tests.

## Design Clarifications (not deviations)

- **Token accounting source.** `CreateMessageResultWithTools` carries no `usage` field (only
  `CreateMessageResult` does). To keep `check_limits(tokens_used, max_tokens)` from the plan
  functional and deterministic, `extract_token_usage` reads an optional advisory
  `_meta.usage.totalTokens` (or `total_tokens`), defaulting to `0`. This is a counter input, never
  a clock, and is fully deterministic for replay.
- **Retry-from-tool path.** The `ToolInvoker::invoke_batch` seam returns `Vec<ToolCallResult>`
  (per-call errors are DATA via `is_error`), not a `Result`, so a tool transport error cannot
  surface to the engine as a retryable failure — tool errors are folded into history so the model
  can react (correct agent-loop behavior). `RunOutcome::RetryRequired`/`Failed` therefore arise
  from `CompletionSource::create_message` errors, funneled through
  `ErrorSignal::from_completion` → `classify_retry`. `ErrorSignal::from_tool` is provided and unit
  tested for hosts/future seam use.
- **`TurnMessage` multi-block turn.** The SDK `SamplingMessage` holds a SINGLE content block, but
  a completion turn mixes text + `tool_use` blocks (and a tool-results turn folds several
  `tool_result` blocks). `TurnMessage{role, Vec<SamplingMessageContent>}` formalizes the turn; the
  engine flattens it into per-block `SamplingMessage`s for `RunState.history`.

## Deviations from Plan

None — plan executed as written (the items above are interface-driven clarifications, not scope
or behavior changes).

## Known Stubs

None. `ReplayInvoker::invoke` returns a trivial ok result only to satisfy the object-safe trait;
the engine always dispatches via `invoke_batch`, which reads recorded batches. This is documented
in-code and is not a data stub feeding the UI.

## Self-Check: PASSED

- Created files exist: `iteration/{result,decide,engine}.rs`, `tests/replay_safety.rs`,
  `tests/digest_fuzz.rs`, `tests/fixtures/golden_trace_{end_turn,tool_loop}.json` — all present.
- Commits exist: `f9453332` (Task 1), `99cf9e42` (Task 2), `1699220a` (Task 3).
- `cargo test -p pmcp-agent`: 42 tests pass; clippy `--all-targets --all-features` clean; fmt clean.
