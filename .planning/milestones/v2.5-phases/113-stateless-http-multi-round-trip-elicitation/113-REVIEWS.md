---
phase: 113
reviewers: [gemini, codex]
reviewed_at: 2026-07-25T01:01:45Z
plans_reviewed: [113-01-PLAN.md, 113-02-PLAN.md, 113-03-PLAN.md, 113-04-PLAN.md, 113-05-PLAN.md, 113-06-PLAN.md, 113-07-PLAN.md, 113-08-PLAN.md, 113-09-PLAN.md, 113-10-PLAN.md, 113-11-PLAN.md, 113-12-PLAN.md]
rounds:
  - round: 1
    reviewed_at: 2026-07-25T01:01:45Z
    reviewers: [gemini, codex]
    plans: [113-01..113-12]
    outcome: replan — added 113-13 (see § Review Adjudication)
  - round: 2
    reviewed_at: 2026-07-26T23:05:11Z
    reviewers: [gemini, codex]
    plans: [113-17-PLAN.md, 113-18-PLAN.md, 113-19-PLAN.md]
    outcome: divergent — codex HIGH (4 findings, all verified), gemini LOW
---

# Cross-AI Plan Review — Phase 113

## Gemini Review

# Plan Review: PMCP SDK Phase 113 — Stateless HTTP + Multi-Round-Trip Elicitation

## Executive Summary & Overall Verdict

**Verdict:** **APPROVED WITH HIGH CONFIDENCE**

Phase 113 presents a exceptionally well-researched, architecturally sound, and security-hardened implementation plan for extending the PMCP SDK with MCP 2026-07-28 (v2) Stateless HTTP transport and Multi-Round-Trip Requests (MRTR / SEP-2322).

### Key Highlights
- **Zero-Fork Era Gating:** Era-gating onto the existing `stateless()` streamable-HTTP branch using a unified `sessions_active(state, era)` predicate avoids transport code duplication and guarantees byte-identical behavior for v1 connections.
- **Cryptographically Robust `requestState` Token:** The AEAD token design (`ring::aead`, ChaCha20-Poly1305 / AES-256-GCM) incorporates principal, method, and salient parameter digest in AAD, mitigating cross-principal and cross-request replay attacks.
- **Conformance & DX Reconciliation:** The `key_id` prefix strategy cleverly resolves the tension between developer DX (fail-closed re-elicitation on fallback key mismatch across instances) and strict conformance testing (`sep-2322-reject-tampered-state` requiring JSON-RPC errors for tampered tokens).
- **Semver & Package Legitimacy Preservation:** Adapting MRTR parameters dynamically via `src/types/mrtr.rs` without modifying public request structs (`CallToolRequest`, etc.) preserves semver stability. Promoting `ring` and `zeroize` to explicit optional dependencies under `streamable-http` adds zero new crates to `Cargo.lock`.

---

## 1. Architectural & Requirement Coverage Matrix

| Requirement | Description | Plan Alignment & Implementation Strategy | Assessment |
| :--- | :--- | :--- | :--- |
| **HTTP-01** | Stateless HTTP (no `initialize` handshake, no `Mcp-Session-Id` header) | Wave 2 (`113-04`): Era-gated onto `stateless()` via `sessions_active(state, era)`. GET/DELETE yield 405 on v2 path. | **Pass** — Zero transport fork |
| **HTTP-02** | `input_required` disposition with `inputRequests` + AEAD `requestState` token | Wave 1 & 2 (`113-02`, `113-03`): Typed MRTR adapter; AEAD token authenticated with AAD binding (principal + method + salient param digest). | **Pass** — Replay-proof & tampered-state checked |
| **HTTP-03** | Client retry with `inputResponses` + `requestState` resumes operation | Wave 3 (`113-06`, `113-07`): Server ingress extracts & validates state; client auto-orchestrates bounded retry with a **fresh** JSON-RPC ID per round. | **Pass** — Fully satisfies SEP-2322 |
| **HTTP-04** | Change notifications via `subscriptions/listen` long-lived stream | Wave 5 (`113-10`): Opt-in SSE stream. Default server configuration advertises no subscription capabilities and returns `-32601` (404), matching official conformance skip rules. | **Pass** — Reconciles D-11 & D-13 |
| **HTTP-05** | No `Last-Event-ID` resumability on v2; response ID derived from live request | Wave 4 (`113-08`): Ignores `Last-Event-ID` on v2; forces response envelope serialization to always use the active request's ID (fixes discovery cache bug class). | **Pass** — Closes ID-replay vulnerability |
| **CLNT-01** | pmcp `Client` speaks v2 (`_meta`, `server/discover`, headers, no `initialize`) | Wave 2 (`113-05`): Explicit per-connection v2 builder opt-in emitting `MCP-Protocol-Version`, `Mcp-Method`, and `Mcp-Name` (with `=?base64?` sentinel). | **Pass** — Full spec header compliance |
| **CLNT-02** | pmcp `Client` fulfills MRTR requests via `ClientHostRegistry` | Wave 3 (`113-07`): 3-way fold across `elicitation`, `sampling`, and `roots` handlers; bounded gather-resend loop. | **Pass** — Reuses existing host registry |

---

## 2. Review of Wave 1 Plans (`113-01-PLAN.md` & `113-02-PLAN.md`)

### Plan 113-01: Foundations & Error Codes
- **Strengths:**
  - Establishes `113-SPEC-RECHECK.md` as an explicit checkpoint to guard against last-minute spec drift before final publication.
  - Promotes `ring` (0.17) and `zeroize` (1.8) to explicit optional dependencies under `streamable-http`. `cargo tree` criteria verify reachability while ensuring zero new entries land in `Cargo.lock`.
  - Defines missing v2 error codes (`HEADER_MISMATCH` -32020, `MISSING_REQUIRED_CLIENT_CAPABILITY` -32021, `UNSUPPORTED_PROTOCOL_VERSION` -32022) with locking unit tests.
- **Verification:** The gated dependency promotion includes appropriate checks (`cargo tree -p pmcp -e normal --features streamable-http --depth 1`) to guarantee direct reachability without dragging `ring` onto `wasm32-unknown-unknown` builds.

### Plan 113-02: MRTR Protocol-Type Layer & Test Harness
- **Strengths:**
  - `src/types/mrtr.rs` serves as a clean, centralized adapter for MRTR wire types (`InputRequests`, `InputResponses`, `InputRequest`, `InputResponse`).
  - Implements `encode_header_value` / `decode_header_value` for non-ASCII `Mcp-Name` values using the spec-mandated `=?base64?...?=` sentinel format.
  - Implements `salient_param_digest` using a strict whitelist approach (canonicalizing parameter keys to make digests order-independent while ignoring `_meta`, `inputResponses`, and `requestState`).
  - Fixes `ElicitRequestParams` deserialization in `src/types/elicitation.rs` so form elicitations omitting `mode` default to `"form"`, resolving compatibility with v2 spec forms.
  - Lifts the test harness into `tests/common/v2.rs`, providing a unified helper (`v2_body`, `v2_headers`, `spawn_default_config`) across integration test suites.

---

## 3. Critical Technical Edge Cases & Recommendations

### 1. Distinguishing Unknown Key IDs from Tampered Tokens (D-15 & Conformance)
- **Context:** In multi-instance deployments where `PMCP_REQUEST_STATE_KEY` is unset, instances fall back to random per-process keys (D-04). Retrying against a different instance fails decryption.
- **Review Finding:** A naive decryption failure handler that re-elicits would fail official conformance check `sep-2322-reject-tampered-state`, which demands a JSON-RPC error when a token is tampered.
- **Recommendation:** Ensure `src/server/request_state.rs` strictly adheres to the planned layout: `base64url(key_id || nonce || ciphertext)`.
  - **Unknown `key_id`:** Instance cannot verify token $\rightarrow$ return fresh `InputRequiredResult` (re-elicit, degraded multi-instance fallback).
  - **Known `key_id` + Tag/Auth Failure:** Token modified or forged $\rightarrow$ return JSON-RPC error `-32602` (conformance compliant).

### 2. Result Envelope Placement for `serverInfo` (Pitfall 6 Fix)
- **Context:** Phase 112 injected `serverInfo` as a top-level property on `Result`. The 2026-07-28 spec places it inside `result._meta["io.modelcontextprotocol/serverInfo"]`.
- **Recommendation:** Verify during Wave 4 (`113-09`) that `inject_v2_result_envelope` constructs `_meta` if absent and places `serverInfo` inside `_meta`.

### 3. Client Capability Derivation (`_meta.clientCapabilities`)
- **Context:** Spec obligation 7 prohibits servers from returning `inputRequests` for client capabilities that were not declared during request negotiation.
- **Recommendation:** In Wave 2 (`113-05`), the client builder should automatically derive `_meta.clientCapabilities` from registered handlers in `ClientHostRegistry` (`elicitation`, `sampling`, `roots`), preventing mismatch errors (`-32021`).

### 4. Trace Context Preservation across MRTR Rounds
- **Context:** Requests carry W3C trace context (`traceparent`, `tracestate`) in `_meta`.
- **Recommendation:** In Wave 3 (`113-07`), ensure the client MRTR loop preserves `_meta` trace context headers across retry rounds so distributed tracing spans remain linked.

---

## 4. Final Summary

The execution roadmap (Waves 1 through 6) is well-ordered and defensively designed. 

- **Wave 1:** Core types, dependencies, error codes, and shared test harness.
- **Wave 2:** AEAD token state engine (`113-03`), HTTP era gate (`113-04`), and client transport headers (`113-05`).
- **Wave 3:** Server MRTR ingress (`113-06`) and client gather-resend loop (`113-07`).
- **Wave 4:** Resumability suppression (`113-08`) and MRTR server egress (`113-09`).
- **Wave 5:** `subscriptions/listen` stream (`113-10`) and E2E conformance harness (`113-11`).
- **Wave 6:** Feature matrix verification and quality gate sweep (`113-12`).

Proceed with executing Wave 1 (`113-01-PLAN.md` and `113-02-PLAN.md`).

---

## Codex Review

# Phase 113 Plan Review

> External-review note: I attempted the available Gemini, Cursor, Claude, and separate Codex CLIs. Sandbox restrictions blocked Gemini/Codex, Cursor’s agent crashed while updating, and Claude was unauthenticated. Therefore this is a direct Codex review, not a multi-model consensus. No repository files were changed.

## Overall assessment

The plans show unusually strong traceability, security thinking, v1 regression coverage, semver awareness, and validation intent. However, they are not ready to execute as written. Several cross-plan contradictions make core outcomes impossible or unsafe: coding against a pre-final draft, an unusable client “return input_required” API, a process-global crypto codec, impossible unknown-key re-elicitation, server-controlled fields that handlers may spoof, incomplete `subscriptions/listen` client support, and incorrect dependency ordering.

Overall risk: **HIGH**. The phase goals are achievable, but the plan set needs an architectural correction pass before implementation.

