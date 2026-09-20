# Pitfalls Research

**Domain:** Dual-version protocol support (MCP 2025-11-25 + 2026-07-28 "v2") in an existing, published Rust SDK (pmcp 2.17.0) with downstream consumers
**Researched:** 2026-07-22
**Confidence:** MEDIUM (spec is RC as of research date, finalizes 2026-07-28 — 6 days out; core code claims verified by grep, spec-behavior claims from RC blog + secondary sources + internal impact memory)

> Scope note: these are pitfalls specific to *adding a second protocol version to THIS codebase*, not generic MCP advice. Each maps to one of the six planned phases (impact memory `project_mcp_spec_2026_07_28_impact`): (1) version plumbing, (2) stateless HTTP + multi-round-trip elicitation, (3) Tasks migration, (4) JSON Schema 2020-12 + caching, (5) auth SEPs, (6) conformance.

## Critical Pitfalls

### Pitfall 1: Flipping `LATEST_PROTOCOL_VERSION` to `2026-07-28`

**What goes wrong:**
`src/types/protocol/version.rs` hardcodes `LATEST_PROTOCOL_VERSION = "2025-11-25"` and `negotiate_protocol_version()` **returns `LATEST` for any unrecognized client version**. If v2 support is added by bumping this constant, every unknown/legacy client silently gets negotiated to `2026-07-28` semantics (stateless, no `initialize`, tasks reshaped), breaking the exact backward-compat the dual-version stack is supposed to preserve. Downstream code that reads `LATEST_PROTOCOL_VERSION` as "the version I speak" (cargo-pmcp scaffolds, book/course badges, test fixtures asserting `"2025-11-25"`, the doctest at `src/lib.rs:255` `assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25")`) drifts or hard-fails.

**Why it happens:**
"Latest = newest we support" feels right, and the fallback-to-LATEST branch makes the bump look harmless in unit tests. The negotiation function's failure mode (return LATEST) is the opposite of what stateless v2 needs (a conservative default to v1).

