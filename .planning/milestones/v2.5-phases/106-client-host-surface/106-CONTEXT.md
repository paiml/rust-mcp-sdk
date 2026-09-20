# Phase 106: Client Host Surface - Context

**Gathered:** 2026-07-17
**Status:** Ready for planning
**Source:** PRD Express Path (docs/design/agents-teams-sdk-extraction-plan.md §4 Phase A, user-approved 2026-07-17 incl. §6 recommendations)

<domain>
## Phase Boundary

Make a pmcp `Client` able to **answer server→client requests** — the MCP spec's host side. Today any incoming server→client request errors as "Unexpected message type" (`src/client/mod.rs:2234`); there is no client-side handler registry at all. This phase adds one, covering spec-direction `sampling/createMessage` (including tools/tool_choice per MCP 2025-11-25), `elicitation/create`, and `roots/list`, plus a human-in-the-loop approval hook on sampling, and documents the legacy inverted sampling path as the distinct "LLM-server pattern".

Everything is **additive** — pmcp minor bump, zero breaking changes. This phase is independently shippable and unblocks Phase 108's `SamplingSource` (the `pmcp-agent` crate's zero-dep completion source).

Out of this phase: anything agent-loop shaped (Phase 108), any new wire methods, any change to the legacy inverted path's behavior (docs only).
</domain>

<decisions>
## Implementation Decisions

### Client host handler registry (locked)
- A `Client` can register a **client-side `SamplingHandler`** answering incoming spec-direction `sampling/createMessage` requests — including `tools`/`tool_choice` params and `tool_use`/`tool_result` content blocks (HOST-01)
- A `Client` can register an **`ElicitationHandler`** answering incoming `elicitation/create` (HOST-02)
- A `Client` answers **`roots/list`** from a registered roots provider (HOST-03)
- Dispatch replaces the `client/mod.rs:2234` "Unexpected message type" error path for registered handler types; unregistered request types get a proper JSON-RPC method-not-found/capability error, not a connection-level failure

### Sampling human-in-the-loop (locked)
- The client sampling path exposes an **async approval callback slot, defaulting to allow**, invoked before returning a completion, per the spec SHOULD (HOST-04). This is the seam consoles/approval flows plug into — no UI in this phase

### Capability advertising (locked)
- `ClientCapabilities` sent on initialize **reflect which host handlers are registered** (sampling/elicitation/roots) rather than being independently assertable (HOST-05). `ClientCapabilities.sampling` comes to mean "I can host"

### Legacy path disambiguation (locked)
- The existing inverted path (`Client::create_message` sending TO a server; server-side `SamplingHandler` in `src/server/mod.rs:353`/`traits.rs:62`) is **kept, not deprecated**, and documented as the **"LLM-server pattern"** — distinct from spec sampling — in rustdoc and the pmcp-book (HOST-06). Zero behavioral changes to it
- Naming note: the new client-side handler trait must not be confusable with the existing server-side `SamplingHandler` trait — pick distinct naming or distinct module paths with explicit docs (exact naming is Claude's discretion, the non-confusability requirement is locked)

### Quality bar (locked, house rules)
- Proven by a duplex round-trip harness test (server requests sampling → client handler answers); reuse `tests/common/duplex.rs`
- ALWAYS requirements apply: property tests, unit tests, fuzz where meaningful, runnable example (a sampling host example is the natural one — a client with a mock/OpenAI-compat handler serving a sampling-requesting server)
- `make quality-gate` green; additive API only

### Claude's Discretion
- Exact trait names, module layout, and builder API shape for handler registration (e.g., `ClientBuilder` methods vs setters)
- How the client's receive loop routes incoming `Request::Server` variants to handlers (dispatch mechanics, task spawning, ordering with in-flight response waits)
- Error taxonomy for unhandled request types (which JSON-RPC error code)
- Whether roots is a full handler trait or a simpler provider closure
- WASM client considerations — investigate whether the wasm client path needs the same surface now or is deferred (document either way)
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design & requirements
- `docs/design/agents-teams-sdk-extraction-plan.md` — the approved milestone design; §2.1 (verified SDK facts), §4 Phase A (this phase), §7 non-goals
- `.planning/REQUIREMENTS.md` — HOST-01..06 definitions

### Code (verified 2026-07-17)
- `src/client/mod.rs:2234` — the "Unexpected message type" rejection this phase replaces; `:1846` the legacy inverted `create_message`
- `src/types/protocol/mod.rs:509-519` — `ServerRequest` enum (CreateMessage / ListRoots / ElicitationCreate)
- `src/types/sampling.rs` — `CreateMessageParams` (incl. `tools`, `tool_choice` — MCP 2025-11-25), `ToolChoice`, `tool_use`/`tool_result` content
- `src/server/peer_impl.rs:58` — server-side dispatch of `ServerRequest::CreateMessage` (the requesting side that must round-trip)
- `src/server/elicitation.rs:84` — server-side elicitation request path
- `src/server/mod.rs:353` + `src/server/traits.rs:62` — the EXISTING server-side `SamplingHandler` (LLM-server pattern; do not confuse or break)
- `src/shared/protocol_helpers.rs:28,76` — where incoming requests parse into `Request::Server`
- `tests/common/duplex.rs` — shared duplex transport harness (Phase 104/105 convention)
</canonical_refs>

<specifics>
## Specific Ideas

- The duplex round-trip test shape: real `Server` (via its peer API) issues `sampling/createMessage` → real `Client` with a registered handler answers → server receives `CreateMessageResult`. Same for elicitation and roots.
- Example naming follows the `sNN_` convention in `examples/` (next free number), demonstrating a sampling host with a mock LLM handler.
- Book: this phase seeds the "Sampling & Hosting" chapter content (full chapter lands in Phase 111; HOST-06's disambiguation section lands NOW with the code per roadmap note).
- pmcp.run relevance (§8.5 of design doc): the durable host adopting client-side sampling later depends on exactly this surface — keep the handler traits object-safe and Send+Sync.
</specifics>

<deferred>
## Deferred Ideas

- `SamplingSource` (CompletionSource over this surface) — Phase 108
- Full "Sampling & Hosting" book chapter + examples beyond the one sampling-host example — Phase 111
- Any deprecation decision on the LLM-server pattern — explicitly kept as-is (design §6.4)
- AgentCore/deploy concerns — none in this phase
</deferred>

---

*Phase: 106-client-host-surface*
*Context gathered: 2026-07-17 via PRD Express Path*
