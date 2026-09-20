# Project Research Summary

**Project:** PMCP SDK Extensions — v2.5 milestone
**Domain:** Dual-version MCP protocol SDK (Rust) — adding MCP spec 2026-07-28 ("v2") support to pmcp 2.17.0 alongside the existing 2025-11-25 stack
**Researched:** 2026-07-22
**Confidence:** HIGH (with one explicit LOW-confidence item flagged below)

## Executive Summary

This milestone adds a second, structurally different MCP protocol version — stateless,
handshake-free 2026-07-28 — to a Rust SDK that has spent five majors optimizing for the
session-based 2025-11-25 handshake. The good news, confirmed independently by all four
research passes: this is almost entirely a logic and typing problem, not a dependency
problem. Zero new runtime crates are required (`jsonschema` bumps 0.46→0.48 for Draft
2020-12; everything else — headers, `_meta` plumbing, `resultType`, Tasks reshape, auth
hardening — is additive Rust source against crates pmcp already vendors). The correct
architecture is also convergent across research: resolve a ProtocolEra once at transport
ingress (HTTP header / stdio handshake / `_meta`), thread it explicitly as a ProtocolContext
alongside auth_context, and era-gate roughly six decision points (session handling,
initialize vs. discover, tasks routing, error codes, result envelope, auth strictness) rather
than forking the transport or the dispatcher. v2's stateless branch is not new code — it is
the existing `stateless()` HTTP configuration wearing a new required-header contract.

The dominant risk is not "can we build it" but "will we silently break v1 or leak identity
across the stateless boundary while doing so." Four pitfalls carry outsized weight: (1) never
flip `LATEST_PROTOCOL_VERSION` — v2 must be strictly opt-in, mirroring the official Rust SDK's
`serve_with_lifecycle` pattern, or every unrecognized/legacy client silently negotiates into
v2 semantics; (2) task ownership binding (`resolve_owner(subject, client_id, session_id)`)
falls through to a session-id last resort that does not exist in stateless v2 — unaddressed,
this is a cross-caller task-leak security regression, not a cosmetic gap; (3) the whole
milestone must stay additive (2.x minor), because several individually-small changes
(flipping LATEST, mutating the shared Tasks dispatch table in place, changing
RequestHandlerExtra field types) cumulatively look like a 3.0; and (4) the spec is RC as of
this research (2026-07-22) with final publication six days out (2026-07-28) — wire-exact
details (error codes, requestState shape, caching-hint field names) should be sequenced
after final publication, while the version-plumbing scaffold (the most stable part) goes
first.

One specific unresolved conflict must be called out rather than silently resolved: research
disagrees on the `-32002`→`-32602` error-code rename. FEATURES.md (tracing the RC changelog)
calls it a stable rename and gives it LOW implementation complexity. PITFALLS.md, reading the
same codebase, flags it as a collision — `-32602` is already the standardized JSON-RPC
InvalidParams in `src/types/protocol/mod.rs:135`, while `-32002` is pmcp's own FROZEN
"task not completed/pending" code with explicit locking tests
(`pending_tasks_result_preserves_minus_32002`). PITFALLS.md rates its own claim LOW confidence
and explicitly says "verify against the authoritative changelog before touching frozen
codes." This is not decided here — it is a required verification gate for whichever phase
implements error-code plumbing, to be resolved against the final 2026-07-28 schema.json,
not the RC blog paraphrase either research file used as a secondary source.

## Key Findings

### Recommended Stack

The stack conclusion is unusually clean: no new runtime dependencies. The only
dependency-line change in the entire milestone is bumping the already-vendored, already
`default-features = false` `jsonschema` crate from 0.46.1 to 0.48.x for stable Draft 2020-12
support — a version bump, not a new crate, and it stays wasm-clean and SEP-2106-compliant
(no external `$ref` auto-dereference) with the current feature configuration. Every other
v2 surface — required headers, per-request `_meta` version/capability plumbing, `resultType`,
Tasks reshape, RFC 9207 `iss` validation, DCR `application_type` — maps onto crates pmcp
already uses (serde/serde_json, hyper/axum/tower, http, url, indexmap) as logic and type
changes, not new machinery. The one genuinely new tool (not crate) is the official
`@modelcontextprotocol/conformance` suite — a Node.js/TypeScript CI-only harness that
drives a pmcp HTTP server or client command; it needs Node.js LTS 22.x in CI but has zero
impact on the published crate's dependency tree or wasm builds. Explicit recommendation
against adding `oauth2`/`openidconnect` crates for the auth-hardening SEPs — pmcp's
hand-rolled, transport-agnostic OAuth flow already covers the needed surface, and those
crates would pull in reqwest and fight the existing wasm-clean posture.

