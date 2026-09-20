# Phase 105: Task poll-decision classifier and durable-consumer docs - Context

**Gathered:** 2026-07-05
**Updated:** 2026-07-05 — folded Codex cross-AI review (`105-REVIEWS.md`): +D-12..D-16, verification guidance
**Status:** Ready for planning

<domain>
## Phase Boundary

A loop-free, public, client-side classifier for "what should a poller do with
this polled `Task`" — shared internally by `Client::wait_for_task` so the
blocking poller and durable/replay pollers cannot drift (D-05 single-decision
discipline) — plus the durable/replay-consumer documentation and a light
runnable example. Client/types layer only. Additive (next minor, 2.13.0-class).

**Scope fences (LOCKED, from ROADMAP):** no wire changes (`tasks/provide_input`
explicitly REJECTED as spec-invention — polling-client input provision is an
upstream spec gap; the classifier's `InputRequired` variant is the seam for
when the WG standardizes it); no new `TaskStatus` variants; no change to
`wait_for_task` blocking behavior or its `input_required` typed-error default
(the 2.12.0 CR-01 fix stays).

**Origin:** pmcp.run dev-team Ask A in
`~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-durable-task-consumer-and-input-required.md`
(Ask B deferred as spec-shaped; SDK response at
`~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-response-durable-task-consumer-and-input-required.md`).

</domain>

<decisions>
## Implementation Decisions

### Classifier API home
- **D-01:** The classifier is a **method on `Task`**:
  `Task::poll_decision(&self) -> TaskPollDecision`, living in
  `src/types/tasks.rs`. Rationale: most discoverable (autocomplete on the
  thing you just polled), matches the 2.12.0 `CallToolResult::related_task()`
  method-accessor precedent, and is naturally a **pure function of the polled
  `Task`** (replay-deterministic — safe inside a memoized durable step).
- **D-02:** `Client::wait_for_task` MUST consume `poll_decision()` internally
  for its terminal / input_required / in-progress branching — the classifier
  is not a parallel implementation, it IS the decision logic, factored out.
  A drift between the two is a defect (mirror Phase 104's D-05 parity
  discipline; add a test that pins `wait_for_task` behavior to the classifier).

### Variant set and shapes
- **D-03:** Three variants, not the four sketched in ROADMAP:
  `TaskPollDecision::Terminal { status } | InProgress { poll_hint } | InputRequired`.
  `Unpollable { reason }` is **dropped** — client-state failures
  (uninitialized, missing `tasks` capability) are not properties of a `Task`
  and stay as typed errors on `wait_for_task`/`tasks_get`; unknown future
  statuses are rejected upstream at `tasks/get` deserialization, so the
  variant is unreachable today and would force consumers to write a dead branch.
- **D-04:** `TaskPollDecision` is `#[non_exhaustive]` so a future variant
  (e.g., whatever the spec lands for input provision) is a non-breaking add.
- **D-05:** `InputRequired` is a **unit variant** (no `{ task }` payload) —
  the caller invoked `task.poll_decision()`, so they already hold the `Task`;
  carrying a clone is redundant.
- **D-06:** `Terminal { status }` carries the terminal `TaskStatus` only. The
  classifier does NOT fetch or carry the `CallToolResult` — the result comes
  from a separate `tasks/result` call the consumer owns (e.g., as its own
  memoized durable step). Document this explicitly.

### poll_hint semantics + shared resolver
- **D-07:** `InProgress { poll_hint }` carries the **raw server-reported
  `pollInterval`** (`Option<u64>`, ms) verbatim — the classifier stays a pure
  fn of `Task`.
- **D-08:** A **second shared helper** owns interval resolution:
  `resolve_poll_interval(caller_override: Option<u64>, hint: Option<u64>) -> u64`
  applying caller-override → server hint → 1000 ms default → 50 ms floor
  (the exact constants currently inline in `wait_for_task`,
  `src/client/mod.rs:685-716`). `wait_for_task` MUST use this helper too.
- **D-09:** Budget clamping (clamp sleep to remaining `max_poll_duration_secs`
  budget, WR-01 fix) **stays inside `wait_for_task`** — it is loop state, not
  task state. Not part of the classifier or resolver.
