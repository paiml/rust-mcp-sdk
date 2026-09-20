# Phase 112: Version Plumbing Spine - Research

> **⚠ CORRECTION (post-review, 2026-07-22) — supersedes all "wasm mirror" / "wasm parity" prose below.**
> This research's recurring premise that there is a compiled **wasm `RequestHandlerExtra` mirror** at `src/shared/cancellation.rs:50` and a "wasm mirror" dispatch site is **FALSE** (verified against the live tree). Facts:
> - `src/shared/cancellation.rs` is a **dead orphan** — not declared in `src/shared/mod.rs` (`grep 'pub mod cancellation'` → absent), never compiled on any target.
> - `src/server/core.rs`, `src/server/mod.rs`'s `ServerCore`, `src/server/cancellation.rs`, and `src/server/builder.rs` are all `#[cfg(not(target_arch = "wasm32"))]` (src/server/mod.rs:37/49) — excluded from wasm32. There is **no "wasm build of core.rs"**.
> - On wasm32, `RequestHandlerExtra` is a **zero-field stub** (`src/server/mod.rs:162`), and the only server is the out-of-scope `WasmServerCore` (`src/server/wasm_core.rs`).
>
> **Therefore this phase's server plumbing is NATIVE-ONLY.** Plan 02 edits `src/server/cancellation.rs` only; Plans 04/05 touch the native dispatch sites only; nothing edits `src/shared/cancellation.rs` or `src/server/wasm_core.rs`. The single wasm obligation is that the cfg-agnostic resolver in `src/types/protocol/context.rs` keeps `cargo build --lib --target wasm32-unknown-unknown` green (it compiles there with no wasm caller). Where the text below says "+ wasm mirror" / "wasm parity", read it as "native-only; wasm build stays green." The PLAN files (112-02/04/05) and 112-VALIDATION.md carry the authoritative, corrected scope.

**Researched:** 2026-07-22
**Domain:** Dual-version MCP protocol plumbing in the pmcp Rust SDK — resolving a per-request `ProtocolContext` (era) once at transport ingress and threading it through dispatch, so one binary serves both 2025-11-25 (v1) and 2026-07-28 (v2) clients, additively (2.x minor), v2 strictly opt-in.
**Confidence:** HIGH on mechanism/integration points (read directly from the 2.17.0 tree); MEDIUM on the exact v2 wire-value details (final schema publishes 2026-07-28, six days after this research) — those are explicitly deferred to structure-only in this phase.

## Summary

This is a plumbing and typing phase against pmcp's own codebase, not a dependency phase. Every integration point in the milestone research (`.planning/research/ARCHITECTURE.md`) was re-verified directly against the 2.17.0 tree during this pass and holds: the negotiation constants (`src/types/protocol/version.rs`), the `ProtocolVersion` newtype and `RequestMeta` flatten map (`src/types/protocol/mod.rs:28`, `:315`), the two dispatch sites (`src/server/core.rs:1118` and `src/server/mod.rs:1252`) plus the wasm mirror, the native and wasm `RequestHandlerExtra` (`src/server/cancellation.rs:179`, `src/shared/cancellation.rs:50`), the HTTP header constants (`src/shared/http_constants.rs`), and the frozen `-32002` task-pending literal (`src/server/core.rs:1145`, `src/server/task_dispatch.rs:576` + locking test `pending_tasks_result_preserves_minus_32002`). **Zero new runtime dependencies** — all VERS-01..09 work is additive Rust source over crates pmcp already vendors.

Two findings materially sharpen the plan beyond the milestone research. First, **`ServerCapabilities.extensions` already exists** (`src/types/capabilities.rs:109`, with round-trip + coexistence locking tests) — VERS-08's *type* is already shipped from the Skills phase, so VERS-08 here is purely populating it via the builder and projecting it read-only through `server/discover`, not adding a field. Second, the error-code surface today is **scattered integer literals** (`ProtocolErrorCode` enum at `mod.rs:132` holds only the 4 standard JSON-RPC codes; `-32002` lives as a bare literal in two dispatch sites) — VERS-06's "one centralized version-gated table" is a genuine consolidation, and the RC clarifies the collision: SEP-2164 renames the spec's *resource-not-found* `-32002`→`-32602`, which is a **different semantic** from pmcp's *task-pending* `-32002`. The final 2026-07-28 schema.json is not published as of this research (6 days out), so v2 error values MUST be structure-only in this phase (locked in CONTEXT D-40/VERS-06).

The dominant risk is not "can we build it" but "will we silently break v1 or trip an accidental 3.0." Two public enums in the plumbing path — `ClientRequest` (`mod.rs:478`) and `ProtocolErrorCode` (`mod.rs:132`) — are **not `#[non_exhaustive]`**, so adding the `ServerDiscover`/`TasksUpdate` variants and any new error-code variant is flagged by `cargo-semver-checks` as a caution-worthy (still-minor) enum-variant addition. Neither `cargo-semver-checks` nor `cargo-public-api` is currently installed; installing and running them is the phase's additive-guarantee gate (Pitfall 5/8).

**Primary recommendation:** Add `2026-07-28` + a `protocol_era()` classifier to `version.rs` keeping `LATEST_PROTOCOL_VERSION` pinned; introduce a new additive `ProtocolContext` value type resolved once at ingress and threaded next to `auth_context` into both dispatch sites (+ wasm mirror parity, per the Phase-109 precedent); expose typed accessors on `RequestHandlerExtra`; add an explicit version accept-list builder method (`.with_supported_protocol_versions([...])`); add `MCP_METHOD`/`MCP_NAME` header constants; scaffold `server/discover` + `resultType` (v2-only, injected at the dispatch/serialization layer) + a centralized error-code module with **v2 values left as TODO pending final schema**. Gate the whole thing with `cargo-semver-checks` proving it stays a 2.x minor.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Era detection (VERS-01) | Transport ingress (HTTP header parse / stdio+`_meta`) | — | The version signal only exists at the boundary; resolve once, carry as a value (ARCHITECTURE Pattern 1) |
| `ProtocolContext` threading (VERS-01) | ServerCore dispatch (`core.rs`/`mod.rs`) | Handler surface (`RequestHandlerExtra`) | Threaded next to `auth_context`; handlers read via typed accessors, never ambient session state |
| Opt-in accept-list (VERS-02) | Builder (`ServerCoreBuilder`) | version.rs constants | Runtime opt-in, matches official Rust SDK `serve_with_lifecycle`; no cargo feature (D-01/D-02) |
| Per-request `_meta` self-description (VERS-03) | Transport ingress → `RequestMeta.other` flatten map | Handler accessors | Reuses Phase-109 namespaced map; add typed accessors, not a new type |
| `server/discover` (VERS-04) | ServerCore dispatch (read-only projection) | Capability computation (`core.rs:210`) | Stateless replacement for initialize's capability exchange; projects already-computed caps |
| Required v2 headers (VERS-05) | Streamable-HTTP transport (`streamable_http_server.rs`) | — | Headers exist only on HTTP; stdio v2 is header-free (D-11) |
| Error-code table (VERS-06) | New centralized constants module | Dispatch call sites | One place to version-gate; v2 values from final schema only |
| `resultType` envelope (VERS-07) | Era-gated dispatch/serialization layer | Internal `ResultType` enum | Injected at serialization, not a public Result field (D-08) — semver-safe |
| `extensions` capability map (VERS-08) | `ServerCapabilities.extensions` (already exists) + builder | `server/discover` projection | Type already shipped; populate + project |
| W3C trace-context (VERS-09) | `RequestMeta.other` accessors | Dispatch propagation | Near-free over existing flatten map; propagation depth is Claude's discretion |

