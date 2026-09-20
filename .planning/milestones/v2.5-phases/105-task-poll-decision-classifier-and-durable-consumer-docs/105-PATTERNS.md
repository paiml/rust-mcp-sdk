# Phase 105: Task poll-decision classifier and durable-consumer docs - Pattern Map

**Mapped:** 2026-07-05
**Files analyzed:** 6 (2 source-edit, 1 new example, 1 test-edit, 2 doc-edit)
**Analogs found:** 6 / 6 (every new symbol and file has an in-repo precedent)

This phase is a **wire-neutral, additive refactor** — every new symbol has a
near-exact analog already in the file it lands in. There are NO "no analog"
files. Pattern discipline here is "copy the shape already sitting next to it,"
not "invent a new module."

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/types/tasks.rs` (ADD `TaskPollDecision` enum + `Task::poll_decision()` + `resolve_poll_interval()` + `pub const` floors) | model / types | transform (pure fn of `Task`) | Same file: `TaskStatus` enum (`:11-26`), `TaskStatus::is_terminal()` (`:40-46`), `Task` impl builders (`:114-159`) | exact (same file, same module conventions) |
| `src/client/mod.rs` (EDIT `wait_for_task` loop → `match task.poll_decision()`; call `resolve_poll_interval`) | service / client | request-response poll loop | Itself pre-refactor (`:680-736`) — mechanical extraction, byte-identical behavior | exact (in-place refactor) |
| `examples/s48_durable_poll_decision.rs` (NEW runnable plain-loop example) | example | request-response poll loop | `tests/task_augmented_result.rs` `mod live` duplex harness (`:120-274`); example prose/structure from `examples/s47_task_augmented_result.rs` | role-match (harness from tests, prose shape from s47) |
| `tests/task_augmented_result.rs` (EDIT: optional drift-pin / message-substring strengthen) | test | request-response | Existing tests in same file (`:289-457`) | exact (same file, established `#[tokio::test]` duplex pattern) |
| `pmcp-book/src/ch12-7-tasks.md` (ADD "Durable and replay consumers" section) | docs | — | Same file: "The Polling Model" §132, "Task Status State Machine" §516 | exact (append a section in the existing chapter) |
| `pmcp-book/src/task-augmented-results.md` (EDIT: cross-link to new section) | docs | — | Existing cross-links in `ch12-7-tasks.md` (`[Recommended Pattern](#...)` §195) | exact (same intra-book link idiom) |

## Pattern Assignments

### `src/types/tasks.rs` — `TaskPollDecision` enum (model, transform)

**Analog:** `TaskStatus` enum in the same file (`src/types/tasks.rs:11-26`). Copy
its derive/doc/serde conventions, but note the two deliberate differences below.

**Enum-shape pattern to copy** (`src/types/tasks.rs:11-26`):
```rust
/// Task status (5-value enum).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being worked on
    #[default]
    Working,
    // ...
}
```

**Deliberate departures for `TaskPollDecision` (from CONTEXT D-03/D-04):**
- Add `#[non_exhaustive]` (D-04) — `TaskStatus` deliberately is NOT `#[non_exhaustive]`
  (`:12`), so do NOT copy that omission. This is the one place the two enums differ
  on attributes, and D-15 forbids conflating the two claims in docs.
- Three variants: `Terminal { status: TaskStatus }`, `InProgress { poll_hint: Option<u64> }`,
  `InputRequired` (unit). `TaskStatus` is `Copy` (`:12` derives `Copy`), so
  `Terminal { status }` carries the status by value for free (D-06 forbids carrying
  the `CallToolResult`).
- Serde is NOT required on `TaskPollDecision` — it is a returned classifier value, not
  a wire type (unlike every other type in this file). Deriving `Debug, Clone, PartialEq, Eq`
  is enough; skip `Serialize/Deserialize` and `#[serde(...)]`.

**Doc-string obligation (D-16):** the `Terminal` variant rustdoc MUST state the caller
still issues a separate `tasks/result` to get the final `CallToolResult`. Model the
doc density on the existing `TaskMetadata` field docs (`:237-248`) which carefully
distinguish ms vs secs units.

---

### `src/types/tasks.rs` — `Task::poll_decision()` method (model, transform)

**Analog (method-accessor precedent, D-01):** `CallToolResult::related_task()` at
`src/types/tools.rs:643-666` and the existing `TaskStatus::is_terminal()` /
`can_transition_to()` methods (`src/types/tasks.rs:40-72`).

**`is_terminal()` total-match pattern to mirror** (`src/types/tasks.rs:40-46`):
```rust
impl TaskStatus {
    /// Returns `true` if this status is terminal (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}
```

