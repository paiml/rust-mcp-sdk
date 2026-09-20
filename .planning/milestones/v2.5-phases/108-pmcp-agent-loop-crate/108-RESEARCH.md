# Phase 108: `pmcp-agent` Loop Crate - Research

**Researched:** 2026-07-17
**Domain:** Rust agent-loop runtime; MCP sampling/tasks; server concurrency; effect-seam trait design
**Confidence:** HIGH (all claims verified against in-repo code + the reference implementation; no external package discovery required)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-106-A fix (pmcp core, this phase)**
- **D-01:** The fix lands **in `pmcp` core**, not as an adapter-local workaround.
- **D-02:** Concurrency semantics: **pump responses only**. Request handling stays serialized (zero behavior change); the message loop keeps routing inbound RESPONSES to pending peer requests while a handler awaits. A tool awaiting `peer.sample()` / `.elicit()` / `.list_roots()` gets its answer.
- **D-03:** Proof standard: **real-loop end-to-end** — hosted-agent example and tests run through the REAL `Server::run` + a real `Client` with `on_sampling`, no raw pump. Phase 106's duplex raw-pump tests stay.
- **D-04:** Release pairing: D-106-A ships as a **`pmcp` minor bump (2.17.0) in the same release train as `pmcp-agent` 0.1.0**, which pins the new pmcp version; cargo-pmcp scaffold-pin tripwire updated.

**Loop shape & replay contract**
- **D-05:** **Async loop generic over the seams + extracted pure decision functions.** Between awaits ALL logic is pure functions (end-turn detection, result digestion, retry classification, iteration/budget limits) on data types. No wall-clock, no randomness — counters, not time.
- **D-06:** `ConversationStore` holds **message history + loop iteration state** (counters, pending tool calls) so a run is resumable mid-iteration; `InMemoryStore` stays trivial.
- **D-07:** Parallel tool dispatch crosses the seam as a **batch method** (`invoke_batch(Vec<ToolCall>) -> Vec<Result>`; default sequential, SDK impl `join_all`). Platform maps ONE seam call onto durable `ctx.map`.
- **D-08:** Recorded effect traces are a **public serde artifact** (`EffectTrace` type): proptest generates them for AGNT-03; golden traces as fixtures.
- **D-09:** **The pmcp.run durable-agent-lambda is the reference implementation and validation target.** Design the crate API so the durable lambda could adopt it and DELETE code. Deliver a shape-compatibility mapping. No private-repo code copied.

**Agent-as-server adapter**
- **D-10:** Agent runs are **task-augmented by default** (SEP-1686): adapter's tool creates a task, returns `ToolOutput::Result` with top-level `related_task` `_meta` immediately, run progresses via `tasks/get` polling. Short runs may complete synchronously.
- **D-11:** Tool surface: **one conversational tool, package-driven** — name/description/input/output schemas from the `AgentPackage`; default = simple message input.
- **D-12:** Conversation semantics: **fresh run per tool call**; continuity lives in the stores. Adapter stateless per call.
- **D-13:** AGNT-07 deploy proof: **native example + CI wasm32 compile gate** (sans feature-gated HTTP sources) — no per-target deploy demos.

**Package config resolution (AGNT-09)**
- **D-14:** Slot resolution is a **seam: `SlotResolver` trait** in `pmcp-agent`, two shipped impls: env-var-convention resolver + explicit programmatic builder.
- **D-15:** Deviation from `tested_value`: **warn and run** — log "tested on X, running on Y" and proceed.
- **D-16:** Connector refs resolve via an **endpoint map supplied by the resolver** (name → URL/command); `ToolInvoker` connects `pmcp::Client`s from that map.

### Claude's Discretion
- Exact trait/type names, module layout, feature-flag names, builder API shapes
- Retry-classification enum shape (mirror the `TaskPollDecision` "classification as data" precedent)
- Error taxonomy across the three seams (shared `RetryClass` accessor vs per-seam error types)
- `SamplingSource` wiring details over the Phase 106 host surface / server-side peer
- OpenAI-compat and Anthropic source internals (reqwest usage, no streaming requirement)
- TaskStore wiring for the adapter (reuse `with_task_store()`; in-memory default)
- How the D-106-A response-pump is implemented inside `Server::run` (select-based, split task, etc.) — semantics in D-02 are the contract
- Example naming/numbering (`sNN_` convention)

### Deferred Ideas (OUT OF SCOPE)
- Per-target deploy demos (Lambda/Docker/WASM) of an agent — Phase 110/111
- `pmcp.toml` slot-resolver wiring — Phase 110
- Capture-and-replay debug tooling over `EffectTrace` — future
- Streaming completions in `OpenAiCompatSource`/`AnthropicSource` — not required
- AgentCore deploy adapter (DEFER-01), additional sources (DEFER-02), platform migration (DEFER-04)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AGNT-01 | Object-safe async effect-seam traits `CompletionSource`/`ToolInvoker`/`ConversationStore`; `CompletionSource` reuses SDK sampling types verbatim | Seam trait design (§Architecture Pattern 1); `CreateMessageParams` verified at `src/types/sampling.rs:200`. **Critical: result type must be `CreateMessageResultWithTools` (line 339), not `CreateMessageResult`** — see Open Question 1 |
| AGNT-02 | Iteration loop runs pure between seams; retry classification exposed as data (no retry/backoff policy inside) | Loop shape mirrors reference `IterationResult` + pure `evaluate_submit_result`; retry-class-as-data mirrors `TaskPollDecision` (Pattern 2, Pattern 4) |
| AGNT-03 | Replay-safety property-tested (proptest over recorded effect traces) | `EffectTrace` public serde artifact (D-08); proptest 1.7 in workspace (Pattern 5) |
| AGNT-04 | `SamplingSource` implements `CompletionSource` over spec sampling via server-side peer, zero deps | `extra.peer().sample()` at `src/server/peer_impl.rs:55`; rides D-106-A fix. **Peer return-type gap** — Open Question 1 |
| AGNT-05 | `OpenAiCompatSource` (feature-gated) over any OpenAI-compatible endpoint | reqwest 0.13 (workspace-pinned); reference `llm/service.rs` transformer shapes (§Standard Stack) |
| AGNT-06 | `AnthropicSource` (feature-gated) over Anthropic Messages API | Same as AGNT-05; feature `anthropic` |
| AGNT-07 | Agent-as-server adapter on `ServerCore`, deployable via existing target adapters | `ServerCore`/`ServerCoreBuilder` (`src/server/core.rs`, `builder.rs`); native example + wasm32 compile gate (D-13) |
| AGNT-08 | `ToolInvoker` over `pmcp::Client` honors task-augmented results via `poll_decision` (SEP-1686) | `Task::poll_decision()` (`src/types/tasks.rs:349`), `Client::wait_for_task` (`src/client/mod.rs:771`); reference `classify` pattern (Pattern 3) |
| AGNT-09 | Agent configured from `AgentPackage` + resolved config slots | `AgentPackage` (`crates/pmcp-package/src/package/agent.rs:41`), `ConfigSlot`/`SlotType` (`slot/types.rs`), `SlotResolver` seam (D-14) |
</phase_requirements>

