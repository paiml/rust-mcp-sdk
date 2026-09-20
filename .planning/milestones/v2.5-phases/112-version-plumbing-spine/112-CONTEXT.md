# Phase 112: Version Plumbing Spine - Context

**Gathered:** 2026-07-22
**Status:** Ready for planning

<domain>
## Phase Boundary

pmcp resolves a per-request protocol era (`ProtocolContext { era, negotiated_version, client_info, client_capabilities }`) once at transport ingress and threads it explicitly through dispatch, so one binary understands both MCP 2025-11-25 and 2026-07-28 clients. v2 is strictly opt-in; v1 behavior is byte-for-byte unchanged; the milestone stays an additive 2.x minor. Covers VERS-01..09: version plumbing + opt-in, per-request `_meta` self-description, `server/discover`, `extensions` capability map, required v2 HTTP headers, `resultType` envelope, W3C trace-context accessors, and the centralized version-gated error-code table (structure-first — v2 values ONLY from the final 2026-07-28 schema.json).

This is the keystone phase: nearly every other v2.5 behavior era-gates off it. It lands first and alone.

</domain>

<decisions>
## Implementation Decisions

### v2 opt-in API shape
- **D-01:** Opt-in is a **builder method, no cargo feature flag**. v2 support is core protocol code, always compiled; zero new deps means a feature gate buys nothing and would grow the already-large feature matrix (feature-unification false-greens have burned this repo twice).
- **D-02:** The opt-in takes the shape of an **explicit version accept-list** (e.g. `.with_supported_protocol_versions([...])`), not a boolean or max-version knob. This makes v1-only (default), dual, and a future v2-only all expressible with one API and directly supports the Phase 117 severability story.
- **D-03:** The version set accepts **typed constants** — new `pub const`s (e.g. a `2026-07-28` constant) alongside the existing `LATEST/DEFAULT/SUPPORTED_PROTOCOL_VERSIONS`, wrapped in the existing `ProtocolVersion` newtype (`src/types/protocol/mod.rs:28`). The `protocol_era()` classifier lives next to them. Arbitrary strings stay constructible for tests.
- **D-04:** A v2-style request hitting a **non-opted-in server behaves exactly as today**: unknown `_meta` keys are already passthrough (Phase-109 flatten map), the request flows down the v1 path and fails naturally (e.g. "server not initialized"). No era-detection code runs on non-opted-in servers — the strongest reading of VERS-02.

### Header enforcement (VERS-05)
- **D-05:** **Strict reject on the v2 path**: if a request self-identifies as v2 and `Mcp-Method`/`Mcp-Name` are missing, reject with a 4xx + structured JSON-RPC error. No lenient transition window, no configurable strictness knob — the official conformance suite (Phase 118) will test exactly this, and lenient-now means a breaking tightening later. v1 requests untouched.
- **D-06:** **`Mcp-Method` is cross-checked against the JSON-RPC body's `method`; mismatch = reject (fail closed).** A header/body desync is either a buggy client or a smuggling attempt (WAF sees one method, server executes another). This is the security-correct reading and cheap to test.

### resultType envelope (VERS-07)
- **D-07:** `resultType` is emitted on **v2 responses only** — v1 responses stay byte-identical to today (no fixture/snapshot churn, no risk to strict v1 consumers). The spec's absent-means-`complete` default covers v1 readers.
- **D-08:** `resultType` is **injected at the era-gated dispatch/serialization layer**, not added as a public field on Result structs. Handlers keep returning today's Result types unchanged (semver-safe, zero public-API churn). A typed `ResultType` enum exists internally so Phases 113 (`input_required`) and 114 (`task`) can set the discriminator.

### server/discover + stdio scope (VERS-04, VERS-01/03)
- **D-09:** `server/discover` is **auto-enabled by v2 opt-in** — no separate toggle. It's a core v2 method (the stateless replacement for initialize's capability exchange) and a read-only projection of already-computed ServerCore capabilities. One knob, not two.
- **D-10:** A v1-era request for `server/discover` gets standard **`-32601` method-not-found** — the method exists only in the v2 dispatch era. Clean era separation; same gate mechanism `tasks/list` will use in the opposite direction in Phase 114.
- **D-11:** **Era-detection is transport-agnostic**: `ProtocolContext` resolution reads per-request `_meta` on ALL transports (stdio included — same `RequestMeta` flatten map), while the `Mcp-Method`/`Mcp-Name` header requirements apply only where headers exist (HTTP). One era-resolution code path; stdio v2 comes essentially for free, letting stdio dev tools (mcp-tester, cargo pmcp dev loops) exercise v2 locally.