## Blocking findings

1. **Final-spec gate is not actually blocking.** Plan 01 permits using the draft on July 24 and then hard-coding values, contradicting the requirement that wire-exact values come only from the published July 28 schema.
2. **The client cannot return an unfulfilled `input_required` result.** Existing methods return `CallToolResult`, `GetPromptResult`, or `ReadResourceResult`; deserialization discards `inputRequests`, `requestState`, and `resultType`.
3. **Unknown-key/expired re-elicitation is underspecified or impossible.** With no decrypted continuation, the server cannot mint a meaningful fresh state-only token while also refusing to invoke the handler.
4. **The crypto codec cannot be process-global.** A `OnceLock` conflicts with builder overrides, multiple servers, integration tests, key rotation, and reliable startup validation.
5. **Handler-controlled `resultType` defeats the eligibility tripwire.** Preserving a handler-provided reserved field lets a handler emit `input_required` on any method.
6. **The internal MRTR signal leaks on v1 and unsupported methods.** Plan 09 explicitly leaves signal-bearing v1 results untouched, exposing continuation state in plaintext.
7. **HTTP-04 lacks the pmcp Client half.** The plans implement a raw server SSE route but no `Client::subscriptions_listen` stream API, and existing v2 client subscribe methods still use retired RPCs.
8. **Mandatory project workflow is not followed.** Contract updates are deferred until Plan 12 rather than preceding implementation; PDMT todo generation and PMAT quality-proxy writes are absent.

---

## Plan 01 — Foundations

[113-01-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-01-PLAN.md)

**Summary:** Good foundational scope, but the schema checkpoint does not enforce the project’s final-spec-only rule.

**Strengths**

- Centralizes the three new error codes with locking tests.
- Carefully validates that `ring` and `zeroize` add no new packages.
- Preserves optional/wasm feature boundaries.
- The human dependency-legitimacy checkpoint is explicit and evidence-based.

**Concerns**

- **HIGH:** Task 1 falls back to `schema/draft/schema.ts`, after which Task 3 still lands wire constants. That violates VERS-06 and the stated out-of-scope rule against pre-final hard-coding.
- **MEDIUM:** `Cargo.lock` is omitted from `files_modified`; promoting direct dependencies normally changes the root package’s dependency list in the lockfile.
- **MEDIUM:** The zeroize-rejection fallback contradicts unconditional must-haves and acceptance criteria requiring `zeroize`.
- **MEDIUM:** The checkpoint records the schema commit but does not re-pin the official conformance-suite commit.
- **LOW:** A blocking human gate in Wave 1 makes the phase non-autonomous, which should be reflected at the phase level.

**Suggestions**

- Make publication of `schema/2026-07-28` a hard prerequisite. If absent, stop after recording `PENDING`, not `CONFIRMED`.
- Re-pin and record the conformance-suite commit in the same checkpoint.
- Add `Cargo.lock` to modified files.
- Remove the zeroize fallback or make all must-haves and acceptance criteria conditional.

**Risk assessment:** **HIGH**, because every downstream wire decision depends on this gate being authoritative.

---

## Plan 02 — MRTR types and shared harness

[113-02-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-02-PLAN.md)

**Summary:** A strong attempt at a single wire adapter, but parsing semantics and public-surface choices need tightening.

**Strengths**

- Correctly avoids adding fields to constructible public request structs.
- Correctly locates MRTR fields at top-level `params`.
- Centralizes eligible methods, logical names, header encoding, and salient-parameter hashing.
- Includes property tests and literal wire-key assertions.
- The stateful test-server helper is an important defense against vacuous tests.

**Concerns**

- **HIGH:** `extract_mrtr_params` conflates absent, malformed, oversized, and wrong-shaped values by returning `None`. A tampered or oversized `requestState` can therefore be treated as absent rather than rejected.
- **HIGH:** `splice_mrtr_params` only inserts present fields. Across rounds, absent fields must remove stale `inputResponses` and `requestState`; otherwise old round data can leak into later retries.
- **HIGH:** The planned `v2_body` helper mentions protocol version but not the required empty-or-populated `clientCapabilities`.
- **MEDIUM:** `InputResponse` as an untagged enum can misclassify overlapping or future result shapes. The corresponding request method/key should guide decoding.
- **MEDIUM:** `pub mod mrtr` plus broad flat re-exports exposes implementation-oriented wire types despite D-10 describing an internal adapter.
- **MEDIUM:** Header encoding rules are underspecified. “Printable ASCII excluding delimiters” needs to reproduce the final spec exactly.
- **LOW:** `common_harness_smoke.rs` is created but missing from `files_modified`.

**Suggestions**

- Return `Result<MrtrRequestParams, MrtrParseError>` with distinct absent and invalid states.
- Have `splice_mrtr_params` remove both keys before inserting current values.
- Include `clientCapabilities: {}` in every shared v2 test request.
- Keep parsing helpers crate-private; expose only deliberate authoring/result types.
- Decode responses using the original `InputRequest` kind rather than an untagged guess.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 03 — `requestState` AEAD

[113-03-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-03-PLAN.md)

**Summary:** Cryptographic primitives and bindings are well selected, but codec ownership and configuration are architecturally wrong.

**Strengths**

- Uses a standard AEAD with random nonces and AAD.
- Binds principal, method, and salient parameters.
- Distinguishes unknown key IDs from authentication failure.
- Includes property and fuzz testing.
- Avoids secret-bearing `Debug` output.

**Concerns**

- **HIGH:** A process-global `OnceLock<RequestStateCodec>` prevents per-server builder configuration, multiple differently configured servers, deterministic integration tests, and clean key rotation.
- **HIGH:** Malformed configured keys are logged and replaced with random fallback keys. D-04 permits fallback only when unset; malformed production configuration should fail server construction.
- **HIGH:** `codec()` initializes on first request, not at server startup, so the required startup warning and startup validation are not reliable.
- **MEDIUM:** D-05 calls for env/builder TTL configuration, but only an env override is planned.
- **MEDIUM:** Expiry tests lack an injected clock; “already elapsed” tokens otherwise require races, sleep, or hand-crafted ciphertext.
- **MEDIUM:** Only decoded key bytes are scrubbed; the environment `String` and intermediate buffers remain.
- **MEDIUM:** An eight-byte key ID can collide, and the accepting set’s collision behavior is unspecified.
- **LOW:** A hidden public fuzz-support seam permanently expands the public API.

**Suggestions**

- Construct `Arc<RequestStateCodec>` during `Server`/`ServerCore` build and store it in server state.
- Add builder methods for current key, previous keys, TTL, and an injectable clock.
- Fail server build on malformed configured keys; use fallback only when the variable is absent.
- Return `Expired(Continuation)` if expiry must permit clean re-elicitation.
- Use a fuzz-only feature or internal harness instead of a permanent public seam.

**Risk assessment:** **HIGH**.

---

## Plan 04 — Stateless HTTP era gate

[113-04-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-04-PLAN.md)

**Summary:** The single-predicate session design is correct, but header rules and rejection plumbing remain inconsistent.

**Strengths**

- Tests against `Default::default()` rather than the already-stateless config.
- One `sessions_active` predicate minimizes v1/v2 drift.
- Explicitly covers inbound bogus session IDs, response suppression, GET/DELETE, and status mappings.
- Keeps v1 behavior separately asserted.

**Concerns**

- **HIGH:** The existing server gate requires all three headers on all v2 requests, while Plan 05 says name-less methods emit no `Mcp-Name`. `server/discover` and list requests cannot satisfy both plans.
- **MEDIUM:** `V2GateOutcome::Reject(code, message)` cannot carry the required `supported` data for `-32022` without an interface change not described here.
- **MEDIUM:** Unknown-method 404 must be applied before or during typed `ClientRequest` deserialization; mapping only an already-built response may miss raw unknown methods.
- **LOW:** Grep-based “no numeric literals” and exact test-count checks are brittle.

**Suggestions**

- Lock the final `Mcp-Name` matrix before implementation: always required with a sentinel/empty representation, or optional for name-less methods. Apply the same rule in Plans 02, 04, and 05.
- Make rejection carry structured `data`.
- Add raw-body tests for unknown methods, malformed JSON, unsupported versions, and string IDs.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 05 — v2 Client transport

[113-05-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-05-PLAN.md)

**Summary:** It identifies the correct client responsibilities, but does not explain how protocol mode reaches the generic transport or how capabilities work without initialization.

**Strengths**

- Explicit per-connection opt-in honors D-08.
- Capabilities derive from registered handlers.
- Headers derive from the outgoing body rather than duplicate caller input.
- Tests cover URI names, Unicode sentinel encoding, no handshake, and no session.

**Concerns**

- **HIGH:** `ClientBuilder` stores protocol mode, but `StreamableHttpTransport` is responsible for HTTP headers. The plan does not define how a generic `Client<T>` propagates mode into `T`.
- **HIGH:** Existing client operations call `assert_capability` using capabilities learned during `initialize`. With no initialize and no automatic discover, normal v2 calls may fail locally before sending.
- **HIGH:** Name-less requests omit `Mcp-Name`, contradicting Plan 04’s three-header gate.
- **HIGH:** The non-ASCII live test needs Plan 04’s server decoder, but Plan 05 does not depend on Plan 04.
- **MEDIUM:** `with_protocol_version` is available for all transports, although stdio-v2 behavior is explicitly out of scope.
- **MEDIUM:** Arbitrary protocol-version inputs need validation.

**Suggestions**

- Define one shared `ProtocolMode` that is passed into transport send context, or constrain the method to the streamable-HTTP client builder.
- Decide whether explicit v2 mode automatically calls `server/discover` for capabilities or makes capability enforcement era-aware. This is not the prohibited era auto-probe.
- Add dependency on Plan 04, or defer the cross-server Unicode test.
- Test every name-less v2 method against the finalized header rule.

**Risk assessment:** **HIGH**.

---

## Plan 06 — MRTR server ingress

[113-06-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-06-PLAN.md)

**Summary:** The security verdict table is good, but its re-elicitation behavior cannot be implemented with the information retained.

**Strengths**

- Verifies before handler invocation.
- Uses authenticated subject rather than self-reported client info.
- Exposes trusted continuation separately from untrusted input responses.
- Covers exact tampering, principal mismatch, argument replay, and method replay.

**Concerns**

- **HIGH:** For `UnknownKey`, no continuation can be decrypted. The server cannot mint a meaningful fresh state-only token while also refusing to invoke the original handler.
- **HIGH:** `Expired` currently contains no continuation, producing the same problem unless the handler is rerun.
- **HIGH:** Malformed/oversized states can already have been converted to `None` by Plan 02, bypassing the verdict table.
- **HIGH:** Using `""` for every unauthenticated caller does not meaningfully satisfy “principal-bound”; tokens become transferable between anonymous callers.
- **HIGH:** No count or byte limits are imposed on `inputResponses`; cloning them through `ProtocolContext` and `RequestHandlerExtra` creates a memory/CPU DoS surface.
- **MEDIUM:** Process-global codec initialization makes environment-controlled integration tests order-dependent.

