# Phase 105: Task poll-decision classifier and durable-consumer docs - Research

**Researched:** 2026-07-05
**Domain:** Rust MCP SDK client-side task polling — pure-function refactor + rustdoc/mdBook docs
**Confidence:** HIGH (all claims verified against the 2.12.0 source in this session)

## Summary

This phase is a **wire-neutral, additive refactor** of logic that already exists inline
inside `Client::wait_for_task` (`src/client/mod.rs:680-736`), plus a documentation deliverable.
There is no new protocol, no new dependency, and no behavior change. The entire technical
surface — `Task`, `TaskStatus`, `wait_for_task`, `WaitForTaskOptions`, the duplex test
harness, the mdBook chapters — was verified to exist exactly as CONTEXT.md describes.
`[VERIFIED: codebase grep + Read]`

The work is: (1) add a pure method `Task::poll_decision(&self) -> TaskPollDecision` and a
free helper `resolve_poll_interval(caller, hint) -> u64` to `src/types/tasks.rs`; (2) rewrite
`wait_for_task`'s loop to structurally `match task.poll_decision()` and call the resolver, so
the two poller shapes cannot drift (D-02/D-13); (3) write a "durable/replay consumer" doc
section (rustdoc + `pmcp-book/src/ch12-7-tasks.md`) and one runnable example (`examples/s48_*`).
The existing 11-test `tests/task_augmented_result.rs` suite is the pre-built regression net
that pins byte-identical `wait_for_task` behavior across the refactor. `[VERIFIED]`

**Primary recommendation:** Put both `TaskPollDecision` (the `#[non_exhaustive]` enum) and
`Task::poll_decision()` **and** the free `resolve_poll_interval()` in `src/types/tasks.rs`.
The types module is compiled for both wasm and non-wasm with no `cfg` gate, so this is the
one home that keeps the wasm boundary clean, satisfies Codex's "re-export near
`TaskPollDecision`" suggestion for free (they're already co-located), and is callable by both
`wait_for_task` and external durable consumers. Then rewrite `wait_for_task` as an explicit
three-arm match. `[VERIFIED: src/types/mod.rs has no cfg gate; src/client/mod.rs:134 impl block is not wasm-gated]`

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Classifier API home**
- **D-01:** Classifier is a **method on `Task`**: `Task::poll_decision(&self) -> TaskPollDecision`, in `src/types/tasks.rs`. Rationale: discoverable (autocomplete on the polled thing), matches the 2.12.0 `CallToolResult::related_task()` method-accessor precedent, pure function of the polled `Task` (replay-deterministic).
- **D-02:** `Client::wait_for_task` MUST consume `poll_decision()` internally for its terminal / input_required / in-progress branching — the classifier IS the decision logic, factored out. Drift is a defect; add a test pinning `wait_for_task` to the classifier.

**Variant set and shapes**
- **D-03:** Three variants, not four: `TaskPollDecision::Terminal { status } | InProgress { poll_hint } | InputRequired`. `Unpollable { reason }` is **dropped** — client-state failures stay as typed errors on `wait_for_task`/`tasks_get`; unknown future statuses are rejected at `tasks/get` deserialization, so the variant is unreachable and would force a dead branch.
- **D-04:** `TaskPollDecision` is `#[non_exhaustive]` so a future variant is a non-breaking add.
- **D-05:** `InputRequired` is a **unit variant** (no `{ task }` payload) — caller already holds the `Task`.
- **D-06:** `Terminal { status }` carries the terminal `TaskStatus` only. The classifier does NOT fetch or carry the `CallToolResult` — the result comes from a separate `tasks/result` call the consumer owns. Document explicitly.

**poll_hint semantics + shared resolver**
- **D-07:** `InProgress { poll_hint }` carries the **raw server-reported `pollInterval`** (`Option<u64>`, ms) verbatim — classifier stays a pure fn of `Task`.
- **D-08:** A **second shared helper** owns interval resolution: `resolve_poll_interval(caller_override: Option<u64>, hint: Option<u64>) -> u64` applying caller-override → server hint → 1000 ms default → 50 ms floor (exact constants inline in `wait_for_task`, `src/client/mod.rs:685-716`). `wait_for_task` MUST use this helper too.
- **D-09:** Budget clamping (clamp sleep to remaining `max_poll_duration_secs`, WR-01) **stays inside `wait_for_task`** — loop state, not task state. Not part of classifier or resolver.
- **D-12:** `resolve_poll_interval` returns **`u64` milliseconds**, not `Duration` — symmetric with its `Option<u64>` inputs and consistent with every existing public interval field. Callers wrap with `Duration::from_millis` at the sleep site.