## Summary

Phase 108 ships `crates/pmcp-agent` — a new 0.x experimental workspace member — plus a paired `pmcp` 2.17.0 minor bump that fixes the D-106-A server-loop deadlock. The crate is a **pure agent decision loop between three object-safe async effect seams** (`CompletionSource`, `ToolInvoker`, `ConversationStore`), with three shipped `CompletionSource`s (sampling-first: zero-dep `SamplingSource`, feature-gated `OpenAiCompatSource` and `AnthropicSource`), an agent-as-server adapter on `ServerCore`, a tasks-aware `ToolInvoker`, and package-driven configuration via a `SlotResolver` seam.

The design is not greenfield: the pmcp.run `durable-agent-lambda/src/handler/iteration.rs` (3,468 lines) is a **battle-tested reference** whose decomposition the crate must formalize. Its `IterationResult { llm_response, assistant_message, tool_results_message, is_final }` value type, its pure `evaluate_submit_result` decision function (deliberately takes no `stop_reason`), its `is_end_turn = stop_reason ∈ {"end_turn","stop"}` match, its `ctx.map` parallel dispatch, and its two-class retry split (transient 500/502/503 vs capacity 429/529) are the exact shapes to lift into pure functions and data-typed classifications. The "non-determinism inside the step, pure classification outside" discipline is already proven by the 2.13.0 `poll_decision` design — the crate generalizes it.

