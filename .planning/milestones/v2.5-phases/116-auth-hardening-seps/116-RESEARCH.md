# Phase 116: Auth Hardening SEPs - Research

**Researched:** 2026-08-02
**Domain:** OAuth 2.1 client hardening (RFC 9207 / 8414 / 7591 + OIDC DCR) in a hand-rolled, wasm-conscious Rust SDK
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

Copied verbatim from `116-CONTEXT.md` § Implementation Decisions. **Where this research
found evidence that a decision's stated premise does not hold, it is flagged inline and
enumerated in `## Decisions Requiring Amendment`. The decision itself is NOT overridden here —
only the planner (or a return to `/gsd:discuss-phase`) may change it.**

**AUTH-01 — RFC 9207 `iss` validation**

- **D-01: Strictness is a hybrid of a floor and a trigger, NOT an era check.** The OAuth flow runs *before* any MCP connection exists, so `Client::era()` (`src/client/mod.rs:669`) is not available at callback time. Instead:
  - **Floor (unconditional, both eras, no config):** if `iss` is present in the callback, always validate it. This cannot break any existing deployment — a v1 AS that never sends `iss` never trips it — so v1 gets *strictly safer*, not merely unchanged.
  - **Trigger (require `iss` to be present):** the RFC 9207 discovery flag `authorization_response_iss_parameter_supported`, **or** an explicit caller override. `OidcDiscoveryMetadata` (`src/server/auth/oauth2.rs:172`) does not parse this field today — add it additively.
- **D-02: Compare against `metadata.issuer`, exact string match.** The anchor is the value the AS itself published in its discovery document — *not* `config.issuer` (a user-typed discovery seed) and *not* the effective issuer at `src/client/oauth.rs:505`. The mix-up attack being defended against is "this response came from a different AS than the one whose metadata I fetched," so the discovered issuer is the semantically correct anchor. **No trailing-slash normalization** — RFC 9207 says exact, and conformance will test exact.
- **D-03: Failure surfaces via the Phase-113 marker pattern, not a new `Error` variant.** `Error` is a plain `thiserror` enum with **no** `#[non_exhaustive]`, so a new variant is semver-major. Follow `RETIRED_ON_V2_MARKER` / `MRTR_ROUND_LIMIT_MARKER` (`src/error/mod.rs:114-131`) exactly: an `ISS_MISMATCH_MARKER` const + an `Error::iss_mismatch(expected, actual)` constructor + an `Error::is_iss_mismatch()` predicate, riding on the existing `Authentication` variant's `data.pmcpError`. Gives conformance fixtures and downstream callers a stable programmatic discriminator instead of message substrings. The authorization code is **never redeemed** on failure; the existing failure HTML is unchanged.
- **D-04: Override is a builder method plus an env var.** `OAuthHelper::with_iss_validation(…)` as an **inherent method** (semver-minor — deliberately NOT a field on `OAuthConfig`), plus a `PMCP_OAUTH_ISS_VALIDATION` env var so an operator can act without a redeploy, matching the house env-var config-injection philosophy. **Precedence: env var > builder > discovery flag.**

**Platform seam**

- **D-05: The hardened logic lands as transport-free primitives; the interactive CLI flow is one caller, not the only caller.** `OAuthHelper::authorization_code_flow` calls `webbrowser::open()` (`src/client/oauth.rs:718`) and binds a loopback `TcpListener` — a Lambda or Workers `oauth-proxy` can do neither. So: `iss` validation lands as a **pure function** over (query params, discovered metadata) that the loopback listener and a platform redirect handler both call; credential storage lands behind a **trait**, not a hardcoded path. Reshapes *where* code lands. No new backends are built, no server-side change, no behavior change for a server behind a platform gateway.
- **D-06: The primitives are wasm-clean and live OUTSIDE the `oauth` feature gate.** `src/client/oauth.rs` is gated `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]` (`src/client/mod.rs:46`) and `oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]` (`Cargo.toml:216`) — so a Cloudflare Workers platform gets **zero** of it. The pure comparison function and the store trait need none of those deps. They go in a module with no wasm32 exclusion and no reqwest/webbrowser/dirs dependency. The browser flow, loopback listener and file store stay behind `oauth` + `not(wasm32)` as the default callers. **Needs a wasm32 build fence in CI** or this silently regresses.
- **D-07: The credential store is keyed by `(issuer, opaque account scope)`.** Async trait, **I/O-free construction** (no `std::env` reads, no network, no disk in the constructor — all values are constructor parameters). The account scope is caller-supplied and **never interpreted by the SDK**: a Cognito sub, a tenant id, or empty for the single-user CLI. This satisfies SEP-2352's "never reuse credentials across ASes" literally while letting a multi-tenant platform key per user without the SDK dictating identity. Ships a file impl and an in-memory impl; DynamoDB/KMS impls stay with the platform.
- **D-08: Headless operation is an explicit opt-in mode, not environment sniffing.** A builder selection (`Interactivity::RefreshOnly` or equivalent) under which refresh failure returns the typed reauth-required error **immediately** and the browser/loopback path is **unreachable by construction**. Today `get_access_token()` (`src/client/oauth.rs:428-480`) silently falls through on refresh failure into `authorization_code_flow`, binding a listener nothing can reach and waiting **5 minutes** — five minutes of burned wall clock per attempt in a Lambda. The existing constructor keeps today's interactive fall-through, so no current caller changes behavior.

**AUTH-02 — DCR `application_type`, without a major bump**

- **D-09: Typed accessors over the existing `#[serde(flatten)] extra` map — not a new field.** `DcrRequest` (`src/server/auth/provider.rs:304`) is fully public (`pub mod auth` at `src/server/mod.rs:48` → `pub mod provider` at `auth/mod.rs:55`, plus a `pub use` re-export at `auth/mod.rs:86`), all-pub-field, **not** `#[non_exhaustive]`, with 10 struct-literal construction sites in-repo. Adding a field is `constructible_struct_adds_field` = **major** under `cargo semver-checks`, which has been clean 223/223 through Phase 115 and which the milestone's additive-2.x-minor constraint depends on. Add **inherent methods** (`application_type()` / `set_application_type()` on `DcrRequest`, `application_type()` on `DcrResponse`) that read and write the `extra` map. Inherent methods are semver-minor and the wire bytes are identical to a real field. **Requires a documented precedence rule** for a caller who also writes the raw `"application_type"` key by hand, plus a collision test.
- **D-10: The value is DERIVED from `redirect_uris`.** Loopback (`127.0.0.1` / `::1` / `localhost`) or a custom scheme ⇒ `"native"`; https non-loopback ⇒ `"web"`. This keeps it consistent with the OIDC rule that constrains the two together and stays correct if `redirect_port` changes. **A mixed `redirect_uris` vec is an explicit ERROR, never a silent pick.** The explicit setter from D-09 remains available as an override. pmcp's own DCR call hardcodes `http://127.0.0.1:{port}/callback` (`src/client/oauth.rs:239`) ⇒ derives `native`; a platform `oauth-proxy` with an https redirect ⇒ derives `web`.
- **D-11: Sent unconditionally on both eras; echo mismatch is recorded and warned, never fatal.** `application_type` has been a standard OIDC Dynamic Registration field since 2014 — an AS that doesn't want it ignores it, and era-gating would require plumbing an era into DCR that (like the callback, D-01) does not exist pre-connection. On the response, record what the AS actually registered and `tracing::warn!` on divergence, but **never fail the registration** — RFC 7591 explicitly permits the AS to modify requested metadata. Keeps AUTH-02 a pure addition with no v1 breakage surface.

**Adjacent gaps accepted into scope**

- **D-12: OAuth `state` validation is IN SCOPE (CSRF).** At `src/client/oauth.rs:712` the state value is generated **inline as a temporary** — `.append_pair("state", &Self::generate_code_verifier())` — never bound to a variable, so it is not merely unchecked but *structurally impossible* to check. The callback extracts `code` only. Bind it, retain it across the flow, compare before redeeming. `iss` and `state` defend the same mix-up/CSRF family and land on the same lines; shipping RFC 9207 validation while leaving `state` unvalidatable would be indefensible in review or in a conformance claim.
- **D-13: SEP-2351 is a CODE fix, not documentation.** `generic_oidc.rs:394` and `cognito.rs:270` build discovery URLs by naive concatenation — `format!("{}/.well-known/openid-configuration", issuer)` — while RFC 8414 §3.1 requires **inserting** the well-known segment between host and path. Any issuer with a path component (`https://host/tenant1` ⇒ `https://host/.well-known/oauth-authorization-server/tenant1`) resolves to the wrong URL today, which breaks multi-tenant IdPs.
- **D-14: SEP-2207 is a real defect cluster, not documentation.** `refresh_token()` (`src/client/oauth.rs:916-949`) has three genuine bugs, all of which directly block D-08's headless mode from actually working:
  1. **The stored refresh token is destroyed on every successful refresh** against an AS that doesn't re-issue one. `TokenResponse.refresh_token` is `#[serde(default)]`, so an omitted field deserializes to `None`, and `cache_token` writes that `None` over the good token (`:987`). Many OIDC ASes omit it meaning "keep the old one." An unattended agent gets exactly one refresh cycle, then a forced re-login. **Fix: preserve the stored token when the response omits one.**
  2. **DCR flows can never refresh at all.** `client_id` is read from `self.config.client_id` (`:922`), but under DCR the client_id is *issued* and lives in `AuthorizationResult`, never in config — so it errors `"cannot refresh token without a cached client_id"`. **Fix: source `client_id` from the D-07 issuer-keyed store**, which holds it per `(issuer, account)`.
  3. **`scope` is never sent on refresh**, which some OIDC ASes require or use to narrow.
- **D-15: D-113-V's 31 unbounded auth-surface reads are IN SCOPE.** Bound the reviewed-unbounded whole-body reads — `providers/generic_oidc.rs` (11), `providers/cognito.rs` (9), `client/oauth.rs` (6), `client/auth.rs` (5) — and **widen the existing tripwire's SCOPE FENCE** to cover those four files. Roadmap-assigned to this phase with Status **OPEN**; the fix shape and the tripwire both already exist from Phase 113.1; these are the exact files this phase edits. Note the tripwire's scanner already *would* find all of them (it strips whitespace and handles rustfmt-broken chains, pinned by a test at `v2_bounded_reads_tripwire.rs:1050`) — only the scope fence keeps them unreported. `refresh_token`'s own error path (`response.text().await.unwrap_or_default()`, `:941`) is one of these.

**AUTH-03 — issuer-keyed credential storage**

- **D-16: SEP-2352's two mandates fall out of the key shape, not enforcement code.** Because the key *includes* the issuer (D-07), "MUST NOT reuse credentials across authorization servers" is true by construction — a server that switches AS simply misses the cache. And because the DCR `client_id` lives in the same record (it must: a client_id issued by AS-A is meaningless at AS-B), "re-register on AS change" is automatic too.
- **D-17: On-disk migration is SPLIT, because the two existing caches are not equivalent.**
  - **pmcp core `~/.pmcp/oauth-tokens.json`** (`TokenCache`, `src/client/oauth.rs:151`) is a single flat token with **no issuer field at all**. It cannot be re-keyed without *guessing* which AS issued it — precisely what SEP-2352 forbids. **Discard on first read; one forced re-login.**
  - **cargo-pmcp `~/.pmcp/oauth-cache.json`** (`TokenCacheV1`, `cargo-pmcp/src/commands/auth_cmd/cache.rs:34`) is multi-entry, keyed by normalized server URL, and **records `issuer` per entry**. **Real `schema_version` 1→2 migration**, re-keying to `(issuer, account)` with account empty. Every existing login is preserved.
- **D-18: Track last-seen issuer per MCP server URL and warn loudly on change — non-blocking.** Issuer-keyed storage makes an AS substitution *safe* but **invisible**: the user is simply walked through a fresh login at an IdP they didn't expect. Record the last issuer seen per server and, on change, emit a prominent warning naming **both** old and new issuer before proceeding. An unattended agent still self-heals; under `RefreshOnly` (D-08) it surfaces as the typed reauth-required error with the issuer change named. Not a hard fail — legitimate issuer changes (tenant moves, provider migrations) do happen, and hard-failing converts a real operational event into an outage.
- **D-19: cargo-pmcp adopts core's store — converge on ONE store and ONE file.** Today a single machine can carry two unrelated OAuth caches with different formats and semantics. Core owns the trait and the default file impl; cargo-pmcp **drops its parallel `TokenCacheV1` implementation**, with its existing `oauth-cache.json` as the migration source and the surviving path. `cargo pmcp auth login/logout/token` become thin wrappers over the same seam a platform would implement. Costs a cargo-pmcp version bump and a dep pin; `auth logout` semantics must be preserved.

**Requirement booking**

- **D-20: AUTH-01/02/03 book `[x]` on measured evidence — NO publication hold, and `[~]` is not inherited.** These SEPs derive from published RFCs (9207, 7591, 8414) and published spec prose, with **no** dependency on `schema.json` or on the still-unpublished `ext-tasks` repo. The roadmap's `D-15` warns in as many words against inheriting `[~]` by habit. Follow Phase 115's discipline: each booking **CITES** the artifact plus a named test binary and count so a future reader can re-derive rather than trust, and the booking task runs **only AFTER** `make quality-gate` and the PR-blocking `pmat quality-gate --checks complexity` both exit 0.

### Claude's Discretion