**Docs & example shape**
- **D-10:** One **light runnable example** (`examples/s48_*`, next free s-number): a plain polling loop driven by `task.poll_decision()` + `resolve_poll_interval()` against an in-process server (reuse the duplex-transport harness from `tests/task_augmented_result.rs`). Satisfies the ALWAYS-example rule. Do NOT build a fake durable runtime / replay-simulation harness.
- **D-11:** The durable/replay pattern ships as **book prose + non-runnable snippets**: a "Durable and replay consumers" section in the existing Tasks chapter (`pmcp-book/src/ch12-7-tasks.md`), covering the typed-accessors-without-the-loop pattern (Temporal-style `ctx.step`/`ctx.wait`), the replay-determinism caveat, and an explicit "when NOT to use `wait_for_task`" subsection. Cross-link from `pmcp-book/src/task-augmented-results.md` and from the `wait_for_task` rustdoc.

**Review-driven locks (Codex)**
- **D-13 (no-drift is STRUCTURAL):** `wait_for_task` MUST be rewritten as an explicit `match task.poll_decision() { Terminal => break, InputRequired => return Err(...), InProgress { poll_hint } => ... }`. No parallel `is_terminal()` / status comparison may remain inline. A regression test MUST pin `wait_for_task`'s current `input_required`-error and terminal behavior byte-identical across the refactor.
- **D-14 (replay-determinism scoped in docs):** `poll_decision()` is replay-deterministic ONLY as a pure function over an already-deserialized `Task`. The network `tasks/get` call AND the serde decode must sit INSIDE the durable runtime's memoized step — docs must state this, and note that an unknown/future status fails at deserialization BEFORE classification runs.
- **D-15 (semver honesty):** `TaskStatus` is exhaustive today. Docs must NOT imply unknown statuses are handled gracefully at runtime. `TaskPollDecision` is `#[non_exhaustive]` (future-proofing affordance, not present runtime capability). Keep the two claims distinct.
- **D-16 (doc sharpness):** the durable section MUST carry an explicit "do NOT wrap `wait_for_task` inside a replay workflow" warning. The `TaskPollDecision::Terminal` rustdoc MUST state the caller still issues a separate `tasks/result` to retrieve the final `CallToolResult`.

### Claude's Discretion
- Exact enum/helper naming polish (`TaskPollDecision` and `resolve_poll_interval` are working names — keep unless a strong codebase-consistency reason emerges).
- Where `resolve_poll_interval` lives (client mod vs types) — pick whichever keeps the wasm boundary clean; must be callable by both `wait_for_task` and external consumers. Codex suggests re-exporting near `TaskPollDecision`.
- Test composition (unit + property mix) per house ALWAYS rules.

### Deferred Ideas (OUT OF SCOPE)
- **Ask B — task elicitation round-trip** (`tasks/provide_input`, typed `task.input_request()` accessor, `on_input_required` option on `wait_for_task`): deferred pending upstream spec standardization. The `InputRequired` variant is the adoption seam.
- **Upstream spec issue co-sign** — small, separate from SDK code; do when pmcp.run takes us up on it.
</user_constraints>

<phase_requirements>
## Phase Requirements

