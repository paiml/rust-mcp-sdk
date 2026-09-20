# Phase 116: Auth Hardening SEPs - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-02
**Phase:** 116-auth-hardening-seps
**Areas discussed:** iss strictness signal + platform/client seam, Semver-safe struct extension, Issuer-keyed credentials, Phase boundary / adjacent gaps

---

## iss strictness signal + platform/client seam

### Q1 — What signal decides strict vs lenient iss validation at the callback?

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid: floor + trigger | Two independent rules: always validate iss when present (both eras, no config, cannot break v1); require-present triggered by the RFC 9207 discovery flag or an explicit override | ✓ |
| Pure discovery-driven | Follow RFC 9207 literally and only that; zero new config surface, but a v2 deployment whose AS omits the flag silently stays lenient | |
| Explicit era/config on OAuthConfig | Caller declares intent up front; matches AUTH-01's wording most literally but adds a field to a constructible pub struct and burdens every caller | |

**User's choice:** Hybrid: floor + trigger
**Notes:** Framed by the discovery that the OAuth flow runs *before* any MCP connection exists, so `Client::era()` (`src/client/mod.rs:669`) is unavailable at callback time — "v2" must be represented by something other than a negotiated era.

### Q2 — What is the returned iss compared against, and with what comparison semantics?

| Option | Description | Selected |
|--------|-------------|----------|
| metadata.issuer, exact match | Compare against the value the AS published in discovery, RFC 9207 exact string comparison | ✓ |
| Exact match + one documented normalization | Same anchor, but normalize a single trailing slash to hedge Auth0-vs-Cognito inconsistency | |
| Compare against effective issuer | Reuse `config.issuer.or(metadata.issuer)` at `oauth.rs:505` so the file has one issuer concept | |

**User's choice:** metadata.issuer, exact match
**Notes:** Rationale accepted that the mix-up attack is "response came from a different AS than the one whose metadata I fetched," making the *discovered* issuer the correct anchor; `config.issuer` is a user-typed discovery seed.

### Q3 — How should an iss validation failure surface?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse the marker pattern | `ISS_MISMATCH_MARKER` + constructor + `is_*` predicate on the existing `Authentication` variant's `data.pmcpError`, mirroring `RETIRED_ON_V2_MARKER` | ✓ |
| Plain Error::internal with a precise message | Matches the rest of `src/client/oauth.rs`; smallest diff, but no programmatic discriminator | |
| Marker pattern + hardened browser page | Adds a dedicated failure page naming the expected issuer; widens the diff into the HTML path | |

**User's choice:** Reuse the marker pattern
**Notes:** Driven by the finding that `Error` is a plain `thiserror` enum with no `#[non_exhaustive]`, so a new variant would be semver-major. Phase 113 had already solved this exact problem.

### Q4 — What shape should the iss-validation override take?

| Option | Description | Selected |
|--------|-------------|----------|
| Builder method + env override | Inherent method on `OAuthHelper` (semver-minor) plus `PMCP_OAUTH_ISS_VALIDATION`; precedence env > builder > discovery | ✓ |
| Builder method only | No ambient environment reaching into a security decision; but no ops lever short of a redeploy | |
| No relax path — force-on only | Override can only make validation stricter; cleanest posture, but a real breakage needs a patch release | |

**User's choice:** Builder method + env override

### Q5 — How far should the phase go in shaping platform-reusable seams?

| Option | Description | Selected |
|--------|-------------|----------|
| Primitives + store seam | Pure transport-free iss function + credential store trait; loopback listener and file store become default callers/impls | ✓ |
| Primitives + store seam + challenge builder | Adds a server-side `WWW-Authenticate` builder for SEP-2350 step-up; honest scope expansion | |
| Pure functions only, keep file store | Smallest diff; SEP-2352 would ship in a form no hosting target can use | |

**User's choice:** Primitives + store seam
**Notes:** Raised by the user's account of the pmcp.run platform architecture — a single API-GW fronting all hosted MCP servers, with `oauth-proxy`/`mcp-proxy` Lambdas performing OAuth and forwarding token/claims in a header so the MCP developer never repeats the platform mechanism. The user asked whether the MCP spec requires tighter server-side integration and stressed that other hosting targets (Lambda, Cloudflare Workers, Cloud Run) are expected to offer similar mechanisms, and that the SDK must serve both platform builders and single-server operators. Verified against `research/STACK.md:30`: none of the six SEPs touch resource-server/JWT validation, so the platform shape is unaffected — but the flaw surfaced was that the iss check would have landed inside `authorization_code_flow`, which calls `webbrowser::open()` and binds a loopback listener that no Lambda can use.

### Q6 — Where should the reusable primitives live, given `oauth` is wasm-excluded?

| Option | Description | Selected |
|--------|-------------|----------|
| Wasm-clean, outside the oauth gate | Pure fn + trait carry no wasm32 exclusion and no reqwest/webbrowser/dirs dep; browser flow stays gated | ✓ |
| New oauth-core feature flag | Same split, named explicitly; more discoverable but adds feature-matrix and CI surface | |
| Keep everything behind oauth | Simplest; locks Cloudflare Workers out by construction as a stated limitation | |

