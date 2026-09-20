# Agents & Teams in the PMCP SDK — Extraction Plan

**Status:** DRAFT for discussion (not yet a GSD milestone)
**Date:** 2026-07-17
**Audience:** PMCP SDK maintainers; a companion note for the pmcp.run platform team will be derived from §8 once this plan stabilizes.

---

## 1. The boundary razor

Every SDK/platform boundary call to date follows one rule, and this plan applies it throughout:

> **Contracts and reference implementations belong in the open SDK; operation and scale belong in the platform.**

Precedents: MCP Tasks (SEP-1686 wire surface + `TaskStore` trait + in-memory store in the SDK; DynamoDB-backed durable store on pmcp.run), the SQL toolkit (library in SDK, hosted built-ins on the platform), deploy targets (adapters in cargo-pmcp, optimized AWS operation on pmcp.run).

Applied to the new concepts:

| Concept | SDK gets (contract + reference) | Platform keeps (operation) |
|---|---|---|
| Agent | Agent loop crate, `CompletionSource` trait, sampling + OpenAI-compat sources, agent-as-server adapter | Durable Lambda execution, `UnifiedLLMService`, DDB config resolution, Secrets Manager, budgets/pricing/obs |
| Team | Team-server tool contracts + dev-grade reference servers, in-process "small team" dev loop | S3/DDB/S3-Vectors backends, EventBridge scheduling, multi-tenant security, console, capture service |
| Package | `pmcp-package` format crate (canonical home: this repo) | Capture API, ECR registry operation, import service |
| LLM access | MCP sampling (spec direction) as the first-class path; thin fallback sources | Provider transformer registry, admin-approved model policy, key custody |

## 2. Grounding — verified facts this plan is built on

### 2.1 SDK (rust-mcp-sdk, v2.15.0)

- **Sampling wire types are complete and spec-correct**: `ServerRequest::CreateMessage` (`src/types/protocol/mod.rs:509-519`), `ModelPreferences` with hints and cost/speed/intelligence priorities, elicitation alongside as a server→client precedent.
- **Sampling carries tool calling** (`src/types/sampling.rs`): `CreateMessageParams.tools: Option<Vec<ToolInfo>>` + `tool_choice`, and `tool_use`/`tool_result` content blocks, per the MCP 2025-11-25 revision. The agent loop's full LLM turn (tools in, tool_use out) is expressible over spec sampling — `SamplingSource` needs no extension, and `CompletionSource` can reuse these types verbatim.
- **Servers can already request sampling from their client**: `src/server/peer_impl.rs:58` dispatches `ServerRequest::CreateMessage`.
- **The `Client` cannot host**: any incoming server→client request errors as "Unexpected message type" (`src/client/mod.rs:2234`). No client-side handler registry exists — sampling, elicitation, and `roots/list` are all unanswerable by a pmcp client today. This is a spec-compliance gap independent of agents.
- **A legacy direction-inverted path exists**: `Client::create_message` sends `sampling/createMessage` *to* the server; `SamplingHandler` lives on `Server`/`ServerCore`. Useful as an "LLM-as-a-server" pattern, but it is not spec sampling and must not be confused with the host path.
- **cargo-pmcp** has a mature command tree (`new`, `dev`, `deploy/`, `secret/`, `workbook/`, `validate`, `doctor`, …) that agent/team verbs slot into, and an established scaffold-pin tripwire convention for cross-crate version pins.
- **Tasks**: `Task::poll_decision()` / `wait_for_task` (2.13.0) and task-augmented results (2.12.0) are published — the durable polling contract the agent loop needs already exists in the SDK.

### 2.2 pmcp.run (private repo, inventoried 2026-07-17)

