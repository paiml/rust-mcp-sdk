# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation - Context

**Gathered:** 2026-07-24
**Status:** Ready for planning

<domain>
## Phase Boundary

v2 HTTP requests run with **no `initialize` handshake and no `Mcp-Session-Id`**, era-gated onto pmcp's **existing `stateless()` streamable-HTTP branch** (`src/server/streamable_http_server.rs:250`) — NOT a transport fork. **Multi-Round-Trip Elicitation (MRTR)** works end-to-end: a handler that needs more input returns a `resultType: input_required` result carrying `inputRequests` + an opaque `requestState`; the client gathers the answers and re-sends the original request with `inputResponses` + the echoed `requestState`, repeating until the operation completes. The pmcp `Client` becomes the v2-speaking counterpart (per-request `_meta`, `server/discover`, required headers, no `initialize`) and fulfils MRTR by folding in the Phase-106 host handlers. SSE resumability (`Last-Event-ID`) is NOT offered on the v2 path, with a regression test proving response ids always derive from the live request (closes the id-replay / discovery-cache bug class). Change-notification delivery (`subscriptions/listen`) is reconciled toward **polling over the Tasks mechanism**, not push (see D-11/D-12).

Requirements: HTTP-01..05, CLNT-01, CLNT-02. **v1 session behavior is untouched.** Builds directly on Phase 112's era gate / `ProtocolContext` spine.

**Emphasis (owner steer):** PMCP's focus is **remote StreamableHTTP servers serving many enterprise users** — decisions favor stateless, load-balancer-friendly, multi-instance-safe designs over per-connection push/session state. stdio and single-local-user/long-running-Docker shapes are secondary and added only if the spec requires them.

</domain>

<decisions>
## Implementation Decisions