**Suggestions**

- Define re-elicitation precisely:

  - Unknown key: strip MRTR fields and re-run the original handler to recreate both request and continuation.
  - Expired authentic token: either retain `Expired(Continuation)` and reseal it, or rerun the handler.

- Reject malformed/oversized fields distinctly at ingress.
- Require an authenticated/stable principal for state-bearing continuation, or explicitly define a safe anonymous-binding mechanism.
- Add limits for response count, per-entry size, total serialized size, and nesting depth.

**Risk assessment:** **HIGH**.

---

## Plan 07 — Client MRTR loop

[113-07-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-07-PLAN.md)

**Summary:** The loop semantics are mostly sound, but the public return type and execution ordering are unresolved.

**Strengths**

- Bounded logical-operation loop with fresh IDs.
- Handles all three request kinds and state-only retries.
- Correctly treats future non-`input_required` result types as terminal.
- Uses the existing host registry and preserves server-assigned keys.

**Concerns**

- **HIGH:** Existing public methods return concrete result structs. If no handler exists, those structs discard `inputRequests`, `requestState`, and `resultType`, so “return the result to the caller” is not possible.
- **HIGH:** Live fixture handlers cannot emit `input_required` before Plan 09, yet Plan 07 does not depend on Plan 09.
- **HIGH:** Mutable retry params can retain stale MRTR fields unless Plan 02’s splice helper deletes old keys first.
- **MEDIUM:** Direct handler calls risk bypassing existing sampling approval, tool-choice handling, and result-review safeguards.
- **MEDIUM:** The fold invokes handlers before proving every requested kind is fulfillable. A later missing handler can leave earlier prompts or side effects wasted.
- **MEDIUM:** Handler errors are swallowed into “return original result” without a logging or observability requirement.
- **LOW:** `Error::Protocol.code` is an `ErrorCode`, not a bare `i32`; the proposed constructor needs the proper wrapper.

**Suggestions**

- Decide the public API first. Options include:

  - New additive `MrtrOutcome<T> { Complete(T), InputRequired(InputRequiredResult) }` methods.
  - Existing methods auto-orchestrate and return a structured client-local error containing the raw result when fulfillment is impossible.
  - A lower-level raw v2 request API for manual handling.

- Make Plan 07 unit/mock-transport only, then leave real server integration to Plan 11; otherwise move it after Plan 09.
- Preflight all requested kinds before invoking any handler.
- Reuse the complete existing host-dispatch pipeline, including approvals and review hooks.

**Risk assessment:** **HIGH**.

---

## Plan 08 — No resumability and live IDs

[113-08-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-08-PLAN.md)

**Summary:** Era-gating event replay is appropriate, but the stated ID invariant conflicts with preserved v1 replay behavior.

**Strengths**

- Separately gates event reads, replay, and storage.
- Preserves legacy event-store code.
- Includes sequential, concurrent, string-ID, and error-ID tests.
- The event-store bypass reduces retained v2 envelopes.

**Concerns**

- **HIGH:** “Every response ID equals the live request ID on both eras” conflicts with v1 replay, whose historical events legitimately retain their original IDs. Direct responses and replayed events must be distinguished.
- **MEDIUM:** Sequential `tools/list` calls do not test the documented cached-envelope bug if the server never caches that response.
- **MEDIUM:** A source-code audit comment and `debug_assert!` are weaker than a design that structurally requires a live ID when enveloping a cached payload.
- **LOW:** Counting `LAST_EVENT_ID` occurrences is brittle.

**Suggestions**

- Scope the invariant to direct responses to a live request; separately assert that v1 replay retains historical event identity.
- Add a fixture that deliberately reuses a cached result payload and verifies it is re-enveloped with each live ID.
- Use an event-store spy to prove zero v2 reads/writes rather than relying only on response behavior.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 09 — MRTR server egress

[113-09-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-09-PLAN.md)

**Summary:** This plan provides the missing handler authoring surface, but reserved-field ownership and signal stripping are unsafe.

**Strengths**

- Covers tools, prompts, and resources through one signal type.
- Mints continuation state at a centralized boundary.
- Includes an exhaustive eligible-method tripwire.
- Corrects `serverInfo` nesting.
- Checks declared client capabilities.

**Concerns**

- **HIGH:** On v1, signal-bearing results are “left untouched,” exposing `dev.pmcp/mrtr` and plaintext continuation state on the wire.
- **HIGH:** On a non-eligible v2 method, early return similarly leaves the internal signal key serialized.
- **HIGH:** `inject_v2_result_envelope` preserves handler-supplied `resultType`. A handler can set `"input_required"` on `tools/list`, bypassing the exhaustive tripwire.
- **HIGH:** Handler-supplied `serverInfo` is also preserved, allowing server identity spoofing.
- **MEDIUM:** Capability checking considers only presence of `elicitation`; it must distinguish form versus URL support and any relevant sampling sub-capabilities.
- **MEDIUM:** `ReadResourceResult._meta: Option<Value>` is less constrained than the map types used by tool and prompt results, complicating safe merge behavior.
- **MEDIUM:** Capability validation occurs after signal extraction/minting instead of before work is performed.
- **LOW:** `MrtrSignal::into_meta_entry` is declared infallible even though `serde_json::to_value` returns a result.

**Suggestions**

- Treat `resultType`, server-info metadata, request state, and the internal signal key as server-owned reserved fields: overwrite or reject collisions.
- Always remove the internal signal before serialization. On v1 or an unsupported method, return a structured internal/invalid-state error rather than leaking it.
- Prefer an explicit internal `HandlerOutcome::InputRequired` over smuggling control flow through result metadata.
- Validate capability submodes before minting.
- Use consistent map-shaped result metadata.

**Risk assessment:** **HIGH**.

---

## Plan 10 — `subscriptions/listen`

[113-10-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-10-PLAN.md)

**Summary:** The capability-gated server concept matches D-13, but the planned stream registry is incomplete for production and does not fulfill the client-facing requirement.

**Strengths**

- Correctly supports both conformant configurations.
- Encodes advertise-implies-serve as a tripwire.
- Requires ack-first delivery and filter intersection.
- Retires legacy subscription RPCs only on v2.
- Recognizes connection limits and the enterprise tradeoff.

**Concerns**

- **HIGH:** No pmcp Client `subscriptions/listen` streaming API is planned. Raw `reqwest` tests do not prove that v2 pmcp clients can receive change notifications.
- **HIGH:** Existing client `resources/subscribe`/`unsubscribe` methods remain callable on v2.
- **HIGH:** Registry keys use the JSON-RPC ID alone; different principals/connections commonly reuse IDs such as `1`, causing collisions.
- **HIGH:** An unbounded channel allows an arbitrarily slow subscriber to accumulate unbounded notifications.
- **HIGH:** Disconnect-safe unregistering is not designed. A dropped SSE response can leak registry entries and concurrency permits.
- **HIGH:** Per-principal limits require an authenticated principal, but this raw transport interception does not explain how `AuthContext` reaches the listen registry.
- **MEDIUM:** In-memory streams are instance-local. Advertising the capability behind a load balancer can silently miss notifications generated on another instance.
- **MEDIUM:** The final SSE result may bypass normal `resultType`/`serverInfo` envelope injection and required outbound headers.
- **MEDIUM:** `src/server/core.rs` and client files may need changes but are absent from `files_modified`.
- **MEDIUM:** No trigger for graceful server-side closure is defined.
- **MEDIUM:** Exact field types for `resourceSubscriptions` and the `notifications` wrapper need to be locked from the final schema.

**Suggestions**

- Add an additive client streaming API and era-gate legacy client methods.
- Key entries by `(principal-or-connection-id, RequestId)`.
- Use bounded channels with a documented lag policy.
- Use an RAII/drop guard or cancellation token to unregister and release permits.
- Advertise stream capabilities only when the configured notification backend can reach all serving instances, or clearly restrict opt-in mode to single-instance/sticky deployments.
- Route stream acknowledgements and final results through shared v2 envelope/header helpers.

**Risk assessment:** **HIGH**.

---

## Plan 11 — Conformance mirror and example

[113-11-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-11-PLAN.md)

**Summary:** Excellent verification intent, but the conformance inventory is static and inherits unresolved API and ordering problems.

**Strengths**

- Mirrors wire behavior with raw HTTP rather than only self-round-tripping pmcp types.
- Adds real Client↔server integration.
- Covers mixed input kinds, multiple rounds, under-supply, and exact IDs.
- Provides the required runnable example.

**Concerns**

- **HIGH:** Tests are derived from the July 24 research table, not a freshly pinned final conformance-suite commit.
- **HIGH:** Real client tests depend on the unresolved Plan 07 return-type problem.
- **MEDIUM:** “Every official check” spans multiple test files, but the plan lacks a manifest mapping each pinned official scenario to one exact local test.
- **MEDIUM:** The example demonstrates mainly the server side; a paired example client would better prove automatic fulfillment.
- **MEDIUM:** The actual 20k fuzz run is not included in this plan’s automated verification or the final phase gate.
- **LOW:** The server-process `timeout` verification is environment-sensitive, though this workspace currently has it.

**Suggestions**

- Generate and commit a scenario-to-test inventory from a pinned conformance commit.
- Fail if an official `sep-2322` scenario exists without a local mapping.
- Add a small real v2 client to the example or a companion example.
- Include the actual bounded fuzz run in phase verification.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 12 — Phase gate

[113-12-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-12-PLAN.md)

**Summary:** Strong final validation scope, but it places mandatory contract work too late and omits required development workflow controls.

**Strengths**

- Explicitly addresses feature-unification false greens.
- Includes no-default, wasm, examples, semver, public API, docs, and full quality gates.
- Reconciles HTTP-04 wording and records SEP-2243 rather than silently absorbing it.
- Enumerates public API additions for human review.

**Concerns**

- **HIGH:** Contract-first work occurs after every implementation plan. Repository rules require contract update and `pmat comply check` before implementation.
- **HIGH:** The referenced `../provable-contracts/contracts/pmcp/` checkout is external to this repository and is not present in the current workspace.
- **HIGH:** Plans do not use mandatory PDMT todo generation or the PMAT quality-proxy for writes.
- **HIGH:** Requirements are marked complete even though the actual official conformance suite is deferred to Phase 118 and the final-spec gate may not have run against the published schema.
- **MEDIUM:** The dev-dependency-free all-features command is described vaguely rather than specified exactly.
- **MEDIUM:** The automated verification uses ordinary `cargo`, while the action requires an absolute rustup cargo path.
- **MEDIUM:** No coverage measurement demonstrates the mandated 80% target.
- **MEDIUM:** The final gate does not run the actual fuzz target.
- **LOW:** Updating roadmap, requirements, external contracts, and full validation in one final plan makes failure recovery cumbersome.