**Target shape** (add an `impl Task` block near the existing one at `:114`):
```rust
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
Because `TaskStatus` is exhaustive (verified `:12` — no `#[non_exhaustive]`), this
match needs no `_` arm and is a total function (guards D-15's exhaustiveness claim).

**Precedent for "method on the thing you just got back"** (`src/types/tools.rs:661`):
```rust
pub fn related_task(&self) -> Option<crate::types::tasks::TaskMetadata> {
    self._meta.as_ref()?
        .get(crate::types::tasks::RELATED_TASK_META_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}
```

---

### `src/types/tasks.rs` — `resolve_poll_interval()` free fn + `pub const` floors (model, transform)

**Analog:** the inline constants and resolution expression currently in
`wait_for_task` (`src/client/mod.rs:685-716`). Lift them VERBATIM so behavior is
byte-identical.

**Source of truth to lift** (`src/client/mod.rs:685-716`):
```rust
const DEFAULT_POLL_MS: u64 = 1000;   // :686
const MIN_POLL_MS: u64 = 50;         // :688
// ...
let mut interval = opts
    .poll_interval
    .or(task.poll_interval)
    .unwrap_or(DEFAULT_POLL_MS)
    .max(MIN_POLL_MS);               // :712-716
```

**Target helper** (place in `src/types/tasks.rs`, co-located with the enum per D-08 /
RESEARCH primary recommendation — the types module has NO cfg gate, keeping the wasm
boundary clean; it is re-exported via `pub use super::tasks::*` in
`src/types/protocol/mod.rs:23`):
```rust
/// Default poll interval when neither caller nor task specify one.
pub const DEFAULT_POLL_MS: u64 = 1000;
/// Floor applied to any interval so a zero value cannot hot-spin.
pub const MIN_POLL_MS: u64 = 50;

pub fn resolve_poll_interval(caller_override: Option<u64>, hint: Option<u64>) -> u64 {
    caller_override.or(hint).unwrap_or(DEFAULT_POLL_MS).max(MIN_POLL_MS)
}
```
Returns `u64` ms (D-12), NOT `Duration` — symmetric with the `Option<u64>` inputs and
consistent with `TaskMetadata.poll_interval` / `WaitForTaskOptions.poll_interval` /
`Task.poll_interval` (all `Option<u64>` ms). Caller wraps at the sleep site.

**Test pattern to copy** (`src/types/tasks.rs:405-607` `#[cfg(test)] mod tests`):
the module already uses plain `#[test]` unit tests with table-style assertions
(see `task_status_terminal_rejects_all` at `:553-573` — a nested-loop exhaustive
table). Mirror that for the D-03 exhaustive status→decision map and D-08/D-12 resolver
precedence. `proptest` (1.7, dev-dep) is available for the property variant.

---

### `src/client/mod.rs` — `wait_for_task` refactor (service, request-response poll loop)

**Analog:** itself, pre-refactor (`src/client/mod.rs:680-736`). This is a
mechanical, behavior-preserving extraction (D-13: STRUCTURAL, not incidental).

**Current inline logic to REPLACE** (`src/client/mod.rs:695-733`):
```rust
loop {
    let task = self.tasks_get(task_id).await?;
    if task.status.is_terminal() {              // <-- REMOVE: replaced by match arm
        break;
    }
    if task.status == TaskStatus::InputRequired {  // <-- REMOVE: replaced by match arm
        return Err(Error::validation(format!(
            "task {task_id} is input_required; wait_for_task cannot provide \
             input — handle the elicitation, then resume polling"
        )));
    }
    let mut interval = opts.poll_interval          // <-- REPLACE with resolve_poll_interval(...)
        .or(task.poll_interval)
        .unwrap_or(DEFAULT_POLL_MS)
        .max(MIN_POLL_MS);
    if let Some(max_secs) = opts.max_poll_duration_secs {   // <-- KEEP inline (D-09 budget clamp)
        let budget_ms = max_secs.saturating_mul(1000);
        let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let remaining_ms = budget_ms.saturating_sub(elapsed_ms);
        if remaining_ms == 0 {
            return Err(Error::timeout(budget_ms));
        }
        interval = interval.min(remaining_ms.max(MIN_POLL_MS));
    }
    crate::runtime::sleep(std::time::Duration::from_millis(interval)).await;
}
self.tasks_result(task_id).await
```

**Target shape** — explicit three-arm `match task.poll_decision()` (D-13). No residual
`is_terminal()` / `== TaskStatus::InputRequired` comparison may remain:
```rust
loop {
    let task = self.tasks_get(task_id).await?;
    match task.poll_decision() {
        TaskPollDecision::Terminal { .. } => break,
        TaskPollDecision::InputRequired => {
            return Err(Error::validation(format!(
                "task {task_id} is input_required; wait_for_task cannot provide \
                 input — handle the elicitation, then resume polling"   // COPY VERBATIM (Pitfall 2)
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
                interval = interval.min(remaining_ms.max(MIN_POLL_MS));
            }
            crate::runtime::sleep(std::time::Duration::from_millis(interval)).await;
        }
    }
}
self.tasks_result(task_id).await
```

**Preserve exactly (regression pins):**
- Wasm-safe timing: `web_time::Instant::now()` (`:694`) + `crate::runtime::sleep` (`:732`)
  — NOT `std::time::Instant` / `tokio::time::sleep` (panics on wasm32).
- The `input_required` `Error::validation` message byte-for-byte (`:706-709`).
- Ordering: interval compute → `remaining_ms == 0` timeout return → `.min(remaining_ms.max(MIN_POLL_MS))`
  clamp (`:712-731`) — reordering changes overshoot behavior.
- Because `TaskPollDecision` is `#[non_exhaustive]` but `wait_for_task` is IN the defining
  crate, all three arms must be matched with no `_` arm (compiler forces future-variant handling).

**Rustdoc obligation (D-11/D-16):** add a cross-link from this fn's rustdoc to the new
book "Durable and replay consumers" section, and a "do NOT wrap `wait_for_task` inside a
replay workflow" note.

---

### `examples/s48_durable_poll_decision.rs` (example, request-response poll loop)

**Analog (harness):** `tests/task_augmented_result.rs` `mod live` (`:120-274`) — reuse
the `DuplexTransport::pair()` + `spawn_counting_pump()` + `build_server()` shape (D-10).
Do NOT build a fake durable runtime.

**Duplex harness pattern to reuse** (`tests/task_augmented_result.rs:135-274`):
```rust
// DuplexTransport::pair() -> in-process client<->ServerCore mpsc pipe (:143-160)
let (client_transport, server_transport) = DuplexTransport::pair();
// spawn_counting_pump drives the server handler (:193-213)
spawn_counting_pump(server_transport, handler, request_count.clone());
// build_server wires an InMemoryTaskStore + a TypedTool (:260-274)
let handler = build_server("complete_now", completing_task_tool());
```

**Example structure/prose pattern:** `examples/s47_task_augmented_result.rs` — copy its
conventions:
- `//!`-doc header explaining the teaching point + `Run with: cargo run --example ...`
  line (`s47:1-54`).
- `#![cfg(not(target_arch = "wasm32"))]` guard (`s47:56`).
- `#[tokio::main] async fn main() -> pmcp::Result<()>` (`s47:78`).
- HARD assertions returning `Err` on invariant violation, so the example doubles as a
  regression harness (`s47:196-217`), plus `println!` narration.

**Core loop the example demonstrates** (the whole point — a plain poll loop over the
classifier + resolver, NOT `wait_for_task`):
```rust
loop {
    let task = client.tasks_get(&task_id).await?;
    match task.poll_decision() {
        TaskPollDecision::Terminal { .. } => break,
        TaskPollDecision::InputRequired => { /* route to elicitation */ break; }
        TaskPollDecision::InProgress { poll_hint } => {
            let interval = resolve_poll_interval(None, poll_hint);
            pmcp::runtime::sleep(std::time::Duration::from_millis(interval)).await;
        }
    }
}
let result = client.tasks_result(&task_id).await?;
```

---

### `tests/task_augmented_result.rs` (test, request-response)

**Analog:** the existing `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`
tests in the same `mod live` (`:289-457`). Any drift-pin / message-substring strengthen
(D-13) copies this exact structure.

**Test pattern to copy** (`tests/task_augmented_result.rs:416-457`
`wait_for_task_surfaces_input_required_instead_of_hanging`):
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_for_task_surfaces_input_required_instead_of_hanging() {
    let store = Arc::new(InMemoryTaskStore::new());
    let server: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new().name("wait-for-task-server").version("1.0.0")
            .tool("stay_pending", pending_task_tool())
            .task_store(store.clone() as Arc<dyn TaskStore>)
            .build().expect("server builds"),
    );
    // ... duplex pair + pump + initialize ...
    store.update_status(&task_id, "local", TaskStatus::InputRequired, None).await
        .expect("transition to input_required");
    let err = tokio::time::timeout(
        std::time::Duration::from_secs(10),   // CI safety net against regression-hang
        client.wait_for_task(&task_id, WaitForTaskOptions::default()),
    ) /* ... */;
}
```

**Keep GREEN unchanged (regression net):** `wait_for_task_returns_terminal_result`
(`:290`), `wait_for_task_times_out_and_does_not_hot_spin` (`:339`),
`wait_for_task_timeout_is_not_overshot_by_large_interval` (`:380`). Any assertion change
in these three is a red flag (Pitfall 1).

---

### `pmcp-book/src/ch12-7-tasks.md` + `task-augmented-results.md` (docs)

**Analog:** existing sections in the same chapter — "The Polling Model" (§132-189)
and "Task Status State Machine" (§516). Append the new "Durable and replay consumers"
section after these, matching the `##` heading + fenced-`json`/`rust` snippet idiom.