### requestState integrity (HTTP-02 — "integrity-protected, principal-bound, TTL'd")
- **D-01:** `requestState` is a **self-contained token** — the full continuation state is serialized INTO the token; the server holds NOTHING between round trips, so any instance behind the proxy can resume. Directly serves the "stateless HTTP" thesis and pmcp.run's multi-instance deployment.
- **D-02:** The token is **AEAD-encrypted** (confidentiality + integrity), not sign-only — the continuation state (partial args, principal id, internal cursors) is OPAQUE to the client and any proxy. **Principal-binding is done via the AEAD AAD** (a different principal cannot replay another's token).
- **D-03:** The AEAD key is injected via the environment variable **`PMCP_REQUEST_STATE_KEY`** (consistent with how pmcp.run injects per-MCP-server config). When set, every instance shares the key → cross-instance retries decrypt and resume.
- **D-04:** When `PMCP_REQUEST_STATE_KEY` is **unset**, fall back to a **random per-process key** (zero-config for dev / single-instance / stdio) AND emit a **startup WARNING**. Consequence: a retry landing on a different instance cannot decrypt → **fail-closed → re-elicit** (safe, just degraded). No silent hard-error. (Chose fallback+warning over mandatory-key.)
- **D-05:** Default `requestState` **TTL ~5 minutes, configurable** (env/builder). Human-scale for interactive prompts AND wide enough for an autonomous agent handler doing its own async MCP calls before resending (D-07). Expiry is baked into the encrypted token and checked at decrypt time; an expired token **re-elicits cleanly**.

### MRTR client + host-handler folding (CLNT-01, CLNT-02)
- **D-06:** Fulfilling `input_required` **REUSES the existing Phase-106 `HostElicitationHandler`** (`src/client/host/elicitation.rs`), the SAME callback that answers interactive elicitation. The Client **auto-orchestrates the gather→resend loop**: parse `inputRequests` → call the handler → attach `inputResponses` + echoed `requestState` → resend the original request. If there is **no handler, or it declines/errors, the client does NOT resend** — it returns the `input_required` result to the caller. App authors write ONE elicitation callback that works on v1 (interactive) and v2 (MRTR).
- **D-07:** MRTR **must NEVER assume a human responder or block on human input.** Two client shapes are first-class: (a) **AI-chat clients** (ChatGPT / Claude Desktop) where a human is usually behind the handler; (b) **AI-agent clients** in a ReAct loop whose handler may satisfy `inputRequests` **programmatically from other MCP servers it holds**, with escalating to a human being just one strategy it may choose. The handler is an opaque async seam that returns responses however obtained. (This is why D-05's TTL must cover async agent work, and why D-08's bound protects BOTH shapes, not just "spamming a human".)
- **D-08:** Client-side v2 is opted into by **mirroring the server accept-list** — `Client::builder().with_protocol_version(PROTOCOL_VERSION_2026_07_28)` (symmetric with Phase-112 D-02's `with_supported_protocol_versions`). **Explicit per-connection; NO auto-probe** via `server/discover` to choose an era (CLNT-01 lock).
- **D-09:** The MRTR gather→resend loop is **bounded** — a small configurable default (≈8 rounds) with a builder override; exceeding it returns a **typed error (`MrtrRoundLimitExceeded`)** to the caller instead of re-invoking the handler. Prevents a buggy/hostile server from looping the client (re-prompting a human, or spinning an agent) indefinitely.

### MRTR type surface (HTTP-02/03)
- **D-10:** **One handler-facing type.** The reused handler continues to see the existing `ElicitRequestParams` / `ElicitResult` (`src/types/elicitation.rs`) for BOTH v1 interactive elicitation and v2 MRTR — a single public elicitation model, semver-stable. The **2026-07-28 wire shape** for `inputRequests`/`inputResponses` is **adapted to/from these types INTERNALLY at the dispatch/serialization boundary** (mirrors how Phase-112 injects `resultType`). The adapter absorbs spec drift; no new public elicitation type unless the wire shape is irreconcilable.

### subscriptions/listen — change notifications (HTTP-04)
- **D-11:** **Polling over the Tasks mechanism is THE pmcp mechanism** for v2 change notifications — stateless, load-balancer-friendly, multi-instance-safe. A per-subscriber held-open **push** stream (websocket/SSE-style) is explicitly the WRONG primitive for pmcp's enterprise remote-server focus (connection-stateful, breaks LB/instance affinity, doesn't scale). Aligns with the Tasks poll model landing in Phase 114.
- **D-12:** **RESEARCH ACTION + roadmap tension (HTTP-04 says "long-lived stream"; Phase 118 runs the official conformance suite).** Research MUST verify the final 2026-07-28 schema.json: **if `subscriptions/listen` streaming is optional / a poll shape is conformant → ship polling only and REWORD HTTP-04**; **if the spec HARD-mandates a stream for conformance → add a minimal conformant stream endpoint but keep polling as the recommended enterprise path.** Either way, flag HTTP-04 for a roadmap edit. (A long-lived push stream for single-local-user / long-running-Docker is DEFERRED — only if spec-mandated; see Deferred Ideas.)

### Post-research owner decisions (2026-07-24, resolves D-12 + research open questions)
- **D-13 (resolves D-12):** **Ship the opt-in `subscriptions/listen` stream in Phase 113.** Research confirmed the conformance mandate is capability-gated: servers advertising any of `tools.listChanged`/`prompts.listChanged`/`resources.listChanged`/`resources.subscribe` MUST serve the stream; advertising none and returning `-32601` is recorded SKIPPED-conformant. pmcp ships the long-lived SSE stream as an **opt-in** (default v2 capability set advertises none of these → conformant skip; a server that opts into listChanged capabilities gets the stream). D-11's polling-over-Tasks stance remains the recommended enterprise default; the stream is the conformant opt-in for servers that want push. HTTP-04 roadmap wording stands (stream ships), with the opt-in/capability-gated nuance noted.
- **D-14 (wasm scope):** MRTR `requestState` AEAD is **native-only this phase**, using `ring` (already a transitive dep of `streamable-http` via rustls — zero new crates; promote to explicit optional dep). `WasmServerCore` gets no MRTR in Phase 113; `chacha20poly1305` swap deferred until wasm MRTR is actually needed.
- **D-15 (expired-token behavior, refines D-05):** An **authentic-but-expired** `requestState` **re-elicits cleanly** (fresh `input_required` with new token), per D-05's original wording. Per research finding on `sep-2322-reject-tampered-state`: tampered or wrong-principal tokens with a **known key-id** MUST return a JSON-RPC error (conformance), while an **unknown key-id** (e.g. per-process fallback key on another instance, D-04) re-elicits. Key-id prefix on the token disambiguates the two cases.

### Claude's Discretion
- Exact env-var read/validation, AEAD algorithm choice, and key-length handling for `PMCP_REQUEST_STATE_KEY` (constraints locked: AEAD, principal-in-AAD, env-injected, per-process fallback+warning). **Research flag:** AEAD needs a crypto primitive — reconcile with the milestone's zero-new-runtime-deps constraint (STACK.md); prefer a wasm-clean pure-Rust AEAD already in the tree, or justify the dep.
- Exact `inputRequests`↔`ElicitRequestParams` adapter mapping and where it sits in the dispatch/serialization layer.
- Precise HTTP-05 implementation: whether the v2 path disables the `EventStore`/resumability code entirely (era-gated off) or just the `Last-Event-ID` replay — the lock is only that no `Last-Event-ID` is offered on v2 AND response ids always derive from the live request (regression test required).
- How the `stateless()` era-gate threads `ProtocolContext` onto the existing branch (Phase-112 spine already resolves era at ingress).
- Whether MRTR is exercised over stdio this phase — HTTP is the required surface (HTTP-02/03); stdio MRTR is not required (owner's StreamableHTTP focus).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone requirements & roadmap
- `.planning/ROADMAP.md` — v2.5 milestone section + Phase 113 detail (goal, HTTP-01..05 / CLNT-01..02 mapping, 5 success criteria). **HTTP-04 is flagged for a reword pending D-12 research.**
- `.planning/REQUIREMENTS.md` — HTTP-01..05, CLNT-01..02 full text; traceability table; Out of Scope (esp. "no hard-coding error codes before final schema").

### Phase 112 spine (direct dependency — READ FIRST)
- `.planning/phases/112-version-plumbing-spine/112-CONTEXT.md` — the carried-forward locks this phase builds on: accept-list opt-in (112 D-02), non-opted-in = zero era code (112 D-04), `resultType` injected at dispatch with `ResponseDisposition::InputRequired` already scaffolded (`src/server/core.rs:1108`, 112 D-08), transport-agnostic era (112 D-11), per-request version authoritative.
- `.planning/phases/112-version-plumbing-spine/112-VERIFICATION.md` — proof the spine (ProtocolContext at ingress, era gate, server/discover, required headers) is live.

### v2.5 research pack (2026-07-22, HIGH confidence)
- `.planning/research/SUMMARY.md` — architecture approach + the final-spec checkpoint discipline (wire-exact values only from the published 2026-07-28 schema.json). **D-12's subscriptions/listen reconciliation and D-10's MRTR wire shapes are exactly this kind of "verify against final schema" item.**
- `.planning/research/PITFALLS.md` — accidental-3.0 pitfall (`cargo semver-checks`/`cargo public-api` gate this phase — D-10's "one public type" keeps the surface additive).
- `.planning/research/STACK.md` — **zero-new-runtime-deps constraint** (reconcile against the AEAD primitive for D-02, Claude's-Discretion flag).

### Project context
- `.planning/PROJECT.md` — v2.5 dual-version framing; the enterprise-remote-StreamableHTTP focus that drives D-11.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`stateless()` branch** (`src/server/streamable_http_server.rs:250`, `session_id_generator: None`): the v2 request path era-gates ONTO this — HTTP-01 is a gate onto existing behavior, not a new transport. Session-generation gate sites at `:867/:910/:1088`.
- **`HostElicitationHandler`** (`src/client/host/elicitation.rs:18`, `handle_elicitation(params: ElicitRequestParams) -> Result<ElicitResult>`): the SAME callback MRTR reuses (D-06). Exported via `src/client/host/mod.rs:37`; registry at `ClientHostRegistry` (`:51`).
- **`ElicitRequestParams` / `ElicitResult` / `ElicitAction`** (`src/types/elicitation.rs`): the one handler-facing elicitation model (D-10); the v2 wire shape adapts to/from these.
- **`ResponseDisposition::InputRequired`** (`src/server/core.rs:1108`): the `resultType` discriminator scaffolded by Phase 112 specifically for this phase to wire; `inject_v2_result_envelope` is the injection point.
- **`ProtocolContext` at ingress + `RequestHandlerExtra` accessors** (Phase 112): era/principal already resolved once at ingress — MRTR's principal-binding (D-02) and era gate read from here.
- **`EventStore` / `Last-Event-ID` replay** (`src/server/streamable_http_server.rs:36`, `:2281` `replay_events_after`): the resumability path HTTP-05 must NOT offer on v2 (era-gate it off or bypass replay).
- **pmcp `Client`** (`src/client/mod.rs:113`): gains the v2 opt-in (D-08) + MRTR loop (D-06/D-09).
- **Tasks mechanism** (Phase 114 / existing `TaskRouter`, `src/types/tasks.rs`): the poll substrate D-11 builds change-notifications on.

### Established Patterns
- Per-request wiring at BOTH dispatch sites (`src/server/core.rs` + `src/server/mod.rs`) + wasm mirror parity — the Phase 109/112 precedent; MRTR's `input_required` emission + `inputResponses` ingestion follow it.
- Era-gated dispatch arms (Phase 112) — the mechanism that turns `Last-Event-ID`/session OFF on v2 while v1 stays byte-identical.
- Envelope injection at serialization (`inject_v2_result_envelope`) — where D-10's wire adapter and the `input_required` disposition live.
- `cargo semver-checks` / `cargo public-api` gate — D-10 keeps the public elicitation surface additive.

### Integration Points
- v2 HTTP ingress (Phase-112 era gate) → `stateless()` branch (no session) → dispatch → handler returns `input_required` → `requestState` (AEAD) minted at the response-assembly boundary.
- Client: `with_protocol_version(v2)` → send with `_meta`/headers → on `input_required` result, invoke `HostElicitationHandler` → resend original + `inputResponses` + `requestState` (bounded loop).
- Change-notifications → Tasks poll substrate (D-11), not a push stream.

</code_context>

<specifics>
## Specific Ideas

- **Multi-instance-behind-a-proxy is the design target**: `PMCP_REQUEST_STATE_KEY` shared across instances is what makes MRTR retries survive on pmcp.run; the per-process fallback (D-04) is dev-only and warns loudly.
- **Two concrete client shapes drive MRTR** (D-07): AI-chat-with-human AND autonomous ReAct agents that answer `inputRequests` from other MCP servers without human input. The design must not privilege the human path.
- **"Stateless" is the north star**: every reconciliation (self-contained token D-01, polling-not-push D-11, no-SSE-resumability HTTP-05) chooses the option that keeps the server holding no cross-request/cross-connection state.
- The v2 client opt-in should read symmetrically with the server builder DSL (D-08) — same mental model both sides.

</specifics>

<deferred>
## Deferred Ideas

- **Long-lived push `subscriptions/listen` stream (websocket/SSE-style)** for single-local-user or long-running-Docker deployments — DEFERRED; add ONLY if the final 2026-07-28 schema hard-mandates streaming for conformance (D-12). pmcp's enterprise-remote focus prefers polling-over-Tasks (D-11).
- **MRTR over stdio** — not required this phase (HTTP is the mandated surface); revisit if a stdio dev-loop use case demands it.
- (Carried from Phase 112) **`server/discover` answered on v1 connections as an upgrade probe** — stays deferred as VERS-F1.

</deferred>

---

*Phase: 113-Stateless HTTP + Multi-Round-Trip Elicitation*
*Context gathered: 2026-07-24*