- **D-12 (review, 2026-07-05):** `resolve_poll_interval` returns **`u64`
  milliseconds**, not `Duration` — symmetric with its `Option<u64>` inputs and
  consistent with every existing public interval field (`TaskMetadata.poll_interval`,
  `WaitForTaskOptions.poll_interval`). Callers wrap with `Duration::from_millis`
  at the sleep site (one line, exactly what `wait_for_task` does today). Codex
  raised `Duration` as a LOW ergonomics option; rejected for cross-API
  consistency.

### Docs & example shape
- **D-10:** One **light runnable example** (`examples/s48_*`, next free
  s-number): a plain polling loop driven by `task.poll_decision()` +
  `resolve_poll_interval()` against an in-process server (reuse the
  duplex-transport harness pattern from `tests/task_augmented_result.rs`).
  Satisfies the house ALWAYS-example rule. Do NOT build a fake durable
  runtime / replay-simulation harness.
- **D-11:** The durable/replay pattern itself ships as **book prose +
  non-runnable snippets**: a "Durable and replay consumers" section in the
  existing Tasks chapter (`pmcp-book/src/ch12-7-tasks.md` area), covering the
  typed-accessors-without-the-loop pattern (Temporal-style `ctx.step`/`ctx.wait`),
  the replay-determinism caveat (typed deserialization inside a memoized step
  must be deterministic), and an explicit "when NOT to use `wait_for_task`"
  subsection. Cross-link from the task-augmented-results chapter
  (`pmcp-book/src/task-augmented-results.md`) and from the `wait_for_task`
  rustdoc.

### Review-driven locks (Codex, `105-REVIEWS.md`, 2026-07-05)
Codex endorsed every decision above (risk LOW–MEDIUM); its concerns are folded
here as LOCKED planner guidance, not open questions:

- **D-13 (no-drift is STRUCTURAL, not incidental):** `wait_for_task` MUST be
  rewritten as an explicit `match task.poll_decision() { Terminal => break,
  InputRequired => return Err(...), InProgress { poll_hint } => ... }` — calling
  the classifier "somewhere" is insufficient. The match arms ARE the loop's
  status handling; no parallel `is_terminal()` / status comparison may remain
  inline. This is the mechanism that makes D-02's parity real. A regression test
  MUST pin `wait_for_task`'s current `input_required`-error and terminal
  behavior byte-identical across the refactor.
- **D-14 (replay-determinism claim scoped precisely in docs):** `poll_decision()`
  is replay-deterministic ONLY as a pure function over an already-deserialized
  `Task`. The network `tasks/get` call AND the serde decode must sit INSIDE the
  durable runtime's memoized step — the docs (D-11) must state this explicitly,
  and note that an unknown/future status fails at deserialization BEFORE
  classification ever runs.
- **D-15 (semver honesty):** `TaskStatus` is exhaustive today. Docs and rustdoc
  must NOT imply unknown statuses are handled gracefully at runtime.
  `TaskPollDecision` is `#[non_exhaustive]` (D-04) so the SDK can ADD a variant
  later without a break — but that is a future-proofing affordance, not a
  present runtime capability. Keep the two claims distinct.
- **D-16 (doc sharpness):** the durable section (D-11) MUST carry an explicit
  "do NOT wrap `wait_for_task` inside a replay workflow" warning (it sleeps,
  loops, and owns the polling lifecycle — non-deterministic under replay). The
  `TaskPollDecision::Terminal` rustdoc MUST state that the caller still issues a
  separate `tasks/result` to retrieve the final `CallToolResult`.