**User's choice:** Wasm-clean, outside the oauth gate
**Notes:** Grounded in `src/client/mod.rs:46` (`#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`) and `Cargo.toml:216` — the whole OAuth module is compiled out on wasm32 today.

### Q7 — What should the credential-store seam be keyed by?

| Option | Description | Selected |
|--------|-------------|----------|
| (issuer, opaque account scope) | Async trait, I/O-free construction, issuer + a caller-supplied scope the SDK never interprets | ✓ |
| Issuer only, flat | Minimum SEP-2352 asks for; multi-tenant consumers each invent their own composite-key convention | |
| (issuer, MCP server URL, account) | Most precise, handles per-server scope divergence; bigger key surface, partly re-litigates SEP-2352 | |

**User's choice:** (issuer, opaque account scope)
**Notes:** Reframed by the user's second context drop — pmcp.run now hosts AI agents as configuration (LLM + instructions + MCP servers), executed by a Durable Lambda running the ReAct loop and using pmcp as a *client* against N servers with M different OAuth providers, where the user logs in once and the SDK should handle refresh. Reference supplied: `/Users/guy/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda`. Reading it surfaced ~2,460 lines of hand-rolled outbound OAuth and three design constraints adopted verbatim (I/O-free construction, tokens never logged raw, fallback as a real mechanism because the client installs one `AuthProvider` per request).

### Q8 — How should the non-interactive (headless) path be expressed?

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit opt-in mode | Builder selection where refresh failure returns the typed reauth error and the browser path is unreachable by construction | ✓ |
| Auto-detect headless | Environment sniffing; zero caller changes but fails in the unusual direction and is hard to test | |
| Never auto-fall-through, any mode | One code path, no mode flag; but a behavior change for every existing `get_access_token()` caller | |

**User's choice:** Explicit opt-in mode
**Notes:** Motivated by the finding that `get_access_token()` (`oauth.rs:428-480`) silently falls through on refresh failure into a 5-minute wait on a loopback listener — five minutes of burned wall clock per attempt in a Lambda.

---

## Semver-safe struct extension

### Q1 — How should application_type be added without a major bump?

| Option | Description | Selected |
|--------|-------------|----------|
| Typed accessors over the extra map | Inherent methods reading/writing the existing `#[serde(flatten)] extra`; semver-minor, wire-identical | ✓ |
| Raw extra map, documented only | Zero new API; no compile-time safety and a required v2 field invisible in rustdoc | |
| Typed field + major bump | Honest typing; breaks 10 in-repo struct literals and contradicts the additive-2.x-minor constraint | |

**User's choice:** Typed accessors over the extra map
**Notes:** Public reachability verified before framing: `pub mod auth` (`src/server/mod.rs:48`) → `pub mod provider` (`auth/mod.rs:55`) plus a `pub use` re-export at `auth/mod.rs:86`. `DcrRequest` is all-pub-field with no `#[non_exhaustive]`, so a new field is `constructible_struct_adds_field` = major.

### Q2 — How is the application_type value determined?

| Option | Description | Selected |
|--------|-------------|----------|
| Derive from redirect_uris | Loopback/custom scheme ⇒ native, https non-loopback ⇒ web; mixed vec is an explicit error | ✓ |
| Hardcode "native" | Correct for every flow pmcp ships today; silently wrong for a platform oauth-proxy | |
| Caller must set it explicitly | Zero magic; but a behavior change for the SDK's own DCR path and risks tripping the lenient-v1 promise | |

**User's choice:** Derive from redirect_uris

### Q3 — Era-gated send, and what happens on echo mismatch?

| Option | Description | Selected |
|--------|-------------|----------|
| Send always; record + warn on echo mismatch | Standard OIDC field since 2014; never fails the registration | ✓ |
| Send always; strict echo check when the require-trigger is on | One strictness concept across AUTH-01/02; can fail on a spec-permitted AS choice | |
| Era-gated send (v2 only) | v1 request body provably unchanged; needs era plumbing DCR doesn't have, for no safety gain | |

**User's choice:** Send always; record + warn on echo mismatch

---

## Issuer-keyed credentials

### Q1 — What happens to credentials already on disk?

| Option | Description | Selected |
|--------|-------------|----------|
| Split: discard core, migrate cargo-pmcp | Core's issuer-less cache cannot be re-keyed without guessing; cargo-pmcp's records issuer and migrates 1→2 | ✓ |
| Discard both, one clean cut | One behavior, no migration code; throws away perfectly migratable cargo-pmcp logins | |
| Dual-read compatibility window | Smoothest upgrade; most code, keeps a legacy path alive, and still can't correctly key core's entries | |

**User's choice:** Split: discard core, migrate cargo-pmcp
**Notes:** The asymmetry is the whole decision — `TokenCache` (`oauth.rs:151`) has no issuer field at all, while `TokenCacheEntry` (`cargo-pmcp/.../cache.rs:34`) records one per entry.

### Q2 — Should a change in a server's advertised issuer be detected and surfaced?

