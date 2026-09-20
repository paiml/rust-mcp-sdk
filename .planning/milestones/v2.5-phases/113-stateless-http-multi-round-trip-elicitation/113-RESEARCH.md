# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation - Research

**Researched:** 2026-07-24
**Domain:** MCP 2026-07-28 (v2) stateless Streamable-HTTP transport semantics + Multi-Round-Trip Requests (MRTR / SEP-2322) in the pmcp Rust SDK
**Confidence:** HIGH (wire shapes read from the live draft schema AND the official conformance suite source; codebase claims verified by direct read) — with ONE dated caveat: the final spec publishes **2026-07-28, four days from this research date**. See "Final-Spec Checkpoint" below.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**requestState integrity (HTTP-02)**
- **D-01:** `requestState` is a **self-contained token** — the full continuation state is serialized INTO the token; the server holds NOTHING between round trips, so any instance behind the proxy can resume.
- **D-02:** The token is **AEAD-encrypted** (confidentiality + integrity), not sign-only — the continuation state (partial args, principal id, internal cursors) is OPAQUE to the client and any proxy. **Principal-binding is done via the AEAD AAD**.
- **D-03:** The AEAD key is injected via the environment variable **`PMCP_REQUEST_STATE_KEY`**. When set, every instance shares the key → cross-instance retries decrypt and resume.
- **D-04:** When `PMCP_REQUEST_STATE_KEY` is **unset**, fall back to a **random per-process key** (zero-config for dev / single-instance / stdio) AND emit a **startup WARNING**. Consequence: a retry landing on a different instance cannot decrypt → **fail-closed → re-elicit** (safe, just degraded). No silent hard-error.
- **D-05:** Default `requestState` **TTL ~5 minutes, configurable** (env/builder). Expiry is baked into the encrypted token and checked at decrypt time; an expired token **re-elicits cleanly**.

**MRTR client + host-handler folding (CLNT-01, CLNT-02)**
- **D-06:** Fulfilling `input_required` **REUSES the existing Phase-106 `HostElicitationHandler`** (`src/client/host/elicitation.rs`). The Client **auto-orchestrates the gather→resend loop**. If there is **no handler, or it declines/errors, the client does NOT resend** — it returns the `input_required` result to the caller.
- **D-07:** MRTR **must NEVER assume a human responder or block on human input.** Two client shapes are first-class: (a) **AI-chat clients**; (b) **AI-agent clients** in a ReAct loop whose handler may satisfy `inputRequests` **programmatically from other MCP servers it holds**.
- **D-08:** Client-side v2 is opted into by **mirroring the server accept-list** — `Client::builder().with_protocol_version(PROTOCOL_VERSION_2026_07_28)`. **Explicit per-connection; NO auto-probe.**
- **D-09:** The MRTR gather→resend loop is **bounded** — a small configurable default (≈8 rounds) with a builder override; exceeding it returns a **typed error (`MrtrRoundLimitExceeded`)**.

**MRTR type surface (HTTP-02/03)**
- **D-10:** **One handler-facing type.** The reused handler continues to see the existing `ElicitRequestParams` / `ElicitResult` for BOTH v1 interactive elicitation and v2 MRTR. The **2026-07-28 wire shape** for `inputRequests`/`inputResponses` is **adapted to/from these types INTERNALLY at the dispatch/serialization boundary**.

**subscriptions/listen — change notifications (HTTP-04)**
- **D-11:** **Polling over the Tasks mechanism is THE pmcp mechanism** for v2 change notifications — stateless, load-balancer-friendly, multi-instance-safe. A per-subscriber held-open **push** stream is explicitly the WRONG primitive for pmcp's enterprise remote-server focus.
- **D-12:** **RESEARCH ACTION + roadmap tension.** Research MUST verify the final 2026-07-28 schema.json: if `subscriptions/listen` streaming is optional / a poll shape is conformant → ship polling only and REWORD HTTP-04; if the spec HARD-mandates a stream for conformance → add a minimal conformant stream endpoint but keep polling as the recommended enterprise path. Either way, flag HTTP-04 for a roadmap edit.

### Claude's Discretion
- Exact env-var read/validation, AEAD algorithm choice, and key-length handling for `PMCP_REQUEST_STATE_KEY` (constraints locked: AEAD, principal-in-AAD, env-injected, per-process fallback+warning). **Research flag:** AEAD needs a crypto primitive — reconcile with the milestone's zero-new-runtime-deps constraint (STACK.md); prefer a wasm-clean pure-Rust AEAD already in the tree, or justify the dep.
- Exact `inputRequests`↔`ElicitRequestParams` adapter mapping and where it sits in the dispatch/serialization layer.
- Precise HTTP-05 implementation: whether the v2 path disables the `EventStore`/resumability code entirely (era-gated off) or just the `Last-Event-ID` replay — the lock is only that no `Last-Event-ID` is offered on v2 AND response ids always derive from the live request (regression test required).
- How the `stateless()` era-gate threads `ProtocolContext` onto the existing branch (Phase-112 spine already resolves era at ingress).
- Whether MRTR is exercised over stdio this phase — HTTP is the required surface (HTTP-02/03); stdio MRTR is not required.

### Deferred Ideas (OUT OF SCOPE)
- **Long-lived push `subscriptions/listen` stream (websocket/SSE-style)** for single-local-user or long-running-Docker deployments — DEFERRED; add ONLY if the final 2026-07-28 schema hard-mandates streaming for conformance (D-12).
- **MRTR over stdio** — not required this phase.
- (Carried from Phase 112) **`server/discover` answered on v1 connections as an upgrade probe** — stays deferred as VERS-F1.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| HTTP-01 | v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch; v1 session behavior unchanged | § "Pattern 1: Per-request era gate over a server-wide config" — identifies the **4** config-level session decision sites (not 3) that must become era-aware, plus the GET/DELETE → **405** rule that CONTEXT.md does not mention. Verified spec text at `/basic/transports/streamable-http`. |
| HTTP-02 | Handler returns `input_required` with `inputRequests` + opaque `requestState` that is integrity-protected, principal-bound, TTL'd | § "MRTR Wire Contract" (exact schema types), § "Pattern 3: requestState token design" (AEAD primitive selection + **key-id discriminator** that reconciles D-04 with the conformance suite's tamper test), § "Pitfall 2" (third binding the spec requires: originating-request identifier). |
| HTTP-03 | Client retry carrying `inputResponses` + echoed `requestState` resumes the operation | § "MRTR Wire Contract" — `inputResponses`/`requestState` are **top-level `params` siblings**, not `_meta`. Retry MUST use a **different JSON-RPC id**. Verified against conformance suite source. |
| HTTP-04 | v2 clients get change notifications via `subscriptions/listen` long-lived stream, replacing GET + `resources/subscribe`/`unsubscribe` | § "D-12 RESOLUTION" — the decisive finding of this research. Conditional mandate; two conformant configurations; **HTTP-04 must be reworded**. |
| HTTP-05 | SSE resumability (`Last-Event-ID`) not offered on v2; regression test proves response ids always derive from the live request | § "Pattern 4" — spec says verbatim "Resumable SSE streams via `Last-Event-ID` are not supported" and "A `Last-Event-ID` header: ignore it". Conformance's `sse-polling` scenario is tagged `removedIn: DRAFT`. |
| CLNT-01 | pmcp `Client` speaks v2 (per-request `_meta`, `server/discover`, required headers, no `initialize`) — selected explicitly per connection | § "Client Gap Inventory" — the client streamable-HTTP transport emits **zero** `Mcp-Method`/`Mcp-Name` headers today; `Accept` must list both content types; `Mcp-Name` needs the **base64 sentinel** encoder. |
| CLNT-02 | pmcp `Client` fulfills MRTR `input_required` by producing `inputResponses`; Phase-106 host handlers folded into the v2 flow | § "Pattern 5: MRTR client loop" — `inputRequests` carries **three** request kinds (`elicitation/create`, `sampling/createMessage`, `roots/list`), so the fold is a 3-way dispatch, not elicitation-only. Also flags the `mode`-optional deserialization bug in `ElicitRequestParams`. |
</phase_requirements>

---

## Summary

The 2026-07-28 draft schema and the **official conformance suite source** together give a nearly complete, unambiguous specification for everything this phase must build. Three findings dominate planning.

**First, D-12 is resolved and it resolves in a way that vindicates D-11's enterprise stance while requiring HTTP-04 to be reworded.** `subscriptions/listen` is fully specified in the draft schema (`SubscriptionsListenRequest`, `SubscriptionFilter`, `SubscriptionsListenResult`, `notifications/subscriptions/acknowledged`) and its response over Streamable HTTP **is** a long-lived SSE stream. There is **no polling shape for change notifications anywhere in the spec** — D-11's "polling over Tasks" is a pmcp extension, not a conformant substitute. However, the conformance suite gates the requirement on capability advertisement: a server that advertises none of `tools.listChanged` / `prompts.listChanged` / `resources.listChanged` / `resources.subscribe` may answer `subscriptions/listen` with `-32601` and the check is recorded **SKIPPED, not FAILURE**; a server that *does* advertise any of them and then answers `-32601` **FAILS** ("it claims a feature it does not serve"). So there are two conformant configurations, and pmcp's enterprise default (advertise nothing subscription-delivered on v2) is one of them.

**Second, MRTR is broader than elicitation.** `inputRequests` is a map whose values are full request objects with `method` + `params`, and the schema constrains those to exactly `ElicitRequest`, `CreateMessageRequest`, or `ListRootsRequest`. `inputResponses` is the symmetric map of `ElicitResult | CreateMessageResult | ListRootsResult`. Both ride as **top-level `params` fields** (`InputResponseRequestParams extends RequestParams`), sibling to `name`/`arguments` — not in `_meta`. MRTR applies to exactly three client requests (`tools/call`, `prompts/get`, `resources/read`) and the server **MUST NOT** return `input_required` on anything else; those are precisely the three methods Phase 112 already threaded `ProtocolContext`/`_meta` into, which is a fortunate alignment. Critically, the whole server-initiated-request direction is **removed** on v2: "Servers **MUST NOT** send independent JSON-RPC *requests* on this stream", so the Phase-106 host surface is not merely "folded in" — on the v2 path it is *replaced* by MRTR.

**Third, two locked decisions collide with hard conformance checks and need planner attention.** (a) The conformance scenario `input-required-result-tampered-state` appends `-TAMPERED` to a valid `requestState` and asserts the server returns a **JSON-RPC error**; it explicitly records that "returning a complete result **OR re-prompting (InputRequiredResult)**" is a FAILURE. D-04's "fail-closed → re-elicit" on decrypt failure would therefore fail conformance, because an AEAD decrypt failure from tampering and one from a wrong per-process key are indistinguishable. The fix is small and preserves both intents: prefix the token with a **non-secret key-id**; unknown key-id → re-elicit (different instance, D-04 satisfied), known key-id + auth failure → JSON-RPC error (conformance satisfied). (b) The spec adds a **third** replay binding beyond D-02's principal and D-05's TTL: "an identifier for the originating request, e.g. the method name and a digest of its salient parameters, rejecting state presented on a request that does not match."

**Primary recommendation:** Build the era gate as a **per-request override of the four server-wide session decision sites** in `streamable_http_server.rs` (not a fork, not a config swap); mint `requestState` as a `keyid.nonce.ciphertext` AEAD token using **`ring::aead`**, which is already a hard transitive dependency of the `streamable-http` feature (via `rustls/ring`) and therefore adds **zero crates to the dependency tree** for the target deployment; implement `subscriptions/listen` as a real SSE stream but keep v2 change-notification capabilities **off by default** so the enterprise-stateless default is the conformant-by-absence configuration; and reword HTTP-04.

---

## Final-Spec Checkpoint (read this before locking wire values)

