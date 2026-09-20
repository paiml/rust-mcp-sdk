# Stack Research

**Domain:** Dual-version MCP SDK (Rust) — adding MCP spec 2026-07-28 (v2) support to pmcp 2.17.0 alongside the existing 2025-11-25 stack
**Researched:** 2026-07-22
**Confidence:** HIGH

## Executive Bottom Line

**This milestone needs essentially ZERO new runtime crate dependencies.** Every v2-spec
feature maps to a crate pmcp already vendors, a hand-rolled logic/field change, or a
CI-only (non-runtime) tool. The four investigated questions all resolve in favour of the
minimal-dependency, wasm-clean posture the SDK already holds:

| Question | Answer | New crate? |
|----------|--------|------------|
| (a) JSON Schema 2020-12 validation | `jsonschema` is **already a dependency** (0.46.1, behind `validation`, `default-features=false`). Bump to 0.48.x, pin draft explicitly. | **No** (version bump only) |
| (b) Official conformance suite | It's a **TypeScript/Node.js** tool (`modelcontextprotocol/conformance`), driven in CI against a pmcp server binary / client command. CI/dev tool, not a Rust dependency. | **No** (Node toolchain in CI) |
| (c) Required headers + session-id removal | `http_constants.rs` + the existing hyper/axum/tower stack already carry `Mcp-Session-Id`/`Mcp-Protocol-Version`. Add 3 header **constants** + negotiation logic. | **No** |
| (d) RFC 9207 `iss` validation / DCR `application_type` | pmcp's OAuth is **hand-rolled** (`src/client/oauth.rs`, `src/server/auth/`), not the `oauth2`/`openidconnect` crates. Both SEPs are a query-param comparison + one serde field. | **No** — and actively AVOID adding an OAuth crate |

## Recommended Stack

### Core Technologies (all already present — changes noted)

| Technology | Current → Target | Purpose | Why Recommended |
|------------|------------------|---------|-----------------|
| `jsonschema` | **0.46.1 → 0.48.x** (`default-features = false`, behind `validation`) | Compile + validate `inputSchema`/`outputSchema` and `structuredContent` as full JSON Schema 2020-12 | Already vendored; supports Draft 2020-12; `default-features=false` **already disables HTTP+file `$ref` resolution**, which is exactly the SEP-2106 security mandate ("implementations MUST NOT auto-dereference external `$ref` URIs"). Wasm-clean in this config. No alternative crate is competitive in Rust. |
| `serde` / `serde_json` | 1.0 (unchanged, `raw_value` + `preserve_order`) | New protocol types: `InputRequiredResult`, `resultType`, per-request `_meta` keys, `CacheableResult` (`ttlMs`/`cacheScope`), `server/discover` result, `tasks/update` | Already the protocol-type backbone; `preserve_order` matters for the new "deterministic `tools/list` order" caching guidance. `structuredContent = any JSON value` is just `serde_json::Value` — a **relaxation**, no new machinery. |
| `hyper` / `hyper-util` / `axum` / `tower` / `tower-http` | unchanged (behind `streamable-http`) | Streamable-HTTP server/client: new required headers, stateless routing, `subscriptions/listen` POST stream | Already the transport stack. Header add/remove and the stateless code path are logic changes, not dependency changes. `tower-http`'s `set-header` feature (already enabled) covers server-side `serverInfo`/`ttlMs` response header stamping if desired. |
| `jsonwebtoken` | 10.3 (unchanged, behind `jwt-auth`) | Server-side bearer/JWT validation | No change needed for the auth-hardening SEPs — they touch the **client authorization-code flow** and **registration request body**, not JWT verification. |
| `http` | 1.1 (unchanged) | `HeaderName`/`HeaderValue` typing for the new headers | Already present; new header constants live in `src/shared/http_constants.rs`. |

