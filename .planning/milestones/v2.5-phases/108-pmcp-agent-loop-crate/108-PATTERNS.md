# Phase 108: `pmcp-agent` Loop Crate - Pattern Map

> **⚠ Superseded in part by the `--reviews` replan (2026-07-17).** Two corrections override any stale references below:
> 1. The iteration module directory is **`src/iteration/`** (not `src/loop/` — `loop` is a Rust keyword).
> 2. The adapter/invoker use **`ServerCoreBuilder::task_store`** + **`Client::wait_for_related_task`** (NOT the legacy `with_task_store`/`wait_for_task` + `WaitForTaskOptions::default()`, which has no overall timeout).
> Additional replan-hardening (transport-actor pump, WithTools end-to-end, checkpoint/RunOutcome, `url-connector` feature gate, stdio cancel-safety) lives in the 108-0N-PLAN.md files, which are authoritative.


**Mapped:** 2026-07-17
**Files analyzed:** 24 (21 new in `crates/pmcp-agent`, 3 modified in `pmcp` core + workspace)
**Analogs found:** 22 / 24 (2 genuinely-new surfaces: D-106-A response pump + `sample_with_tools` peer path)

Every hard primitive this crate needs already exists in `pmcp` or `pmcp-tasks`/`pmcp-package`. The dominant instruction to the planner: **compose existing analogs; do not reimplement.** The only genuinely new SDK code is the D-106-A response pump in `Server::run` and the paired `sample_with_tools` peer addition (Open Q1).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/pmcp-agent/Cargo.toml` | config | — | `crates/pmcp-tasks/Cargo.toml` | exact |
| `crates/pmcp-agent/src/lib.rs` | module root | — | `crates/pmcp-tasks/src/lib.rs` | exact |
| `src/seams/completion.rs` | trait (seam) | request-response | `src/client/host/sampling.rs` `HostSamplingHandler` | exact |
| `src/seams/tool.rs` | trait (seam) | batch/request-response | `crates/pmcp-tasks/.../store/mod.rs` `TaskStore` (async trait shape) | role-match |
| `src/seams/store.rs` | trait (seam) + `InMemoryStore` | CRUD | `crates/pmcp-tasks/.../store/backend.rs` `StorageBackend` + `InMemoryBackend` | exact |
| `src/loop/decide.rs` | pure functions | transform | `src/types/tasks.rs:349` `poll_decision` | exact |
| `src/loop/result.rs` | data types | transform | `src/types/tasks.rs` `TaskPollDecision` (classification enum) | exact |
| `src/loop/engine.rs` | service (orchestrator) | event-driven | reference `iteration.rs` (read-only) + `tests/common/duplex.rs` loop shape | role-match |
| `src/sources/sampling.rs` | provider (seam impl) | request-response | `src/server/peer_impl.rs:55` `DispatchPeerHandle::sample` | exact |
| `src/sources/openai_compat.rs` | provider (seam impl) | request-response | (feature-gated HTTP) `reqwest` workspace convention | role-match |
| `src/sources/anthropic.rs` | provider (seam impl) | request-response | same as `openai_compat.rs` | role-match |
| `src/invoker/client.rs` | service (seam impl) | request-response + polling | `src/client/mod.rs:771` `wait_for_task` + `poll_decision` | exact |
| `src/adapter/server.rs` | adapter | request-response | `src/server/builder.rs:766` `with_task_store` + `ToolOutput::Result` | exact |
| `src/config/resolver.rs` | trait (seam) + 2 impls | transform | `crates/pmcp-package/src/slot/deviation.rs` `detect_deviation` | role-match |
| `src/config/endpoint.rs` | utility | transform | `crates/pmcp-package/src/package/agent.rs` `AgentPackage.connectors` | role-match |
| `src/trace.rs` | data type (serde artifact) | transform | `TaskPollDecision` serde + `store/backend.rs` `VersionedRecord` | role-match |
| `tests/real_loop_sampling.rs` | test | request-response | `tests/common/duplex.rs` `call_via_server` | exact |
| `tests/replay_safety.rs` + `tests/fixtures/*.json` | test | transform | `crates/pmcp-tasks` proptest + golden-fixture convention | role-match |
| `examples/s50_standalone_vs_sampled.rs` | example | request-response | `examples/s49_sampling_host.rs`, `examples/s43_handler_peer_sample.rs` | exact |
| `fuzz/fuzz_targets/agent_digest.rs` | fuzz target | transform | existing `fuzz/fuzz_targets/` (proptest-style digest fuzz) | role-match |
| **MODIFY** `src/server/mod.rs` `spawn_message_handler` | core fix | event-driven | (self — D-106-A site, lines 1043–1147) | NEW surface |
| **MODIFY** `src/server/peer_impl.rs` / `src/shared/peer.rs` | core addition | request-response | `DispatchPeerHandle::sample` (extend → `sample_with_tools`) | NEW surface |
| **MODIFY** `tests/in_tool_peer_roundtrip.rs` (pmcp) | test | request-response | `tests/common/duplex.rs` `call_via_server` (real `Server::run`) | exact |
| **MODIFY** root `Cargo.toml` (members + version 2.16.0→2.17.0), `CLAUDE.md` publish order | config | — | existing `[workspace].members` line 582 | exact |

## Pattern Assignments

### `crates/pmcp-agent/Cargo.toml` (config)

**Analog:** `crates/pmcp-tasks/Cargo.toml` — the 0.x isolation precedent. Copy verbatim structure; bump pmcp dep to `2.17.0`, swap DDB/redis optional-dep block for `reqwest`-gated `openai-compat`/`anthropic`.

**Package + core deps** (lines 1–19):
```toml
[package]
name = "pmcp-agent"
version = "0.1.0"
edition = "2021"
description = "Agent decision-loop runtime for the PMCP SDK (experimental)"
license = "MIT"
rust-version = "1.91.0"

[dependencies]
pmcp = { version = "2.17.0", path = "../..", default-features = false }
pmcp-package = { version = "0.1", path = "../pmcp-package" }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }   # preserve_order = replay determinism (Pitfall 3)
async-trait = "0.1"
thiserror = "2.0"
futures = "0.3"
tracing = "0.1"
```

**Feature-gated optional-dep block** (mirror pmcp-tasks lines 25–33 `dynamodb`/`redis` → `openai-compat`/`anthropic`; keeps wasm32 clean per D-13/Pitfall 4):
```toml
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls"], optional = true }

[features]
openai-compat = ["dep:reqwest"]
anthropic = ["dep:reqwest"]

[dev-dependencies]
pmcp = { version = "2.17.0", path = "../..", features = ["full"] }   # real-loop D-03 tests
tokio = { version = "1", features = ["full"] }
proptest = "1.7"
pretty_assertions = "1.4"
```

Note pmcp-tasks pins its optional AWS deps to `default-https-client` for RUSTSEC reasons (lines 21–24) — the same `# Why:` annotation discipline applies if any optional dep needs a non-default TLS path.

---

### `crates/pmcp-agent/src/lib.rs` (module root)

**Analog:** `crates/pmcp-tasks/src/lib.rs` — copy the `//!` crate-doc + `pub mod` + ergonomic `pub use` re-export layout exactly.

**Pattern** (lines 28–65): declare each module with a one-line rustdoc, then re-export the public surface. Feature-gate the HTTP sources exactly as pmcp-tasks gates `dynamodb`/`redis`:
```rust
pub mod seams;
pub mod config;
pub mod invoker;
pub mod adapter;
pub mod trace;

pub use seams::{CompletionSource, ToolInvoker, ConversationStore};
pub use invoker::ClientToolInvoker;
pub use sources::SamplingSource;                         // zero-dep, always available
#[cfg(feature = "openai-compat")]
pub use sources::OpenAiCompatSource;
#[cfg(feature = "anthropic")]
pub use sources::AnthropicSource;
```
Also copy the top-of-file `#![allow(clippy::used_underscore_binding)]` if `_meta`/`_task_id` MCP fields are touched (lib.rs line 2).

---

### `src/seams/completion.rs` (trait, request-response) — AGNT-01/AGNT-04

**Analog:** `src/client/host/sampling.rs:36` `HostSamplingHandler` — the closest existing "produce a completion from `CreateMessageParams`" async trait.

**Existing analog signature** (sampling.rs lines 36–54 — note it returns the WRONG type for an agent loop):
```rust
#[async_trait]
pub trait HostSamplingHandler: Send + Sync {
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResult>;   // ← single Content, NO tool_use — cannot drive a tool loop
}
```

**Load-bearing deviation (Open Q1 / Pitfall 1):** `CompletionSource` MUST return `CreateMessageResultWithTools` (`src/types/sampling.rs:339` — carries `Vec<SamplingMessageContent>` with `ToolUse`/`ToolResult`), not `CreateMessageResult` (`:293` — single `content: Content`). Reuse `CreateMessageParams` (`:200`) verbatim as input. Shape:
```rust
#[async_trait]
pub trait CompletionSource: Send + Sync {
    async fn create_message(
        &self,
        params: pmcp::types::sampling::CreateMessageParams,
    ) -> Result<pmcp::types::sampling::CreateMessageResultWithTools, CompletionError>;
}
```
Error type via `thiserror` (workspace convention); expose a `retry_class(&self) -> RetryClass` accessor (see `result.rs`).

---

### `src/seams/store.rs` (trait + `InMemoryStore`, CRUD) — AGNT-01/D-06

**Analog:** `crates/pmcp-tasks/src/store/backend.rs` `StorageBackend` trait + `InMemoryBackend`, and `store/mod.rs:220` `TaskStore` async-trait shape.

**Trait doc + thread-safety pattern** (backend.rs is a "dumb KV" — mirror the load/save split; store/mod.rs:208–219 documents `Send + Sync` for concurrent handlers):
```rust
#[async_trait]
pub trait ConversationStore: Send + Sync {          // D-06: history + loop iteration state
    async fn load(&self, run_id: &str) -> Result<Option<RunState>, StoreError>;
    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), StoreError>;
}
```
`InMemoryStore` copies `InMemoryBackend`'s `DashMap`-backed trivial impl (pmcp-tasks uses `dashmap`/`parking_lot`; for a laptop-default in-mem store a `parking_lot::Mutex<HashMap>` is enough). `RunState` must be `Serialize + Deserialize` with **no floats, counters not timestamps** (Pitfall 3) — mirror `VersionedRecord`'s "serialized bytes + monotonic version" discipline (backend.rs lines 30–58) if optimistic-concurrency/resume is needed.

**StoreError** taxonomy: copy `StorageError` enum shape (backend.rs:82) via `thiserror`.

---

### `src/loop/result.rs` + `src/loop/decide.rs` (data types + pure fns, transform) — AGNT-02/D-05

**Analog:** `src/types/tasks.rs:349` `Task::poll_decision()` — THE "classification as data" precedent. This is the single most important pattern in the phase.

**The exhaustive, no-`_`-arm, replay-deterministic classifier** (tasks.rs lines 314–361) — mirror this shape for BOTH the retry classifier and end-turn detection:
```rust
// The rustdoc contract to copy verbatim in spirit (tasks.rs:314–335):
//   "pure, total function ... no `_` wildcard arm because [the enum] is
//    exhaustive ... so the mapping cannot silently drift ... replay-deterministic
//    and safe to call inside a memoized durable/replay step."
pub fn poll_decision(&self) -> TaskPollDecision {
    match self.status {
        TaskStatus::Working => TaskPollDecision::InProgress { poll_hint: self.poll_interval },
        TaskStatus::InputRequired => TaskPollDecision::InputRequired,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled =>
            TaskPollDecision::Terminal { status: self.status },
    }
}
```

**Apply to `decide.rs`** as separate pure fns (keeps each under PMAT cog ≤25, Pitfall 5):
```rust
fn is_end_turn(stop_reason: Option<&str>) -> bool {         // reference iteration.rs:383 shape
    matches!(stop_reason, Some("end_turn" | "stop"))
}
// evaluate_submit_result(...) — takes NO stop_reason (reference iteration.rs:1899);
// only relevant when AgentPackage.output_schema.is_some().
// check_limits(iteration_idx, max_iterations, tokens_used, max_tokens) -> Continue | Stop
```

**Apply to `result.rs`** — `IterationResult` (Serialize+Deserialize for checkpoint round-trip) and the retry enum, both `#[non_exhaustive]` (mirror `TaskPollDecision`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    pub assistant_message: Message,
    pub tool_results_message: Option<Message>,
    pub is_final: bool,
}

#[non_exhaustive]
pub enum RetryClass {                       // reference two-class split; NO backoff policy here
    Fatal,
    Transient { attempt_hint: u32 },        // 5xx
    Capacity  { attempt_hint: u32 },        // 429/529 — caller uses a LONGER wait
}
```
**Discipline (D-05):** no `std::time::now`, no RNG anywhere in `decide.rs`/`result.rs` — iteration counters only. This is what makes AGNT-03 hold.

---

### `src/loop/engine.rs` (orchestrator, event-driven) — AGNT-02

**Analog:** the reference `iteration.rs::execute_iteration` (read-only, D-09 — **do not copy**) for shape; `tests/common/duplex.rs` for the "thin async orchestrator awaiting seams" loop style.

**Pattern:** keep the engine a thin `loop { }` that only awaits the three seams and delegates every between-await decision to `decide.rs` pure fns (Pitfall 5 — do NOT write one big `execute_iteration` with `#[allow]`s). Sequence per the RESEARCH architecture diagram (lines 158–185): build params (pure) → `CompletionSource::create_message` (await) → classify (pure) → `ToolInvoker::invoke_batch` (await) → digest (pure) → `ConversationStore::save` (await) → `check_limits` (pure).

---

### `src/sources/sampling.rs` (seam impl, request-response) — AGNT-04

**Analog:** `src/server/peer_impl.rs:53` `DispatchPeerHandle` — dispatches a `ServerRequest` and decodes the JSON result. `SamplingSource` wraps a peer and implements `CompletionSource`.

**Exact analog to extend** (peer_impl.rs lines 53–66 — currently returns `CreateMessageResult`; the Open-Q1 gap):
```rust
#[async_trait]
impl PeerHandle for DispatchPeerHandle {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        let value = self.dispatcher
            .dispatch(ServerRequest::CreateMessage(Box::new(params)))
            .await?;
        serde_json::from_value::<CreateMessageResult>(value)          // ← decode target is the gap
            .map_err(|e| Error::protocol(ErrorCode::INTERNAL_ERROR, format!("Invalid sample response: {e}")))
    }
    // ...
}
```
**Paired `pmcp` 2.17.0 addition (Wave 0, resolves Open Q1):** add `sample_with_tools(params) -> CreateMessageResultWithTools` on `PeerHandle`/`DispatchPeerHandle` — SAME `dispatch(ServerRequest::CreateMessage(...))` call, only the `from_value::<_>` decode target changes to `CreateMessageResultWithTools`. `SamplingSource::create_message` calls that. Zero-dep; rides the D-106-A fix so the round-trip completes on the stock server loop.

---

### `src/invoker/client.rs` (seam impl, request-response + polling) — AGNT-08

**Analog:** `src/client/mod.rs:771` `Client::wait_for_task` — already wraps `poll_decision`, budget clamp, and `input_required` handling. **Don't hand-roll a poll loop.**

**Exact composition to copy** (mod.rs lines 771–800 pattern; the doctest at 771 shows the call site):
```rust
if let Some(meta) = call_result.related_task() {
    let final_result = client
        .wait_for_task(&meta.task_id, WaitForTaskOptions::default())
        .await?;                                  // drives poll_decision internally
}
```
`wait_for_task` returns `Err` on `input_required` (mod.rs:793 — correct for a short-lived non-durable SDK invoker; a durable host substitutes its own polling). `invoke_batch` default impl loops sequentially; SDK impl uses `futures::future::join_all` (D-07).

---

### `src/adapter/server.rs` (adapter, request-response) — AGNT-07/D-10/D-11/D-12

**Analog:** `src/server/builder.rs:766` `with_task_store` (wire the TaskStore) + `src/server/mod.rs:278` `ToolOutput::Result` (own the full `CallToolResult` envelope, no raw JSON-RPC bypass).

**Wire the task store** (builder.rs:766):
```rust
pub fn with_task_store(mut self, router: Arc<dyn TaskRouter>) -> Self { /* ... */ }
```

**Return task-augmented result** (mod.rs:246–278 `ToolOutput` enum; RESEARCH code example lines 409–416):
```rust
use pmcp::server::ToolOutput;
use pmcp::types::CallToolResult;
let result = CallToolResult::default().with_related_task(&related_task_meta);  // top-level _meta.related_task (SEP-1686)
Ok(ToolOutput::Result(result))                                                // handler owns full envelope (D-10)
```
ONE package-driven tool: name/description/`input_schema`/`output_schema` come from `AgentPackage` (D-11). Adapter holds NO cross-call state — fresh run per call (D-12). Builds on `ServerCore`/`ServerCoreBuilder` so it inherits Lambda/Docker/WASM adapters unchanged.

---

### `src/config/resolver.rs` (trait + 2 impls, transform) — AGNT-09/D-14/D-15/D-16

**Analog:** `crates/pmcp-package/src/slot/deviation.rs:28` `detect_deviation` for the warn-and-run path; `AgentPackage` (`package/agent.rs:41`) for the config source.

**Deviation warn-and-run** (deviation.rs:28 returns `Option<Deviation>`; D-15 — warn, never hard-error):
```rust
if let Some(dev) = pmcp_package::slot::detect_deviation(&tested_slot, &proposed_slot) {
    tracing::warn!(slot = %dev.slot_name, tested = %dev.tested, running = %dev.proposed,
        "config deviates from tested value — running anyway (D-15)");
}   // proceed regardless
```
`SlotResolver` trait with two shipped impls (env-var-convention + programmatic builder). Never `tracing` API keys (Security V7). `endpoint.rs` maps `AgentPackage.connectors` (`Vec<ComponentRef>` ranges, agent.rs:52–55) → name→URL/command map (D-16); `ToolInvoker` connects `pmcp::Client`s from it.

---

### `src/trace.rs` (serde artifact, transform) — AGNT-03/D-08

**Analog:** `TaskPollDecision` serde derive + `store/backend.rs:49` `VersionedRecord`. `EffectTrace` is a public `#[derive(Serialize, Deserialize)]` recording the ordered sequence of effect results. Use `serde_json` with `preserve_order`, no floats (Pitfall 3). Golden traces as `tests/fixtures/*.json`. A `ReplaySource`/`ReplayInvoker` reads from a trace instead of doing I/O so proptest can assert identical-inputs→identical-decisions.

