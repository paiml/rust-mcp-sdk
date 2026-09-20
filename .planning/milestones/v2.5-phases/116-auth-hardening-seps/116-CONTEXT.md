# Phase 116: Auth Hardening SEPs - Context

**Gathered:** 2026-08-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Harden pmcp's **hand-rolled OAuth client stack** to the six v2 auth-hardening SEPs, strict on
v2 and lenient on v1, so existing deployments keep working. Zero new crates — no
`oauth2`, no `openidconnect`. Additive 2.x minor.

**The six SEPs and how each lands:**

| SEP | Subject | Treatment in this phase |
|---|---|---|
| SEP-2468 | RFC 9207 `iss` validation | **Code** — AUTH-01 |
| SEP-837 | DCR `application_type` | **Code** — AUTH-02 |
| SEP-2352 | Issuer-keyed credential storage | **Code** — AUTH-03 |
| SEP-2207 | Refresh-token requests from OIDC servers | **Code** — real defect cluster found, not docs-only (D-14) |
| SEP-2351 | `.well-known` discovery suffix | **Code** — RFC 8414 §3.1 path insertion is wrong today (D-13) |
| SEP-2350 | Step-up scope accumulation | **Deferred whole** — travels with its server-side half |

**Also in scope (accepted during discussion):** OAuth `state` validation (D-12) and
D-113-V's 31 unbounded auth-surface reads (D-15).