> No area was delegated with "you decide." Every decision above was selected explicitly. Left to
> the planner: wave/plan decomposition, module naming and exact placement for the wasm-clean
> primitives, the mixed-`redirect_uris` error type, and the fuzz/property target design under the
> house ALWAYS requirements.

### Deferred Ideas (OUT OF SCOPE)

- **SEP-2350 step-up scope accumulation — deferred WHOLE, both halves together.** The server half is a `WWW-Authenticate: Bearer realm/scope/error=insufficient_scope` challenge builder; pmcp emits **zero** `WWW-Authenticate` anywhere today (one comment at `task_dispatch.rs:584`, no code). The client half (request the union of stored and newly-required scopes on re-auth) is implementable standalone but would have nothing to trigger it. Ships as one coherent feature in its own phase.
- **Extract `UpstreamAuthDecorator` + `HEADER_UPSTREAM_AUTH` into the SDK.** A standing request written into the durable agent's own source (its `D-12`) — the module was authored to be copy-pasted into `rust-mcp-sdk` and re-exported without rewriting call sites. Tiny, but new public surface unrelated to AUTH-01..03.
- **Extract the outbound-OAuth vending core.** The `OutboundOAuthCore` shape: per-server token vending, TTL cache, `OnceCell` inflight-dedup stampede prevention, `reauth_required` → `ConsentRequired` on both the discovery and tool-call paths. ~987 lines hand-rolled in the durable agent. **No phase exists for this** — `docs/design/agents-teams-sdk-extraction-plan.md`'s Phases A–F don't cover it, so it needs a roadmap slot rather than an assumption that Phase 117 absorbs it.
- **Cognito internal/external providers and the CognitoExternal→CognitoInternal fallback chain.** Platform-specific policy (AbsentCustody/RefreshRevoked ⇒ M2M bearer, InfraDenied ⇒ loud propagated error). Stays in pmcp.run.
- **Token-at-rest encryption in core.** The platform uses KMS; a plaintext `~/.pmcp` file is the status quo and this phase does not change it.
- **Whether the store trait carries token REFRESH itself or only load/save/delete.** Left open; the planner may settle it, but a deliberate answer belongs with the vending-core extraction.
- **Typed accessors for the other RFC 7591 fields `DcrResponse` currently drops into `extra`.** Same mechanism as D-09, but out of scope here.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **AUTH-01** | OAuth callback validates RFC 9207 `iss` (strict on v2, lenient on v1 to protect existing deployments) | The MCP draft spec publishes a **normative 4-row decision table** for exactly this (§ Authorization Response Validation) and it *matches D-01's floor+trigger* — see `## Code Examples` → "The normative `iss` decision table". RFC 9207 §2.4 supplies the comparison rule (RFC 3986 §6.2.1 simple string comparison) and the reject-and-do-not-redeem obligation. Zero `iss` handling exists in `src/` today (measured: `grep -rn '"iss"' src/` → 0 hits), so this is greenfield. **Blocking prerequisite discovered:** the anchor `metadata.issuer` is itself unvalidated today (RFC 8414 §3.3 check absent) — see Pitfall 1. |
| **AUTH-02** | Dynamic client registration sends/accepts `application_type` | Spec text is verbatim in `## Code Examples` → "SEP-837 normative text". `DcrRequest`/`DcrResponse` both already carry `#[serde(flatten)] extra` (`provider.rs:349`, `:380`), so D-09's accessor approach needs no struct change. Construction site is `src/client/oauth.rs:241-257`. `constructible_struct_adds_field` semantics confirmed against the cargo-semver-checks lint definition — D-09's reasoning is correct and **generalizes to two more structs this phase touches** (see `## Decisions Requiring Amendment` A1). |
| **AUTH-03** | Remaining auth-hardening SEPs (issuer-keyed credential storage + three clarifications) applied without breaking existing v1 OAuth deployments; no `oauth2`/`openidconnect` crates added | SEP-2352 spec text ("Authorization Server Binding") is verbatim in `## Code Examples`; it is a **MUST** for issuer-keying, confirming D-07/D-16. SEP-2351's real rule is a **3-URL probe sequence**, not a swap — see Pitfall 2, which includes a measured counter-example. SEP-2207's real content is `offline_access` + `grant_types` DCR metadata, which is *additional* to D-14's three defects — see `## Decisions Requiring Amendment` A3. The "no oauth2 crate" claim needs precise scoping: a **pre-existing direct `oauth2 = "5.0"`** already sits in `cargo-pmcp/Cargo.toml:84` — see Pitfall 6. |
</phase_requirements>

## Summary

Phase 116 is unusually well-specified going in: `116-CONTEXT.md` is a 20-decision document whose
code-level premises this research verified almost entirely correct. Every file path, line number
and structural claim in CONTEXT was checked against the tree and **19 of 20 decisions hold as
written**. The value this research adds is therefore not "what to build" — CONTEXT settled that —
but four categories the planner cannot discover from CONTEXT alone: (1) **four decision premises
that are factually wrong or incomplete**, each of which would produce a plan that ships a
regression or fails `cargo semver-checks`; (2) **the normative spec text**, which turns out to
publish a decision table and a URL-probe order that CONTEXT approximates rather than states;
(3) **three genuine security gaps adjacent to AUTH-01 that CONTEXT does not name at all**, one of
which makes the entire RFC 9207 check bypassable; and (4) **a measured gate blind spot** — `make
quality-gate` does not compile, lint or test a single line of `src/client/oauth.rs`.

That last finding is the one to internalize first. `full` (`Cargo.toml:205`) does **not** include
`oauth`. `make lint` and `make test` both pass `--features "full"`. `tests/oauth_dcr_integration.rs`
is `#![cfg(feature = "oauth")]`. Measured under `--features full`: `cargo nextest list -E
'binary(oauth_dcr_integration)'` returns **zero tests** and `-E 'binary(/oauth/)'` returns **zero
binaries**. An executor who writes the whole phase, runs `make quality-gate`, and sees exit 0 has
proven *nothing* about the code they wrote. CI's separate `test` job uses `--all-features` and does
cover it, but only after push, and with a different (narrower) clippy lint set than `make lint`.
Every plan verification block in this phase must explicitly pass `--features full,oauth`.

The strongest architectural news is that D-05/D-06's "wasm-clean primitives outside the `oauth`
gate" is not a new pattern to invent — `src/shared/pkce.rs` already **is** that pattern (ungated,
`getrandom`-based, crate-root re-exported, RFC-vector tested), and it already exports
`generate_state()`, the exact primitive D-12 needs. Its real consumer is a wasm browser client
(`examples/web-channel-client/`) that **already validates `state` correctly** — so the CLI flow is
the only caller missing the check, and the correct implementation is sitting in the repo.

**Primary recommendation:** Structure the phase as *one shared, wasm-clean validation module
modelled on `src/shared/pkce.rs`*, holding a per-request `AuthorizationRequestRecord` (issuer +
code_verifier + state, bound together as the spec requires) and a pure
`validate_authorization_response()` implementing the spec's 4-row table; wire the existing CLI
loopback callback as its first caller; and gate every plan's verification on `--features full,oauth`
plus an explicit `cargo semver-checks` run, because three of the structs this phase wants to extend
are semver landmines, not two.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| RFC 9207 `iss` comparison + `state` comparison | **Pure library (wasm-clean, ungated)** | — | Zero I/O; inputs are (query pairs, recorded record). Exactly the `src/shared/pkce.rs` tier. Must be callable by a Workers/Lambda redirect handler that has no `TcpListener`, per D-05/D-06. |
| Per-request record (issuer + verifier + state) | **Pure library (wasm-clean, ungated)** | — | The spec requires these three to live in **one** record (see Code Examples → "the record requirement"). A struct, not three locals. |
| Discovery URL candidate construction (SEP-2351) | **Pure library (wasm-clean, ungated)** | HTTP client (probing) | URL derivation is pure `url::Url` arithmetic and is the part worth property-testing; only the *probing* needs `reqwest`. Splitting them is what makes the MUST-ordered sequence testable without a network. |
| Discovery fetch + RFC 8414 §3.3 issuer validation | **Client HTTP (`oauth`/`http-client` feature, native)** | — | `OidcDiscoveryClient` (`src/client/auth.rs`) already owns this; the §3.3 check belongs immediately after deserialization, before the metadata escapes the function. |
| DCR request body composition (`application_type`, `grant_types`) | **Client HTTP (`oauth` feature)** | Pure library (derivation) | The `native`/`web` *derivation* from `redirect_uris` is pure and testable; the POST is not. |
| Credential store trait + in-memory impl | **Pure library (wasm-clean, ungated)** | — | D-07 mandates I/O-free construction and no `dirs`. A Workers platform must be able to implement it. |
| Credential store file impl | **Client (`oauth` feature, native)** | — | Needs `dirs` + `tokio::fs`; `~` is unwritable on Lambda/Workers, so this is explicitly the *default caller*, not the seam. |
| Browser launch + loopback listener | **Client CLI (`oauth` feature, native)** | — | `webbrowser` + `TcpListener`. Unchanged in kind; becomes *one* caller of the pure tier. |
| Bounded whole-body reads (D-15) | **Client HTTP + server auth providers** | Test tripwire | Fix lands per-read-site; closure is enforced by widening `tests/v2_bounded_reads_tripwire.rs`'s `EXTRA_SCOPE`. |
| cargo-pmcp `auth` subcommands | **CLI binary (separate crate)** | Core store trait | D-19 makes these thin wrappers. Separate crate ⇒ separate version bump and dep pin. |
| **MCP server side (resource server, JWT validation, `oauth_passthrough`)** | **UNTOUCHED** | — | Confirmed: none of the six SEPs push work into the resource server. A pmcp server behind pmcp.run's API-GW + proxy Lambdas is unaffected. |

## Standard Stack

**This phase adds no dependencies.** AUTH-03's success criterion forbids it and every capability
below is already present. The table records what to *use*, not what to install.

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `url` | 2.x (existing, non-optional) | `query_pairs()` percent-decoding for `iss`/`state`/`code`; path arithmetic for SEP-2351 candidates | Already the discovery/callback parser. `query_pairs()` performs the `application/x-www-form-urlencoded` decode RFC 9207 §2.4 *requires before comparison* — using it satisfies that clause for free. [VERIFIED: `src/client/oauth.rs:30`, RFC 9207 §2.4] |
| `serde_json` | 1.x (existing) | `#[serde(flatten)] extra` map read/write for `application_type` (D-09) | The carrier already exists on both `DcrRequest` and `DcrResponse`. [VERIFIED: `src/server/auth/provider.rs:349,380`] |
| `getrandom` | existing, non-optional | RNG for `state` in the wasm-clean tier | Already the RNG behind `src/shared/pkce.rs`; the reason that module is ungated. [VERIFIED: `src/shared/pkce.rs:1-20`] |
| `sha2` + `base64` | existing | S256 challenge; sha256-prefix token logging | Already used by `pkce.rs`. sha256-prefix is the platform's never-log-raw-tokens convention (D-07 input). |
| `tracing` | existing | D-11 echo-divergence warning; D-18 issuer-change warning | House logging. |
| `async-trait` | existing | The credential-store trait (D-07) | House convention for async traits (CLAUDE.md). |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `mockito` | 1.5.0 (dev-dep) | Mock AS: discovery, `/register`, `/token` | **The ready-made harness.** `tests/oauth_dcr_integration.rs` already has a `discovery_body(base, with_reg)` helper to copy. Use for every AUTH-01/02/03 integration test. [VERIFIED: `Cargo.toml:192`, `tests/oauth_dcr_integration.rs:11-32`] |
| `proptest` | 1.7 (dev-dep) | Property tests for iss-comparison invariants and URL-candidate derivation | House ALWAYS requirement. |
| `quickcheck` | 1.0 (dev-dep) | Alternative property harness | Both are present; `proptest` is the more used. |
| `tempfile` | 3.19 (dev-dep) + cargo-pmcp runtime | Store-impl tests; atomic writes | `cargo-pmcp`'s `TokenCacheV1::write_atomic` already uses `NamedTempFile::persist` — reuse that shape in the core file store. [VERIFIED: `cargo-pmcp/src/commands/auth_cmd/cache.rs:108-120`] |
| `cargo-fuzz` | 0.13.1 (installed) | Fuzz targets. `fuzz/fuzz_targets/auth_flows.rs`, `pkce_helper.rs`, `dcr_response_parser.rs` already exist | Extend an existing target rather than minting a new one where the surface fits. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled `iss`/`application_type`/store | `oauth2` 5.x / `openidconnect` | **Explicitly forbidden by AUTH-03 and by `research/STACK.md`.** They are reqwest-coupled, not wasm-default-clean, and impose non-MCP type shapes. Note `oauth2 = "5.0"` *already exists* as a direct dep of `cargo-pmcp` for pmcp.run **deployment** login — a different concern; see Pitfall 6. |
| New `Error` variant for iss/state failures | Marker-const on `Error::Protocol` | D-03 correctly rules out a new variant (major). But its stated carrier is wrong — see `## Decisions Requiring Amendment` A2. |
| New field on `OidcDiscoveryMetadata` for the RFC 9207 flag | New sibling type + new method | Adding the field is **major** — see A1. |
| Replacing append with insert in discovery URLs | Ordered 3-candidate probe | Replacing breaks Microsoft Entra ID; **measured** in Pitfall 2. |

**Installation:**

