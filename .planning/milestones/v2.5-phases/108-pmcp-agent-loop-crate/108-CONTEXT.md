# Phase 108: `pmcp-agent` Loop Crate - Context

**Gathered:** 2026-07-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship the agent runtime as an open, deploy-anywhere `crates/pmcp-agent` (0.x, experimental, isolated from `pmcp` core like `pmcp-tasks` was) — a pure decision loop between object-safe async effect seams (`CompletionSource` / `ToolInvoker` / `ConversationStore`), three CompletionSources (sampling-first: `SamplingSource` zero-dep, feature-gated `OpenAiCompatSource` and `AnthropicSource`), an agent-as-server adapter on `ServerCore`, and a tasks-aware `ToolInvoker` honoring SEP-1686 `poll_decision` — all configured from an `AgentPackage` (pmcp-package, Phase 107) plus resolved config slots. Requirements AGNT-01..09.

This phase ALSO fixes D-106-A in `pmcp` core (the `Server::run` serialized-loop deadlock on in-tool `peer.sample()`), because the hosted-agent flow depends on it — shipped as a paired `pmcp` minor bump.

Out of this phase: team servers (Phase 109), cargo-pmcp verbs (Phase 110), book chapters beyond rustdoc (Phase 111), any platform-side migration (DEFER-04), additional CompletionSources (DEFER-02), AgentCore adapter (DEFER-01). No provider matrix — the trait is the extension point (design §7).
</domain>

<decisions>
## Implementation Decisions

### D-106-A fix (pmcp core, this phase)
- **D-01:** The fix lands **in `pmcp` core**, not as an adapter-local workaround — every server doing in-tool sampling/elicitation benefits; the landmine is removed permanently.
- **D-02:** Concurrency semantics: **pump responses only**. Request handling stays serialized (zero behavior change for existing servers); the message loop keeps routing inbound RESPONSES to pending peer requests while a handler awaits. Fixes exactly the deadlock — a tool awaiting `peer.sample()` / `.elicit()` / `.list_roots()` gets its answer.
- **D-03:** Proof standard: **real-loop end-to-end** — the hosted-agent example and tests run through the REAL `Server::run` and a real `Client` with `on_sampling`, no raw pump. Phase 106's duplex raw-pump tests stay; the new invariant is "in-tool peer round-trips work on the stock server loop" (sampling, elicitation, roots).
- **D-04:** Release pairing: the D-106-A fix ships as a **`pmcp` minor bump (2.17.0) in the same release train as `pmcp-agent` 0.1.0**, which pins the new pmcp version; cargo-pmcp scaffold-pin tripwire updated per convention.

### Loop shape & replay contract
- **D-05:** Loop expression: **async loop generic over the seams + extracted pure decision functions**. The crate owns the async iteration engine; between awaits, ALL logic lives in pure functions (end-turn detection, result digestion, retry classification, iteration/budget limits) operating on data types. Matches design §8.1 (platform wraps seam impls in durable `ctx.step` and reuses the loop) and the production `IterationResult` shape. Determinism discipline inside the loop: no wall-clock, no randomness — counters, not time.
- **D-06:** `ConversationStore` holds **message history + loop iteration state** (counters, pending tool calls) so a run is resumable mid-iteration — what the platform's durable host needs; `InMemoryStore` stays trivial for laptops.
- **D-07:** Parallel tool dispatch crosses the seam as a **batch method** (`invoke_batch(Vec<ToolCall>) -> Vec<Result>`; default impl sequential, SDK impl `join_all`). The platform maps ONE seam call onto durable `ctx.map`; parallelism stays a seam concern, the loop stays pure.
  - *Planning refinement (--reviews replan, 2026-07-17):* the SDK `invoke_batch` override uses **bounded** parallel dispatch (a `Semaphore`/`buffered(N)` cap) rather than an unbounded `join_all`, for DoS safety on large tool batches. Parallelism semantics are unchanged — this only caps simultaneous in-flight calls.
- **D-08:** Recorded effect traces are a **public serde artifact** (`EffectTrace` type in the crate): proptest generates them for the AGNT-03 replay-safety property, golden traces live as fixtures, and platform/debug capture-and-replay tooling can build on them later.
- **D-09:** **The pmcp.run durable-agent-lambda is the reference implementation and the validation target** (user-directed). Researcher/planner MUST study `~/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda` (esp. `src/handler/iteration.rs`, plus `src/llm/`, `src/mcp/`, `src/flow/`). Design the crate API so the durable lambda could adopt it and DELETE code (decision types first, then the loop — design §8.2 incremental path). Deliver a shape-compatibility mapping (which `pmcp-agent` types/functions replace which `iteration.rs` pieces) as a design-validation artifact. The actual platform migration stays out of SDK scope (DEFER-04); no private-repo code is copied into the open SDK (boundary razor).