**This phase does NOT touch the MCP server side.** No resource-server change, no JWT
validation change, no `ProxyProvider` change, no `oauth_passthrough` change. A pmcp server
deployed behind a platform gateway (pmcp.run's API-GW + `oauth-proxy`/`mcp-proxy` Lambdas)
is unaffected. `research/STACK.md:30` states this directly: the SEPs "touch the client
authorization-code flow and registration request body, not JWT verification."

</domain>

<decisions>
## Implementation Decisions

### AUTH-01 — RFC 9207 `iss` validation

- **D-01: Strictness is a hybrid of a floor and a trigger, NOT an era check.** The OAuth flow
  runs *before* any MCP connection exists, so `Client::era()` (`src/client/mod.rs:669`) is not
  available at callback time. Instead:
  - **Floor (unconditional, both eras, no config):** if `iss` is present in the callback, always
    validate it. This cannot break any existing deployment — a v1 AS that never sends `iss`
    never trips it — so v1 gets *strictly safer*, not merely unchanged.
  - **Trigger (require `iss` to be present):** the RFC 9207 discovery flag
    `authorization_response_iss_parameter_supported`, **or** an explicit caller override.
    `OidcDiscoveryMetadata` (`src/server/auth/oauth2.rs:172`) does not parse this field today —
    add it additively.
- **D-02: Compare against `metadata.issuer`, exact string match.** The anchor is the value the
  AS itself published in its discovery document — *not* `config.issuer` (a user-typed discovery
  seed) and *not* the effective issuer at `src/client/oauth.rs:505`. The mix-up attack being
  defended against is "this response came from a different AS than the one whose metadata I
  fetched," so the discovered issuer is the semantically correct anchor. **No trailing-slash
  normalization** — RFC 9207 says exact, and conformance will test exact.
- **D-03: Failure surfaces via the Phase-113 marker pattern, not a new `Error` variant.**
  `Error` is a plain `thiserror` enum with **no** `#[non_exhaustive]`, so a new variant is
  semver-major. Follow `RETIRED_ON_V2_MARKER` / `MRTR_ROUND_LIMIT_MARKER`
  (`src/error/mod.rs:114-131`) exactly: an `ISS_MISMATCH_MARKER` const + an
  `Error::iss_mismatch(expected, actual)` constructor + an `Error::is_iss_mismatch()` predicate,
  riding on the existing `Authentication` variant's `data.pmcpError`. Gives conformance fixtures
  and downstream callers a stable programmatic discriminator instead of message substrings. The
  authorization code is **never redeemed** on failure; the existing failure HTML is unchanged.
- **D-04: Override is a builder method plus an env var.** `OAuthHelper::with_iss_validation(…)`
  as an **inherent method** (semver-minor — deliberately NOT a field on `OAuthConfig`), plus a
  `PMCP_OAUTH_ISS_VALIDATION` env var so an operator can act without a redeploy, matching the
  house env-var config-injection philosophy. **Precedence: env var > builder > discovery flag.**

### Platform seam — the SDK must serve hosting platforms AND the single-server case

- **D-05: The hardened logic lands as transport-free primitives; the interactive CLI flow is one
  caller, not the only caller.** `OAuthHelper::authorization_code_flow` calls
  `webbrowser::open()` (`src/client/oauth.rs:718`) and binds a loopback `TcpListener` — a Lambda
  or Workers `oauth-proxy` can do neither. So: `iss` validation lands as a **pure function** over
  (query params, discovered metadata) that the loopback listener and a platform redirect handler
  both call; credential storage lands behind a **trait**, not a hardcoded path. Reshapes *where*
  code lands. No new backends are built, no server-side change, no behavior change for a server
  behind a platform gateway.
- **D-06: The primitives are wasm-clean and live OUTSIDE the `oauth` feature gate.**
  `src/client/oauth.rs` is gated `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`
  (`src/client/mod.rs:46`) and `oauth = ["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]`
  (`Cargo.toml:216`) — so a Cloudflare Workers platform gets **zero** of it. The pure comparison
  function and the store trait need none of those deps. They go in a module with no wasm32
  exclusion and no reqwest/webbrowser/dirs dependency. The browser flow, loopback listener and
  file store stay behind `oauth` + `not(wasm32)` as the default callers. **Needs a wasm32 build
  fence in CI** or this silently regresses.
- **D-07: The credential store is keyed by `(issuer, opaque account scope)`.** Async trait,
  **I/O-free construction** (no `std::env` reads, no network, no disk in the constructor — all
  values are constructor parameters). The account scope is caller-supplied and **never
  interpreted by the SDK**: a Cognito sub, a tenant id, or empty for the single-user CLI. This
  satisfies SEP-2352's "never reuse credentials across ASes" literally while letting a
  multi-tenant platform key per user without the SDK dictating identity. Ships a file impl and an
  in-memory impl; DynamoDB/KMS impls stay with the platform.
- **D-08: Headless operation is an explicit opt-in mode, not environment sniffing.** A builder
  selection (`Interactivity::RefreshOnly` or equivalent) under which refresh failure returns the
  typed reauth-required error **immediately** and the browser/loopback path is **unreachable by
  construction**. Today `get_access_token()` (`src/client/oauth.rs:428-480`) silently falls
  through on refresh failure into `authorization_code_flow`, binding a listener nothing can reach
  and waiting **5 minutes** — five minutes of burned wall clock per attempt in a Lambda. The
  existing constructor keeps today's interactive fall-through, so no current caller changes
  behavior.

### AUTH-02 — DCR `application_type`, without a major bump

- **D-09: Typed accessors over the existing `#[serde(flatten)] extra` map — not a new field.**
  `DcrRequest` (`src/server/auth/provider.rs:304`) is fully public (`pub mod auth` at
  `src/server/mod.rs:48` → `pub mod provider` at `auth/mod.rs:55`, plus a `pub use` re-export at
  `auth/mod.rs:86`), all-pub-field, **not** `#[non_exhaustive]`, with 10 struct-literal
  construction sites in-repo. Adding a field is `constructible_struct_adds_field` = **major**
  under `cargo semver-checks`, which has been clean 223/223 through Phase 115 and which the
  milestone's additive-2.x-minor constraint depends on. Add **inherent methods**
  (`application_type()` / `set_application_type()` on `DcrRequest`, `application_type()` on
  `DcrResponse`) that read and write the `extra` map. Inherent methods are semver-minor and the
  wire bytes are identical to a real field. **Requires a documented precedence rule** for a
  caller who also writes the raw `"application_type"` key by hand, plus a collision test.
- **D-10: The value is DERIVED from `redirect_uris`.** Loopback (`127.0.0.1` / `::1` /
  `localhost`) or a custom scheme ⇒ `"native"`; https non-loopback ⇒ `"web"`. This keeps it
  consistent with the OIDC rule that constrains the two together and stays correct if
  `redirect_port` changes. **A mixed `redirect_uris` vec is an explicit ERROR, never a silent
  pick.** The explicit setter from D-09 remains available as an override. pmcp's own DCR call
  hardcodes `http://127.0.0.1:{port}/callback` (`src/client/oauth.rs:239`) ⇒ derives `native`; a
  platform `oauth-proxy` with an https redirect ⇒ derives `web`.
- **D-11: Sent unconditionally on both eras; echo mismatch is recorded and warned, never fatal.**
  `application_type` has been a standard OIDC Dynamic Registration field since 2014 — an AS that
  doesn't want it ignores it, and era-gating would require plumbing an era into DCR that (like
  the callback, D-01) does not exist pre-connection. On the response, record what the AS actually
  registered and `tracing::warn!` on divergence, but **never fail the registration** — RFC 7591
  explicitly permits the AS to modify requested metadata. Keeps AUTH-02 a pure addition with no
  v1 breakage surface.