**Core technologies (all pre-existing, one version bump):**
- `jsonschema` 0.46→0.48 (`default-features = false`, behind `validation`) — Draft 2020-12 validation for inputSchema/outputSchema/structuredContent; already SEP-2106-compliant via its existing no-`$ref`-resolution config
- `serde`/`serde_json` (unchanged) — new protocol types (InputRequiredResult, resultType, CacheableResult) are additive shapes on the existing backbone; structuredContent-as-any-JSON-value is a validation relaxation, not new machinery
- `hyper`/`axum`/`tower`/`tower-http` (unchanged) — the Streamable-HTTP stack already carries the header/session machinery; v2 reuses the existing stateless() branch
- `http` (unchanged) — new header constants (Mcp-Method, Mcp-Name) live in the existing http_constants.rs pattern
- Node.js LTS 22.x (CI-only, new) — required to run the official conformance suite; zero Rust dependency-tree impact

### Expected Features

FEATURES.md frames this precisely as "table stakes for a legitimate v2 conformance claim"
versus "differentiators that make pmcp the reference dual-version SDK" versus explicit
anti-features for a dual-version (not cutover) milestone. The single most important
structural fact: nearly every table-stakes item is version-gated off one keystone — the
version negotiation engine — which is why it must be Phase 1 regardless of which other work
parallelizes.

**Must have (table stakes for v2 conformance):**
- Per-request `_meta` version+capabilities plumbing (protocolVersion/clientCapabilities/clientInfo) — the stateless replacement for the initialize handshake
- `server/discover` RPC — stateless capability discovery
- Drop initialize/notifications-initialized and Mcp-Session-Id on the v2 path only (v1 path unchanged)
- Required headers Mcp-Method/Mcp-Name (plus continuing MCP-Protocol-Version)
- MRTR: InputRequiredResult/requestState/inputResponses — inverts, not extends, the existing Phase-106 server-initiated sampling/elicitation/roots surface
- resultType envelope discriminator (complete/input_required/task), defaulting missing to complete for backcompat
- Tasks-as-extension reshape: tasks/get, tasks/update, tasks/cancel; drop tasks/list routing on v2; unsolicited task handles; extensions capability map
- JSON Schema 2020-12 + unrestricted (any-JSON) structuredContent
- Caching hints ttlMs/cacheScope on the 5 list/read results
- Error-code rename -32002 → -32602 — see Open Verification Item below; do not implement until reconciled against final schema
- Six auth-hardening SEPs (RFC 9207 iss, DCR application_type, issuer-keyed credentials, three clarifications)
- Conformance against the official suite

**Should have (differentiators):**
- A genuinely single-point, per-request version negotiation engine serving both spec versions from one binary — the milestone's headline differentiator
- W3C/OpenTelemetry trace-context keys in `_meta` (near-free given the existing flatten map)
- server/discover as a STDIO backcompat probe
- tasks/update client-input-into-running-task (no v1 analog; aligns with the pmcp-agent/teams surface)

