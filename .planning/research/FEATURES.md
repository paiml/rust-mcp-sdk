# Feature Research

**Domain:** MCP protocol SDK — adding 2026-07-28 (v2 / draft) spec support to the pmcp Rust SDK as a dual-version stack
**Researched:** 2026-07-22
**Confidence:** HIGH for wire shapes (traced to spec draft pages + changelog + SEP PRs); MEDIUM on final-vs-RC stability (RC locked 2026-05-21, final publishes 2026-07-28 — six days after this research)

## Scope Note

This is a SUBSEQUENT milestone. The pmcp SDK already speaks 2025-11-25 fully (initialize handshake, streamable-HTTP `stateless()`, experimental Tasks in `pmcp-tasks`, structured-output bridge, OAuth, client host surface, MCP Apps, Phase-109 conformance harness). The milestone shape is a **dual-version stack: per-request negotiation between 2025-11-25 and 2026-07-28**. Features below are ONLY the NEW v2-spec deltas. "Table stakes" here means "required to legitimately claim v2 conformance"; "differentiators" means "beyond-minimum things that make pmcp the reference dual-version SDK"; "anti-features" means "tempting but wrong for a dual-version stack."

## Critical RC-vs-Final Risk (read first)

The changelog's **error-code allocation policy** already renumbered the *new* draft error codes AGAIN after the RC: `HeaderMismatch -32001→-32020`, `MissingRequiredClientCapability -32003→-32021`, `UnsupportedProtocolVersion -32004→-32022`, plus a new range reservation (`-32020`..`-32099` = MCP spec, `-32000`..`-32019` = implementation-defined/grandfathered). The `-32002→-32602` resource-not-found rename is stable. **But** the tasks extension reference page still shows the *old* `-32003` for missing capability — evidence the schema is still settling between RC and final. **Do not hard-code the new `-3202x` codes until the 2026-07-28 final schema.json lands.** Treat all numeric error codes as a single centralized constant table, version-gated.