### Adjacent gaps accepted into scope

- **D-12: OAuth `state` validation is IN SCOPE (CSRF).** At `src/client/oauth.rs:712` the state
  value is generated **inline as a temporary** — `.append_pair("state",
  &Self::generate_code_verifier())` — never bound to a variable, so it is not merely unchecked but
  *structurally impossible* to check. The callback extracts `code` only. Bind it, retain it across
  the flow, compare before redeeming. `iss` and `state` defend the same mix-up/CSRF family and land
  on the same lines; shipping RFC 9207 validation while leaving `state` unvalidatable would be
  indefensible in review or in a conformance claim.
- **D-13: SEP-2351 is a CODE fix, not documentation.** `generic_oidc.rs:394` and `cognito.rs:270`
  build discovery URLs by naive concatenation —
  `format!("{}/.well-known/openid-configuration", issuer)` — while RFC 8414 §3.1 requires
  **inserting** the well-known segment between host and path. Any issuer with a path component
  (`https://host/tenant1` ⇒ `https://host/.well-known/oauth-authorization-server/tenant1`)
  resolves to the wrong URL today, which breaks multi-tenant IdPs.
- **D-14: SEP-2207 is a real defect cluster, not documentation.** `refresh_token()`
  (`src/client/oauth.rs:916-949`) has three genuine bugs, all of which directly block D-08's
  headless mode from actually working:
  1. **The stored refresh token is destroyed on every successful refresh** against an AS that
     doesn't re-issue one. `TokenResponse.refresh_token` is `#[serde(default)]`, so an omitted
     field deserializes to `None`, and `cache_token` writes that `None` over the good token
     (`:987`). Many OIDC ASes omit it meaning "keep the old one." An unattended agent gets exactly
     one refresh cycle, then a forced re-login. **Fix: preserve the stored token when the response
     omits one.**
  2. **DCR flows can never refresh at all.** `client_id` is read from `self.config.client_id`
     (`:922`), but under DCR the client_id is *issued* and lives in `AuthorizationResult`, never in
     config — so it errors `"cannot refresh token without a cached client_id"`. **Fix: source
     `client_id` from the D-07 issuer-keyed store**, which holds it per `(issuer, account)`.
  3. **`scope` is never sent on refresh**, which some OIDC ASes require or use to narrow.
- **D-15: D-113-V's 31 unbounded auth-surface reads are IN SCOPE.** Bound the reviewed-unbounded
  whole-body reads — `providers/generic_oidc.rs` (11), `providers/cognito.rs` (9),
  `client/oauth.rs` (6), `client/auth.rs` (5) — and **widen the existing tripwire's SCOPE FENCE**
  to cover those four files. Roadmap-assigned to this phase with Status **OPEN**; the fix shape
  and the tripwire both already exist from Phase 113.1; these are the exact files this phase
  edits. Note the tripwire's scanner already *would* find all of them (it strips whitespace and
  handles rustfmt-broken chains, pinned by a test at `v2_bounded_reads_tripwire.rs:1050`) — only
  the scope fence keeps them unreported. `refresh_token`'s own error path
  (`response.text().await.unwrap_or_default()`, `:941`) is one of these.