**Cross-link idiom to copy** (`ch12-7-tasks.md:195`): `[Recommended Pattern](#recommended-pattern-tools-as-tasks)`
— use the same intra-book relative-anchor style for the cross-link from
`task-augmented-results.md` into the new section.

**Doc content obligations (D-11/D-14/D-15/D-16):**
- Temporal-style `ctx.step`/`ctx.wait` typed-accessors-without-the-loop pattern
  (non-runnable snippets).
- D-14: state that `poll_decision()` is replay-deterministic ONLY over an
  already-deserialized `Task`; the `tasks/get` network call AND the serde decode must
  sit INSIDE the memoized step; unknown/future status fails at deserialization BEFORE
  classification runs.
- D-15: keep "`TaskStatus` is exhaustive today" and "`TaskPollDecision` is
  `#[non_exhaustive]` (future-proofing)" as DISTINCT claims. Do not imply runtime
  graceful handling of unknown statuses.
- D-16: explicit "do NOT wrap `wait_for_task` inside a replay workflow" warning.
- SUMMARY.md registration only needed if a NEW page is created — the plan uses an
  in-chapter section (`SUMMARY.md:34-35` already registers the two existing pages),
  so no SUMMARY edit is required unless the planner splits it out.

## Shared Patterns

### Single shared-decision-fn + parity discipline (Phase 104 D-05 precedent)
**Source:** `src/server/task_dispatch.rs` `resolve_tool_output` (consumed by both
dispatchers, with tests pinning both to it — cited in CONTEXT code_context / 104-CONTEXT D-05).
**Apply to:** `poll_decision()` + `resolve_poll_interval()` MUST be the ONLY place the
decision/interval logic lives; `wait_for_task` consumes them; a test pins the two together.
This is the entire thesis of the phase (D-02/D-13).