```bash
# Intentionally empty. AUTH-03 forbids new crates; every capability above is already vendored.
# The fence the planner should run instead:
git diff --stat <phase-base>..HEAD -- Cargo.toml Cargo.lock cargo-pmcp/Cargo.toml
# Expected for pmcp core: EMPTY. cargo-pmcp may change only its own version + pmcp pin (D-19).
```

**Version verification:** No package versions were looked up because no package is being added.
The dev-dependency versions above are transcribed from `Cargo.toml` lines 182-192 as they exist
in-tree. [VERIFIED: `Cargo.toml`]

## Package Legitimacy Audit

**This phase installs zero external packages.** Adding one would violate AUTH-03's success
criterion ("no `oauth2`/`openidconnect` crates are added") and the v2.5 roadmap non-goal of zero
new runtime dependencies.

| Package | Registry | Disposition |
|---------|----------|-------------|
| *(none)* | — | No package installed; slopcheck not applicable |

**Packages removed due to slopcheck [SLOP] verdict:** none — none proposed.
**Packages flagged as suspicious [SUS]:** none — none proposed.

**The audit that *is* required here is the inverse one** — proving nothing was added. Recommended
fence for the planner, which also catches the Pitfall 6 false alarm:

```bash
# 1. pmcp core dependency surface must be byte-unchanged.
git diff --exit-code <phase-base>..HEAD -- Cargo.toml Cargo.lock && echo "core deps unchanged"

# 2. No oauth2/openidconnect anywhere in pmcp core, at any point.
! grep -rn "openidconnect" Cargo.toml && \
! grep -rnE "^oauth2\s*=" Cargo.toml && echo "core is oauth2-free"

# 3. cargo-pmcp's PRE-EXISTING oauth2 dep is for pmcp.run DEPLOYMENT login, not MCP auth.
#    It must remain confined to deployment/, never reaching auth_cmd/.
! grep -rn "oauth2::" cargo-pmcp/src/commands/ && echo "auth_cmd is oauth2-free"
```

## Architecture Patterns

### System Architecture Diagram

```
                     ┌───────────────────────────────────────────────────────┐
                     │ CALLER TIER (one of N — D-05: CLI is not the only one)│
                     └───────────────────────────────────────────────────────┘
   ┌──────────────────────┐   ┌──────────────────────┐   ┌────────────────────────┐
   │ CLI loopback flow    │   │ Platform redirect    │   │ Browser/wasm client    │
   │ (oauth + !wasm32)    │   │ handler (Lambda/     │   │ examples/web-channel-  │
   │ webbrowser +         │   │ Workers oauth-proxy) │   │ client (already        │
   │ TcpListener          │   │ NO listener, NO      │   │ validates `state`)     │
   └──────────┬───────────┘   │ browser              │   └───────────┬────────────┘
              │               └──────────┬───────────┘               │
              │                          │                           │
              └──────────────┬───────────┴───────────────────────────┘
                             │  all three call the same pure functions
                             ▼
   ┌──────────────────────────────────────────────────────────────────────────────┐
   │ PURE VALIDATION TIER — ungated, wasm-clean, no reqwest/webbrowser/dirs        │
   │ (model: src/shared/pkce.rs)                                                  │
   │                                                                              │
   │  AuthorizationRequestRecord { expected_issuer, code_verifier, state }        │
   │        │  built BEFORE redirect (spec MUST: one record, not three locals)    │
   │        ▼                                                                     │
   │  validate_authorization_response(query_pairs, &record, iss_policy)           │
   │        ├── decode via url::query_pairs()  ── RFC 9207 §2.4 form-decode       │
   │        ├── state compare  ────────────────── CSRF (D-12)                     │
   │        ├── iss 4-row table ───────────────── RFC 9207 §2.4 + spec table      │
   │        │     simple string comparison, NO normalization                      │
   │        └── error-response handling ───────── MUST NOT display error on       │
   │                                              iss mismatch                     │
   │        ▼                                                                     │
   │  Ok(AuthorizationCode)  |  Err(Error::…iss_mismatch / state_mismatch)        │
   │                                                                              │
   │  discovery_url_candidates(issuer) -> [Url; 2..3]   ── SEP-2351, pure         │
   │  derive_application_type(&[redirect_uri]) -> native|web|Err(mixed) ── D-10   │
   └──────────────────┬───────────────────────────────────────────────────────────┘
                      │ code never redeemed unless validation returned Ok
                      ▼
   ┌──────────────────────────────────────────────────────────────────────────────┐
   │ HTTP TIER (oauth / http-client feature, native)                              │
   │                                                                              │
   │  OidcDiscoveryClient::discover(issuer)                                       │
   │    ├─ probe candidates IN ORDER (MUST)                                       │
   │    ├─ ★ RFC 8414 §3.3: metadata.issuer == issuer-used-to-build-URL           │
   │    │     ABSENT TODAY — without it the iss anchor is attacker-chosen         │
   │    └─ bounded body read (D-15)                                               │
   │                                                                              │
   │  DCR POST /register  ── + application_type (D-09/D-10)                       │
   │                      ── + grant_types ["authorization_code","refresh_token"] │
   │  token exchange / refresh ── + scope, + preserved refresh_token (D-14)       │
   └──────────────────┬───────────────────────────────────────────────────────────┘
                      ▼
   ┌──────────────────────────────────────────────────────────────────────────────┐
   │ CREDENTIAL STORE TIER — trait ungated & wasm-clean, impls tiered             │
   │   trait CredentialStore { load/save/delete by (issuer, account_scope) }      │
   │   I/O-FREE CONSTRUCTION (all values are ctor params — no std::env, no disk)  │
   │   ├── InMemoryStore  (ungated)                                               │
   │   ├── FileStore      (oauth + !wasm32, dirs + tokio::fs, 0o600 atomic)       │
   │   └── DynamoDB/KMS   (NOT BUILT HERE — platform implements the trait)        │
   └──────────────────────────────────────────────────────────────────────────────┘

   ┌──────────────────────────────────────────────────────────────────────────────┐
   │ UNTOUCHED: MCP server side — resource server, JWT validation, ProxyProvider, │
   │ oauth_passthrough. A pmcp server behind a platform gateway sees no change.   │
   └──────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
src/shared/
├── pkce.rs                  # EXISTING — ungated, wasm-clean. The MODEL to copy.
│                            #   already exports generate_state() (D-12 needs exactly this)
└── oauth_validation.rs      # NEW (name is planner's discretion) — same gating as pkce.rs:
                             #   AuthorizationRequestRecord, validate_authorization_response,
                             #   discovery_url_candidates, derive_application_type,
                             #   CredentialStore trait + InMemoryStore
src/client/
├── auth.rs                  # OidcDiscoveryClient — candidate probing + RFC 8414 §3.3 check
│                            #   + 5 bounded reads (D-15)
└── oauth.rs                 # OAuthHelper — becomes a CALLER of the pure tier.
                             #   D-04 builder, D-08 Interactivity, D-14 refresh fixes,
                             #   FileStore, 6 bounded reads (D-15)
src/server/auth/
├── provider.rs              # DcrRequest/DcrResponse inherent accessors (D-09)
└── providers/
    ├── generic_oidc.rs      # SEP-2351 call site + 11 bounded reads (D-15)
    └── cognito.rs           # SEP-2351 call site + 9 bounded reads (D-15)
tests/
├── v2_bounded_reads_tripwire.rs   # widen EXTRA_SCOPE + REQUIRED_FILES (D-15)
└── oauth_*                        # new mockito-driven suites; MUST run under --features full,oauth
cargo-pmcp/src/commands/auth_cmd/  # D-17 migration + D-19 convergence
```

### Pattern 1: The wasm-clean shared primitive (`src/shared/pkce.rs`)

**What:** An ungated module that provides pure crypto/validation primitives usable identically on
host and `wasm32`, by swapping optional-dep RNG for `getrandom`.
**When to use:** For every new primitive D-05/D-06 asks for. **Do not invent a new pattern** — this
module is the precedent, it is crate-root re-exported, it has RFC test vectors, and it already has
a dedicated ALWAYS-coverage test file at `tests/pkce_helper.rs`.

```rust
// Source: src/shared/pkce.rs:1-20 (verbatim excerpt of the module doc)
//! Target-agnostic PKCE (RFC 7636) crypto helper for OAuth 2.0 Authorization
//! Code flows.
//! ...
//! Unlike the native CLI flow in
//! [`crate::client::oauth`] (which uses the optional `rand` dependency and is
//! therefore not available on `wasm32`), this module is **ungated** and uses
//! [`getrandom::fill`] for randomness so it compiles and runs identically on
//! the host and on `wasm32-unknown-unknown` (Web Crypto via the `wasm_js`
//! backend).
```

Its declaration in `src/shared/mod.rs:20-22` carries the rationale comment the new module should
mirror:

```rust
// Source: src/shared/mod.rs:20-22
/// Ungated on purpose — compiles on host AND wasm32 via `getrandom::fill`
/// (contrast the `#[cfg(not(target_arch = "wasm32"))]` peer/stdio entries).
pub mod pkce;
```

Public surface already available (`src/lib.rs:106` re-exports all three at crate root):
`generate_code_verifier()`, `code_challenge_s256(&str)`, **`generate_state()`**.

### Pattern 2: The marker-const error identity (D-03's model)

**What:** A stable `data.pmcpError` discriminator string + constructor + `is_*` predicate, riding
an existing `Error` variant, because `Error` is not `#[non_exhaustive]`.
**When to use:** For `iss` mismatch and `state` mismatch failures.
**Critical correction:** the pattern is implemented **only on `Error::Protocol`**, not on
`Error::Authentication`. See `## Decisions Requiring Amendment` A2.

```rust
// Source: src/error/mod.rs:588-611 (the exact shape to replicate)
#[must_use]
pub fn retired_on_v2(method: &str, replacement: &str) -> Self {
    Self::Protocol {
        code: ErrorCode::METHOD_NOT_FOUND,
        message: format!("{method} was removed in MCP 2026-07-28 …"),
        data: Some(serde_json::json!({
            PMCP_ERROR_KEY: RETIRED_ON_V2_MARKER,
            RETIRED_METHOD_KEY: method,
            RETIRED_REPLACEMENT_KEY: replacement,
        })),
    }
}

/// Whether this is the [`Error::retired_on_v2`] local fail-fast.
#[must_use]
pub fn is_retired_on_v2(&self) -> bool {
    self.pmcp_error_marker() == Some(RETIRED_ON_V2_MARKER)
}
```

The private helpers the predicate rests on are **hard-wired to `Protocol`**:

```rust
// Source: src/error/mod.rs:637-648
fn protocol_data(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match self {
        Self::Protocol { data, .. } => data.as_ref()?.as_object(),
        _ => None,
    }
}
fn pmcp_error_marker(&self) -> Option<&str> {
    self.protocol_data()?.get(PMCP_ERROR_KEY)?.as_str()
}
```

And the marker const carries a compatibility note the new marker must copy:

```rust
// Source: src/error/mod.rs:126-131
/// The stable programmatic identity of [`Error::retired_on_v2`].
///
/// Carried in the error's `data.pmcpError`. It is the discriminator
/// [`Error::is_retired_on_v2`] matches on, so it is part of the crate's
/// compatibility surface: **do not change this string**.
pub const RETIRED_ON_V2_MARKER: &str = "RetiredOnV2";
```

### Pattern 3: Semver-safe struct extension

**What:** Never add a public field to an all-pub-field, non-`#[non_exhaustive]` struct. Extend via
inherent methods, new constructor functions, marker constants, `#[serde(flatten)] extra` maps, or a
**new sibling type**.
**When to use:** Every struct this phase wants to extend. Three of them are landmines, not one.

```rust
// Source: cargo-semver-checks lint `constructible_struct_adds_field`
// "A struct exhaustively constructible with a literal using only public API
//  added a new pub field."  -> requires MAJOR bump.
// Triggers when: struct is public, lacks #[non_exhaustive], previously had no
// non-public fields, and a new public field is added.
```

Phase 115's escape hatch (make the struct `#[non_exhaustive]` first) is **not available** here —
those six structs were *already* `#[non_exhaustive]`; marking a struct `#[non_exhaustive]` now is
itself a major break.

### Pattern 4: Bounded whole-body read (D-15's fix shape)

**What:** A `Content-Length` early refusal (advisory) plus `Response::chunk()` accumulated against
an overflow-safe running total checked *before* each append, so an over-cap body is never held
whole.
**When to use:** All 31 reviewed-unbounded reads.

```rust
// Source: src/shared/sse_optimized.rs:280-286 (the worked reqwest example)
async fn collect_sse_text_within_cap(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<String> {
    // Refusal 1 — advisory, and only ever an early exit.
    if let Some(declared) = response.content_length() {
        if declared > max_bytes as u64 {
```

Its refusal-message doc states an invariant the new bounded reads must preserve:

```rust
// Source: src/shared/sse_optimized.rs:321-327
/// Names the LIMIT and the observed size, and deliberately echoes no body
/// content: the refusal must not become a channel for the very bytes it
/// refused.
```

**Note the existing DCR cap is the weaker shape** and should be upgraded while in the area:
`src/client/oauth.rs:280-291` allocates the whole body via `.bytes()` and *then* measures it
(`MAX_DCR_RESPONSE_BYTES`). D-113-V records this explicitly: "it is a POST-HOC check — the body is
allocated whole and then measured — so it bounds what is ACCEPTED, not what is ALLOCATED."