### AUTH-03 — issuer-keyed credential storage

- **D-16: SEP-2352's two mandates fall out of the key shape, not enforcement code.** Because the
  key *includes* the issuer (D-07), "MUST NOT reuse credentials across authorization servers" is
  true by construction — a server that switches AS simply misses the cache. And because the DCR
  `client_id` lives in the same record (it must: a client_id issued by AS-A is meaningless at
  AS-B), "re-register on AS change" is automatic too.
- **D-17: On-disk migration is SPLIT, because the two existing caches are not equivalent.**
  - **pmcp core `~/.pmcp/oauth-tokens.json`** (`TokenCache`, `src/client/oauth.rs:151`) is a
    single flat token with **no issuer field at all**. It cannot be re-keyed without *guessing*
    which AS issued it — precisely what SEP-2352 forbids. **Discard on first read; one forced
    re-login.**
  - **cargo-pmcp `~/.pmcp/oauth-cache.json`** (`TokenCacheV1`,
    `cargo-pmcp/src/commands/auth_cmd/cache.rs:34`) is multi-entry, keyed by normalized server
    URL, and **records `issuer` per entry**. **Real `schema_version` 1→2 migration**, re-keying to
    `(issuer, account)` with account empty. Every existing login is preserved.
- **D-18: Track last-seen issuer per MCP server URL and warn loudly on change — non-blocking.**
  Issuer-keyed storage makes an AS substitution *safe* but **invisible**: the user is simply walked
  through a fresh login at an IdP they didn't expect. Record the last issuer seen per server and,
  on change, emit a prominent warning naming **both** old and new issuer before proceeding. An
  unattended agent still self-heals; under `RefreshOnly` (D-08) it surfaces as the typed
  reauth-required error with the issuer change named. Not a hard fail — legitimate issuer changes
  (tenant moves, provider migrations) do happen, and hard-failing converts a real operational event
  into an outage.
- **D-19: cargo-pmcp adopts core's store — converge on ONE store and ONE file.** Today a single
  machine can carry two unrelated OAuth caches with different formats and semantics. Core owns the
  trait and the default file impl; cargo-pmcp **drops its parallel `TokenCacheV1` implementation**,
  with its existing `oauth-cache.json` as the migration source and the surviving path.
  `cargo pmcp auth login/logout/token` become thin wrappers over the same seam a platform would
  implement. Costs a cargo-pmcp version bump and a dep pin; `auth logout` semantics must be
  preserved.

### Requirement booking

- **D-20: AUTH-01/02/03 book `[x]` on measured evidence — NO publication hold, and `[~]` is not
  inherited.** These SEPs derive from published RFCs (9207, 7591, 8414) and published spec prose,
  with **no** dependency on `schema.json` or on the still-unpublished `ext-tasks` repo. The
  roadmap's `D-15` warns in as many words against inheriting `[~]` by habit. Follow Phase 115's
  discipline: each booking **CITES** the artifact plus a named test binary and count so a future
  reader can re-derive rather than trust, and the booking task runs **only AFTER**
  `make quality-gate` and the PR-blocking `pmat quality-gate --checks complexity` both exit 0.

### Claude's Discretion

No area was delegated with "you decide." Every decision above was selected explicitly. Left to
the planner: wave/plan decomposition, module naming and exact placement for the wasm-clean
primitives, the mixed-`redirect_uris` error type, and the fuzz/property target design under the
house ALWAYS requirements.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone research (auth-specific sections)
- `.planning/research/STACK.md` §(d) lines 112-134 — the hand-rolled decision: `iss` is
  `url::Url::query_pairs()` logic, `application_type` is one serde field, issuer-binding is a
  storage-key change. **Explicitly rejects `oauth2` 5.x / `openidconnect`** as reqwest-coupled and
  not wasm-default-clean.