| Option | Description | Selected |
|--------|-------------|----------|
| Track last-seen issuer, warn loudly | Non-blocking warning naming old and new issuer; unattended agents still self-heal | ✓ |
| No tracking — the key does the work | SEP-2352's letter satisfied with zero extra state; an AS substitution is completely invisible | |
| Track and hard-fail until cleared | Strongest posture; converts legitimate IdP migrations into per-server outages | |

**User's choice:** Track last-seen issuer, warn loudly
**Notes:** Preceded by the observation that issuer-keyed storage makes SEP-2352's two mandates true *by construction* (a new issuer is simply a cache miss), leaving only detection as an open question.

### Q3 — Does cargo-pmcp adopt core's store, or keep its own?

| Option | Description | Selected |
|--------|-------------|----------|
| Converge on one store + one file | Core owns trait and file impl; cargo-pmcp drops its parallel TokenCacheV1 implementation | ✓ |
| Core ships the trait; cargo-pmcp keeps its own | Least churn; two implementations and two files persist indefinitely | |
| Core trait only, no default file impl | Cleanest for wasm/Lambda; every consumer rewrites the same file store | |

**User's choice:** Converge on one store + one file

---

## Phase boundary / adjacent gaps

### Q1 — Which adjacent items are in scope? *(multi-select)*

| Option | Description | Selected |
|--------|-------------|----------|
| state validation (CSRF) | State is generated inline as a temporary at `oauth.rs:712` and never bound, so it is structurally impossible to validate | ✓ |
| .well-known suffix as a CODE fix | RFC 8414 §3.1 requires inserting, not appending, the well-known segment; path-bearing issuers resolve wrong today | ✓ |
| D-113-V's 31 unbounded auth reads | Roadmap-assigned to this phase, Status OPEN; fix shape and tripwire already exist from Phase 113.1 | ✓ |

**User's choice:** All three
**Notes:** A self-correction was recorded during this exchange — the `.well-known` item had been mis-filed as "adjacent" when SEP-2351 is in fact one of the three clarifications AUTH-03 explicitly names, making it squarely in scope; the only real question was docs-only versus code fix.

### Q2 — How should SEP-2207 and SEP-2350 be treated?

| Option | Description | Selected |
|--------|-------------|----------|
| Fix refresh; defer 2350 whole | SEP-2207 lands as a real fix; scope accumulation ships as one coherent feature with its server half | ✓ |
| Fix refresh + SEP-2350 client half | Adds scope-union on re-auth now; lands half a feature whose trigger doesn't exist yet | |
| Documentation-only for both | Tightly bounded; but ships a headless mode sitting on a refresh path that destroys its own token | |

**User's choice:** Fix refresh; defer 2350 whole
**Notes:** Grounded in three defects found in `refresh_token()` (`oauth.rs:916-949`): the stored refresh token is destroyed whenever the AS omits one from the response (`#[serde(default)]` → `None` → written over the good token at `:987`); DCR flows can never refresh at all because `client_id` is read from config where a DCR-issued id never lives; and `scope` is never sent.

### Q3 — What booking posture should AUTH-01/02/03 take?

| Option | Description | Selected |
|--------|-------------|----------|
| Book [x] on measured evidence | No publication hold applies; Phase 115's citation discipline, booked only after the gates exit 0 | ✓ |
| Add a SPEC-RECHECK gate first | Mirrors 113/114; creates a hold where no publication dependency exists | |
| Book [x], no citation discipline | Faster; drops the mechanism that caught four premature closures in Phase 115 | |

**User's choice:** Book [x] on measured evidence
**Notes:** Framed against roadmap `D-15`'s explicit warning that a `[~]` must not be inherited "by habit," and the fact that these SEPs derive from published RFCs rather than from `schema.json` or the still-unpublished `ext-tasks` repo.

---

## Claude's Discretion

None. Every gray area was decided explicitly by the user — no "you decide" selections were made.

Left to the planner as ordinary implementation latitude: wave/plan decomposition, module naming
and exact placement for the wasm-clean primitives, the mixed-`redirect_uris` error type, and
fuzz/property target design under the house ALWAYS requirements.

## Deferred Ideas

- **SEP-2350 step-up scope accumulation** — deferred whole (both halves), so it ships as one
  coherent feature rather than a client half with no trigger.
- **Extract `UpstreamAuthDecorator` + `HEADER_UPSTREAM_AUTH` into the SDK** — a standing request
  written into the durable agent's own source, deliberately authored for copy-paste extraction.
- **Extract the outbound-OAuth vending core** (`OutboundOAuthCore`: per-server vending, TTL cache,
  `OnceCell` inflight dedup, `reauth_required` → `ConsentRequired` on both paths) — ~987 lines
  hand-rolled in the durable agent, and no phase exists for it in
  `docs/design/agents-teams-sdk-extraction-plan.md`.
- **Cognito internal/external providers and the CognitoExternal→CognitoInternal fallback chain** —
  platform-specific policy, stays in pmcp.run.
- **Token-at-rest encryption in core** — the platform uses KMS; plaintext file is the status quo.
- **Whether the store trait carries token refresh itself or only load/save/delete.**
- **Typed accessors for the other RFC 7591 fields `DcrResponse` drops into `extra`.**