**Suggestions**

- Move contract updates and initial compliance checks into a new Wave 0 plan.
- Add the required PDMT and quality-proxy workflow to every implementation plan.
- Specify exact reproducible matrix commands.
- Add coverage and bounded fuzz execution to the phase gate.
- Mark requirements complete only after the published schema checkpoint and a pinned scenario inventory pass.

**Risk assessment:** **HIGH**.

---

## Cross-plan dependency assessment

The current graph needs these corrections:

| Plan | Problem | Recommended correction |
|---|---|---|
| 05 | Unicode live test requires Plan 04’s server decoder | Add dependency on 04, or move that test later |
| 07 | Scripted real server cannot emit MRTR until Plan 09 | Use a mock transport only, or move 07 after 09 |
| 09 | Reserved-field behavior depends on envelope ownership decisions | Resolve before client loop and E2E work |
| 10 | Needs client, core-envelope, auth, and notification-backend integration | Expand files/dependencies and add client work |
| 11 | Uses static research rather than final pinned suite | Depend on a real final-spec/conformance checkpoint |
| 12 | Contract-first task occurs last | Split contract work into Wave 0 |

A safer sequence would be:

1. Final schema and conformance pin; contract/PDMT setup.
2. Internal wire types and per-server codec configuration.
3. Stateless HTTP and v2 transport mode.
4. Server ingress and egress together.
5. Client outcome API and MRTR loop.
6. HTTP-05 invariants.
7. Subscription server and client surfaces.
8. Conformance mirror, example, feature matrix, and quality gate.

## Requirement coverage assessment

| Requirement | Assessment |
|---|---|
| HTTP-01 | Achievable after resolving the header contradiction |
| HTTP-02 | Partial; re-elicitation and reserved-field ownership are blockers |
| HTTP-03 | Partial; stale params and public client outcome API are blockers |
| HTTP-04 | Not fully covered; server route exists, pmcp Client support does not |
| HTTP-05 | Partial; direct-response and historical-replay ID semantics conflict |
| CLNT-01 | Partial; transport-mode propagation and capability bootstrap are missing |
| CLNT-02 | Not achieved for no-handler/decline paths with current return types |

## Final risk assessment

**Overall risk: HIGH.**

The plans have excellent testing discipline and unusually good security traceability, but several issues are architectural rather than implementation details. Executing the current waves would likely produce mid-phase redesigns, cyclic dependencies, or superficially green tests that cannot support the promised public behavior. The most important next step is a focused replanning pass around:

1. the client result/outcome API;
2. per-server request-state codec ownership;
3. exact re-elicitation semantics;
4. server ownership of reserved envelope fields;
5. the full client-and-server subscription surface;
6. the final-spec and contract-first gates.

---

## Consensus Summary

The two reviewers reached **opposite overall verdicts** — Gemini: *approved with high confidence* (proceed to Wave 1); Codex: *overall risk HIGH, needs an architectural correction pass before execution*. Codex's review is substantially deeper (per-plan findings with severities); Gemini's is a requirement-coverage validation. Where they overlap, they agree on the strengths and on four technical pressure points.

### Agreed Strengths (both reviewers)

- **Zero-fork era gating** onto the existing `stateless()` branch via the single `sessions_active(state, era)` predicate — no transport duplication, v1 behavior isolated.
- **AEAD `requestState` design** (principal + method + salient-param digest in AAD) is cryptographically sound and replay-resistant; the **key-id prefix** cleverly reconciles D-04's fail-closed re-elicitation with the `sep-2322-reject-tampered-state` conformance check.
- **Semver-preserving MRTR adapter** (`src/types/mrtr.rs` splice/extract at the wire boundary instead of widening public request structs).
- **Dependency hygiene** — `ring`/`zeroize` promoted from transitive to explicit optional deps with zero new crates, verified by `cargo tree --depth 1` + lockfile assertions.
- **Testing discipline** — property/fuzz coverage, literal wire-key assertions, stateful test-server helpers guarding against vacuous tests.

### Agreed Concerns (raised by both, differing severity)

1. **Re-elicitation semantics for unknown-key/expired tokens** — Gemini prescribes the verdict table as the thing to "strictly adhere to"; Codex goes further (HIGH): with no decrypted continuation, the server *cannot* mint a meaningful fresh state-only token without re-running the handler — the exact mechanism must be defined (strip MRTR fields and re-run handler, or carry `Expired(Continuation)`).
2. **Client capability bootstrap without `initialize`** — Gemini recommends auto-deriving `_meta.clientCapabilities` from registered host handlers; Codex flags (HIGH) that existing `assert_capability` calls rely on initialize-learned capabilities and may fail locally on v2.
3. **`serverInfo` / result-envelope ownership** — Gemini says verify `_meta` placement in Wave 4; Codex flags (HIGH) that handler-supplied `resultType`/`serverInfo` are *preserved* by the envelope injector, enabling spoofing and tripwire bypass — reserved fields must be server-owned (overwrite or reject).
4. **Conformance fidelity** — both anchor correctness to the official `sep-2322` checks; Codex additionally demands a freshly pinned conformance-suite commit + scenario-to-test manifest rather than the July-24 research table.

### Divergent Views (worth investigating before execution)

- **Overall readiness**: Gemini = proceed; Codex = replan first. Codex's 8 "blocking findings" are the crux — most are architectural (client return-type for unfulfilled `input_required`, process-global `OnceLock` codec vs per-server builder config, HTTP-04 missing the pmcp-Client streaming half, v1 leakage of the internal MRTR signal, `subscriptions/listen` registry robustness: id-collision keys, unbounded channels, drop-guard unregistration).
- **HTTP-04 coverage**: Gemini scores it *Pass* (server route + conformant skip); Codex scores it *not fully covered* because no `Client::subscriptions_listen` streaming API is planned and legacy client subscribe methods stay callable on v2.
- **Final-spec gate**: Codex insists the July-28 published schema be a hard prerequisite for wire constants (draft fallback = violation of VERS-06); Gemini treats the SPEC-RECHECK checkpoint as adequate.
- **Project-workflow compliance**: only Codex flags contract-first ordering (contracts updated in Plan 12 instead of before implementation) and absent PDMT/PMAT-proxy usage.

### Top shared action items

1. Define exact re-elicitation mechanics for `UnknownKey`/`Expired` verdicts (the one concern both reviewers converge on hardest).
2. Decide the client-facing API for unfulfilled `input_required` (additive `MrtrOutcome`-style return vs structured error) before Wave 3.
3. Make reserved envelope fields (`resultType`, `serverInfo`, MRTR signal key) server-owned — strip/overwrite on every path, including v1 and non-eligible methods.
4. Resolve the `Mcp-Name` header matrix contradiction between Plans 04 and 05 (three-headers-always vs no-name-methods-omit) against the final spec.

---

## Review Adjudication

**Adjudicated:** 2026-07-25 · **Mode:** `/gsd:plan-phase --reviews` replan · **Outcome:** 12 plans revised in place, 1 plan added (113-13), waves recomputed 6 → 7.

Every Codex blocking finding and HIGH concern was verified against the actual codebase before being accepted. Claims that were wrong, or that conflict with a locked owner decision, are REJECTED with a one-line rationale. Gemini's four recommendations are adjudicated at the end.

### Verification performed before adjudicating

| Claim under test | Method | Result |
|---|---|---|
| Client methods discard `inputRequests`/`requestState`/`resultType` | Read `src/types/tools.rs` `CallToolResult`, `src/types/resources.rs` `ReadResourceResult`, `src/client/mod.rs:577` `call_tool` | **CONFIRMED, worse than claimed.** `CallToolResult.content` has `#[serde(default)]`, so an `input_required` result deserializes into a SILENTLY EMPTY success. `ReadResourceResult.contents` has no default and a custom deserializer, so the same result fails to deserialize entirely. |
| Plan 04's three-header gate contradicts Plan 05's name-less behavior | Read `src/server/streamable_http_server.rs:446` `require_three_headers` and `:492` `cross_check_name` | **CONFIRMED.** `require_three_headers` demands `Mcp-Name` be PRESENT on every v2 request; `cross_check_name` skips the VALUE comparison for non-name-bearing methods. Plan 05's "emit no `Mcp-Name`" would 400 every `tools/list`. |
| Plan 03 uses a process-global `OnceLock` codec | Read 113-03-PLAN.md Task 1 | **CONFIRMED.** The plan text specified `pub(crate) fn codec() -> &'static RequestStateCodec` backed by `std::sync::OnceLock`. |
| `Error::Protocol.code` is `ErrorCode`, not `i32` | Read `src/error/mod.rs:18-28` | **CONFIRMED.** |
| `assert_capability` blocks v2 calls | Read `src/client/mod.rs:2316` `ensure_initialized`, `:2325` `assert_capability` | **CONFIRMED.** `server_capabilities` is populated only by `initialize`; on v2 it is `None`, so `is_some_and(..)` is false and every `call_tool` fails locally before sending. Plan 05 addressed `ensure_initialized` but not this. |
| Existing client subscribe methods stay callable on v2 | Read `src/client/mod.rs:1848`, `:1912` | **CONFIRMED.** |
| `../provable-contracts` checkout is absent | `ls -d ../provable-contracts` | **CONFIRMED ABSENT.** `pmat` IS on PATH; `pdmt` is NOT. |
| Transport has no mode-propagation seam | Read `src/shared/transport.rs:279`/`:328`, `src/shared/streamable_http.rs:289-405` | **PARTIALLY REJECTED.** The seam half-exists: `StreamableHttpTransport` already owns `protocol_version: Arc<RwLock<Option<String>>>` (`:300`), an inherent `set_protocol_version` (`:400`), and header emission from it (`:580`). What is genuinely missing is a `Transport`-trait path from a generic `Client<T>`. |

### Codex blocking findings