- `.planning/research/PITFALLS.md` Pitfall 10 (lines 172-186) — unconditional hardening fails
  closed and 401s existing deployments; gate on version, preserve the documented `stateless()` +
  `AllowedOrigins::any()` proxy exception.
- `.planning/research/FEATURES.md` line 34 — the canonical enumeration of all six SEPs with their
  PR numbers, and the note that DCR itself is deprecated in favor of Client ID Metadata Documents
  (PR #2858) while staying for backcompat.
- `.planning/research/SUMMARY.md` §"Phase 5: Auth-Hardening SEPs" (lines 243-253).

### Deferred item this phase owns
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md` §`D-113-V`
  (from line 1280) — the measured four-file table, the stated counting method (naive grep vs
  rustfmt-split chains), the two named exclusions with line numbers, the fix shape, and the stated
  reason the tripwire's scope fence was not widened. **Owner: Phase 116. Status: OPEN.**

### Prior-art patterns this phase must follow
- `src/error/mod.rs:114-131` — the marker-const + constructor + `is_*` predicate pattern
  (`RETIRED_ON_V2_MARKER`, `MRTR_ROUND_LIMIT_MARKER`) that D-03 replicates. The rustdoc's "do not
  change this string" compatibility note applies to the new marker too.
- `.planning/ROADMAP.md` §"Phase 116" (line 2614) and the v2.5 Non-goals (line 2216) — zero new
  runtime dependencies; auth SEPs land as source changes.

### Platform consumer — the reason the seams exist (D-05..D-08)
- `/Users/guy/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/mcp/outbound_oauth_provider.rs`
  (987 lines) — `OutboundOAuthCore` + `ServerAuthProvider`. Read lines 1-16 for the constraints
  that became design inputs: per-execution shared state, `tokio::sync::Mutex`, `OnceCell` inflight
  dedup against cache stampedes, and `reauth_required` → `ConsentRequired` propagated on **both**
  the connection-discovery path and the tool-call reply path.
- `/Users/guy/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/mcp/cognito_external_provider.rs`
  lines 1-25 — three hard-won constraints D-07 adopts: **I/O-free construction** (no `std::env`
  inside the provider, all values as constructor params), **token never logged raw** (sha256
  prefix, enforced by a mirrored static-source invariant test), and **fallback must be a real
  mechanism** because "the MCP client installs ONE `AuthProvider` per request."
- `/Users/guy/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/mcp/upstream_auth_decorator.rs`
  lines 1-20 — a **standing SDK-extraction request (D-12 in that repo)**: written to be
  copy-pasted into `rust-mcp-sdk` as a standalone module and re-exported without rewriting call
  sites. Deferred here, but do not lose it.
- `docs/design/agents-teams-sdk-extraction-plan.md` — Phases A–F. **Contains no phase for
  outbound-OAuth extraction**, which is why the vending-core deferral below has no home yet.

### Specifications
- RFC 9207 (OAuth 2.0 Authorization Server Issuer Identification) — the `iss` parameter, the
  `authorization_response_iss_parameter_supported` metadata flag, and the **exact string
  comparison** requirement (D-02).
- RFC 8414 §3.1 (OAuth 2.0 Authorization Server Metadata) — the well-known URI construction rule
  D-13 fixes: the well-known segment is **inserted** between host and path, not appended.
- RFC 7591 §3.1 (Dynamic Client Registration) + OpenID Connect Dynamic Client Registration §2 —
  `application_type` values `web`/`native` and their redirect-URI constraints (D-10); and the AS's
  right to modify requested metadata (D-11).
- SEP PRs, from `research/FEATURES.md:166`: [2468 iss](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2468) ·
  [837 application_type](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/837) ·
  [2352 issuer-binding](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2352)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp::shared::streamable_http::AuthProvider`** — the per-transport token-vending seam
  **already exists and is already in production use** by the durable agent
  (`cognito_external_provider.rs:35`). This phase supplies the OAuth machinery *behind* that
  trait, not a replacement for it.
- **`src/error/mod.rs:114-131`** — the marker/constructor/predicate error-identity pattern. D-03
  copies its shape exactly; no new `Error` variant needed.
- **`#[serde(flatten)] extra: HashMap<String, Value>`** on both `DcrRequest`
  (`provider.rs:349`) and `DcrResponse` (`provider.rs:380`) — the semver-free carrier D-09 uses.
- **`OidcDiscoveryMetadata`** (`src/server/auth/oauth2.rs:172`) — already carries `issuer` and is
  already fetched on every flow; D-01's trigger flag is an additive `Option<bool>` field.
- **`cargo-pmcp`'s `TokenCacheV1`** (`auth_cmd/cache.rs`) — atomic writes, `schema_version`,
  `normalize_cache_key`, `BTreeMap` entries. D-19 keeps the *file and its data* and migrates the
  *implementation* into core.
- **Phase 113.1's bounded-read tripwire** (`tests/v2_bounded_reads_tripwire.rs`) — its scanner
  strips whitespace and handles rustfmt-broken chains (pinned by a test at `:1050`), so widening
  the scope fence (D-15) is the whole job; the detector needs no change.
- **`Error::Authentication(String)`** — the existing variant D-03's marker rides on.

### Established Patterns
- **`Error` is NOT `#[non_exhaustive]`** and `DcrRequest`/`OAuthConfig` are all-pub-field
  constructible structs — so the semver-safe extension shapes are: inherent methods, constructor
  functions, marker constants, and the `extra` flatten map. Never a new field or variant.
- **`make quality-gate` is the gate, not bare cargo commands** — it runs `make lint` with
  `--features "full"` plus pedantic + nursery. `cargo semver-checks` and the PR-blocking
  `pmat quality-gate --checks complexity` (cog ≤ 25) both apply.
- **Phase 115's evidence discipline:** fences must be OBSERVED to fail before the fix (negative
  controls), and derived invariants beat restated ones — three rounds of SCHM-01 reopened because
  every fence restated the code's own rule. Security assertions in this phase should prefer
  invariants derived from the RFC over invariants restated from the implementation.