### Agent-as-server adapter
- **D-10:** Agent runs are **task-augmented by default** (SEP-1686): the adapter's tool creates a task, returns `ToolOutput::Result` with top-level `related_task` `_meta` immediately, and the run progresses via `tasks/get` polling. Short runs may still complete synchronously. This is exactly the contract Phase 109's team-mcp composes (TEAM-05) and what the platform does today.
- **D-11:** Tool surface: **one conversational tool, package-driven** — name/description/input/output schemas come from the `AgentPackage` (`input_schema`/`output_schema` fields exist today; default = simple message input). Matches `team_mcp__<member>` one-tool-per-member composition.
- **D-12:** Conversation semantics: **fresh run per tool call**; continuity lives in the stores (`ConversationStore`; team memory via mem-mcp arrives in Phase 109). Adapter is stateless per call, matching platform team-member behavior.
- **D-13:** AGNT-07 deploy proof: **native example + CI wasm32 compile gate** for `pmcp-agent` (sans feature-gated HTTP sources) — proves the `ServerCore` path stays target-clean without building per-target deploy demos (those fit Phases 110/111).
  - *Planning refinement (--reviews replan, 2026-07-17):* the wasm32 gate proves the **loop + seams + zero-dep SamplingSource** are target-clean. The agent-as-server ADAPTER is **native-only** because `ServerCoreBuilder::task_store` (the store-backed task lifecycle the adapter needs) is `#[cfg(not(target_arch="wasm32"))]` in the shipped SDK; likewise the URL connector-client (`StreamableHttpTransport`) is native-only and opt-in behind a `url-connector` feature. The gate therefore proves loop/seam/source target-cleanliness, NOT adapter-on-wasm. AGNT-07's claim is scoped accordingly. (Roadmap unchanged.)

### Package config resolution (AGNT-09)
- **D-14:** Slot resolution is a **seam: `SlotResolver` trait** defined in `pmcp-agent`, with two shipped impls — an env-var-convention resolver and an explicit programmatic builder. cargo-pmcp wires `pmcp.toml` onto the same trait in Phase 110; the platform's DDB join is its own impl.
- **D-15:** Deviation from `tested_value` (pmcp-package already detects it): **warn and run** — log "tested on X, running on Y" clearly and proceed. Strict enforcement is host policy, not a loop concern.
- **D-16:** Connector refs (capture-time ranges) resolve via an **endpoint map supplied by the resolver** (name → URL/command); the `ToolInvoker` connects `pmcp::Client`s from that map. `WorkflowManifest` digest-pinning stays a platform/capture concern.

### Claude's Discretion
- Exact trait/type names, module layout, feature-flag names, and builder API shapes
- Retry-classification enum shape (mirror the `TaskPollDecision` "classification as data" precedent)
- Error taxonomy across the three seams (shared `RetryClass` accessor vs per-seam error types)
- `SamplingSource` wiring details over the Phase 106 host surface / server-side peer
- OpenAI-compat and Anthropic source internals (reqwest usage, no streaming requirement stated)
- TaskStore wiring for the adapter (reuse `with_task_store()` infrastructure; in-memory default)
- How the D-106-A response-pump is implemented inside `Server::run` (select-based, split task, etc.) — semantics in D-02 are the contract
- Example naming/numbering (`sNN_` convention) for the standalone-vs-sampled example

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & requirements
- `docs/design/agents-teams-sdk-extraction-plan.md` — approved milestone design; §2.1 verified SDK facts, §3 target architecture (load-bearing properties), §4 Phase C (this phase), §7 non-goals, §8.1–8.2 how pmcp.run adopts the loop
- `.planning/REQUIREMENTS.md` — AGNT-01..09 definitions + out-of-scope table
- `.planning/phases/106-client-host-surface/deferred-items.md` — D-106-A full technical detail (the deadlock this phase fixes)
- `.planning/phases/106-client-host-surface/106-CONTEXT.md` — locked host-surface decisions `SamplingSource` builds on

### Reference implementation (external, private repo — read, never copy)
- `~/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/handler/iteration.rs` — THE reference agent loop (~3.5k lines); its `IterationResult`, end-turn detection, retry class-split, and parallel `ctx.map` dispatch are the shapes to formalize
- `~/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/llm/`, `src/mcp/`, `src/flow/` — the platform's `CompletionSource`/`ToolInvoker`-shaped code the seams must be able to absorb