**Defer / explicit anti-features for this milestone:**
- Hard cutover to v2 (dropping 2025-11-25) — ecosystem is still overwhelmingly v1
- Hard-coding the new -3202x/-32602 codes before the final schema lands
- Rewriting pmcp-tasks rather than reshaping its wire API (storage backends/state machine survive)
- Removing Roots/Sampling/Logging — deprecated with a 12-month advisory window, not removed
- subscriptions/listen unified stream is a P2 scope question (not in the current six-phase cut, but is a v2 transport change — flag for explicit go/no-go, don't silently include or exclude)

### Architecture Approach

The convergent architectural idea across ARCHITECTURE.md and PITFALLS.md: detect protocol
era once at ingress, carry it as an explicit value, era-gate a small number of decision
points. Concretely, a new ProtocolContext { era, negotiated_version, client_info,
client_capabilities } is built at the transport boundary (HTTP header parse or stdio/_meta
parse) and threaded next to auth_context into ServerCore::handle_request_internal and then
into RequestHandlerExtra via typed accessors. v1 requests keep hitting the existing
initialize/session-stateful code paths unmodified; v2 requests route through the already
existing stateless() branch (session_id_generator: None) with one new gate: skip session
resolution entirely and never emit Mcp-Session-Id. The serde_json::Value boundary in
TaskRouter — proven during the v1.0→v1.2 Tasks evolution — is explicitly named as the
mechanism that lets the Tasks wire API reshape (drop tasks/list, add tasks/update,
server-directed creation) without touching pmcp core types or the DynamoDB/Redis storage
investment. The Phase-109 RequestMeta.other flatten map is likewise already the exact shape
v2 needs for io.modelcontextprotocol/clientInfo and W3C trace keys — it needs typed
accessors, not a new type.

**Major components (new vs. modified):**
1. ProtocolContext (new value type) — era/version/clientInfo/capabilities resolved once at ingress, threaded through dispatch instead of read ambiently from session state
2. version.rs negotiation (modified) — add "2026-07-28" to supported versions with an explicit protocol_era() classifier; LATEST_PROTOCOL_VERSION stays pinned to "2025-11-25" (v2 is opt-in)
3. Streamable-HTTP server/client (modified) — era-gate session resolution onto the existing stateless branch; add/enforce the three required v2 headers; suppress Mcp-Session-Id on v2 responses
4. ServerCore dispatch (modified) — era-gate the initialize arm and not-initialized guard; add server/discover and tasks/update arms; version-gate error-code emission
5. TaskRouter/pmcp-tasks (modified, additive) — handle_tasks_update added; handle_tasks_list kept but era-gated off for v2; owner-binding reworked to not depend on session id for v2 callers
6. Schema validation (output_validation.rs/schema_utils.rs, modified) — accept Draft 2020-12 keywords and non-object top-level structuredContent; pin the draft explicitly rather than relying on $schema auto-detect

### Critical Pitfalls

1. **Flipping LATEST_PROTOCOL_VERSION to 2026-07-28** — the negotiation fallback returns LATEST for any unrecognized client version, so this single change silently upgrades every legacy/unknown client into v2 semantics. Keep LATEST pinned to 2025-11-25; make v2 strictly opt-in (builder method/feature flag), matching the official Rust SDK's `serve_with_lifecycle` pattern.
2. **Task owner-binding collapses when session_id disappears** — `resolve_owner(subject, client_id, session_id)` falls back to session id, which doesn't exist under stateless v2; unaddressed, unauthenticated v2 task servers resolve all callers to the same (or no) owner, a cross-caller data-leak security regression. Require OAuth `sub` or a stable per-request `_meta` identity for v2, and fail closed if absent.
3. **Two negotiation mechanisms colliding** — v1's session-stored, one-shot-at-initialize version vs. v2's required-per-request header must have an explicit precedence rule (per-request header authoritative when present, session fallback only for v1), or handlers read the wrong protocol context intermittently.
4. **Session-identity assumptions leaking across the stateless boundary** — the already-documented pmcp.run mcp-proxy-rust discovery-cache bug (replaying a cached JSON-RPC id across callers) is a live preview of this exact failure class; the general fix (response id always re-derived from the live request, never replayed) is the root prevention for both the known bug and future v2 session-keyed state.
5. **Sleepwalking into an accidental pmcp 3.0** — flipping LATEST, mutating the shared Tasks dispatch table in place instead of era-gating, or changing existing public type shapes each look like small changes but cumulatively constitute a breaking major; `cargo semver-checks`/`cargo public-api` should gate every phase, not just the last one.

## Open Verification Item (do not silently resolve)

The `-32002` → `-32602` error-code rename is in direct conflict across two research
passes and must be verified against the final 2026-07-28 schema before any code touches
the frozen constant.

- FEATURES.md, tracing the RC changelog's "Minor changes" section, states the rename is
  stable and rates it LOW implementation complexity — "version-gated constant, same as the
  broader error-code table."
- PITFALLS.md, reading the codebase directly, flags that `-32602` is already the
  standardized JSON-RPC InvalidParams (`src/types/protocol/mod.rs:135`), while `-32002` is
  pmcp's own FROZEN "task not completed/pending" code with explicit locking tests
  (`pending_tasks_result_preserves_minus_32002` in `task_dispatch_tests.rs` / `core.rs:1145`).
  If the v2 rename genuinely repoints resource-not-found semantics onto `-32602`, then a
  reused `-32602` becomes ambiguous between "invalid params" and (if naively mirrored onto
  the frozen task code) "task pending" — `wait_for_task` and any client branching on the
  code could misinterpret a running task as a hard client error. PITFALLS.md self-rates this
  claim LOW confidence and explicitly calls for changelog verification before touching
  the frozen constant.
- Both files agree the broader error-code table is still moving: the changelog's
  allocation policy already renumbered other new draft codes once post-RC
  (-3200x → -3202x), and the Tasks-extension reference page still shows an old code as of
  research date — independent evidence the schema hadn't fully settled as of 2026-07-22.

**Resolution path:** whichever phase implements error-code plumbing (Phase 1 per
ARCHITECTURE's build order, cross-checked in the Tasks phase) must (a) pull the exact
mapping from the final 2026-07-28 schema.json (not the RC blog paraphrase both
research passes partially relied on), (b) confirm whether the rename actually targets
`-32602` or a different code, (c) if it does target `-32602`, design version-gated
disambiguation that preserves the v1 frozen `-32002` "pending" semantics unchanged, and
(d) treat all error codes as a single centralized, version-gated constant table rather than
scattered literals, so this class of conflict is a one-place fix. Do not edit the frozen
`-32002` locking tests to make this "pass" without that verification.

## Implications for Roadmap

Based on combined research, the six-phase cut already named in PROJECT.md's Current
Milestone is corroborated end-to-end by ARCHITECTURE.md's dependency-driven build order and
PITFALLS.md's phase mapping — use it as the roadmap's phase spine, with the ordering and
gating below.

### Phase 1: Version Plumbing Spine
**Rationale:** Keystone — ARCHITECTURE.md and FEATURES.md both identify nearly every other
v2 behavior as version-gated off this. Nothing else can be era-aware until it lands.
**Delivers:** ProtocolContext value type; version.rs extended with 2026-07-28 +
protocol_era() (LATEST stays pinned to 2025-11-25, v2 opt-in); MCP_METHOD/MCP_NAME
header constants; ServerDiscover/TasksUpdate enum variant scaffolding (types only);
centralized version-gated error-code constant table (values TBD pending Open Verification
Item above).
**Addresses:** Per-request `_meta` plumbing, required headers, error-code rename (structure
only, not final values).
**Avoids:** Pitfall 1 (LATEST flip), Pitfall 2 (dual negotiation collision), sets the
additive/no-3.0 constraint referenced by Pitfall 8 for every later phase.

### Phase 2: Stateless Streamable-HTTP + Multi-Round-Trip Elicitation
**Rationale:** Directly depends on Phase 1's ProtocolContext; reuses pmcp's existing
stateless() branch rather than forking the transport (ARCHITECTURE Anti-Pattern 1).
**Delivers:** Era-gated session resolution (v2 skips session entirely, no Mcp-Session-Id
emitted); inbound v2 required-header enforcement + outbound emission; InputRequiredResult/
requestState/inputResponses MRTR flow, inverting the Phase-106 client host surface
(server-initiated sampling/elicitation/roots become client-fulfilled-on-retry).
**Addresses:** FEATURES table-stakes items "drop initialize/session-id on v2 path,"
"required headers," "MRTR."
**Avoids:** Pitfall 3 (session-identity leak across the stateless boundary — add a
regression test reproducing the pmcp.run discovery-cache id-replay bug class; enforce
response id == live request id, never replayed from cache).

### Phase 3: Tasks Extension Migration
**Rationale:** Needs the resolved era from Phase 1 and loosely needs Phase 2's statelessness
semantics for owner-binding; the serde_json::Value TaskRouter boundary is proven
insulation (already survived v1.0→v1.2), so this is an API reshape, not a rewrite.
**Delivers:** tasks/update handler; tasks/list kept but era-gated off for v2 only;
server-directed task creation; owner-binding reworked so v2 requires OAuth sub or a stable
per-request identity instead of falling through to a nonexistent session id.
**Addresses:** FEATURES "Tasks-as-extension migration," differentiator "tasks/update
client-input-into-running-task."
**Avoids:** Pitfall 4 (owner-binding collapse — CRITICAL, ship a no-session owner-isolation
security test before merge), Pitfall 5 (in-place reshape breaking v1 tasks/list
consumers — keep v1 fixtures green), and cross-checks Pitfall 6 (the -32002/-32602
Open Verification Item — this phase is where the frozen task-pending code most directly
collides with the error-code rename).

### Phase 4: JSON Schema 2020-12 + Structured-Output Bridge + Caching Hints
**Rationale:** Largely independent of Phases 2-3; only depends on Phase 1 for era-gating
validation strictness — can parallelize.
**Delivers:** jsonschema 0.46→0.48 bump with Draft::Draft202012 pinned explicitly (not
$schema auto-detect); relax structuredContent validation/dispatch to accept any JSON
value (scalar/array/null), keeping v1 object-only behavior for v1-negotiated tools;
composition-keyword (oneOf/anyOf/allOf) depth bounds per SEP-2106; additive
ttlMs/cacheScope fields on the 5 list/read results.
**Addresses:** FEATURES "JSON Schema 2020-12 + unrestricted structuredContent," "caching
hints."
**Avoids:** Pitfall 7 (object-shaped 2.15 bridge silently mis-validating scalar/array
structuredContent or the wrong schema dialect).

### Phase 5: Auth-Hardening SEPs
**Rationale:** Fully independent — can parallelize with Phases 2-4; only needs Phase 1's
era gate to avoid breaking v1 deployments.
**Delivers:** RFC 9207 iss validation in the OAuth callback parser (src/client/oauth.rs),
DCR application_type field, issuer-keyed credential storage, three clarifications — all as
hand-rolled additions to the existing OAuth stack (explicitly do NOT add oauth2/
openidconnect crates).
**Addresses:** FEATURES "six auth-hardening SEPs."
**Avoids:** Pitfall 10 (unconditional hardening 401-ing existing OAuth deployments — Lambda
oauth_passthrough, the Graph/M365 example, the documented AllowedOrigins::any() proxy
exception). Gate strict validation on v2, stay lenient on v1.

### Phase 6: Conformance Against the Official Suite
**Rationale:** Depends on all preceding phases; validates the whole dual-version claim —
must run last.
**Delivers:** Node.js LTS 22.x + @modelcontextprotocol/conformance in CI, driven against a
new dual-version pmcp server example binary; Phase-109 Rust conformance harness extended
with v2 fixtures for the fast offline inner loop; v1 fixtures kept running alongside v2
(dual-version = dual conformance, not a replacement).
**Addresses:** FEATURES "conformance against official v2 suite" (P1, required for the
conformance claim).
**Avoids:** Pitfall 11 (false-green under `cargo test --all-features` feature-flag
unification — verify with a dev-dependency-free `cargo build --all-features`; route all
header/session-absence assertions through the HTTP ConformanceTarget, since the in-memory
DuplexTransport cannot exercise the stateless header path at all), Pitfall 12 (verify
Roots/Sampling/Logging still work at runtime under v2 negotiation — deprecated-but-advisory,
not removed).

### Phase Ordering Rationale

- Phase 1 must be first and alone — ARCHITECTURE.md's dependency diagram shows Phases
  2, 3, 4, 5 all gated on the version-plumbing spine; none of the era-gating patterns exist
  before it lands.
- Phases 4 and 5 parallelize with 2/3 — both are architecturally near-independent
  (JSON Schema touches only the structured-output bridge; auth touches only src/client/oauth.rs
  and src/server/auth/), so a team could work them concurrently with the stateless-HTTP and
  Tasks phases once Phase 1 lands.
- Phase 3 has a loose dependency on Phase 2 — not because of routing, but because
  correct owner-binding for stateless v2 tasks needs the same "derive identity from
  per-request _meta/OAuth, not session" pattern Phase 2 establishes for MRTR/general
  statelessness; sequencing them close together reduces the risk of two independently-invented
  identity schemes.
- Phase 6 must be last — it is a verification phase over the union of all prior work by
  construction (it runs the official suite against whatever the dual-version binary actually
  does).
- The error-code Open Verification Item cuts across Phases 1 and 3 — do not treat it as
  fully resolved by "put it in Phase 1"; the Tasks phase is where the frozen -32002
  semantics are most exposed, so a cross-check belongs in both.

### Research Flags

Phases likely needing deeper research during planning (`/gsd:plan-phase --research-phase <N>`):
- **Phase 1 (Version Plumbing):** the error-code table specifically — MUST be re-verified
  against the final 2026-07-28 schema.json (publishes six days after this research), not
  the RC blog. This is the single highest-value re-research item in the whole milestone.
- **Phase 2 (Stateless HTTP + MRTR):** requestState wire shape and MRTR security posture
  (HMAC/AEAD, principal binding, TTL, anti-replay) are RC-era and explicitly flagged by
  PITFALLS.md as likely to shift before final; also the official Rust SDK's
  ProtocolVersion::V_2026_07_28 / serve_with_lifecycle opt-in model should be
  cross-checked once stable, to avoid inventing incompatible wire assumptions.
- **Phase 3 (Tasks):** the v1 5-state → v2 status-enum mapping table and the exact
  server-directed-creation trigger shape need spec-final confirmation.
- **Phase 5 (Auth SEPs):** DCR deprecation → Client ID Metadata Documents (PR #2858) is
  still additive/optional this milestone but worth a fresh look at final-spec text before
  implementation, since it's the one auth item still evolving upstream.

Phases with standard, well-documented patterns (research-phase likely skippable):
- **Phase 4 (JSON Schema 2020-12):** jsonschema 0.48's Draft 2020-12 support and API
  (options().with_draft(Draft::Draft202012).build()) are confirmed stable via docs.rs;
  this is a mechanical bump + explicit-draft-pin change.
- **Phase 6 (Conformance):** the official suite's CI integration contract (env vars,
  mode/url/command, conformance-baseline.yml) is documented and stable; the Phase-109
  harness pattern to extend already exists in-repo.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Every conclusion traced to either the authoritative MCP changelog/RC post or direct codebase inspection (Cargo.toml, http_constants.rs, oauth.rs); the "zero new deps" finding is corroborated by all four passes' close reading of the existing dependency tree. |
| Features | HIGH for wire shapes, MEDIUM on final-vs-RC stability | Wire shapes traced to spec draft pages + changelog + individual SEP PRs (HIGH). The researcher's own confidence note: RC locked 2026-05-21, final publishes 2026-07-28 — six days after this research — and the changelog's error-code allocation was already renumbered once post-RC, with the Tasks-extension reference page still showing a stale code as of research date. |
| Architecture | HIGH | Every integration point (file/line references for version.rs, core.rs:1118, streamable_http_server.rs, tasks.rs:24, cancellation.rs:179) was read directly from the pmcp 2.17.0 tree, not inferred; v2 wire semantics cross-checked against the RC changelog. |
| Pitfalls | MEDIUM | Codebase claims (frozen -32002 tests, session-id owner fallback, LATEST_PROTOCOL_VERSION fallback behavior) verified by direct grep — HIGH within that scope. Spec-behavior claims (RC blog, secondary sources, internal impact memory) are explicitly self-rated MEDIUM by the researcher because the spec is still RC. |

**Overall confidence:** HIGH on architecture/mechanism, MEDIUM on exact wire-level details
that are still six days from finalizing as of this research date (2026-07-22 research,
2026-07-28 final spec).

### Gaps to Address

- **Error-code rename target (-32002 vs -32602)** — the explicit open conflict above;
  must be resolved against the final schema before Phase 1/3 implementation, not assumed
  from either research file.
- **RC-vs-final drift generally** — PITFALLS.md's Pitfall 9 recommends sequencing
  wire-exact phases (2, 4, 5) after 2026-07-28 final publication where practical, and diffing
  the final changelog against the RC before coding any wire format. The roadmap should treat
  the final-spec publication date as a milestone checkpoint, not just a research artifact
  date.
- **subscriptions/listen scope question** — FEATURES.md flags this as a differentiator
  that is NOT in the current six-phase cut but is nonetheless a v2 transport change; the
  roadmap should make an explicit go/no-go decision on it rather than letting it default in
  or out silently, since full v2 conformance may require it.
- **Owner-binding identity source for stateless v2 (no OAuth case)** — research is directionally
  clear (require OAuth sub or fail closed) but the exact behavior for a legitimately
  unauthenticated v2 task server (if any deployment needs that) is not fully specified; needs
  a security-review decision during Phase 3 planning, not just implementation.
- **Official conformance suite version pinning** — the suite itself may be RC-versioned and
  change before final; PITFALLS.md recommends pinning to a specific commit and re-pinning
  after 2026-07-28, which the roadmap's Phase 6 planning should encode as an explicit task.

## Sources

### Primary (HIGH confidence)
- https://modelcontextprotocol.io/specification/draft/changelog — authoritative 2026-07-28 change list (all SEPs referenced above)
- https://modelcontextprotocol.io/specification/draft/basic/patterns/mrtr — exact InputRequiredResult/inputRequests/inputResponses/requestState wire shapes
- https://github.com/modelcontextprotocol/conformance — official conformance suite mechanics (env vars, checks.json, conformance-baseline.yml)
- https://tasks.extensions.modelcontextprotocol.io/ and https://github.com/modelcontextprotocol/ext-tasks — Tasks extension wire shapes (noted: still showed a stale -32003 as of research date)
- https://docs.rs/jsonschema/0.48.5 — Draft 2020-12 support, default-features=false no-$ref-resolution behavior, explicit-draft API
- Direct codebase inspection: src/types/protocol/version.rs, src/types/protocol/mod.rs (RequestMeta, ClientRequest, InvalidParams = -32602 at line 135), src/server/core.rs (dispatch:1118, initialize:451, frozen -32002 test at :1145, structured bridge:686-703), src/server/streamable_http_server.rs, src/server/tasks.rs/task_dispatch.rs, src/shared/http_constants.rs, src/server/cancellation.rs:179, crates/pmcp-tasks/src/security.rs:150/router.rs:289, crates/pmcp-team-servers/src/conformance/runner.rs, root Cargo.toml

### Secondary (MEDIUM confidence)
- https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/ — RC post (locked 2026-05-21, final 2026-07-28); source of the "six auth SEPs" enumeration and the error-code rename claim disputed in this summary
- https://blog.modelcontextprotocol.io/posts/sdk-betas-2026-07-28/ — explicit "public APIs may still change between beta and stable, pin exact versions" warning
- https://modelcontextprotocol.io/community/sdk-tiers — SDK tiering context (search-surfaced, corroborated by RC post)
- https://github.com/modelcontextprotocol/rust-sdk (Issue #212) — official Rust SDK's serve_with_lifecycle + ProtocolVersion::V_2026_07_28 opt-in precedent
- Internal memory project_mcp_spec_2026_07_28_impact — repo-grounded impact map, source of the six-phase cut in PROJECT.md
- Internal memory pmcp_run_proxy_discovery_cache_id_bug — the documented preview of the session-statelessness identity-leak failure class

### Tertiary (LOW confidence — explicitly flagged, needs validation)
- The -32002 → -32602 error-code rename direction/scope — see Open Verification Item above. PITFALLS.md self-rates this LOW confidence; do not implement without re-verifying against the final 2026-07-28 schema.json.

---
*Research completed: 2026-07-22*
*Ready for roadmap: yes, with the error-code Open Verification Item flagged for Phase 1/3 planning*