---

### `tests/real_loop_sampling.rs` (test) + pmcp `tests/in_tool_peer_roundtrip.rs` — D-03/D-106-A

**Analog:** `tests/common/duplex.rs` — `DuplexTransport::pair()` + `call_via_server` (lines 86–100) drives a real `Client` against a real `Server::run()`. **This is the D-03 harness convention.** Extend it: a tool handler that awaits `extra.peer().sample()` (and `.elicit()`, `.list_roots()`), a client built with `on_sampling`, asserting the round-trip completes on the stock loop. Include this module per-crate via `#[path = "common/duplex.rs"] mod duplex;` (duplex.rs line 11).

---

### `examples/s50_standalone_vs_sampled.rs` (example)

**Analog:** `examples/s49_sampling_host.rs` (host sampling) + `examples/s43_handler_peer_sample.rs` (in-tool `peer.sample()`). **Verified next free number is `s50`** (s49 is the highest existing). SAME loop, two sources: a standalone runner on a mock/`OpenAiCompatSource` and the same agent hosted-and-sampled. Live-LLM path must degrade to a mock in CI (no Docker/live-service in default tests — house rule).

---

## Shared Patterns

### Async trait definition
**Source:** every seam trait in the SDK — `src/client/host/sampling.rs:36`, `crates/pmcp-tasks/src/store/mod.rs:220`
**Apply to:** all three seams + `SlotResolver`
```rust
#[async_trait]
pub trait X: Send + Sync { async fn ... }
```
All object-safe (`Arc<dyn X>`), `#[async_trait]`, `Send + Sync` (documented for concurrent handler access — store/mod.rs:208–212).

