# Phase 105: Task poll-decision classifier and durable-consumer docs - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-05
**Phase:** 105-task-poll-decision-classifier-and-durable-consumer-docs
**Areas discussed:** Classifier API home, poll_hint semantics, Unpollable coverage, Docs & example shape

---

## Classifier API home

| Option | Description | Selected |
|--------|-------------|----------|
| Method on Task | `task.poll_decision() -> TaskPollDecision` in src/types/tasks.rs; most discoverable, matches `related_task()` accessor precedent, pure fn of Task | ✓ |
| Free fn in client mod | `pmcp::client::classify_task(&Task)` next to `wait_for_task`; less discoverable | |
| Associated fn on enum | `TaskPollDecision::classify(&Task)`; type-centric middle ground | |

**User's choice:** Method on Task (recommended option)
**Notes:** Preview showed the durable-consumer match shape (`Terminal` → fetch `tasks/result` as own memoized step, `InProgress` → durable wait, `InputRequired` → platform answer channel).

---

## poll_hint semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Raw hint + shared resolver | `poll_hint` = raw server pollInterval; second shared helper `resolve_poll_interval(caller_override, hint)` owns override→hint→1000ms default→50ms floor; budget clamp stays in `wait_for_task`; classifier stays pure/replay-deterministic | ✓ |
| Raw hint only | No resolver extracted; durable consumers re-derive resolution by hand (the complaint being fixed) | |
| Classifier resolves it | `poll_decision(opts)` returns resolved interval; Task method gains a client-options dependency | |

**User's choice:** Raw hint + shared resolver (recommended option)

---

## Unpollable coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Drop it | Three variants; every TaskStatus maps to exactly one; `#[non_exhaustive]` enum for future room; client-state errors stay typed errors on wait_for_task/tasks_get | ✓ |
| Keep it | Four variants per roadmap sketch; currently unreachable dead branch (exhaustive TaskStatus + serde rejects unknown statuses upstream) | |
| You decide | Claude picks during planning | |

**User's choice:** Drop it (recommended option)
**Notes:** Derived consequence recorded without asking: `InputRequired` becomes a unit variant — method-on-Task means the caller already holds the Task; carrying `{ task }` would be a redundant clone.

---

## Docs & example shape

| Option | Description | Selected |
|--------|-------------|----------|
| Light example + doc prose | Runnable examples/s48 plain classifier-driven poll loop (satisfies ALWAYS rule); durable/replay pattern as book prose + non-runnable snippets in existing Tasks chapter, cross-linked from 12.7 chapter and wait_for_task rustdoc | ✓ |
| Full replay-sim example | In-process fake durable runtime (event-history replay, memoized steps); high fidelity, meaningful harness cost | |
| Docs only, no example | Least effort; breaks house ALWAYS-example rule, needs documented exception | |

**User's choice:** Light example + doc prose (recommended option)

---

## Claude's Discretion

- Enum/helper naming polish (`TaskPollDecision`, `resolve_poll_interval` are working names)
- Where `resolve_poll_interval` lives (client vs types) — keep wasm boundary clean, callable by both wait_for_task and external consumers
- Test composition (unit + property mix) per house ALWAYS rules

## Deferred Ideas

- Ask B (task elicitation round-trip: `tasks/provide_input`, `task.input_request()`, `on_input_required` option) — deferred pending upstream spec standardization; `InputRequired` variant is the adoption seam
- Co-signing an upstream spec issue on polling-client input provision with pmcp.run's flow as the motivating example

---

## Review Fold — Codex (2026-07-05, `/gsd:review --codex` → `/gsd:discuss-phase 105 --reviews`)

Codex reviewed the design pre-planning; endorsed all locked decisions (risk LOW–MEDIUM). One genuine gray area surfaced and was decided; the rest folded as locked planner guidance.

| Review item | Severity | Disposition |
|-------------|----------|-------------|
| Resolver return type `u64` ms vs `Duration` | LOW | **User chose `u64` ms** (cross-API consistency) → D-12 |
| `wait_for_task` must structurally `match poll_decision()` | MEDIUM | Folded → D-13 |
| Replay-determinism claim must be scoped in docs | MEDIUM | Folded → D-14 |
| Semver honesty (`TaskStatus` exhaustive today) | MEDIUM | Folded → D-15 |
| Do-not-wrap warning + `Terminal` rustdoc | LOW | Folded → D-16 |
| Test matrix (status map, precedence, error pin, clamp) | — | Folded → Verification guidance |
| Re-export `resolve_poll_interval` near `TaskPollDecision` | LOW | Noted in Claude's Discretion |

**User's choice (resolver type):** u64 ms — symmetric with `Option<u64>` inputs and consistent with every existing public `poll_interval` field; caller wraps `Duration::from_millis` at the sleep site.