## User Constraints (from CONTEXT.md)

### Locked Decisions

**v2 opt-in API shape**
- **D-01:** Opt-in is a **builder method, no cargo feature flag**. v2 support is core protocol code, always compiled; zero new deps means a feature gate buys nothing and would grow the already-large feature matrix (feature-unification false-greens have burned this repo twice).
- **D-02:** The opt-in takes the shape of an **explicit version accept-list** (e.g. `.with_supported_protocol_versions([...])`), not a boolean or max-version knob. This makes v1-only (default), dual, and a future v2-only all expressible with one API and directly supports the Phase 117 severability story.
- **D-03:** The version set accepts **typed constants** — new `pub const`s (e.g. a `2026-07-28` constant) alongside the existing `LATEST/DEFAULT/SUPPORTED_PROTOCOL_VERSIONS`, wrapped in the existing `ProtocolVersion` newtype (`src/types/protocol/mod.rs:28`). The `protocol_era()` classifier lives next to them. Arbitrary strings stay constructible for tests.
- **D-04:** A v2-style request hitting a **non-opted-in server behaves exactly as today**: unknown `_meta` keys are already passthrough (Phase-109 flatten map), the request flows down the v1 path and fails naturally (e.g. "server not initialized"). No era-detection code runs on non-opted-in servers — the strongest reading of VERS-02.

**Header enforcement (VERS-05)**
- **D-05:** **Strict reject on the v2 path**: if a request self-identifies as v2 and `Mcp-Method`/`Mcp-Name` are missing, reject with a 4xx + structured JSON-RPC error. No lenient transition window, no configurable strictness knob — the official conformance suite (Phase 118) will test exactly this, and lenient-now means a breaking tightening later. v1 requests untouched.
- **D-06:** **`Mcp-Method` is cross-checked against the JSON-RPC body's `method`; mismatch = reject (fail closed).** A header/body desync is either a buggy client or a smuggling attempt (WAF sees one method, server executes another). This is the security-correct reading and cheap to test.

**resultType envelope (VERS-07)**
- **D-07:** `resultType` is emitted on **v2 responses only** — v1 responses stay byte-identical to today (no fixture/snapshot churn, no risk to strict v1 consumers). The spec's absent-means-`complete` default covers v1 readers.
- **D-08:** `resultType` is **injected at the era-gated dispatch/serialization layer**, not added as a public field on Result structs. Handlers keep returning today's Result types unchanged (semver-safe, zero public-API churn). A typed `ResultType` enum exists internally so Phases 113 (`input_required`) and 114 (`task`) can set the discriminator.

**server/discover + stdio scope (VERS-04, VERS-01/03)**
- **D-09:** `server/discover` is **auto-enabled by v2 opt-in** — no separate toggle. It's a core v2 method (the stateless replacement for initialize's capability exchange) and a read-only projection of already-computed ServerCore capabilities. One knob, not two.
- **D-10:** A v1-era request for `server/discover` gets standard **`-32601` method-not-found** — the method exists only in the v2 dispatch era. Clean era separation; same gate mechanism `tasks/list` will use in the opposite direction in Phase 114.
- **D-11:** **Era-detection is transport-agnostic**: `ProtocolContext` resolution reads per-request `_meta` on ALL transports (stdio included — same `RequestMeta` flatten map), while the `Mcp-Method`/`Mcp-Name` header requirements apply only where headers exist (HTTP). One era-resolution code path; stdio v2 comes essentially for free, letting stdio dev tools (mcp-tester, cargo pmcp dev loops) exercise v2 locally.