Source: [changelog "Minor changes" #12 + #6](https://modelcontextprotocol.io/specification/draft/changelog)

## Feature Landscape

### Table Stakes (Required for a legitimate v2 conformance claim)

| Feature | Wire shape / method | Why Expected | Complexity | Notes / pmcp dependency |
|---------|--------------------|--------------|------------|-------------------------|
| **Per-request `_meta` version+capabilities plumbing** | `_meta` keys `io.modelcontextprotocol/protocolVersion`, `io.modelcontextprotocol/clientCapabilities`, `io.modelcontextprotocol/clientInfo` on every request; results carry `io.modelcontextprotocol/serverInfo` | v2 is stateless — there is no handshake to carry these; every request self-describes | HIGH | Builds on Phase-109 additive `RequestMeta` `#[serde(flatten)]` namespaced map + `RequestHandlerExtra.request_meta`. That seam is EXACTLY the hook. Version dispatch reads `protocolVersion` from `_meta`. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |
| **`server/discover` RPC** | New method; server MUST implement; returns supported protocol versions + capabilities + identity. Client MAY call before anything else, or use as STDIO backcompat probe | The stateless replacement for capability discovery that `initialize` used to do | MEDIUM | Pure additive method on the router. Feeds the negotiation layer. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |
| **Drop `initialize`/`notifications/initialized` on v2 path** | Handshake removed; version/caps move to `_meta`; version mismatch → `UnsupportedProtocolVersionError` | Core of stateless architecture | HIGH | Dual-version: 2025-11-25 path KEEPS the handshake; v2 path must route without it. This is the branch point of the whole milestone. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |
| **Drop `Mcp-Session-Id` on v2 path** | Header removed; list endpoints (`tools/list`/`resources/list`/`prompts/list`) MUST NOT vary per-connection; cross-call state moves to server-minted handles passed as ordinary tool args | Any request lands on any instance; no sticky routing | MEDIUM | pmcp already has `stateless()` HTTP mode — this is the natural extension of that investment. [SEP-2567](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2567) |
| **Required headers `Mcp-Method`, `Mcp-Name`** | MUST appear on Streamable HTTP POST so gateways route without parsing the body. Plus `x-mcp-header` custom-header-from-tool-params support. `MCP-Protocol-Version` header continues from prior spec | Load balancers/rate-limiters need them; conformance suite checks them | LOW–MEDIUM | Header emission on client, validation on server; mismatch → `HeaderMismatch` error (`-32001`→`-32020`). [SEP-2243](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2243) |
| **MRTR: `InputRequiredResult` + `requestState`** | `result.resultType: "input_required"`, `result.inputRequests` (map of server-assigned key → `ElicitRequest`/`CreateMessageRequest`/`ListRootsRequest`), `result.requestState` (opaque server blob). Client retries the ORIGINAL request with `inputResponses` (same keys → `ElicitResult`/`CreateMessageResult`/`ListRootsResult`) and echoes `requestState`. **New JSON-RPC `id` on retry.** Only valid on `prompts/get`, `resources/read`, `tools/call` | Replaces ALL server-initiated requests (roots/sampling/elicitation) — breaking change, no shared storage | HIGH | This INVERTS pmcp's Phase-106 client host surface: today the server *initiates* `sampling/createMessage`/`elicitation/create`/`roots/list`; v2 folds them into a return value the client fulfills on retry. The Phase-106 handler registry becomes the client's `inputResponses` producer. `requestState` MUST be treated as attacker-controlled (HMAC/AEAD, principal binding, TTL, request-digest anti-replay). [SEP-2322](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2322) |
| **`resultType` on ALL results** | Required field: `"complete"` (ordinary) or `"input_required"` (MRTR interim). Clients MUST treat missing field (older servers) as `"complete"` | Discriminator that makes MRTR + Tasks polymorphic results parseable | MEDIUM | Serde-level: add `resultType` to result envelope; default-to-`complete` deserialization for backcompat. Tasks adds a third value `"task"`. [SEP-2322](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2322) |
| **Tasks-as-extension migration (`io.modelcontextprotocol/tasks`)** | `resultType: "task"`; `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`; poll via `tasks/get{taskId}` → full `Task{taskId,status,statusMessage,createdAt,lastUpdatedAt,ttlMs,pollIntervalMs,result?,error?,inputRequests?}`; `tasks/update{taskId,inputs:{key→response}}` for client-to-server input; `tasks/cancel{taskId}`. Status enum: `working`, `input_required`, `completed`, `failed`, `cancelled` (last 3 terminal). **`tasks/list` REMOVED.** `tasks/result` (blocking) REMOVED — replaced by polling `tasks/get`. Servers MAY return task handles UNSOLICITED (no per-request opt-in). Capability: `extensions: { "io.modelcontextprotocol/tasks": {} }` | The 2025-11-25 experimental Tasks API is superseded; conformance suite tests the extension shape | HIGH | The v1.x `pmcp-tasks` investment (TaskStore trait, DynamoDB/Redis backends, state machine) SURVIVES — this is an API RESHAPE, not a rewrite. Map old `tasks/result`→`tasks/get` polling (pmcp already polling-only, per PROJECT.md — good fit); DROP `tasks/list` routing; ADD `tasks/update` (new — client input into a running task); ADD unsolicited-handle path. Status enum differs from v1 5-state machine — needs a mapping table. [SEP-2663](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2663) · [ext-tasks](https://github.com/modelcontextprotocol/ext-tasks) · [tasks.extensions.modelcontextprotocol.io](https://tasks.extensions.modelcontextprotocol.io/) |
| **JSON Schema 2020-12 + unrestricted `structuredContent`** | `inputSchema`/`outputSchema` accept any 2020-12 keywords (`oneOf`/`anyOf`/`allOf`, conditionals, `$ref`). `structuredContent` accepts ANY JSON value (not just objects). Adds `$ref` resolution requirements + composition-keyword resource bounds | Richer tool contracts; conformance suite validates 2020-12 | MEDIUM | Directly affects the **2.15 structured-output bridge** (outputSchema→structuredContent). Bridge currently likely assumes object-valued structuredContent — must relax to any JSON value. `$ref` resolution + composition bounds are the real work. [SEP-2106](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2106) |
| **Caching hints `ttlMs` + `cacheScope`** | New `CacheableResult` interface: `tools/list`, `prompts/list`, `resources/list`, `resources/read`, `resources/templates/list` MUST return `ttlMs` (freshness hint, ms) + `cacheScope` (`"public"`\|`"private"`). Complements `listChanged` | Client-side cache; modeled on HTTP Cache-Control; conformance checks presence | LOW | Additive fields on 5 list/read results. "Required" on v2 path is the sharp edge — every list handler must emit them. [SEP-2549](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2549) |
| **Error code `-32002`→`-32602`** | Resource-not-found changes from MCP-custom `-32002` to JSON-RPC standard `-32602` (Invalid Params) | Conformance; clients matching literal `-32002` break | LOW | Version-gated constant. On 2025-11-25 path keep `-32002`; on v2 emit `-32602`. Part of the broader error-code table (see RC-vs-Final Risk above). [changelog Minor #6](https://modelcontextprotocol.io/specification/draft/changelog) |
| **Six auth-hardening SEPs** | (1) `iss` validation per RFC 9207 — client MUST validate present `iss` vs recorded issuer before redeeming code [SEP-2468]; (2) DCR `application_type` required [SEP-837]; (3) credentials keyed by issuer, MUST NOT reuse across AS, re-register on AS change [SEP-2352]; (4) refresh-token requests from OIDC servers documented [SEP-2207]; (5) scope accumulation during step-up clarified [SEP-2350]; (6) `.well-known` discovery suffix clarified [SEP-2351] | Auth conformance; real security fixes | MEDIUM | Builds on existing OAuth stack. (1)(2)(3) are code changes; (4)(5)(6) are clarifications/validation. Note: DCR itself is now DEPRECATED in favor of Client ID Metadata Documents [PR #2858] — but DCR stays for backcompat. Blog lists these as "the six." |
| **`extensions` capability map** | New `extensions` field on `ClientCapabilities` + `ServerCapabilities`; reverse-DNS IDs (e.g. `io.modelcontextprotocol/tasks`); negotiated in caps map | Tasks (and future extensions) negotiate through it | LOW–MEDIUM | Prereq for the Tasks extension capability. Additive to capability types. [SEP-2133 extensions framework](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2133) |

### Differentiators (Make pmcp the reference dual-version SDK)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Per-request version negotiation engine** | One server binary transparently serving BOTH 2025-11-25 and v2 clients, chosen per request from `_meta.protocolVersion` / `server/discover`. This is the milestone's headline. | HIGH | Central dispatch table keyed by protocol version; each protocol-version-sensitive behavior (handshake, error codes, session header, result envelope, tasks routing) branches here. Biggest architecture risk — needs its own phase. |
| **W3C / OpenTelemetry trace context in `_meta`** | `_meta` keys `traceparent`, `tracestate`, `baggage` propagate distributed traces across SDKs/gateways into OTel backends | LOW–MEDIUM | Documented convention, not a new type. pmcp `RequestMeta` flatten map already carries arbitrary namespaced keys — near-free to surface + honor. [SEP-414](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/414) |
| **`server/discover` as STDIO backcompat probe** | Lets a v2 client safely detect an old STDIO server and downgrade — a real dual-version UX win | MEDIUM | Client-side negotiation logic; pairs with the negotiation engine. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |
| **Conformance against the OFFICIAL v2 suite** | Phase-109 exportable wire-level conformance harness re-aimed at the official suite proves the claim, not just self-tests | MEDIUM | The Phase-109 `ConformanceTarget` (in-memory + HTTP) is the runway; align fixtures to official suite. |
| **`tasks/update` client-input-into-running-task** | Genuinely new capability (no v1 analog): feed input to a task mid-flight. Enables long-running interactive agent tasks — aligns with the `pmcp-agent`/teams surface. | MEDIUM | New route; `inputs` map mirrors MRTR `inputResponses` shape. Reuses the state-machine + store. |
| **`subscriptions/listen` unified change-notification stream** | Single long-lived POST-response stream replacing HTTP GET + `resources/subscribe`/`unsubscribe`; client opts into `toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/`resourceSubscriptions`; server tags with `io.modelcontextprotocol/subscriptionId` | HIGH | NOT in the six-phase cut per PROJECT.md, but it's a v2 transport change. Flag as scope question — may be needed for full v2 conformance. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |

### Anti-Features (Tempting but wrong for this milestone)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Hard cutover to v2 (drop 2025-11-25)** | "Cleaner, less branching" | Ecosystem still overwhelmingly 2025-11-25; final v2 not even published until 2026-07-28. Breaks every existing client. | Dual-version per-request negotiation (the stated milestone shape). Non-goal per PROJECT.md. |
| **Hard-coding the new `-3202x` error codes now** | "Spec says so" | RC still shifting (tasks page shows old `-32003`; allocation policy renumbered post-RC). | Centralized version-gated error-code constant table; fill v2 codes from FINAL schema.json. |
| **Rewriting `pmcp-tasks` for the extension** | "New Task shape, new methods" | The TaskStore trait, DynamoDB/Redis backends, CAS, security model all survive — only the wire API reshapes. | API-adapter layer over the existing store; map v1 5-state ↔ v2 status enum; add `tasks/update`, drop `tasks/list` routing. |
| **Implementing Roots/Sampling/Logging removal now** | "They're deprecated in v2" | They're DEPRECATED, not removed — 12-month minimum window; remain fully functional through every spec version published within a year. Removal needs separate SEPs. | Zero work this milestone (explicit non-goal). Optionally add `#[deprecated]` doc annotations only. [SEP-2577](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2577) · [SEP-2596 lifecycle](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2596) |
| **Keeping SSE resumability on v2** | "Don't lose in-flight requests" | v2 REMOVES `Last-Event-ID` + SSE event IDs; broken stream drops the in-flight request, client re-issues with new id. Retrofitting resumability fights the stateless model. | On v2 path, re-issue as new request; keep resumability only on the 2025-11-25 path if at all. [SEP-2575](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) |
| **Preserving server-initiated `sampling/createMessage` etc. on v2** | "Phase-106 host surface already does this" | v2 MRTR makes server-initiated requests unsupported — breaking change. `notifications/elicitation/complete` + `elicitationId` removed. | Fold Phase-106 handlers into the client's MRTR `inputResponses` producer; correlate via server-encoded `requestState`, not `elicitationId`. |
| **Per-connection list caching / stateful load balancing** | "Faster lists" | v2 requires list endpoints not vary per-connection; `Mcp-Session-Id` gone. | Deterministic-order `tools/list` + `ttlMs`/`cacheScope` caching hints (spec-blessed). |

## Feature Dependencies

```
extensions capability map (SEP-2133)
    └──requires──> Tasks-as-extension (SEP-2663)   [tasks negotiated via extensions map]

per-request _meta version+caps plumbing (SEP-2575)
    └──requires──> version negotiation engine
                       └──requires──> server/discover (SEP-2575)
                       └──gates──> initialize removal, Mcp-Session-Id removal,
                                   error-code branch, resultType envelope,
                                   tasks routing branch   [ALL version-gated behaviors]

resultType field (SEP-2322)
    └──requires──> MRTR InputRequiredResult ("input_required")
    └──requires──> Tasks resultType ("task")   [shared discriminator]

MRTR (SEP-2322) ──inverts──> Phase-106 client host surface
    [server-initiated sampling/elicitation/roots → client-fulfilled-on-retry]

Tasks-as-extension (SEP-2663) ──reshapes──> existing pmcp-tasks store/backends
    [tasks/list dropped, tasks/update added, status enum remapped]

JSON Schema 2020-12 (SEP-2106) ──affects──> 2.15 structured-output bridge
    [structuredContent: object → any JSON value]

six auth SEPs ──enhance──> existing OAuth stack   [additive validation]

W3C trace keys (SEP-414) ──enhances──> Phase-109 _meta flatten map   [near-free]
```

### Dependency Notes

- **Version negotiation engine is the keystone.** Nearly every v2 behavior (handshake removal, session-header removal, error codes, `resultType` envelope, tasks routing) is *version-gated* — the dispatch table must exist before any gated behavior can be conditionally applied. It is the correct Phase-1 target.
- **`resultType` is shared by MRTR and Tasks.** Both `"input_required"` and `"task"` ride the same result envelope discriminator; the serde envelope work is a common prerequisite (and must default missing→`"complete"` for backcompat).
- **MRTR inverts, not extends, Phase-106.** The Phase-106 handler registry (sampling/elicitation/roots handlers) becomes the *client's* machinery for producing `inputResponses` on retry, rather than answering server-initiated JSON-RPC requests. Same handlers, different trigger.
- **`pmcp-tasks` reshape, not rewrite.** PROJECT.md already made pmcp Tasks polling-only — a direct fit for v2's `tasks/get` polling (v2 dropped blocking `tasks/result`). The 5-state v1 machine needs a mapping to the v2 `working/input_required/completed/failed/cancelled` enum; `tasks/update` is genuinely new.
- **`extensions` map before Tasks extension.** Tasks negotiates through the `extensions` capability map, so that additive capability type must land first.

## MVP Definition

### Launch With (v2.5 core — the six-phase cut)

- [ ] **Version negotiation engine + per-request `_meta` plumbing** — keystone; everything gates on it (SEP-2575)
- [ ] **`server/discover` RPC** — stateless capability discovery replacement (SEP-2575)
- [ ] **`resultType` result envelope** (`complete`/`input_required`/`task`, default-missing→complete) — shared discriminator (SEP-2322)
- [ ] **Stateless streamable-HTTP v2 path** — drop `initialize`/`initialized` + `Mcp-Session-Id` on v2 (SEP-2575/2567)
- [ ] **Required headers** `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` + mismatch error (SEP-2243)
- [ ] **MRTR** `InputRequiredResult`+`requestState`+`inputResponses` with AEAD/HMAC state protection (SEP-2322)
- [ ] **Tasks extension** — `tasks/get`/`tasks/update`/`tasks/cancel`, drop `tasks/list`, unsolicited handles, `extensions` cap map (SEP-2663/2133)
- [ ] **JSON Schema 2020-12** + unrestricted `structuredContent` (bridge relaxation) (SEP-2106)
- [ ] **Caching hints** `ttlMs`/`cacheScope` on the 5 list/read results (SEP-2549)
- [ ] **Error-code rename** `-32002`→`-32602` (version-gated constant table)
- [ ] **Six auth-hardening SEPs** (iss/application_type/issuer-binding + 3 clarifications)
- [ ] **Conformance against official v2 suite** (re-aim Phase-109 harness)

### Add After Validation (v2.5.x)

- [ ] **W3C trace-context `_meta` keys** — near-free given the flatten map; add once core negotiation is stable (SEP-414)
- [ ] **`subscriptions/listen` unified stream** — SCOPE QUESTION: not in the six-phase cut but is a v2 transport change; add if full-conformance requires it (SEP-2575)
- [ ] **`x-mcp-header` custom-header-from-tool-params** — secondary part of SEP-2243

### Future Consideration (post-milestone)

- [ ] **Roots/Sampling/Logging deprecation annotations** — 12-month window; doc-only `#[deprecated]` at most, no removal
- [ ] **Client ID Metadata Documents** (replacing DCR) — DCR deprecated but backcompat-retained (PR #2858)
- [ ] **Removal SEPs tracking** — watch for the separate removal SEPs that will eventually land after the deprecation window

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Version negotiation engine + `_meta` plumbing | HIGH | HIGH | P1 |
| `resultType` envelope | HIGH | MEDIUM | P1 |
| `server/discover` | HIGH | MEDIUM | P1 |
| Stateless v2 HTTP (drop initialize/session-id) | HIGH | HIGH | P1 |
| Required headers | MEDIUM | LOW | P1 |
| MRTR (InputRequiredResult/requestState) | HIGH | HIGH | P1 |
| Tasks extension reshape | HIGH | HIGH | P1 |
| JSON Schema 2020-12 + structuredContent | MEDIUM | MEDIUM | P1 |
| Caching hints ttlMs/cacheScope | MEDIUM | LOW | P1 |
| Error-code rename -32002→-32602 | MEDIUM | LOW | P1 |
| Six auth SEPs | MEDIUM | MEDIUM | P1 |
| Conformance vs official suite | HIGH | MEDIUM | P1 |
| W3C trace context in _meta | LOW | LOW | P2 |
| subscriptions/listen | MEDIUM | HIGH | P2 (scope question) |
| Roots/Sampling/Logging deprecation notes | LOW | LOW | P3 |

**Priority key:** P1 = required for v2 conformance claim · P2 = should-have, add post-core · P3 = deferred (non-goal this milestone)

## Competitor Feature Analysis

| Feature | TypeScript SDK (reference) | Python SDK | pmcp (our approach) |
|---------|---------------------------|------------|---------------------|
| v2 dual-version stack | Reference impl tracks draft; likely v2-forward | Follows reference | **Per-request negotiation both versions in one binary** — differentiator |
| Tasks | `ext-tasks` reference repo (experimental) | TBD | Reshape existing `pmcp-tasks` store; keep DynamoDB/Redis backends |
| Conformance | Official conformance suite is the arbiter | — | Re-aim Phase-109 exportable harness at official suite |
| MRTR | Reference defines the pattern | — | Fold Phase-106 host surface into client `inputResponses` producer |

## Sources

- [Draft changelog (Key Changes 2025-11-25→draft)](https://modelcontextprotocol.io/specification/draft/changelog) — HIGH (authoritative, all 9 topic areas)
- [MRTR pattern page (exact InputRequiredResult/inputRequests/inputResponses/requestState wire shapes + security rules)](https://modelcontextprotocol.io/specification/draft/basic/patterns/mrtr) — HIGH
- [MCP Tasks Extension site (CreateTaskResult/Task/tasks/get/update/cancel wire shapes, status enum, capability key)](https://tasks.extensions.modelcontextprotocol.io/) — HIGH (still shows old -32003; see RC risk)
- [ext-tasks reference repo](https://github.com/modelcontextprotocol/ext-tasks) — HIGH
- [2026-07-28 Release Candidate blog post](https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/) — MEDIUM (RC locked 2026-05-21, final 2026-07-28; the six auth SEP enumeration)
- SEP PRs: [2575 stateless/discover](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2575) · [2567 session removal](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2567) · [2322 MRTR/resultType](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2322) · [2663 tasks](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2663) · [2106 JSON Schema 2020-12](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2106) · [2549 caching hints](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2549) · [2243 required headers](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2243) · [2133 extensions framework](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2133) · [414 trace context](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/414) · [2577 deprecations](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2577) · [2596 feature lifecycle](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2596) · Auth: [2468 iss](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2468) / [837 application_type](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/837) / [2352 issuer-binding](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2352)

---
*Feature research for: MCP 2026-07-28 (v2) dual-version support in pmcp Rust SDK*
*Researched: 2026-07-22*