- **Durable agent Lambda** (`amplify/functions/durable-agent-lambda/`, ~42.6k lines): built on `lambda-durable-execution-rust` (`ctx.step` / `ctx.map` / `ctx.wait` / `wait_for_callback`). The agent loop lives in `handler/iteration.rs` (~3.5k lines): per-attempt memoized LLM calls, end-turn detection, parallel tool dispatch via `ctx.map`, retry class-split (capacity errors back off via durable wait). Uses `pmcp::Client<StreamableHttpTransport>` with client caching, `list_tools`/`call_tool`, SDK Tasks polled durably, and custom `AuthProvider` impls (Cognito internal/external, outbound OAuth).
- **There is no shared LLM crate.** The LLM layer is an in-crate module: `UnifiedLLMService` (reqwest), `ProviderConfig`, and a **trait-shaped transformer registry** (`anthropic_v1`, `openai_v1` — also serving xAI/DeepSeek — and `gemini_v1`). API keys come from **AWS Secrets Manager** (5-min TTL cache); model/provider/agent/team config is joined from **DynamoDB** tables (`AgentConfig`, `LLMModel`, `LLMProvider`, `McpServer`, `AgentTeam`).
- **Team servers** (`built-in/agents-api/servers/`, deployed via `cargo pmcp deploy`):
  - `team-mcp` — one `team_mcp__<member>` tool per member agent; `tools/list` computed per request from DDB via a per-request `pmcp::Server` (dynamic tool sets work fine). Member `tools/call` dispatch **bypasses `pmcp::Server`** with raw JSON-RPC — the in-code rationale (`main.rs:361-366`) is that the pre-2.12 `ToolHandler` API buried `_meta[related_task]` inside stringified `content[0].text`. **That rationale is obsolete**: `ToolOutput::Result` (pmcp 2.12.0, built for exactly this) lets a handler own the full `CallToolResult` envelope including top-level `_meta`. Migration is an SDK upgrade + adoption, not a new SDK capability.
  - `team-fs` — 11 `fs__*` tools over S3 + DDB (presigned URLs, review sync); built cleanly on `Server::builder()` per request.
  - `mem-mcp` — 6 `mem__*` tools over a vendored `mem0-rust` port (OpenAI embeddings + S3 Vectors, **already falls back to in-memory** when unconfigured; embedder configs include Mock/Ollama).
  - `approval-mcp` — static `resolve_approval`/`get_approval` plus dynamic per-human `team_approval__ask_*` tools; **already implements the SDK's `TaskStore` contract** over DynamoDB, with EventBridge expiry and HMAC callbacks.
- **`crates/pmcp-package`** (~4.2k lines, standalone, publish-ready pending review fixes): `AgentPackage` / `TeamPackage` / `ServerPackage` / `WorkflowManifest`; config slots split into identity-bearing (secret, oauth-client, channel-binding, human-role — structurally cannot hold values) and behavior-relevant (`LlmProvider`/`BudgetOverride` with `tested_value` + deviation detection); canonical-JSON digests; OCI layout pack/unpack.
- **MCP sampling is implemented nowhere in pmcp.run.** The agent calls providers directly via `UnifiedLLMService`.

## 3. Target architecture

```
                    host (owns LLM access, model policy, budget)
                      ▲ sampling/createMessage        │ tools/call
      ┌───────────────┴───────────┐                   ▼
      │  pmcp-agent loop           │  ← SDK crate: pure decision loop
      │  (instructions → decide    │     between effect seams
      │   tool calls → digest      │
      │   results → end turn)      │
      └──┬────────┬────────┬───────┘
         │        │        │           effect seams (traits):
         │        │        └── CompletionSource   (LLM)
         │        └─────────── ToolInvoker        (downstream MCP servers)
         └──────────────────── ConversationStore  (state/memory)

  SDK impls:  SamplingSource (spec, zero deps) · OpenAiCompatSource (feature-gated)
              PmcpClientInvoker (tasks-aware) · InMemoryStore
  Platform impls: UnifiedLLMService-backed source · durable ctx.step wrappers · DDB store
```

Two properties are load-bearing:

1. **The trait seams double as the durability seams.** pmcp.run wraps each effect (`create_message`, `call_tool`, store reads) in a memoized `ctx.step`; the loop between effects stays pure and replay-safe. The same loop code runs replay-safe on the platform and plain on a laptop, Docker, WASM, or AgentCore. This mirrors the 2.13.0 `poll_decision` design: non-determinism inside the step, pure classification outside.
2. **Sampling-first, not sampling-only.** When the agent is hosted (every team member behind `team-mcp`, or an agent exposed as a server to Claude Desktop/Code), `SamplingSource` gives it an LLM with zero keys and zero provider code. Standalone deployments fall back to `OpenAiCompatSource` (one implementation covers Ollama, vLLM, OpenRouter, xAI, DeepSeek, and most gateways) or a custom `CompletionSource`. The SDK never grows a provider matrix; pmcp.run's transformer registry remains the platform's own implementation of the trait.