**How to avoid:**
Add `2026-07-28` to `SUPPORTED_PROTOCOL_VERSIONS` but **keep `LATEST_PROTOCOL_VERSION = "2025-11-25"` and make v2 opt-in** (builder method / feature flag / explicit lifecycle), exactly as the official Rust SDK does with `serve_with_lifecycle` + `ProtocolVersion::V_2026_07_28` (opt-in, not default) and TypeScript does with explicit `createMcpHandler` opt-in. Change the negotiation fallback for the *stateless header path* to default to v1 when no `MCP-Protocol-Version` header is present. Update the `SUPPORTED_PROTOCOL_VERSIONS.len()` doctest/tests deliberately (they will break — that's the tripwire working).

**Warning signs:**
Existing 2025-11-25 integration tests start expecting `server/discover` instead of `initialize`; the `latest_version_is_2025_11_25` unit test needs "fixing"; downstream crates fail to compile against a fixture asserting the old string.

**Phase to address:** Phase 1 (version plumbing) — the very first design decision.

---

### Pitfall 2: Two negotiation mechanisms colliding (echo-back vs per-request header)

**What goes wrong:**
Today negotiation is one-shot at `initialize` (`negotiate_protocol_version` echoes the highest common version, stored on the session). v2 has **no `initialize`** — protocol version arrives on a required `MCP-Protocol-Version` header *per request*, with clientInfo/caps in per-request `_meta`. If both systems run without a clear precedence rule, a request can be dispatched under v1 assumptions (session-stored version) while carrying v2 headers, or vice versa. Handlers then read the wrong protocol context.

**Why it happens:**
The session-stored negotiated version and the per-request header are two sources of truth. Retrofitting v2 tends to bolt the header check onto the existing session path rather than making per-request the authority.

**How to avoid:**
Make the dispatch path compute an explicit `ProtocolContext` per request: (v2) header + `_meta` clientInfo → authoritative; (v1) fall back to session-negotiated version. Never let a session-stored version override an explicit per-request header. Thread this context through `RequestHandlerExtra` so handlers branch on a resolved value, not on ambient session state.

**Warning signs:**
Handlers calling `session.protocol_version()` on a request that has no session (stateless); intermittent behavior differences between the first request and subsequent requests on the same connection.

**Phase to address:** Phase 1 (plumbing) and Phase 2 (stateless HTTP).

---

### Pitfall 3: Session-identity assumptions leaking across the stateless boundary (the pmcp.run discovery-cache id bug is a preview)

**What goes wrong:**
v2 removes `Mcp-Session-Id`. pmcp has session assumptions baked into `src/shared/{session,streamable_http,event_store}.rs` and downstream (`pmcp-run/amplify/.../mcp-proxy-rust`). The **known, documented** pmcp.run discovery-cache bug (`pmcp_run_proxy_discovery_cache_id_bug`) is exactly this failure class: state keyed/correlated by session identity replays a stale JSON-RPC `id`, producing `invalid type: string, expected i64`. Under stateless v2, *every* per-request identity that used to come from the session (owner binding, event-store resumption, caches, request→response `id` correlation) loses its anchor. Any code that keys state by session id will mis-correlate or leak across callers.

**Why it happens:**
Session id was a convenient, always-present correlation key. Stateless mode removes it silently — code compiles and mostly works until two callers with different id shapes share a keyspace.

**How to avoid:**
Audit every session-keyed store/cache before enabling v2. Enforce the invariant *response `id` MUST equal request `id`, always re-derived from the live request* (never replayed from cache) — this holds in both versions and is the root fix for the known proxy bug. In v2 mode, derive caller identity from per-request `_meta` clientInfo / OAuth `sub`, not session id. Type response ids as untagged `RequestId` (String|Number), not `i64`, as defense-in-depth.

**Warning signs:**
`expected i64` / id-mismatch errors; discovery methods (`tools/list`, `resources/list`) failing while `initialize`/`server/discover` succeeds; per-server intermittency on a 5-minute (cache-TTL) cadence.

**Phase to address:** Phase 2 (stateless streamable-HTTP). Add a regression test reproducing the discovery-cache id class.

---

### Pitfall 4: Tasks owner-binding collapses when `session_id` disappears

**What goes wrong:**
`pmcp-tasks` resolves task ownership via `resolve_owner_id(subject, client_id, session_id)` (`crates/pmcp-tasks/src/security.rs:150`, `router.rs:289`) — **session id is the last-resort fallback**, and there's a test `router.resolve_owner(None, None, Some("session-1"))` proving unauthenticated servers rely on it. In stateless v2, `session_id` is gone. Owner binding then collapses to `subject`/`client_id` only; for a task server without OAuth, every request resolves to the *same* (or `None`) owner → tasks become cross-owner readable or un-ownable. This is a **security regression**, and it hits published consumers (pmcp.run's 3 hand-rolled task servers, DynamoDB/Redis-backed stores).

**Why it happens:**
The session-id fallback was a pragmatic "works without auth" convenience in v1.0. It's invisible in tests that always pass a session id.

**How to avoid:**
For v2, replace the session-id fallback with a per-request stable client identity from `_meta` clientInfo (or require OAuth `sub` and fail closed if absent). Keep the v1 session-id path intact for v1 clients. Add a security test: stateless request with no OAuth and no session must NOT resolve two different callers to the same owner.

**Warning signs:**
Two anonymous stateless clients seeing each other's tasks; `resolve_owner(None, None, None)` returning a usable owner; owner-isolation security tests only covering the session-id case.

**Phase to address:** Phase 3 (Tasks extension migration).

---

### Pitfall 5: Reshaping the Tasks API in place breaks published `pmcp-tasks` + `tasks/list` consumers

**What goes wrong:**
v2 **removes `tasks/list`** (implemented at `crates/pmcp-tasks/src/constants.rs` `METHOD_TASKS_LIST`), **adds `tasks/update`**, and moves to server-directed task creation. If the `TaskRouter` drops `tasks/list` when v2 is negotiated but the change is done in-place on the shared router, v1 clients still calling `tasks/list` get "method not found." Conversely the `-32002` "task not completed / pending" code (FROZEN with locking tests at `src/server/core.rs:1145`, `task_dispatch_tests.rs`) may be affected by the v2 error-code changes (see Pitfall 6).