| Fact | Status |
|------|--------|
| Today's date | 2026-07-24 [VERIFIED: session environment] |
| Final spec publication | **2026-07-28** — four days out [CITED: blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/] |
| A `schema/2026-07-28/` directory exists in the spec repo | **No** — only `2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25, draft` [VERIFIED: `gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema`] |
| Last change to `schema/draft/schema.ts` | 2026-07-16 — "feat(schema): add optional serverInfo response metadata and make clientInfo optional (#3002)" [VERIFIED: `gh api .../commits?path=schema/draft/schema.ts`] |

Everything quoted in this document comes from `schema/draft/schema.ts` @ main (the RC, 8 days stable at research time) and from the official conformance suite source, **not** from blog paraphrase. That is the strongest source available today. **Plan a Wave-0 task that re-diffs `schema/draft/schema.ts` (or the new `schema/2026-07-28/`) on or after 2026-07-28 and re-pins the conformance suite commit** — this is cheap insurance and matches the discipline `.planning/research/SUMMARY.md` already established.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Era resolution (v1 vs v2) per request | Transport ingress (`streamable_http_server.rs`) | — | Phase 112 already resolves `ProtocolContext` here; 113 consumes it, never re-resolves. |
| Session suppression / `Mcp-Session-Id` absence | Transport (HTTP) | — | Session minting/validation/echo lives entirely in the HTTP layer; era-gating it is a transport concern. |
| GET/DELETE → 405 on v2 | Transport (HTTP router) | — | HTTP-method-level behavior; no dispatch involvement. |
| `Last-Event-ID` suppression + EventStore bypass | Transport (HTTP) | — | Resumability is a transport-layer replay mechanism. |
| Response JSON-RPC id derivation | Transport (response assembly) | ServerCore dispatch | Invariant must hold at the point the response envelope is built — the layer where a cache could replay a stale id. |
| `resultType: "input_required"` emission | ServerCore dispatch/serialization (`inject_v2_result_envelope`) | — | Phase 112 D-08 locked the injection point; 113 selects the `InputRequired` disposition. |
| `inputRequests` / `inputResponses` wire adaptation | Protocol types (`src/types/`) + dispatch boundary | Handler-facing traits | D-10: adapter absorbs wire drift; handler traits stay stable. |
| `requestState` mint / verify (AEAD) | ServerCore response assembly (mint) + dispatch ingress (verify) | Transport (nothing) | Must be transport-agnostic so a future stdio MRTR reuses it; principal comes from `AuthContext`, era from `ProtocolContext`. |
| Principal resolution for AAD binding | Auth layer (`AuthContext`) | ProtocolContext | Reuses the existing per-request auth principal; **must not** fall back to session id (which does not exist on v2). |
| `subscriptions/listen` stream | Transport (HTTP, SSE) | ServerCore (notification source) | Stream lifecycle is transport; the notification events originate in ServerCore. |
| v2 request emission (`_meta`, headers, no initialize) | Client transport (`src/shared/streamable_http.rs`) | Client (`src/client/mod.rs`) | Headers are a transport binding concern; `_meta` construction is client-protocol. |
| MRTR gather→resend loop | Client (`src/client/mod.rs`) | Client host registry | Protocol-level retry semantics, transport-agnostic by design. |
| Host-handler dispatch for `inputRequests` | Client host registry (`src/client/host/`) | — | Phase-106 registry already routes the three request kinds. |

---

## Standard Stack

### Core (all already present; **zero new crates in the dependency tree**)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ring` | 0.17.14 | AEAD for `requestState` (`AES_256_GCM` or `CHACHA20_POLY1305`) with AAD principal binding | **Already a hard transitive dependency of the `streamable-http` feature**: `streamable-http = [..., "dep:rustls", ...]` and `rustls = { features = ["ring", ...] }`. Confirmed present in `Cargo.lock` @ 0.17.14, pulled by `rustls` and `rustls-webpki`. Promoting it to an explicit `optional = true` dep folded into `streamable-http` adds **zero crates** to the compiled tree for the phase's target deployment. [VERIFIED: Cargo.toml:114/186 + Cargo.lock lines 5784/5957/6020] |
| `getrandom` | 0.4.2 | CSPRNG for AEAD nonces and the per-process fallback key (D-04) | Already a **normal (non-optional, all-target)** dependency at `Cargo.toml:93`, deliberately so the wasm-safe PKCE helper links on every target. Same rationale applies to nonce generation. [VERIFIED: Cargo.toml:89-93] |
| `base64` | 0.22 | `requestState` token encoding; `Mcp-Name` base64-sentinel encoding (`=?base64?...?=`) | Already a normal dependency at `Cargo.toml:86`. The sentinel encoding is a **spec MUST** for non-ASCII names/URIs and is currently unimplemented on both sides. [VERIFIED: Cargo.toml:86 + spec Value Encoding section] |
| `serde` / `serde_json` | 1.0 | `InputRequests`/`InputResponses` maps, `requestState`, `SubscriptionFilter` | The protocol-type backbone. `preserve_order` is already on. |
| `axum` / `hyper` / `tokio-stream` | existing | `subscriptions/listen` SSE stream, 405 responses | `Sse::new(...)` is already used by `handle_get_sse`; the listen stream reuses that machinery with a different lifecycle. |
| `chrono` | 0.4 | TTL stamping/expiry inside the token (D-05) | Already a normal dependency. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ring::aead` | `chacha20poly1305` 0.11.0 (RustCrypto) | **Genuinely new crate** (+ `chacha20`, `poly1305`, `aead`, `universal-hash`, `cipher`), but **pure Rust and wasm32-clean**. Choose this **only if** MRTR must work on the wasm server (`WasmServerCore`) or on a build without `streamable-http`. For HTTP-only MRTR (the mandated surface, HTTP-02/03), `ring` is strictly cheaper. |
| `ring::aead` | `aes-gcm` 0.11.0 (RustCrypto) | Same tradeoff as `chacha20poly1305`; prefer ChaCha20-Poly1305 over AES-GCM in pure Rust because it has no timing-safety dependence on AES-NI. |
| AEAD (D-02) | HMAC-SHA256 sign-only (`hmac` 0.13.0 + already-present `sha2` 0.11) | The spec explicitly permits either ("e.g. HMAC or AEAD") and the conformance scenario says "integrity-protected requestState (e.g. **HMAC-signed**)". This would be the *smallest* dependency delta (one tiny pure-Rust crate). **But D-02 is LOCKED on AEAD for confidentiality** — the continuation state must be opaque to the client and any proxy. Recorded here only so the planner knows the fallback is conformant if the AEAD dep is ever rejected. |
| Explicit `ring` dep | Reuse `jsonwebtoken` (already optional, `jwt-auth`) to mint a JWE | `jsonwebtoken` 10.3 has **no JWE/encryption support** (signing only) and its dep list is `base64/getrandom/js-sys/pem/serde/serde_json/signature/simple_asn1` — no AEAD. Not viable. [VERIFIED: Cargo.lock:3464-3477] |

**Dependency-line change (the only one this phase needs):**

```toml
# Root Cargo.toml [dependencies]
ring = { version = "0.17", optional = true }

# [features]
streamable-http = ["dep:hyper", "dep:hyper-util", "dep:hyper-rustls", "dep:rustls",
                   "dep:futures-util", "dep:bytes", "dep:axum", "dep:tower",
                   "dep:tower-http", "dep:ring"]
#                                     ^^^^^^^^^^ NEW — already in the tree via rustls/ring
```

**Version verification performed:** `cargo search ring` → `0.17.14`; `cargo search chacha20poly1305` → `0.11.0`; `cargo search aes-gcm` → `0.11.0`; `cargo search hmac` → `0.13.0`; `cargo info` confirms source repos for all. [VERIFIED: cargo registry, 2026-07-24]

---

## Package Legitimacy Audit

> This phase installs **no npm packages**. The only candidate additions are Rust crates. `slopcheck` could not be installed in this environment (`slopcheck NOT available`), so per the graceful-degradation protocol the packages below are **not** tagged `[VERIFIED]` on slopcheck grounds alone — but each was independently confirmed via `cargo info` (registry presence, license, and a real upstream source repository) and, for `ring`, by direct presence in the repository's own `Cargo.lock`.

| Package | Registry | Age / Version | Source Repo | slopcheck | Disposition |
|---------|----------|---------------|-------------|-----------|-------------|
| `ring` | crates.io | 0.17.14 (crate since 2015) | github.com/briansmith/ring | unavailable | **Approved (recommended)** — already in `Cargo.lock` as a transitive dep of `rustls`; promotion to explicit dep adds no crate |
| `chacha20poly1305` | crates.io | 0.11.0 | github.com/RustCrypto/AEADs | unavailable | Approved (alternative, only if wasm/stdio MRTR is required) |
| `aes-gcm` | crates.io | 0.11.0 | github.com/RustCrypto/AEADs | unavailable | Approved (alternative, lower preference than ChaCha20) |
| `hmac` | crates.io | 0.13.0 | github.com/RustCrypto/MACs | unavailable | Approved (documented fallback only; contradicts locked D-02) |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

**Planner action:** because slopcheck was unavailable, add a `checkpoint:human-verify` before the `Cargo.toml` edit that adds `ring` (or any alternative). The check is one line: confirm `ring 0.17.14` already appears in `Cargo.lock` before the edit, proving no new crate enters the tree.

---

## The MRTR Wire Contract (authoritative)

All of the following is quoted or transcribed from `schema/draft/schema.ts` @ main and the MRTR spec page. [CITED: modelcontextprotocol.io/specification/draft/basic/patterns/mrtr, modelcontextprotocol.io/specification/draft/schema]

### Types

```typescript
// schema/draft/schema.ts:540-608
export type InputRequest  = ElicitRequest | CreateMessageRequest | ListRootsRequest;
export type InputResponse = CreateMessageResult | ListRootsResult | ElicitResult;

export interface InputRequests  { [key: string]: InputRequest;  }
export interface InputResponses { [key: string]: InputResponse; }

export interface InputRequiredResult extends Result {
  inputRequests?: InputRequests;
  requestState?: string;
}

/* Request parameter type that includes input responses and request state.
 * These parameters may be included in any client-initiated request. */
export interface InputResponseRequestParams extends RequestParams {
  inputResponses?: InputResponses;
  requestState?: string;
}