### Wasm-safe timing (never `std::time::Instant` / `tokio::time::sleep`)
**Source:** `src/client/mod.rs:694` (`web_time::Instant::now()`) and `:732`
(`crate::runtime::sleep`).
**Apply to:** `wait_for_task` refactor AND the s48 example poll loop. `std::time::Instant`
panics on wasm32 (RESEARCH "Don't Hand-Roll").

### Types-layer home keeps the wasm boundary clean
**Source:** `src/types/protocol/mod.rs:23` (`pub use super::tasks::*`); `src/types/mod.rs`
has no cfg gate.
**Apply to:** put `TaskPollDecision`, `poll_decision`, `resolve_poll_interval`, and the
`pub const` floors ALL in `src/types/tasks.rs` — never in `src/client/mod.rs` near the
`http-client`/`oauth`-gated items (Pitfall 3).

### Doc-check is stricter than quality-gate on rustdoc links
**Source:** house rule (CONTEXT code_context); `Makefile:418` (`doc-check`), `:660`
(`quality-gate`).
**Apply to:** any new rustdoc intra-doc link (`[TaskPollDecision]`, cross-links). Run BOTH
gates before push; use fully-qualified intra-doc paths (Pitfall 4).

### `#[non_exhaustive]` + builder-`new()` convention for public task types
**Source:** every public struct/enum in `src/types/tasks.rs` that is meant to grow
(`Task` `:92`, `TaskMetadata` `:232`, `CreateTaskResult` `:280`, etc.) carries
`#[non_exhaustive]` and a `new()` + `with_*` builder.
**Apply to:** `TaskPollDecision` gets `#[non_exhaustive]` (D-04). `TaskStatus` is the
deliberate EXCEPTION (`:12`, exhaustive) — do not "fix" it (D-15).

## No Analog Found

None. Every file and every new symbol in this phase has an in-repo precedent — this is a
mechanical extraction/refactor plus docs, not new architecture. RESEARCH.md confirms
"the entire technical surface ... was verified to exist exactly as CONTEXT.md describes."

## Metadata

**Analog search scope:** `src/types/tasks.rs`, `src/types/tools.rs`, `src/client/mod.rs`,
`tests/task_augmented_result.rs`, `examples/s4*`, `pmcp-book/src/ch12-7-tasks.md`,
`pmcp-book/src/SUMMARY.md`
**Files scanned:** 7 (full reads of tasks.rs, tools.rs excerpt, client/mod.rs excerpts,
task_augmented_result.rs, s47 example, ch12-7-tasks.md excerpts; s4x example listing)
**Pattern extraction date:** 2026-07-05