## 4. Phases

Ordering follows the dependency spine: **compliance → contracts → agent → teams → CLI → docs**. Phases A and B are independent and can run in parallel; everything else is sequential in spirit (C needs A+B; D needs C; E needs B–D).

### Phase A — Client host surface (spec compliance; prerequisite, independently valuable)

**Goal:** a pmcp `Client` can answer server→client requests.

- Add a client-side handler registry dispatched from the client's receive path (replacing the `client/mod.rs:2234` error): `SamplingHandler` (client-side — new trait, host semantics), `ElicitationHandler`, roots provider. Capabilities advertised accordingly (`ClientCapabilities.sampling` should mean "I can host", which today it does not).
- Human-in-the-loop hook on the sampling handler (spec SHOULD): an async approval callback slot, defaulting to allow — the seam approval-mcp/consoles plug into.
- Disambiguate the legacy inverted path: keep `Client::create_message` + server `SamplingHandler` as the documented **"LLM-server pattern"** (rustdoc renamed conceptually, book section), explicitly distinct from spec sampling. No breaking changes.
- **Ships:** pmcp minor bump. Tests: duplex-harness round trip (server requests sampling → client handler answers), property tests on preference passthrough, example `sampling host` (client with a mock/OpenAI-compat handler serving a sampling-requesting server).

### Phase B — Contracts (`pmcp-package` + team tool contracts)

**Goal:** the portability contracts exist, versioned, wire-frozen, with the SDK repo as canonical home.