// schema/draft/schema.ts:214
export type ResultType = "complete" | "input_required" | string;
```

### Where the fields actually go

| Field | Location | Evidence |
|-------|----------|----------|
| `resultType: "input_required"` | inside `result` object | `InputRequiredResult extends Result`; `Result.resultType` is **required** ("Servers implementing this protocol version MUST include this field") |
| `inputRequests` | inside `result` object | `InputRequiredResult` |
| `requestState` (server→client) | inside `result` object | `InputRequiredResult` |
| `inputResponses` (client→server) | **top-level `params`**, sibling to `name`/`arguments`/`uri` | `InputResponseRequestParams extends RequestParams`; corroborated by conformance source: `sendRpc(url, 'tools/call', { name, arguments: {}, inputResponses: {...}, requestState })` |
| `requestState` (client→server echo) | **top-level `params`** | same |

**This is NOT in `_meta`.** Getting this wrong is the single most likely silent interop failure in the phase.

### Server obligations (spec MUST/SHOULD, verbatim intent)

1. MRTR applies to **exactly** `prompts/get`, `resources/read`, `tools/call`. "Servers **MUST NOT** send `InputRequiredResult` responses on any other client requests."
2. `inputRequests` keys are server-assigned and **MUST** be unique within the request scope; values **MUST** be one of `ElicitRequest`, `CreateMessageRequest`, `ListRootsRequest`.
3. Servers are free to encode `requestState` in any format ("base64-encoded JSON, encrypted JWT, serialized binary").
4. `requestState` from a client **MUST** be treated as attacker-controlled. If it influences authorization, resource access, or business logic, servers **MUST** protect its integrity (**e.g. HMAC or AEAD**) and **MUST reject state that fails verification**.
5. To prevent replay, servers **SHOULD** include **and verify** inside the integrity-protected payload: **(a)** the authenticated principal; **(b)** a short expiry (TTL); **(c) an identifier for the originating request, e.g. the method name and a digest of its salient parameters** — rejecting state presented on a request that does not match. A `<Warning>` notes these bound the replay window but do not guarantee single-use.
6. Servers **MUST** include at least one of `inputRequests` or `requestState` in every `InputRequiredResult`.
7. Servers **MUST NOT** send an `inputRequests` entry for a capability the client has not declared in `clientCapabilities` (→ use `-32021 MissingRequiredClientCapability`, HTTP 400, with `error.data.requiredCapabilities` as a **ClientCapabilities object** e.g. `{"sampling": {}}`, not an array).
8. Servers **MUST NOT** assume clients will fulfill or retry; a server **MAY** return `InputRequiredResult` repeatedly.
9. If the client under-supplies, the server **SHOULD** respond with a **new `InputRequiredResult`** re-requesting the missing information rather than an error.

### Client obligations

1. If `inputRequests` is present, the client **MUST** construct the inputs before retrying. If absent (requestState-only "load shedding"), the client **MAY** retry immediately.
2. The client **MUST** echo the exact `requestState` value and **MUST NOT** inspect/parse/modify it. If absent, the client **MUST NOT** include one in the retry.
3. **The JSON-RPC `id` MUST be different between the initial request and the retry** — they are independent requests.
4. `inputRequests`/`requestState` affect only the retry of the original request; **MUST NOT** be reused on parallel requests.

### Official conformance checks this phase is graded on

| Check id | What it asserts |
|----------|-----------------|
| `sep-2322-elicitation-incomplete` / `-complete` | `tools/call` → `input_required` w/ `elicitation/create`; retry with `inputResponses.user_name` → complete |
| `sep-2322-sampling-incomplete` / `-complete` | same for `sampling/createMessage` |
| `sep-2322-list-roots-incomplete` / `-complete` | same for `roots/list` |
| `sep-2322-request-state-incomplete` / `-complete` | round-trip with both `inputRequests` **and** `requestState` |
| `sep-2322-multiple-inputs-incomplete` / `-complete` | **multiple `inputRequests` of different kinds in one result** |
| `sep-2322-multi-round-r1/r2/r3` | **three** rounds with **evolving `requestState`** |
| `sep-2322-missing-response-rerequests` | under-supplied responses → server re-requests (obligation 9) |
| `sep-2322-non-tool-incomplete` / `-complete` | MRTR on a **non-`tools/call`** method (`prompts/get` or `resources/read`) |
| `sep-2322-result-type-included` | `resultType` is **explicitly present** and equals `"input_required"` |
| `sep-2322-not-on-unsupported-requests` | `input_required` never returned on other methods |
| `sep-2322-reject-tampered-state` | tampered `requestState` → **JSON-RPC error**; a complete result **or a re-prompt** is a FAILURE |
| `input-required-result-capability-check` | only sends `inputRequests` for declared client capabilities |

[VERIFIED: github.com/modelcontextprotocol/conformance `src/scenarios/server/input-required-result.ts`, 1644 lines, read directly]

---

## D-12 RESOLUTION — `subscriptions/listen` (HTTP-04)

### What the spec says

`subscriptions/listen` is fully specified and is a first-class member of `ClientRequest`:

```typescript
// schema/draft/schema.ts:3140-3150
export type ClientRequest =
  | DiscoverRequest | CompleteRequest | GetPromptRequest | ListPromptsRequest
  | ListResourcesRequest | ListResourceTemplatesRequest | ReadResourceRequest
  | SubscriptionsListenRequest | CallToolRequest | ListToolsRequest;
```

`resources/subscribe` and `resources/unsubscribe` are **gone** from the schema entirely — the only remaining mention is the comment "Replaces the former `resources/subscribe` RPC" on `SubscriptionFilter.resourceSubscriptions`.

Over Streamable HTTP the response to `subscriptions/listen` **is** a long-lived SSE stream: "Long-lived notification streams are obtained by sending a `subscriptions/listen` request. The server's response is itself an SSE stream that stays open." The stream protocol is:

1. Server **MUST** send `notifications/subscriptions/acknowledged` as the **first** message, carrying `_meta["io.modelcontextprotocol/subscriptionId"]`, and **MUST NOT** send any notification before it. Its `notifications` field reflects the subset the server agreed to honor.
2. Every subsequent notification carries the same `subscriptionId` (= the JSON-RPC id of the listen request).
3. Server **MUST NOT** send notification types the client did not explicitly request.
4. Request-scoped notifications (`progress`, `message`) are **not** delivered on the listen stream.
5. Graceful teardown: server **SHOULD** send the `SubscriptionsListenResult` (empty result + `_meta.subscriptionId`) as the JSON-RPC response before closing.
6. Servers are encouraged to emit SSE comment keep-alives (`:\r\n`) and **SHOULD** set `X-Accel-Buffering: no`.

**There is no polling shape for change notifications anywhere in the spec.** D-11's "polling over Tasks" is a valid pmcp extension but is **not** a conformant substitute for HTTP-04.

### The conditional mandate (the decisive finding)

The official conformance suite gates the requirement on capability advertisement. Verbatim from `src/scenarios/server/stateless.ts`:

```typescript
// A server that advertises no subscription-delivered capability has
// nothing to serve on subscriptions/listen, so a -32601 there is a
// legitimate feature absence (SKIPPED). A server that DOES advertise
// listChanged/subscribe but rejects the method fails: it claims a
// feature it does not serve.
const advertisesSubscriptions = !!(
  discoverCapabilities?.tools?.listChanged ||
  discoverCapabilities?.prompts?.listChanged ||
  discoverCapabilities?.resources?.listChanged ||
  discoverCapabilities?.resources?.subscribe
);
```

If `-32601` comes back on `subscriptions/listen`:
- `server/discover` was observed **and** advertises nothing subscription-delivered → **SKIPPED** (conformant).
- advertises something → **FAILURE** ("claims a feature it does not serve").
- `server/discover` was not observed → **FAILURE/untestable** (cannot attribute the gap to intentional absence).

[VERIFIED: github.com/modelcontextprotocol/conformance `src/scenarios/server/stateless.ts` lines 975-1015, read directly]

### Recommendation

**Implement both configurations; default to the conformant-by-absence one.**

1. **Default (enterprise remote, honors D-11's stance):** on the v2 path, do **not** advertise `tools.listChanged` / `prompts.listChanged` / `resources.listChanged` / `resources.subscribe` in the `server/discover` projection. `subscriptions/listen` returns `-32601` (HTTP 404, see Pitfall 5). Conformance records SKIPPED. Polling over Tasks remains the recommended pmcp mechanism, documented as an extension — not as the spec mechanism.
2. **Opt-in:** a builder method that turns v2 change-notification capabilities on. When on, `subscriptions/listen` **must** be implemented as a real SSE stream meeting checks 1-6 above. This is the "minimal conformant stream endpoint" branch of D-12, and it is unavoidable if any pmcp user wants `listChanged` on v2.
3. **A tripwire test** asserting the invariant: *advertised subscription capability ⇒ `subscriptions/listen` is implemented*. This is exactly the conformance rule, encoded locally so it can never silently drift.

**HTTP-04 must be reworded.** Suggested text:

> **HTTP-04**: On the v2 path, `resources/subscribe`/`unsubscribe` and the HTTP GET stream endpoint are removed. v2 change notifications are delivered via a `subscriptions/listen` long-lived stream (`toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/`resourceSubscriptions` opt-ins, `subscriptionId` tagging, `notifications/subscriptions/acknowledged` first). The stream is **opt-in**: pmcp's stateless enterprise default advertises no subscription-delivered capability, for which answering `subscriptions/listen` with method-not-found is conformant. A tripwire test enforces that advertising any subscription capability requires serving the stream.

**Deferred-idea status change:** CONTEXT.md's Deferred Ideas said the push stream is added "ONLY if the final schema hard-mandates streaming." The mandate is *conditional*, not absolute — so shipping the stream behind an opt-in (rather than not at all) is the correct reading, and the planner should confirm this scope expansion with the owner. It is the one place where research materially widens the phase.

---

## Architecture Patterns

### System Architecture Diagram