### Pattern 5: Tripwire scope widening (D-15's closure mechanism)

**What:** Add the four auth files to two consts; the scanner needs no change.

```rust
// Source: tests/v2_bounded_reads_tripwire.rs:66-82
/// The two individually-named files HTTP-09 puts in scope beyond `src/shared/`.
const EXTRA_SCOPE: &[&str] = &[
    "src/client/subscriptions.rs",
    "src/server/streamable_http_server.rs",
];

/// Files whose absence from the discovered scope means discovery is broken.
const REQUIRED_FILES: &[&str] = &[
    "http.rs", "sse_parser.rs", "streamable_http.rs",
    "streamable_http_server.rs", "subscriptions.rs",
];
```

**Both** consts must be extended — `REQUIRED_FILES` is the anti-vacuity guard, and adding to
`EXTRA_SCOPE` alone would let a future path typo silently drop coverage. The module doc quotes
HTTP-09's requirement text verbatim as its scope justification; adding auth files means that doc
must also name AUTH-03/D-15 as the second owner, or the file will read as enforcing something its
own stated requirement does not cover.

### Anti-Patterns to Avoid

- **Swapping append→insert in discovery URL construction.** Breaks Microsoft Entra ID. Measured.
  See Pitfall 2. The spec requires an *ordered sequence*, not a replacement.
- **Anchoring `iss` on an unvalidated `metadata.issuer`.** The spec says in as many words that the
  validation "provides no protection if the expected issuer was obtained from an unvalidated
  source." See Pitfall 1.
- **Adding a field to `OidcDiscoveryMetadata`, `DcrRequest`, or `OAuthConfig`.** Major bump. A1.
- **Keeping issuer, `code_verifier` and `state` as three separate locals.** The spec requires them
  associated in one per-request record; three locals is how `state` became unbindable in the first
  place (D-12).
- **Verifying with `--features oauth` alone.** Does not compile at HEAD — `examples/s51_v2_tasks_agent.rs`
  fails with 4 errors under that feature set. Measured. Use `--features full,oauth`.
- **Verifying with `make quality-gate` alone.** Compiles zero lines of `src/client/oauth.rs`. Measured.
- **`cargo nextest -E 'test(/name/)'` for file selection.** Selects by *test name*, not file, and
  silently returns zero while exiting 0. Measured again in this session. Use `binary(...)`.
- **Displaying AS-supplied `error_description` after an `iss` mismatch.** Explicit spec MUST NOT.
- **Hard-failing on issuer change for DCR-issued credentials.** D-18 chose warn; the spec's
  "surface an error" applies to *pre-registered* credentials. See A4 for the distinction.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CSRF `state` generation | A new random-string helper, or reusing `generate_code_verifier()` as D-12 currently does | `pmcp::shared::pkce::generate_state()` | Already exists, already public at crate root (`src/lib.rs:106`), already wasm-clean, already ALWAYS-covered by `tests/pkce_helper.rs`. Reusing the *verifier* generator for `state` conflates two RFC roles. |
| `state` comparison on callback return | A bespoke comparison in the CLI flow | The already-correct implementation at `examples/web-channel-client/client/src/lib.rs:247-251` (tagged `T-103-CSRF`) | The browser client already does this correctly. Lift its logic into the shared tier so both callers share one implementation instead of two. |
| PKCE verifier/challenge | The private duplicates at `src/client/oauth.rs:593-604` | `pmcp::shared::pkce::{generate_code_verifier, code_challenge_s256}` | The CLI flow re-implements what `pkce.rs` exports, using optional `rand` instead of `getrandom`. This duplication is *why* the CLI is not wasm-clean. Consolidating is in the natural path of D-06. |
| `application/x-www-form-urlencoded` decoding of `iss` before comparison | A manual percent-decode | `url::Url::query_pairs()` | RFC 9207 §2.4 *requires* the decode; `query_pairs()` already performs it. Hand-rolling risks getting `+`→space wrong. |
| Bounded HTTP body reads | A new capping helper | The `collect_sse_text_within_cap` shape (`src/shared/sse_optimized.rs:280`) / `collect_body_within_cap` (`src/shared/streamable_http.rs:528`) | Two worked, reviewed implementations exist — one reqwest, one hyper. The auth files are reqwest. |
| Atomic 0o600 credential file writes | A fresh write path | `TokenCacheV1::write_atomic` (`cargo-pmcp/src/commands/auth_cmd/cache.rs:95-120`) | Already does tempfile-in-same-dir → chmod 0o600 → parent chmod 0o700 → persist. D-19 moves this code into core; port it, don't rewrite it. |
| Detecting unbounded reads | A new lint or grep | `tests/v2_bounded_reads_tripwire.rs` scope widening | D-113-V measured that the scanner already resolves rustfmt-split chains (pinned at `:1050`) and would find all 31 unaided. "Expect the fence widening itself to be the whole job." |
| A mock authorization server | A hand-rolled `TcpListener` fixture | `mockito` + the `discovery_body()` helper in `tests/oauth_dcr_integration.rs:15-32` | Already the established harness for exactly this surface. |
| An OAuth client/flow engine | — | The existing hand-rolled stack | AUTH-03 forbids `oauth2`/`openidconnect`; `research/STACK.md` recommends explicitly against. |

**Key insight:** Nearly every primitive this phase needs already exists somewhere in the repo — the
wasm-clean module pattern, `generate_state`, correct `state` validation, bounded-read helpers,
atomic credential writes, and the mock-AS harness. The phase's real work is **consolidation and
wiring**, not invention. A plan that writes new primitives from scratch is a plan that missed
`src/shared/pkce.rs` and `examples/web-channel-client/`.

## Decisions Requiring Amendment

> Four CONTEXT decisions rest on premises this research measured to be false or incomplete.
> Each would produce a defective plan if implemented literally. **These are findings, not
> overrides** — the planner must either adopt the corrected form or escalate to
> `/gsd:discuss-phase`. Listed in severity order.

### A1 — D-01's "add it additively" to `OidcDiscoveryMetadata` is a MAJOR semver break

D-01 says the RFC 9207 discovery flag is "an additive `Option<bool>` field" on
`OidcDiscoveryMetadata`. But `OidcDiscoveryMetadata` (`src/server/auth/oauth2.rs:171-220`) is
`pub`, has **all public fields**, and is **not** `#[non_exhaustive]` — the exact precondition D-09
identified for `DcrRequest`. Adding a field triggers `constructible_struct_adds_field` = **major**.

Measured: 2 non-test literal construction sites in-repo (`examples/c07_oidc_discovery.rs:25`,
`src/client/oauth.rs:1140`), plus any downstream user's. Phase 115's fix (make it
`#[non_exhaustive]`) is unavailable — that is itself a major break.

**The same hazard applies to `OAuthConfig`** (`src/client/oauth.rs:63-88`, all-pub-field,
not `#[non_exhaustive]`, and constructed by struct literal in the module's own doctest at
`:1052-1061`). D-04 already avoids it correctly by choosing a builder method — but D-08's
`Interactivity` selection must make the same choice, and D-01 did not.

**Corrected shape:** the flag must not live on `OidcDiscoveryMetadata`. Options, all semver-minor:
(a) a new method on `OidcDiscoveryClient` returning `(OidcDiscoveryMetadata, DiscoveryExtras)`;
(b) a new `AuthorizationServerPolicy` type carrying issuer + the flag, returned alongside;
(c) parse the raw discovery JSON once and keep the flag in the per-request record (which the spec
already requires to exist — see Code Examples).

Option (c) composes best: the flag is only ever consulted at callback time, and the record is
already the mandated home for the issuer it is validated against.

### A2 — D-03's marker cannot ride `Error::Authentication`

D-03 specifies the marker riding "the existing `Authentication` variant's `data.pmcpError`".
`Error::Authentication` is a **tuple variant carrying a bare `String`** (`src/error/mod.rs:40-42`)
— it has no `data` member. The entire marker machinery is hard-wired to `Error::Protocol`:
`protocol_data()` matches `Self::Protocol { data, .. }` and returns `None` for every other variant
(`:637-643`), and `pmcp_error_marker()` is built on it (`:645-648`).

An `Error::iss_mismatch` returning `Authentication` would make `is_iss_mismatch()` return `false`
for its own constructor's output.

**Corrected shape:** use `Error::Protocol` with an appropriate `ErrorCode`, exactly as
`retired_on_v2` does. D-03's *intent* — marker pattern, no new variant, semver-minor, stable
programmatic discriminator — is fully preserved; only the carrier variant changes.

### A3 — D-13 and D-14 each describe a *different* SEP than the one they cite

Both decisions correctly identify real defects. Both attribute them to SEPs whose actual published
content is something else, which matters because AUTH-03's booking must cite the SEP it claims to
have implemented.

**SEP-2351** is titled *"Explicitly specify RFC 8414 well-known URI suffix for MCP"* and its stated
summary is: "Explicitly state that MCP uses the default `oauth-authorization-server` well-known URI
suffix defined in RFC 8414 Section 3.1 … Clarify that MCP does not define an application-specific
well-known URI suffix." [CITED: github.com/modelcontextprotocol/modelcontextprotocol/pull/2351,
merged 2026-03-28]

The resulting normative requirement is an **ordered multi-endpoint probe** (verbatim in Code
Examples), not the single insertion D-13 describes. pmcp today implements only the *last* candidate
in that list and never tries `oauth-authorization-server` at all — including for path-less issuers,
where the spec makes it **first**. D-13's fix as literally written would regress Microsoft Entra ID
(Pitfall 2, measured).

**SEP-2207** is titled *"Refresh token requests from OIDC servers"* and is about `offline_access`
scope and refresh-token issuance guidance. Its landed spec text (verbatim in Code Examples) adds
two obligations D-14 does not mention:
- Clients **SHOULD** include `refresh_token` in their `grant_types` client metadata. pmcp hardcodes
  `grant_types: vec!["authorization_code".to_string()]` (`src/client/oauth.rs:248`) — a one-line
  fix at the **same construction site** as AUTH-02's `application_type`.
- Clients **MAY** add `offline_access` to the scope parameter when the AS metadata's
  `scopes_supported` contains it. `OidcDiscoveryMetadata.scopes_supported` already exists
  (`oauth2.rs:213`), so the check is free.

D-14's three defects are real and should still be fixed. The amendment is that they are
**additional to**, not **instead of**, SEP-2207's actual content.

### A4 — D-18's "never a hard fail" is narrower than the spec, in a recoverable way

SEP-2352's landed text says: "If the authorization server indicated by protected resource metadata
no longer matches the one the credentials were registered with, clients **SHOULD** surface an error
rather than silently attempting to use mismatched credentials."

D-18 chose warn-and-proceed. The reconciliation is that the spec's sentence is scoped to
**pre-registered** credentials, while the immediately preceding sentence handles DCR credentials
with a different remedy ("**MUST NOT** reuse … and **MUST** re-register"). So:

- **DCR-issued credentials** (`config.client_id == None`): D-18's warn-then-re-register is exactly
  the spec's MUST. No conflict.
- **Pre-registered credentials** (`config.client_id == Some(..)`): the spec says surface an error.
  Silently re-running a browser login against an unexpected IdP with a client_id that was
  provisioned for a different one is the case the spec is warning about.

**Corrected shape:** branch on credential provenance. This is a small refinement of D-18, not a
reversal, and it makes the AUTH-03 booking defensible against the spec text.

## Common Pitfalls

### Pitfall 1: The `iss` anchor is itself unvalidated — the whole check is bypassable

**What goes wrong:** AUTH-01 compares the callback's `iss` against `metadata.issuer`. But nothing
validates that `metadata.issuer` is legitimate. `OidcDiscoveryClient::fetch_discovery`
(`src/client/auth.rs:170-197`) fetches the document, checks HTTP status, deserializes, and returns —
with **no** comparison between the document's `issuer` field and the issuer used to build the URL.

An attacker who can influence discovery (a hostile `mcp_server_url`, DNS, or a compromised
discovery host) serves a document whose `issuer` is whatever they like, and the RFC 9207 comparison
then trivially succeeds against the attacker's own value. The hardening becomes decorative.

**Why it happens:** RFC 8414 §3.3 validation and RFC 9207 §2.4 validation are in different specs
and read as separate concerns. CONTEXT names neither.

**How to avoid:** Implement RFC 8414 §3.3 / OIDC Discovery §4.3 in `fetch_discovery` *before* the
metadata escapes the function. The spec is explicit, with a worked attack example (verbatim in
Code Examples): "a document fetched from `https://attacker.example/.well-known/oauth-authorization-server`
that contains `"issuer": "https://honest.example"` **MUST** be rejected." And the authorization spec
states the dependency directly: the `iss` validation "provides no protection if the expected issuer
was obtained from an unvalidated source."

**Warning signs:** An AUTH-01 test suite where every fixture's discovery document has the same
`issuer` as its mock base URL — such a suite cannot distinguish "validated" from "not validated."
Require a negative-control fixture whose document lies about its issuer.

### Pitfall 2: Replacing append with insert breaks Microsoft Entra ID (measured)

**What goes wrong:** D-13 reads as "the append form is wrong, insert instead." Implemented
literally, discovery breaks for every AS that serves only the OIDC-appended form — which includes
Microsoft Entra ID, whose URL is in this SDK's own doctest (`src/client/auth.rs:127`:
`client.discover("https://login.microsoftonline.com/common/v2.0")`).