- **Adopt `pmcp-package` into this repo** (resolves asks 2–3 of the platform's publish request; the private-repo `repository` field problem disappears). Apply the pre-publish review fixes: public-facing description, README + license files, docs.rs check. Publish 0.1.0. Wire-freeze policy: 0.1.x = digest/serialization-stable (golden fixtures enforce), serialized-shape changes bump 0.2.0. cargo-pmcp depends on `"0.1"` (caret), not `=0.1.0`.
- **Team tool contracts**: extract the tool surfaces of the four team servers into a versioned contract document + conformance tests (names, input/output schemas, `_meta` conventions): `fs__*` (11 tools), `mem__*` (6), `team_mcp__<member>` naming + dispatch semantics, `resolve_approval`/`get_approval` + dynamic `team_approval__ask_*`. These are PMCP extensions — namespaced and documented as provisional, the same way Task variables and `diagnosticDetail` were, so working-group convergence replaces rather than collides.
- **Ships:** `pmcp-package` 0.1.0 on crates.io; `docs/design/team-server-contracts.md` (or contracts YAML per house contract-first convention).

### Phase C — `pmcp-agent` crate (the loop)

**Goal:** the agent runtime as an open, deploy-anywhere crate; pmcp.run's `handler/iteration.rs` becomes a platform-specific composition of it.

- New experimental crate `crates/pmcp-agent` (0.x, isolated like `pmcp-tasks` was planned; NOT folded into `pmcp` core until stable).
- **Effect seams** (traits): `CompletionSource` (`create_message(CreateMessageParams) -> CreateMessageResult` — reuse the SDK sampling types verbatim so `SamplingSource` is a passthrough), `ToolInvoker`, `ConversationStore`. All object-safe, all async.
- **The loop**: iteration engine extracted from the shape of `handler/iteration.rs` — LLM call → tool-call decision → parallel tool dispatch → result digestion → end-turn detection → iteration/budget limits. Pure between effects; no retry/backoff *policy* inside (retry classification exposed as data, like `poll_decision`, so the platform maps it onto durable waits and a standalone runner maps it onto tokio sleeps). The production loop already validates this decomposition: its per-iteration result is a small value type (`IterationResult { llm_response, assistant_message, tool_results_message, is_final }`), end-turn detection is a pure `stop_reason` match, and its submit-result gate is documented in-code as "a PURE, side-effect-free decision function" — the pure/effect split this crate formalizes is one the platform team already converged on.
- **Definition type**: the loop is configured by `AgentPackage` (from Phase B) plus resolved slots — instructions, model preferences, connector list, limits. One definition format from laptop to platform.
- **SDK-shipped sources**: `SamplingSource` (via the agent's server-side peer when hosted — zero deps), `OpenAiCompatSource` (feature `openai-compat`), optionally `AnthropicSource` (feature). Nothing else, ever; the trait is the extension point.
- **Agent-as-server adapter**: expose an agent as an MCP server (its "chat" tool + optional typed input/output schemas from the package), built on `ServerCore` so it deploys to Lambda/Docker/WASM like any server. This is the unit `team-mcp` composes N of.
- **Tasks-aware `ToolInvoker`**: wraps `pmcp::Client`, honors task-augmented results with `poll_decision` (SEP-1686) — long tool calls surface as pollable state, not blocking awaits.
- **Ships:** `pmcp-agent` 0.1.0. ALWAYS-requirements: property tests (loop determinism between effects: same effect results ⇒ same decisions — the replay-safety invariant, proptest over recorded effect traces), unit tests, fuzz on the message/tool-result digestion path, example `standalone agent vs sampled agent` (same loop, two sources).

### Phase D — Team reference servers

**Goal:** the four team servers exist as open reference implementations with dev-grade backends; "small team, one process" works locally.

Per-server plan (backend traits follow the `TaskStore` pattern — contract in SDK, dev backend in SDK, operated backend stays platform-side):

- **`team-fs`** — cleanest lift; already built on `Server::builder()`. Reference backend: local directory (std fs) behind a `TeamFsBackend` trait. Platform keeps S3/presign/review-sync as its impl.
- **`approval-mcp`** — already on the SDK `TaskStore` contract. Reference: in-memory `TaskStore` + console/stdin (dev) and webhook (CI) approval channels. Platform keeps DDB/EventBridge/HMAC.
- **`mem-mcp`** — contract + naive reference only: `TeamMemoryBackend` trait with a keyword/BM25 in-memory impl. **Do not open-source or depend on the vendored `mem0-rust`** (heavy, embedding-coupled, breaks WASM); its Mock/in-memory mode informs the trait shape, the crate stays platform-side.
- **`team-mcp`** — reference impl composes Phase C agent-as-server instances as members, with member dispatch as ordinary `ToolHandler`s returning `ToolOutput::Result` (top-level `related_task` `_meta`, task-augmented responses). No new SDK capability is needed — the production server's raw-JSON-RPC bypass predates `ToolOutput::Result` (2.12.0) and its stated rationale no longer holds; the reference impl doubles as the migration template.
- Packaging: one crate `crates/pmcp-team-servers` with per-server feature flags (avoids four new publish-order entries), binaries feature-gated for the dev loop.
- **Ships:** `pmcp-team-servers` 0.1.0; conformance tests proving reference and (via the Phase B contract fixtures) platform servers expose identical tool surfaces.

### Phase E — cargo-pmcp verbs

**Goal:** cargo-pmcp is the on-ramp for agents and teams, matching its server story.

- `cargo pmcp agent new` — scaffold an agent (AgentPackage manifest + standalone runner).
- `cargo pmcp agent dev` — run the loop locally (OpenAI-compat/Ollama source or as a sampling-hosted server under an interactive host).
- `cargo pmcp team dev` — the in-process small team: member agents + all four reference team servers with dev backends, wired from a `TeamPackage`. The local mirror of a pmcp.run team.
- `cargo pmcp package capture|show` — the platform team's requested subcommands, thin clients to the capture API, `pmcp-package = "0.1"` dep, with a scaffold-pin-style tripwire test against version drift.
- Deploy: agents deploy through the existing target adapters (an agent-as-server is just a server binary); **AgentCore lands later as one more deploy adapter** (like Azure Container Apps was) — explicitly out of scope for the first pass.
- **Ships:** cargo-pmcp minor bump.

### Phase F — Docs in three shapes + examples

Per house rule (README + pmcp-book chapter + pmcp-course chapter, leading with the CLI workflow):

- Book: "Agents as MCP Clients" (loop, sampling-first, CompletionSource), "Agent Teams" (four servers, small-team dev loop, when you've outgrown it → pmcp.run), "Sampling & Hosting" (Phase A host surface; LLM-server pattern disambiguation).
- Examples: sampling host, standalone agent, hosted (sampled) agent, small team end-to-end.
- README positioning: deploy-anywhere for agents/teams, preferred target pmcp.run.

## 5. Release & publish-order impact

- New publish-order entries: `pmcp-package` (leaf, before cargo-pmcp), `pmcp-agent` (after `pmcp`), `pmcp-team-servers` (after `pmcp-agent`). cargo-pmcp moves after all three.
- Version-pin tripwires: cargo-pmcp ↔ `pmcp-package` (new), agent scaffold ↔ `pmcp-agent` version (new, mirrors the workbook `PMCP_VERSION` tripwire).
- All new crates 0.x/experimental; `pmcp` core changes (Phase A) are additive minor bumps.

## 6. Open decisions (recommendation in bold)

1. Crate name for the agent runtime: **`pmcp-agent`** vs `pmcp-agents`.
2. Team servers: **one crate + feature flags** vs four crates.
3. `pmcp-package` first publish: **adopt into SDK repo first, publish from here** vs publish 0.1.0 from pmcp-run now and migrate later. (If the platform team is time-blocked on `capture`, publishing from pmcp-run with the four review fixes is acceptable; the repo move then happens before 0.2.)
4. Legacy inverted sampling: **keep + document as LLM-server pattern** vs deprecate.
5. `AnthropicSource` in-tree: **yes, feature-gated** (dogfooding + most common) vs OpenAI-compat only.
6. Contract format for team tools: **provable-contracts YAML** (house convention) vs markdown spec + conformance tests only.

## 7. Non-goals (this milestone)

- No provider matrix in the SDK (the trait is the extension point; three sources maximum).
- No open-sourcing of `mem0-rust`, `UnifiedLLMService`, the durable Lambda, budgets/pricing, capture service, or any operated backend.
- No distributed-team portability claim: deploy-anywhere teams are "small team, one process"; scale-out teams are the platform's product.
- No invented wire methods in working-group territory (matches the Ask-B posture on task elicitation): all extensions namespaced and provisional.
- No AgentCore adapter in the first pass (follow-on deploy target).

## 8. What pmcp.run adjusts (seed for the companion note — send after this plan is approved)

1. **`UnifiedLLMService` becomes a `CompletionSource` impl** (Phase C trait). No behavior change; the transformer registry, Secrets Manager custody, and DDB model policy all stay — they're just behind the trait now, and the durable `ctx.step` wrapper moves to the trait boundary (`llm-call-{attempt}` steps wrap `create_message`).
2. **`handler/iteration.rs` migrates to the `pmcp-agent` loop** incrementally: first adopt the loop's decision types (like `poll_decision` replaced hand-rolled `is_terminal`), then the loop itself, keeping platform-specific egress/budget/wrap-up as host code around it.
3. **`team-mcp` can drop the raw JSON-RPC bypass today** — its stated reason (no way to place `_meta[related_task]` at the `CallToolResult` top level through `ToolHandler`) was solved by `ToolOutput::Result` in pmcp 2.12.0. This is independent of the rest of the plan: upgrade the pinned pmcp and return `ToolOutput::Result` from the member-dispatch handler. Phase D's reference impl will be the worked example.
4. **Config joins gain a package-shaped spine**: `AgentConfig`/`AgentTeam` DDB rows align field-wise with `AgentPackage`/`TeamPackage` so capture (DDB → package) and import (package → DDB) stay lossless — the deviation-detection slots already anticipate this.
5. **Optional, later**: the durable host advertises client-side sampling (Phase A), letting hosted third-party agent code run key-less on the platform — the choke point for model policy, spend caps, and audit that admin-approved providers already implement.

## 9. Suggested execution path

Run as a GSD milestone (roadmap phases mapping ≈ A→F), one phase per release train, Phase A first since it's small, independently shippable, and unblocks C's `SamplingSource`. Contract-first per house rule: Phase B contracts precede C/D implementations.
