---
phase: 108
reviewers: [codex, gemini]
reviewed_at: 2026-07-18T05:26:31Z
plans_reviewed: [108-01-PLAN.md, 108-02-PLAN.md, 108-03-PLAN.md, 108-04-PLAN.md, 108-05-PLAN.md, 108-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 108

## Codex Review

# Cross-AI Plan Review — Phase 108

## Overall assessment

The plans have strong decomposition, traceability, and test intent, but they are not ready to execute unchanged. Several current SDK APIs contradict assumptions embedded in the plans, and the most serious issues affect AGNT-04, AGNT-07, AGNT-08, and replay-safe resumability. Overall risk is **HIGH** until the sampling host return type, response-pump backpressure, engine checkpoint model, task adapter lifecycle, and package-to-runtime composition are resolved.

## Plan 108-01 — Core response pump and `sample_with_tools`

### Summary

The plan correctly identifies the server-loop deadlock and preserves serialized request execution. The real-loop tests are the right proof standard. However, the proposed bounded request channel can recreate the deadlock, and adding only a peer-side `sample_with_tools` decoder is insufficient because the client host handler still returns the single-content result type.

### Strengths

- Correctly fixes D-106-A in core rather than hiding it in an adapter.
- Preserves serialized request handling.
- Uses real `Server::run` and `Client` coverage for sampling, elicitation, and roots.
- Keeps `sample()` intact while proposing an explicit tool-capable path.
- Includes release-pin and changelog updates.

### Concerns

- **HIGH — bounded-channel deadlock:** If the receive loop awaits `request_tx.send()` while the request worker is blocked in `peer.sample()`, a full queue prevents the receive loop from reading the sampling response. That recreates the original deadlock under load.
- **HIGH — incomplete WithTools path:** [`HostSamplingHandler::handle_create_message`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/host/sampling.rs:44) currently returns `CreateMessageResult`, so the proposed client `on_sampling` callback cannot return `ToolUse` blocks. A peer-side decode change alone cannot make the planned test compile.
- **HIGH — public trait compatibility:** Adding a required method to [`PeerHandle`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/peer.rs:50) breaks downstream trait implementations, contradicting the claim that 2.17.0 is additive.
- **MEDIUM — notification semantics:** Moving notifications into the independent receive loop changes more than “pump responses only” and may introduce cancellation races.
- **MEDIUM — lifecycle underspecified:** Worker termination, channel closure, transport errors, pending dispatcher failures, and server shutdown/join behavior are not defined.
- **LOW — release files:** `Cargo.lock` and any exact downstream version constraints should be included after a repository-wide pin audit.

### Suggestions

- Ensure the receive loop never awaits request-queue capacity. Use an unbounded internal queue, reserved response path, or another design that guarantees responses are always drained.
- Extend the host surface with a compatible WithTools handler path. Preserve existing handlers through a default adapter that converts legacy single-content results.
- Give `PeerHandle::sample_with_tools` a compatibility-preserving default, or introduce an extension/subtrait without breaking existing implementors.
- Add saturation and shutdown tests, including a queued second request while the first awaits sampling.
- Explicitly decide whether notifications retain old ordering or are intentionally handled concurrently.

### Risk assessment

**HIGH.** The core idea is sound, but two blockers—the host return type and bounded-channel backpressure—prevent the plan from proving its central behavior.

---

## Plan 108-02 — Crate scaffold and effect seams

### Summary

The interface-first wave is well structured and enables later parallel work. The seam boundaries match the intended architecture, but several load-bearing data contracts remain vague, particularly tool-call identity, runtime configuration, error serialization, and the exact history type.

### Strengths

- Correctly isolates experimental APIs in `pmcp-agent` 0.1.0.
- Explicitly proves object safety with `Arc<dyn Trait>`.
- Uses the correct `CreateMessageResultWithTools` completion result.
- Makes batch invocation a seam with a sequential default.
- Keeps retry classification as data.
- Defines resumable state and an in-memory implementation early.

### Concerns

- **HIGH — missing tool-call correlation:** `ToolCall` is described as name plus arguments, but MCP/provider tool calls require a stable call ID. Parallel results must retain that ID so `tool_result` blocks correlate with the correct `tool_use`.
- **HIGH — runtime configuration contract absent:** No `AgentConfig`/`ResolvedAgent` type binds instructions, tools, limits, schemas, model selection, and endpoints to the engine. Later plans assume these values exist.
- **MEDIUM — history type undefined:** “The crate’s message type” should be fixed explicitly, preferably the SDK `SamplingMessage` type.
- **MEDIUM — manifest inconsistency:** `InMemoryStore` proposes `parking_lot::Mutex` or DashMap, but neither dependency is included. Use `std::sync::Mutex` or add the chosen dependency explicitly.
- **MEDIUM — feature selection:** `pmcp` is configured with `default-features = false`, while later code requires client, server, tasks, and peer APIs. The precise required PMCP features need to be enumerated.
- **MEDIUM — batch contract:** Result count, input-order preservation, duplicate IDs, and missing-result behavior are not specified.
- **LOW — retry model:** `attempt_hint` mixes classification with policy/state and may be better stored in run state or returned separately.

### Suggestions

- Define `ToolCall { id, name, arguments, connector }` and a result carrying the same ID.
- Establish a `ResolvedAgentConfig` type in this plan for later engine and adapter consumption.
- Specify `RunState.history: Vec<SamplingMessage>` and a checkpoint phase/state enum.
- Require `invoke_batch` to return exactly one result per input, in input order, with matching IDs.
- Make recorded/replayable error representations serializable without exposing secrets.
- Add compile checks for default, all-feature, and wasm-target feature combinations.

### Risk assessment

**MEDIUM-HIGH.** The decomposition is good, but underspecified contracts would cause significant rework in Waves 3 and 4.

---

## Plan 108-03 — Pure engine, trace, replay, and fuzzing

### Summary

This is conceptually the strongest plan: a thin async engine plus pure decision functions is the correct architecture. The current sequence, however, does not actually provide mid-iteration resumability and leaves retry outcomes, checkpoint placement, token accounting, and schema validation unresolved.

### Strengths

- Clean separation between effects and pure decisions.
- Explicit prohibition on time and randomness in decision logic.
- Public serializable `EffectTrace` is valuable for testing and future diagnostics.
- Golden traces plus property tests give complementary coverage.
- Complexity control naturally follows from extracting decision functions.
- Includes fuzz-style testing for untrusted tool data.

### Concerns

- **HIGH — `ConversationStore::load` is missing from the engine:** The engine description awaits `create_message`, `invoke_batch`, and `save`, but never loads resumable state.
- **HIGH — checkpoint ordering is unsafe:** Pending tool calls are not saved before tool invocation. A crash after invocation but before save repeats side-effecting tools. Final assistant state also appears to return without being saved.
- **HIGH — retry classification is not exposed by the run outcome:** `IterationResult { is_final }` cannot represent “host must retry as Transient/Capacity.” AGNT-02 requires classification as returned data.
- **HIGH — limit inputs are undefined:** `tokens_used`, tools, system prompt, and limits are not present in the specified `RunState`; no separate engine configuration is defined.
- **HIGH — output-schema validation has no implementation dependency:** `evaluate_submit_result` promises JSON Schema validation, but the new crate does not enable or depend on a validator.
- **MEDIUM — replay assertion is too weak:** Comparing final results alone does not prove identical decision sequences.
- **MEDIUM — incomplete effect trace:** Completion errors, store load state, exhausted traces, unused effects, and tool-batch errors need explicit recording semantics.
- **MEDIUM — malformed JSON fuzz claim:** `serde_json::Value` is already parsed. Arbitrary values test unusual structures, not malformed JSON bytes.

### Suggestions

- Model execution as explicit checkpoint phases, such as `ReadyForCompletion`, `PendingTools`, and `ToolsCompleted`.
- Save pending calls before dispatch and save completion/tool results before advancing.
- Define `RunOutcome::{Completed, LimitReached, RetryRequired { class }, Failed}`.
- Define exact resume/idempotency semantics, including duplicate tool-call protection.
- Add a public decision trace or per-step outcome and compare that in replay tests.
- Record success and failure effects using secret-safe serializable error records.
- Either enable the existing PMCP validation feature or scope output validation out explicitly.
- Fuzz raw response bytes/parser entry points if malformed JSON is part of the claim.

### Risk assessment

**HIGH.** Without a durable state machine and explicit retry outcome, the plan does not yet satisfy AGNT-02 or D-06 despite having the right architectural direction.

---

## Plan 108-04 — Completion sources

### Summary

The three-source boundary is appropriately constrained, and the provider adapters are sensibly feature-gated. SamplingSource is blocked by Plan 108-01’s incomplete host API, while the HTTP plans need more precise compatibility and transport-policy coverage.

### Strengths

- Keeps SamplingSource dependency-free.
- Correctly feature-gates provider HTTP dependencies.
- Preserves tool calls in both provider translations.
- Includes secret-redaction tests.
- Avoids live-service dependencies in default tests.
- Tests both optional provider features together.

### Concerns

- **HIGH — SamplingSource cannot receive ToolUse yet:** The current host sampling trait returns the legacy result type.
- **HIGH — HTTPS-only contradicts Ollama support:** Typical local Ollama endpoints use loopback HTTP. Strict HTTPS would fail an explicitly named AGNT-05 use case.
- **MEDIUM — pure transform tests are insufficient:** They do not verify actual URL construction, headers, authentication, timeouts, response-size handling, or status classification through the HTTP client.
- **MEDIUM — provider mappings underspecified:** Role conversion, tool-result messages, tool-choice modes, multiple choices, missing usage, finish reasons, and malformed tool-call arguments need cases.
- **MEDIUM — no request timeout:** External requests can hang indefinitely unless the host supplies a configured timeout.
- **MEDIUM — endpoint/key binding:** A key can accidentally be sent to a caller-supplied hostile endpoint unless configuration makes that trust boundary explicit.

### Suggestions

- Fix the host WithTools response contract before implementing SamplingSource.
- Allow HTTP only for loopback/local endpoints or behind an explicit insecure-local opt-in.
- Add local mock-server tests that inspect paths, headers, bodies, status handling, and timeouts.
- Add table-driven provider compatibility cases for every sampling content and tool-choice variant.
- Bound response bodies and expose configurable request deadlines.
- Use a redacted secret wrapper with a custom `Debug`, rather than relying only on tests.

### Risk assessment

**HIGH** as currently ordered because AGNT-04 is blocked. After the host contract is fixed, the HTTP-source portion is **MEDIUM** risk.

---

## Plan 108-05 — Tasks-aware invoker and slot resolution

### Summary

The plan composes existing task primitives rather than reimplementing polling, which is correct. Its main gaps are using the wrong task-waiting option path, leaving the heterogeneous client/connector architecture undecided, and not completing the promised package-to-runtime configuration flow.

### Strengths

- Reuses the SDK task polling implementation.
- Converts errors to data instead of panicking.
- Overrides batch dispatch with input-order-preserving `join_all`.
- Defines resolver policy as a seam.
- Includes warn-and-run behavior for tested-value deviation.
- Treats secret logging as a testable security requirement.

### Concerns

- **HIGH — wrong polling call/options:** `WaitForTaskOptions::default()` has no overall timeout. The SDK provides [`wait_for_related_task`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs:847), which incorporates `TaskMetadata` polling hints and budget. The current plan can poll indefinitely.
- **HIGH — connector-client design unresolved:** [`Client<T>`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs:113) is transport-generic. “A client or a map of clients” is not a minor implementation choice when endpoint entries can be URL or command transports.
- **HIGH — AGNT-09 is only partially achieved:** Resolving slots does not show that an `AgentPackage` fully constructs the engine, source, limits, tool set, and invoker.
- **MEDIUM — incorrect deviation call shape:** `detect_deviation` accepts `&SlotType`, not `ConfigSlot` or an arbitrary resolved string. A proposed behavior-relevant `SlotType` must be constructed.
- **MEDIUM — endpoint resolution is underspecified:** `ComponentRef` contains name/version/type, not an endpoint slot. `SlotResolver` needs an explicit `resolve_endpoint(name)` contract.
- **MEDIUM — unbounded batch concurrency:** `join_all` can dispatch arbitrarily many tool calls simultaneously.
- **MEDIUM — environment tests:** Process-global environment mutation needs scoped restoration and serialization.

### Suggestions

- Use `wait_for_related_task(meta, configured_options)` and enforce a host-configurable hard maximum.
- Separate `ConnectorClient`/`ConnectorInvoker` from concrete `Client<T>`, or define a client-factory seam capable of URL and command transports.
- Add a `resolve_agent(&AgentPackage) -> ResolvedAgentConfig` path and an end-to-end AGNT-09 test.
- Define endpoint resolution directly on `SlotResolver`.
- Add a maximum batch size/concurrency limit and preserve call IDs.
- Test `input_required`, failed, cancelled, timeout, missing result, and malformed related-task metadata.

### Risk assessment

**HIGH.** The current design can poll forever and does not yet define how heterogeneous resolved endpoints become usable MCP clients.

---

## Plan 108-06 — Adapter, example, wasm gate, and mapping

### Summary

The outward-facing deliverables are appropriate, and the D-09 mapping is particularly valuable. The adapter plan, however, is the largest unresolved area: it does not define a valid task lifecycle, conflicts with wasm support, and cannot construct a request-scoped SamplingSource using the proposed constructor.

### Strengths

- Provides a concrete agent-as-server proof.
- Tests package-derived surface and call independence.
- Includes a runnable, network-free default example.
- Adds the requested wasm compile gate.
- Delivers the durable-lambda shape mapping without copying private code.
- Uses full-loop integration testing rather than mocks alone.

### Concerns

- **HIGH — task lifecycle is not implemented:** Attaching `related_task` metadata does not create, update, complete, cancel, or persist a task. The plan lacks task ID creation, owner binding, state transitions, background execution, result storage, and failure handling.
- **HIGH — wrong builder method:** `with_task_store` accepts the legacy `TaskRouter`; the standard store setter is `task_store`. Both are native-only in the current [`ServerCoreBuilder`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/builder.rs:766).
- **HIGH — wasm contradiction:** The task-store builder APIs are `cfg(not(target_arch = "wasm32"))`. Hiding the adapter on wasm might make the crate compile, but would not prove an agent-as-server adapter deploys to wasm.
- **HIGH — SamplingSource construction mismatch:** `SamplingSource` needs the per-request `extra.peer()`, but `AgentServer` is specified to accept a preconstructed `CompletionSource`. Hosted sampling therefore needs a request-scoped source factory.
- **HIGH — package description assumption is false:** [`AgentPackage`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/package/agent.rs:41) has `name`, schemas, and instructions, but no description field.
- **MEDIUM — example feature mismatch:** `OpenAiCompatSource` is compile-time gated, while the verification command provides no `--features openai-compat`.
- **MEDIUM — AGNT-06 proof is weak:** The “same loop” example covers sampling and OpenAI/mock, not Anthropic.
- **MEDIUM — background execution portability:** An unqualified `tokio::spawn` task is unsuitable for some transports and wasm, and shutdown/cancellation semantics are absent.
- **MEDIUM — fresh-run identity:** The plan does not define collision-safe run IDs or how store state is isolated between calls.

### Suggestions

- Design the adapter task lifecycle first, using either the standard task-enabled tool/store path or a complete router implementation. Do not manually attach orphan metadata.
- Add a `CompletionSourceFactory` receiving `RequestHandlerExtra` so SamplingSource can be created per call.
- Resolve the wasm contract explicitly: provide a supported wasm task backend, a synchronous wasm mode, or narrow AGNT-07’s claim.
- Derive description from a documented rule or revise `AgentPackage`; do not assume a nonexistent wire-frozen field.
- Add task tests for immediate completion, asynchronous completion, failure, cancellation, ownership isolation, and server shutdown.
- Make feature commands consistent and include Anthropic in a same-engine compile/mock proof.
- Ensure the example proves the actual OpenAI adapter through a local mock endpoint rather than substituting only a generic mock source.

### Risk assessment

**HIGH.** The task adapter and wasm story are not implementable as currently written, and the hosted SamplingSource cannot be injected through the proposed API.

---

## Cross-cutting gaps

- **HIGH — contract-first workflow is absent.** Repository instructions require contract YAML updates and `pmat comply check` before and after implementation.
- **HIGH — package-to-runtime acceptance test is missing.** Add one test that loads an `AgentPackage`, resolves slots/endpoints, constructs the source/invoker/engine, runs it, and exposes it through the adapter.
- **MEDIUM — quality coverage is incomplete.** Add explicit rustdoc/doctest, all-feature clippy, coverage, PMAT proxy, and feature-matrix commands for the new crate; the current root quality gate may not lint every new crate/feature combination.
- **MEDIUM — error matrices should be shared.** Each seam should have tests for transient, capacity, fatal, malformed, cancellation, timeout, and exhaustion outcomes.
- **MEDIUM — performance controls need contracts.** Bound tool-batch concurrency, HTTP duration/body sizes, trace length, message history, and task polling duration.

## Final risk assessment

**Overall risk: HIGH — revise before execution.**

The six-plan wave structure is strong, and the intended architecture is credible. Approval should be withheld until these five blockers are resolved in the plans:

1. End-to-end WithTools support on both the client host and server peer surfaces.
2. A response pump that cannot block on request-queue capacity.
3. A checkpointed run-state machine with explicit retry/limit outcomes.
4. A real task lifecycle and a coherent native/wasm adapter contract.
5. An end-to-end `AgentPackage → resolved runtime → engine/source/invoker` composition API.

---

## Gemini Review

# Cross-AI Plan Review: Phase 108 (`pmcp-agent` Loop Crate)

This document provides a structured review of the proposed implementation plans (`108-01-PLAN.md` through `108-06-PLAN.md`) for **Phase 108: `pmcp-agent` Loop Crate** of the PMCP SDK Extensions.

---

## 1. Summary Assessment

The proposed implementation plans are exceptionally thorough, mapping the requirements (AGNT-01 through AGNT-09) to concrete software design patterns, property tests, and a dedicated workspace structure. The separation of the experimental runtime into `crates/pmcp-agent` (analogous to `pmcp-tasks`) is a sound architectural choice that preserves the core SDK's clean dependency footprint. 

However, the plans contain one **critical structural flaw (HIGH Severity)**: splitting `Server::run` into concurrent tasks using `Arc<RwLock<T>>` will introduce a permanent deadlock when sending outbound requests or progress notifications. The background receive loop will hold the `RwLockWriteGuard` for the entire duration of its blocked network reads, starving any concurrent writes. By refactoring the server transport layer to use a **Transport Actor Pattern** (or a single unified `tokio::select!` loop), this deadlock can be eliminated entirely.

---

## 2. Plan Strengths

*   **Purity & Replay Safety (AGNT-02, AGNT-03):** The separation of async orchestration from the pure decision loop (`iteration/decide.rs`) is extremely well-designed. Avoiding timestamps, random states, and floats ensures deterministic replay behavior. The use of proptest over serialized `EffectTrace`s in [replay_safety.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-agent/tests/replay_safety.rs) is highly robust.
*   **Decoupled Config Resolution (AGNT-09):** Defining `SlotResolver` as a clean trait seam allows different host environments (env vars for local laptops, DynamoDB join tables for the platform) to resolve configurations cleanly. The implementation of `detect_deviation` as a warning guard rather than a hard error matches the target operational policies perfectly.
*   **Spec Compliance & Tool Support:** Resolving the `CreateMessageResultWithTools` vs `CreateMessageResult` gap ensures that sampling-based agents can properly handle tool use blocks, mapping directly to the MCP 2025-11-25 revisions.
*   **WASM Gate & Dep Isolation (D-13):** Feature-gating HTTP completion sources (`openai-compat`, `anthropic`) using cargo feature-flags ensures that `pmcp-agent` compiles target-clean for `wasm32-unknown-unknown` under default features.

---

## 3. Key Concerns & Risks

### 🔴 Concurrent `RwLock` Write-Lock Deadlock inside `Server::run` (HIGH Severity)
*   **The Issue:** In [108-01-PLAN.md](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/108-pmcp-agent-loop-crate/108-01-PLAN.md), the proposed fix splits `Server::run` into a dedicated background receive loop and a request worker task. The receive loop continuously calls [receive_message_from_transport](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs#L1075-L1081), which acquires the write lock on the transport `RwLock` and awaits `t.receive().await`. Because `receive()` blocks waiting for client input (which may take a long time), this `RwLockWriteGuard` is held continuously. 
*   **The Deadlock:** While a tool handler is running on the worker task, if it attempts to send a progress notification or invoke `peer.sample()`, it dispatches a write request via `spawn_notification_handler` or `spawn_server_request_drain`. Both tasks attempt to acquire `transport.write().await` and block indefinitely. Because the client is waiting for the server's request/notification, the client never sends any message. The receive loop remains blocked in `receive()`, and the server deadlocks.

### 🟡 Single-Content vs Multi-Content Sampling Robustness (MEDIUM Severity)
*   **The Issue:** The new [sample_with_tools](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/peer_impl.rs) path deserializes raw client responses directly into `CreateMessageResultWithTools`. 
*   **The Risk:** If a client conforms to an older spec version or returns a standard [CreateMessageResult](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/sampling.rs#L293) (where `content` is a single `Content` object rather than a `Vec<SamplingMessageContent>`), the deserialization will fail, crashing the tool call.

### 🟡 Stateless Adapter vs Session Continuity (MEDIUM Severity)
*   **The Issue:** [108-06-PLAN.md](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/108-pmcp-agent-loop-crate/108-06-PLAN.md) implements a stateless `ServerCore` adapter where a "fresh run" is spawned per tool call, and continuity is deferred to the store.
*   **The Risk:** If the conversational tool input schema is package-driven and defaults to a simple string message, there is no way for a client to pass a `conversation_id` or `run_id` to correlate the call to an existing session in the `ConversationStore`. Every incoming tool call will execute as a brand-new, isolated session.

### 🟡 Anthropic Messages API Constraints (MEDIUM Severity)
*   **The Issue:** The Anthropic Messages API requires strict alternation of roles (`user` / `assistant`) starting with `user`, system prompt separation, and specific `tool_use` / `tool_result` mappings.
*   **The Risk:** If the agent loop does parallel tool execution, the history will contain consecutive `tool_result` items. If `AnthropicSource` simply transforms these one-to-one into messages without restructuring, Anthropic will return a 400 Bad Request error.

---

## 4. Suggestions & Improvements

### Suggestion 1: Adopt the Transport Actor Pattern
To avoid the `RwLock` write-lock deadlock, refactor `Server::run` in [src/server/mod.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs#L971) to route all I/O through a single owner task (the **Transport Actor**) that owns the `transport` mutably. Eliminate the concurrent tasks directly acquiring the write lock.

```rust
// Suggested Transport Actor structure:
struct TransportActor<T: Transport> {
    transport: T,
    send_rx: mpsc::Receiver<TransportMessage>,
    receive_tx: mpsc::Sender<TransportMessage>,
}

impl<T: Transport> TransportActor<T> {
    async fn run(mut self) {
        loop {
            tokio::select! {
                msg_res = self.transport.receive() => {
                    let msg = match msg_res {
                        Ok(msg) => msg,
                        Err(_) => break,
                    };
                    if self.receive_tx.send(msg).await.is_err() { break; }
                }
                Some(msg) = self.send_rx.recv() => {
                    if self.transport.send(msg).await.is_err() { break; }
                }
            }
        }
    }
}
```
All other tasks (worker tasks, notifications, and drains) will simply write to the actor's channel instead of locking the transport `RwLock` directly. This keeps the execution of the handlers concurrent while serializing transport writes and reads safely.

### Suggestion 2: Make `sample_with_tools` Deserialization Robust
Add a fallback deserialization block inside `sample_with_tools` in [src/server/peer_impl.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/peer_impl.rs). If decoding into `CreateMessageResultWithTools` fails, try decoding into the standard `CreateMessageResult` and manually convert it into the `WithTools` shape by wrapping the single content item into a vector:

```rust
let result = match serde_json::from_value::<CreateMessageResultWithTools>(value.clone()) {
    Ok(res) => res,
    Err(_) => {
        let legacy = serde_json::from_value::<CreateMessageResult>(value)?;
        CreateMessageResultWithTools {
            model: legacy.model,
            stop_reason: legacy.stop_reason,
            role: legacy.role,
            content: vec![SamplingMessageContent::Text {
                text: legacy.content.text,
                meta: None,
            }],
            meta: legacy.meta,
        }
    }
};
```

### Suggestion 3: Add `conversation_id` / `run_id` to Adapter Tool Schema
Modify the [AgentPackage](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/package/agent.rs#L41) tool-generation logic. The tool input schema should generate an optional `run_id: String` or `conversation_id: String` parameter. If provided, the adapter looks up and resumes the session state using the `ConversationStore`; if absent, it generates a fresh one.

### Suggestion 4: Implement Role Alternation & Message Merging in `AnthropicSource`
Inside `AnthropicSource::create_message`, implement a normalization pass over the message history before sending it to the API:
1. Merge consecutive messages with the same role into a single message by joining their content blocks.
2. Ensure that `tool_result` content blocks are correctly packed into a single `user` message following the `assistant` message that carried the matching `tool_use` IDs.
3. Extract any system messages and pass them as the top-level `system` parameter in the request payload.

---

## 5. Risk Assessment

*   **Overall Risk Level:** **HIGH**
*   **Justification:** While the application logic and testing plans are detailed and correct, the concurrency architecture in Wave 1 contains a critical design flaw (the `RwLock` write-lock starvation). If implemented exactly as written, any tool executing a sampling call or sending a progress notification will deadlock permanently. Resolving this via the Transport Actor Pattern (Suggestion 1) decreases the risk to **LOW**.

---

## Consensus Summary

Both reviewers independently rate the phase **HIGH risk as planned** — not because the architecture is wrong (both praise it), but because the Wave-1 core concurrency fix and the WithTools contract have design flaws that would surface as deadlocks or compile failures during execution.

### Agreed Strengths

- **Pure decision core + replay safety (108-03)** — both call the effects/pure-function separation the strongest part: no wall-clock/randomness, public serde `EffectTrace`, proptest + golden traces (Codex: "conceptually the strongest plan"; Gemini: "extremely well-designed").
- **Crate isolation & feature gating** — `pmcp-agent` 0.x isolation mirroring `pmcp-tasks`, feature-gated HTTP sources, wasm32 compile gate under default features.
- **SlotResolver as a seam (AGNT-09)** — trait-based resolution with warn-and-run deviation policy matches operational reality (env vars locally, platform joins later).
- **Correct primitive reuse** — task polling, `CreateMessageResultWithTools`, sequential-default `invoke_batch` with `join_all` override.

### Agreed Concerns (highest priority — raised by both)

1. **108-01 pump design can recreate the deadlock it fixes (HIGH, both reviewers).** Codex: if the receive loop awaits `request_tx.send()` on a bounded channel while the worker is blocked in `peer.sample()`, a full queue re-deadlocks. Gemini: a background receive loop holding the transport `RwLock` write guard across blocked `receive()` starves every concurrent send (notifications, peer requests) — permanent deadlock. **Shared invariant both demand: the receive/drain path must NEVER block on request-queue capacity or transport locks.** Gemini's concrete remedy: Transport Actor pattern (single owner task + `tokio::select!` + mpsc send channel); Codex's: unbounded internal queue / reserved response path + saturation tests.
2. **WithTools contract is incomplete end-to-end (HIGH Codex / MEDIUM Gemini).** Codex: `HostSamplingHandler::handle_create_message` still returns single-content `CreateMessageResult`, so the client `on_sampling` path can't return `ToolUse` blocks — a peer-side decoder alone can't make the planned test compile; also `PeerHandle::sample_with_tools` as a required trait method breaks downstream implementors (2.17.0 would not be additive). Gemini: `sample_with_tools` deserialization must fall back to the legacy single-content shape or older clients crash the tool call. **Both require legacy↔WithTools compatibility handled on BOTH client-host and server-peer surfaces.**

### Divergent Views (single-reviewer, worth investigating)

**Codex-only (deep API-verification findings):**
- `ToolCall` lacks a stable call ID — parallel `tool_result` blocks can't correlate to `tool_use` (108-02, HIGH).
- 108-03 engine never calls `ConversationStore::load`; checkpoint ordering unsafe (pending tool calls not saved before dispatch → crash repeats side-effecting tools); `IterationResult { is_final }` can't express `RetryRequired { class }` — suggests `RunOutcome::{Completed, LimitReached, RetryRequired, Failed}` (HIGH).
- 108-05: `WaitForTaskOptions::default()` has no overall timeout — should use `wait_for_related_task` (src/client/mod.rs:847) which honors `TaskMetadata` hints/budget; connector-client transport-generics (`Client<T>`) unresolved for URL-vs-command endpoints; AGNT-09 needs an end-to-end `resolve_agent(&AgentPackage) -> ResolvedAgentConfig` composition test (HIGH).
- 108-06: adapter task lifecycle not actually implemented (orphan `related_task` metadata without create/update/complete); `with_task_store` vs `task_store` builder-method mismatch, both native-only (`cfg(not(wasm32))`) — contradicting the wasm claim; `SamplingSource` needs a request-scoped factory (`RequestHandlerExtra`-driven), not a preconstructed source; `AgentPackage` has no `description` field (HIGH).
- Cross-cutting: contract-first workflow (contract YAML + `pmat comply check`) absent from all plans; HTTPS-only contradicts loopback Ollama (AGNT-05).
- 108-04: no request timeouts/body bounds; endpoint/key trust boundary; provider mapping edge-case matrix.

**Gemini-only:**
- Stateless adapter + package-driven schema leaves no way to pass `conversation_id`/`run_id` — every call is an isolated session; suggest an optional run-id parameter in the generated tool schema (MEDIUM).
- `AnthropicSource` must normalize history (merge consecutive same-role messages, pack `tool_result`s into one user message, hoist system prompt) or the Messages API returns 400 on parallel-tool histories (MEDIUM).

### Recommended Action

Replan with `/gsd:plan-phase 108 --reviews` before executing. Highest-leverage fixes: (1) transport-actor/never-block pump design + saturation tests in 108-01; (2) end-to-end WithTools compatibility (host handler + peer + legacy fallback, additive trait method); (3) `ToolCall` call-ID + `RunOutcome` retry classes + checkpoint ordering in 108-02/03; (4) `wait_for_related_task` + adapter task-lifecycle/wasm-contract corrections in 108-05/06.