### Classification-as-data (no policy inside the loop)
**Source:** `src/types/tasks.rs:349` `poll_decision` + `TaskPollDecision`
**Apply to:** `RetryClass`, `is_end_turn`, `check_limits`, `EffectTrace` decisions
Exhaustive `match` with **no `_` arm** (in-crate enums), `#[non_exhaustive]` for the public enum, pure/total, no wall-clock/RNG. The loop returns classifications as data; the caller (standalone runner → `tokio::sleep`, platform → `ctx.wait`) interprets them. NO `sleep`/backoff in `pmcp-agent`.

### Error taxonomy
**Source:** `thiserror` workspace convention; `crates/pmcp-tasks/src/store/backend.rs:82` `StorageError`, `pmcp-tasks/src/error.rs` `TaskError`
**Apply to:** `CompletionError`, `StoreError`, invoker/resolver errors
`#[derive(thiserror::Error)]` enums; expose a `retry_class()` accessor where a caller needs to classify (Claude's discretion: shared accessor vs per-seam type).

### Serde / replay determinism
**Source:** `serde_json` `features = ["preserve_order"]` (pmcp-tasks Cargo.toml:12, :38); `AgentPackage` no-bare-floats policy (package/agent.rs:17–33)
**Apply to:** `RunState`, `IterationResult`, `EffectTrace`
No `f32`/`f64` (breaks canonicalization + replay); counters not timestamps; `preserve_order` on every serde_json feature list.

### Never log secrets
**Source:** host surface sanitizes handler errors to `-32603` (sampling.rs:44 rustdoc); pmcp-package identity-slots structurally hold no values
**Apply to:** `OpenAiCompatSource`/`AnthropicSource`/`SlotResolver`
API keys from env/`SlotResolver` only; never in `Debug`/`tracing`.

## No Analog Found

Genuinely new SDK surfaces — the planner should treat these as design work, guided by RESEARCH not a copy-source:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `src/server/mod.rs` `spawn_message_handler` (D-106-A pump) | core fix | event-driven | No existing split-receive/serialized-worker loop exists; the current loop (lines 1043–1147) blocks on `handle_request` inline at line 1144 — that IS the deadlock. Response routing already exists (line 1093–1117), so the change is making the receive loop not block on request execution. Recommended shape in RESEARCH lines 329–345 (dedicated receive loop routes `Response`→`dispatcher.handle_response` immediately, `Request`→bounded mpsc→single sequential worker). D-02: pump responses only, request handling stays serialized. |
| `PeerHandle::sample_with_tools` (Open Q1 addition) | core addition | request-response | Extends `DispatchPeerHandle::sample` (peer_impl.rs:55) with a `CreateMessageResultWithTools` decode target — the analog is the existing `sample`, but the WithTools peer return path does not exist yet. Ships in the same 2.17.0 train. |

## Verified Facts For The Planner

- Root `pmcp` version is **2.16.0** (`Cargo.toml:3`) → bump to **2.17.0** per D-04.
- Workspace members line is `Cargo.toml:582`; add `"crates/pmcp-agent"` (regular member — A4: only root `pmcp` is clippy-gated, so a regular member is fine).
- Next free example number is **`s50`** (verified: `s49_sampling_host.rs` is highest).
- `HostSamplingHandler::handle_create_message` (sampling.rs:44) and `PeerHandle::sample` (peer_impl.rs:55) BOTH return `CreateMessageResult` — confirms the Open-Q1 gap across the host and peer paths.
- `CreateMessageResultWithTools` at `src/types/sampling.rs:339` is the tool-carrying result type; `CreateMessageParams` at `:200` is already tool-calling-complete (`tools`/`tool_choice` fields at :224–229).
- `with_task_store` at `src/server/builder.rs:766` takes `Arc<dyn TaskRouter>`.

## Metadata

**Analog search scope:** `crates/pmcp-tasks/` (isolation + async-trait + store precedent), `crates/pmcp-package/` (AgentPackage, slot deviation), `src/types/` (sampling, tasks), `src/server/` (core loop, peer, builder, ToolOutput), `src/client/` (host surface, wait_for_task), `tests/common/`, `examples/`
**Files scanned:** ~14 read in full/targeted ranges + 6 grep passes
**Pattern extraction date:** 2026-07-17