### Code (verified 2026-07-17)
- `crates/pmcp-package/src/package/agent.rs` — `AgentPackage` (wire-frozen 0.1): instructions, `llm: ConfigSlot`, limits, `connectors: Vec<ComponentRef>` (ranges, not pins), `input_schema`/`output_schema`, `budget_defaults`
- `crates/pmcp-package/src/slot/` — `ConfigSlot`/`SlotType`, classification, deviation detection (D-14/D-15 build on this)
- `src/types/sampling.rs` — `CreateMessageParams` incl. `tools`/`tool_choice` (MCP 2025-11-25); `CompletionSource` reuses these verbatim (AGNT-01)
- `src/client/host/` — Phase 106 host surface (`sampling.rs`, `elicitation.rs`, `roots.rs`) that `SamplingSource` and the hosted example use
- `src/types/tasks.rs:349` — `Task::poll_decision()` (the "classification as data" precedent AND the AGNT-08 mechanism)
- `src/client/mod.rs:771` — `Client::wait_for_task` (wasm-safe polling the tasks-aware `ToolInvoker` composes)
- `contracts/team-servers-v1.yaml` — PKG-03 contracts incl. `team_mcp__<member>` dispatch semantics the adapter must compose into (Phase 109 conformance runs against these)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ServerCore` + `ServerCoreBuilder` (`src/server/core.rs`, `builder.rs`) — the adapter builds on this, inheriting all transport adapters (Lambda/Docker/WASM)
- `ToolOutput::Result` (pmcp 2.12.0) — the verbatim `CallToolResult` pass-through the adapter uses for top-level `related_task` `_meta` (no raw JSON-RPC bypass needed)
- Phase 106 `ClientBuilder::on_sampling`/`on_elicitation`/`on_roots` + preflight approval — the host surface `SamplingSource` rides on
- `tests/common/duplex.rs` — duplex harness convention (Phase 104/105/106); extend for the real-loop D-106-A tests
- `pmcp-tasks` isolation precedent — separate 0.x crate, `serde_json::Value`/local-mirror-types tricks to avoid circular deps if needed

### Established Patterns
- "Non-determinism inside the step, pure classification outside" — the 2.13.0 `poll_decision` design D-05 generalizes
- Feature-gated optional deps (`openai-compat`, `anthropic` features mirror `dynamodb`/`redis` precedent)
- ALWAYS requirements: property + unit + fuzz + runnable example; `make quality-gate` before commits; PMAT cog ≤25
- Release: publish order gains `pmcp-agent` after `pmcp` (CLAUDE.md list needs the new entry); one release train per D-04

### Integration Points
- `Server::run` / `spawn_message_handler` message loop — where the D-02 response pump lands
- `src/server/peer_impl.rs` — server-side peer requests (`ServerRequest::CreateMessage`) that `SamplingSource` issues when hosted
- `crates/pmcp-agent/` — new workspace member (regular member, unlike workspace-excluded pmcp-package; decide in planning whether root gates lint it — note the rust-1.95 gate reality: only root `pmcp` is clippy-gated today)

</code_context>

<specifics>
## Specific Ideas

- **"Simplify the durable lambda" as the acid test** (user-stated): the best proof of the SDK agent implementation is that pmcp.run's durable-agent-lambda (and future implementations) could adopt it and get simpler. The shape-compatibility mapping deliverable (D-09) exists to demonstrate this without doing the migration.
- The standalone-vs-sampled example (AGNT-04/05/06 proof): SAME loop, two sources — a standalone runner on `OpenAiCompatSource` (e.g., Ollama) and the same agent hosted as a server sampling through its client — now runnable on the stock `Server::run` thanks to D-01..03.
- The platform's loop already validates the decomposition: `IterationResult { llm_response, assistant_message, tool_results_message, is_final }`, pure `stop_reason` end-turn match, "PURE, side-effect-free decision function" submit gate (design §4 Phase C) — the pure decision functions in D-05 should land close to these shapes to keep §8.2 adoption incremental.

</specifics>

<deferred>
## Deferred Ideas

- Per-target deploy demos (Lambda/Docker/WASM) of an agent — Phase 110/111 territory (D-13 keeps this phase to native example + wasm compile gate)
- `pmcp.toml` slot-resolver wiring — Phase 110 (`cargo pmcp agent dev`), on the D-14 trait
- Capture-and-replay debug tooling over `EffectTrace` — future; D-08 makes it possible without breaking changes
- Streaming completions in `OpenAiCompatSource`/`AnthropicSource` — not required this phase; revisit with real usage
- AgentCore deploy adapter (DEFER-01), additional sources (DEFER-02), platform migration (DEFER-04)

</deferred>

---

*Phase: 108-pmcp-agent-loop-crate*
*Context gathered: 2026-07-17*