### Verification guidance (from review — carry into plan `<verification>`)
Codex's requested test matrix (fold into the plan, not exhaustive of ALWAYS reqs):
- Exhaustive map: every current `TaskStatus` → its expected `TaskPollDecision`
  (property or table test; guards D-15's exhaustiveness claim)
- Resolver precedence: caller override beats server hint; hint beats 1000 ms
  default; zero/low interval floors to 50 ms
- `wait_for_task` `input_required` error behavior byte-identical post-refactor
  (D-13 regression pin)
- Budget clamp still prevents oversleep AND remains OUTSIDE the resolver (D-09)

### Claude's Discretion
- Exact enum/helper naming polish (`TaskPollDecision` and
  `resolve_poll_interval` are the working names from discussion — keep unless
  a strong codebase-consistency reason emerges during planning).
- Where `resolve_poll_interval` lives (client mod vs types) — pick whichever
  keeps the wasm boundary clean; it must be callable by both `wait_for_task`
  and external consumers. Codex suggests re-exporting it near `TaskPollDecision`
  so consumers who import task APIs find it — adopt if it doesn't cross the wasm
  boundary awkwardly.
- Test composition (unit + property mix) per house ALWAYS rules.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Origin request + SDK response (the contract this phase fulfills)
- `~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-issue-durable-task-consumer-and-input-required.md` — Ask A (classifier + durable docs), the durable-consumer shape (Temporal-style replay, out-of-band completion), and their reference implementation pointers
- `~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-response-durable-task-consumer-and-input-required.md` — what the SDK promised (classifier shape sketch, Ask B deferral rationale, non-goals)

### Cross-AI review (folded into decisions above)
- `.planning/phases/105-task-poll-decision-classifier-and-durable-consumer-docs/105-REVIEWS.md` — Codex pre-planning design review; concerns folded as D-13..D-16 + verification guidance

### Prior-phase decisions that constrain this one
- `.planning/phases/104-task-augmented-tool-results-dx/104-CONTEXT.md` — D-05 single-decision discipline, D-02 no-implicit-sniffing philosophy, D-04a middleware-bypass posture
- `docs/design/sep-1686-task-augmented-results.md` — the junction rationale; this phase's docs cross-link into its ecosystem

### Code this phase refactors/extends
- `src/client/mod.rs:680-736` — `wait_for_task` loop: the decision logic to extract (terminal check, InputRequired error, interval resolution constants DEFAULT_POLL_MS=1000/MIN_POLL_MS=50, WR-01 budget clamp that stays)
- `src/types/tasks.rs` — `Task`, `TaskStatus` (5 variants, `is_terminal()`, transition table), `TaskMetadata`; home of the new method + enum

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/task_augmented_result.rs` `live` module — in-process duplex-transport
  harness (client ↔ ServerCore pump) ideal for the s48 example and classifier
  integration tests
- `WaitForTaskOptions` (`from_metadata`/`or_from_metadata`) — the caller-override
  source feeding `resolve_poll_interval`
- `pmcp::testing::assert_roundtrips_through_client` — if any wire-shape
  assertion is needed (probably not; this phase is wire-neutral)

### Established Patterns
- Method-accessor precedent: `CallToolResult::related_task()` (2.12.0) — the
  discoverability pattern D-01 follows
- D-05 shared-decision-fn + parity-test pattern from Phase 104
  (`task_dispatch::resolve_tool_output` consumed by both dispatchers, with
  tests pinning both to it) — replicate for `wait_for_task` vs classifier
- House quality bar: `make quality-gate` AND `make doc-check` both green
  before push (doc-check is stricter than quality-gate on rustdoc links)

### Integration Points
- `wait_for_task` internals (`src/client/mod.rs`) — swap inline logic for
  classifier + resolver calls, behavior byte-identical (existing 11-test
  `task_augmented_result` suite is the regression net)
- `pmcp-book/src/SUMMARY.md` — register the new book section if it's a new page

</code_context>

<specifics>
## Specific Ideas

- The pmcp.run durable poller (`amplify/functions/durable-agent-lambda/src/mcp/client.rs`,
  `poll_task_durably`) is the real consumer to design for: every `tasks/get`
  is a memoized `ctx.step`, suspension is `ctx.wait`, completion happens
  out-of-band by a different actor. The book section's snippet should mirror
  that shape (without naming their internals).
- The SDK response note promised the classifier as "the seam" for future
  spec-standardized input provision — the `#[non_exhaustive]` enum (D-04) is
  what makes that promise cheap to keep.

</specifics>

<deferred>
## Deferred Ideas

- **Ask B — task elicitation round-trip** (`tasks/provide_input`, typed
  `task.input_request()` accessor, `on_input_required` option on
  `wait_for_task`): deferred pending upstream spec standardization of
  polling-client input provision. Revisit when the WG lands a mechanism;
  the `InputRequired` variant is the adoption seam. Offer stands to co-sign
  an upstream spec issue with pmcp.run's flow as the motivating example.
- **Upstream spec issue co-sign** — small, separate from SDK code; do when
  pmcp.run takes us up on it.

</deferred>

---

*Phase: 105-task-poll-decision-classifier-and-durable-consumer-docs*
*Context gathered: 2026-07-05*