### Supporting Libraries (already present — no additions)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `url` | 2.5 | Parse the OAuth callback query string to extract + validate the `iss` param (RFC 9207 / SEP-2468) | Already used in `src/client/oauth.rs` callback parsing (line ~697); `iss` validation is an added comparison, not new code infrastructure |
| `urlencoding` / `base64` / `sha2` | 2.1 / 0.22 / 0.11 | PKCE, `requestState` opaque-token encode/decode for MRTR (SEP-2322) | `requestState` is a server-encoded opaque blob echoed by the client — base64+serde_json is sufficient; no signing crate required for the reference impl |
| `indexmap` | 2.10 | Deterministic `tools/list` ordering (minor change #3: "return tools in deterministic order for prompt-cache hits") | Already used for ordered maps; keeps list output stable across calls |
| `garde` | 0.23 (behind `validation`) | Complements `jsonschema` for derive-style struct validation | Already paired with `jsonschema` in the `validation` feature; unchanged |

### Development / CI Tools (ONE genuinely new tool — CI-only, not a crate)

| Tool | Purpose | Notes |
|------|---------|-------|
| **`@modelcontextprotocol/conformance`** (Node.js/TypeScript, from `github.com/modelcontextprotocol/conformance`) | The **official** cross-SDK conformance gate for SDK-tier certification. Drives a pmcp **server HTTP endpoint** (server mode) or a pmcp **client command** (client mode); passes scenario context via `MCP_CONFORMANCE_SCENARIO` / `MCP_CONFORMANCE_CONTEXT` env vars; emits `checks.json` pass/fail + a `conformance-baseline.yml` for known-fails | **Node.js 20+ / LTS 22.x** must be available in CI. Invoke via the project's GitHub Action (`mode` + `url`/`command`). Scenarios are **TypeScript modules** in `src/scenarios/`, not YAML — you do **not** author them; you point the runner at pmcp. This is a `[dev]`/CI concern with **zero impact on the published crate or its dependency tree or wasm builds**. |
| pmcp conformance server binary (new example) | A small pmcp HTTP server the suite can boot (dual-version: negotiates 2025-11-25 and 2026-07-28) | Ship as an `examples/` binary (or a `mcp-tester`/`cargo-pmcp` subcommand). Reuses the existing `streamable-http` stack; no new deps. |
| Existing Rust wire-level conformance harness (Phase 109, `crates/pmcp-team-servers/src/conformance/runner.rs` + `crates/mcp-tester/src/conformance/`) | In-process/HTTP fixture replay (fixture schema v2) — **complements** the official suite for fast, offline, deterministic Rust-native checks | Keep and extend with v2 fixtures. It is NOT a substitute for the official Node suite (which is the tier gate), but it is the fast inner-loop gate. |

## Installation

pmcp is a Rust workspace, not npm. Concretely:

```toml
# Root Cargo.toml — the ONLY dependency-line change this milestone requires:
# jsonschema = { version = "0.46", optional = true, default-features = false }
jsonschema = { version = "0.48", optional = true, default-features = false }
#            ^^^^^^ bump 0.46 → 0.48 (Draft 2020-12 stable, same feature flags,
#                   same wasm-clean default-features=false posture)
```

```bash
# CI only — the official conformance gate (NOT added to Cargo.toml):
#   .github/workflows/*.yml
#   - uses: actions/setup-node@v4  with node-version: 22
#   - uses: modelcontextprotocol/conformance/action@main
#     with: { mode: server, url: "http://127.0.0.1:PORT/mcp" }
```

No `serde`, `hyper`, `axum`, `jsonwebtoken`, `url`, or auth-crate additions. New protocol
types and header constants are net-new **source**, not net-new **dependencies**.

## Per-Question Detail

### (a) JSON Schema 2020-12 — version bump, not a new crate

- **Already present:** `jsonschema = { version = "0.46", optional = true, default-features = false }` behind the `validation` feature; used today in `src/server/output_validation.rs` via `jsonschema::validator_for(schema)`.
- **Draft 2020-12 support:** confirmed for 0.48.x (latest is 0.48.5, published 2026-07-22). Bump 0.46 → 0.48.
- **Security match (SEP-2106):** the spec mandates "implementations MUST NOT auto-dereference external `$ref` URIs." pmcp **already** sets `default-features = false`, which disables both HTTP and file `$ref` resolution by default (you'd have to opt into `resolve-http`/`resolve-file` to get network fetches). **No change needed — the current config is already compliant.**
- **`structuredContent` = any JSON value:** a relaxation. Today's structured-output bridge (2.15) validates against `outputSchema`; under v2 `structuredContent` may be any JSON. This is a validation-loosening in `output_validation.rs`/`task_dispatch.rs`, not a crate change.
- **Recommended code change:** replace `jsonschema::validator_for(schema)` (auto-detect draft from `$schema`) with an **explicit** pin so v2 tools behave predictably regardless of whether the tool author supplies `$schema`:
  ```rust
  jsonschema::options()
      .with_draft(jsonschema::Draft::Draft202012)
      .build(schema)
  ```
  (or `jsonschema::draft202012::new(schema)`). Explicit pinning also future-proofs against the crate changing its default-draft fallback.
- **Composition-keyword resource bounds (SEP-2106):** the spec asks implementations to bound `oneOf`/`anyOf`/`allOf` nesting depth to prevent DoS. `jsonschema` compiles these fine; the *bound* is a pmcp-side pre-compile schema-depth/size check — **SDK logic, no crate.**
- **Wasm:** `jsonschema` with `default-features = false` compiles on `wasm32-unknown-unknown` (confirmed). The `validation` feature is not in the default build, so the reqwest-free/wasm-clean default publish build is unaffected either way.

### (b) Official conformance suite — CI/dev tool (Node.js), not a Rust dependency

- **What it is:** `github.com/modelcontextprotocol/conformance` — a TypeScript/Node.js framework (≈98% TS). Scenarios are TypeScript modules in `src/scenarios/` implementing a `Scenario` interface (`start()`/`stop()`/`getChecks()`); a built-in registry covers initialize, tools, resources, prompts, and auth flows.
- **How a Rust SDK runs against it:** the suite orchestrates. In **server mode** it connects as an MCP client to a running pmcp HTTP endpoint (URL passed as arg); in **client mode** it starts a test server and launches a pmcp client **command**. Scenario context arrives via `MCP_CONFORMANCE_SCENARIO` / `MCP_CONFORMANCE_CONTEXT` env vars.
- **What pmcp must provide:** (1) a runnable dual-version **server binary** exposing a Streamable-HTTP endpoint, and/or (2) a **client command**; (3) an optional `conformance-baseline.yml` recording known/allowed failures during the dual-version transition; (4) a CI job (their GitHub Action) with `mode` + `url`/`command`.
- **Transports tested:** HTTP/URL-based server + command-based client. Aligns with pmcp's `streamable-http`.
- **Tier significance:** per the RC post, a Standards-Track SEP can't reach Final until a matching scenario lands in the suite; Tier-1 SDKs must ship support within the validation window. Passing this suite is the external certification gate.
- **Stack implication:** add **Node.js (LTS 22.x)** to the CI image and a conformance job. **No Rust crate, no runtime dep, no wasm impact.** Keep the Phase-109 Rust harness as the fast offline inner loop.

### (c) HTTP-layer: required headers + session-id removal — constants + logic, no crate

- **Already present:** `src/shared/http_constants.rs` defines `MCP_SESSION_ID = "mcp-session-id"` and `MCP_PROTOCOL_VERSION = "mcp-protocol-version"`; `src/shared/streamable_http.rs` already reads/writes both.
- **Add (constants only):**
  - `MCP_METHOD = "mcp-method"` and `MCP_NAME = "mcp-name"` (SEP-2243 required POST headers).
  - `X_MCP_HEADER` prefix support (`x-mcp-header`) for custom headers from tool params (SEP-2243).
  - Reject requests where header ↔ JSON-RPC body disagree (`HeaderMismatchError`, now `-32020`).
- **Session-id removal (SEP-2567/SEP-2575):** a **negotiation code path**, not a dependency change. When the negotiated version is 2026-07-28: stop minting/reading `Mcp-Session-Id`, drop the `initialize`/`initialized` handshake, and require per-request `_meta` (`io.modelcontextprotocol/protocolVersion`, `io.modelcontextprotocol/clientCapabilities`, `io.modelcontextprotocol/clientInfo`). The existing `stateless()` mode is the natural home; the session machinery (`session.rs`, `event_store.rs`) stays for the 2025-11-25 path.
- **`Last-Event-ID` / SSE resumability removal (SEP-2575):** delete the code path for v2; keep the `LAST_EVENT_ID` constant for v1. No dep change.
- **`subscriptions/listen` (SEP-2575):** replaces the HTTP GET endpoint + `resources/subscribe`/`unsubscribe` with a single long-lived POST-response stream. Still hyper/axum SSE-style streaming — no new crate, a new route + handler.
- **Stack implication:** the hyper/hyper-util/axum/tower/tower-http stack is unchanged. `tower-http`'s already-enabled `set-header` feature suffices for stamping response-side `_meta`/cache headers. **No new crate.**

### (d) RFC 9207 `iss` validation + DCR `application_type` — hand-rolled, and AVOID OAuth crates

- **pmcp's auth is hand-rolled and transport-agnostic:** client flow in `src/client/oauth.rs` (discovery via `.well-known/openid-configuration` + `.well-known/oauth-authorization-server`, DCR, authorization-code + device-code, PKCE via `src/shared/pkce.rs`); server side in `src/server/auth/` (`oauth2.rs`, `jwt_validator.rs`, providers). The `oauth2` 5.0.0 crate in `Cargo.lock` is a **transitive** dependency of other tooling, **not** used by pmcp's auth.
- **RFC 9207 / SEP-2468 (`iss` validation):** the callback handler in `src/client/oauth.rs` (~line 697) currently parses `code` + `state` from the redirect query but does **not** read `iss`. The change: extract the `iss` query param and, if present, string-compare it against the recorded/discovered issuer before redeeming the code; reject on mismatch. **Pure logic on `url::Url::query_pairs()` — no crate.**
- **DCR `application_type` / SEP-837:** add an `application_type` field (default `"native"` for CLI/desktop redirect-to-localhost clients, `"web"` where applicable) to the client-registration request body constructed in `src/server/auth/oauth2.rs` (the `register_client` path / registration request struct). **One serde field — no crate.**
- **Issuer-keyed credential binding (SEP-2352):** persist DCR credentials keyed by issuer identifier and re-register when the AS changes. This is a storage-key change in the credential cache (`dirs`-based store already exists behind `oauth`). **No crate.**
- **DCR deprecation → Client ID Metadata Documents (PR #2858):** the spec now prefers CIMD over RFC 7591 DCR. This is additive discovery/registration logic (fetch a client-id metadata document by URL) — still `reqwest`/`serde_json` behind the existing `oauth` feature. **No new crate.**
- **Why NOT add `oauth2` / `openidconnect` crates:** they would (1) duplicate/fight the existing hand-rolled, transport-agnostic flow; (2) pull `reqwest` and other **wasm-unfriendly** deps into a path pmcp keeps reqwest-free by default; (3) impose their own type system on protocol structs that must match the MCP schema exactly. The SEP changes are trivially additive to the existing code. **Recommend explicitly against.**

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Bump existing `jsonschema` 0.48 | `boon`, `valico`, hand-rolled validator | Never for this milestone. `jsonschema` is already vendored, supports 2020-12, is wasm-clean with `default-features=false`, and already satisfies the no-external-`$ref` mandate. Switching costs churn for zero gain. |
| Official Node conformance suite in CI + keep Rust Phase-109 harness | Rust-only conformance (Phase-109 harness alone) | The Rust harness is the fast inner loop, but it is **not** the tier-certification gate. You need the official Node suite for Tier-1 status; run both. |
| Hand-rolled `iss`/`application_type` additions | `oauth2` 5.x / `openidconnect` crates | Only if pmcp ever abandons its hand-rolled, wasm-clean, transport-agnostic auth design — not this milestone. These crates are reqwest-coupled and not wasm-default-clean. |
| New header **constants** in `http_constants.rs` | A dedicated header-modeling crate | Never — `http` 1.1 + string constants is the established pattern; a new crate is overkill. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `oauth2` / `openidconnect` crates for the auth-hardening SEPs | Duplicates the hand-rolled flow; pulls `reqwest`; not wasm-default-clean; imposes non-MCP type shapes | Add `iss` comparison + `application_type` serde field to existing `src/client/oauth.rs` / `src/server/auth/oauth2.rs` |
| Enabling `jsonschema` `resolve-http`/`resolve-file` features | Would auto-dereference external `$ref` URIs — a **direct SEP-2106 security violation** and a wasm-breaker | Keep `default-features = false` (current, compliant). Enforce composition-depth bounds in SDK code. |
| A second/alternate JSON-Schema crate | Duplicate dependency, larger tree, no 2020-12 advantage over the incumbent | Bump the incumbent `jsonschema` to 0.48 |
| Authoring the conformance suite scenarios in Rust/YAML | The official suite scenarios are TypeScript modules you don't author; you only provide the endpoint/command | Point the official Node runner at a pmcp server binary; use `conformance-baseline.yml` for transition-period known-fails |
| Removing the 2025-11-25 session/event-store code | Milestone is **dual-version** (per-request negotiation, no hard cutover) | Gate v2 behavior on negotiated version; keep `session.rs`/`event_store.rs` for the v1 path |

## Stack Patterns by Variant

**If building the dual-version negotiation core (version plumbing phase):**
- Add header constants (`MCP_METHOD`, `MCP_NAME`) + per-request `_meta` protocolVersion/clientCapabilities/clientInfo parsing + `server/discover` RPC.
- No new deps; pure `serde_json` + `http` work. Error renumbering (`-32002`→`-32602`, and `-32001`/`-32003`/`-32004`→`-32020`/`-32021`/`-32022`) is a constants change.

**If building the JSON Schema 2020-12 phase:**
- Bump `jsonschema` 0.46→0.48; pin `Draft::Draft202012` explicitly; loosen `structuredContent` to any JSON value; add composition-depth bound check. Keep `default-features=false`.

**If building the conformance phase:**
- Add Node.js LTS 22.x + the official Node suite to CI, plus a dual-version pmcp server example binary. Extend the Phase-109 Rust harness with v2 fixtures for the offline inner loop.

**If building the auth-hardening phase:**
- Hand-roll `iss` validation in the callback parser, `application_type` in DCR, issuer-keyed credential storage, optional CIMD discovery. Zero new crates. All behind the existing `oauth` feature (already reqwest-gated / non-wasm).

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `jsonschema@0.48` | `serde_json@1.0` | Validates `serde_json::Value` directly; drop-in for the current 0.46 call sites. API used (`options().with_draft().build()` / `validator_for`) is stable across 0.46→0.48. |
| `jsonschema@0.48` (`default-features=false`) | `wasm32-unknown-unknown` | Compiles wasm-clean without default features (no external `$ref` resolvers). Matches pmcp's current gating. |
| Official conformance suite (Node) | pmcp `streamable-http` server | Drives HTTP endpoint / client command via env-var scenario context; unrelated to the Rust dependency tree. Requires Node.js ≥20 (use LTS 22.x) in CI. |
| `jsonwebtoken@10.3` | unchanged | Auth-hardening SEPs do not touch server-side JWT verification; no bump needed. |

## Wasm Compatibility Summary (per the SDK's wasm-clean constraint)

| Change | Wasm impact |
|--------|-------------|
| `jsonschema` 0.46→0.48 (`default-features=false`, behind `validation`) | Wasm-clean; not in default build regardless |
| New protocol types / header constants (`serde`, `http`) | Wasm-clean (these deps are already normal cross-target deps) |
| Streamable-HTTP header/session changes (`hyper`/`axum`/`tower`) | Server-side, native-only (already `#[cfg(not(wasm32))]`); no new wasm surface |
| OAuth `iss`/`application_type` (behind `oauth` feature) | `oauth` feature is already reqwest-gated and non-default/non-wasm; no change |
| Node conformance suite | CI-only; zero wasm impact |

**Net:** the default publish build stays reqwest-free and wasm-clean. The only dependency-line change (`jsonschema` bump) is feature-gated and already wasm-compatible in its configured form.

## Sources

- https://modelcontextprotocol.io/specification/draft/changelog — authoritative 2026-07-28 change list (SEP-2567 sessionless, SEP-2575 stateless core/`server/discover`/`subscriptions/listen`, SEP-2663 Tasks extension, SEP-2322 MRTR/`InputRequiredResult`/`resultType`, SEP-2243 required headers/`x-mcp-header`, SEP-2549 `ttlMs`/`cacheScope`, SEP-2106 JSON Schema 2020-12/`$ref` bounds, SEP-2468 RFC 9207 `iss`, SEP-837 DCR `application_type`, SEP-2352 issuer-keyed creds, PR #2858 CIMD, error-code renumbering) — **HIGH**
- https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/ — RC post: conformance-suite-as-gate, SDK tiering + validation window, stateless/MRTR/auth-hardening summary — **HIGH**
- https://github.com/modelcontextprotocol/conformance — official conformance suite: TypeScript/Node, server-mode URL / client-mode command, `MCP_CONFORMANCE_*` env, `checks.json`, `conformance-baseline.yml` — **HIGH**
- https://modelcontextprotocol.io/community/sdk-tiers — SDK tiering system context — **MEDIUM** (search-surfaced, corroborated by RC post)
- https://docs.rs/jsonschema/0.48.5 + `cargo search jsonschema` — 0.48.5 latest (2026-07-22), Draft 2020-12 supported, `default-features=false` disables HTTP+file `$ref` resolution, wasm32 supported with default features off, explicit-draft via `options().with_draft(Draft::Draft202012)` / `draft202012::new` — **HIGH**
- Local codebase inspection (root `Cargo.toml`; `src/shared/http_constants.rs`; `src/shared/streamable_http.rs`; `src/server/output_validation.rs`; `src/server/auth/oauth2.rs`; `src/client/oauth.rs`; `crates/pmcp-team-servers/src/conformance/runner.rs`; `crates/mcp-tester/src/conformance/`) — confirms `jsonschema` already vendored (`default-features=false`), headers already modeled, auth hand-rolled (not `oauth2`/`openidconnect`), Phase-109 Rust harness present — **HIGH**

---
*Stack research for: dual-version MCP 2026-07-28 (v2) support in the pmcp Rust SDK*
*Researched: 2026-07-22*