**Why it happens:**
The router is version-agnostic (`serde_json::Value` boundary), so it's tempting to mutate one dispatch table for both versions.

**How to avoid:**
Keep `tasks/list` served for v1-negotiated requests; gate its removal on the resolved protocol context. Add `tasks/update` and server-directed creation as additive methods. Because `pmcp-tasks` is a separate crate with independent semver, the v2 reshape can be a **major bump of `pmcp-tasks`** without forcing a `pmcp` major (see Pitfall 8) — but only if core `pmcp` keeps the `TaskRouter` trait boundary stable. Document the migration for pmcp.run's three task servers before publishing.

**Warning signs:**
v1 conformance fixtures for `tasks/list` failing; the frozen `-32002` tests needing edits; pmcp.run task servers erroring on `tasks/list`.

**Phase to address:** Phase 3 (Tasks migration).

---

### Pitfall 6: The `-32002` → `-32602` error-code change collides with standard JSON-RPC `InvalidParams`

**What goes wrong:**
The impact memory records a v2 error-code rename `-32002` → `-32602`. But in THIS codebase `-32602` is already `InvalidParams` (`src/types/protocol/mod.rs:135 InvalidParams = -32602`), and `-32002` is the **FROZEN** "task not completed / pending" code with explicit locking tests (`pending_tasks_result_preserves_minus_32002`). If v2 remaps the task-pending signal onto `-32602`, task-pending becomes **indistinguishable from invalid-params** on the wire — clients (and pmcp's own `wait_for_task`) that branch on the code will treat a still-running task as a client error, or vice versa.

**Why it happens:**
Blindly applying a changelog "rename" without noticing the target code is already occupied by a semantically different, standardized error.

**How to avoid:**
**Verify this against the authoritative changelog** (`modelcontextprotocol.io/specification/draft/changelog`) before touching frozen codes — the memory claim is a secondary-source paraphrase and may misstate direction or scope. If the spec genuinely reuses `-32602`, branch error interpretation on the negotiated protocol version so the frozen v1 `-32002` semantics are preserved and v2's `-32602` is disambiguated by context. Do NOT silently change the frozen constant.

**Warning signs:**
`wait_for_task` treating a pending task as a hard error; the `-32002` freeze tests being "updated" without a spec citation; a single code carrying two meanings.

**Phase to address:** Phase 1 (error-code plumbing) with a Phase 3 (Tasks) cross-check. **Flagged: needs changelog verification — LOW confidence on the exact rename.**

---

### Pitfall 7: JSON Schema 2020-12 — `structuredContent = any JSON value` breaks the 2.15 object-shaped bridge

**What goes wrong:**
The v2.15 structured-output bridge (`src/server/output_validation.rs`, dispatcher dual-emit) assumes `structuredContent` is an **object** derived from an object `outputSchema`. v2 + JSON Schema 2020-12 permits `structuredContent` to be **any JSON value** (scalar, array, null). Cached validators built with `jsonschema::validator_for` (dep `jsonschema = "0.46"`) auto-detect the `$schema` dialect from the document — a tool declaring an older draft (`draft-07`/`2019-09`) while the client expects 2020-12 will validate under the wrong dialect (2020-12 renamed `items`→`prefixItems`, added `$dynamicRef`, changed `$recursiveRef`), so schemas silently mis-validate. A scalar `structuredContent` may be rejected by object-assuming validation or mis-wrapped by the dispatcher.

**Why it happens:**
The 2.15 bridge was designed against object outputSchemas (the only shape `#[mcp_tool]`/`TypedToolWithOutput` derived). "Any JSON value" and dialect-strictness are new degrees of freedom.

**How to avoid:**
Confirm `jsonschema` 0.46 is invoked with the 2020-12 draft explicitly for v2 tools (don't rely on `$schema` auto-detect). Loosen the dispatcher/validation to accept non-object `structuredContent`. Add property tests over scalar/array/null structured content. Keep v1 object-only behavior for v1-negotiated tools.

**Warning signs:**
Valid scalar/array tool outputs rejected as "schema violation"; validation passing on malformed 2020-12 schemas (wrong dialect); `prefixItems`/`$dynamicRef` keywords ignored.

**Phase to address:** Phase 4 (JSON Schema 2020-12 + caching).

---

### Pitfall 8: Sleepwalking into a `pmcp` 3.0 when the milestone is meant to be additive

**What goes wrong:**
The dual-version stack is *intended* to be additive (stays 2.x minor per CLAUDE.md release rules: new features = minor, breaking = major). It quietly becomes a **breaking 3.0** the moment you: flip `LATEST_PROTOCOL_VERSION` (Pitfall 1 — changes `negotiate` output = behavioral break), remove `tasks/list` from core dispatch unconditionally, change the serde shape of any public type (e.g., putting per-request clientInfo into `RequestHandlerExtra` by changing an existing field's type), change owner-binding fallback signatures, or change default server behavior to stateless.

**Why it happens:**
Each individual change feels small; the cumulative default-behavior shift is a major break. During a 2.x window there's pressure to "just make it work."

**How to avoid:**
Adopt the isolation pattern that already worked for `pmcp-tasks`: v2 behind opt-in builder methods + a feature flag, all new types additive, existing public types unchanged, `LATEST` pinned. `pmcp-tasks` itself MAY take a major bump (separate crate) without forcing core `pmcp` major. Write down the "what forces 3.0" list at phase start and treat any item on it as a milestone-level decision, not an implementation detail.

**Warning signs:**
`cargo public-api` / semver-checks flagging removed or changed items; existing downstream crates (toolkit, agent, team-servers) needing code changes to compile against the new `pmcp`; default `Server::run` behavior changing for existing users.

**Phase to address:** Phase 1 (set the additive constraint); re-checked every phase.

---

### Pitfall 9: Building hard against the RC while the final spec is days away

**What goes wrong:**
Research date is 2026-07-22; the spec **finalizes 2026-07-28**. The RC is feature-complete but the SDK-betas post explicitly warns "Public APIs may still change between the beta and the stable releases, so pin exact versions." Hard-coding RC-only quirks (exact header casing, provisional `requestState` shape for multi-round-trip elicitation, `InputRequiredResult` fields, `ttlMs`/`cacheScope` hint names) risks a rewrite when final lands.

**Why it happens:**
Eagerness to ship against the RC; assuming "release candidate" means frozen.

**How to avoid:**
Sequence the milestone so wire-exact details (Phase 2 elicitation `requestState`, Phase 4 caching hints, Phase 5 auth SEP specifics) land **after** 2026-07-28 final publication; do the version-plumbing scaffolding (Phase 1) first since it's the most stable. Diff the final changelog (`modelcontextprotocol.io/specification/draft/changelog`) against the RC before coding wire formats. Cross-reference the official Rust SDK's `ProtocolVersion::V_2026_07_28` constant/lifecycle once it stabilizes rather than inventing your own wire assumptions.

**Warning signs:**
Wire fixtures citing the RC blog rather than the final spec; the official SDK betas changing field names between beta and stable.

**Phase to address:** Milestone sequencing decision; concentrated in Phases 2, 4, 5.

---

### Pitfall 10: Breaking existing OAuth deployments with unconditional auth hardening

**What goes wrong:**
v2's six auth-hardening SEPs (RFC 9207 `iss` validation on authorization responses, DCR `application_type`) fail **closed** if enforced unconditionally. Existing pmcp OAuth deployments — Lambda `oauth_passthrough`, the Graph/M365 read-only example, load-testing OAuth (v1.5), the DNS-rebinding `AllowedOrigins::any()` proxy config — may not emit `iss` or set `application_type`, and would suddenly be rejected.

**Why it happens:**
Security SEPs are written as "MUST validate," and it's natural to gate them on nothing.

**How to avoid:**
Gate hardening on the negotiated protocol version: v2 clients get strict `iss`/DCR validation; v1 stays lenient. Provide a migration path and clear errors. Preserve the documented `stateless()` + `AllowedOrigins::any()` proxy exception (feedback memory `feedback_lambda_dns_rebinding`).

**Warning signs:**
Existing OAuth integration/e2e tests failing with `invalid issuer` / DCR errors; Lambda proxy deployments 401-ing after upgrade.

**Phase to address:** Phase 5 (auth SEPs).

---

### Pitfall 11: Conformance-suite integration masked by feature-flag unification

**What goes wrong:**
The Phase-109 conformance harness (in-memory + HTTP `ConformanceTarget`, fixture schema v2) is the intended alignment point. Two documented traps recur: (a) `cargo test --all-features` masks feature-flag gaps because the dev-dependency `pmcp` `full` feature unifies flags (Phase 109 gotcha — the `pmcp/http` gap that only surfaced under a dev-dep-free `--all-features`/publish build); (b) the in-memory `DuplexTransport` target **cannot** exercise the stateless HTTP header path (`MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name`, session-id absence), so header/statelessness conformance must run against the HTTP target only. Additionally the official conformance suite may itself be RC-versioned and change before final.

**Why it happens:**
`--all-features` green feels like proof; in-memory transport is the easy default for conformance fixtures.

**How to avoid:**
Verify v2 conformance with a **dev-dependency-free** build (absolute rustup `cargo build --all-features`), not just `cargo test`. Route all header/session-absence assertions through the HTTP `ConformanceTarget`. Pin the official conformance suite to a specific commit and re-pin after the 2026-07-28 final. Keep v1 conformance fixtures running alongside v2 (dual-version = dual conformance).

**Warning signs:**
Conformance passing under `cargo test --all-features` but failing on a fresh publish build; header tests "passing" against in-memory transport that never sends headers.

**Phase to address:** Phase 6 (conformance).

---

### Pitfall 12: Deleting/breaking deprecated Roots/Sampling/Logging too early

**What goes wrong:**
v2 marks Roots, Sampling, Logging **deprecated but advisory-only** — they MUST keep working in v2 and every spec version published within 12 months. pmcp shipped `sampling/createMessage` (with tools/tool_choice), `roots/list`, and the client host surface in v2.16 (Phase 106), and `pmcp-agent`'s provider-direct design depends on them as compat-window features. Removing or breaking these to "clean up for v2" strands `pmcp-agent`, `pmcp-team-servers`, and any client relying on sampling/roots.

**Why it happens:**
"Deprecated" reads as "remove now" instead of "annotate, keep for 12 months."

**How to avoid:**
Zero removal work this milestone (matches PROJECT.md non-goal). Add deprecation annotations only; keep full runtime behavior. Validate `pmcp-agent` sampling-with-tools and `list_roots` still work under v2 negotiation.

**Warning signs:**
`#[deprecated]` on sampling/roots types turning into `#[cfg(not(v2))]` gating; agent/team examples failing under v2.

**Phase to address:** Phase 1 (annotate-only policy); verified in Phase 6.

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Flip `LATEST_PROTOCOL_VERSION` to 2026-07-28 to "turn on" v2 | v2 works instantly in demos | Silent v2 for legacy clients; forces `pmcp` 3.0; downstream fixture drift | **Never** during dual-version window |
| One shared dispatch table mutated for both versions | Less code | v1 methods (`tasks/list`) vanish for v1 clients; hard to reason about | Never — branch on resolved protocol context |
| Enforce auth SEPs unconditionally | Spec-compliant, simple | Breaks all existing OAuth deployments at once | Only if every deployment is known-v2 |
| Reuse in-memory conformance target for header tests | Fast to write | False-green on statelessness/header conformance | For non-transport semantic tests only |
| Pin task ownership to session-id fallback in v2 | Auth-free servers keep working | Cross-owner task leakage when session-id is gone | Never in v2 mode |
| Build wire formats against RC blog | Start early | Rewrite when 2026-07-28 final differs | Scaffolding only, not wire-exact fields |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| pmcp.run mcp-proxy-rust (discovery cache) | Replay cached JSON-RPC envelope incl. `id` | Re-write `id` to live request id on every cache hit; cache only `result` |
| `pmcp-tasks` (published, DynamoDB/Redis) | Remove `tasks/list` / change owner binding in place | Additive `tasks/update`; version-gated `tasks/list`; per-request identity for ownership; major-bump the crate, not core `pmcp` |
| cargo-pmcp scaffolds | Hardcoded protocol/version strings drift silently | Extend the existing `emitted_pmcp_version_matches_workspace_pin`-style tripwire to cover protocol-version strings |
| `jsonschema` 0.46 validators | Rely on `$schema` auto-detect for dialect | Invoke 2020-12 draft explicitly for v2 tools |
| Official Rust SDK (rmcp) interop | Invent own v2 wire assumptions | Cross-check `ProtocolVersion::V_2026_07_28` + `serve_with_lifecycle` opt-in model |
| Book/course/README protocol badges | Badge still says 2025-11-25 (or wrongly flipped) | Show dual-version; keep 2025-11-25 as default, 2026-07-28 as opt-in |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Per-request `_meta` clientInfo re-parsing on every stateless call | Higher per-request CPU vs one-shot `initialize` | Cache parsed clientInfo by a stable per-request key, not by session | High-QPS stateless serverless (Lambda) |
| Rebuilding JSON Schema validators per request | Validation latency spikes | Keep the existing cached-validator map; key by schema hash, dialect-aware | Many distinct outputSchemas under load |
| Discovery responses re-computed without caching in stateless mode | `tools/list` latency at scale | Cache `result` (not envelope) with `ttlMs`/`cacheScope` hints, re-wrap with live id | Large tool catalogs, many clients |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Session-id owner fallback under stateless v2 | Cross-owner task read/leak | Per-request OAuth `sub`/clientInfo identity; fail closed if absent |
| Unconditional `iss`/DCR hardening | Breaks existing deployments (availability) OR skipping it (spoofing) | Version-gate hardening; strict for v2, lenient for v1 |
| Reusing `-32602` for task-pending | Task-pending indistinguishable from InvalidParams; wrong client branching | Verify changelog; disambiguate by protocol version; don't touch frozen `-32002` v1 semantics |
| Trusting per-request `_meta` clientInfo without validation | Client-supplied identity spoofing (no handshake to anchor it) | Bind identity to OAuth token, not just `_meta` self-report |
| Dropping DNS-rebinding origin checks when going stateless | Rebinding attacks on proxy deployments | Keep documented `AllowedOrigins::any()` exception only for known proxy topology |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent version switch (no way to know which version negotiated) | Confusing behavior differences, hard to debug | Expose resolved protocol version on `RequestHandlerExtra` + logs |
| Docs show only one version | Users can't tell what's opt-in | Three-shapes docs (README + book + course) showing dual-version + `cargo pmcp` on-ramp (Phase 111 folded in) |
| Cryptic "method not found" on v1 `tasks/list` after upgrade | Broken task servers with no migration hint | Clear error naming the version mismatch + migration link |

## "Looks Done But Isn't" Checklist

- [ ] **Version negotiation:** Often missing the stateless header path — verify a request with no session and only `MCP-Protocol-Version` header resolves correctly, and that a *missing* header defaults to v1
- [ ] **Backward compat:** Often missing v1 regression — verify all 2025-11-25 conformance fixtures still pass with v2 enabled
- [ ] **Tasks ownership:** Often missing the no-session case — verify two anonymous stateless callers get distinct owners (or fail closed)
- [ ] **structuredContent:** Often missing non-object shapes — verify scalar/array/null validate and emit under 2020-12
- [ ] **Auth hardening:** Often missing the v1 leniency path — verify an existing OAuth deployment without `iss` still connects as v1
- [ ] **Error codes:** Often missing the frozen-`-32002` cross-check — verify `wait_for_task` still distinguishes pending from invalid-params
- [ ] **Conformance:** Often false-green under `cargo test --all-features` — verify with dev-dep-free publish build + HTTP transport target
- [ ] **Deprecated capabilities:** Often over-removed — verify sampling/roots/logging still work at runtime under v2
- [ ] **Downstream crates:** Often only pmcp tested — verify toolkit/agent/team-servers/cargo-pmcp compile + pass against the new pmcp
- [ ] **Semver:** Often an accidental break — run `cargo semver-checks`/`cargo public-api` to confirm it's a minor, not a 3.0

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| `LATEST` flipped, legacy clients broken | MEDIUM | Revert constant; move v2 to opt-in builder; patch release; fix drifted fixtures |
| Task owner leak under stateless | HIGH | Emergency patch: fail closed when no OAuth+no session; audit stores for cross-owner reads; notify pmcp.run |
| `-32602`/`-32002` collision shipped | MEDIUM | Version-gate error interpretation; restore frozen v1 semantics; add disambiguation test |
| structuredContent scalar rejected | LOW | Loosen validation to any-JSON for v2; add property tests |
| Auth hardening broke deployments | MEDIUM | Version-gate hardening; ship lenient-v1 path; document migration |
| Accidental 3.0 break shipped | HIGH | Yank/patch; re-isolate v2 behind opt-in; restore public types; re-run semver-checks |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Flip `LATEST` | Phase 1 | `LATEST_PROTOCOL_VERSION` unchanged; v2 opt-in; `latest_version_is_2025_11_25` test intact |
| 2. Dual negotiation collision | Phase 1 + 2 | Per-request `ProtocolContext` authoritative; missing header → v1 |
| 3. Session-identity leak | Phase 2 | Discovery-cache id regression test; response id == request id invariant |
| 4. Tasks owner collapse | Phase 3 | No-session owner-isolation security test |
| 5. Tasks API in-place reshape | Phase 3 | v1 `tasks/list` fixtures pass; `pmcp-tasks` major-bumped separately |
| 6. `-32002`/`-32602` collision | Phase 1 + 3 | Changelog-verified; `wait_for_task` pending-vs-invalid test |
| 7. structuredContent any-JSON | Phase 4 | Scalar/array/null property tests; explicit 2020-12 dialect |
| 8. Accidental 3.0 | Every phase | `cargo semver-checks` green as minor |
| 9. RC vs final drift | Sequencing (2,4,5 after final) | Wire fixtures cite final changelog, not RC blog |
| 10. Auth hardening breakage | Phase 5 | Existing OAuth e2e passes as v1; strict for v2 |
| 11. Conformance false-green | Phase 6 | Dev-dep-free build + HTTP target; dual-version fixtures |
| 12. Over-removing deprecated caps | Phase 1 (policy) + 6 | Sampling/roots/logging runtime tests under v2 |

## Sources

- MCP RC blog (2026-07-28 release candidate) — https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/ (feature-complete RC, only breaking change = deprecated caps; advisory-only deprecation)
- MCP SDK betas post — https://blog.modelcontextprotocol.io/posts/sdk-betas-2026-07-28/ ("pin exact versions; public APIs may still change"; Python answers `initialize` alongside `server/discover`; TS/Go opt-in; client fallback to `initialize`)
- Official Rust SDK (rmcp) — https://github.com/modelcontextprotocol/rust-sdk and Issue #212 (stateless streamable HTTP still open; `serve_with_lifecycle` + `ProtocolVersion::V_2026_07_28` opt-in exists) — https://github.com/modelcontextprotocol/rust-sdk/issues/212
- 4sysops overview — https://4sysops.com/archives/2026-07-28-model-context-protocol-mcp-stateless-multi-round-trip-routable-headers-authorization-hardening/
- Internal memory `project_mcp_spec_2026_07_28_impact` — repo-grounded impact map
- Internal memory `pmcp_run_proxy_discovery_cache_id_bug` — the session-statelessness collision preview
- Internal memory `project_structured_output_release` — v2.15 bridge assumptions + scaffold-pin tripwire
- Codebase grep (verified 2026-07-22): `src/types/protocol/version.rs`, `src/types/protocol/mod.rs:135` (`InvalidParams = -32602`), `src/server/core.rs:1145`/`task_dispatch_tests.rs` (frozen `-32002`), `crates/pmcp-tasks/src/security.rs:150`/`router.rs:289` (session-id owner fallback), `Cargo.toml:124` (`jsonschema = "0.46"`), `src/server/output_validation.rs`

---
*Pitfalls research for: MCP 2026-07-28 dual-version support in pmcp Rust SDK*
*Researched: 2026-07-22*