Per the phase description, **no requirement IDs are mapped** to this phase ("none mapped
(TBD in roadmap)"). The phase is driven entirely by the LOCKED decisions D-01..D-16 above,
which serve as the requirement set. The planner should treat each `D-NN` as a
requirement to satisfy and map plan tasks to them.

| ID | Description | Research Support |
|----|-------------|------------------|
| D-01 | `Task::poll_decision()` method in `src/types/tasks.rs` | Method-accessor precedent verified: `CallToolResult::related_task()` at `src/types/tools.rs:661`; `Task` at `src/types/tasks.rs:91` |
| D-02/D-13 | `wait_for_task` structurally matches `poll_decision()` | Current inline logic at `src/client/mod.rs:695-733` fully mapped below |
| D-03/D-04/D-05/D-06 | Three-variant `#[non_exhaustive]` enum, `InputRequired` unit, `Terminal { status }` no result | `TaskStatus` 5-variant enum verified `src/types/tasks.rs:11-26`; `is_terminal()` at :44 |
| D-07/D-08/D-12 | `resolve_poll_interval(Option<u64>, Option<u64>) -> u64` | Exact constants `DEFAULT_POLL_MS=1000`/`MIN_POLL_MS=50` at `src/client/mod.rs:686-688`; resolution expression at :712-716 |
| D-09 | Budget clamp stays in `wait_for_task` | WR-01 clamp at `src/client/mod.rs:723-731` |
| D-10 | Runnable example `examples/s48_*` | Next free s-number confirmed s48 (s47 is last); harness at `tests/task_augmented_result.rs:120` |
| D-11/D-14/D-16 | Book section + cross-links | `pmcp-book/src/ch12-7-tasks.md` (has "The Polling Model" §132, "Task Status State Machine" §516); `task-augmented-results.md`; SUMMARY.md:34-35 |
| D-15 | Semver-honest docs | `TaskStatus` is NOT `#[non_exhaustive]` — verified exhaustive at `src/types/tasks.rs:12` |
</phase_requirements>

## Architectural Responsibility Map

This is an SDK/library, so "tiers" are crate layers rather than deployment tiers.

| Capability | Primary Layer | Secondary Layer | Rationale |
|------------|--------------|-----------------|-----------|
| Pure task-state classification (`poll_decision`) | **Types layer** (`src/types/tasks.rs`) | — | Pure fn of `Task`; no I/O, no client state; compiled for wasm + native with no cfg gate |
| Stateless interval policy (`resolve_poll_interval`) | **Types layer** (`src/types/tasks.rs`) | — | Takes plain `Option<u64>` args, no dependency on client types; co-locating with the enum satisfies Codex's re-export suggestion for free |
| Blocking poll loop + budget/timeout state | **Client layer** (`src/client/mod.rs` `wait_for_task`) | Types layer (consumes classifier + resolver) | Budget clamp is loop state (D-09); owns `tasks/get`/`tasks/result` I/O and the `input_required` typed-error default |
| Durable/replay consumption pattern | **Docs** (rustdoc + mdBook) + example | Types layer (the primitive consumed) | Pattern is prose + snippets (D-11); the SDK ships the primitive, not a durable runtime |

**Why this matters:** The one design decision with teeth is the home of `resolve_poll_interval`
(Claude's Discretion in D-08). Putting it in the **types layer** (not the client layer) is the
recommendation: it removes any risk of the free fn being wasm-gated by proximity to
`http-client`/`oauth`-gated client code, and keeps the classifier + resolver importable as a
pair by external durable consumers who `use pmcp::types::tasks::*`.

## Standard Stack

### Core

No new external libraries. This phase uses only what the SDK already depends on.
`[VERIFIED: Cargo.toml]`

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` / `serde_json` | existing | `Task`/`TaskStatus` (de)serialization — unchanged | Already the protocol serde layer |
| `web_time` | existing | Wasm-safe `Instant` in `wait_for_task` — unchanged | Already used at `src/client/mod.rs:694` |
| `proptest` | 1.7 (dev-dep) | Property test for the exhaustive status→decision map | Available `[VERIFIED: Cargo.toml:145]`; house ALWAYS-property rule |
| `quickcheck` / `quickcheck_macros` | 1.0 / 1.1 (dev-dep) | Alternative property harness | Available `[VERIFIED: Cargo.toml:146-147]` |
| `tokio` | existing (dev) | Duplex-harness async tests + example | Already used by `tests/task_augmented_result.rs` |

### Supporting

No supporting libraries needed. The `resolve_poll_interval` helper wraps into
`std::time::Duration::from_millis` at the single sleep call site (D-12) — no `Duration`-typed
public surface. `[CITED: D-12]`

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `resolve_poll_interval -> u64` | `-> Duration` | Codex rated LOW; rejected by D-12 for cross-API consistency (`TaskMetadata.poll_interval`, `WaitForTaskOptions.poll_interval`, `Task.poll_interval` are all `Option<u64>` ms) |
| Resolver in `src/types/tasks.rs` | Resolver in `src/client/mod.rs` | Client mod is where callers of the wasm-gated `http-client`/`oauth` items live; placing a pure helper there risks accidental gating and a less-natural import path for external consumers. Types layer is cleaner. |
| `Terminal { status: TaskStatus }` | `Terminal { status, task }` or fetch result | D-06 forbids carrying the result; `TaskStatus` is `Copy` `[VERIFIED: src/types/tasks.rs:12 derives Copy]` so carrying it is free |

**Installation:** None — no `cargo add`. `[VERIFIED: no external package added]`

**Version verification:** Root crate is `pmcp` `2.12.0` `[VERIFIED: Cargo.toml:3]`. This phase
is a next-minor additive change (2.13.0-class per CONTEXT.md). No dependency version bumps.

## Package Legitimacy Audit

**This phase installs no external packages.** All libraries used (`serde`, `web_time`,
`tokio`, `proptest`, `quickcheck`) are pre-existing verified dependencies in `Cargo.toml`.
No slopcheck run is required — there is nothing new to vet. `[VERIFIED: Cargo.toml]`

**Packages removed due to slopcheck [SLOP] verdict:** none (none added)
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                         ┌─────────────────────────────────────────┐
   polled Task           │        src/types/tasks.rs (PURE)         │
   (already              │                                          │
    deserialized)  ─────►│  Task::poll_decision(&self)              │
                         │      match self.status {                 │
                         │        Working        => InProgress{hint}│
                         │        InputRequired  => InputRequired   │
                         │        Completed|Failed|Cancelled        │
                         │                       => Terminal{status}│
                         │      }                                   │
                         │                                          │
                         │  resolve_poll_interval(caller, hint)     │
                         │      caller.or(hint)                     │
                         │            .unwrap_or(1000).max(50)      │
                         └───────────────┬──────────────┬──────────┘
                                         │              │
              ┌──────────────────────────┘              └────────────────────┐
              │ (internal consumer, D-02)                  (external consumer)│
              ▼                                                               ▼
┌─────────────────────────────────────┐          ┌──────────────────────────────────────┐
│ src/client/mod.rs  wait_for_task     │          │ Durable/replay runtime (pmcp.run)    │
│ loop {                               │          │ loop {                               │
│   task = tasks_get(id).await?  ◄──I/O│          │   task = ctx.step(|| tasks_get)  ◄─I/O│  memoized
│   match task.poll_decision() {       │          │   match task.poll_decision() {       │  step:
│     Terminal   => break              │          │     Terminal   => ctx.step(||         │  network +
│     InputRequired => return Err(...) │  CR-01   │                   tasks_result)      │  serde INSIDE
│     InProgress{hint} =>              │          │     InputRequired => route to form   │  (D-14)
│       interval =                     │          │     InProgress{hint} =>              │
│         resolve_poll_interval(...)   │          │       ctx.wait(resolve_poll_interval)│  ctx.wait,
│       // budget clamp (WR-01, D-09)  │          │   }                                  │  NOT sleep
│       sleep(interval).await          │          │ }                                    │
│   }                                  │          │ (NO wait_for_task here — D-16)       │
│ }                                    │          └──────────────────────────────────────┘
│ tasks_result(id).await   ◄────────I/O│
└─────────────────────────────────────┘
```

Data flow to trace: a polled `Task` → `poll_decision()` (pure, no I/O) → the caller's loop
decides break/error/sleep. The terminal `CallToolResult` is always a **separate** `tasks/result`
fetch owned by the caller (D-06/D-16), never carried in the `Terminal` variant.

### Recommended Project Structure

```
src/types/tasks.rs        # ADD: TaskPollDecision enum, Task::poll_decision(),
                          #      resolve_poll_interval() free fn + unit/property tests
src/client/mod.rs         # EDIT: rewrite wait_for_task loop as match poll_decision();
                          #       call resolve_poll_interval(); budget clamp stays inline
examples/s48_durable_poll_decision.rs   # ADD: runnable plain-loop example (D-10)
tests/task_augmented_result.rs          # KEEP GREEN: existing 11 tests = regression net;
                          #       optionally ADD a drift-pin test (D-13)
pmcp-book/src/ch12-7-tasks.md           # ADD: "Durable and replay consumers" section (D-11)
pmcp-book/src/task-augmented-results.md # EDIT: cross-link to the new section
```

### Pattern 1: Shared decision function + parity (Phase 104 D-05 precedent)

**What:** A single pure function is the ONLY place a decision is made; every consumer
(blocking + durable) calls it, and a test pins them together.
**When to use:** Whenever two code paths must not drift on the same logic.
**Precedent:** `task_dispatch::resolve_tool_output` consumed by both dispatchers with tests
pinning both to it. `[VERIFIED: src/server/task_dispatch.rs exists; CITED: 104-CONTEXT D-05]`
**Example (the target shape for `wait_for_task`):**

```rust
// Source: derived from src/client/mod.rs:695-733 (current inline logic) + Codex suggestion
loop {
    let task = self.tasks_get(task_id).await?;
    match task.poll_decision() {
        TaskPollDecision::Terminal { .. } => break,
        TaskPollDecision::InputRequired => {
            return Err(Error::validation(format!(
                "task {task_id} is input_required; wait_for_task cannot provide \
                 input — handle the elicitation, then resume polling"
            )));
        }
        TaskPollDecision::InProgress { poll_hint } => {
            let mut interval = resolve_poll_interval(opts.poll_interval, poll_hint);
            // WR-01 budget clamp STAYS here (D-09) — loop state, not task state:
            if let Some(max_secs) = opts.max_poll_duration_secs {
                let budget_ms = max_secs.saturating_mul(1000);
                let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                let remaining_ms = budget_ms.saturating_sub(elapsed_ms);
                if remaining_ms == 0 {
                    return Err(Error::timeout(budget_ms));
                }
                interval = interval.min(remaining_ms.max(50)); // MIN_POLL_MS
            }
            crate::runtime::sleep(std::time::Duration::from_millis(interval)).await;
        }
    }
}
self.tasks_result(task_id).await
```

Note: because `TaskPollDecision` is `#[non_exhaustive]`, code **inside the defining crate**
(i.e. `wait_for_task`) can still match all three variants exhaustively with no `_` arm — so
the compiler forces you to handle a future variant when it is added. Only *external* consumers
need a `_` arm. This is the semver payoff of D-04. `[VERIFIED: Rust non_exhaustive semantics]`

### Pattern 2: Pure classifier method (`poll_decision`)

**What:** The mapping is a total match over the 5 `TaskStatus` values:

```rust
// Source: derived from src/types/tasks.rs:12-45 (TaskStatus + is_terminal)
impl Task {
    pub fn poll_decision(&self) -> TaskPollDecision {
        match self.status {
            TaskStatus::Working => TaskPollDecision::InProgress { poll_hint: self.poll_interval },
            TaskStatus::InputRequired => TaskPollDecision::InputRequired,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                TaskPollDecision::Terminal { status: self.status }
            }
        }
    }
}
```

Because `TaskStatus` is exhaustive (NOT `#[non_exhaustive]`, verified), this match needs no
`_` arm and stays a total function. `[VERIFIED: src/types/tasks.rs:12]`

### Pattern 3: `resolve_poll_interval` mirrors the current inline chain exactly

The current code (`src/client/mod.rs:712-716`) is:
`opts.poll_interval.or(task.poll_interval).unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS)`.
The helper must reproduce this precisely so behavior is byte-identical:

```rust
// Source: src/client/mod.rs:686-716
pub const DEFAULT_POLL_MS: u64 = 1000;
pub const MIN_POLL_MS: u64 = 50;

pub fn resolve_poll_interval(caller_override: Option<u64>, hint: Option<u64>) -> u64 {
    caller_override.or(hint).unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS)
}
```

Consider making `MIN_POLL_MS` / `DEFAULT_POLL_MS` `pub const` next to the resolver so the
budget clamp in `wait_for_task` (which references `MIN_POLL_MS` at line 730) and the resolver
share one source of truth rather than re-declaring the constant. `[VERIFIED: constant used in two places]`

### Anti-Patterns to Avoid

- **Calling `poll_decision()` "somewhere" but keeping `is_terminal()` inline (D-13):** The
  whole point is that the match arms ARE the status handling. Any residual
  `task.status.is_terminal()` or `task.status == TaskStatus::InputRequired` check inside
  `wait_for_task` reintroduces the drift the phase exists to eliminate.
- **Carrying the `CallToolResult` in `Terminal` (D-06/D-16):** The result is a separate
  `tasks/result` I/O call. The classifier is pure and must not fetch.
- **Documenting `#[non_exhaustive]` on `TaskPollDecision` as "handles unknown statuses" (D-15):**
  `TaskStatus` is exhaustive today; an unknown wire status fails at serde deserialization
  BEFORE `poll_decision()` ever runs. Keep the two claims distinct in prose.
- **Wrapping `wait_for_task` inside a replay/durable workflow (D-16):** it sleeps, loops, and
  owns the polling lifecycle — non-deterministic under replay.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Terminal-status detection in the durable consumer | A hand-copied `matches!(status, Completed\|Failed\|Cancelled)` | `task.poll_decision()` | This phase exists precisely so consumers stop re-deriving it (drift risk) |
| Interval precedence logic | Re-implementing caller→hint→default→floor | `resolve_poll_interval()` | Same drift risk; the helper is the single source |
| A fake durable-runtime harness for the example | A replay-simulator | Plain in-process duplex loop (D-10) | Explicitly out of scope; adds test surface with no value |
| Wasm-safe timing | `std::time::Instant` / `tokio::time::sleep` | `web_time::Instant` + `crate::runtime::sleep` | Already used; `std::time::Instant` panics on wasm32 `[VERIFIED: src/client/mod.rs:693-694, 732]` |

**Key insight:** The value of this phase is *deletion of hand-rolled divergence*, mirroring
the SDK's stated philosophy ("we just spent three releases deleting hand-rolled divergences
from typed contracts"). The primitive must be the *only* implementation, enforced structurally.

## Common Pitfalls

### Pitfall 1: Behavior drift in `wait_for_task` across the refactor
**What goes wrong:** The rewritten loop changes ordering (e.g., computes interval before the
budget check, or floors differently), producing subtly different timeout/overshoot behavior.
**Why it happens:** The current code interleaves interval computation, the `remaining_ms == 0`
timeout return, and the `interval.min(remaining_ms.max(MIN_POLL_MS))` clamp in a specific order
(`src/client/mod.rs:712-731`).
**How to avoid:** Preserve the exact ordering shown in Pattern 1. Keep the existing tests
`wait_for_task_times_out_and_does_not_hot_spin` and
`wait_for_task_timeout_is_not_overshot_by_large_interval` green unchanged — they are the pin.
**Warning signs:** Any change to these two tests' assertions is a red flag.

### Pitfall 2: `input_required` error message changes byte-for-byte (D-13 regression)
**What goes wrong:** The refactor rewords the `Error::validation` message.
**Why it happens:** Copy-paste while restructuring the match.
**How to avoid:** Copy the message verbatim from `src/client/mod.rs:706-709`. The existing test
`wait_for_task_surfaces_input_required_instead_of_hanging` (`tests/task_augmented_result.rs:417`)
asserts this path — keep it green. Consider strengthening it to assert the message substring.
**Warning signs:** That test needing edits.

### Pitfall 3: `resolve_poll_interval` accidentally wasm-gated or awkward to import
**What goes wrong:** Placing the helper in `src/client/mod.rs` near `http-client`/`oauth`-gated
items, or forgetting to re-export it, leaves external durable consumers unable to import it
next to `TaskPollDecision`.
**Why it happens:** `wait_for_task` lives in `src/client/mod.rs`, so it feels natural to put
the helper there.
**How to avoid:** Put it in `src/types/tasks.rs` (D-08 discretion) — the module has no cfg
gate and is re-exported via `pub use super::tasks::*` in `src/types/protocol/mod.rs:23`.
`[VERIFIED: src/types/protocol/mod.rs:23]`
**Warning signs:** A `#[cfg(...)]` appearing anywhere near the helper.

### Pitfall 4: `doc-check` failing on rustdoc intra-doc links
**What goes wrong:** New rustdoc links (`[TaskPollDecision]`, cross-links to book) break
`make doc-check`, which is stricter than `make quality-gate` on rustdoc links.
**Why it happens:** House rule: both gates must be green before push
`[CITED: CONTEXT.md code_context "doc-check is stricter than quality-gate on rustdoc links"]`.
**How to avoid:** Run `make doc-check` (target exists at `Makefile:418`) in addition to
`make quality-gate` (`Makefile:660`). Use fully-qualified intra-doc link paths.
**Warning signs:** `cargo doc` warnings about unresolved links.

## Runtime State Inventory

This is a greenfield-additive + refactor phase with **no rename, no data migration, no live
service state**. Not applicable in the migration sense, but the "what persists" check still
matters for one thing:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no datastore keys change | None (verified: phase is wire-neutral, no `TaskStatus` variant or serde rename) |
| Live service config | None | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | New public symbols (`TaskPollDecision`, `poll_decision`, `resolve_poll_interval`) become part of the `pmcp` public API surface | Additive only; `#[non_exhaustive]` on the enum keeps future adds non-breaking. Confirm no doc-test or semver-check tooling flags the additions. |

**Nothing found in the migration categories:** confirmed — no wire changes, no stored-key
renames, no new `TaskStatus` variants (scope fence, verified against `src/types/tasks.rs`).

## Code Examples

### Reuse the duplex-transport harness for the example and integration tests
```rust
// Source: tests/task_augmented_result.rs:120-274 (mod live)
// DuplexTransport::pair() gives an in-process client<->ServerCore mpsc pipe;
// spawn_counting_pump() drives the server; build_server() wires an InMemoryTaskStore.
// The s48 example (D-10) and any classifier integration test should reuse this shape
// (a plain poll loop over task.poll_decision()), NOT a fabricated durable runtime.
let (client_t, server_t) = DuplexTransport::pair();
let handler = build_server("complete_now", completing_task_tool());
spawn_counting_pump(server_t, handler, request_count.clone());
// client polls: loop { let task = client.tasks_get(id).await?; match task.poll_decision() {...} }
```

### Method-accessor precedent (D-01 discoverability model)
```rust
// Source: src/types/tools.rs:661
// CallToolResult::related_task(&self) -> Option<TaskMetadata>
// poll_decision() follows this exact "method on the thing you just got back" shape.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Poll decision buried in `wait_for_task` loop | Extracted pure `Task::poll_decision()` + `resolve_poll_interval()` | This phase (2.13.0-class) | Durable/replay consumers get the primitive without re-deriving it |
| `input_required` silently spins or errors only in blocking poller | `InputRequired` is an actionable classifier variant | This phase | `input_required` becomes a branchable state for every consumer shape |

**Deprecated/outdated:** Nothing deprecated. The 2.12.0 CR-01 `input_required` typed-error
default in `wait_for_task` is explicitly **preserved verbatim** (scope fence). `[VERIFIED: src/client/mod.rs:705-710]`

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Target version is "2.13.0-class" (next minor) | Summary | Low — CONTEXT.md states this; actual number set at release time. No code impact. |
| A2 | Making `DEFAULT_POLL_MS`/`MIN_POLL_MS` `pub const` next to the resolver is acceptable | Pattern 3 | Low — they are currently private inline consts; exposing them is additive but is a design choice the planner may reject in favor of keeping them private and duplicating the floor. |

**Note:** All *behavioral* claims (variant mapping, constants, error message, harness shape,
file locations) are `[VERIFIED]` against source read this session, not assumed. The two entries
above are the only genuinely open choices, both LOW risk.

## Open Questions

*(Both resolved during planning — plans implement the recommendations below.)*

1. **(RESOLVED — Plan 105-01 Task 2 exposes them as `pub const`)** **Should `MIN_POLL_MS`/`DEFAULT_POLL_MS` become `pub const`?**
   - What we know: They're private inline consts in `wait_for_task` (`src/client/mod.rs:686-688`);
     `MIN_POLL_MS` is referenced twice (resolver logic + budget clamp).
   - What's unclear: Whether to expose them (single source of truth, discoverable defaults) or
     keep private and let the resolver own them internally.
   - Recommendation: Expose as `pub const` in `src/types/tasks.rs` next to the resolver — makes
     the "50 ms floor / 1000 ms default" contract documentable and testable. Low risk.

2. **(RESOLVED — Plan 105-03 follows the recommendation: pure unit/property test + runnable example; drift-pin stays in `tests/task_augmented_result.rs`)** **Does the example (`s48`) need a distinct integration test, or does it double as one?**
   - What we know: House ALWAYS rule wants example + unit + property + fuzz/proptest. The
     existing 11-test suite already covers `wait_for_task`.
   - What's unclear: Whether a separate `poll_decision` integration test over the duplex harness
     adds value beyond the property/table test of the status→decision map.
   - Recommendation: A pure unit/property test of `poll_decision()` (no harness) plus the runnable
     example is sufficient; the drift-pin test (D-13) belongs in `tests/task_augmented_result.rs`.

## Environment Availability

Code/docs-only phase. The only tooling beyond the Rust toolchain is the mdBook build and the
two make gates, all already present.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `make quality-gate` | Pre-push gate | ✓ | `Makefile:660` | — |
| `make doc-check` | rustdoc-link gate (stricter) | ✓ | `Makefile:418` | — |
| `proptest` | property test (ALWAYS rule) | ✓ | 1.7 (Cargo.toml:145) | quickcheck 1.0 |
| mdBook (pmcp-book) | D-11 book section | ✓ (repo has `pmcp-book/`) | — | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none material — proptest vs quickcheck is interchangeable.

## Validation Architecture

> `workflow.nyquist_validation` is `true` in `.planning/config.json` — section included.
> `[VERIFIED: .planning/config.json]`

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]`; `proptest` 1.7 for property tests |
| Config file | none (cargo-native); `Cargo.toml` dev-deps |
| Quick run command | `cargo test --test task_augmented_result` (11-test regression net) + `cargo test --lib types::tasks` |
| Full suite command | `make quality-gate` (fmt + clippy pedantic/nursery + build + test + audit) then `make doc-check` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| D-03 (exhaustive map) | every `TaskStatus` → expected `TaskPollDecision` | unit/property | `cargo test --lib types::tasks::poll_decision` | ❌ Wave 0 (new tests in `src/types/tasks.rs`) |
| D-08/D-12 (resolver precedence) | caller override > hint > 1000 default; floors to 50 ms | unit | `cargo test --lib types::tasks::resolve_poll_interval` | ❌ Wave 0 |
| D-02/D-13 (drift pin) | `wait_for_task` matches classifier; `input_required` error byte-identical | integration | `cargo test --test task_augmented_result wait_for_task_surfaces_input_required` | ✅ (exists at `tests/task_augmented_result.rs:417`; may strengthen) |
| D-09 (budget clamp) | clamp still prevents oversleep, stays outside resolver | integration | `cargo test --test task_augmented_result wait_for_task_timeout_is_not_overshot` | ✅ (exists at `:380`) |
| D-13 (terminal behavior) | terminal → returns `tasks/result` unchanged | integration | `cargo test --test task_augmented_result wait_for_task_returns_terminal_result` | ✅ (exists at `:290`) |
| D-10 (example) | runnable plain-loop example compiles & runs | example | `cargo run --example s48_durable_poll_decision` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib types::tasks` (fast, pure classifier/resolver tests)
- **Per wave merge:** `cargo test --test task_augmented_result` (11-test regression net) + `cargo run --example s48_*`
- **Phase gate:** `make quality-gate` AND `make doc-check` both green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/types/tasks.rs` — add `#[cfg(test)]` unit + proptest for `poll_decision()` exhaustive map (D-03) — covers D-03
- [ ] `src/types/tasks.rs` — add unit tests for `resolve_poll_interval` precedence + floor (D-08/D-12)
- [ ] `examples/s48_durable_poll_decision.rs` — new runnable example (D-10)
- [ ] (optional) strengthen `tests/task_augmented_result.rs:417` to assert the `input_required` message substring (D-13 pin)
- [ ] Framework install: none — proptest already a dev-dep

## Security Domain

> `security_enforcement` not set to `false` in config → treated as enabled. This phase adds no
> new network surface, auth, or crypto; the one relevant control is input validation of the
> already-deserialized `Task`.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase touches no auth path |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | `Task`/`TaskStatus` are validated by serde at `tasks/get` deserialization (D-14); `poll_decision()` runs only over an already-typed `Task`, so no untrusted-string branching. Unknown wire status → hard serde error BEFORE classification. |
| V6 Cryptography | no | — |

### Known Threat Patterns for {Rust SDK client-side task polling}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Hot-loop / busy-spin (DoS via zero/absent `pollInterval`) | Denial of Service | 50 ms floor in `resolve_poll_interval` (preserved from current `MIN_POLL_MS`); property-tested (D-12 verification) |
| Unbounded poll (server never terminal) | Denial of Service | `max_poll_duration_secs` budget clamp stays in `wait_for_task` (D-09, WR-01); unchanged |
| Unknown/malformed status handled as a live branch | Tampering / misparse | Serde rejects unknown status at deserialization; `poll_decision()` is total over the 5 known variants — no silent default path (D-15) |

## Sources

### Primary (HIGH confidence)
- `src/types/tasks.rs` (Read) — `TaskStatus` (5 variants, exhaustive, `is_terminal()` :44), `Task` struct :91, `TaskMetadata`, existing test module
- `src/client/mod.rs:61-103, 634-756` (Read) — `WaitForTaskOptions`, `wait_for_task` loop with `DEFAULT_POLL_MS`/`MIN_POLL_MS`, CR-01 `input_required` error, WR-01 budget clamp, wasm-safe timing
- `src/types/tools.rs:630,661` (grep) — `CallToolResult::related_task()`/`with_related_task()` method-accessor precedent
- `src/types/protocol/mod.rs:23` (grep) — `pub use super::tasks::*` re-export
- `tests/task_augmented_result.rs:117-437` (Read) — duplex-transport `mod live` harness; 6 top-level + 5 live async tests = 11-test regression net
- `Cargo.toml` (grep) — version 2.12.0; proptest 1.7, quickcheck 1.0/1.1 dev-deps
- `Makefile:222,418,660` (grep) — `test-doc`, `doc-check`, `quality-gate` targets
- `.planning/config.json` (Read) — nyquist_validation true, commit_docs true
- `pmcp-book/src/{ch12-7-tasks.md,task-augmented-results.md,SUMMARY.md}` (grep) — chapter structure and registration
- `~/Development/mcp/sdk/pmcp-run/.planning/notes/sdk-response-durable-task-consumer-and-input-required.md` (Read) — the contract this phase fulfills; classifier shape sketch, Ask B deferral rationale

### Secondary (MEDIUM confidence)
- `.planning/phases/105-.../105-CONTEXT.md` + `105-REVIEWS.md` — LOCKED decisions D-01..D-16 + Codex review (folded)

### Tertiary (LOW confidence)
- None — all technical claims verified against source this session.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; all libraries verified present in `Cargo.toml`
- Architecture: HIGH — current inline logic fully read; target shape is a mechanical extraction
- Pitfalls: HIGH — derived from the exact ordering/message in the read source + house doc-check rule

**Research date:** 2026-07-05
**Valid until:** 2026-08-04 (stable — internal refactor of a shipped 2.12.0 surface; re-verify only if `wait_for_task` or `TaskStatus` changes upstream before planning)