| # | Finding | Verdict | Disposition |
|---|---|---|---|
| 1 | Final-spec gate is not actually blocking | **ACCEPTED** | Plan 01 Task 1 now emits a three-state verdict (`PUBLISHED-CONFIRMED` / `PUBLISHED-DRIFT` / `PENDING`); Task 2 is a blocking human gate that also decides the spec path (`proceed` / `wait` / `exception`); Task 3 refuses to land wire constants under `PENDING` without a written `## Recorded Exception`. Plan 12 Task 3 re-verifies before flipping any requirement. `Cargo.lock` added to `files_modified`; the conformance-suite commit is re-pinned in the same checkpoint. |
| 2 | Client cannot return an unfulfilled `input_required` result | **ACCEPTED** (verified worse than reported) | Plan 02 adds public `InputRequiredResult` + `MrtrOutcome<T>`. Plan 07 adds `call_tool_mrtr` / `get_prompt_mrtr` / `read_resource_mrtr` returning `MrtrOutcome`, and makes the EXISTING methods return `Err(Error::input_required_unfulfilled(result))` on v2 instead of deserializing into an empty success. Plan 11 asserts both live. |
| 3 | Unknown-key/expired re-elicitation is underspecified or impossible | **ACCEPTED** (both reviewers' top consensus item) | Plan 03 changes `Verdict::Expired` to `Expired(Continuation)` — the tag already verified, so the plaintext IS available. Plan 06 defines the mechanic exactly: `Reelicit` strips MRTR fields via `splice_mrtr_params(&mut params, &Default::default())` and RE-RUNS the original handler, so the fresh `input_required` carries real `inputRequests`. `UnknownKey` → round 0; `Expired(c)` → round `c.round` preserved, so a hostile server cannot reset the client's D-09 bound. Handler idempotence-up-to-first-`input_required` is documented on `MrtrSignal`. |
| 4 | The crypto codec cannot be process-global | **ACCEPTED** | Plan 03 removes `OnceLock` and the `codec()` free function entirely. `Arc<RequestStateCodec>` is built once in `ServerBuilder::build()` / `ServerCoreBuilder::build()` and stored on server state. Adds `with_request_state_key` / `with_request_state_previous_keys` / `with_request_state_ttl` and an injectable `RequestStateClock`. A malformed CONFIGURED key now FAILS the server build (D-04's fallback covers the UNSET case only); the startup warning is emitted at build time so it is reliable. Key-id collision policy specified. Plan 06's live tests use the builder instead of mutating `PMCP_REQUEST_STATE_KEY`. |
| 5 | Handler-controlled `resultType` defeats the eligibility tripwire | **ACCEPTED** | Plan 09 Task 2 replaces `entry().or_insert()` with OWNERSHIP for a closed reserved set (`resultType`, `_meta` `serverInfo`, `requestState`, `inputRequests`, `dev.pmcp/mrtr`) via `own_reserved_result_fields`, each overwrite/removal logged. Non-reserved handler `_meta` keys still survive. |
| 6 | The internal MRTR signal leaks on v1 and unsupported methods | **ACCEPTED** | Plan 09 Task 1 adds `strip_mrtr_signal`, called UNCONDITIONALLY before any era or eligibility branch. A signal on v1 or a non-eligible method is stripped, logged at `error`, and answered with a JSON-RPC internal error. Plan 11 adds `mrtr_signal_key_never_on_wire`, greping every raw response body. |
| 6b | Prefer an explicit internal `HandlerOutcome::InputRequired` over `_meta` smuggling | **REJECTED** | Would require changing the return types of the public `ToolHandler`/`PromptHandler`/`ResourceHandler` traits — a MAJOR semver break, forbidden by the milestone's additive constraint (`cargo semver-checks` gates every phase). Rationale recorded as a doc comment on `MRTR_SIGNAL_META_KEY`; noted as the right shape for a future 3.0. |
| 7 | HTTP-04 lacks the pmcp Client half | **ACCEPTED** | Judged WITHIN the requirement, not scope creep: HTTP-04's grammatical subject is "**v2 clients get** change notifications", and D-13 scopes only the SERVER ADVERTISEMENT as opt-in. New **plan 113-13** (wave 6) adds `Client::subscriptions_listen -> SubscriptionStream`, era-gates the retired `subscribe_resource`/`unsubscribe_resource` to a typed `retired_on_v2` error, and proves both live. Deliberately minimal: one stream type, one method, one gate. |
| 8 | Mandatory project workflow not followed (contract-first, PDMT, PMAT proxy) | **PARTIALLY ACCEPTED — environment-constrained** | Contract-first ORDERING accepted: plan 01 Task 1 Section C now records the contract-first state BEFORE any implementation plan runs, and plan 12 consumes that record rather than re-discovering it. `../provable-contracts` VERIFIED ABSENT, so the executable step is the in-repo `pmat comply check --path .` that `make quality-gate` already chains — recorded as an environment constraint, not silently skipped. **This is an EXPLICIT, DOCUMENTED DEVIATION from two CLAUDE.md directives that are marked MANDATORY — it is not a claim of compliance.** Plan 01 Task 1 Section C now carries a `### Deviation from CLAUDE.md MANDATORY directives` subsection naming, for each directive, the deviation + the substitute + the residual risk. **PDMT REJECTED as a blocking requirement**: `pdmt` is not on PATH; the GSD `<acceptance_criteria>` + `<verify>` blocks already carry the equivalent quality-gate/success-criteria structure. **PMAT quality-proxy REJECTED as a per-plan requirement**: it needs a running `pmat mcp-server --enable-quality-proxy` process a plan executor cannot assume; the binding checks are `make quality-gate` locally and the PMAT job in CI (`pmat` IS present, so plan 12's complexity run is now mandatory rather than skippable). |

### Codex per-plan HIGH/MEDIUM concerns

| Plan | Concern | Verdict | Disposition |
|---|---|---|---|
| 01 | `Cargo.lock` omitted from `files_modified` | ACCEPTED | Added. |
| 01 | zeroize-rejection fallback contradicts unconditional must-haves | ACCEPTED | Must-haves and acceptance criteria now name the rejection path explicitly. |
| 01 | conformance-suite commit not re-pinned | ACCEPTED | New `## Conformance Suite Pin` section enumerating the `sep-2322` scenario ids; plan 11 derives from it. |
| 01 | blocking human gate makes the phase non-autonomous | ACCEPTED | `autonomous: false` was already set on plan 01; noted in the return summary. |
| 02 | `extract_mrtr_params` conflates absent/malformed/oversized | ACCEPTED | Now returns `Result<MrtrRequestParams, MrtrParseError>` with six distinct variants; plan 06 maps every `Err` to `-32602` at HTTP 400 before dispatch. |
| 02 | `splice_mrtr_params` leaves stale keys | ACCEPTED | Removes both keys unconditionally before inserting; proptest asserts a default-splice leaves neither. |
| 02 | shared `v2_body` omits `clientCapabilities` | ACCEPTED | Now mandatory on every shared test request, with `v2_body_with_caps` for deliberate under-declaration. |
| 02 | untagged `InputResponse` can misclassify | ACCEPTED | `InputRequestKind` + `InputResponse::decode_for(kind, value)`; the untagged path is an explicitly documented server-ingress-only fallback. |
| 02 | broad flat re-exports contradict D-10's "internal adapter" | ACCEPTED | Only `InputRequest`, `InputRequestKind`, `InputRequests`, `InputResponse`, `InputResponses`, `InputRequiredResult`, `MrtrOutcome`, `MrtrSignal` are `pub`; every parsing helper is `pub(crate)`. |
| 02 | header encoding rules underspecified | ACCEPTED | Exact allowed byte set (`0x20..=0x7E` minus `"`, `,`, `;`, `\`), empty-string round-trip guaranteed, self-sentinel values force-encoded. |
| 02 | `common_harness_smoke.rs` missing from `files_modified` | ACCEPTED | Added. |
| 03 | malformed configured key replaced by a random fallback | ACCEPTED | Now fails `ServerBuilder::build()`. D-04 governs the UNSET case only. |
| 03 | lazy init makes the startup warning unreliable | ACCEPTED | Emitted at build time. |
| 03 | D-05 wants env AND builder TTL | ACCEPTED | `with_request_state_ttl` added; builder beats env. |
| 03 | expiry tests need an injected clock | ACCEPTED | `RequestStateClock` trait + `FixedClock`; acceptance criteria forbid `sleep` in the module. |
| 03 | only decoded key bytes scrubbed | ACCEPTED | The env `String` is zeroized too. |
| 03 | 8-byte key-id can collide | ACCEPTED | Every matching entry is tried; `UnknownKey` only when none matches, so a collision yields `AuthFailed`, never a false `Ok`. Forced-collision test required. |
| 03 | hidden public fuzz seam expands the API | ACCEPTED | Replaced by a `fuzzing = []` feature not reachable from `full`/`default`; plan 12 asserts that. |
| 04 | Mcp-Name matrix contradiction | ACCEPTED | Resolved in favor of the EXISTING implementation and Phase-112 D-05 (locked, non-overridable): presence always, empty value for name-less methods, value cross-checked only for name-bearing. Written into plan 01's checkpoint as `## Mcp-Name Header Rule` and implemented identically in plans 02, 04 and 05, with tests on both sides. |
| 04 | `Reject` cannot carry `-32022`'s `supported` | ACCEPTED | Widened to `Reject { code, message, data }`. |
| 04 | unknown-method 404 may miss raw unknown methods | ACCEPTED | The status mapper now runs at the RAW level on the code about to be emitted, with the id taken from the raw body; `v2_unknown_method_404` drives it with a method that cannot typed-parse. |
| 04 | brittle grep gates and exact test counts | ACCEPTED | Comment-filtered greps; counts relaxed to "at least N". |
| 05 | transport-mode propagation undefined | ACCEPTED (claim partially refined) | Additive defaulted `Transport::set_negotiated_protocol_version` + `supports_negotiated_protocol_version`, overridden by `StreamableHttpTransport` to delegate to its EXISTING inherent setter and field. A build-time warning fires when v2 is selected on a transport with no wire representation. |
| 05 | `assert_capability` fails locally on v2 | ACCEPTED | Era-aware: on v2 with no observed capabilities it returns `Ok`; an EXPLICIT `server_discover()` populates them and enforcement resumes. Distinguished in-doc from D-08's prohibited era auto-probe. |
| 05 | Unicode live test needs plan 04's decoder | ACCEPTED | `depends_on` now `["113-02", "113-04"]`; plan 05 moves to wave 3. |
| 05 | `with_protocol_version` on non-HTTP transports / arbitrary versions | ACCEPTED | Validated against `SUPPORTED_PROTOCOL_VERSIONS`; inert selections warn. |
| 06 | `""` principal makes tokens transferable between anonymous callers | ACCEPTED (hardened, residual accepted) | On a server WITH an auth provider, an unauthenticated request is now REFUSED MRTR entirely. Only a server with NO auth provider uses the named `ANONYMOUS_PRINCIPAL`, where there are no principals to separate; T-113-22 upgraded from `accept` to `mitigate` with TTL + originating-request binding as residual controls. |
| 06 | no limits on `inputResponses` | ACCEPTED | Five bounds (`MAX_REQUEST_STATE_LEN`, count, per-entry bytes, total bytes, depth) enforced at parse time in plan 02, before any clone; live test for the count bound. |
| 06 | env-controlled integration tests are order-dependent | ACCEPTED | Plan 06 Task 3 uses `with_request_state_key`; `grep -c PMCP_REQUEST_STATE_KEY tests/v2_mrtr_ingress.rs` must be `0`. |
| 07 | live fixtures cannot emit `input_required` before plan 09 | ACCEPTED | Plan 07 is now mock-transport only; the live client↔server MRTR tests moved to plan 11 (which depends on 07 and 09). Plan 07 moves to wave 4. |
| 07 | direct handler calls bypass approval/result-review | ACCEPTED | The fold routes through the same internal dispatch helpers the v1 host path uses; dedicated tests for a rejecting approval hook and a running result review. |
| 07 | handlers invoked before proving all kinds fulfillable | ACCEPTED | `preflight_input_requests` runs first; a test asserts ZERO invocations when the SECOND entry is unfulfillable. |
| 07 | handler errors swallowed silently | ACCEPTED | Every `CannotFulfil` path emits `tracing::warn!` with the entry key and reason. |
| 07 | `Error::Protocol.code` is `ErrorCode` | ACCEPTED | Constructors wrap the value. |
| 08 | id invariant conflicts with v1 replay | ACCEPTED | Scoped to DIRECT responses; `v1_replayed_event_retains_original_id` asserts historical identity separately as correct behavior. |
| 08 | `tools/list` does not test the cached-envelope bug | ACCEPTED | `cached_payload_is_reenveloped_with_live_id` forces genuine payload reuse. |
| 08 | `debug_assert!` weaker than a structural design | ACCEPTED | `envelope_for_live_request(payload, live_id)` takes payload and id as SEPARATE arguments, so a whole cached envelope cannot be passed through; the `debug_assert!` is retained as belt-and-braces. |
| 08 | event-store spy needed | ACCEPTED | `SpyEventStore` asserts zero stores/replays on v2 and NON-zero on v1 so the assertion is not vacuous. |
| 08 | brittle `LAST_EVENT_ID` count | ACCEPTED | Replaced with an "at least 1, retained for v1" assertion plus behavioral tests. |
| 09 | capability check considers only presence | ACCEPTED | Submode-aware: form vs URL elicitation, sampling sub-capabilities, with a fallback note to re-check the URL sub-field against the final schema. |
| 09 | `_meta` shapes inconsistent across result types | ACCEPTED | One `result_meta_object_mut` helper operating on the SERIALIZED JSON; non-object `_meta` is replaced with a warning. |
| 09 | capability validation after minting | ACCEPTED | Precheck runs first; a test asserts a failed precheck mints ZERO tokens. |
| 09 | `into_meta_entry` declared infallible | ACCEPTED | Returns `Result`. |
| 10 | registry keyed by JSON-RPC id alone | ACCEPTED | `ListenKey { principal, request_id }`; `two_callers_same_request_id_do_not_cross` proves it live. |
| 10 | unbounded channel | ACCEPTED | Bounded `mpsc::channel(LISTEN_CHANNEL_CAPACITY)` with a documented disconnect-on-overflow policy; `unbounded_channel` count must be `0`. |
| 10 | disconnect-safe unregistering not designed | ACCEPTED | RAII `ListenGuard` holding the key, the registry `Arc` and the semaphore permit, moved into the stream future; `disconnect_releases_registry_slot` proves the reclaim live. |
| 10 | `AuthContext` path to the listen registry unexplained | ACCEPTED | Threaded from the same POST-path resolution that feeds `handle_request_with_context`, with a per-connection fallback when no auth provider is configured. |
| 10 | instance-local streams behind a load balancer | ACCEPTED | Documented as single-instance/sticky-only, with a build-time `tracing::warn!` when subscription capabilities are advertised. Cross-instance backends explicitly out of scope. |
| 10 | SSE frames bypass envelope/header helpers | ACCEPTED | The ack and terminal result go through `inject_v2_result_envelope` + `own_reserved_result_fields` + `apply_v2_outbound_headers`. |
| 10 | `src/server/core.rs` and client files missing from `files_modified` | ACCEPTED | `core.rs` and `mod.rs` added to plan 10; the client work is plan 13. |
| 10 | no graceful-closure trigger defined | ACCEPTED | Three named triggers (client disconnect, shutdown signal, overflow); only shutdown sends the terminal result. |
| 10 | field types must be locked from the final schema | ACCEPTED | Taken from plan 01's `## Verdict` record, with a delta note if they differ. |
| 11 | tests derived from the July-24 research table | ACCEPTED | `113-CONFORMANCE-MANIFEST.md` generated from the PINNED commit with a must-be-empty `## Unmapped` section and a mechanical id→test-name derivation; a `## Research-Table Delta` section records discrepancies. |
| 11 | example demonstrates only the server side | ACCEPTED | `examples/s48_v2_mrtr_client.rs` added, proving automatic fulfilment AND the no-handler typed-error path. |
| 11 | the 20k fuzz run is not in any gate | ACCEPTED | Moved into plan 12's phase gate as a recorded row. |
| 11 | `timeout` is environment-sensitive | ACCEPTED | Guarded with a `command -v timeout` branch and a background-spawn fallback. |
| 12 | dev-dep-free command described vaguely | ACCEPTED | Two exact commands recorded verbatim with an explanation of what each catches. |
| 12 | verification used bare `cargo` | ACCEPTED | Every row and the `<verify>` block use `$(rustup which cargo)`. |
| 12 | no coverage measurement | ACCEPTED | Per-file line coverage recorded for the five new/changed files against the 80% target. |
| 12 | requirements marked complete prematurely | ACCEPTED | Task 3 Step 1 gates every flip on a `PUBLISHED-*` verdict AND an empty `## Unmapped` manifest section, with a partial marker and a blocked-phase report as the documented alternative. |
| 12 | one final plan makes recovery cumbersome | REJECTED | Plan 12 has three independent tasks with separate verifies; splitting further would add a wave for no parallelism (it is the terminal wave by construction). |

### Gemini recommendations

| # | Recommendation | Verdict | Disposition |
|---|---|---|---|
| 1 | Strict unknown-key vs tampered-token layout adherence | ACCEPTED | Already the design; strengthened by plan 03's key-id collision policy and `Expired(Continuation)`. |
| 2 | Verify `serverInfo` nesting inside `_meta` in wave 4 | ACCEPTED and strengthened | Plan 09 Task 3 does the move AND makes the key server-OWNED (overwritten), which Gemini did not ask for but Codex correctly did. |
| 3 | Auto-derive `_meta.clientCapabilities` from `ClientHostRegistry` | ACCEPTED | Already in plan 05; now paired with the era-aware `assert_capability` fix that makes v2 calls possible at all. |
| 4 | Preserve W3C trace context across MRTR rounds | ACCEPTED | Plan 05's `_meta` helper MERGES rather than replaces, with a unit test asserting a caller `traceparent` survives; plan 07's resend reuses the same params object. |

### Deferred

| Item | Reason |
|---|---|
| Cross-instance notification backend for `subscriptions/listen` | Out of scope this phase; the opt-in is documented and warned as single-instance/sticky-only (plan 10). Revisit if an enterprise user needs `listChanged` behind a load balancer. |
| Typed `HandlerOutcome::InputRequired` replacing the `_meta` signal | Requires a public handler-trait signature change = MAJOR semver break. Recorded on `MRTR_SIGNAL_META_KEY` as the right shape for a future 3.0. |
| PDMT todo generation per implementation plan | **Documented deviation from a CLAUDE.md MANDATORY directive.** `pdmt` is not installed in this workspace; the substitute is the GSD per-task `<acceptance_criteria>`/`<verify>` structure, which carries the same quality-gate/success-criteria/validation-command shape PDMT emits. Residual risk: no machine-generated todo determinism. Recorded verbatim in plan 01 Section C. |
| PMAT quality-proxy MCP writes | **Documented deviation from a CLAUDE.md MANDATORY directive.** Requires a long-running `pmat mcp-server --enable-quality-proxy` process outside a plan executor's control, so writes did not go through `quality_proxy`. Substitutes: `make quality-gate` (chains `pmat comply check`), the PR-blocking PMAT `quality-gate` CI job, and plan 12 Task 2's non-skippable `pmat analyze complexity --max-cognitive 25` run. Residual risk: quality issues are caught after the write rather than at it. Recorded verbatim in plan 01 Section C. |
| External `../provable-contracts` YAML updates | Checkout VERIFIED absent from this workspace. Plan 01 records the environment; plan 12 runs the in-repo `pmat comply check --path .` instead, and updates the YAML only if the checkout has appeared. |

### Wave restructuring

Two ordering bugs the review exposed forced a recompute (6 waves → 7):

| Plan | Was | Now | Reason |
|---|---|---|---|
| 05 | wave 2, `depends_on: [02]` | wave 3, `depends_on: [02, 04]` | The non-ASCII `Mcp-Name` live test needs plan 04's server-side sentinel decoder. |
| 06 | wave 3 | wave 3 (unchanged) | — |
| 07 | wave 3, `depends_on: [02, 05]` | wave 4, same deps | Follows 05's move; live MRTR tests relocated to plan 11. |
| 08 | wave 4 | wave 4 (unchanged) | — |
| 09 | wave 4 | wave 4 (unchanged) | — |
| 10 | wave 5, `depends_on: [04, 08]` | wave 5, `depends_on: [04, 08, 09]` | The listen stream must route its ack/result through plan 09's envelope helpers. |
| 11 | wave 5 | wave 5 (unchanged) | — |
| 13 | — | wave 6, `depends_on: [05, 10]` | New plan closing HTTP-04's client half. |
| 12 | wave 6 | wave 7, `depends_on` gains `113-13` | Terminal gate follows the new wave 6. |

Same-wave `files_modified` overlap was re-checked after the move: no two plans in any wave share a file.

---

# Cross-AI Plan Review — Phase 113, ROUND 2 (gap-closure plans 113-17/18/19)

**Reviewed:** 2026-07-26T23:05:22Z
**Reviewers:** codex (`codex-cli 0.144.5`), gemini (via the `agy` Antigravity shim at `~/.local/bin/gemini`)
**Plans reviewed:** `113-17-PLAN.md`, `113-18-PLAN.md`, `113-19-PLAN.md` (commit `188b5cf3`)
**Not reviewed:** 113-01..113-16 — already shipped; 113-01..113-12 covered by Round 1 above.
**Claude CLI skipped:** this review was orchestrated from Claude Code, so a `claude` reviewer would not be independent.

## Round 2 — Codex Review

## Summary

These plans are not ready to execute unchanged. Plan 113-17’s core parser change genuinely closes the reproduced newline-carrying `current_event.data` growth path. Plan 113-18 preserves the single-lock duplicate check, generation-scoped teardown, and principal-scoped keys—but its `sender.is_closed()` predicate does not detect the real ungraceful-network-disconnect window it claims to fix. Plan 113-19 improves the fuzz seam, but its load-bearing negative control cannot fail because the new post-drain check remains active. The set therefore does not yet prove HTTP-04 gap closure.

## Strengths

- 113-17 correctly removes the peer-controlled newline escape and accounts for `current_event.data`, directly addressing the 899,999-byte and 1,000,000-byte reproductions.

- Both incremental consumers continue polling the parser latch and terminate their streams. The bypass tripwire preventing `feed_complete_body` use in incremental paths is valuable.

- 113-18’s proposed `HashMap::Entry` replacement remains inside one `entries.write()` guard. That preserves 113-14’s check-then-act fix.

- The replacement carries a different generation, while unchanged [`take_entry`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/subscriptions.rs:734) compares generations before removal. A stale guard or overflow teardown therefore cannot remove the successor.

- Reclaim is structurally principal-scoped because `ListenKey` contains both `principal` and `request_id`. Principal B cannot reach principal A’s occupied entry through the proposed lookup.

- 113-19 correctly gates `decode_listen_chunks_for_fuzz` out of the normal public surface and exposes retained-byte observations to the fuzzer.

## Concerns

- **HIGH — 113-18 Task 1 fixes an artificial receiver-drop state, not the stated ungraceful disconnect.** The production SSE stream owns `(receiver, guard)` together in [`streamable_http_server.rs`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:2970). A dead remote TCP client does not call `mpsc::Receiver::close()`; the receiver remains alive until Hyper detects the disconnect and drops the response body. Consequently, `sender.is_closed()` remains false during the same keep-alive/retransmit window that caused the regression. When the receiver is finally dropped, the guard follows during the same stream-state teardown. The proposed unit test deliberately dropping the receiver while retaining the guard ([113-18 Task 1](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-18-PLAN.md:214)) creates a state that does not represent the 15-second network-stale period. This is the same narrowing failure shape as 113-15.

  The built-in client also generates a fresh UUID for every listen request in [`Client::subscriptions_listen`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs:3953). The plans need to distinguish “pmcp client reconnect using a fresh ID” from “third-party client takeover using the same ID.” The server cannot safely distinguish a live co-tenant duplicate from an undetected dead connection using `sender.is_closed()` alone.

- **HIGH — both SSE negative controls are internally impossible as written.** 113-17 changes the post-drain check to `buffered_bytes() > max_buffer_size` ([Task 1.3](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-17-PLAN.md:176)), but its negative control disables only the pre-check and expects both new tests to fail ([acceptance criterion](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-17-PLAN.md:278)). The newline flood will still be caught by the post-check and remain green; only the complete-line test should fail.

  The same defect is worse in 113-19: restoring only `&& !data.contains('\n')` leaves the total-retention post-check active, so every post-chunk `peak_buffered_bytes` sample remains within the bound. A complete oversized event also resets `current_event` before sampling. Therefore the promised fuzz crash at [113-19 lines 226–231](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-19-PLAN.md:226) cannot occur on that assertion.

- **HIGH — `feed_complete_body`’s safety precondition is false at both intended call sites.** The plan says these bodies were collected “under a separate size cap,” but both paths use unbounded `response.collect()` in [`streamable_http.rs`](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/streamable_http.rs:497) and [the POST path](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/streamable_http.rs:1011). The GET path is nominally SSE and can itself be long-lived. Thus the proposed bypass merely moves the unboundedness before the parser, while the threat model incorrectly marks the boundary as capped. Making the bypass public also exposes an easy-to-misuse unbounded parser entry point.

- **HIGH — the fixed 16 MiB `HttpTransport` ceiling is not justified and the media claim is false.** MCP image and audio content are unconstrained base64 strings. Base64 expands data by roughly 4/3, so a 12 MiB binary becomes 16 MiB before JSON, SSE prefixes, MIME type, and envelope overhead. It will therefore exceed the proposed ceiling. Large text, resources, or `structuredContent` can also legitimately exceed it. The plan offers no configuration escape hatch and no boundary compatibility test, while claiming image/audio are “unaffected” ([113-17 threat T-113-84](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-17-PLAN.md:454)).

- **MEDIUM — the proposed pre-check is a transport-chunk limit, not purely an in-progress-event limit.** Checking `buffered_bytes() + data.len()` before parsing rejects a chunk containing many individually small, complete events whenever the chunk total exceeds the limit. For example, a 300 KiB body frame containing hundreds of complete 1 KiB events is rejected by the 256 KiB listen parser even though retained state never needed to exceed one small event. Messages claiming “one undelivered event” exceeded the bound would be inaccurate. This makes behavior depend on Hyper’s chunking.

- **MEDIUM — 113-18 Task 2’s semaphore-pruning tests do not reproduce WR-06.** Both proposed tests perform the rejection first and release incumbent guards afterward ([lines 338–343](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-18-PLAN.md:338)). The current code already passes those sequences because the later guard drop sees the rejected registration’s temporary `Arc` gone and prunes normally. WR-06 requires the incumbent guard to drop while the rejecting registration still retains its semaphore reference. A test hook, barrier, or extracted cleanup helper is needed to make removal of either new `prune_principal` call fail a test.

- **MEDIUM — the cross-principal test is weaker than its security claim.** Dropping Bob’s receiver before Alice reclaims her key and then asserting only `live_streams() == 2` would not prove Bob’s exact entry survived; replacing the wrong entry could preserve the same count. The algorithm itself is safe because the full `ListenKey` is used, but the proposed acceptance test does not pin that property.

- **LOW — “total retained bytes” and “strictly newer generation” overstate the implementation.** `buffered_bytes()` excludes `current_event.id`, `current_event.event`, and `last_event_id`; an `id:` value is retained twice. These fields cannot grow without bound, but the stated exact ceiling is false. Separately, generation values are allocated before taking the entry lock, so a delayed registration can insert a numerically older—but still unique—generation after a newer one. Uniqueness is sufficient for teardown safety; “strictly newer than the incumbent” is not guaranteed.

## Suggestions

1. Replace the synthetic reconnect test with a real HTTP test that opens an SSE response, abruptly kills/drops the socket, and immediately retries the same ID before the keep-alive interval. Require that test to fail before the fix.

2. Resolve the fundamental reconnect ambiguity explicitly:

   - guarantee fresh request IDs on reconnect and document that behavior;
   - or return a retryable duplicate/stale-state error until teardown;
   - or introduce an authenticated takeover token/generation.

   Automatic same-ID takeover of a connection the server still considers live cannot remain safe for co-tenants without additional proof.

3. Correct the negative controls: disable both the unconditional pre-check and total post-check to recreate GAP-A. Seed the fuzz run with a deterministic repeated `data: A\n` corpus rather than expecting an empty-corpus 2,000-run campaign to synthesize it.

4. Make `feed_complete_body` `pub(crate)` and enforce a real, separately configured collection cap at every caller. The SSE GET path should preferably be migrated to incremental parsing instead of `collect()`.

5. Make the general HTTP SSE ceiling configurable. Add tests immediately below, at, and above the limit, including a base64 payload whose raw size demonstrates the 4/3 expansion.

6. Add barrier-controlled semaphore cleanup tests whose negative control removes each rejection-path prune call and fails.

7. Keep Bob’s receiver live in the cross-principal test; after Alice reclaims, assert both exact keys remain present and both receivers still receive fan-out.

## Risk Assessment

**HIGH.** The parser implementation direction is substantially better, and the registry replacement mechanics do not reopen 113-14’s atomicity or generation protections. However, the liveness predicate does not represent remote connection liveness, two required negative controls cannot fail, the whole-body bypass relies on nonexistent caps, and the new 16 MiB compatibility limit is unconfigurable and demonstrably breaks legitimate media payloads.

---

## Round 2 — Gemini Review

# Cross-AI Plan Review: Phase 113 GAP CLOSURE (round 2)

**Reviewed Plans:**
- [`113-17-PLAN.md`](#) — GAP-A Closure: Unbounded SSE Client Buffer & Multi-Accumulator Bound Enforcement
- [`113-18-PLAN.md`](#) — GAP-B & GAP-C Closure: Liveness-Aware Reconnect & Rejection-Path Semaphore Pruning
- [`113-19-PLAN.md`](#) — GAP-D & GAP-E Closure: Fuzz Seam Encapsulation, Peak-Retention Invariant & Phase Gate

---

## 1. Summary

**Verdict: The plans fully close the verified gap contract.** 

Unlike the previous partial fix in Plan 113-15 (which left newline-carrying streams unbounded), **Plan 113-17** introduces an **unconditional in-flight buffer bound** across both parser accumulators (`buffer` and `current_event.data`), closing GAP-A completely. **Plan 113-18** solves the reconnect regression (GAP-B) by making `ListenRegistry::register` liveness-aware under a single write lock, maintaining generation-scoped teardown and principal isolation without reopening race conditions. **Plan 113-19** gates the fuzz seam out of the public API (GAP-D) and updates the fuzz target to assert peak memory retention (GAP-E). Crucially, all three plans mandate **negative control verifications** to prove their acceptance criteria are genuinely falsifiable.

---

## 2. Detailed Evaluation of Core Review Questions

### 1. Does Plan 113-17 actually close the SSE buffer gap (GAP-A)?
**Yes.** Plan 113-15 failed because `feed` skipped its bound check whenever `buffer` or `data` contained a newline (`\n`), allowing multi-line `data:` frames to bypass the limit entirely.
- **Unconditional Pre-check:** Plan 113-17 Task 1 removes the `!data.contains('\n')` escape condition completely. The pre-check evaluates `self.buffered_bytes().saturating_add(data.len()) > self.max_buffer_size` unconditionally.
- **Multi-Accumulator Scope:** `buffered_bytes()` is defined as `buffer.len() + current_event.data.len()`. This accounts for both unterminated lines in `buffer` and lines already pushed into `current_event.data` (which only clear on a blank line dispatch). 
- **Exact Accumulator Property:** In SSE, fields like `id:` and `event:` overwrite previous values in `EventBuilder`, while `data:` is the *only* field that appends across lines. Measuring `buffer.len() + current_event.data.len()` accurately bounds all memory retention.
- **Explicit Bypass:** Collected-body transport call sites are explicitly migrated to `feed_complete_body`, while a tripwire assertion guarantees no incremental feeder touches the bypass.

### 2. Does the GAP-B fix risk reopening 113-14's concurrency & teardown guarantees?
**No.** The liveness-aware reclaim path in Plan 113-18 Task 1 is carefully structured:
- **Atomicity (No Check-Then-Act Window):** The check (`occupied.get().sender.is_closed()`) and replacement (`occupied.insert(...)`) occur within the **same single `entries.write()` guard**. Two concurrent `register` calls for the same key process sequentially; the second caller will observe a live sender (`is_closed() == false`) and receive `Err(DuplicateSubscriptionId)`.
- **Generation Scoping Intact:** The reclaimed entry receives a fresh generation (`fresh_gen`). When the stale connection's `ListenGuard` drops later, `remove_entry` compares `entry.generation == guard.generation`. Since `fresh_gen > guard.generation`, the guard drop will safely ignore the successor entry.

### 3. Is the reclaim path safe across principals?
**Yes.** In `ListenRegistry`, streams are keyed by `ListenKey { principal: String, request_id: RequestId }`.
- Reclaim is strictly scoped to matching `ListenKey` instances.
- Principal B (`principal = "bob"`) cannot match or inspect Principal A's entry (`principal = "alice"`).
- Task 1 includes a dedicated cross-principal isolation test proving that when Alice's receiver is closed, Bob registering with the same `request_id` creates a separate stream (`live_streams() == 2`), leaving Alice's slot untouched until Alice herself reconnects.

### 4. Is the 16 MiB `MAX_HTTP_SSE_BUFFERED_BYTES` ceiling appropriate?
**Yes.** General HTTP SSE connections (`connect_sse`) carry arbitrary JSON-RPC responses (including base64-encoded tool outputs and images). 
- Setting `MAX_HTTP_SSE_BUFFERED_BYTES = 16 MiB` accommodates large single JSON-RPC payloads (e.g., ~10–12 MB raw image tool outputs) while bounding heap growth over long-lived streams.
- The 256 KiB bound (`MAX_LISTEN_LINE_BYTES`) remains active for `subscriptions/listen` streams.
- **Payload Impact:** Only legitimate single JSON-RPC SSE frames exceeding 16 MiB will be truncated/ended. In MCP, bulk binary transfers exceeding 16 MiB should use resource streams or out-of-band links rather than inline SSE JSON-RPC text events.

### 5. Are acceptance criteria genuinely falsifiable?
**Yes.** All three plans include mandatory **Negative Controls**:
- **Plan 113-17:** Forcing the pre-check condition to `false` causes the new newline-carrying flood test to fail; restoring it passes.
- **Plan 113-18:** Reverting the `sender.is_closed()` check causes the reconnect test to fail with `Err(DuplicateSubscriptionId)`.
- **Plan 113-19:** Reverting 113-17's pre-check causes `cargo fuzz` to crash on the new peak-retention assertion (`peak <= max_buffer_size`).

---

## 3. Strengths

- **Unconditional In-Flight Bound:** Plan 113-17 eliminates the flawed newline escape clause in `SseParser::feed`, bounding both `buffer` and `current_event.data`.
- **Single-Lock Reclaim Execution:** Plan 113-18 performs entry liveness check and replacement inside a single `entries.write()` guard, preventing check-then-act race windows.
- **Lock Hierarchy Compliance:** Plan 113-18 Task 2 explicitly releases the `entries` lock before acquiring `per_principal` locks during semaphore pruning, avoiding lock inversion deadlocks.
- **Fuzz Seam Encapsulation:** Plan 113-19 gates `decode_listen_chunks_for_fuzz` behind `#[cfg(any(feature = "fuzzing", test))]`, keeping non-standard test methods off the published public API surface.
- **Falsifiable Verification:** Every plan includes negative controls that explicitly test for failure under broken logic before confirming pass states.

---

## 4. Concerns & Risks

### [MEDIUM] Rapid Reconnects May Hit Principal Stream Rate Limits
- **Location:** Plan 113-18, Task 1 & Threat Model (T-113-90)
- **Detail:** When an ungraceful disconnect occurs, `register` reclaims the `ListenKey` if `sender.is_closed()` is `true`. However, the stale connection's `ListenGuard` still holds its per-principal semaphore permit until the keep-alive timeout (~15s) fires. The reconnecting client consumes a *second* per-principal permit.
- **Impact:** If a single principal reconnects more than 4 times (`MAX_LISTEN_STREAMS_PER_PRINCIPAL = 4`) within a single ~15s keep-alive window, subsequent reconnect attempts will fail with `ListenRejection::PerPrincipalLimit`.
- **Mitigation/Status:** The plan acknowledges this in T-113-90. The rejection returns `RATE_LIMITED` (`-32005`), which is retryable (unlike the previous non-retryable `-32600` HTTP 400). This is acceptable for Phase 113 gap closure, but worth noting for operational visibility.

### [LOW] Error Message Substring Assertion Coupling
- **Location:** Plan 113-18, Task 1
- **Detail:** `tests/v2_subscriptions.rs:731` asserts `message.contains("already open for this subscription id")`. If `ListenRejection::DuplicateSubscriptionId::message()` is reworded without updating this test assertion simultaneously, `cargo test` will fail.
- **Mitigation:** Task 1 explicitly highlights this coupling and requires updating both in lockstep if reworded.

---

## 5. Actionable Suggestions

1. **Document Rapid Reconnect Limit Behavior in Code Comments:**
   Add a short comment in `ListenRegistry::register` explaining that while a closed incumbent key is reclaimed immediately, its semaphore permit is retained by the background `ListenGuard` until dropped. This clarifies why rapid reconnect bursts (> 4 in 15s) yield `-32005 RATE_LIMITED`.

2. **Verify Memory Drop Outside Lock:**
   In Plan 113-18 Task 1, ensure the stale `ListenEntry` returned from `occupied.insert(...)` is explicitly dropped *after* `entries` (the `RwLockWriteGuard`) is dropped to avoid holding the registry lock during `Sender` cleanup.

---

## 6. Risk Assessment

| Phase / Plan | Risk Level | Rationale |
|--------------|------------|-----------|
| **113-17** | **LOW** | Clean, unconditional bound enforcement over both accumulators; whole-body bypass is restricted to expected non-incremental callers with explicit tripwire assertions. |
| **113-18** | **LOW** | Liveness check is scoped within a single write guard; generation checks prevent late guard drop races; cross-principal key isolation is maintained by key structure. |
| **113-19** | **LOW** | Encapsulates fuzz seam behind Cargo features and provides a meaningful peak memory retention assertion for `cargo fuzz`. |

**Overall Phase Risk:** **LOW**. Plans 113-17, 113-18, and 113-19 directly resolve all items in the gap contract without introducing regressions or altering public API contracts.

---

## Round 2 — Consensus Summary

The two reviewers **diverged sharply**, which is itself the most important output of this
round. Gemini returned `LOW` risk and "the plans fully close the verified gap contract."
Codex returned `HIGH` risk and "these plans are not ready to execute unchanged," with four
HIGH findings. Because a divergence this wide cannot be averaged, the orchestrator
independently checked Codex's four HIGH findings against the live source tree before
recording this summary. **All four hold.** Gemini's review appears to have reasoned from the
plan text alone; Codex read the implementation.

### Agreed Strengths (both reviewers)

- 113-17's parser change is the right direction: dropping the `!data.contains('\n')` escape
  and accounting for `current_event.data` genuinely addresses the reproduced 899,999-byte and
  1,000,000-byte measurements. This is a real improvement over 113-15.
- 113-18's `HashMap::Entry` replacement stays inside one `entries.write()` guard, so
  113-14's check-then-act fix is **not** reopened.
- The replacement carries a different generation and `take_entry` still compares generations
  before removal, so a stale guard drop or overflow teardown **cannot** remove the successor.
  113-14's generation-scoped teardown is **not** reopened.
- Reclaim is structurally principal-scoped: `ListenKey` contains both `principal` and
  `request_id`, so principal B cannot reach principal A's entry.
- 113-19 correctly gates `decode_listen_chunks_for_fuzz` off the default public surface.

### Agreed Concerns (both reviewers, differing severity)

- **Error-message / test-assertion coupling in 113-18 Task 1.** Both flagged it; both agree
  the plan already names the coupling. LOW.

### Divergent Views — adjudicated against the source

| # | Codex (HIGH) | Gemini | Orchestrator verification | Verdict |
|---|---|---|---|---|
| 1 | `sender.is_closed()` does not detect a dead remote client — the production stream owns `(receiver, guard)` in one `stream::unfold` state tuple, so the receiver stays alive for the whole keep-alive window and drops only at the same moment the guard drops | "No — reclaim is safe and correct" | **CONFIRMED.** `src/server/streamable_http_server.rs:2973` — `futures_util::stream::unfold((receiver, guard), ...)`. There is no production path where the receiver is closed while the guard is alive. During the ~15 s window the regression describes, `is_closed()` is `false`, so the reconnect is still refused. | **Codex correct.** Same narrowing shape as 113-15. |
| 2 | Both SSE negative controls cannot fail — 113-17 item 3 also changes the POST-drain check to `buffered_bytes()`, but the negative control disables only the pre-check | "All three plans include mandatory negative controls… genuinely falsifiable" | **CONFIRMED.** 113-17 Task 1 item 3 makes the post-drain check `buffered_bytes() > max_buffer_size`. With only the pre-check disabled, the newline-flood test still latches on the post-check and stays green. Only the oversized-complete-line test would fail (its `current_event` is dispatched and reset before the post-check samples). | **Codex correct.** Gemini's claim is the reverse of the truth. |
| 3 | `feed_complete_body`'s "already collected under a separate size cap" precondition is false at both intended call sites | not addressed | **CONFIRMED.** `src/shared/streamable_http.rs:499` and `:1013` both use bare `response.collect()` with no cap. (`read_body_with_limit` / `max_request_bytes` exists on the *server* path, not these client paths.) The bypass moves the unboundedness upstream of the parser rather than removing it. | **Codex correct.** |
| 4 | 16 MiB ceiling breaks legitimate media: base64 expands ~4/3, so a 12 MiB binary is already 16 MiB before JSON/SSE/MIME overhead; no configuration escape hatch | "accommodates ~10–12 MB raw image tool outputs" | **Arithmetic favors Codex.** Both cite the same 4/3 expansion; 12 MiB × 4/3 = 16 MiB lands *at* the ceiling, so it fails once envelope overhead is added. Gemini treats the same boundary as comfortable headroom. Neither the plan nor Gemini offers a config escape hatch. | **Codex correct** on the risk; the ceiling needs to be configurable and boundary-tested. |

### Codex-only findings not independently re-verified (MEDIUM/LOW, recorded as-is)

- **MEDIUM** — the pre-check is a *transport-chunk* limit, not an in-progress-*event* limit:
  a chunk carrying many small complete events is rejected on chunk total, making behavior
  depend on Hyper's chunking.
- **MEDIUM** — 113-18 Task 2's semaphore tests reject first and release incumbent guards
  afterward, a sequence the current code already passes; they would not fail if the new
  `prune_principal` calls were removed, so they do not actually pin WR-06.
- **MEDIUM** — the cross-principal test asserts only `live_streams() == 2`, which does not
  prove Bob's *exact* entry survived.
- **LOW** — `buffered_bytes()` excludes `current_event.id`/`event` and `last_event_id`, so
  "total retained bytes" overstates the bound; and generations are allocated before the entry
  lock is taken, so "strictly newer" is not guaranteed (uniqueness is, which is what teardown
  safety actually needs).

### Bottom line

Finding 1 is the phase-blocking one. The verifier's own reproduction of the GAP-B regression
(`drop(receiver)` while holding the guard) creates a state that the production code path
cannot produce, which means **both the reported regression's severity and 113-18's fix for it
rest on the same artificial premise**. That needs to be resolved before 113-18 executes —
either with a real socket-level reconnect test that fails before the fix, or by resolving the
reconnect ambiguity a different way (guaranteed-fresh request ids on reconnect, a retryable
stale-state error, or an authenticated takeover token).

Findings 2, 3 and 4 are correctable within the existing plan structure and do not change its
shape.