- **House ALWAYS requirements** apply to every new feature: fuzz, property, unit, and a runnable
  `cargo run --example`.

### Integration Points
- `src/client/oauth.rs:695-720` — the loopback callback task. Receives D-01/D-02 (`iss`) and D-12
  (`state`). The `oneshot` channel currently carries a bare `Option<String>` code; it will need to
  carry the full parsed response (code, state, iss, error) or validate in-task.
- `src/client/oauth.rs:239-257` — the `DcrRequest` construction site. Receives D-10's derivation.
- `src/client/oauth.rs:916-949` + `:952-992` — `refresh_token` / `load_cached_token` /
  `cache_token`. Receives D-14's three fixes and D-17's discard behavior.
- `src/server/auth/providers/generic_oidc.rs:394`, `providers/cognito.rs:270` — discovery URL
  construction. Receives D-13.
- `cargo-pmcp/src/commands/auth_cmd/` (`cache.rs`, `login.rs`) — receives D-17's migration and
  D-19's convergence.
- `src/client/mod.rs:46` + `Cargo.toml:216` — the gating that D-06 must place the new primitives
  *outside* of.

</code_context>

<specifics>
## Specific Ideas

- **"The MCP developer doesn't need to repeat the platform mechanism."** The pmcp.run shape —
  one API-GW fronting all hosted MCP servers, `oauth-proxy`/`mcp-proxy` Lambdas doing OAuth, the
  server receiving a token or claims in a header and *trusting the platform* — must remain
  untouched by this phase, and every other hosting target (Lambda, Cloudflare Workers, Cloud Run)
  is expected to offer a similar mechanism. **Verified compatible:** none of the six SEPs push work
  into the MCP server. This is the origin of D-05/D-06.