```
                         ┌──────────────── pmcp Client (v2 opt-in per connection) ────────────────┐
                         │                                                                          │
  caller ──call_tool()──▶│  build params {name, arguments, _meta{protocolVersion,                   │
                         │                clientInfo?, clientCapabilities}}                          │
                         │                        │                                                  │
                         │                        ▼                                                  │
                         │   ┌── MRTR loop (bounded, D-09) ────────────────────────────────┐        │
                         │   │  send ──▶ [transport] ──▶ ... ──▶ response                   │        │
                         │   │     ▲                              │                          │       │
                         │   │     │                   resultType?├── "complete" ──▶ return  │       │
                         │   │     │                              │                          │       │
                         │   │     │                              └── "input_required"        │       │
                         │   │     │                                     │                    │       │
                         │   │     │        ┌─── for each inputRequests[k] ───┐               │       │
                         │   │     │        │  method == elicitation/create ──▶ HostElicitationHandler
                         │   │     │        │  method == sampling/createMessage ▶ HostSamplingHandler(WithTools)
                         │   │     │        │  method == roots/list ───────────▶ RootsProvider
                         │   │     │        └──────────────┬──────────────────┘               │       │
                         │   │     │           no handler / decline / error ──▶ return result  │       │
                         │   │     │                       │                                   │       │
                         │   │     └── NEW id + params{..., inputResponses, requestState} ◀────┘       │
                         │   │        round++ ; round > limit ⇒ Err(MrtrRoundLimitExceeded)     │       │
                         │   └──────────────────────────────────────────────────────────────┘        │
                         └────────────────────────────┬─────────────────────────────────────────────┘
                                                      │  POST (Accept: application/json, text/event-stream)
                                                      │  MCP-Protocol-Version / Mcp-Method / Mcp-Name[b64 sentinel]
                                                      ▼
   ┌───────────────────────── Streamable-HTTP server transport (native only) ─────────────────────────┐
   │                                                                                                   │
   │  POST ─▶ classify_http_ingress ─▶ resolve ProtocolContext (Phase 112) ─▶ era?                     │
   │                                                                              │                     │
   │              ┌───────────────── era == V1 ────────────────────────────────────┘                    │
   │              │  session mint/validate/echo  •  EventStore  •  Last-Event-ID replay  (UNCHANGED)    │
   │              │                                                                                      │
   │              └───────────────── era == V2 ──────────────────────────────────────────────┐          │
   │                    • header gate (Mcp-Method/Mcp-Name, body cross-check) → -32020/400   │          │
   │                    • session sites 1-4 SUPPRESSED (no mint, no validate, no echo)       │          │
   │                    • Last-Event-ID ignored; EventStore bypassed                          │          │
   │                    • unknown method → HTTP 404 + -32601                                  │          │
   │                    • GET / DELETE → HTTP 405                                             │          │
   │                    • method == subscriptions/listen ─▶ SSE listen stream (opt-in)        │          │
   │                              ack ▶ notifications(filtered, subscriptionId) ▶ result      │          │
   │                    └──────────────────────────┬──────────────────────────────────────────┘          │
   └───────────────────────────────────────────────┼──────────────────────────────────────────────────┘
                                                   ▼
   ┌───────────────────────── ServerCore dispatch (tools/call, prompts/get, resources/read) ───────────┐
   │                                                                                                    │
   │  params.requestState present? ──▶ verify_request_state(token, principal, method, param_digest)     │
   │       ├─ unknown key-id       ──▶ re-elicit (fresh InputRequiredResult)      [D-04 degraded path]  │
   │       ├─ auth/tag failure     ──▶ JSON-RPC error (-32602)                    [conformance MUST]    │
   │       ├─ TTL expired          ──▶ JSON-RPC error (-32602)                                          │
   │       ├─ principal mismatch   ──▶ JSON-RPC error (-32602)                                          │
   │       └─ ok ──▶ continuation state ──┐                                                             │
   │                                       ▼                                                             │
   │  handler(args, continuation, inputResponses) ──▶ Complete | NeedsInput{inputRequests, state}       │
   │                                                          │                                          │
   │                          mint_request_state(state, AAD=principal|method|param_digest, TTL) ◀────────┤
   │                                                          ▼                                          │
   │  inject_v2_result_envelope(disposition = InputRequired)  ⇒ resultType + _meta.serverInfo            │
   └────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| File | Responsibility in this phase |
|------|------------------------------|
| `src/server/streamable_http_server.rs` | Era-gate the 4 session sites; 405 on GET/DELETE for v2; ignore `Last-Event-ID`; bypass `EventStore`; 404+`-32601`; `subscriptions/listen` SSE route |
| `src/server/core.rs` | Select `ResponseDisposition::InputRequired`; mint/verify `requestState`; adapt `inputRequests`/`inputResponses` at the dispatch boundary; **fix `serverInfo` placement** |
| `src/server/mod.rs` | Twin wiring of the above at the high-level `Server` dispatch site (Phase 109/112 precedent) |
| `src/types/tools.rs`, `src/types/prompts.rs`, `src/types/resources.rs` | Additive `input_responses` / `request_state` `Option` fields on the 3 MRTR-eligible request structs |
| `src/types/mrtr.rs` (new) | `InputRequests`/`InputResponses`/`InputRequest`/`InputResponse` types + the `ElicitRequestParams` adapter |
| `src/types/protocol/error_codes.rs` | Add `-32020`, `-32021`, `-32022` |
| `src/server/request_state.rs` (new) | AEAD mint/verify, key resolution (`PMCP_REQUEST_STATE_KEY` + fallback + warning), key-id, TTL, AAD composition |
| `src/shared/streamable_http.rs` | Client: emit `Mcp-Method`/`Mcp-Name` (+ base64 sentinel), correct `Accept`, suppress session id on v2 |
| `src/client/mod.rs` | `with_protocol_version` opt-in; per-request `_meta`; the bounded MRTR loop |
| `src/client/host/mod.rs` | 3-way dispatch of `inputRequests` onto the existing registry |

### Pattern 1: Per-request era gate over a **server-wide** config

**What:** `stateless()` is a *constructor-level* config, not a per-request mode. `StreamableHttpServerConfig::stateless()` sets `session_id_generator: None, event_store: None` once at build time. A dual-version server built with `Default::default()` has `session_id_generator: Some(...)` and would mint a session for a v2 request unless the era overrides it per request.

**When to use:** everywhere the code currently branches on `session_id_generator.is_some()/is_none()`.

**The four sites** (all in `src/server/streamable_http_server.rs`):

| Site | Line | Current predicate | v2 behavior |
|------|------|-------------------|-------------|
| `process_init_session` | ~867 | `if let Some(generator) = &state.config.session_id_generator` | never reached on v2 (no `initialize`), but must be defensive |
| `validate_non_init_session` | ~910 | `if state.config.session_id_generator.is_some()` | must **not** require or validate a session id |
| `validate_protocol_version_matches_session` | ~1088 | `if state.config.session_id_generator.is_none() { return Ok(()) }` | must short-circuit on v2 — per-request version is authoritative (Phase 112 lock) |
| response header emission | ~1495 | `if let Some(sid) = response_session_id { headers.insert(MCP_SESSION_ID, ...) }` | must **never** emit `Mcp-Session-Id` on v2 |

**Recommended shape:** a single helper `fn sessions_active(state: &ServerState, era: Option<Era>) -> bool` = `!matches!(era, Some(Era::V2)) && state.config.session_id_generator.is_some()`, and route all four sites through it. One predicate, one place to test, no fork. This directly implements CONTEXT.md's "era-gated onto the existing `stateless()` branch, NOT a transport fork."

### Pattern 2: GET / DELETE → 405 on v2 (**not in CONTEXT.md — new requirement from spec**)

Spec, verbatim: "A server that supports only this revision and receives such traffic from an older client **SHOULD** respond as follows: HTTP GET or DELETE to the MCP endpoint: respond with `405 Method Not Allowed`. An `Mcp-Session-Id` header on a request: ignore it, and do not mint or echo session IDs. A `Last-Event-ID` header: ignore it; streams are not resumable."

pmcp is **dual-version**, so GET/DELETE must keep working for v1. GET has no body, hence no `_meta` — the only era signal is the `MCP-Protocol-Version` header. **Rule:** `MCP-Protocol-Version: 2026-07-28` on GET or DELETE → 405; otherwise existing behavior. Current router: `.route("/", get(handle_get_sse)).route("/", delete(handle_delete_session))` at `streamable_http_server.rs:290-291`.

### Pattern 3: `requestState` token design (reconciles D-01..D-05 with conformance)

**Token layout (recommended):**

```
requestState = base64url( key_id_len(1) || key_id || nonce(12) || AEAD_seal(plaintext, AAD) )

plaintext = serde_json of { continuation state, exp: unix_ts, round: u8 }
AAD       = principal_id ‖ 0x00 ‖ method ‖ 0x00 ‖ sha256(salient_params)
```

**Why the `key_id` prefix is load-bearing:** it is the *only* thing that lets the server distinguish "wrong instance's per-process key" (D-04 → re-elicit) from "tampered token" (conformance → JSON-RPC error). Without it, an AEAD auth failure is a single undifferentiated outcome and D-04's re-elicit contract would fail `sep-2322-reject-tampered-state`. Derive `key_id` as, e.g., the first 8 bytes of `sha256(key)`; it is non-secret and leaks nothing.

**Verification decision table:**

| Condition | Outcome | Driver |
|-----------|---------|--------|
| key_id unknown to this instance | fresh `InputRequiredResult` (re-elicit) | D-04 |
| key_id known, AEAD tag fails | JSON-RPC error `-32602` | conformance `sep-2322-reject-tampered-state` |
| decrypts, `exp` in the past | JSON-RPC error `-32602` | spec 5(b) "rejecting state presented after it lapses" |
| decrypts, AAD principal ≠ live principal | JSON-RPC error `-32602` | D-02 + spec 5(a) |
| decrypts, AAD method/param-digest ≠ live request | JSON-RPC error `-32602` | **spec 5(c) — not in CONTEXT.md** |
| all pass | resume | — |

**Note on D-05's "an expired token re-elicits cleanly":** that phrasing conflicts with the table above. Once integrity has *passed*, a clean re-elicit is defensible (the client learns nothing) and the conformance suite only tests the tamper case. But the spec says "rejecting state presented after it lapses," and returning an error is unambiguously safe. **Recommendation: error on expiry; document the choice.** Flag for planner/owner confirmation.

**Key resolution (D-03/D-04):**

```rust
// Pseudocode — src/server/request_state.rs
fn resolve_key() -> (KeyId, aead::LessSafeKey) {
    match std::env::var("PMCP_REQUEST_STATE_KEY") {
        Ok(raw) => {
            let bytes = decode_key(&raw)?;           // base64 or hex; require exactly 32 bytes
            (key_id_of(&bytes), unbound(bytes))
        }
        Err(_) => {
            let mut k = [0u8; 32];
            getrandom::fill(&mut k).expect("CSPRNG");
            tracing::warn!(
                "PMCP_REQUEST_STATE_KEY is unset — using a random per-process key. \
                 MRTR retries that land on a different instance cannot be resumed and \
                 will re-elicit. Set PMCP_REQUEST_STATE_KEY for multi-instance deployments."
            );
            (key_id_of(&k), unbound(k))
        }
    }
}
```

Resolve **once at server build**, not per request. Support a key **set** (current + previous key_id) so key rotation does not strand in-flight tokens — cheap now, expensive to retrofit.

### Pattern 4: HTTP-05 — no resumability, ids always live

Two independent obligations:

**(a) No `Last-Event-ID` on v2.** Recommended implementation: on the v2 path, do not read `LAST_EVENT_ID` at all and do not call `replay_events_after` / `replay_sse_events_from_header`. Leave the `EventStore` code and the `LAST_EVENT_ID` constant intact for v1 (a Phase-117/SMPL-01 severability concern, not this phase's). This is the lighter of the two options in Claude's Discretion and it minimizes v1 blast radius. Current call sites: `replay_events_after` at `streamable_http_server.rs:48/94`, `replay_sse_events_from_header` at `:2376` (inside `handle_get_sse`, which is 405 on v2 anyway).

**(b) Response id always derived from the live request.** This is the general fix for the documented `pmcp_run_proxy_discovery_cache_id_bug` class. The regression test must prove that a `result` reused/cached across callers is **re-enveloped** with the live request's id — i.e. cache the `result` payload, never the JSON-RPC envelope. Note the MRTR spec independently reinforces this: retries **MUST** use a different id, so any id-replay would be visible immediately.

The official suite corroborates both: the `sse-polling` scenario carries `readonly source = { introducedIn: '2025-11-25', removedIn: DRAFT_PROTOCOL_VERSION }`, and `stateless.ts` asserts "All error returns must preserve original request ID mappings."

### Pattern 5: MRTR client loop (CLNT-02)

```rust
// Pseudocode — src/client/mod.rs
async fn send_with_mrtr(&self, mut params: Params) -> Result<Value> {
    for round in 0..self.mrtr_round_limit {          // D-09, default 8
        let result = self.send_request_v2(&params).await?;   // NEW id each time (spec MUST)
        match result_type(&result) {
            ResultType::Complete | ResultType::Task => return Ok(result),
            ResultType::InputRequired => {
                let Some(reqs) = result.get("inputRequests") else {
                    // requestState-only "load shedding": retry immediately
                    params.request_state = result.get("requestState").cloned();
                    continue;
                };
                let mut responses = Map::new();
                for (key, req) in reqs {
                    match req["method"].as_str() {
                        "elicitation/create"     => { /* HostElicitationHandler */ }
                        "sampling/createMessage" => { /* HostSamplingHandlerWithTools, else adapter */ }
                        "roots/list"             => { /* RootsProvider */ }
                        _ => return Ok(result),   // unknown kind: hand back, do not resend
                    }
                }
                // No handler, or handler declined/errored ⇒ return `result` (D-06)
                params.input_responses = Some(responses);
                params.request_state   = result.get("requestState").cloned();
            }
        }
    }
    Err(Error::MrtrRoundLimitExceeded { limit: self.mrtr_round_limit })
}
```

The three-way dispatch maps cleanly onto the existing `ClientHostRegistry` (`src/client/host/mod.rs:51`), whose fields are exactly `sampling`, `sampling_with_tools`, `elicitation`, `roots`. `classify_host_request` at `:114` already classifies by method; an `inputRequests`-shaped analogue is a small sibling function.

**Capability honesty (spec obligation 7 / conformance `capability-check`):** the client's `_meta.clientCapabilities` must accurately reflect which of the three handlers are registered, so the server can honor the MUST-NOT. Derive it from `ClientHostRegistry` rather than letting the caller hand-write it. Note `ClientCapabilities.elicitation` in v2 is `{ form?, url? }` — "form mode only (implicit)" is a documented example — so a registry with only a `HostElicitationHandler` should advertise `elicitation: {}` or `{form:{}}`.

### Anti-Patterns to Avoid

- **Forking the transport for v2.** CONTEXT.md forbids it and the code doesn't need it — the four session sites plus the router are the whole surface. A fork guarantees v1/v2 drift.
- **Putting `inputResponses`/`requestState` in `_meta`.** They are `params` fields. This will pass every in-house test and fail every conformance check.
- **Reusing the JSON-RPC id on the MRTR retry.** Spec MUST-violation and it re-creates the id-replay bug class HTTP-05 exists to close.
- **Server-side storage for continuation state.** D-01 forbids it and it defeats the whole point; any `HashMap<request_state_id, State>` is the failure mode.
- **Re-eliciting on an AEAD tag failure.** Fails `sep-2322-reject-tampered-state`. Only an *unknown key-id* may re-elicit.
- **Sending `inputRequests` for undeclared client capabilities.** Use `-32021` instead.
- **Trusting `_meta.clientInfo` as an identity anchor.** Per spec PR #3002 `clientInfo` is now optional and servers **MUST NOT** require it; it is self-reported and unverified. Bind the AAD principal to the OAuth subject / `AuthContext`, never to `clientInfo`.
- **Emitting `serverInfo` as a top-level result key.** See Pitfall 6.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Authenticated encryption for `requestState` | Encrypt-then-MAC from `sha2` + a hand-rolled stream cipher | `ring::aead` (`CHACHA20_POLY1305` / `AES_256_GCM`) | Nonce misuse, tag truncation, and non-constant-time comparison are all trivially introduced and catastrophic here — this token is explicitly attacker-controlled input. |
| Constant-time tag/principal comparison | `==` on byte slices | AEAD's built-in tag verification; `subtle::ConstantTimeEq` (already in `Cargo.lock`) for any auxiliary comparison | Timing oracles on an attacker-supplied token. |
| Nonce generation | Counter or timestamp | `getrandom` (already a normal dep) | A repeated nonce under the same key breaks AEAD confidentiality **and** integrity. |
| Base64 for the token + `Mcp-Name` sentinel | Custom encoder | `base64` 0.22 (already present) | Padding/URL-safe alphabet mistakes are a classic interop failure; the `=?base64?…?=` sentinel is a spec MUST with exact case-sensitive markers. |
| SSE framing for `subscriptions/listen` | Hand-written chunked writer | `axum::response::Sse` (already used by `handle_get_sse`) + explicit keep-alive comments | Correct SSE framing, backpressure, and `Content-Type` come free; only the lifecycle differs. |
| JSON Schema validation of `inputResponses` | Ad-hoc field checks | The existing `jsonschema` path behind `validation` | Already the project's validator; SEP-2106 posture is already correct. |
| Protocol version negotiation / era classification | New logic in this phase | Phase 112's `ProtocolContext` at ingress | Two era resolvers = research Pitfall 2 (dual negotiation collision), already paid for. |

**Key insight:** `requestState` is the one place in this SDK where a cryptographic mistake is directly exploitable by a remote, unauthenticated party — the spec says so explicitly ("servers **MUST** treat `requestState` as an attacker-controlled input"). Every primitive needed is already in the dependency tree; there is no defensible reason to hand-roll any of it.

---

## Common Pitfalls

### Pitfall 1: `stateless()` is a build-time config, so the era gate silently doesn't fire

**What goes wrong:** the phase is planned as "route v2 to the `stateless()` branch," which reads as "use the stateless config." But a dual-version server is built with `Default::default()` (`session_id_generator: Some(uuid)`), and every one of the four session sites keys off that config field, not off the era. v2 requests then get session ids minted and echoed.
**Why it happens:** CONTEXT.md's phrasing ("era-gated onto pmcp's existing `stateless()` branch") is naturally read as config selection.
**How to avoid:** single `sessions_active(state, era)` predicate at all four sites (Pattern 1). Test with a server built via `Default::default()` — **not** via `stateless()` — and assert no `Mcp-Session-Id` response header on a v2 request.
**Warning signs:** the v2 tests only ever construct `StreamableHttpServerConfig::stateless()`.

### Pitfall 2: `requestState` binds principal + TTL but not the originating request

**What goes wrong:** a valid token minted for `tools/call{name:"read_file", path:"/safe"}` is replayed by the same principal within the TTL onto `tools/call{name:"read_file", path:"/etc/shadow"}`. Principal-binding and TTL both pass. The continuation state (which may carry an already-authorized resource handle or partial args) is resumed against different arguments.
**Why it happens:** CONTEXT.md D-02/D-05 name only two bindings; the third is in the spec's replay-prevention list and is easy to miss.
**How to avoid:** include `method` and a digest of the salient params in the AAD (Pattern 3). Salient = the params that determined the authorization decision — for `tools/call` that is at minimum `name`, and arguably a canonicalized digest of `arguments` minus `inputResponses`/`requestState`/`_meta`.
**Warning signs:** the AAD is just the principal id; no test replays a token onto a different tool.

### Pitfall 3: `input_required` leaked onto a method the spec forbids

**What goes wrong:** a generic "if the handler needs input, envelope it" path lets `input_required` escape on `tools/list`, `completion/complete`, or `server/discover`. Conformance check `sep-2322-not-on-unsupported-requests` fails.
**Why it happens:** `inject_v2_result_envelope` is deliberately generic (Phase 112 verified it as "era-gated only, no method-specific logic").
**How to avoid:** gate the `InputRequired` **disposition selection** (not the injection helper) to the three MRTR-eligible dispatch arms only. Add an exhaustive-variant tripwire like Phase 112's `all_meta_bearing_client_requests_are_extracted`.

### Pitfall 4: `ElicitRequestParams` cannot deserialize a spec-shaped form elicitation

**What goes wrong:** `src/types/elicitation.rs:26` declares `#[serde(tag = "mode")] pub enum ElicitRequestParams { Form{..}, Url{..} }`. The v2 schema declares `ElicitRequestFormParams.mode?: "form"` — **optional** — and `ElicitRequestURLParams.mode: "url"` — required. The conformance suite's own A1 example omits `mode` entirely:

```json
"user_name": { "method": "elicitation/create",
               "params": { "message": "What is your name?", "requestedSchema": {...} } }
```

A `#[serde(tag)]` enum rejects this with "missing field `mode`". This breaks the **client** side of CLNT-02 against any conforming server.
**Why it happens:** the type was written for 2025-11-25 where `mode` was mandatory; the v2 schema made form-mode implicit.
**How to avoid:** custom `Deserialize` (or an untagged shim) that defaults a missing `mode` to `Form`. **Keep serialization emitting `"mode":"form"`** — that is still valid and preserves v1 byte-compatibility. This is a serde-only change, no public type change, so it stays semver-additive (D-10 preserved).
**Warning signs:** round-trip tests only ever feed pmcp-serialized JSON back into pmcp.

### Pitfall 5: v2 error surface is missing three codes and two HTTP status mappings

**What goes wrong:** `src/types/protocol/error_codes.rs` currently defines 12 constants and **none** of `-32020` / `-32021` / `-32022` — grep for `32020|32021|HeaderMismatch|MissingRequiredClientCapability|UnsupportedProtocolVersion` across `src/` returns **zero** matches. Meanwhile the conformance suite requires:

| Situation | Code | HTTP status |
|-----------|------|-------------|
| header/body mismatch or missing required header | `-32020` HeaderMismatch | **400** |
| server needs an undeclared client capability | `-32021` MissingRequiredClientCapability (`error.data.requiredCapabilities` = ClientCapabilities **object**) | **400** |
| unsupported protocol version (with `supported` list) | `-32022` UnsupportedProtocolVersion | **400** |
| unknown method on the v2 path | `-32601` | **404** (pmcp currently returns **200**) |
| `_meta` missing or missing `protocolVersion`/`clientCapabilities` | `-32602` | **400** |

The 404 mapping is called out explicitly in the spec ("If the server does not implement the requested RPC method, it **MUST** respond with `404 Not Found` and a JSON-RPC error with code `-32601`") *and* in `stateless.ts` ("Removed legacy endpoints (`initialize`, `ping`, `logging/setLevel`, etc.) or generic unknown methods must cleanly yield an HTTP status code `404 Not Found` alongside a JSON-RPC `-32601`"). Phase 112's verification report documents the current behavior as "`-32601@200`".
**Why it happens:** Phase 112's VERS-06 correctly built the *table* but deliberately deferred v2 *values* pending the final schema. This phase is where the values land.
**How to avoid:** add the three constants to `error_codes.rs` with the same locking-test discipline the frozen `-32002` has, and add an era-gated HTTP status mapper. **Note for the `-32002` open item:** the draft schema resolves it — "Codes defined by earlier protocol versions remain reserved and are never reused: `-32002` (**resource not found**, 2025-11-25 and earlier; replaced by `-32602`)". The rename targets *resource-not-found*, not task-pending; pmcp's proprietary `V1_TASK_PENDING` squat on `-32002` is unaffected and must stay frozen.

### Pitfall 6: `serverInfo` is emitted in the wrong place (carried defect from Phase 112)

**What goes wrong:** `inject_v2_result_envelope` (`src/server/core.rs:1141`) does `obj.entry("serverInfo").or_insert_with(...)` — a **top-level key on the result object**. The schema puts it in result `_meta`:

```typescript
// schema/draft/schema.ts:143-158
export interface ResultMetaObject extends MetaObject {
  "io.modelcontextprotocol/serverInfo"?: Implementation;
}
```

and the conformance suite says `server/discover` "**SHOULD** identify itself via `_meta['io.modelcontextprotocol/serverInfo']` (spec PR #3002)".
**Why it happens:** the key name matches; the nesting does not. Phase 112 verified *presence*, not *placement*, and had no conformance harness to catch it.
**How to avoid:** move to `result._meta["io.modelcontextprotocol/serverInfo"]`. Phase 113 owns the v2 HTTP response path and is the first phase graded by conformance, so it is the right place to fix. Cheap now; a wire-visible break later. **This also means the `result._meta` object must be created when absent** — currently there is no `_meta` construction in the envelope path at all.

### Pitfall 7: the v2 client sends none of the required headers

**What goes wrong:** `grep MCP_METHOD|MCP_NAME src/shared/streamable_http.rs` → **zero matches**. The client transport sets `Mcp-Session-Id` (`:576`) and reads it back (`:676-678`), but never emits `Mcp-Method`/`Mcp-Name`. Against pmcp's own Phase-112 header gate (strict reject, D-05), every v2 request from a pmcp client is rejected. There is also **no client-side `resultType` handling anywhere** (`grep resultType src/client/ src/shared/` → zero matches).
**How to avoid:** CLNT-01 must (a) emit `Mcp-Method` on every v2 POST, (b) emit `Mcp-Name` from `params.name` (tools/call, prompts/get) or `params.uri` (resources/read) — mirroring the method-aware extraction Phase 112 already built server-side at `streamable_http_server.rs:537`, (c) apply the base64 sentinel when the value is not header-safe, (d) send `Accept: application/json, text/event-stream` (the `ACCEPT_STREAMABLE` constant already exists), (e) **not** send `Mcp-Session-Id` on v2.
**Warning signs:** the only v2 tests are server-side and drive raw `reqwest` (which is exactly what `tests/v2_required_headers.rs` does today).

### Pitfall 8: MRTR loop re-invoked forever by an under-supplying server

