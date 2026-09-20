# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-24
**Phase:** 113-Stateless HTTP + Multi-Round-Trip Elicitation
**Areas discussed:** requestState integrity, MRTR client + host handlers, subscriptions/listen, MRTR type reuse

---

## requestState integrity

### State model
| Option | Description | Selected |
|--------|-------------|----------|
| Self-contained token | Full state serialized into requestState; server holds nothing; any instance resumes | ✓ |
| Server-stored + opaque handle | State stored server-side behind a signed handle; needs a shared store for multi-instance | |
| Pluggable, self-contained default | Self-contained default behind a codec trait so an app can plug a server store | |

### Crypto
| Option | Description | Selected |
|--------|-------------|----------|
| AEAD-encrypted | Encrypt+authenticate; state opaque to client/proxy; principal bound via AAD | ✓ |
| HMAC-signed (plaintext) | Sign-only; tamper-evident but state visible to client/proxy | |
| You decide | Defer sign-vs-encrypt to planning against final spec | |

### Key source
| Option | Description | Selected |
|--------|-------------|----------|
| Configured, per-process fallback | Builder key + random per-process fallback | |
| Always configured (required) | Mandatory key, no fallback | |
| Derive from existing auth secret | HKDF from the OAuth/signing key | |

**User's choice:** Other → "We use environment variables as a mechanism to inject keys or other per-mcp-server variables." → key via env var `PMCP_REQUEST_STATE_KEY`; unset → per-process random fallback + startup warning (option a), name confirmed by user.

### TTL
| Option | Description | Selected |
|--------|-------------|----------|
| ~5 min, configurable | Human-scale default with override | ✓ |
| Short (~60s), configurable | Tight default for fast/agent clients | |
| You decide | Human-scale default in planning | |

**Notes:** Self-contained + AEAD + env-injected key is the truly-stateless, multi-instance-safe path for pmcp.run behind a proxy. Fallback fails-closed → re-elicit (safe, degraded).

---

## MRTR client + host handlers

### Handler reuse
| Option | Description | Selected |
|--------|-------------|----------|
| Reuse HostElicitationHandler | Same callback for v1 interactive + v2 MRTR; client auto gather→resend | ✓ |
| Distinct MRTR callback | Separate client-side MRTR handler | |
| Reuse, but app-driven resend | Reuse to produce responses but surface resend to the app | |

### Client opt-in
| Option | Description | Selected |
|--------|-------------|----------|
| Mirror server accept-list | `Client::builder().with_protocol_version(...)`, symmetric | ✓ |
| Explicit v2 constructor/flag | Dedicated connect_v2 / flag | |
| You decide | Exact API in planning; lock = explicit, no auto-probe | |

### Loop bound
| Option | Description | Selected |
|--------|-------------|----------|
| Bounded, configurable | Cap rounds (~8) + override; typed error on exceed | ✓ |
| Unbounded (trust server) | No cap | |
| Handler decides | No numeric cap; handler aborts | |

**User's choice:** All three as above. **Key clarification the user added:** MCP servers serve TWO client shapes — (1) AI-chat with a human (ChatGPT/Claude Desktop) and (2) AI agents in a ReAct loop that can satisfy `inputRequests` programmatically from OTHER MCP servers without human input. MRTR must support the agent use case and must not assume/block on a human. (Also: user asked what "auto-resend" meant → clarified that the gather step IS the HostElicitationHandler call; the client only automates the plumbing, never invents answers.)

**Notes:** Loop bound reframed to protect both shapes (runaway agent OR human-spam). TTL (5 min) must cover an agent handler's own async MCP calls.

---

## subscriptions/listen

| Option | Description | Selected |
|--------|-------------|----------|
| Connection-scoped, no replay (push) | One long-lived streaming response; sub lives only while conn open | |
| Durable subscription id (push) | Server tracks sub across reconnects; needs shared store | |
| Poll-first; stream only if spec-forced | Polling over Tasks as the mechanism; push only if schema mandates; reword HTTP-04 | ✓ |
| Defer subscriptions entirely to 114 | Move change-notify delivery into Phase 114 | |

**User's choice:** Poll-first; stream only if spec-forced. **Key steer the user added:** PMCP focus is remote StreamableHTTP servers for many enterprise users; per-subscriber push streams don't scale / break LB affinity. Use the scalable, secure Tasks mechanism (poll-based) for long-running processes and subscription/listen use cases. Add push (single-local-user / long-running Docker) only if in the spec.

**Notes:** Flags a HTTP-04 wording tension vs Phase-118 conformance — research must verify the final 2026-07-28 schema before rewording/relocating HTTP-04.

---

## MRTR type reuse

| Option | Description | Selected |
|--------|-------------|----------|
| One handler type, adapt at wire | Keep ElicitRequestParams/ElicitResult public; adapt v2 wire internally | ✓ |
| Distinct public MRTR types | New public Mrtr* types mirroring the wire | |
| Extend Elicit* additively | Grow Elicit* with v2 fields, no adapter | |

**User's choice:** One handler type, adapt at wire. Consistent with reusing the handler; semver-stable; adapter absorbs spec drift.

---

## Claude's Discretion

- AEAD algorithm choice + key-length handling for `PMCP_REQUEST_STATE_KEY` (research flag: reconcile AEAD primitive with zero-new-runtime-deps / wasm-clean).
- Exact `inputRequests`↔`ElicitRequestParams` adapter mapping and its placement in the dispatch/serialization layer.
- HTTP-05 precise implementation (era-gate the EventStore off vs bypass replay); lock = no `Last-Event-ID` on v2 + ids derive from live request (regression test).
- How the `stateless()` era-gate threads `ProtocolContext`.
- Whether MRTR is exercised over stdio this phase (HTTP is the required surface).

## Deferred Ideas

- Long-lived push `subscriptions/listen` stream (websocket/SSE) for single-local-user / long-running-Docker — only if spec-mandated.
- MRTR over stdio — not required this phase.
- (Carried from 112) `server/discover` as a v1 upgrade probe — VERS-F1, deferred.