- **The durable-agent client case is the primary consumer of AUTH-03, not the CLI.** A Durable
  Lambda ReAct loop using `pmcp::Client` talks to N MCP servers with M different OAuth providers on
  behalf of a user; the user logs in once and the SDK should handle refresh and the rest. This is
  what makes D-07 (multi-issuer key + account scope), D-08 (no browser, ever) and D-14 (refresh
  that actually survives) load-bearing rather than nice-to-have.
- **`~/.pmcp/*.json` is not a viable credential store for any hosting target.** `~` is unwritable
  on Lambda and per-container on Workers and Cloud Run. "Issuer-keyed storage" shipped as a
  hardcoded file path would be unusable by every platform named. Hence the trait (D-07).
- **Do not rediscover what the platform already learned.** I/O-free construction, tokens never
  logged raw (sha256 prefix + a static-source invariant test), and fallback as a real mechanism
  rather than a caller-side assumption are all recorded constraints in the durable-agent source —
  adopt them as SDK design inputs.

</specifics>

<deferred>
## Deferred Ideas

- **SEP-2350 step-up scope accumulation — deferred WHOLE, both halves together.** The server half
  is a `WWW-Authenticate: Bearer realm/scope/error=insufficient_scope` challenge builder; pmcp
  emits **zero** `WWW-Authenticate` anywhere today (one comment at `task_dispatch.rs:584`, no
  code). The client half (request the union of stored and newly-required scopes on re-auth) is
  implementable standalone but would have nothing to trigger it. Ships as one coherent feature in
  its own phase.
- **Extract `UpstreamAuthDecorator` + `HEADER_UPSTREAM_AUTH` into the SDK.** A standing request
  written into the durable agent's own source (its `D-12`) — the module was authored to be
  copy-pasted into `rust-mcp-sdk` and re-exported without rewriting call sites. Tiny, but new
  public surface unrelated to AUTH-01..03.
- **Extract the outbound-OAuth vending core.** The `OutboundOAuthCore` shape: per-server token
  vending, TTL cache, `OnceCell` inflight-dedup stampede prevention, `reauth_required` →
  `ConsentRequired` on both the discovery and tool-call paths. ~987 lines hand-rolled in the
  durable agent. **No phase exists for this** —
  `docs/design/agents-teams-sdk-extraction-plan.md`'s Phases A–F don't cover it, so it needs a
  roadmap slot rather than an assumption that Phase 117 absorbs it.
- **Cognito internal/external providers and the CognitoExternal→CognitoInternal fallback chain.**
  Platform-specific policy (AbsentCustody/RefreshRevoked ⇒ M2M bearer, InfraDenied ⇒ loud
  propagated error). Stays in pmcp.run.
- **Token-at-rest encryption in core.** The platform uses KMS; a plaintext `~/.pmcp` file is the
  status quo and this phase does not change it.
- **Whether the store trait carries token REFRESH itself or only load/save/delete.** Left open;
  the planner may settle it, but a deliberate answer belongs with the vending-core extraction.
- **Typed accessors for the other RFC 7591 fields `DcrResponse` currently drops into `extra`.**
  Same mechanism as D-09, but out of scope here.
- **RFC 9728 Protected Resource Metadata discovery (MCP-spec client MUST).** Deferred by owner
  decision 2026-08-02 (plan-phase research escalation). pmcp derives the AS from the MCP base URL
  directly (`src/client/oauth.rs:366-390`) instead of PRM discovery. **Named dependency:** D-18's
  SEP-2352 AS-change detection is specified in the spec as "detected via updated protected
  resource metadata" — until RFC 9728 lands, AS-change detection uses whatever provenance signal
  is available today, not the spec's stated mechanism. Needs a roadmap slot; owner: Guy.
- **RFC 8707 `resource` parameter on authorization + token requests (MCP-spec client MUST).**
  Deferred by owner decision 2026-08-02 (same escalation). pmcp sends no `resource` parameter
  (`src/client/oauth.rs:664-672`). Ships together with the RFC 9728 item above; owner: Guy.

</deferred>

---

*Phase: 116-auth-hardening-seps*
*Context gathered: 2026-08-02*