**Primary recommendation:** Build the loop as an async engine generic over the seams, with every between-await decision extracted into a pure, unit-testable, `Serialize`-able function operating on data types that mirror the reference `IterationResult`/`SubmitResultDecision`/retry-class shapes. Reuse the SDK sampling types verbatim — but resolve the `CreateMessageResultWithTools` vs `CreateMessageResult` peer-return-type gap (Open Question 1) **first**, because it gates whether `SamplingSource` can drive a tool-calling agent at all.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Agent decision loop (end-turn, digestion, budget) | pmcp-agent crate (pure fns) | — | Pure between seams; no I/O — this IS the extracted contract |
| LLM completion | `CompletionSource` seam | pmcp Client host / reqwest | Sampling-first: hosted → server-side peer; standalone → HTTP source |
| Tool invocation | `ToolInvoker` seam | pmcp `Client` + Tasks | Downstream MCP servers; task-augmented via `poll_decision` |
| Conversation/loop state | `ConversationStore` seam | InMemoryStore (SDK) / DDB (platform) | Resumable mid-iteration state lives behind the seam |
| Agent-as-server exposure | `ServerCore` adapter | Lambda/Docker/WASM target adapters | Reuses existing transport adapters unchanged |
| Server→client request pump (D-106-A) | `pmcp` core `Server::run` | — | Core concurrency fix; every in-tool-sampling server benefits |
| Package → runtime config | `SlotResolver` seam | pmcp-package `AgentPackage`/`ConfigSlot` | Config resolution is host policy; loop consumes resolved values |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` (path + version) | `2.17.0` (this train) | Sampling types, `ServerCore`, `Client`, Tasks, host surface | The SDK this crate extends; reuse types verbatim `[VERIFIED: root Cargo.toml]` |
| `pmcp-package` (path + version) | `0.1` (caret) | `AgentPackage`, `ConfigSlot`, `detect_deviation` | Phase 107 wire-frozen definition format `[VERIFIED: crates/pmcp-package]` |
| `async-trait` | `0.1` | Object-safe async seam traits | Every async trait in the SDK uses it `[VERIFIED: workspace]` |
| `serde` / `serde_json` | `1.0` (`preserve_order`) | `EffectTrace`, message types, package config | Protocol-wide convention `[VERIFIED: workspace]` |
| `futures` | `0.3` | `join_all` for the batch `ToolInvoker` default | Already a core pmcp dep `[VERIFIED: root Cargo.toml:73]` |
| `tracing` | `0.1` | D-15 deviation warnings, loop diagnostics | Workspace convention `[VERIFIED]` |
| `thiserror` | `2.0` | Seam error taxonomy | Workspace convention `[VERIFIED]` |

### Supporting (feature-gated)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | `0.13`, `default-features=false`, features `["json","rustls"]` | `OpenAiCompatSource` / `AnthropicSource` HTTP | ONLY under `openai-compat` / `anthropic` features (keeps wasm32 clean per D-13) `[VERIFIED: root Cargo.toml:135]` |
| `chrono` / `uuid` | `0.4` / `1.17` | Task IDs, message timestamps if needed | Match pmcp-tasks Cargo.toml `[VERIFIED: crates/pmcp-tasks/Cargo.toml]` |

### Dev-dependencies
| Library | Version | Purpose |
|---------|---------|---------|
| `proptest` | `1.7` | AGNT-03 replay-safety property + fuzz-style generation `[VERIFIED: root Cargo.toml:149]` |
| `pmcp` (features `full`) | `2.17.0` | Real-loop end-to-end tests (D-03) |
| `pretty_assertions`, `tokio` (`full`) | `1.4`, `1` | Test ergonomics (match pmcp-tasks) |

**Installation:** No `cargo add` from external registries. All dependencies are workspace-internal or already-pinned workspace crates. New `crates/pmcp-agent/Cargo.toml` mirrors `crates/pmcp-tasks/Cargo.toml` structure (verified isolation precedent).

**Version verification:**
```bash
grep '^version' /Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml   # pmcp 2.16.0 → bump to 2.17.0
```
`pmcp` is at 2.16.0 today (post-Phase-106); D-04 requires the 2.17.0 bump paired with `pmcp-agent` 0.1.0. `[VERIFIED: root Cargo.toml:3]`

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| reqwest for HTTP sources | `hyper` directly | reqwest is already the workspace HTTP client (mcp-tester, openapi-server, toolkit all use 0.13); no reason to diverge |
| Custom retry loop in crate | — | FORBIDDEN by AGNT-02/D-05: retry classification is data, not policy. Host owns backoff |
| Folding into `pmcp` core | — | FORBIDDEN: 0.x experimental isolation (pmcp-tasks precedent, REQUIREMENTS out-of-scope table) |

## Package Legitimacy Audit

> No external packages are introduced. All dependencies are workspace-internal (`pmcp`, `pmcp-package`) or already-pinned workspace dependencies verified present in the resolved tree.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `reqwest` | crates.io | mature | very high | github.com/seanmonstar/reqwest | n/a — already in workspace | Approved (already pinned `0.13`) |
| `async-trait`, `serde`, `futures`, `proptest`, `thiserror`, `tracing` | crates.io | mature | very high | established | n/a — already in workspace | Approved |
| `pmcp`, `pmcp-package` | crates.io / path | — | — | this repo | n/a — first-party | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*No new registry lookups performed — every dependency already appears in `Cargo.toml`/workspace lock and is transitively resolved today. slopcheck not run because no new package names are introduced (its purpose — catching hallucinated names — is moot when the dependency set is a subset of the existing verified workspace).*

## Architecture Patterns

### System Architecture Diagram

```
  ┌─────────────────────────────────────────────────────────────────┐
  │                        AGENT RUN (fresh per call, D-12)           │
  │                                                                   │
  │  AgentPackage + SlotResolver ──► resolved config (instructions,   │
  │        │  (D-14/15/16)                model prefs, connectors,     │
  │        │                              limits, endpoint map)        │
  │        ▼                                                           │
  │  ┌──────────────── async iteration engine (D-05) ──────────────┐  │
  │  │  loop {                                                      │  │
  │  │    build CreateMessageParams (history + tools) ── PURE       │  │
  │  │        │                                                     │  │
  │  │        ▼  await                                              │  │
  │  │    CompletionSource::create_message ═════════╗ effect seam   │  │
  │  │        │                                     ║               │  │
  │  │        ▼                                      ╠══► SamplingSource (peer)
  │  │    classify response ── PURE:                 ╠══► OpenAiCompatSource
  │  │      • is_end_turn (stop_reason match)        ╚══► AnthropicSource
  │  │      • evaluate_submit_result (output_schema) │  │            │  │
  │  │      • has_tool_calls?                        │  │            │  │
  │  │        │ final? ──yes──► digest & return IterationResult{is_final}
  │  │        │ no                                                   │  │
  │  │        ▼  await                                              │  │
  │  │    ToolInvoker::invoke_batch(Vec<ToolCall>) ═╗ effect seam    │  │
  │  │        │  (D-07: join_all SDK / ctx.map plat)║               │  │
  │  │        │                                      ╚══► pmcp::Client + Tasks
  │  │        ▼                                          (poll_decision, AGNT-08)
  │  │    digest results ── PURE ──► tool_results_message           │  │
  │  │        │                                                     │  │
  │  │        ▼  await                                              │  │
  │  │    ConversationStore::append + save iter state ═╗ effect seam │  │
  │  │        │  (D-06 resumable)                       ╚══► InMemory/DDB
  │  │        ▼                                                     │  │
  │  │    check iteration/budget limits ── PURE ──► continue|stop   │  │
  │  │  }                                                           │  │
  │  └─────────────────────────────────────────────────────────────┘  │
  └─────────────────────────────────────────────────────────────────┘

  Agent-as-server (AGNT-07, D-10/11):
    ServerCore adapter ──► one package-driven tool ──► spawns AGENT RUN
      └─► returns ToolOutput::Result with top-level related_task _meta (SEP-1686)
          └─► deployable via existing Lambda/Docker/WASM adapters

  D-106-A fix (pmcp core): Server::run message loop pumps inbound RESPONSES
    to the peer dispatcher while a tool handler awaits peer.sample()/.elicit()