### Claude's Discretion
- W3C trace-context (VERS-09) accessor shape and propagation depth (typed accessors + propagation through dispatch required; integration depth with the `tracing`/observability module is Claude's call).
- `extensions` capability map (VERS-08) builder/plumbing details.
- Error-code table module placement and structure (constraints locked below: centralized, version-gated, values-from-final-schema-only, frozen `-32002` untouched).
- Exact builder method naming, `ProtocolContext` field/accessor naming, and where the era gate lives in `ServerCore` dispatch — subject to the locked research guidance (resolved once at ingress, threaded next to `auth_context`, typed accessors on `RequestHandlerExtra`, wired at BOTH `core.rs` and `server/mod.rs` dispatch sites + wasm mirror parity per the Phase 109-00 precedent).

### Locked Specifics (not up for re-decision in planning)
- The opt-in should mirror the official Rust SDK's runtime opt-in (`serve_with_lifecycle`/`preferred_versions`) rather than compile-time gating. [CITED: github.com/modelcontextprotocol/rust-sdk]
- Per-request version signal is authoritative over session-stored version when both exist (research Pitfall 2/3). Locked.
- Final-spec checkpoint discipline: anything wire-exact (error-code values) is structure-first in this phase; values land only from the published final 2026-07-28 schema.json.

### Deferred Ideas (OUT OF SCOPE)
- **`server/discover` answered on v1 connections as an upgrade probe** — this is deferred VERS-F1 (client-side STDIO backcompat probe) and stays deferred.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| VERS-01 | Resolve `ProtocolContext` (era, negotiated version, clientInfo, clientCapabilities) once at ingress; thread through dispatch; typed accessors on `RequestHandlerExtra` | New additive value type; threaded next to `auth_context` at `core.rs:1118` + `mod.rs:1252` + wasm mirror; accessors added to `RequestHandlerExtra` (both `cancellation.rs`) — see Code Examples |
| VERS-02 | Support 2026-07-28 as explicit opt-in; `LATEST_PROTOCOL_VERSION` stays 2025-11-25; v1 negotiates as before; stays 2.x minor | Add const + `protocol_era()` to `version.rs`; builder accept-list (D-02); Pitfall 1 (never flip LATEST); semver gate |
| VERS-03 | v2 requests self-describe via per-request `_meta` (`io.modelcontextprotocol/protocolVersion`/`clientInfo`/`clientCapabilities`); v2 results carry `serverInfo` | `RequestMeta.other` flatten map already round-trips these keys (`mod.rs:315`); add typed accessors; discover/dispatch attaches `serverInfo` on v2 |
| VERS-04 | `server/discover` as read-only projection of computed ServerCore capabilities | `ServerCore.capabilities` (`core.rs:210`) already computed; add `handle_discover()` returning same projection without handshake side effects; `ClientRequest::ServerDiscover` variant |
| VERS-05 | Required `Mcp-Method`/`Mcp-Name` (+ `MCP-Protocol-Version`) enforced inbound and emitted outbound on v2 HTTP | Add `MCP_METHOD`/`MCP_NAME` to `http_constants.rs`; strict reject (D-05) + body cross-check (D-06) in `streamable_http_server.rs` validate path |
| VERS-06 | One centralized version-gated error-code table; v2 values ONLY from final schema.json; frozen `-32002` unchanged | Today codes are scattered literals; consolidate into a module; **v2 values = TODO pending 2026-07-28 schema**; keep frozen `-32002` (`core.rs:1145`, `task_dispatch.rs:576`) untouched with its locking test |
| VERS-07 | Every result carries `resultType` (`complete`/`input_required`/`task`); missing defaults to `complete` | Internal `ResultType` enum; injected at era-gated serialization (D-08), v2-only (D-07); Phases 113/114 set non-complete values |
| VERS-08 | `extensions` capability map (reverse-DNS IDs) supported in capability negotiation | **`ServerCapabilities.extensions` field already exists** (`capabilities.rs:109`); populate via builder + project via discover |
| VERS-09 | W3C trace-context keys (`traceparent`/`tracestate`/`baggage`) in `_meta` surfaced via typed accessors and propagated | Same `RequestMeta.other` flatten map; add typed accessors; propagate through dispatch (depth = discretion) |

## Standard Stack

### Core
No new libraries. All work is additive Rust source over the crates pmcp already vendors.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` / `serde_json` | existing | New additive protocol shapes (`ProtocolContext`, `ResultType`, discover result); `RequestMeta.other` map | Already the protocol backbone; camelCase rename convention in place |
| `http` | existing | New header name constants (`Mcp-Method`, `Mcp-Name`) | `http_constants.rs` already carries `MCP_SESSION_ID`/`MCP_PROTOCOL_VERSION` |
| `hyper`/`axum`/`tower`/`tower-http` | existing | Streamable-HTTP header enforcement | The v2 request path reuses the existing `stateless()` branch (Phase 113 gates onto it) |

### Supporting (dev/CI tooling — required by this phase's semver gate, not runtime deps)
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `cargo-semver-checks` | latest | Prove the phase stays a 2.x minor (Pitfall 8) | Run before commit; **not currently installed** |
| `cargo-public-api` | latest | Diff the public API surface for accidental breaks | Optional complement to semver-checks; **not currently installed** |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Builder accept-list (D-02) | cargo feature flag | REJECTED by D-01 — grows the feature matrix, risks feature-unification false-greens |
| Internal `ResultType` injected at serialization (D-08) | Public field on every Result struct | REJECTED by D-08 — wide additive public-API churn; risks fixture churn |
| New `ProtocolContext` value type | Overload existing `RequestHandlerExtra` fields in place | Changing an existing field's type = accidental 3.0 (Pitfall 8); a new additive field/accessor stays minor |

**Installation (dev tooling only):**
```bash
cargo install cargo-semver-checks --locked
cargo install cargo-public-api --locked
```

**Version verification:** No crate versions to verify — this phase adds zero runtime dependencies. The only `Cargo.toml` change anywhere in the milestone is a `jsonschema` bump that belongs to Phase 115 (SCHM), not this phase.

## Package Legitimacy Audit

No external packages are installed by this phase. `cargo-semver-checks` and `cargo-public-api` are first-party Rust-project dev tooling from the `obi1kenobi` / `cargo-public-api` maintainers respectively, installed via `cargo install` from crates.io as CI/dev gates — they are not added to any `Cargo.toml` and do not enter the published crate's dependency tree or wasm build. slopcheck / registry audit is N/A: nothing is added to the runtime graph.

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────────┐
│  TRANSPORT INGRESS  — per-request era resolution lives HERE (VERS-01)   │
│  ┌──────────────────────────┐     ┌──────────────────────────────────┐  │
│  │ streamable_http_server   │     │ stdio / wasm_http                │  │
│  │  read MCP-Protocol-Version│     │  read _meta                      │  │
│  │  read Mcp-Method/Mcp-Name │     │   io.modelcontextprotocol/*      │  │
│  │  (v2 strict reject D-05,  │     │  (headers N/A — D-11)            │  │
│  │   body cross-check D-06)  │     │                                  │  │
│  └───────────┬──────────────┘     └───────────────┬──────────────────┘  │
│              │  IF server opted-in (D-04):                               │
│              │  protocol_era(version) + parse _meta                      │
│              │  → BUILD ProtocolContext{era, negotiated_version,         │
│              │                          client_info, client_capabilities}│
├──────────────┼──────────────────────────────────────┼───────────────────┤
│              ▼   (threaded NEXT TO auth_context)     ▼                    │
│  DISPATCH  handle_request_internal                                       │
│            core.rs:1118  AND  mod.rs:1252  (+ wasm mirror parity)        │
│   ┌──────────────────────────────────────────────────────────────────┐ │
│   │ era-gate:                                                          │ │
│   │   V1 → initialize arm reached (unchanged); server/discover=-32601 │ │
│   │   V2 → initialize arm never reached; server/discover projects caps│ │
│   │        (VERS-04) incl. extensions map (VERS-08)                    │ │
│   │   error codes resolve from ONE centralized table (VERS-06)        │ │
│   │   resultType injected here, v2-only (VERS-07, D-07/D-08)           │ │
│   └───────────────────────────────┬──────────────────────────────────┘ │
├───────────────────────────────────┼─────────────────────────────────────┤
│  HANDLER SURFACE                   ▼                                     │
│   RequestHandlerExtra                                                    │
│     + protocol_context() / era() / protocol_version()                   │
│     + client_info() / client_capabilities()   (VERS-01/03)              │
│     + trace_context() → traceparent/tracestate/baggage (VERS-09)        │
│   (frozen -32002 task-pending path UNTOUCHED — core.rs:1145)            │
└──────────────────────────────────────────────────────────────────────── ┘
```

Trace the primary use case: a v2 `tools/call` enters at HTTP ingress → headers validated (strict) → `protocol_era("2026-07-28")=V2` → `ProtocolContext` built from headers + `_meta` → threaded into `handle_request_internal` next to `auth_context` → handler reads `extra.client_info()` → response gets `resultType:"complete"` injected + `serverInfo`. A v1 `tools/call` on the same server skips era-detection entirely if the server never opted in (D-04), or resolves `era=V1` and hits the unchanged path if it did.

### Component Responsibilities

| Component | File:line | Status | This-phase responsibility |
|-----------|-----------|--------|---------------------------|
| version constants + `protocol_era()` | `src/types/protocol/version.rs` | MODIFIED | Add `2026-07-28` const + `protocol_era(&str) -> Era`; **keep `LATEST_PROTOCOL_VERSION = "2025-11-25"`**; update the `supports_four_versions` test deliberately (tripwire) |
| `ProtocolContext` | new — `src/shared/` or `src/server/` | NEW | `{ era, negotiated_version, client_info, client_capabilities }`; built at ingress, threaded through dispatch |
| `Era` enum | new (next to `protocol_era`) | NEW | `V1 { 2025-11-25/... }` / `V2 { 2026-07-28 }` classifier |
| builder accept-list | `src/server/builder.rs` (`impl ServerCoreBuilder`, from :112) | MODIFIED | `.with_supported_protocol_versions([...])`; default = v1-only (D-04) |
| `RequestHandlerExtra` (native) | `src/server/cancellation.rs:179` | MODIFIED | Add additive `protocol_context` field + typed accessors (`era()`, `protocol_version()`, `client_info()`, `client_capabilities()`, `trace_context()`) |
| ~~`RequestHandlerExtra` (wasm)~~ **CORRECTED — NOT MODIFIED** | ~~`src/shared/cancellation.rs:50`~~ | **stale premise** | **DO NOT EDIT.** Post-review verification: `src/shared/cancellation.rs` is a DEAD ORPHAN (not declared in `src/shared/mod.rs`, never compiled). The wasm32 `RequestHandlerExtra` is a zero-field stub (`src/server/mod.rs:162`); extending it is out of scope. This phase's plumbing is NATIVE-ONLY — Plan 02 edits `src/server/cancellation.rs` only. |
| `RequestMeta` | `src/types/protocol/mod.rs:315` | REUSED as-is | `#[serde(flatten)] other` already round-trips `io.modelcontextprotocol/*` + trace keys — no type change |
| `http_constants.rs` | `src/shared/http_constants.rs` | MODIFIED | Add `MCP_METHOD = "mcp-method"`, `MCP_NAME = "mcp-name"` |
| Streamable-HTTP inbound | `src/server/streamable_http_server.rs` | MODIFIED | v2 strict header reject (D-05) + `Mcp-Method` vs body method cross-check (D-06); emit headers outbound |
| ServerCore dispatch | `src/server/core.rs:1118` + `src/server/mod.rs:1252` | MODIFIED | Thread `ProtocolContext`; era-gate `server/discover` (V2) vs `-32601` (V1, D-10); inject `resultType` (v2-only) |
| `ClientRequest` enum | `src/types/protocol/mod.rs:478` | MODIFIED (additive) | Add `ServerDiscover` (+ `TasksUpdate` scaffold-only if 114 needs the variant early). **NOT `#[non_exhaustive]`** — semver-checks flags as minor; see Pitfall 4 |
| `ServerCapabilities.extensions` | `src/types/capabilities.rs:109` | REUSED (field exists) | Populate via builder + project via `server/discover`; type + locking tests already shipped |
| centralized error-code table | new module (placement = discretion) | NEW | Consolidate scattered literals; **v2 values TODO pending final schema**; frozen `-32002` re-exported unchanged |
| `ResultType` enum | new (internal) | NEW | `Complete`/`InputRequired`/`Task`; injected at serialization; only `Complete` used this phase |

### Pattern 1: Era detection at ingress, era-gating at decision points
**What:** Resolve `Era` once where the version signal exists (HTTP header / stdio `_meta`) and thread it as an explicit `ProtocolContext`; gate the handful of divergent decisions (initialize-vs-discover, error code, resultType) on it.
**When:** Dual-protocol coexistence — exactly this phase.
**Example:** see Code Examples below.

### Pattern 2: Additive field + typed accessor, never mutate existing field types
**What:** `ProtocolContext` rides as a NEW field on `RequestHandlerExtra` with NEW accessor methods; existing fields (`auth_context`, `request_meta`) are untouched.
**Why:** Changing an existing public field's type is a breaking (3.0) change (Pitfall 8). Adding a field to a struct constructed via `new()` + `with_*` builders is minor.

### Pattern 3: Flattened namespaced `_meta` as the v2 transport for out-of-band context
**What:** The Phase-109 `RequestMeta.other` flatten map (`mod.rs:344`) is already the exact shape v2 uses for `io.modelcontextprotocol/clientInfo`, `protocolVersion`, `clientCapabilities`, and W3C trace keys. Add typed accessors, not new types.
**Trade-off:** Stringly-typed keys; but zero new protocol types and byte-identical v1 serialization (empty `other` emits nothing — confirmed by the doc comment at `mod.rs:341-343`).

### Anti-Patterns to Avoid
- **Flipping `LATEST_PROTOCOL_VERSION` to 2026-07-28** (Pitfall 1): `negotiate_protocol_version` returns `LATEST` for any unrecognized version (`version.rs:32`), so this silently upgrades every legacy client into v2. Keep it pinned; v2 opt-in only.
- **Forking the transport into a v2 server**: the eras differ only in session handling + headers; era-gate inside existing handlers.
- **Deleting the `initialize` arm / `tasks/list` to "modernize"**: v1 clients still handshake and call `tasks/list`; era-gate so V2 never reaches them (Phase 114 territory, but the gate mechanism is defined here).
- **Adding `resultType` as a public Result field**: D-08 forbids it — inject at serialization.
- **Touching the frozen `-32002` constant or its locking test to "make v2 pass"** (Pitfall 6): the spec's `-32002`→`-32602` is *resource-not-found*, a different semantic from pmcp's *task-pending* `-32002`. Do not silently reconcile — VERS-06 fills v2 values from final schema only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-request namespaced context transport | A new `V2Meta` struct + serde plumbing | Existing `RequestMeta.other` flatten map (`mod.rs:344`) | Already round-trips `io.modelcontextprotocol/*` byte-identically; Phase-109 proved it |
| Capability projection for `server/discover` | Recomputing capabilities in the discover handler | Read `ServerCore.capabilities` (`core.rs:210`) already computed at build | Discover is a *read-only view* (D-09) — recomputation risks drift from initialize's projection |
| `extensions` capability type | A new reverse-DNS map type | `ServerCapabilities.extensions: Option<HashMap<String, Value>>` (`capabilities.rs:109`) | Already shipped with round-trip + coexistence locking tests |
| Version opt-in machinery | A bespoke lifecycle mode enum | Mirror official Rust SDK `serve_with_lifecycle`/accept-list shape (D-02) | Cross-SDK wire/DX compatibility; avoids inventing incompatible assumptions |
| Additive-safety proof | Manual reasoning about semver | `cargo-semver-checks` | Mechanically catches the enum-variant / field-type traps that constitute an accidental 3.0 |

**Key insight:** Almost every "new" surface in this phase already has a home in the tree (flatten map, extensions field, computed capabilities, header-constants module). The phase is mostly *wiring existing pieces together behind an era gate*, plus two genuinely new value types (`ProtocolContext`, `Era`) and one new module (error-code table). Treat "did I add a new type where an existing one would do?" as a review question.

## Runtime State Inventory

This is an **additive greenfield-within-a-codebase plumbing phase**, not a rename/refactor/migration. There is no string being renamed across stored data, no data migration, and no live-service reconfiguration.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no datastore keys/collections/user_ids change. Task storage backends are explicitly untouched (TASK-06 is a later phase; this phase adds no storage writes). | None |
| Live service config | None — no external service (n8n, Datadog, etc.) config references the new constants. | None |
| OS-registered state | None — no Task Scheduler / pm2 / systemd registrations involved. | None |
| Secrets/env vars | None — no new secret keys or env var names. | None |
| Build artifacts | None — pure source additions; no package rename, no egg-info/binary-name change. Downstream workspace crates (`mcp-tester`, `cargo-pmcp`, `pmcp-agent`, toolkit) must still **compile** against the new pmcp — verify, don't migrate. | Recompile-check downstream crates (Pitfall 8 "downstream crates" checklist item) |

**Nothing found in the first four categories — verified by:** this phase's diff is confined to `src/types/protocol/`, `src/server/`, `src/shared/`, and a new error-code module; grep of the plan surface shows no datastore/OS/secret touchpoints.

## Common Pitfalls

### Pitfall 1: Flipping `LATEST_PROTOCOL_VERSION` to 2026-07-28
**What goes wrong:** `negotiate_protocol_version` (`version.rs:29-35`) returns `LATEST` for any unrecognized version. Bumping the constant silently negotiates every unknown/legacy client into v2 semantics — the exact backward-compat break the dual-version stack exists to prevent. Downstream readers (`src/lib.rs:255` doctest `assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25")`, cargo-pmcp scaffolds, fixtures) drift or hard-fail.
**Why it happens:** "Latest = newest we support" feels right; the fallback-to-LATEST branch makes the bump look harmless in unit tests.
**How to avoid:** Add `2026-07-28` to a *supported-versions* set behind the opt-in accept-list; keep `LATEST_PROTOCOL_VERSION = "2025-11-25"`. For the stateless header path, default to v1 when no `MCP-Protocol-Version` header is present.
**Warning signs:** `latest_version_is_2025_11_25` test (`version.rs:42`) needs "fixing"; v1 integration tests start expecting `server/discover` instead of `initialize`. [VERIFIED: codebase grep — `version.rs:33`, `lib.rs:255`, `.planning/research/PITFALLS.md`]

### Pitfall 2: Two negotiation mechanisms colliding (session-stored vs per-request header)
**What goes wrong:** v1 negotiates once at `initialize` and stores the version on the session; v2 has no initialize — version arrives per-request. Without an explicit precedence rule a request can be dispatched under v1 assumptions while carrying v2 headers.
**How to avoid:** `ProtocolContext` is authoritative: per-request header/`_meta` wins when present; session-stored version is the v1-only fallback. Never let session state override an explicit per-request signal. This is a **locked** decision (CONTEXT Specifics), not a planning choice.
**Warning signs:** handlers calling `session.protocol_version()` on a request that has no session (stateless). [VERIFIED: `.planning/research/PITFALLS.md`; codebase `mod.rs:1262`]

### Pitfall 3: Wiring only one dispatch site / missing the wasm mirror
**What goes wrong:** pmcp dispatches at BOTH `src/server/core.rs:1118` and `src/server/mod.rs:1252`, plus a wasm mirror. `request_meta` is already threaded at both (`core.rs:514`, `mod.rs:1540`). If `ProtocolContext` is only wired at one, era-awareness is intermittent by transport/path.
**How to avoid:** Thread the context at both native sites AND the wasm mirror — this is the exact Phase-109-00 precedent for per-request field plumbing (CONTEXT "Established Patterns"). Add a parity test.
**Warning signs:** era resolves under HTTP but is `None` under stdio, or works native but not wasm. [VERIFIED: codebase grep — dispatch sites + `with_request_meta` call sites]

### Pitfall 4: Adding enum variants to non-`#[non_exhaustive]` public enums looks minor but trips semver-checks
**What goes wrong:** `ClientRequest` (`mod.rs:478`) and `ProtocolErrorCode` (`mod.rs:132`) are **not `#[non_exhaustive]`**. Adding `ServerDiscover`/`TasksUpdate` (VERS-04) or a new error-code variant is classified by `cargo-semver-checks` as `enum_variant_added` — technically minor, but it breaks any downstream *exhaustive* `match` without a wildcard arm.
**Why it happens:** "It's just adding a variant" — but strict semver treats it as caution-worthy.
**How to avoid:** Run `cargo-semver-checks` and confirm the result is classified minor (not major). Consider whether `ClientRequest`/`ProtocolErrorCode` should gain `#[non_exhaustive]` now (itself a one-time minor change) to make all future variant additions safe — a planning decision. Keep new codes out of the `ProtocolErrorCode` C-style enum if that risks the discriminant surface; the centralized table can be `pub const i32`s instead.
**Warning signs:** `cargo-semver-checks` reporting `enum_variant_added`; downstream workspace crates failing to compile on an exhaustive match. [VERIFIED: codebase — neither enum carries `#[non_exhaustive]`]

### Pitfall 5: Sleepwalking into a pmcp 3.0
**What goes wrong:** flipping LATEST, changing an existing public field's type (e.g. shoving clientInfo into an existing `RequestHandlerExtra` field), or mutating a shared dispatch table in place each look small but cumulatively constitute a breaking major.
**How to avoid:** All new types additive; existing public types unchanged; LATEST pinned; run `cargo-semver-checks`/`cargo-public-api` as a phase gate (neither is currently installed — install first). Write the "what forces 3.0" list at phase start.
**Warning signs:** semver-checks flagging removed/changed items; toolkit/agent/team-servers/cargo-pmcp needing code changes to compile. [VERIFIED: `.planning/research/PITFALLS.md`; tooling absence confirmed via `which`]

### Pitfall 6: The `-32002`/`-32602` semantic collision (VERS-06 gate)
**What goes wrong:** SEP-2164 renames the spec's *resource-not-found* `-32002`→`-32602`. But in THIS codebase `-32602` is already `InvalidParams` (`mod.rs:135`) and `-32002` is pmcp's own FROZEN *task-pending* code (`core.rs:1145`, `task_dispatch.rs:576`, locking test `pending_tasks_result_preserves_minus_32002`). These are *different* uses of `-32002`. Blindly "applying the rename" onto the frozen task code would make task-pending indistinguishable from invalid-params, breaking `wait_for_task`.
**Why it happens:** treating a changelog "rename" as global without noticing pmcp's `-32002` is a semantically different, frozen code.
**How to avoid:** VERS-06 is **structure-first**. Build the centralized table now with v1 values (including the frozen `-32002` re-exported verbatim) and v2 values as explicit `TODO`s. Fill v2 values ONLY from the final 2026-07-28 schema.json (not the RC blog), and even then version-gate error interpretation so v1's frozen `-32002` semantics are preserved. **Do NOT edit the frozen locking test.** The final schema is not published as of 2026-07-22 (6 days out).
**Warning signs:** the `-32002` freeze test being "updated" without a final-schema citation; a single code carrying two meanings. [VERIFIED: codebase grep for `-32002`; RC clarification via WebSearch — SEP-2164 targets resource-not-found] [CITED: 4sysops MCP 2026-07-28 overview]

### Pitfall 7: v1 wire fixture churn from resultType
**What goes wrong:** emitting `resultType` on v1 responses churns every existing wire fixture/snapshot and risks strict v1 consumers.
**How to avoid:** D-07 — v2 responses only; absent-means-`complete` covers v1 readers. Inject at the era-gated serialization layer (D-08).
**Warning signs:** existing v1 response snapshot tests failing after adding the envelope. [VERIFIED: CONTEXT D-07/D-08]

## Code Examples

> These are illustrative shapes grounded in the current tree, not verbatim from external docs. Exact naming is Claude's discretion within the locked guidance.

### Era classifier next to the version constants (VERS-02, D-03)
```rust
// src/types/protocol/version.rs — ADD; keep LATEST_PROTOCOL_VERSION pinned to "2025-11-25"
// Source pattern: existing constants at version.rs:1-20 (verified this session)

/// MCP 2026-07-28 ("v2") protocol version — opt-in only, never the negotiation default.
pub const PROTOCOL_VERSION_2026_07_28: &str = "2026-07-28";

/// Protocol era — the coarse behavioral split that dispatch gates on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    /// 2025-11-25 and earlier: session/initialize handshake, tasks/list, -32002 semantics.
    V1,
    /// 2026-07-28: stateless, server/discover, required headers, resultType.
    V2,
}

/// Classify a negotiated version string into its era. Unknown → V1 (conservative).
pub fn protocol_era(version: &str) -> Era {
    match version {
        PROTOCOL_VERSION_2026_07_28 => Era::V2,
        _ => Era::V1, // 2025-11-25 / 2025-06-18 / 2025-03-26 / 2024-11-05 and unknowns
    }
}
```

### ProtocolContext resolved at ingress, threaded next to auth_context (VERS-01)
```rust
// New additive value type (placement: src/shared/ or src/server/ — discretion)
#[derive(Debug, Clone)]
pub struct ProtocolContext {
    pub era: Era,
    pub negotiated_version: ProtocolVersion,   // existing newtype, mod.rs:28
    pub client_info: Option<Implementation>,   // existing type, mod.rs
    pub client_capabilities: Option<ClientCapabilities>, // existing, capabilities.rs:25
}

// Built at transport ingress from headers (HTTP) or _meta (all transports, D-11),
// then threaded into handle_request_internal alongside the existing auth_context param
// at BOTH core.rs:1118 and mod.rs:1252 (+ wasm mirror).
```

### Typed accessors on RequestHandlerExtra over the existing flatten map (VERS-03, VERS-09)
```rust
// src/server/cancellation.rs (native, :179) AND src/shared/cancellation.rs (wasm, :50) — mirror both
impl RequestHandlerExtra {
    /// Resolved protocol era for this request (None if server is not v2-opted-in — D-04).
    pub fn era(&self) -> Option<Era> { self.protocol_context.as_ref().map(|c| c.era) }

    /// Per-request client identity from _meta `io.modelcontextprotocol/clientInfo` (VERS-03).
    pub fn client_info(&self) -> Option<&Implementation> {
        self.protocol_context.as_ref().and_then(|c| c.client_info.as_ref())
    }

    /// W3C trace-context (traceparent/tracestate/baggage) read from request_meta.other (VERS-09).
    pub fn trace_context(&self) -> Option<TraceContext> {
        // request_meta is already Option<serde_json::Value> populated from RequestMeta.other
        // (with_request_meta at core.rs:514 / mod.rs:1540) — read the W3C keys out of it.
        self.request_meta.as_ref().and_then(TraceContext::from_meta)
    }
}
```

### Centralized version-gated error-code table — structure only (VERS-06)
```rust
// New module. v1 values are real; v2 values are TODO pending the final 2026-07-28 schema.json.
// The frozen task-pending code is re-exported verbatim — do NOT redefine or edit its locking test.

/// v1 (2025-11-25) frozen task-pending code. FROZEN — locking test:
/// `pending_tasks_result_preserves_minus_32002` (task_dispatch_tests.rs).
pub const V1_TASK_PENDING: i32 = -32002; // used at core.rs:1145, task_dispatch.rs:576

// pub const V2_RESOURCE_NOT_FOUND: i32 = /* TODO: fill from final 2026-07-28 schema.json
//   (SEP-2164 maps the spec's resource-not-found -32002 → -32602, a DIFFERENT semantic from
//   pmcp's frozen task-pending -32002 above). Do NOT hard-code before final publication. */;
```

### Header constants (VERS-05)
```rust
// src/shared/http_constants.rs — ADD next to existing MCP_SESSION_ID / MCP_PROTOCOL_VERSION
pub const MCP_METHOD: &str = "mcp-method";
pub const MCP_NAME: &str = "mcp-name";
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-version SDK, negotiation implicit at `initialize` | Per-request `ProtocolContext` resolved at ingress, era-gated dispatch | This milestone (v2.5) | The keystone this phase delivers |
| Session-based stateful handshake (2025-11-25) | Stateless, handshake-free 2026-07-28 (no initialize, no Mcp-Session-Id) | MCP spec 2026-07-28 | v2 reuses pmcp's existing `stateless()` branch (Phase 113 wires it) |
| Compile-time / cargo-feature version gating | Runtime opt-in via `serve_with_lifecycle` / version accept-list | Official Rust SDK beta (2026-07-28 RC) | pmcp mirrors this (D-01/D-02) |
| Scattered error-code integer literals | One centralized version-gated table | This phase (VERS-06) | Makes the `-32002`/`-32602` collision a one-place fix |

**Deprecated/outdated:**
- Roots/Sampling/Logging are marked deprecated-but-advisory in v2 (12-month window) — **do not remove** anything this phase (Pitfall 12 in milestone research; CONF-03 verifies runtime).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The final 2026-07-28 schema.json is NOT yet published as of research (6 days out), so v2 error-code and any wire-exact values must be structure-only this phase | VERS-06, Pitfall 6 | If final already published, planner could fill v2 values — but VERS-06 mandates final-schema-only regardless, so risk is low (over-conservative) |
| A2 | Adding `ServerDiscover`/`TasksUpdate` to `ClientRequest` and new codes to the error table stays a 2.x minor per `cargo-semver-checks` | Pitfall 4/5 | If semver-checks classifies major (e.g. some other incidental break), the phase must adjust to stay additive; mitigated by running the tool as a gate |
| A3 | The official Rust SDK opt-in is `serve_with_lifecycle` + `ClientLifecycleMode::Discover` + `preferred_versions` (runtime, wire-level) | Standard Stack, State of the Art | If the SDK API differs at stable, pmcp's DX naming diverges cosmetically — low impact since D-02 only requires *mirroring the runtime-opt-in spirit*, not the exact signature |
| A4 | Adding `#[non_exhaustive]` to `ClientRequest`/`ProtocolErrorCode` is itself a tolerable one-time minor change (planner's call) | Pitfall 4 | If deemed breaking by downstream, planner keeps enums as-is and relies on the additive-variant-is-minor classification instead |

**These four items should be confirmed during planning/execution** — A1 is resolved by checking whether `schema.json` is published on the day execution begins; A2 by running the tool; A3 by cross-checking the stable rmcp release; A4 by a semver-checks dry run.

## Open Questions (RESOLVED)

*Resolved during Phase 112 planning (2026-07-22).*

1. **Should `ClientRequest` and `ProtocolErrorCode` gain `#[non_exhaustive]` in this phase?**
   - What we know: neither is `#[non_exhaustive]` today; both will receive additive variants across v2.5 (discover/tasks-update here; more in 113/114).
   - What's unclear: whether adding the attribute now is worth the one-time minor churn vs. relying on additive-variant-is-minor each time.
   - Recommendation: run `cargo-semver-checks` both ways during planning; prefer adding `#[non_exhaustive]` once now if it classifies minor, to de-risk every later phase.
   - **RESOLVED → Plan 112-03 Task 2 executes the decision procedure at build time:** run `cargo semver-checks check-release` on `ClientRequest` WITHOUT then WITH `#[non_exhaustive]`; add the attribute iff it classifies minor, otherwise rely on additive-variant-is-minor. The chosen path + semver classification are recorded in `112-03-SUMMARY.md`. (`ProtocolErrorCode` is intentionally NOT touched — Q2 keeps it untouched via a `pub const` table.)

2. **Where does the centralized error-code table live?** (Discretion, per CONTEXT.)
   - Recommendation: a new `src/types/protocol/error_codes.rs` (or `src/error/codes.rs`) of `pub const i32`s re-exporting the frozen `-32002`, keeping the C-style `ProtocolErrorCode` enum untouched to avoid discriminant-surface semver risk.
   - **RESOLVED → adopted `src/types/protocol/error_codes.rs`** as a module of `pub const i32` values (standard JSON-RPC codes + `PARSE_ERROR` + frozen `V1_TASK_PENDING = -32002`, v2 values commented TODO), defined in Plan 112-03 and adopted at every emitting call site in Plan 112-07. The `ProtocolErrorCode` enum is left untouched; a consistency test asserts the consts equal the enum values.

3. **How deep does W3C trace-context propagation go?** (Explicit discretion, VERS-09.)
   - Recommendation: typed accessors + propagation through dispatch is the required floor; integration with the existing observability/`tracing` module (`with_observability`, `builder.rs:590`) is optional and can be a thin follow-up.
   - **RESOLVED → per CONTEXT.md Claude's-Discretion:** implement the required floor only — `TraceContext::from_meta` + typed `extra.trace_context()` accessor over the existing `request_meta` (Plans 112-01/112-02), propagated through dispatch. Deep integration with the `tracing`/`with_observability` module is explicitly deferred as an optional thin follow-up (NOT in Phase 112 scope).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / rustc (stable) | Everything | ✓ | workspace toolchain | — |
| `cargo-semver-checks` | Additive-guarantee gate (Pitfall 5) | ✗ | — | `cargo install cargo-semver-checks --locked` |
| `cargo-public-api` | API-surface diff (optional complement) | ✗ | — | `cargo install cargo-public-api --locked`; or rely on semver-checks alone |
| final 2026-07-28 `schema.json` | VERS-06 v2 error-code values | ✗ (publishes 2026-07-28, 6 days out) | — | **No fallback** — VERS-06 is structure-only until it lands; leave v2 values as TODO |

**Missing dependencies with no fallback:**
- final schema.json — blocks *filling* v2 error-code values, NOT the phase (VERS-06 is explicitly structure-first). Plan the table skeleton; a later task fills values once published.

**Missing dependencies with fallback:**
- `cargo-semver-checks` / `cargo-public-api` — install via `cargo install`. Required before the phase's semver gate can run.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / `#[tokio::test]` (no external test framework); property tests via `proptest`/`quickcheck` per CLAUDE.md ALWAYS-requirements |
| Config file | none — `cargo test`; CI runs with `--test-threads=1` (race-condition prevention, per CLAUDE.md) |
| Quick run command | `cargo test --lib protocol::version` (targeted) |
| Full suite command | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit — matches CI exactly) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| VERS-02 | `LATEST_PROTOCOL_VERSION` stays "2025-11-25"; `protocol_era("2026-07-28")==V2`; unknown→V1 | unit | `cargo test --lib protocol::version` | ⚠️ extend `version.rs` tests (update `supports_four_versions` → five) |
| VERS-01 | `ProtocolContext` threaded to both dispatch sites + wasm; accessors return resolved era | unit + integration | `cargo test protocol_context` | ❌ Wave 0 |
| VERS-03 | `_meta` `io.modelcontextprotocol/*` round-trips into `client_info()`/`protocol_version()` | unit | `cargo test request_meta_client_info` | ❌ Wave 0 (RequestMeta round-trip tests exist for other keys — extend) |
| VERS-04 | v2 `server/discover` projects capabilities incl. extensions; v1 gets `-32601` (D-10) | integration | `cargo test server_discover` | ❌ Wave 0 |
| VERS-05 | v2 missing `Mcp-Method`/`Mcp-Name` → 4xx (D-05); header/body method mismatch → reject (D-06) | integration (HTTP target) | `cargo test v2_required_headers` | ❌ Wave 0 (HTTP target only — Pitfall 11: in-memory transport can't exercise headers) |
| VERS-06 | frozen `-32002` preserved; centralized table compiles; v2 values are TODO | unit | `cargo test pending_tasks_result_preserves_minus_32002` (existing) + new table test | ✅ frozen test exists (`task_dispatch_tests.rs:355`); ❌ table test Wave 0 |
| VERS-07 | v2 response carries `resultType:"complete"`; v1 response byte-identical (no envelope) | unit + snapshot | `cargo test result_type_envelope` | ❌ Wave 0 |
| VERS-08 | builder populates `extensions`; discover projects it | unit | `cargo test extensions_capability` (extend existing) | ✅ round-trip tests exist (`capabilities.rs:801`); ❌ discover-projection Wave 0 |
| VERS-09 | `traceparent`/`tracestate`/`baggage` surfaced via accessor + propagated | unit | `cargo test trace_context` | ❌ Wave 0 |
| (gate) | Phase stays 2.x minor | tooling | `cargo semver-checks check-release` | ❌ Wave 0 (install tool) |

### Sampling Rate
- **Per task commit:** targeted `cargo test --lib` for the touched module (e.g. `protocol::version`)
- **Per wave merge:** `make quality-gate` (full fmt/clippy/build/test/audit)
- **Phase gate:** `make quality-gate` green + `cargo semver-checks` classifies minor + all v1 fixtures still green (dual-version regression) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Install `cargo-semver-checks` (+ optionally `cargo-public-api`) — the additive-guarantee gate
- [ ] `tests/` or `src/server/*_tests.rs` module for `protocol_context` threading (both native + wasm parity) — VERS-01
- [ ] v2 required-header enforcement tests routed through the **HTTP** `ConformanceTarget`, not in-memory transport (Pitfall 11) — VERS-05
- [ ] `server/discover` projection test incl. v1 `-32601` era-gate — VERS-04
- [ ] `resultType` v2-only injection test + v1 byte-identity snapshot — VERS-07
- [ ] centralized error-code table compile + frozen-`-32002`-unchanged test — VERS-06
- [ ] trace-context accessor + propagation test — VERS-09
- [ ] deliberately update `version.rs` `supports_four_versions_including_2024` test (tripwire — it MUST change when the supported set grows) — VERS-02

## Security Domain

`security_enforcement` is not set to `false` in config → treated as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | partial | This phase lays plumbing only; v2 task owner-binding/OAuth `sub` enforcement is Phase 114 (TASK-05). But the `client_info()` accessor surfaces *self-reported* identity — must be documented as untrusted (see threat table) |
| V3 Session Management | yes | v2 removes `Mcp-Session-Id`; this phase must NOT assume a session exists on the v2 path. Per-request signal authoritative (Pitfall 2/3) |
| V4 Access Control | deferred | Owner-binding is Phase 114; this phase must not introduce a session-id-keyed identity that Phase 114 would have to unwind |
| V5 Input Validation | yes | v2 header enforcement (D-05) + `Mcp-Method` vs body cross-check (D-06) — reject on desync (anti-smuggling) |
| V6 Cryptography | no | No crypto in this phase (MRTR `requestState` HMAC/AEAD is Phase 113) |

### Known Threat Patterns for dual-version MCP dispatch

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Header/body method desync (WAF sees `Mcp-Method: tools/list`, server runs `tools/call`) | Tampering / Elevation | D-06: cross-check `Mcp-Method` against JSON-RPC body `method`; reject on mismatch (fail closed) |
| Missing required v2 headers slipping through as under-routed requests | Spoofing / Tampering | D-05: strict 4xx reject on the v2 path when `Mcp-Method`/`Mcp-Name` absent |
| Trusting self-reported `_meta` `clientInfo` as an identity anchor | Spoofing | Surface `client_info()` as informational only; bind real identity to OAuth token, not `_meta` self-report (enforced in Phase 114; document the boundary here) |
| Silent v2 negotiation for legacy clients (LATEST flip) | Tampering (behavioral) | Pitfall 1: keep `LATEST` pinned; v2 opt-in; unknown version → V1 (conservative) |
| Session-keyed state leaking across the stateless boundary | Information Disclosure | Never key per-request identity by a (now-absent) session id on the v2 path; per-request signal only. (Full id-replay regression test is Phase 113/HTTP-05.) |

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection (this session, pmcp 2.17.0 tree): `src/types/protocol/version.rs` (constants + `negotiate_protocol_version`), `src/types/protocol/mod.rs` (`ProtocolVersion:28`, `ProtocolErrorCode:132`, `InvalidParams=-32602:135`, `RequestMeta:315`+flatten map`:344`, `ClientRequest:478` not-non_exhaustive), `src/types/capabilities.rs` (`extensions:109` + locking tests `:788-842`), `src/server/cancellation.rs:179` (native `RequestHandlerExtra`), `src/shared/cancellation.rs:50` (wasm mirror), `src/server/core.rs` (dispatch:1118, capabilities:210, frozen -32002:1145, with_request_meta:514), `src/server/mod.rs` (dispatch:1252, with_request_meta:1540), `src/server/task_dispatch.rs:576` + `task_dispatch_tests.rs:355` (frozen -32002 locking test), `src/shared/http_constants.rs`, `src/server/builder.rs`
- Milestone research pack (2026-07-22, HIGH): `.planning/research/SUMMARY.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/PITFALLS.md`
- `.planning/REQUIREMENTS.md` (VERS-01..09 full text + Out of Scope), `.planning/phases/112-version-plumbing-spine/112-CONTEXT.md` (D-01..D-11)

### Secondary (MEDIUM confidence)
- Official Rust SDK opt-in model — `serve_with_lifecycle` / `ClientLifecycleMode::Discover` / `preferred_versions` [github.com/modelcontextprotocol/rust-sdk] (WebSearch, 2026-07-22)
- SEP-2164 error-code clarification (resource-not-found -32002→-32602) — [4sysops MCP 2026-07-28 overview], [MCP RC blog] (WebSearch, 2026-07-22)

### Tertiary (LOW confidence — flagged for validation)
- Exact v2 error-code values / any wire-exact 2026-07-28 detail — final schema.json publishes 2026-07-28, NOT available at research time. VERS-06 mandates structure-only until then.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new runtime deps, corroborated by all four milestone research passes + direct Cargo.toml/tree inspection
- Architecture / integration points: HIGH — every file:line re-verified against the 2.17.0 tree this session
- Pitfalls: HIGH on codebase claims (frozen -32002, LATEST fallback, non_exhaustive enums, dual dispatch sites — all grep-verified); MEDIUM on spec-value claims (final spec 6 days out)
- Error-code v2 values (VERS-06): LOW by design — deferred to final schema.json, structure-only this phase

**Research date:** 2026-07-22
**Valid until:** 2026-07-29 (7 days — fast-moving; re-check the day execution begins because the final 2026-07-28 spec/schema.json lands inside this window and directly affects VERS-06)