### Claude's Discretion
- W3C trace-context (VERS-09) accessor shape and propagation depth (typed accessors + propagation through dispatch required; integration depth with the `tracing`/observability module is Claude's call).
- `extensions` capability map (VERS-08) builder/plumbing details.
- Error-code table module placement and structure (constraints locked below: centralized, version-gated, values-from-final-schema-only, frozen `-32002` untouched).
- Exact builder method naming, `ProtocolContext` field/accessor naming, and where the era gate lives in `ServerCore` dispatch — subject to the locked research guidance (resolved once at ingress, threaded next to `auth_context`, typed accessors on `RequestHandlerExtra`, wired at BOTH `core.rs` and `server/mod.rs` dispatch sites + wasm mirror parity per the Phase 109-00 precedent).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone requirements & roadmap
- `.planning/ROADMAP.md` — v2.5 milestone section + Phase 112 detail (goal, VERS-01..09 mapping, success criteria, final-spec checkpoint note)
- `.planning/REQUIREMENTS.md` — VERS-01..09 full text; Out of Scope table (esp. "no hard-coding error codes before final schema"); Future Requirements (VERS-F1 discover-as-client-probe explicitly deferred)

### v2.5 research pack (2026-07-22, HIGH confidence)
- `.planning/research/SUMMARY.md` — architecture approach (ProtocolContext at ingress, era-gate decision points), critical pitfalls 1–5, and the **Open Verification Item**: the `-32002`→`-32602` rename MUST be re-verified against the final 2026-07-28 schema.json before touching the frozen `-32002` task-pending constant (`src/types/protocol/mod.rs:135` area; locking test `pending_tasks_result_preserves_minus_32002`). Do NOT silently resolve.
- `.planning/research/ARCHITECTURE.md` — component-level new-vs-modified breakdown and build order
- `.planning/research/PITFALLS.md` — LATEST-flip pitfall, dual-negotiation collision, accidental-3.0 pitfall (`cargo semver-checks`/`cargo public-api` should gate this phase)
- `.planning/research/FEATURES.md` — v2 feature inventory and complexity ratings
- `.planning/research/STACK.md` — zero-new-runtime-deps constraint

### Project context
- `.planning/PROJECT.md` — v2.5 milestone framing (dual-version stack, v2 strategic primary, non-goals)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `RequestMeta` `#[serde(flatten)]` namespaced map (Phase 109-00): already the exact shape v2 needs for `io.modelcontextprotocol/protocolVersion`/`clientInfo`/`clientCapabilities` and W3C trace keys — needs typed accessors, not a new type.
- `ProtocolVersion` newtype (`src/types/protocol/mod.rs:28`) + `LATEST/DEFAULT/SUPPORTED_PROTOCOL_VERSIONS` consts (re-exported via `src/types/mod.rs:32`): the typed-constant opt-in (D-03) extends these.
- `RequestHandlerExtra` (`src/server/cancellation.rs` native / `src/shared/cancellation.rs` wasm): the established per-request carrier — `ProtocolContext` accessors ride here, next to `auth_context` and `request_meta`.
- Existing `stateless()` streamable-HTTP branch (`session_id_generator: None`): the v2 request path Phase 113 era-gates onto; 112 only lays the era plumbing.
- `serde_json::Value` TaskRouter boundary: proven insulation (survived v1.0→v1.2) — 112's dispatch-arm scaffolding must not disturb it.

### Established Patterns
- Per-request fields wired at BOTH dispatch sites (`src/server/core.rs` AND `src/server/mod.rs`) + wasm mirror parity — the Phase 109-00 precedent for exactly this kind of plumbing.
- Era/version-gated dispatch arms — same mechanism serves `server/discover`-on-v2-only (D-10) and `tasks/list`-off-on-v2 (Phase 114).
- Centralized constants with locking tests — the frozen `-32002` has explicit locking tests; the new error-code table follows the same discipline (structure now, v2 values only from final schema.json).
- Quality gates: `make quality-gate` before commit; `cargo semver-checks`/`cargo public-api` to prove the phase stays additive (research Pitfall 5).

### Integration Points
- Transport ingress (streamable-HTTP header parse; stdio/_meta parse) → `ProtocolContext` construction → `ServerCore::handle_request_internal` → `RequestHandlerExtra` typed accessors.
- `src/shared/version.rs`-equivalent negotiation code: add 2026-07-28 + `protocol_era()` classifier; `LATEST_PROTOCOL_VERSION` stays "2025-11-25".
- ServerCore capability computation → `server/discover` read-only projection (+ `extensions` reverse-DNS map in capability types, `src/types/capabilities.rs`).

</code_context>

<specifics>
## Specific Ideas

- The opt-in should read like the rest of the builder DSL and mirror the official Rust SDK's runtime opt-in approach (`serve_with_lifecycle` pattern) rather than compile-time gating.
- Per-request version signal is authoritative over session-stored version when both exist (research Pitfall 3) — locked, not up for re-decision in planning.
- Final-spec checkpoint discipline: the spec finalizes 2026-07-28 (six days after roadmap creation). Anything wire-exact (error-code values) is structure-first in this phase; values land only from the published final schema.json.

</specifics>

<deferred>
## Deferred Ideas

- **`server/discover` answered on v1 connections as an upgrade probe** — came up while deciding D-10; this is exactly the deferred VERS-F1 (client-side STDIO backcompat probe) and stays deferred.

</deferred>

---

*Phase: 112-Version Plumbing Spine*
*Context gathered: 2026-07-22*