**What goes wrong:** spec obligation 9 tells servers to re-request rather than error when the client under-supplies. A buggy server that always considers the response insufficient produces an infinite gather→resend loop — re-prompting a human, or spinning an agent (D-07's second client shape).
**How to avoid:** D-09's bounded loop, already locked. Two refinements worth planning: (a) count rounds per *logical operation*, not per handler invocation, so a multi-key `inputRequests` counts once; (b) surface the round count to the handler so an agent can decide to give up early.

### Pitfall 9: false-green from feature unification (repeat offender in this repo)

**What goes wrong:** `cargo test --all-features` unifies the dev-dependency `pmcp` `full` feature with the crate under test, masking feature-flag gaps. This burned Phase 109 (the `pmcp/http` gap) and is called out in `.planning/research/PITFALLS.md` #11. Adding `ring` behind `streamable-http` creates exactly this hazard: `request_state.rs` will compile under `full` but must also compile (or be correctly `cfg`'d out) without `streamable-http`.
**How to avoid:** verify with a **dev-dependency-free** `cargo build --all-features` using the absolute rustup cargo, plus an explicit `cargo build --lib --no-default-features` and `cargo build --lib --target wasm32-unknown-unknown`. The wasm build matters because `src/server/core.rs` is `#[cfg(not(target_arch = "wasm32"))]` — MRTR server code must not leak into `WasmServerCore`.

### Pitfall 10: accidental 3.0 via the request-param additions

**What goes wrong:** adding `input_responses` / `request_state` to `CallToolRequest`, `GetPromptRequest`, `ReadResourceRequest` touches three public structs. Done wrong (non-`Option`, no `#[serde(default)]`, no `skip_serializing_if`) it is a breaking change and a wire change for v1.
**How to avoid:** copy the existing precedent verbatim — `CallToolRequest.task` at `src/types/tools.rs:464-470` is `Option<Value>` with `#[serde(skip_serializing_if = "Option::is_none", default)]`. Run `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` (Phase 112 got `223 pass, 30 skip, no semver update required` — hold that line).

---

## Code Examples

### Verify a `requestState` token (server, `ring`)

```rust
// Source: ring 0.17 aead API — https://docs.rs/ring/0.17.14/ring/aead/
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};

fn verify(
    token_b64: &str,
    keys: &KeyRing,             // key_id -> LessSafeKey (current + previous)
    principal: &str,
    method: &str,
    param_digest: &[u8; 32],
) -> Verdict {
    let raw = match base64_url_decode(token_b64) { Ok(r) => r, Err(_) => return Verdict::Tampered };
    let (key_id, rest) = match split_key_id(&raw) { Some(p) => p, None => return Verdict::Tampered };

    // UNKNOWN key-id => this instance did not mint it (D-04 degraded path) => re-elicit.
    let Some(key) = keys.get(key_id) else { return Verdict::UnknownKey };

    if rest.len() < NONCE_LEN { return Verdict::Tampered; }
    let (nonce_bytes, sealed) = rest.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce_bytes).unwrap();

    let mut aad_buf = Vec::new();
    aad_buf.extend_from_slice(principal.as_bytes()); aad_buf.push(0);
    aad_buf.extend_from_slice(method.as_bytes());    aad_buf.push(0);
    aad_buf.extend_from_slice(param_digest);

    let mut buf = sealed.to_vec();
    // AEAD tag verification is constant-time and covers the AAD, so a principal
    // or originating-request mismatch fails HERE, not in a later `==`.
    match key.open_in_place(nonce, Aad::from(&aad_buf), &mut buf) {
        Ok(plain) => match serde_json::from_slice::<Continuation>(plain) {
            Ok(c) if c.exp > now_unix() => Verdict::Ok(c),
            Ok(_)  => Verdict::Expired,
            Err(_) => Verdict::Tampered,
        },
        Err(_) => Verdict::Tampered,   // known key + bad tag => MUST be a JSON-RPC error
    }
}
```

### Emit an `InputRequiredResult` (server, wire shape)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "resultType": "input_required",
    "inputRequests": {
      "github_login": {
        "method": "elicitation/create",
        "params": {
          "mode": "form",
          "message": "Please provide your GitHub username",
          "requestedSchema": {
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
          }
        }
      }
    },
    "requestState": "<opaque AEAD blob>",
    "_meta": {
      "io.modelcontextprotocol/serverInfo": { "name": "pmcp", "version": "2.18.0" }
    }
  }
}
```
[CITED: modelcontextprotocol.io/specification/draft/basic/patterns/mrtr — the spec's own example literally uses `"requestState": "AEAD-protected blob"`, corroborating D-02]

### Client retry (wire shape — note the **new id** and the **params-level** fields)

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "create_repo",
    "arguments": { "visibility": "private" },
    "inputResponses": {
      "github_login": { "action": "accept", "content": { "name": "octocat" } }
    },
    "requestState": "<echoed verbatim>",
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": { "elicitation": {} }
    }
  }
}
```
Headers: `MCP-Protocol-Version: 2026-07-28`, `Mcp-Method: tools/call`, `Mcp-Name: create_repo`, `Accept: application/json, text/event-stream`.
[VERIFIED: conformance `input-required-result.ts` `sendRpc(serverUrl, 'tools/call', { name, arguments, inputResponses, requestState })`]

### `subscriptions/listen` stream (opt-in path)

```
POST /mcp  {"jsonrpc":"2.0","id":7,"method":"subscriptions/listen",
            "params":{"_meta":{...},"notifications":{"toolsListChanged":true}}}

200 OK  Content-Type: text/event-stream   X-Accel-Buffering: no

data: {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged",
       "params":{"_meta":{"io.modelcontextprotocol/subscriptionId":7},
                 "notifications":{"toolsListChanged":true}}}

:                                    <- keep-alive comment during quiet periods

data: {"jsonrpc":"2.0","method":"notifications/tools/list_changed",
       "params":{"_meta":{"io.modelcontextprotocol/subscriptionId":7}}}

data: {"jsonrpc":"2.0","id":7,
       "result":{"resultType":"complete",
                 "_meta":{"io.modelcontextprotocol/subscriptionId":7}}}   <- graceful close
```
[CITED: modelcontextprotocol.io/specification/draft/basic/patterns/subscriptions]

---

## State of the Art

| Old Approach (2025-11-25) | Current Approach (2026-07-28) | Impact on this phase |
|---------------------------|-------------------------------|----------------------|
| `initialize` / `notifications/initialized` handshake | Per-request `_meta` + `server/discover` | Phase 112 done; 113 turns off the session machinery that rode on it |
| `Mcp-Session-Id` header, HTTP DELETE to terminate | Removed; ignore inbound, never mint or echo; DELETE → 405 | HTTP-01 |
| HTTP GET → standalone SSE stream for server-initiated messages | Removed; GET → 405. Long-lived streams come from `subscriptions/listen` | HTTP-04 |
| `Last-Event-ID` resumable SSE | "Resumable SSE streams via `Last-Event-ID` are not supported"; ignore the header | HTTP-05 |
| Server sends JSON-RPC **requests** on SSE (sampling/elicitation/roots) | "Servers **MUST NOT** send independent JSON-RPC requests on this stream" — replaced by MRTR `inputRequests`. Spec calls this "a breaking change" | CLNT-02: the Phase-106 host surface is *replaced*, not merely folded, on v2 |
| `resources/subscribe` / `resources/unsubscribe` RPCs | Removed from the schema; `SubscriptionFilter.resourceSubscriptions` replaces them | HTTP-04 |
| `-32002` = resource not found | Reserved and never reused; replaced by `-32602` | Resolves the research pack's Open Verification Item — pmcp's `V1_TASK_PENDING` squat is unaffected |
| No transport-level error taxonomy | `-32020` HeaderMismatch, `-32021` MissingRequiredClientCapability, `-32022` UnsupportedProtocolVersion (all HTTP 400); unknown method → HTTP 404 | Pitfall 5 |
| `clientInfo` required in `_meta` | **Optional** since spec PR #3002 (2026-07-16); servers **MUST NOT** require it | Do not gate v2 acceptance on `clientInfo` presence |
| `serverInfo` (Phase 112, top-level result key) | `result._meta["io.modelcontextprotocol/serverInfo"]`, SHOULD be on every response | Pitfall 6 |
| Elicitation `mode` mandatory | `mode?: "form"` optional (implicit); `mode: "url"` required | Pitfall 4 |