**Measured 2026-08-02:**

| URL form | Result |
|---|---|
| `https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration` (append — today's behavior) | **200** |
| `https://login.microsoftonline.com/.well-known/openid-configuration/common/v2.0` (OIDC insert) | **404** |
| `https://login.microsoftonline.com/.well-known/oauth-authorization-server/common/v2.0` (RFC 8414 insert) | **404** |

**Why it happens:** RFC 8414 §3.1 genuinely specifies insertion — but for the
`oauth-authorization-server` suffix. OpenID Connect Discovery 1.0 §4.1 specifies appending for
`openid-configuration`. RFC 8414 §5 reconciles them with a *fallback order*, and the MCP spec makes
that order a client **MUST**.

**How to avoid:** Implement the ordered candidate list (verbatim in Code Examples). Today's
behavior becomes candidate #3, not a deleted branch. Keep `src/client/auth.rs:435-461`'s
`test_discovery_url_construction` — but reframe it: its third case
(`https://auth.example.com/oauth` → appended) is not wrong, it is the *last* candidate.

**Warning signs:** A test suite that asserts a single expected URL per issuer. Assert the ordered
*list*, and assert probe order with a mockito server that 404s the first two candidates.

### Pitfall 3: `make quality-gate` proves nothing about this phase's code (measured)

**What goes wrong:** CLAUDE.md mandates `make quality-gate` before every commit. For this phase it
is nearly blind.

**Measured 2026-08-02:**
- `full = [... "http-client", "logging", "macros", "testing"]` — **`oauth` is absent**
  (`Cargo.toml:205`).
- `make lint` → `cargo clippy --features "full" --lib --tests` (`Makefile:152`). `src/client/oauth.rs`
  is `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]` (`src/client/mod.rs:46`) ⇒
  **never linted**.
- `make test` → `cargo nextest run --features "full"` (`Makefile:212`).
  `tests/oauth_dcr_integration.rs` is `#![cfg(feature = "oauth")]` ⇒ compiles to an empty binary.
- `cargo nextest list --features full -E 'binary(/oauth/)'` → **0 binaries**.

**The A/B, run both ways on the same tree — this is the decisive measurement:**

| Command | Tests selected |
|---|---|
| `cargo nextest list --features full     -E 'binary(oauth_dcr_integration)'` | **0** |
| `cargo nextest list --features full,oauth -E 'binary(oauth_dcr_integration)'` | **5** |

The five that `make quality-gate` never runs: `dcr_body_matches_rfc7591`,
`dcr_fires_when_eligible`, `dcr_not_fired_when_client_id_present`,
`dcr_rejects_http_non_localhost_registration_endpoint_against_live_mock`,
`dcr_rejects_response_larger_than_1mib`. Every one of them is an AUTH-02 regression test.

CI's separate `test` job *does* use `--all-features` (`ci.yml:63,90,93`) and covers it, but with a
narrower clippy allow-list than `make lint`'s pedantic+nursery — so the two gates are
**incomparable**, and neither alone is sufficient for new `oauth` code.

**How to avoid:** every plan's verification block runs `--features full,oauth` explicitly, for
clippy (with `make lint`'s pedantic+nursery flag set) *and* for tests. Treat a green
`make quality-gate` as necessary, never sufficient.

**Warning signs:** A summary citing a test-count delta from `make quality-gate` as evidence for an
AUTH requirement. The delta will be zero regardless of what was written.

### Pitfall 4: `--features oauth` alone does not compile (measured)

**What goes wrong:** The obvious correction to Pitfall 3 — `cargo nextest run --features oauth` —
fails to build at HEAD. `examples/s51_v2_tasks_agent.rs` produces 4 errors (E0308/E0432/E0433)
because it needs feature-gated task APIs that `oauth` alone does not enable; `cargo test --no-run`
builds examples, so the whole run aborts before a single test executes.

**How to avoid:** always `--features full,oauth`. Verified to compile cleanly (3m30s cold).

**Warning signs:** `error: command cargo test --no-run … exited with code 101` while the plan's
verify block reports a test failure — the tests never ran.

### Pitfall 5: `cargo nextest -E 'test(/name/)'` silently selects nothing

**What goes wrong:** `test(...)` matches **test names**, not file/binary names. A selector naming a
file returns zero tests and exits **0**, so a verify block "passes" having executed nothing.

**Measured 2026-08-02 under `--features full`:** `-E 'test(/dcr/)'` returned 6 tests — but all six
are lib unit tests in `src/server/auth/provider.rs` and `providers/cognito.rs`, i.e. the *server*
DCR types, **not** the integration file the selector was aimed at.

This exact defect was recorded as hitting Phase 114 seven times.

**How to avoid:** use `binary(name)` for file/binary selection; use `test(...)` only for actual test
names. Verify any new selector with `cargo nextest list` before committing it to a plan, and assert
a **non-zero count**, not merely exit 0.

### Pitfall 6: "No `oauth2` crate" is already false in the workspace

**What goes wrong:** AUTH-03's booking claims no `oauth2`/`openidconnect` crates were added. A
reviewer greps and finds `oauth2 = "5.0"` at `cargo-pmcp/Cargo.toml:84`, and the booking is
reopened. Both `research/STACK.md` and CONTEXT describe this dep as *transitive* — it is **direct**.

**Measured:** 14 `oauth2::` references, all in
`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` — the pmcp.run **deployment** login (Cognito
device flow for `cargo pmcp deploy`), a wholly separate concern from `auth_cmd/`, which uses
`pmcp::client::oauth::{OAuthConfig, OAuthHelper}` (`auth_cmd/login.rs:6`). Zero `oauth2::`
references anywhere under `cargo-pmcp/src/commands/`.

**How to avoid:** scope the claim precisely — "no new `oauth2`/`openidconnect` dependency; `pmcp`
core remains oauth2-free; `cargo-pmcp`'s pre-existing direct `oauth2 = 5.0` is confined to
`deployment/` and is untouched" — and fence it with the three commands in `## Package Legitimacy
Audit`. D-19's convergence work must not let `oauth2` leak into `auth_cmd/`.

### Pitfall 7: `make doc-check` is RED at HEAD and blocks the org-required `gate`

**What goes wrong:** CI's `quality-gate` job runs `make doc-check` as a step
(`.github/workflows/ci.yml:230-231`) *before* `make quality-gate`, and `quality-gate` is in the
org-required `gate` aggregate's `needs:` list (`ci.yml:386`). `make doc-check` is red.

**Measured 2026-08-02 at branch HEAD:** exit **2**, **28** `^error` lines. Per-file distribution:
`types/subscriptions.rs` 3, `types/mrtr.rs` 4, `types/protocol/context.rs` 4,
`types/caching.rs` 2, `types/protocol/mod.rs` 2, `shared/sse_parser.rs` 2,
`shared/streamable_http.rs` 2, `shared/protocol_helpers.rs` 1, `shared/http.rs` 1,
`error/mod.rs` 1, `client/mod.rs` 1.

**Zero of the 28 are in the files this phase edits** — so the phase does not inherit the defect,
but it also cannot merge past it. Recorded as `D-113-W` / `D-114-V`, owner **UNASSIGNED**.

Two traps follow: (a) `make doc-check`'s feature list **does** include `oauth`
(`Makefile:421`), so it is the *only* gate that compiles this phase's rustdoc — new doc links in
`src/client/oauth.rs` will surface here and nowhere else; (b) `src/error/mod.rs` already carries 1
error, and this phase adds rustdoc there (the new marker), so the count must be re-measured as a
**delta against 28**, not asserted as zero.

**How to avoid:** measure `make doc-check` at the phase base and at HEAD and assert the error set is
byte-identical or smaller, exactly as 114-18 did. Surface the merge blocker as an open question
rather than silently absorbing it.

### Pitfall 8: `check-unwraps` and `unused-deps` are no-op gates

**What goes wrong:** A plan cites `make quality-gate`'s `check-unwraps` as evidence that the phase
introduced no `unwrap()` in production code. The target does not check anything:

```make
# Source: Makefile:768-772
check-unwraps:
	@echo "$(BLUE)Checking for unwrap() calls outside tests...$(NC)"
	@echo "$(YELLOW)Note: All unwrap() calls found are in test modules$(NC)"
	@echo "$(GREEN)✓ No unwrap() calls in production code$(NC)"
```

`unused-deps` (`Makefile:202-206`) is likewise a stub — `cargo machete` is commented out.
`check-todos` (`:763-766`) **is** real and greps `src/` for `TODO|FIXME|HACK|XXX`.

This matters here because `src/client/oauth.rs:436` and `:964` both call
`.duration_since(UNIX_EPOCH).unwrap()` in production paths the phase edits.

**How to avoid:** do not cite `check-unwraps` as evidence. If the phase wants an unwrap invariant,
add a real check.

### Pitfall 9: `cargo semver-checks` has two different baselines and they disagree

**What goes wrong:** CONTEXT cites "clean 223/223 through Phase 115." Phase 114's measurement found
that number names two different baselines: against published crates.io 2.17.0 it is **222/223 with
1 pre-existing failure** (a `#[deprecated]` on `OptimizedSseTransport`); against the phase base it
is **223/223, no update required**.

**How to avoid:** state the baseline in the command. `cargo semver-checks check-release -p pmcp
--baseline-rev <phase-base-sha>` is the phase's own result; the crates.io comparison will show a
pre-existing minor failure that is not this phase's.

### Pitfall 10: Unconditional hardening 401s existing deployments

Carried forward from `research/PITFALLS.md` Pitfall 10, and the reason the phase exists in its
current shape. The mitigation is already encoded in D-01 (floor+trigger) and D-11 (send
unconditionally, never fail on echo divergence), and the spec's 4-row table independently confirms
D-01 is the right shape. Preserve the documented `stateless()` + `AllowedOrigins::any()` proxy
exception. **Warning signs:** existing OAuth integration tests failing with `invalid issuer` / DCR
errors; Lambda proxy deployments 401-ing after upgrade.

## Code Examples

Verified patterns and normative text from official sources.

### The normative `iss` decision table (AUTH-01's specification)

```
Source: https://modelcontextprotocol.io/specification/draft/basic/authorization
        § Authorization Response Validation

"Before redirecting the user-agent, the client MUST record the `issuer` value from the
 selected authorization server's validated metadata document ... and associate it with the
 same per-request record used to store the PKCE code verifier (and the `state` value, if
 used). The validation in this section depends on that recorded value being authentic; it
 provides no protection if the expected issuer was obtained from an unvalidated source."

| authorization_response_iss_parameter_supported | iss in response | Client action                                  |
| ---------------------------------------------- | --------------- | ---------------------------------------------- |
| true                                           | present         | Compare to recorded issuer, simple string cmp  |
| true                                           | absent          | Reject the response                            |
| false or absent                                | present         | Compare to recorded issuer, simple string cmp  |
| false or absent                                | absent          | Proceed                                        |

"After decoding the `iss` value from the `application/x-www-form-urlencoded` response per
 RFC 9207 Section 2.4, clients MUST NOT apply scheme or host case folding, default-port
 elision, trailing-slash, or percent-encoding normalization (RFC 3986 Sections 6.2.2-6.2.3)
 before comparison."

"This validation applies equally to error responses - on mismatch the client MUST NOT act on
 or display `error`, `error_description`, or `error_uri`."
```

Rows 1/2 are D-01's *trigger*; rows 3/4 are D-01's *floor*. **D-01 matches the spec exactly.**
The no-normalization sentence is the ideal source for property-test invariants — derived from the
RFC rather than restated from the implementation, which is the discipline Phase 115 established.

RFC 9207 §2.4 supplies the matching client obligations:

```
Source: https://www.rfc-editor.org/rfc/rfc9207.html §2.4

"Clients that support this specification MUST extract the value of the `iss` parameter from
 authorization responses they receive if the parameter is present."

"Clients MUST then decode the value from its 'application/x-www-form-urlencoded' form
 according to Appendix B of [RFC6749] and compare the result to the issuer identifier of the
 authorization server where the authorization request was sent to. This comparison MUST use
 simple string comparison as defined in Section 6.2.1 of [RFC3986]."

"If the value does not match the expected issuer identifier, clients MUST reject the
 authorization response and MUST NOT proceed with the authorization grant."
```

### SEP-2351 — the ordered discovery-URL probe (AUTH-03)

```
Source: https://modelcontextprotocol.io/specification/draft/basic/authorization/
        authorization-server-discovery § Authorization Server Metadata Discovery

"MCP uses the default `oauth-authorization-server` well-known URI suffix defined in RFC 8414
 Section 3.1 for authorization server metadata discovery. MCP does not define an
 application-specific well-known URI suffix."

"... MCP clients MUST attempt multiple well-known endpoints when discovering authorization
 server metadata."

For issuer URLs WITH path components (e.g. https://auth.example.com/tenant1):
  1. https://auth.example.com/.well-known/oauth-authorization-server/tenant1
  2. https://auth.example.com/.well-known/openid-configuration/tenant1
  3. https://auth.example.com/tenant1/.well-known/openid-configuration     <- pmcp's ONLY form today

For issuer URLs WITHOUT path components (e.g. https://auth.example.com):
  1. https://auth.example.com/.well-known/oauth-authorization-server        <- pmcp never tries this
  2. https://auth.example.com/.well-known/openid-configuration

"After retrieving a metadata document, MCP clients MUST validate it as required by RFC8414
 Section 3.3 or OpenID Connect Discovery Section 4.3: the `issuer` value in the document MUST
 be identical to the issuer identifier used to construct the well-known URL. If they differ,
 the client MUST NOT use the metadata. For example, a document fetched from
 https://attacker.example/.well-known/oauth-authorization-server that contains
 "issuer": "https://honest.example" MUST be rejected."
```

The current implementation, for contrast:

```rust
// Source: src/client/auth.rs:136-140 — the ONLY candidate pmcp builds
pub async fn discover(&self, issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );
```

### SEP-837 — `application_type` normative text (AUTH-02)

```
Source: https://modelcontextprotocol.io/specification/draft/basic/authorization/
        client-registration § Application Type and Redirect URI Constraints

"MCP clients MUST specify an appropriate `application_type` during Dynamic Client
 Registration. Omitting it defaults to "web" under OIDC, which can conflict with native-style
 redirect URIs; non-OIDC servers safely ignore the parameter."

"* Native applications (desktop applications, mobile apps, CLI tools, and locally-hosted web
   applications accessed via `localhost`) SHOULD use application_type: "native"
 * Web applications (remote browser-based applications served from a non-local host) SHOULD
   use application_type: "web""

"MCP clients MUST be prepared to handle registration failures due to redirect URI constraints
 when authorization servers implement OIDC. When a registration request is rejected, clients
 SHOULD surface a meaningful error to the user or developer. Clients MAY retry registration
 with an adjusted `application_type` or with redirect URIs that conform to the authorization
 server's requirements for the given application type."
```

Two notes for the planner. The "non-OIDC servers safely ignore the parameter" clause is the
spec's own confirmation of **D-11** (send unconditionally, no era gate). And the third paragraph
is an obligation **D-09/D-10/D-11 do not cover**: registration-failure handling with an optional
`application_type` retry. Also note the spec classifies by *application nature* while D-10 derives
from `redirect_uris` — these agree for every case pmcp produces, and D-09's explicit setter remains
the authoritative override, but the derivation should be documented as a *heuristic for the common
case*, not as the spec's rule.

The construction site that receives it (note `grant_types`, which SEP-2207 also touches):

```rust
// Source: src/client/oauth.rs:241-257
let request = crate::server::auth::provider::DcrRequest {
    redirect_uris: vec![redirect_uri],
    client_name: Some(client_name),
    // ...
    token_endpoint_auth_method: Some("none".to_string()),
    grant_types: vec!["authorization_code".to_string()],   // <- SEP-2207: add "refresh_token"
    response_types: vec!["code".to_string()],
    scope: None,
    software_id: None,
    software_version: None,
    extra: Default::default(),                             // <- D-09's carrier for application_type
};
```

### SEP-2352 — Authorization Server Binding (AUTH-03)

```
Source: https://modelcontextprotocol.io/specification/draft/basic/authorization/
        client-registration § Authorization Server Binding

"Clients that use pre-registered credentials, or persist client credentials obtained via
 Dynamic Client Registration, MUST associate those credentials with the specific authorization
 server that issued them, keyed by the authorization server's `issuer` identifier. When the
 authorization server changes (detected via updated protected resource metadata), clients MUST
 NOT reuse client credentials from a different authorization server and MUST re-register with
 the new authorization server."

"Pre-registered credentials are inherently specific to a particular authorization server. If
 the authorization server indicated by protected resource metadata no longer matches the one
 the credentials were registered with, clients SHOULD surface an error rather than silently
 attempting to use mismatched credentials."

"Client IDs based on Client ID Metadata Documents are portable across authorization servers,
 since they are self-hosted HTTPS URLs resolved by the authorization server on demand. No
 re-registration is needed when the authorization server changes."
```

Also, from the discovery page, a second MUST that reinforces D-07's key shape:

```
"Clients MUST maintain separate registration state (client credentials, tokens) per
 authorization server and MUST NOT assume that credentials valid for one authorization server
 will be accepted by another."
```

### SEP-2207 — Refresh Tokens (AUTH-03)

```
Source: https://modelcontextprotocol.io/specification/draft/basic/authorization § Refresh Tokens

"MCP Clients that desire refresh tokens:
 * MUST keep refresh tokens confidential in transit and storage ...
 * SHOULD include `refresh_token` in their `grant_types` client metadata
 * MAY add `offline_access` to the `scope` parameter of the authorization and token requests
   when the Authorization Server metadata contains it in `scopes_supported`
 * MUST NOT assume refresh tokens will be issued; the AS retains discretion

 MCP Servers (Protected Resources) SHOULD NOT include `offline_access` in `WWW-Authenticate`
 scope or Protected Resource Metadata `scopes_supported`, as refresh tokens are not a resource
 requirement."
```

The three D-14 defects, at source:

```rust
// Source: src/client/oauth.rs:922-924 — defect 2: DCR flows can never refresh
let client_id = self.config.client_id.as_deref().ok_or_else(|| {
    Error::internal("cannot refresh token without a cached client_id".to_string())
})?;

// Source: src/client/oauth.rs:926-933 — defect 3: no `scope` on the refresh request
let response = self.client.post(token_endpoint)
    .form(&[
        ("client_id", client_id),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ])

// Source: src/client/oauth.rs:970-975 — defect 1: an omitted refresh_token overwrites the good one
let cache = TokenCache {
    access_token: token.access_token.clone(),
    refresh_token: token.refresh_token.clone(),   // None when the AS omitted it
    expires_at,
    scopes: self.config.scopes.clone(),
};

// Source: src/client/oauth.rs:941 — a D-15 unbounded read on the same function's error path
response.text().await.unwrap_or_default()
```

### D-12 at source, and the correct implementation that already exists

```rust
// Source: src/client/oauth.rs:664-672 — `state` generated as an unnamed temporary
auth_url.query_pairs_mut()
    .append_pair("client_id", &resolved_client_id)
    .append_pair("response_type", "code")
    .append_pair("redirect_uri", &redirect_uri)
    .append_pair("scope", &self.config.scopes.join(" "))
    .append_pair("code_challenge", &code_challenge)
    .append_pair("code_challenge_method", "S256")
    .append_pair("state", &Self::generate_code_verifier()); // Random state for CSRF protection

// Source: src/client/oauth.rs:697-700 — the callback reads `code` and nothing else
let code = callback_url
    .query_pairs()
    .find(|(key, _)| key == "code")
    .map(|(_, value)| value.to_string());
```

The browser client already does it right — lift this, do not re-derive it:

```rust
// Source: examples/web-channel-client/client/src/lib.rs:247-251
// CSRF: the returned state MUST equal the state we generated (T-103-CSRF).
let expected = storage_get(KEY_STATE)?
    .ok_or_else(|| js_error("no stored OAuth state — start login again"))?;
if state != expected {
    return Err(js_error("OAuth state mismatch — possible CSRF, aborting"));
}
```

### The mockito harness to copy for every new test

```rust
// Source: tests/oauth_dcr_integration.rs:9-32
#![cfg(feature = "oauth")]

use mockito::{Matcher, Server};
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
use serde_json::json;

fn discovery_body(base: &str, with_reg: bool) -> String {
    let mut v = json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/authorize", base),
        "token_endpoint": format!("{}/token", base),
        // ...
        "scopes_supported": ["openid"],
        "code_challenge_methods_supported": ["S256"],
    });
    if with_reg { v["registration_endpoint"] = json!(format!("{}/register", base)); }
    v.to_string()
}
```

**Note the `#![cfg(feature = "oauth")]` on line 9** — this is precisely why the file contributes
zero tests under `make quality-gate` (Pitfall 3). New test files will inherit the same property.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| DCR (RFC 7591) as the primary zero-prior-relationship registration path | **Client ID Metadata Documents** (`draft-ietf-oauth-client-id-metadata-document-00`); DCR explicitly **deprecated**, retained for backcompat | MCP draft spec, PR #2858 | AUTH-02 hardens a mechanism the spec now marks deprecated. Still correct — DCR remains for ASes without CIMD — but the planner should not over-invest, and D-09's "typed accessors for the *other* `DcrResponse` fields" is rightly deferred. CIMD support is **out of scope**. |
| MCP clients discovering the AS from the server base URL's `openid-configuration` | RFC 9728 **Protected Resource Metadata** (`/.well-known/oauth-protected-resource`) is a client **MUST** for AS discovery | MCP spec 2025-06-18 onward | pmcp's client does neither today (it goes straight to the MCP base URL's `openid-configuration`, `src/client/oauth.rs:366-390`). Not one of the six SEPs and **not in AUTH-01..03** — see Open Questions 1. |
| No resource binding on token requests | RFC 8707 `resource` parameter **MUST** be sent on both authorization and token requests, "regardless of whether authorization servers support it" | MCP spec 2025-06-18 | pmcp sends neither. Not one of the six SEPs — see Open Questions 1. |
| `authorization_response_iss_parameter_supported` optional/ignored | Spec publishes a normative 4-row client decision table and signals a future upgrade of AS `iss` inclusion from SHOULD to MUST | MCP draft (SEP-2468 merged 2026-05-17) | Building the trigger as configurable now (D-04) is the right call — the spec explicitly says the keying "will continue to be keyed on `authorization_response_iss_parameter_supported` until that revision defines the upgrade path." |
| Step-up scope handling undefined | Step-Up Authorization Flow + scope-union is now **in the core spec**, described as "a client-side responsibility" | MCP draft | Mildly weakens CONTEXT's SEP-2350 deferral rationale ("would have nothing to trigger it") — a *non-pmcp* server can trigger the client half. The deferral remains defensible as a coherence decision; the rationale should be restated. |

**Deprecated/outdated:**
- **`research/STACK.md:112`'s claim that the callback "parses `code` + `state`"** — it parses `code`
  only. CONTEXT's D-12 already caught and corrected this.
- **`research/STACK.md`'s claim that `oauth2` 5.0.0 is purely transitive** — it is a direct
  dependency of `cargo-pmcp` (Pitfall 6).
- **The roadmap's / AUTH-01's "strict on v2, lenient on v1" framing** — superseded by D-01, which
  correctly observes the era is unavailable pre-connection, and independently confirmed by the
  spec's table. See Open Questions 3.

## Runtime State Inventory

> This phase changes on-disk credential formats and is therefore partly a migration.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | `~/.pmcp/oauth-tokens.json` — core `TokenCache` (`src/client/oauth.rs:151-156`): flat single token, fields `access_token`/`refresh_token`/`expires_at`/`scopes`, **no `issuer` field**. `~/.pmcp/oauth-cache.json` — cargo-pmcp `TokenCacheV1` (`auth_cmd/cache.rs:25-52`): `schema_version:1` + `BTreeMap<normalized_url, Entry>`, **each entry records `issuer` and `client_id`**. | **Both**: D-17 splits them — core file discarded on first read (one forced re-login); cargo-pmcp file gets a real 1→2 migration. Data migration **and** code edit, as separate tasks. |
| **Live service config** | None found. This phase registers nothing with an external service, creates no dashboards, no workflows, no ACL tags. Verified by inspecting the four edited source areas and `cargo-pmcp/src/commands/auth_cmd/` — all read/write local files and speak to caller-configured IdPs at runtime only. | None. |
| **OS-registered state** | None found. No scheduled tasks, no launchd/systemd units, no pm2 processes. Verified: no registration code in `auth_cmd/` or `src/client/oauth.rs`. | None. |
| **Secrets/env vars** | **New** env var `PMCP_OAUTH_ISS_VALIDATION` (D-04) — additive, absent = current behavior, so no existing deployment is affected. No existing secret key is renamed. Existing OAuth secrets live in caller-supplied `OAuthConfig`, unchanged. | Document the new var; ensure absent-means-default. |
| **Build artifacts** | `cargo-pmcp` gets a version bump and a `pmcp` dep re-pin (D-19). Anyone with `cargo install cargo-pmcp` holds a binary carrying the old `TokenCacheV1` **which hard-errors on an unknown `schema_version`**: `read()` bails with *"cache schema_version {} unsupported (expected {}); upgrade cargo-pmcp"* (`cache.rs:74-80`). | **Forward-compat trap:** once core writes `schema_version: 2`, an older installed `cargo-pmcp` fails hard rather than degrading. Either keep the v2 writer behind the new binary only, or make the error message actionable. Must be a named plan task, not an assumption. |

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / `rustc` | everything | ✓ | 1.97.1 (2026-06-30 / 2026-07-14) | — |
| `cargo-nextest` | `make test`, per-task verification | ✓ | 0.9.102 | `cargo test` |
| `pmat` | PR-blocking complexity gate | ✓ | **3.15.0** — exactly the pinned CI version | — |
| `cargo-semver-checks` | D-09/A1 additive-only proof | ✓ | 0.49.0 | `cargo public-api` |
| `cargo-public-api` | additive-surface diff | ✓ | 0.52.0 | — |
| `cargo-fuzz` | ALWAYS fuzz requirement | ✓ | 0.13.1 | — |
| `cargo-deny` | `make purity-check` | ✓ | **0.18.3** — matches the Makefile's pinned CLI form | newer versions **break the gate** (flag reordering) |
| `cargo-audit` | `make audit` | ✓ | 0.22.0 | — |
| `mdbook` | book chapters, if docs land here | ✓ | 0.4.52 | — |
| `mockito` | mock AS in tests | ✓ (dev-dep) | 1.5.0 | — |
| `gh` | SEP PR lookup | ✓ | 2.64.0 | web fetch |
| Network access to `login.microsoftonline.com` | Pitfall 2 verification | ✓ | — | the measurement is recorded above; no need to re-run |
| `wasm32-unknown-unknown` target | D-06's wasm fence | **NOT VERIFIED** | — | `rustup target add wasm32-unknown-unknown`; `make wasm-build` exists |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** the `wasm32-unknown-unknown` target was not probed in this
session. D-06 requires a wasm build fence; the planner should add `rustup target add
wasm32-unknown-unknown` as an explicit setup step rather than assuming it. `make wasm-build` exists
and was reported green (with 91 pre-existing dead-code warnings) at Phase 114 HEAD.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo nextest` 0.9.102 (+ `cargo test` for doctests, which nextest does not run) |
| Config file | none dedicated; feature sets come from `Makefile` targets |
| Quick run command | `cargo nextest run --features full,oauth -E 'binary(<new_test_binary>)'` — **never** `--features oauth` alone (Pitfall 4), **never** `test(/…/)` for file selection (Pitfall 5) |
| Full suite command | `make quality-gate` **plus** `cargo nextest run --features full,oauth` — the first alone covers zero oauth code (Pitfall 3) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | 4-row `iss` table: all four rows, incl. "supported=true + absent ⇒ reject" | unit (pure fn) | `cargo nextest run --features full,oauth -E 'binary(oauth_iss_validation)'` | ❌ Wave 0 |
| AUTH-01 | No normalization: case, default port, trailing slash, percent-encoding all ⇒ mismatch | property | `cargo nextest run --features full,oauth -E 'binary(oauth_iss_validation)'` | ❌ Wave 0 |
| AUTH-01 | Code is **never redeemed** on mismatch (assert zero hits on the mock token endpoint) | integration (mockito) | `cargo nextest run --features full,oauth -E 'binary(oauth_iss_integration)'` | ❌ Wave 0 |
| AUTH-01 | Error responses: on `iss` mismatch, `error_description` is neither acted on nor displayed | unit | same binary | ❌ Wave 0 |
| AUTH-01 | **Anchor validation (Pitfall 1):** discovery doc whose `issuer` ≠ URL-issuer is rejected | integration (mockito) | `cargo nextest run --features full,oauth -E 'binary(oauth_discovery_validation)'` | ❌ Wave 0 |
| AUTH-01 (D-12) | `state` round-trips and mismatch aborts before redemption | unit + integration | `-E 'binary(oauth_state_csrf)'` | ❌ Wave 0 |
| AUTH-01 (D-04) | Precedence env > builder > discovery flag | unit | `-E 'binary(oauth_iss_validation)'` | ❌ Wave 0 |
| AUTH-02 | `application_type` present in the **wire body** (mockito `Matcher::JsonString`) | integration | `cargo nextest run --features full,oauth -E 'binary(oauth_dcr_integration)'` (extend) | ✅ extend |
| AUTH-02 (D-10) | native/web derivation; mixed `redirect_uris` ⇒ explicit error | unit + property | `-E 'binary(oauth_dcr_integration)'` | ✅ extend |
| AUTH-02 (D-09) | Precedence when a caller also wrote raw `"application_type"` into `extra` | unit | `-E 'binary(oauth_dcr_integration)'` | ✅ extend |
| AUTH-02 (D-11) | Echo divergence warns, never fails registration | integration | `-E 'binary(oauth_dcr_integration)'` | ✅ extend |
| AUTH-02 | Semver: additive only | gate | `cargo semver-checks check-release -p pmcp --baseline-rev <phase-base>` | n/a |
| AUTH-03 (2351) | Candidate list + **probe order** (first two 404, third succeeds) | unit + integration | `-E 'binary(oauth_discovery_urls)'` | ❌ Wave 0 |
| AUTH-03 (2352) | Store keyed by `(issuer, account)`; AS change ⇒ cache miss + re-register | unit | `-E 'binary(oauth_credential_store)'` | ❌ Wave 0 |
| AUTH-03 (2207) | Omitted `refresh_token` preserves the stored one | unit | `-E 'binary(oauth_refresh)'` | ❌ Wave 0 |
| AUTH-03 (2207) | DCR-issued `client_id` is usable on refresh | integration | `-E 'binary(oauth_refresh)'` | ❌ Wave 0 |
| AUTH-03 (2207) | `scope` sent on refresh; `grant_types` includes `refresh_token`; `offline_access` added only when in `scopes_supported` | integration | `-E 'binary(oauth_refresh)'` | ❌ Wave 0 |
| AUTH-03 (D-17) | cargo-pmcp 1→2 migration preserves every entry; core cache discarded | unit | `cargo nextest run -p cargo-pmcp -E 'binary(/auth/)'` | ❌ Wave 0 |
| AUTH-03 (D-15) | 4 auth files in tripwire scope; `WHOLE_BODY_ALLOWLIST` stays empty | tripwire | `cargo nextest run --features full,oauth -E 'binary(v2_bounded_reads_tripwire)'` | ✅ extend |
| AUTH-03 (D-06) | Pure tier compiles for wasm32 without `oauth` | build fence | `cargo build --target wasm32-unknown-unknown` (default features) | ❌ Wave 0 |
| AUTH-03 | No new dependency | gate | `git diff --exit-code <base>..HEAD -- Cargo.toml Cargo.lock` | n/a |
| ALL | ALWAYS: fuzz + example | fuzz + example | `make test-fuzz`; `cargo run --example <new>` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run --features full,oauth -E 'binary(<the binary this task touched>)'` — and assert a **non-zero test count**, because a mis-typed selector exits 0 having run nothing (Pitfall 5).
- **Per wave merge:** `make quality-gate` **AND** `cargo nextest run --features full,oauth` **AND** `cargo clippy --features full,oauth --lib --tests` with `make lint`'s pedantic+nursery flags. All three; none subsumes the others.
- **Phase gate:** the above, plus `pmat quality-gate --fail-on-violation --checks complexity`, `cargo semver-checks check-release -p pmcp --baseline-rev <phase-base>`, `make doc-check` measured as a **delta against the 28-error baseline**, and the wasm32 build fence.

### Wave 0 Gaps

- [ ] `tests/oauth_iss_validation.rs` — AUTH-01 pure-function table + no-normalization properties
- [ ] `tests/oauth_iss_integration.rs` — AUTH-01 end-to-end, incl. "code never redeemed" negative control
- [ ] `tests/oauth_discovery_validation.rs` — RFC 8414 §3.3 anchor validation (Pitfall 1)
- [ ] `tests/oauth_discovery_urls.rs` — SEP-2351 candidate list + probe order
- [ ] `tests/oauth_state_csrf.rs` — D-12
- [ ] `tests/oauth_credential_store.rs` — D-07/D-16 store semantics
- [ ] `tests/oauth_refresh.rs` — D-14 + SEP-2207
- [ ] cargo-pmcp migration test (crate-local)
- [ ] wasm32 build fence (CI step or Makefile target) — D-06
- [ ] New/extended fuzz target + runnable example — house ALWAYS requirements
- [ ] **No framework install needed** — nextest, mockito, proptest, cargo-fuzz all present

**Negative controls are mandatory here.** Phase 115's discipline: a fence must be *observed to fail*
before the fix. For AUTH-01 the highest-value control is a discovery document that lies about its
`issuer` — a suite without it cannot distinguish a validated anchor from an unvalidated one.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | **yes** | RFC 9207 `iss` validation; RFC 8414 §3.3 metadata validation; PKCE S256 (`src/shared/pkce.rs`, already present) |
| V3 Session Management | **yes** | OAuth `state` as CSRF token (D-12) — single-use, compared before redemption, per `examples/web-channel-client`'s `T-103-CSRF` shape |
| V4 Access Control | partial | Scope handling on refresh (SEP-2207). Step-up scope accumulation (SEP-2350) is **deferred**. |
| V5 Input Validation | **yes** | All AS-supplied inputs: `iss`, `state`, `code`, `error*`, discovery JSON, DCR response. Includes the **bounded-read** work (D-15) — an unbounded read *is* an input-validation failure. `url::Url::query_pairs()` for decoding; never hand-rolled percent-decode. |
| V6 Cryptography | **yes** | `sha2` for S256 and for sha256-prefix token logging; `getrandom` for `state`. **Never hand-roll** — `src/shared/pkce.rs` already provides all of it. |
| V7 Error Handling & Logging | **yes** | Tokens never logged raw (sha256 prefix, platform convention, enforceable by a static-source invariant test). Refusal messages must not echo refused body content (`sse_optimized.rs:321-327`). On `iss` mismatch, AS-supplied `error_description` must **not** be displayed. |
| V8 Data Protection at Rest | partial | Credential file stays 0o600 + parent 0o700 + atomic write. **Token-at-rest encryption is explicitly deferred** (platform uses KMS). |

### Known Threat Patterns for hand-rolled OAuth client in Rust

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Authorization-server **mix-up** (response from AS-B accepted as AS-A) | Spoofing | RFC 9207 `iss`, simple string comparison, reject before redeeming (AUTH-01) |
| **Discovery-document spoofing** — the mix-up defense's own anchor is forged | Spoofing / Tampering | RFC 8414 §3.3: `metadata.issuer` **MUST** equal the issuer used to build the URL. **Absent today** (Pitfall 1) |
| **CSRF / authorization-code injection** into the callback | Tampering | `state` bound to the per-request record and compared before redemption (D-12); PKCE S256 already present |
| Cross-AS **credential reuse** (client_id from AS-A replayed at AS-B) | Spoofing / Elevation | Issuer-keyed store (D-07/D-16) — true by construction, not by enforcement code |
| **Silent AS substitution** — user walked through a login at an unexpected IdP | Spoofing | D-18 warning naming both issuers; hard error for pre-registered credentials (A4) |
| **Memory exhaustion** from a hostile/compromised IdP response body | Denial of Service | Streaming running-total bound (D-15), `collect_sse_text_within_cap` shape — not the post-hoc `.bytes()`-then-measure shape |
| **Token leakage via logs** | Information Disclosure | sha256-prefix only; mirrored static-source invariant test (platform-proven convention). Note `src/client/oauth.rs:1018-1021` currently logs the first 20 chars of an access token at debug level |
| **Refresh-token destruction** ⇒ forced re-auth ⇒ credential-prompt fatigue | Denial of Service | Preserve stored token when the response omits one (D-14 defect 1) |
| **Open redirect / redirect-URI mismatch** | Tampering | Loopback literal `127.0.0.1` (already correct per RFC 8252 §7.3); `application_type` consistency (D-10) |
| **Headless browser fall-through** — 5-minute hang per attempt in a Lambda | Denial of Service | `Interactivity::RefreshOnly` making the browser path unreachable by construction (D-08) |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The 28 `make doc-check` errors are pre-existing and not attributable to any uncommitted working-tree change. Measured at current branch HEAD only; not re-measured at a clean phase-base commit. | Pitfall 7 | Low. If some are working-tree-induced, the baseline number shifts but the method (delta, not absolute) still holds. |
| A2 | `make quality-gate` currently exits 0 at this branch HEAD. **Not re-measured this session** — carried from Phase 114's recorded result (4899 passed / 0 failed). | Pitfall 3, Validation | Medium. If it is already red, the phase inherits a blocker. The planner should measure it at the phase base before Wave 1. |
| A3 | `cargo semver-checks` is not wired into CI or the Makefile — it is run manually as a plan verification step. Inferred from grepping `Makefile`, `.github/workflows/*.yml`, `justfile` (no hits) plus its appearance in Phase 115 plan text. | Pitfalls, Validation | Low. Means the planner must add it explicitly; it will not fire on its own. |
| A4 | The MCP **draft** spec pages fetched here are the authority for the six SEPs. All five SEP PRs are merged (verified via `gh`), but the draft can still move before the 2026-07-28 final. | Code Examples, all | Medium. Re-check the draft pages at plan time; the `iss` table and discovery order are the two most load-bearing. |
| A5 | `wasm32-unknown-unknown` is installable/available in this environment. Not probed. | Environment | Low. `rustup target add` is a one-liner. |
| A6 | The D-113-V population of 31 reads is still accurate at HEAD. Transcribed from the phase-113.1 measurement, not independently re-counted this session. | D-15 scope | Low-Medium. The fix is "widen the fence and bound whatever it reports," which is self-correcting; the count is only used for estimation. |
| A7 | No project skill in `.agents/skills/` constrains this phase. Verified the only skill pack is `spike-findings-rust-mcp-sdk` (schema/SQL/skills-server topics); none mention auth or OAuth. | Project Constraints | Low. |

## Project Constraints (from CLAUDE.md)

Directives the planner must verify compliance against. These carry the same authority as CONTEXT's
locked decisions.

| Directive | Applies here as |
|-----------|-----------------|
| **Zero tolerance for defects; `make quality-gate` before any commit** | Mandatory — **but see Pitfall 3: it is nearly blind to this phase.** Must be paired with `--features full,oauth` runs, or the mandate is satisfied in letter only. |
| **PR-blocking `pmat quality-gate --fail-on-violation --checks complexity`, cog ≤ 25, hard cap 50** | `authorization_code_flow_inner` is already ~150 lines and this phase adds record construction, state + iss validation, and error-response handling to it. **Complexity budget is a live risk** — extract to the pure tier rather than inlining. pmat 3.15.0 is installed locally. Do NOT weaken the gate. |
| **Zero SATD — no `TODO`/`FIXME`/`HACK`/`XXX` in `src/`** | `make check-todos` is a *real* gate (`Makefile:763-766`), unlike `check-unwraps`. Deferred work goes in `deferred-items.md`, never in a source comment. |
| **80%+ test coverage** | Applies to the new pure tier especially — it is the easiest to cover and the most security-critical. |
| **ALWAYS requirements for every new feature: FUZZ + PROPERTY + UNIT + runnable `cargo run --example`** | Four separate obligations. Existing fuzz homes: `fuzz/fuzz_targets/auth_flows.rs`, `pkce_helper.rs`, `dcr_response_parser.rs`. Note `make test-examples` only **builds** examples — a runnable end-to-end example needs its own harness if it is to be enforced. |
| **Comprehensive rustdoc with working examples on all public APIs** | Every new public item. `make doc-check` includes `oauth` and is the only gate that compiles this phase's rustdoc (Pitfall 7). |
| **Contract-first: update contract YAML in `../provable-contracts/contracts/<crate>/`, `pmat comply check` before and after** | `make comply` is inside `make quality-gate`. Check whether an auth contract exists before editing. |
| **Feature flags for optional functionality; builder pattern; `async_trait`; `serde(rename_all="camelCase")` for protocol types** | D-07's store trait uses `async_trait`. D-04/D-08 use builder methods (also the semver-safe choice, A1). |
| **Prefer `justfile`** (user global) | This repo uses `Makefile` and CI invokes `make` targets directly. Follow the repo. |
| **Release: workspace publish order — `pmcp` (2) before `cargo-pmcp` (12)** | D-19 bumps `cargo-pmcp` and re-pins `pmcp`. Ordering already correct; the pin must be updated in `cargo-pmcp/Cargo.toml`. |
| **`make quality-gate` before pushing a commit or PR** | Reiterated in CLAUDE.md's closing line. |

## Open Questions (RESOLVED)

> All five questions were resolved during plan-phase on 2026-08-02. Per-item resolutions are
> annotated inline; the resolving artifacts are the owner's scope decision (recorded in
> 116-CONTEXT.md `<deferred>`) and plans 116-01/116-05/116-15 (committed `6b57ca10`).

1. **Two MCP-spec client MUSTs are unimplemented and are not in AUTH-01..03: RFC 9728 Protected
   Resource Metadata discovery, and the RFC 8707 `resource` parameter.**
   - *What we know:* The spec is unambiguous — "MCP clients **MUST** use OAuth 2.0 Protected
     Resource Metadata for authorization server discovery" and "`resource` … **MUST** be included in
     both authorization requests and token requests … regardless of whether authorization servers
     support it." pmcp's client does neither: it derives the AS from the MCP base URL directly
     (`src/client/oauth.rs:366-390`) and sends no `resource` parameter (`:664-672`).
   - *What's unclear:* Whether the phase should absorb them. They are **not** among the six SEPs,
     not in CONTEXT, and not in AUTH-01..03 — but they sit on the exact lines this phase edits, and
     SEP-2352's AS-change detection is specified as being "detected via updated protected resource
     metadata," which pmcp cannot detect without RFC 9728.
   - *Recommendation:* **Do not silently absorb.** Record both as deferred items with a named owner
     and note the D-18 dependency explicitly. If the phase wants AS-change detection to match the
     spec's stated mechanism, that is a scope decision for `/gsd:discuss-phase`, not for the planner.
   - ✅ **RESOLVED (2026-08-02):** Owner chose **defer both**. Recorded as two named deferred items
     in 116-CONTEXT.md `<deferred>` with the D-18 dependency stated; zero plan tasks implement
     them; 116-15 Task 3's deferred-items register carries them with owners.

2. **`make doc-check` is red (28 errors) and blocks the org-required `gate`; owner is UNASSIGNED.**
   - *What we know:* Measured exit 2 / 28 errors at branch HEAD; zero in files this phase edits;
     recorded as `D-113-W` and `D-114-V`, both open, both unowned; Phase 119 was floated as a home
     but it blocks merge now.
   - *What's unclear:* Whether Phase 116 is expected to merge independently or ride a larger branch
     merge where this is handled once.
   - *Recommendation:* Do not adopt it into scope. Measure it as a **delta** and state in the phase
     summary that the phase neither caused nor cleared it.
   - ✅ **RESOLVED (2026-08-02):** Recommendation adopted as orchestrator directive — measured as a
     delta in 116-01 (baseline) and 116-15 (closing gates); not adopted into scope.

3. **AUTH-01's requirement text says "strict on v2, lenient on v1," which D-01 shows is not
   implementable and the spec does not ask for.**
   - *What we know:* `Client::era()` is private and depends on `negotiated_protocol_version`, which
     does not exist during the OAuth flow — the flow runs before any MCP connection. The spec's
     4-row table keys on the RFC 9207 discovery flag, not on protocol era. D-01 chose floor+trigger,
     which matches the spec exactly.
   - *What's unclear:* How AUTH-01 gets booked `[x]` when its text names a mechanism that will not
     be built. D-20 requires each booking to cite artifact + named test + count.
   - *Recommendation:* Book against the spec's table and state plainly that "strict on v2 / lenient
     on v1" was realized as "strict whenever the AS advertises or emits `iss`, lenient otherwise" —
     which is *strictly safer for v1 than the requirement asked for*, and say so. Consider proposing
     a requirement-text amendment rather than booking against words the code does not implement.
   - ✅ **RESOLVED (2026-08-02):** Booking language adopted verbatim in 116-15 Task 2 — AUTH-01 is
     booked against the spec's RFC 9207 table ("strict whenever the AS advertises or emits `iss`,
     lenient otherwise") with the realization stated plainly.

4. **Does the credential-store trait carry refresh, or only load/save/delete?**
   - CONTEXT explicitly leaves this open and notes a deliberate answer belongs with the deferred
     vending-core extraction. It is load-bearing for D-14 defect 2 (refresh needs the stored
     `client_id`) and D-08 (`RefreshOnly` needs to know refresh failed without a browser).
   - *Recommendation:* Smallest viable seam — `load`/`save`/`delete` only, with refresh staying in
     `OAuthHelper` and *reading* the store for `client_id`. A trait that owns refresh would need an
     HTTP client, breaking D-07's I/O-free-construction and D-06's wasm-cleanliness in one move.
   - ✅ **RESOLVED (2026-08-02):** Recommendation adopted — 116-05 cites this question explicitly
     and implements `load`/`save`/`delete` only; refresh stays in `OAuthHelper`, which *reads* the
     store for `client_id`.

5. **Does an existing provable-contract cover the auth surface?**
   - `make comply` runs inside `make quality-gate` and CLAUDE.md mandates contract-first. Not
     inspected this session (contracts live outside this repo at `../provable-contracts/`).
   - *Recommendation:* First plan task should check `../provable-contracts/contracts/pmcp/` for an
     auth contract before any source edit.
   - ✅ **RESOLVED (2026-08-02):** 116-01 Task 1 performs the check before any source edit. Planner
     measurement: contracts are in-repo at `contracts/` (not `../provable-contracts/`); `make
     comply` resolves `contracts/{binding,mcp-protocol-sdk-v1,team-servers-v1}.yaml`, and a grep
     for `oauth|dcr|issuer|credential` returns zero hits — the executor re-verifies and records it.

## Sources

### Primary (HIGH confidence)

- `https://www.rfc-editor.org/rfc/rfc9207.html` — §2 `iss` inclusion, §2.3 metadata flag, §2.4 client validation (form-decode, RFC 3986 §6.2.1 simple string comparison, reject-and-do-not-proceed, reject-when-absent-from-supporting-AS)
- `https://www.rfc-editor.org/rfc/rfc8414.html` — §3.1 well-known URI construction with worked path/no-path examples; §5 Compatibility Notes (try the RFC 8414 transformation first, fall back to OIDC Discovery's)
- `https://modelcontextprotocol.io/specification/draft/basic/authorization` — Authorization Response Validation (the 4-row table, the record requirement, the no-normalization enumeration, the error-response clause); Refresh Tokens (SEP-2207 landed text); Overview (DCR deprecation); Scope Selection / Step-Up
- `https://modelcontextprotocol.io/specification/draft/basic/authorization/authorization-server-discovery` — the ordered candidate lists (SEP-2351 landed text) and the RFC 8414 §3.3 validation MUST with its attacker worked example
- `https://modelcontextprotocol.io/specification/draft/basic/authorization/client-registration` — Application Type and Redirect URI Constraints (SEP-837 landed text); Authorization Server Binding (SEP-2352 landed text); CIMD
- `https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization` — the v1 baseline, for the "SHOULD use and verify state parameters" clause
- `gh pr view {2468,837,2352,2351,2207} --repo modelcontextprotocol/modelcontextprotocol` — titles, merge dates, author summaries. All five MERGED (2026-03-28 ×3, 2026-05-17, and SEP-2207)
- `https://github.com/obi1kenobi/cargo-semver-checks/…/constructible_struct_adds_field.ron` — lint description, major-bump requirement, exact trigger conditions
- **In-repo source, read directly:** `src/client/oauth.rs`, `src/client/auth.rs`, `src/client/mod.rs`, `src/error/mod.rs`, `src/server/auth/oauth2.rs`, `src/server/auth/provider.rs`, `src/shared/pkce.rs`, `src/shared/mod.rs`, `src/shared/sse_optimized.rs`, `src/shared/streamable_http.rs`, `tests/v2_bounded_reads_tripwire.rs`, `tests/oauth_dcr_integration.rs`, `examples/web-channel-client/client/src/lib.rs`, `cargo-pmcp/src/commands/auth_cmd/{cache.rs,login.rs}`, `Cargo.toml`, `Makefile`, `.github/workflows/ci.yml`
- **In-repo planning docs:** `116-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md`, `.planning/research/{STACK,PITFALLS}.md`, `113/deferred-items.md` §D-113-V/§D-113-W, `114/deferred-items.md` §D-114-V
- **Platform reference (read):** `pmcp-run/amplify/functions/durable-agent-lambda/src/mcp/{cognito_external_provider.rs,outbound_oauth_provider.rs}` header constraints

### Measured in this session (HIGH confidence — reproducible commands recorded inline)

- `curl -o /dev/null -w "%{http_code}"` × 3 against Microsoft Entra ID discovery URL forms → 200 / 404 / 404 (Pitfall 2)
- `make doc-check` → exit 2, 28 `^error`, per-file distribution (Pitfall 7)
- `cargo nextest list --features full -E 'binary(oauth_dcr_integration)'` → 0; `-E 'binary(/oauth/)'` → 0; `-E 'test(/dcr/)'` → 6 server-side lib tests only (Pitfalls 3, 5)
- `cargo nextest list --features oauth …` → compile failure, 4 errors in `examples/s51_v2_tasks_agent.rs` (Pitfall 4)
- `grep -rn "oauth2::" cargo-pmcp/src/` → 14 hits, all in `deployment/targets/pmcp_run/auth.rs`; 0 in `commands/` (Pitfall 6)
- `grep -rn '"iss"' src/` → 0 hits (AUTH-01 is greenfield)
- `grep -rn "OidcDiscoveryMetadata {" ` → 4 hits, 2 non-definition construction sites (A1)
- Toolchain probe: 12 tools, versions in `## Environment Availability`

### Secondary (MEDIUM confidence)

- Phase 114/115 SUMMARY and STATE.md figures for `make quality-gate` (4899 tests), semver dual-baseline, `make test-feature-flags` redness — transcribed, not re-measured this session (see Assumptions A2)
- D-113-V's population of 31 reads across 4 files — transcribed from its recorded measurement (Assumptions A6)

### Tertiary (LOW confidence)

- None. No claim in this document rests on an unverified web search.

## Metadata

**Confidence breakdown:**

- **Standard stack: HIGH** — nothing is being added; every library cited was read from `Cargo.toml`
  or from the source that already uses it.
- **Architecture: HIGH** — the recommended tiering is not invented; `src/shared/pkce.rs` is an
  existing, ungated, wasm-clean, crate-root-re-exported instance of exactly the pattern D-05/D-06
  ask for, with a second real consumer (`examples/web-channel-client/`) already in tree.
- **Specifications: HIGH** — all normative text quoted verbatim from RFC editor and
  modelcontextprotocol.io; all five SEP PRs confirmed merged via `gh`.
- **Pitfalls: HIGH** — nine of ten are measured in this session with reproducible commands; the
  tenth (Pitfall 10) is carried from milestone research and independently corroborated by the
  spec's own table.
- **Decisions Requiring Amendment: HIGH** — A1 rests on a read of the struct definition plus the
  official lint semantics; A2 on a read of the variant definition and the private helper it would
  need; A3 on the SEP PR bodies and the landed spec text; A4 on the landed spec paragraph structure.
- **Runtime state / migration: MEDIUM-HIGH** — both cache formats read directly; the
  older-`cargo-pmcp`-hard-errors forward-compat trap is read from `cache.rs:74-80` but its
  operational blast radius (how many installed binaries) is unknowable from here.

**Research date:** 2026-08-02
**Valid until:** 2026-09-01 for the in-repo findings (stable). **2026-08-16 for the spec quotations**
— the MCP draft is actively moving toward the 2026-07-28 final, and the `iss` table and discovery
order are the two most load-bearing items; re-fetch both pages at plan time.