```

### Recommended Project Structure
```
crates/pmcp-agent/
├── Cargo.toml            # mirror pmcp-tasks: pmcp path+version, features openai-compat/anthropic
├── src/
│   ├── lib.rs
│   ├── seams/
│   │   ├── completion.rs # CompletionSource trait (reuses CreateMessageParams verbatim)
│   │   ├── tool.rs       # ToolInvoker trait + invoke_batch (D-07)
│   │   └── store.rs      # ConversationStore trait + InMemoryStore (D-06)
│   ├── loop/
│   │   ├── engine.rs     # async iteration engine (the only I/O-bearing code)
│   │   ├── decide.rs     # PURE decision fns: end-turn, submit_result, digestion, limits
│   │   └── result.rs     # IterationResult, RetryClass, loop state data types (Serialize)
│   ├── sources/
│   │   ├── sampling.rs    # SamplingSource (zero-dep, over server-side peer) AGNT-04
│   │   ├── openai_compat.rs # feature "openai-compat" AGNT-05
│   │   └── anthropic.rs     # feature "anthropic" AGNT-06
│   ├── adapter/
│   │   └── server.rs     # agent-as-server on ServerCore AGNT-07/D-10/11
│   ├── invoker/
│   │   └── client.rs     # tasks-aware ToolInvoker over pmcp::Client AGNT-08
│   ├── config/
│   │   ├── resolver.rs   # SlotResolver trait + EnvVarResolver + ProgrammaticBuilder D-14
│   │   └── endpoint.rs   # connector-ref → endpoint map D-16
│   └── trace.rs          # EffectTrace public serde artifact D-08
├── tests/                # real-loop end-to-end (D-03), conformance vs contracts
└── examples/             # sNN_standalone_vs_sampled.rs (next free: s50+)
```

### Pattern 1: Object-safe async effect seam (AGNT-01)
**What:** Three traits, all `Send + Sync`, `#[async_trait]`, object-safe (`Arc<dyn ...>`), reusing SDK sampling types verbatim.
**When to use:** Every effect the loop performs crosses one of these three seams — nothing else.
**Example (shape — names are Claude's discretion):**
```rust
// CompletionSource reuses SDK types verbatim (AGNT-01). NOTE the result type
// choice is load-bearing — see Open Question 1.
#[async_trait]
pub trait CompletionSource: Send + Sync {
    async fn create_message(
        &self,
        params: pmcp::types::sampling::CreateMessageParams,
    ) -> Result<pmcp::types::sampling::CreateMessageResultWithTools, CompletionError>;
    //          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ tool_use blocks REQUIRED for an
    //          agent loop; CreateMessageResult (single Content) cannot carry them.
}

#[async_trait]
pub trait ToolInvoker: Send + Sync {
    // D-07: batch is THE seam method; default impl loops sequentially,
    // SDK impl uses futures::future::join_all, platform maps onto ctx.map.
    async fn invoke_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolCallResult>;
}

#[async_trait]
pub trait ConversationStore: Send + Sync {
    // D-06: holds message history + loop iteration state (counters, pending
    // tool calls) so a run resumes mid-iteration.
    async fn load(&self, run_id: &str) -> Result<Option<RunState>, StoreError>;
    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), StoreError>;
}
```

### Pattern 2: Pure decision functions between awaits (D-05, AGNT-02)
**What:** Every between-await computation is a synchronous, side-effect-free function on data types. Mirror the reference `IterationResult` and `evaluate_submit_result` shapes.
**Reference shapes (from `iteration.rs`, describe — do not copy):**
```rust
// Mirror of the reference IterationResult (types.rs:1016) — Serialize+Deserialize
// for replay/checkpoint round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    pub llm_response: /* CreateMessageResultWithTools or a crate mirror */,
    pub assistant_message: Message,
    pub tool_results_message: Option<Message>,
    pub is_final: bool,
}

// End-turn detection is a PURE stop_reason match (reference iteration.rs:383):
fn is_end_turn(stop_reason: Option<&str>) -> bool {
    matches!(stop_reason, Some("end_turn" | "stop"))
}

// Optional output-schema termination (reference evaluate_submit_result:1899)
// deliberately takes NO stop_reason — the decision cannot depend on a signal it
// never receives. Only relevant if AgentPackage.output_schema is Some.
```
**Key discipline:** No `std::time::now`, no RNG inside the loop — use iteration counters. This is what makes AGNT-03 replay-safety hold.

### Pattern 3: Tasks-aware ToolInvoker (AGNT-08)
**What:** The `pmcp::Client`-backed `ToolInvoker` calls `call_tool`, reads `result.related_task()`, and drives `poll_decision`/`wait_for_task` for long calls.
**Reference (`mcp/client.rs` `classify`, `src/types/tasks.rs:349`):**
```rust
// Client::wait_for_task already implements the poll loop over poll_decision
// (src/client/mod.rs:771). For a NON-durable SDK invoker, wait_for_task is the
// right primitive. For a durable host, expose the classification as data so the
// host maps it onto ctx.wait — but that is the platform's concern; the SDK
// invoker can call wait_for_task directly.
if let Some(meta) = result.related_task() {
    let final_result = client.wait_for_task(&meta.task_id, WaitForTaskOptions::default()).await?;
}
```
**Note:** The reference deliberately does NOT use `wait_for_task` (it errors on `input_required`, wrong for a durable/replay consumer — see `mcp/client.rs` `PollDecision::InputRequired` doc). The SDK invoker CAN use it (short-lived, non-durable). Design the invoker so a host can substitute its own polling if needed.

### Pattern 4: Retry classification as data (D-05, AGNT-02)
**What:** No retry/backoff policy in the loop. Classification is a data enum the caller interprets — exactly the `TaskPollDecision` precedent.
**Reference two-class split (`iteration.rs`):** class-1 transient (500/502/503, short backoff, max 2 attempts) vs class-2 capacity (429/529, long durable wait, max 3 attempts). Recommended enum:
```rust
#[non_exhaustive]
pub enum RetryClass {
    Fatal,                              // non-retryable — surface immediately
    Transient { attempt_hint: u32 },    // 5xx transient
    Capacity  { attempt_hint: u32 },    // 429/529 — caller uses a LONGER wait
}
// The loop returns this as data; a standalone runner maps it to tokio::sleep,
// the platform maps it to ctx.wait. NO sleep/backoff lives in pmcp-agent.
```

### Pattern 5: EffectTrace replay-safety property (D-08, AGNT-03)
**What:** A public `#[derive(Serialize, Deserialize)]` `EffectTrace` recording the sequence of effect results (completion results, tool results). proptest generates random-but-consistent traces; the property asserts: **feeding identical effect results to the loop yields identical decisions.** Golden traces live as `tests/fixtures/*.json`.
**Implementation:** A `ReplaySource`/`ReplayInvoker` that reads from an `EffectTrace` instead of doing I/O, so the "same loop" runs deterministically. This doubles as the future capture-and-replay tooling substrate (deferred).

### Pattern 6: Agent-as-server adapter (AGNT-07, D-10/11/12)
**What:** A `ServerCore`-based adapter exposing ONE package-driven tool that spawns a fresh agent run and returns task-augmented results.
```rust
// One tool, name/schema from AgentPackage (D-11). Handler returns
// ToolOutput::Result (src/server/mod.rs:278) so it owns the full CallToolResult
// envelope INCLUDING top-level related_task _meta (SEP-1686, D-10) — no raw
// JSON-RPC bypass. Uses CallToolResult::with_related_task().
// Wire TaskStore via ServerCoreBuilder::with_task_store() (builder.rs:766);
// in-memory default. Fresh run per call (D-12) — no cross-call state in adapter.
```

### D-106-A fix architecture (pmcp core, D-01/D-02)
**The deadlock (verified at `src/server/mod.rs:1043-1163`):** `spawn_message_handler` runs a single serialized loop: `receive → handle_transport_message → (for a Request) await server.handle_request INLINE`. When a tool handler awaits `peer.sample()`, the outbound request leaves via the separate `spawn_server_request_drain` task, but the client's RESPONSE arrives as a `TransportMessage::Response` that can only be read by the same loop — which is blocked awaiting `handle_request`. Hang.

**D-02-compliant fix (recommended shape):** Split the receive task from request execution. A dedicated receive loop reads every inbound `TransportMessage` and routes by kind:
- `Response` → `dispatcher.handle_response(...)` **immediately** (this unblocks the awaiting peer round-trip — the whole point).
- `Request` → forward to a bounded mpsc consumed by a **single** worker task that processes requests sequentially (preserves D-02 "request handling stays serialized, zero behavior change").
- `Notification` → existing cancellation handling.

This keeps request ordering/serialization identical while letting responses pump concurrently. The existing `handle_transport_message` already routes `Response` to the dispatcher (line 1108) — the change is making the receive loop not block on request execution. **Proof (D-03):** a real `Server::run` + real `Client` with `on_sampling`, a tool that awaits `extra.peer().sample()`, completing end-to-end (also cover `.elicit()` and `.list_roots()`).

### Anti-Patterns to Avoid
- **Putting sleep/backoff inside the loop** — violates AGNT-02/D-05. Classification is data; the caller sleeps.
- **Wall-clock or RNG inside the loop** — breaks AGNT-03 replay-safety. Use iteration counters.
- **Returning `CreateMessageResult` (single Content) from `CompletionSource`** — cannot carry `tool_use` blocks; the loop can never dispatch tools. Use `CreateMessageResultWithTools`.
- **Adapter holding conversation state across calls** — violates D-12 (fresh run per call); state lives in `ConversationStore`.
- **Copying `iteration.rs` code verbatim** — boundary razor (D-09). Formalize shapes, do not lift private-repo source.
- **Spawning tool handlers concurrently in the D-106-A fix** — D-02 requires request serialization unchanged; only pump responses.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Task polling loop | Custom poll+sleep+timeout | `Client::wait_for_task` (`src/client/mod.rs:771`) | Already wraps `poll_decision`, budget clamp, input_required handling |
| Task status → decision | `if status == Completed` chains | `Task::poll_decision()` (`src/types/tasks.rs:349`) | Exhaustive, replay-deterministic, non-drift precedent |
| related_task _meta placement | Raw JSON-RPC envelope | `ToolOutput::Result(CallToolResult::with_related_task(...))` (2.12.0) | This is exactly what obsoleted the platform's raw bypass |
| Sampling wire types | New request/result structs | `CreateMessageParams`/`CreateMessageResultWithTools` (`src/types/sampling.rs`) | Spec-correct, tool-calling-complete (MCP 2025-11-25) |
| Deviation detection | String compare of tested vs proposed | `detect_deviation` (`crates/pmcp-package/src/slot/deviation.rs:28`) | Handles identity-bearing short-circuit; returns typed `Deviation` |
| Parallel dispatch fan-out | Manual `tokio::spawn` + join | `futures::future::join_all` behind `invoke_batch` | Default seam impl; platform swaps for `ctx.map` |
| Transport/deploy adapters | New server plumbing | `ServerCore` + `ServerCoreBuilder` | Inherits Lambda/Docker/WASM adapters unchanged |

**Key insight:** Almost every hard primitive this loop needs already exists in `pmcp` (Tasks polling, sampling types, `ToolOutput::Result`, `ServerCore`) or `pmcp-package` (config slots, deviation). The crate's job is composition and the pure decision functions — NOT reimplementation. The single genuinely-new SDK surface is the D-106-A server-loop response pump.

## Common Pitfalls

### Pitfall 1: `CompletionSource` result type cannot carry tool calls
**What goes wrong:** Using `CreateMessageResult` (which has a single `content: Content` field) means the model's `tool_use` blocks have nowhere to go; the agent can never call a tool.
**Why it happens:** `PeerHandle::sample()` and `HostSamplingHandler::handle_create_message` BOTH currently return `CreateMessageResult` (verified `src/shared/peer.rs:57`, `src/server/peer_impl.rs:55`, `src/client/host/sampling.rs:44`). Only `CreateMessageResultWithTools` (line 339) carries `Vec<SamplingMessageContent>` with `ToolUse`/`ToolResult` variants.
**How to avoid:** Resolve Open Question 1 in planning. The `CompletionSource` trait must return the WithTools shape (or a crate mirror). `SamplingSource` needs a peer path that deserializes the raw `sampling/createMessage` result as `CreateMessageResultWithTools` — likely a small paired `pmcp` addition (`PeerHandle::sample_with_tools` or SamplingSource issuing `ServerRequest::CreateMessage` and decoding WithTools itself). This lands in the same 2.17.0 train.
**Warning signs:** Loop compiles but every agent run ends after one iteration with no tool dispatch; `tool_use` content silently dropped.

### Pitfall 2: Idle-host limitation of the Phase 106 client surface
**What goes wrong:** `SamplingSource` sampling works only while the client has an in-flight request; the client has no background receive loop (documented `src/client/host/mod.rs` "Idle-host limitation").
**Why it happens:** The pmcp `Client` reads inbound requests only while awaiting its own request's response.
**How to avoid:** The hosted-agent flow is inherently request-scoped: the client calls the agent's tool (`tools/call`), and the agent samples back DURING that call — so the client is always in-flight. This matches the intended flow. Verify the real-loop test (D-03) exercises exactly this nesting.
**Warning signs:** Sampling hangs when the client is idle — expected; do not design a flow that samples outside an in-flight client request.

### Pitfall 3: Replay non-determinism from checkpoint round-trip
**What goes wrong:** A loop-state type that isn't `Serialize + Deserialize` byte-stable breaks resumability and the AGNT-03 property.
**Why it happens:** `HashMap` iteration order, floats, or wall-clock fields leak non-determinism.
**How to avoid:** Loop state and `EffectTrace` use `serde_json` with `preserve_order`; NO floats (pmcp-package already enforces this crate-wide — `AgentPackage` carries no bare float; budget values are string-encoded); counters not timestamps. The reference `IterationResult` is explicitly documented "Must be Serialize + Deserialize for checkpoint round-trip (Pitfall 3 from research)."
**Warning signs:** proptest replay property fails intermittently; digests differ across identical inputs.

### Pitfall 4: wasm32 target contamination from HTTP sources (D-13)
**What goes wrong:** Pulling `reqwest` unconditionally breaks the wasm32 compile gate.
**How to avoid:** `reqwest` is a dependency ONLY under the `openai-compat`/`anthropic` features. `SamplingSource`, the loop, the adapter, and the invoker must compile on wasm32 with default features. Add a CI `cargo build --target wasm32-unknown-unknown` gate for the default feature set (mirror existing wasm gates).
**Warning signs:** wasm build fails referencing `tokio` net or `reqwest`.

### Pitfall 5: PMAT cognitive-complexity on the loop engine
**What goes wrong:** The async engine, if written as one big function like the reference `execute_iteration` (which carries `#[allow]`s), trips the CI PMAT cog ≤25 gate.
**How to avoid:** Extract each decision into its own pure function (Pattern 2 — which the design mandates anyway). If a dispatch/parse function is irreducibly complex, use the `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]` template (CLAUDE.md, hard cap cog 50). Keep the engine a thin orchestrator calling pure fns.
**Warning signs:** `pmat quality-gate --fail-on-violation --checks complexity` fails in CI.

## Runtime State Inventory

> N/A — this is a greenfield crate-creation phase (plus an additive core fix), not a rename/refactor/migration. No stored data, live-service config, OS-registered state, secrets, or build artifacts carry a string being renamed. The one adjacent concern (publish-order list + scaffold-pin tripwire) is covered under State of the Art / release impact, not runtime state.

## Code Examples

### Building the sampling params from history (verified type)
```rust
// Source: src/types/sampling.rs:200 (CreateMessageParams), :226 tools field
use pmcp::types::sampling::{CreateMessageParams, SamplingMessage};
let params = CreateMessageParams::new(messages)   // Vec<SamplingMessage>
    .with_system_prompt(agent_package.instructions.clone())
    .with_max_tokens(agent_package.max_tokens as u32)
    .with_tools(tool_infos)          // Option<Vec<ToolInfo>> — MCP 2025-11-25
    .with_tool_choice(ToolChoice::Auto);
```

### Task-augmented tool result on the adapter (verified API)
```rust
// Source: src/server/mod.rs:278 (ToolOutput::Result), SEP-1686 with_related_task
use pmcp::server::ToolOutput;
use pmcp::types::CallToolResult;
let result = CallToolResult::default()
    .with_related_task(&related_task_meta);   // top-level _meta.related_task
Ok(ToolOutput::Result(result))                 // handler owns full envelope (D-10)
```

### Consuming a task-augmented result in the invoker (verified API)
```rust
// Source: src/client/mod.rs:771 wait_for_task, src/types/tasks.rs:349 poll_decision
if let Some(meta) = call_result.related_task() {
    let final_result = client
        .wait_for_task(&meta.task_id, WaitForTaskOptions::default())
        .await?;                                // drives poll_decision internally
}
```

### Deviation warn-and-run (D-15, verified API)
```rust
// Source: crates/pmcp-package/src/slot/deviation.rs:28
if let Some(dev) = pmcp_package::slot::detect_deviation(&tested_slot, &proposed_slot) {
    tracing::warn!(slot = %dev.slot_name, tested = %dev.tested, running = %dev.proposed,
        "config deviates from tested value — running anyway (D-15)");
}
// proceed regardless (warn and run, not a hard error)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw JSON-RPC bypass to place `related_task` _meta | `ToolOutput::Result(CallToolResult)` | pmcp 2.12.0 | Adapter needs NO bypass (obsoletes platform's `team-mcp` hack) |
| Hand-rolled `is_terminal` task checks | `Task::poll_decision()` / `TaskPollDecision` | pmcp 2.13.0 | "Classification as data" precedent the retry-class enum mirrors |
| Client cannot answer server→client requests | Client host surface (`on_sampling`/`on_elicitation`/`on_roots`) | pmcp 2.16.0 (Phase 106) | `SamplingSource` builds on this; D-106-A completes the loop end-to-end |
| Single-Content sampling result | `CreateMessageResultWithTools` (tool_use/tool_result blocks) | MCP 2025-11-25 revision | Full agent turn expressible over spec sampling — but peer path still returns single-Content (Open Q1) |

**Deprecated/outdated:**
- The reference `iteration.rs` durable `ctx.step`/`ctx.map` wrappers are **platform-specific** — the SDK loop stays plain-async; the durability seam IS the trait boundary (design §3 property 1). Do not import durability into the crate.

**Release impact (D-04):** Publish-order list (CLAUDE.md) gains `pmcp-agent` **after `pmcp`** (before `pmcp-team-servers`/cargo-pmcp). `pmcp` bumps 2.16.0 → 2.17.0 (D-106-A minor); `pmcp-agent` 0.1.0 pins `pmcp = "2.17"`. New scaffold-pin tripwire (agent scaffold ↔ `pmcp-agent` version) mirrors the workbook `PMCP_VERSION` convention. Same release train.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The D-02 fix is best implemented as a split receive-task + single-consumer request worker | D-106-A architecture | LOW — D-02 semantics are the contract; impl is Claude's discretion (D-07 list). A different select-based impl satisfying "pump responses, serialize requests" is equally valid |
| A2 | `SamplingSource` needs a paired `pmcp` addition to return `CreateMessageResultWithTools` from the peer | Open Q1 / Pitfall 1 | MEDIUM — if the loop only ever needs single-content completions (no tool calling via sampling), the gap is moot; but AGNT-04 + the tool-calling loop strongly imply WithTools is required. Confirm in planning |
| A3 | Next free example number is `s50` (s49 = sampling_host exists) | Project structure | LOW — cosmetic; verify `ls examples/` at plan time |
| A4 | `crates/pmcp-agent` is a REGULAR workspace member (root-listed), unlike workspace-excluded `pmcp-package` | Project structure / CONTEXT code_context | LOW — CONTEXT flags "decide in planning whether root gates lint it"; rust-1.95 gate reality: only root `pmcp` is clippy-gated today, so a regular member is fine |
| A5 | No streaming needed in HTTP sources for this phase | Standard Stack | LOW — explicitly deferred in CONTEXT |

## Open Questions (RESOLVED)

> All three questions were resolved during Phase 108 planning (2026-07-17). Resolutions recorded inline below.

1. **`CompletionSource` result type: `CreateMessageResultWithTools` vs `CreateMessageResult` — and the peer path to produce it.** (HIGH PRIORITY)
   - What we know: `CreateMessageParams` (input) is tool-calling-complete. But `PeerHandle::sample()` returns `CreateMessageResult` (single `Content`, no tool_use), as does `HostSamplingHandler::handle_create_message`. Only `CreateMessageResultWithTools` carries `Vec<SamplingMessageContent>` with `ToolUse`.
   - What's unclear: Whether the loop drives tools *through sampling* (implied by AGNT-04 "same loop, sampling source") — which requires WithTools — or only through `OpenAiCompatSource`/`AnthropicSource` (which build their own responses and can return WithTools freely).
   - Recommendation: Design `CompletionSource` to return the WithTools shape (or a crate-local mirror). Add a paired `pmcp` 2.17.0 addition so `SamplingSource` gets tool_use blocks back over the peer (e.g. `PeerHandle::sample_with_tools` returning `CreateMessageResultWithTools`, or SamplingSource issuing the `ServerRequest` and decoding WithTools). This is the single biggest design decision in the phase — resolve it in the plan's Wave 0.
   - **RESOLVED:** `CompletionSource::create_message` returns `CreateMessageResultWithTools`; a paired zero-dep `PeerHandle::sample_with_tools` decode path is added in **plan 108-01 Task 2** (Wave 1), consumed by `SamplingSource` in plan 108-04 Task 1.

2. **Should the D-106-A fix and the sampling-with-tools peer addition ship as one `Server::run`/peer change or two?** Both are `pmcp` 2.17.0 additive changes. Recommendation: treat them as sibling tasks in the same wave — the real-loop D-03 proof needs both to demonstrate a tool-calling hosted agent.
   - **RESOLVED:** Both ship in ONE 2.17.0 release train (D-04) as sibling tasks in **plan 108-01** (Task 1 = D-106-A response pump, Task 2 = `sample_with_tools`); the tool_use round-trip test exercises both together.

3. **`RunState` / `ConversationStore` granularity (D-06):** how much loop state (pending tool calls, per-class retry counters, iteration index) must persist for mid-iteration resume vs. what can be recomputed from message history? Recommendation: persist the minimum the reference checkpoints — message history + iteration counter + pending tool-call set — and derive the rest purely.
   - **RESOLVED:** `RunState` = message history + iteration counter + pending tool-call set (no floats/time), defined in **plan 108-02 Task 3** (D-06); the rest is derived purely in plan 108-03.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable toolchain | build/test | ✓ (assumed CI parity) | latest stable | — |
| `wasm32-unknown-unknown` target | D-13 compile gate | verify at plan time (`rustup target list --installed`) | — | `rustup target add wasm32-unknown-unknown` |
| An OpenAI-compat endpoint (Ollama) | example runtime (AGNT-05 demo) | ✗ (likely not on CI) | — | Example gated / uses a mock; real Ollama is `#[ignore]`+env-gated per house "no Docker/live-service in default tests" rule |
| pmcp.run reference repo | D-09 study only (read-only) | ✓ at `~/Development/mcp/sdk/pmcp-run` | — | — |

**Missing dependencies with no fallback:** none block implementation.
**Missing dependencies with fallback:** live LLM endpoints — the `standalone-vs-sampled` example must degrade to a mock source in CI (the SamplingSource path can use a mock `HostSamplingHandler`; the OpenAiCompat path uses a stub server or is env-gated). Property/unit/fuzz tests need NO network.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` 1.7 + doctests |
| Config file | none (cargo default); workspace `Cargo.toml` |
| Quick run command | `cargo test -p pmcp-agent --test-threads=1` |
| Full suite command | `make quality-gate` (fmt --all, clippy pedantic+nursery -D warnings, build, workspace test, audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AGNT-01 | Seams are object-safe (`Arc<dyn ...>`) & reuse sampling types | unit (compile) | `cargo test -p pmcp-agent seams::` | ❌ Wave 0 |
| AGNT-02 | Loop pure between seams; retry class is data | unit | `cargo test -p pmcp-agent loop::decide` | ❌ Wave 0 |
| AGNT-03 | Same effect results ⇒ same decisions | property | `cargo test -p pmcp-agent replay_safety` | ❌ Wave 0 |
| AGNT-04 | SamplingSource over server-side peer (real loop) | integration (D-03) | `cargo test -p pmcp-agent --test real_loop_sampling` | ❌ Wave 0 |
| AGNT-05/06 | OpenAiCompat/Anthropic sources build & parse | unit (mock HTTP) | `cargo test -p pmcp-agent --features openai-compat,anthropic sources::` | ❌ Wave 0 |
| AGNT-07 | Agent-as-server exposes package tool; wasm32 compiles | integration + compile gate | `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | ❌ Wave 0 |
| AGNT-08 | Invoker honors related_task via poll_decision | integration | `cargo test -p pmcp-agent invoker::task_augmented` | ❌ Wave 0 |
| AGNT-09 | Agent configured from AgentPackage + SlotResolver | unit | `cargo test -p pmcp-agent config::resolver` | ❌ Wave 0 |
| D-106-A | Tool awaiting peer.sample()/.elicit()/.list_roots() completes on stock Server::run | integration (D-03) | `cargo test -p pmcp --test in_tool_peer_roundtrip` | ❌ Wave 0 |
| Fuzz | message/tool-result digestion path (CLAUDE.md ALWAYS) | fuzz | `cargo fuzz run agent_digest` (or proptest) | ❌ Wave 0 |
| Example | standalone-vs-sampled runs (CLAUDE.md ALWAYS) | example | `cargo run -p pmcp-agent --example s50_standalone_vs_sampled` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p pmcp-agent` (+ `-p pmcp` for the D-106-A tasks)
- **Per wave merge:** `make quality-gate`
- **Phase gate:** `make quality-gate` green (incl. PMAT cog ≤25 in CI) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/pmcp-agent/Cargo.toml` — new member (mirror pmcp-tasks); add to root `[workspace].members`
- [ ] `crates/pmcp-agent/tests/real_loop_sampling.rs` — D-03 real-loop harness (extend `tests/common/duplex.rs` convention → real `Server::run` + `Client`)
- [ ] `pmcp` `tests/in_tool_peer_roundtrip.rs` — D-106-A proof (sampling + elicitation + roots)
- [ ] `crates/pmcp-agent/tests/replay_safety.rs` + `tests/fixtures/*.json` golden traces — AGNT-03
- [ ] fuzz target `fuzz/fuzz_targets/agent_digest.rs` (or proptest equivalent) — ALWAYS requirement
- [ ] `examples/s50_standalone_vs_sampled.rs` — ALWAYS requirement (verify next free number at plan time)
- [ ] CI wasm32 compile gate entry for `pmcp-agent` default features (D-13)

## Security Domain

> `security_enforcement` absent from `.planning/config.json` = enabled. `pmcp-agent` is a library crate; its only network surface is the feature-gated HTTP sources (API keys) — most ASVS categories are N/A.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (auth is host/transport concern; adapter inherits ServerCore) | — |
| V3 Session Management | no | — |
| V4 Access Control | partial | Host-side sampling approval seam (`PreflightApproval`, Phase 106) gates LLM calls — the adapter/SamplingSource ride it, don't reimplement |
| V5 Input Validation | yes | `serde` typed deserialization of LLM/tool responses; `evaluate_submit_result` validates against `output_schema`; never `unwrap` untrusted JSON |
| V6 Cryptography | no (no crypto in crate) | — |
| V7 Error Handling / Logging | yes | **Never log API keys.** OpenAiCompat/Anthropic sources read keys from env/SlotResolver, never `tracing` them; host handler errors are sanitized to `-32603` (already done in host surface) |
| V9 Communications | yes | reqwest with `rustls` (workspace default), HTTPS endpoints only |

### Known Threat Patterns for a Rust agent-loop crate
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key leakage via logs/traces | Information Disclosure | Keys only in env/SlotResolver; never in `Debug`/`tracing`; identity-bearing slots structurally cannot hold values (pmcp-package design) |
| Unbounded loop (runaway tool calls / cost) | Denial of Service | `max_iterations`/`max_tokens` from `AgentPackage`; pure limit-check function every iteration (AGNT-02) |
| Malicious tool result triggering panic | Tampering / DoS | Typed `serde` parse with error propagation (no `unwrap`); fuzz the digestion path (ALWAYS) |
| Prompt-injection via connector tool output | Tampering | Out of SDK scope (host/model policy); note in rustdoc — the loop treats tool output as data, applies no privileged action beyond declared tools |
| Deviation silently changing model/budget at import | Repudiation | `detect_deviation` + D-15 warn-and-run (loud log), never silent |

## Sources

### Primary (HIGH confidence)
- `src/types/sampling.rs` (verified 2026-07-17) — `CreateMessageParams:200`, `CreateMessageResultWithTools:339`, `SamplingMessageContent:132` (ToolUse/ToolResult), `ToolChoice`/`ToolChoiceMode`
- `src/types/tasks.rs:349` — `Task::poll_decision()`, `TaskPollDecision`
- `src/client/mod.rs:771` — `Client::wait_for_task`, `WaitForTaskOptions`
- `src/client/host/{mod,sampling}.rs` — Phase 106 host surface, `HostSamplingHandler`, `PreflightApproval`, idle-host limitation
- `src/server/mod.rs:971-1163` — `Server::run` / `spawn_message_handler` / `handle_transport_message` (D-106-A deadlock site)
- `src/server/peer_impl.rs:55` + `src/shared/peer.rs:57` — `PeerHandle::sample` returns `CreateMessageResult` (the Open-Q1 gap)
- `src/server/mod.rs:246,278` — `ToolOutput::Result`; `src/server/core.rs`, `builder.rs:766` — `ServerCore`/`with_task_store`
- `crates/pmcp-package/src/package/agent.rs:41` — `AgentPackage`; `slot/types.rs`, `slot/deviation.rs:28` — `ConfigSlot`/`SlotType`/`detect_deviation`
- `crates/pmcp-tasks/Cargo.toml` — 0.x isolation precedent
- `docs/design/agents-teams-sdk-extraction-plan.md` §2.1/§3/§4-C/§7/§8 — approved design
- Reference (read-only, D-09): `pmcp-run/.../handler/iteration.rs` (`execute_iteration`, `evaluate_submit_result`, `call_llm_with_retry` class-split, `ctx.map` dispatch), `types.rs:1016` (`IterationResult`), `mcp/client.rs` (`classify`/`PollDecision`), `llm/service.rs` (reqwest transformer shape)
- Root `Cargo.toml` — versions (pmcp 2.16.0, reqwest 0.13, proptest 1.7, workspace members)
- `CLAUDE.md` — ALWAYS requirements, PMAT gate, publish order, release train

### Secondary (MEDIUM confidence)
- `.planning/phases/106-client-host-surface/deferred-items.md` — D-106-A technical detail + `/simplify` follow-ups (classify/extract fusion lands here)

### Tertiary (LOW confidence)
- none — all claims verified against in-repo code or the reference implementation

## Project Constraints (from CLAUDE.md)
- **ALWAYS requirements (NO EXCEPTIONS):** fuzz + property + unit tests + a runnable `cargo run --example` for the new feature.
- **Quality gate:** `make quality-gate` before every commit/push (fmt --all --check, clippy pedantic+nursery -D warnings, build, test, audit).
- **PMAT cog ≤25** per function (CI-enforced, hard cap 50 with `// Why:`-annotated `#[allow]`). Extract pure decision fns to stay under.
- **Zero SATD** comments; comprehensive rustdoc with working examples.
- **Tests run `--test-threads=1`** (race prevention).
- **Publish order:** add `pmcp-agent` after `pmcp` in CLAUDE.md's list; update scaffold-pin tripwire.
- **Contract-first:** the adapter must compose into `contracts/team-servers-v1.yaml` (`team_mcp__<member>` dispatch) — Phase 109 conformance runs against these.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dependency already pinned in the verified workspace; no external discovery.
- Architecture: HIGH — reference implementation studied directly; SDK integration points read at exact line numbers.
- Pitfalls: HIGH — Pitfall 1 (result-type gap) verified across four source files; others grounded in existing docs.
- D-106-A fix: HIGH on the diagnosis (verified deadlock site), MEDIUM on the exact impl (A1 — semantics fixed, impl is discretion).
- Open Q1 (WithTools peer path): the single item needing a locked decision before implementation.

**Research date:** 2026-07-17
**Valid until:** 2026-08-16 (stable internal APIs; re-verify only if pmcp minor version changes before planning)