**Deprecated/outdated:**
- HTTP+SSE (2024-11-05) — deprecated since 2025-03-26, eligible for removal. Not this phase's concern.
- Roots / Sampling / Logging capabilities — **deprecated but advisory only**, must keep working ≥12 months (research Pitfall 12). On v2 they are reachable *only* through MRTR `inputRequests`.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / rustc | everything | ✓ | workspace toolchain (run `rustup update stable` per CLAUDE.md pre-flight) | — |
| `cargo-nextest` | `make test` | ✓ (referenced by Makefile:212) | — | `cargo test` |
| `ring` 0.17.14 | AEAD `requestState` | ✓ **already in `Cargo.lock`** | 0.17.14 | `chacha20poly1305` 0.11.0 (new crate, pure Rust) |
| `gh` CLI | fetching conformance suite source during planning | ✓ | — | raw.githubusercontent via curl |
| Node.js LTS 22 + `@modelcontextprotocol/conformance` | running the official suite | **not verified in this environment** | — | Phase 118 owns CI integration; this phase should encode the checks as **Rust tests**, not depend on Node |
| `cargo semver-checks` | additive-milestone gate | ✓ (used successfully in Phase 112) | — | `cargo public-api` |
| `pmat` | CI cognitive-complexity gate | CI-only per CLAUDE.md (D-07 of Phase 75) | 3.15.0 pinned | — |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** the official Node conformance suite — deliberately deferred to Phase 118 (CONF-01). This phase should mirror the specific `sep-2322-*` and `sep-2575-*` assertions as native Rust integration tests so the phase is self-verifying without a Node toolchain.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]`, `proptest` for property tests, `cargo-fuzz` for fuzz targets |
| Config file | none (Cargo-native); orchestration in `Makefile` (`test`, `test-unit`, `test-doc`, `test-property`, `test-fuzz`, `test-integration`, `test-all`, `quality-gate`) |
| Quick run command | `cargo test --lib --features full` (Phase 112 baseline: 1229 passed) |
| Targeted integration | `cargo test --test v2_required_headers --features full` (25 tests — the live-HTTP v2 harness this phase extends) |
| Full suite command | `make quality-gate` (fmt-check → lint → build → test-all → package gate → audit → unused-deps → check-todos → check-unwraps → validate-always → purity-check → comply) |

The existing `tests/v2_required_headers.rs` (35.8K, 25 tests, real socket + real `StreamableHttpServer` + real `reqwest`) is the correct home/pattern for this phase's HTTP assertions. `tests/server_subscriptions.rs` is the existing v1 subscribe test that must stay green.

### Phase Requirements → Test Map

| Req | Behavior | Type | Automated Command | File Exists? |
|-----|----------|------|-------------------|-------------|
| HTTP-01 | v2 POST on a **`Default::default()`** (stateful-config) server emits no `Mcp-Session-Id` and requires none | integration | `cargo test --test v2_stateless_http --features full -- no_session_id_on_v2` | ❌ Wave 0 |
| HTTP-01 | v1 request on the same server still mints + echoes a session id (byte-identity) | integration | `... -- v1_session_unchanged` | ❌ Wave 0 |
| HTTP-01 | GET and DELETE with `MCP-Protocol-Version: 2026-07-28` → 405 | integration | `... -- v2_get_delete_405` | ❌ Wave 0 |
| HTTP-01 | unknown method on v2 → HTTP 404 + `-32601` | integration | `... -- v2_unknown_method_404` | ❌ Wave 0 |
| HTTP-02 | `input_required` result carries `resultType`, `inputRequests`, `requestState` | integration | `cargo test --test v2_mrtr --features full -- emits_input_required` | ❌ Wave 0 |
| HTTP-02 | tampered `requestState` (known key-id) → JSON-RPC error, **not** a re-prompt | integration | `... -- tampered_state_errors` | ❌ Wave 0 |
| HTTP-02 | token from a different principal → error | integration | `... -- principal_mismatch_errors` | ❌ Wave 0 |
| HTTP-02 | token replayed onto a different method/params → error | integration | `... -- originating_request_mismatch_errors` | ❌ Wave 0 |
| HTTP-02 | expired token → error | unit | `cargo test --lib --features full -- request_state::expiry` | ❌ Wave 0 |
| HTTP-02 | unknown key-id → re-elicit (D-04 degraded path) | unit | `... -- request_state::unknown_key_reelicits` | ❌ Wave 0 |
| HTTP-02 | round-trip property: mint→verify is identity for arbitrary state/principal/TTL | property | `PROPTEST_CASES=1000 cargo test --features full -- --ignored property_request_state` | ❌ Wave 0 |
| HTTP-02 | fuzz: arbitrary bytes as `requestState` never panic, always reject | fuzz | `cargo fuzz run fuzz_request_state` | ❌ Wave 0 |
| HTTP-03 | 2-round and 3-round MRTR complete with evolving `requestState` | integration | `cargo test --test v2_mrtr --features full -- multi_round` | ❌ Wave 0 |
| HTTP-03 | multiple `inputRequests` of different kinds in one result | integration | `... -- multiple_inputs` | ❌ Wave 0 |
| HTTP-03 | MRTR on `prompts/get` and `resources/read`, not just `tools/call` | integration | `... -- mrtr_non_tool` | ❌ Wave 0 |
| HTTP-03 | `input_required` never emitted on a non-MRTR method (exhaustive tripwire) | unit | `cargo test --lib --features full -- mrtr_eligible_methods_only` | ❌ Wave 0 |
| HTTP-04 | advertised subscription capability ⇒ `subscriptions/listen` served (tripwire) | integration | `cargo test --test v2_subscriptions --features full -- advertise_implies_serve` | ❌ Wave 0 |
| HTTP-04 | with capabilities off, `subscriptions/listen` → 404 + `-32601` and discover advertises none | integration | `... -- absent_capability_is_conformant` | ❌ Wave 0 |
| HTTP-04 | ack is the first frame; every frame carries `subscriptionId`; no unrequested types | integration | `... -- listen_stream_protocol` | ❌ Wave 0 |
| HTTP-05 | v2 request with `Last-Event-ID` is served normally, header ignored, no replay | integration | `cargo test --test v2_stateless_http --features full -- last_event_id_ignored` | ❌ Wave 0 |
| HTTP-05 | **id-replay regression:** a cached `result` re-served to a second caller carries the second caller's live id | integration | `... -- response_id_always_from_live_request` | ❌ Wave 0 |
| CLNT-01 | v2 client POST carries `Mcp-Method`, correct `Mcp-Name` (name **and** uri forms), `Accept`, no session id | integration | `cargo test --test v2_client --features full -- emits_required_headers` | ❌ Wave 0 |
| CLNT-01 | non-ASCII tool name / resource URI → base64 sentinel `=?base64?…?=` | unit | `cargo test --lib --features full -- header_value_encoding` | ❌ Wave 0 |
| CLNT-02 | client fulfills elicitation / sampling / roots inputRequests via the host registry | integration | `cargo test --test v2_client --features full -- mrtr_three_kinds` | ❌ Wave 0 |
| CLNT-02 | no handler / decline ⇒ returns the `input_required` result, does **not** resend | integration | `... -- no_handler_returns_result` | ❌ Wave 0 |
| CLNT-02 | round limit exceeded ⇒ `MrtrRoundLimitExceeded` | integration | `... -- round_limit` | ❌ Wave 0 |
| CLNT-02 | retry uses a **different** JSON-RPC id | integration | `... -- retry_uses_new_id` | ❌ Wave 0 |
| CLNT-02 | form-mode elicitation with **no** `mode` field deserializes | unit | `cargo test --lib --features full -- elicit_params_mode_optional` | ❌ Wave 0 |
| all | additive semver | gate | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | ✓ (used in 112) |
| all | no feature-unification false-green | gate | dev-dep-free `cargo build --all-features` + `cargo build --lib --target wasm32-unknown-unknown` | ✓ (Phase 109 pattern) |

### Sampling Rate

- **Per task commit:** `cargo test --lib --features full` + the one integration file the task touches.
- **Per wave merge:** `make test-unit && make test-doc && make test-integration`.
- **Phase gate:** `make quality-gate` green (this is also the pre-commit and pre-push requirement per CLAUDE.md), plus `cargo semver-checks`, plus the wasm build.

### Wave 0 Gaps

- [ ] `tests/v2_stateless_http.rs` — HTTP-01, HTTP-05
- [ ] `tests/v2_mrtr.rs` — HTTP-02, HTTP-03
- [ ] `tests/v2_subscriptions.rs` — HTTP-04
- [ ] `tests/v2_client.rs` — CLNT-01, CLNT-02
- [ ] `fuzz/fuzz_targets/fuzz_request_state.rs` — CLAUDE.md ALWAYS requirement
- [ ] property tests for `requestState` mint/verify — CLAUDE.md ALWAYS requirement
- [ ] `examples/` — a runnable v2 stateless server demonstrating MRTR (CLAUDE.md ALWAYS requirement: `cargo run --example`)
- [ ] shared test helper for constructing a valid v2 request (`_meta` + the three headers) — every new test file needs it; the pattern exists inside `tests/v2_required_headers.rs` and should be lifted to `tests/common/`

No framework install is needed.

---

## Security Domain

`security_enforcement` is not set to `false` in `.planning/config.json`, so this section applies.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Existing `AuthProvider` / `AuthContext`. **New:** the MRTR principal must come from the authenticated subject, never from self-reported `_meta.clientInfo` (now optional per PR #3002) and never from a session id (absent on v2). |
| V3 Session Management | yes (inverted) | v2 has **no** sessions. The control is proving that no session-derived identity survives into the v2 path — the same failure class as `pmcp_run_proxy_discovery_cache_id_bug`. |
| V4 Access Control | yes | `requestState` may carry authorization-relevant continuation state. Spec: bind to principal **and** originating request; reject on mismatch. |
| V5 Input Validation | yes | `requestState` is explicitly attacker-controlled; `inputResponses` is client-supplied and must be schema-validated (existing `jsonschema` path). Header/body cross-check already exists (Phase 112 D-06) and now needs the `-32020` code. |
| V6 Cryptography | yes | `ring::aead` — **never hand-roll**. 32-byte key, random 12-byte nonce per token, AAD carries the bindings, no custom comparison. |
| V7 Error Handling & Logging | yes | Reject messages must not leak whether failure was tamper / expiry / principal-mismatch beyond what the spec requires; log the discriminated reason server-side only. |
| V9 Communications | yes | `X-Accel-Buffering: no` on SSE; DNS-rebinding `Origin` validation stays (spec MUST) — preserve the documented `AllowedOrigins::any()` proxy exception for `stateless()`. |
| V13 API & Web Service | yes | 400/404/405 status mapping per spec; `-32020`/`-32021`/`-32022`. |

### Known Threat Patterns for stateless MCP over HTTP

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| `requestState` tampering to escalate authorization | Tampering / Elevation of Privilege | AEAD with AAD = principal ‖ method ‖ param-digest; reject on tag failure (spec MUST) |
| Cross-principal `requestState` replay | Spoofing | Principal in AAD (D-02) |
| Cross-request `requestState` replay (same principal, different tool/args) | Elevation of Privilege | **Originating-request identifier in AAD (spec 5c — missing from CONTEXT.md)** |
| Long-window replay | Repudiation / Tampering | Short TTL baked into the ciphertext (D-05); note the spec's warning that TTL bounds but does not prevent reuse — single-use requires server-side enforcement and is explicitly **out of scope** for a self-contained token |
| Identity collapse when `session_id` disappears | Spoofing / Information Disclosure | Never fall back to session id on v2; fail closed with `-32021`/auth error. (This is Phase 114's TASK-05 in full, but the *pattern* must be established here.) |
| JSON-RPC id replay across callers (discovery-cache bug class) | Spoofing / Tampering | Response id always re-derived from the live request; cache `result`, never the envelope (HTTP-05) |
| Client-supplied `clientCapabilities` over-claiming | Spoofing | Server may send `inputRequests` only for declared capabilities; a *lying* client only harms itself. Do **not** derive authorization from `clientCapabilities`. |
| Header/body desync smuggling (WAF sees one method, server executes another) | Tampering | Phase 112 D-06 cross-check, now with the `-32020` code and 400 status |
| SSE stream exhaustion via many `subscriptions/listen` connections | Denial of Service | Cap concurrent listen streams per principal; this is a real cost of the opt-in stream and a concrete argument for keeping it off by default |
| Key exposure via `PMCP_REQUEST_STATE_KEY` in process listings / logs | Information Disclosure | Read once at startup, zeroize the source string (`zeroize` 1.8.2 is already in `Cargo.lock`), never log the key or the key-id-to-key mapping |

---

## Project Constraints (from CLAUDE.md)

| Directive | Effect on this phase |
|-----------|----------------------|
| **Zero tolerance for defects**; `make quality-gate` before **every** commit and before any push/PR | Every plan task's verification step must be a real command, not a claim |
| Cognitive complexity **≤ 25** per function (PMAT gates PRs in CI) | The MRTR client loop and the `requestState` verifier are both natural complexity hotspots — decompose deliberately (verdict enum + small verify steps) rather than one large `match` |
| **Zero SATD** — no TODO/FIXME/XXX/HACK/PLACEHOLDER | The `#[allow(dead_code)]` on `ResponseDisposition::InputRequired` (`core.rs:1119`) exists specifically for this phase and **must be removed** when wired |
| **ALWAYS requirements for new features:** fuzz + property + unit + a runnable `cargo run --example` | Reflected in the Wave 0 gaps above; `requestState` is the obvious fuzz/property target |
| 80%+ test coverage; comprehensive rustdoc with working examples | Doctests run in `make test-doc` |
| Contract-first: update `../provable-contracts/contracts/<crate>/`, `pmat comply check` before and after | `make quality-gate` runs `comply`; plan a contract update task |
| Semver: new features = minor. Milestone must stay **2.x** | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` |
| Downstream crates pinning `pmcp` must be bumped together | `pmcp-agent`, `pmcp-team-servers`, `mcp-tester`, `cargo-pmcp`, toolkit crates |
| Do **not** weaken or remove the CI quality gate | — |
| Prefer `justfile` for project scripts (global user instruction) | A `justfile` exists; the authoritative gate here is the `Makefile`'s `quality-gate` — do not fragment it |

**Project skills:** `.claude/skills/spike-findings-rust-mcp-sdk/` and `.agents/skills/spike-findings-rust-mcp-sdk/` — SEP-2640 Skills + the schema-server toolkit lift. Not directly relevant to Phase 113's protocol work; its "dual-surface invariant" discipline (a feature must work identically through both the library API and the config-driven surface) is worth remembering if MRTR ever needs to be expressible in toolkit config.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `draft` schema @ 2026-07-16 is what publishes as `2026-07-28`; no further wire changes land in the remaining 4 days | Final-Spec Checkpoint, MRTR Wire Contract | MEDIUM — a late rename to `inputResponses`/`requestState`/`resultType` would require a wire-adapter edit. Mitigated by D-10's adapter and by the Wave-0 re-diff task. |
| A2 | `ring` 0.17's `aead` module is reachable from pmcp once declared as a direct dep (i.e. `rustls` 0.23 with `features=["ring"]` genuinely resolves the same `ring` 0.17.x, not a distinct major) | Standard Stack | LOW — `Cargo.lock` shows exactly one `ring` entry (0.17.14) referenced by `rustls` and `rustls-webpki`. Verify by building after the `Cargo.toml` edit. |
| A3 | `ring` is not required on `wasm32-unknown-unknown` because MRTR server code lives behind `#[cfg(not(target_arch="wasm32"))]` (`src/server/core.rs`) | Standard Stack, Pitfall 9 | MEDIUM — if MRTR must work on `WasmServerCore`, switch to `chacha20poly1305`. **Needs owner confirmation**: CONTEXT.md's discretion list covers stdio but is silent on wasm. |
| A4 | pmcp's *client* never needs to decrypt `requestState` (it is opaque and echoed verbatim), so no crypto ships in the wasm client | Architecture | LOW — this is a spec MUST ("Clients **MUST NOT** inspect, parse, modify"). |
| A5 | Expired-but-authentic tokens should return an error rather than re-elicit, despite D-05's "re-elicits cleanly" | Pattern 3 | LOW-MEDIUM — no conformance check covers it; either is arguably conformant. Flagged for owner confirmation. |
| A6 | Fixing the `serverInfo` placement (top-level → `_meta`) inside Phase 113 is acceptable scope, since 113 owns the v2 response path | Pitfall 6 | LOW — it is a small, wire-visible correctness fix; deferring it means shipping a known non-conformant response shape. Needs planner acknowledgement. |
| A7 | Shipping `subscriptions/listen` behind an opt-in widens the phase beyond CONTEXT.md's Deferred Ideas note | D-12 Resolution | MEDIUM — CONTEXT.md deferred the stream unless "hard-mandated"; the mandate is conditional. **Owner decision required**: implement opt-in stream now, or ship capabilities-off-only and defer the stream to a later phase (which would make `listChanged` unavailable on v2). |
| A8 | The `Mcp-Param-{Name}` / `x-mcp-header` feature (SEP-2243) is out of scope for this phase | Pitfall 7 | MEDIUM — the spec says clients **MUST** support `x-mcp-header` mirroring, and the header-mismatch validation table covers `Mcp-Param-*`. Neither VERS-05 nor HTTP-01..05 nor CLNT-01 mentions it. **This looks like a genuine milestone-level requirements gap**, not a Phase-113 gap. Flag to the roadmap. |
| A9 | `notifications/cancelled` is not needed on the HTTP v2 path (stream close is the cancellation signal) | Architecture | LOW — spec states this explicitly for Streamable HTTP; stdio still uses the notification. |

---

## Open Questions (RESOLVED)

> **Status: all 6 resolved before planning was finalized (2026-07-24).** Questions 1, 2 and
> 6 were answered by owner decisions D-13/D-14/D-15 in `113-CONTEXT.md`; questions 3, 4 and
> 5 were adopted as-recommended and are cited to the specific plan and task that implements
> them. No question is left open for the executor to decide.

1. **Does MRTR need to work on the wasm server (`WasmServerCore`)?**
   - What we know: `src/server/core.rs` (where `ResponseDisposition`/`inject_v2_result_envelope` live) is `#[cfg(not(target_arch="wasm32"))]`; the wasm server is the separate minimal `WasmServerCore`. `ring` is native-only in practice.
   - What's unclear: whether Cloudflare-Workers-style wasm pmcp servers are expected to serve v2 MRTR.
   - Recommendation: assume **no** for this phase (HTTP is the mandated surface per CONTEXT.md). If yes, swap `ring` → `chacha20poly1305` and site the token module outside the native-only tree. Confirm with owner before Wave 1.
   - **RESOLVED: no** — locked by **D-14** (`113-CONTEXT.md`): MRTR `requestState` AEAD is
     native-only this phase using `ring`; `WasmServerCore` gets no MRTR in Phase 113.
     Adopted by **plan 03 Task 1**, which gates `src/server/request_state.rs` on
     `#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]` and verifies
     `cargo build --lib --target wasm32-unknown-unknown`. The `chacha20poly1305` swap is
     deferred until wasm MRTR is actually needed.

2. **Opt-in `subscriptions/listen` stream: in or out of Phase 113?** (see A7)
   - What we know: conditional mandate; capabilities-off is conformant; there is no conformant polling alternative.
   - Recommendation: implement the capability-gating + the tripwire test **in this phase** (cheap, and it makes HTTP-04 honestly satisfiable), and treat the SSE stream implementation as a separately-plannable slice within the phase that can be descoped to a follow-on if the wave budget is tight. Reword HTTP-04 either way.
   - **RESOLVED: in** — locked by **D-13** (`113-CONTEXT.md`), which also resolves D-12: the
     opt-in, capability-gated `subscriptions/listen` SSE stream ships in Phase 113. Adopted
     by **plan 10** — Task 1 (subscription types, capability gate, advertise-implies-serve
     tripwire), Task 2 (the long-lived SSE stream), Task 3 (live-SSE acceptance). D-11's
     polling-over-Tasks stance remains the recommended enterprise default. HTTP-04's roadmap
     wording stands; the opt-in nuance is reconciled by **plan 12 Task 3**.

3. **Should `-32021 MissingRequiredClientCapability` be emitted by Phase 113 or Phase 114?**
   - What we know: the conformance `stateless` scenario tests it via a `test_missing_capability` tool; it is directly entangled with MRTR obligation 7.
   - Recommendation: **113**, alongside `-32020`/`-32022`, because MRTR is the feature that needs it. Keep the constants in the Phase-112 centralized table with locking tests.
   - **RESOLVED: 113**, as recommended. Adopted by **plan 01 Task 3**, which adds
     `MISSING_REQUIRED_CLIENT_CAPABILITY: i32 = -32021` to
     `src/types/protocol/error_codes.rs` alongside `-32020`/`-32022` with locking tests;
     **plan 09 Task 2** emits it when a server signals for an undeclared capability; and
     **plan 11 Task 1**'s `input_required_result_capability_check` asserts HTTP 400 with an
     OBJECT-shaped `data.requiredCapabilities` (threat T-113-32).

4. **`x-mcp-header` / `Mcp-Param-{Name}` (SEP-2243) — which phase?** (see A8)
   - What we know: clients **MUST** support it; servers **MAY** use it; the header-mismatch validator must handle it.
   - Recommendation: raise as a milestone requirements gap. It is closest to CLNT-01's header work but is not in any current requirement. Do not silently absorb it into 113.
   - **RESOLVED: not absorbed into 113**, as recommended. Raised as a milestone-level
     requirements gap and adopted by **plan 12 Task 3**, whose `<automated>` verify greps
     for `x-mcp-header` in BOTH `.planning/REQUIREMENTS.md` and `.planning/ROADMAP.md`, so
     the gap cannot ship undocumented (threat T-113-42). No Phase-113 plan implements
     `Mcp-Param-{Name}` mirroring.

5. **Is `resultType: "task"` reachable from an MRTR retry?**
   - What we know: `ResultType` is an open union (`"complete" | "input_required" | string`); Phase 114 owns `"task"`.
   - Recommendation: the client MRTR loop must treat any non-`input_required` `resultType` as terminal (return to caller) so Phase 114 composes without touching the loop.
   - **RESOLVED: as recommended.** Adopted by **plan 07 Task 3** (the bounded gather→resend
     loop): any `resultType` other than `input_required` is terminal and is returned to the
     caller unmodified, so Phase 114's `"task"` result composes without a loop change.

6. **Key rotation for `PMCP_REQUEST_STATE_KEY`.**
   - What we know: D-03 locks a single env var; the key-id design (Pattern 3) naturally supports a set.
   - Recommendation: accept an optional second/previous key (e.g. `PMCP_REQUEST_STATE_KEY_PREVIOUS`) now. Retrofitting a key-id scheme after tokens are in the wild is a breaking change.
   - **RESOLVED: accepted**, as recommended. Adopted by **plan 03 Task 1**:
     `PMCP_REQUEST_STATE_KEY_PREVIOUS` (that exact spelling — the shared
     `PMCP_REQUEST_STATE_KEY` prefix groups both vars for secret scanning) is read into the
     codec's `accepting` set as **verify-only, never for minting**. **Plan 03 Task 2**'s
     behavior list asserts a token minted under the previous key still yields
     `Verdict::Ok`, and that an unknown key-id yields `Verdict::UnknownKey` rather than
     `AuthFailed` (D-15).

---

## Sources

### Primary (HIGH confidence)
- `github.com/modelcontextprotocol/modelcontextprotocol` → `schema/draft/schema.ts` @ main (3184 lines, downloaded and read in full for the relevant sections; last commit 2026-07-16 #3002) — `InputRequests`/`InputResponses`/`InputRequiredResult`/`InputResponseRequestParams` (540-608), `ResultType`/`Result` (205-235), `ResultMetaObject.serverInfo` (143-158), `SubscriptionFilter`/`SubscriptionsListenRequest`/`SubscriptionsListenResult` (1260-1355), `ClientRequest`/`ServerNotification`/`ServerResult` unions (3140-3184), `ClientCapabilities`/`ServerCapabilities` (745-880), error constants `-32020`/`-32021`/`-32022` + the `-32002` reservation note (420-460), `ElicitRequestFormParams.mode?` (2775-2845)
- `modelcontextprotocol.io/specification/draft/basic/patterns/mrtr` — full MRTR server/client requirements incl. the 3-part replay-prevention list and the `"AEAD-protected blob"` example
- `modelcontextprotocol.io/specification/draft/basic/transports/streamable-http` — GET/session/`Last-Event-ID` removal, 405/404/400 mappings, required-header table, base64 sentinel encoding, `x-mcp-header`, `subscriptions/listen` stream semantics, MRTR message-flow diagrams
- `modelcontextprotocol.io/specification/draft/basic/patterns/subscriptions` — ack-first MUST, `subscriptionId` tagging, filter containment, graceful closure
- `modelcontextprotocol.io/specification/draft/basic/transports` — binding model, "servers do not initiate JSON-RPC requests"
- `github.com/modelcontextprotocol/conformance` → `src/scenarios/server/input-required-result.ts` (1644 lines, read directly — all 13 MRTR scenarios and their check ids), `src/scenarios/server/stateless.ts` (the SEP-2575 scenario incl. the decisive `advertisesSubscriptions` gating logic at 975-1015), scenario directory listing (`sse-polling.ts` tagged `removedIn: DRAFT`), `src/scenarios/server/negative-mrtr.test.ts`
- Direct codebase reads (2026-07-24): `Cargo.toml` (deps 52-142, features 169-212), `Cargo.lock` (ring/aws-lc-rs/hmac/sha2/getrandom/subtle/zeroize entries), `src/server/streamable_http_server.rs` (stateless() 250, router 289-291, session sites 867/910/1088/1495, `handle_get_sse` 2347, `replay_events_after` 48/94), `src/server/core.rs` (`ResponseDisposition` 1092-1122, `inject_v2_result_envelope` 1141-1170, `build_discover_response` 1201), `src/types/protocol/error_codes.rs` (12 constants, no -3202x), `src/types/elicitation.rs` (tagged enum), `src/types/tools.rs:454-471` (`CallToolRequest` + the `task` additive-field precedent), `src/client/host/mod.rs` (registry 51, `classify_host_request` 114), `src/client/mod.rs` (builder 2891), `src/shared/streamable_http.rs` (no MCP_METHOD/MCP_NAME), `src/shared/http_constants.rs`, `Makefile` (test targets, quality-gate 673)
- `.planning/phases/112-version-plumbing-spine/112-VERIFICATION.md` — the verified state of the spine this phase builds on

### Secondary (MEDIUM confidence)
- `blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/` — RC lock date (2026-05-21) and final publication date (2026-07-28)
- `.planning/research/{SUMMARY,PITFALLS,STACK}.md` (2026-07-22) — milestone-level architecture, the zero-new-deps constraint, and the Open Verification Item this research resolves
- `cargo search` / `cargo info` for `ring`, `chacha20poly1305`, `aes-gcm`, `hmac`, `jsonschema` — versions and source repositories

### Tertiary (LOW confidence — flagged)
- The claim that the draft schema will publish unchanged on 2026-07-28 (A1). No source can confirm this before the date.
- `slopcheck` verdicts — tool unavailable in this environment; crate legitimacy established by `cargo info` + `Cargo.lock` presence instead.

---

## Metadata

**Confidence breakdown:**
- **MRTR wire contract:** HIGH — every field, location, and obligation traced to the schema source **and** independently corroborated by the official conformance suite's own request construction.
- **D-12 / `subscriptions/listen`:** HIGH — resolved by reading the conformance suite's gating logic verbatim, not by inference from prose.
- **HTTP-01 / HTTP-05 semantics:** HIGH — spec states the removals in explicit MUST/SHOULD language with status codes.
- **Codebase integration points:** HIGH — every file/line reference read directly this session.
- **Crypto primitive availability:** HIGH for `Cargo.lock` presence; MEDIUM for the assertion that promoting `ring` to a direct dep is friction-free (A2 — verify by building).
- **Final-spec stability:** MEDIUM — 4 days from publication; the draft has been stable for 8 days and the RC locked 2026-05-21, but this is the one irreducible unknown.
- **Scope boundaries (A7, A8):** MEDIUM — two items (opt-in stream, `x-mcp-header`) genuinely need an owner decision rather than a research answer.

**Research date:** 2026-07-24
**Valid until:** **2026-07-28** — the final spec publication date. Re-verify the schema diff and re-pin the conformance suite commit on that date before locking any wire-exact value.
