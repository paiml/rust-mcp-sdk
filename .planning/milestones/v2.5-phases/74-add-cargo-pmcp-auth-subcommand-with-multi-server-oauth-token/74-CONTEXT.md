# Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Two complementary deliverables shipped together because the CLI work depends on the SDK work:

1. **SDK: Dynamic Client Registration (DCR, RFC 7591) support in `src/client/oauth.rs`.** Any PMCP-SDK-built client can now auto-register with OAuth servers that advertise a `registration_endpoint` instead of requiring pre-provisioned `client_id`. This is a general-purpose SDK feature — not CLI-specific.

2. **CLI: `cargo pmcp auth` command group with multi-server token cache.** Consolidates the existing OAuth PKCE flow (currently nested inside every server-connecting command via `AuthFlags`) into a dedicated command group: `login`, `logout`, `status`, `token`, `refresh`. Replaces the single-blob `~/.pmcp/oauth-tokens.json` with a per-server-keyed `~/.pmcp/oauth-cache.json`. Exposes the new SDK DCR feature via a `--client <name>` flag on `auth login` (primarily useful for testing pmcp.run's client-branded login pages).

**In scope:**

*SDK (`pmcp` crate):*
- New DCR request/response types and public API in `src/client/oauth.rs` (or a new `src/client/dcr.rs` module)
- `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` fields
- `OAuthHelper` auto-performs DCR when: (a) `dcr_enabled`, (b) `client_id` is `None`, and (c) server discovery advertises a `registration_endpoint`
- Fuzz/property/unit tests + working example demonstrating DCR against a mock server (per CLAUDE.md ALWAYS requirements)
- Semver: `pmcp` minor bump (additive public API, no breaking change)

*CLI (`cargo-pmcp` crate):*
- New top-level `cargo pmcp auth` command group with 5 subcommands: `login`, `logout`, `status`, `token`, `refresh`
- New per-server-keyed token cache at `~/.pmcp/oauth-cache.json` with `schema_version` field
- `--client <name>` flag on `auth login` that sets `OAuthConfig::client_name` for DCR
- When `--oauth-client-id` is absent (and server supports DCR), DCR fires automatically
- Existing `AuthFlags` consumers (on `test/*`, `connect`, `preview`, `schema`, `dev`, `loadtest/run`) start consulting the new cache as the lowest-precedence auth fallback
- Migrate `cargo-pmcp/src/commands/pentest.rs` from its duplicate `--api-key` flag to shared `AuthFlags`
- Semver: `cargo-pmcp 0.8.1 → 0.9.0` minor bump

**Out of scope (explicitly):**
- Any change to the SDK's **server-side** auth (`src/server/auth/*`) — that side already supports DCR; this phase is client-side only
- Daemon/background refresh — refresh is strictly on-demand
- Migration of the legacy `~/.pmcp/oauth-tokens.json` (users re-login once)
- Multi-identity per server (two different tokens for the same mcp_server_url)
- DCR client credential rotation — out of RFC 7591 scope
- Removing `--oauth-client-id` — kept as the escape hatch for enterprise IdPs with DCR disabled

</domain>

<decisions>
## Implementation Decisions

### SDK: Dynamic Client Registration (D-01..D-05)
- **D-01:** DCR support is a **general-purpose SDK feature** in `src/client/oauth.rs` (or new `src/client/dcr.rs`) — not a CLI-only wiring. The SDK owns the DCR types, request/response handling, and discovery integration. Library users (not just cargo-pmcp) can build clients that auto-register.
- **D-02:** `OAuthConfig` gains two new fields AND the existing `client_id` field changes type:
  ```rust
  pub struct OAuthConfig {
      // ... existing fields ...
      pub client_id: Option<String>,     // WAS: String (required). NOW: Option<String>.
                                          // None → SDK auto-performs DCR when eligible (D-03).
      pub client_name: Option<String>,   // NEW — RFC 7591 client_name for DCR (D-04)
      pub dcr_enabled: bool,             // NEW — default: true; fire DCR when eligible
  }
  ```
  **Breaking change:** `client_id: String → Option<String>` is a breaking struct field change. Per MEMORY.md "v2.0 cleanup philosophy — during breaking-change window, consolidate aggressively," this is accepted in the v2.x line. Migration for external callers: replace `client_id: "x".to_string()` with `client_id: Some("x".to_string())`. All in-repo callers get fixed in the same phase. Semver: **pmcp minor bump** (not major) with an explicit CHANGELOG migration note. See also D-22.
- **D-03:** **DCR fires automatically when all of the following are true:**
  1. `dcr_enabled == true`
  2. `client_id` is absent (i.e., not provided by caller)
  3. `/.well-known/oauth-authorization-server` returns a `registration_endpoint`

  If the server does NOT advertise a `registration_endpoint` AND no `client_id` is provided, the SDK returns an actionable error: `"server does not support DCR — pass a pre-registered client_id"`.

- **D-04:** `client_name` defaults to **`None`** at the SDK layer. Library users (including cargo-pmcp) are expected to set it to a meaningful value. If the SDK has to send a DCR request with no caller-provided name, it falls back to the literal string `"pmcp-sdk"`. cargo-pmcp explicitly sets `"cargo-pmcp"` when `--client` is absent, `<user-value>` when `--client` is passed.
- **D-05:** DCR request body shape (RFC 7591 compliant):
  ```json
  {
    "client_name": "<name>",
    "redirect_uris": ["http://localhost:<port>/callback"],
    "grant_types": ["authorization_code"],
    "token_endpoint_auth_method": "none"
  }
  ```
  Public PKCE client (no secret). Response is parsed for `client_id` (required) and `client_secret` (optional — present only for confidential clients, which the SDK does not request).

### CLI: Token Cache Schema & Location (D-06..D-07)
- **D-06:** Cache key = **normalized `mcp_server_url`**. Normalization = `scheme://host[:port]` only (strip path, strip trailing slash, lowercase host). Multi-OAuth-app-per-server is a rare edge deferred to a follow-up.
- **D-07:** Cache file = **`~/.pmcp/oauth-cache.json`** (new filename). Schema contains `schema_version: 1` + `entries: { "<normalized_url>": { access_token, refresh_token, expires_at, scopes, issuer, client_id } }`. Legacy `~/.pmcp/oauth-tokens.json` is **not read, not deleted, not migrated** — users re-login once.

### CLI: Command Surface (D-08..D-10)
- **D-08:** Ship all 5 subcommands: `login`, `logout`, `status`, `token`, `refresh`.
- **D-09:** `cargo pmcp auth logout` **with no args errors out** (`error: specify a server URL or --all to log out of everything`). Accepted forms: `auth logout <url>` or `auth logout --all`.
- **D-10:** Subcommand behaviors:
  - `login <url> [--client <name>] [--oauth-client-id <id>] [--oauth-issuer <url>] [--oauth-scopes <list>] [--oauth-redirect-port <port>]` — PKCE (+ auto-DCR when eligible per D-03), cache result
  - `logout <url | --all>` — remove entry/entries
  - `status [<url>]` — tabular output (server | issuer | scopes | expires_in | has_refresh_token). No args = all cached servers
  - `token <url>` — raw access token to stdout (refreshes silently if expired/near-expiry)
  - `refresh <url>` — force-refresh using cached refresh_token; error if no refresh_token available

### CLI: Output & DX (D-11..D-12)
- **D-11:** `auth token <url>` prints **raw access token only** to stdout (+ trailing newline). All status/error messages go to stderr. Matches `gh auth token` ergonomics. Enables `TOKEN=$(cargo pmcp auth token URL)` and `curl -H "Authorization: Bearer $(cargo pmcp auth token URL)"`.
- **D-12:** `auth login` success output = `Logged in to <url> (issuer: <issuer>, scopes: <scopes>, expires in <duration>)`. **Access token is never printed** — safer for shared terminals / shell history. Users who need the token call `auth token <url>` explicitly.

### CLI: Auth Resolution Precedence (D-13..D-14)
- **D-13:** Precedence order for every server-connecting command = **explicit flag > env var > cache**. `--api-key` / `--oauth-client-id` win over `MCP_API_KEY` / `MCP_OAUTH_CLIENT_ID` env, which win over the cache. Matches current "explicit overrides implicit" behavior — additive, no CI breakage.
- **D-14:** **Silent fallback** when both a cached token and an explicit flag exist for the same URL. No warning printed. `auth status <url>` is the explicit inspection tool.

### CLI: Refresh Behavior (D-15..D-16)
- **D-15:** **On-demand refresh only.** At use time (inside `resolve_auth_header` / `resolve_auth_middleware` and inside `auth token <url>`): if the cached entry is expired OR within **60 seconds** of expiry, transparently use the cached `refresh_token` to obtain a new access token, update the cache, and proceed. If refresh fails → propagate error with actionable message: `cargo pmcp auth login <url>` to re-authenticate.
- **D-16:** `auth refresh <url>` is the **explicit force-refresh escape hatch** — rotates the access token now, ignoring expiry timing. Errors if no refresh_token is cached for that URL.

### CLI: `--client` flag → SDK DCR (D-17..D-19)
- **D-17:** **`--client <name>` flag on `auth login` ONLY** (not `refresh`, `token`, `status`, `logout`). Rationale: `--client` only affects the interactive login UI. Once the token is cached, subsequent calls don't need to re-identify.
- **D-18:** `--client` is **transient** (not persisted to the cache entry). It's a login-time-only flag that sets `OAuthConfig::client_name` for the DCR request.
- **D-19:** `--client` and `--oauth-client-id` are **mutually exclusive** at parse time (clap `conflicts_with`). Rationale: `--oauth-client-id` means "skip DCR, use this pre-registered id"; `--client` means "do DCR with this name to get a fresh id". Passing both is a contradiction.

### CLI: Escape Hatch & Enterprise Compatibility (D-20)
- **D-20:** `--oauth-client-id` / env `MCP_OAUTH_CLIENT_ID` is **kept as the escape hatch**. When provided, DCR is **skipped entirely** — the SDK uses the pre-registered client_id. Rationale: enterprise IdPs often disable DCR; removing `--oauth-client-id` would break those users. Additive, zero breakage.

### Scope & Release (D-21..D-23)
- **D-21:** Include `cargo-pmcp/src/commands/pentest.rs` migration from its duplicate `--api-key` flag (line 62, env `MCP_API_KEY`) to the shared `AuthFlags` struct. Uniforms the flag surface.
- **D-22:** Semver bumps:
  - **`pmcp` crate: current → current+1 minor** (e.g., `2.3.0 → 2.4.0`). New public DCR API is additive; no breaking change.
  - **`cargo-pmcp 0.8.1 → 0.9.0`** minor. New top-level command group + new cache format.
- **D-23:** Follow CLAUDE.md release workflow. Bump order: `pmcp` first (no deps in this phase), then `cargo-pmcp` (update its `pmcp = { version = "..." }` pin to match the new minor). `mcp-tester` / `mcp-preview` / `pmcp-macros` are **not** bumped — they gain no new behavior. Tag one release covering both bumped crates per the release workflow.

### Claude's Discretion
- Concrete struct/enum shapes for `TokenCacheV1`, the DCR request/response types, and the per-entry record
- File-locking strategy for concurrent `auth login` invocations (temp-file-plus-atomic-rename recommended in Specific Ideas)
- Whether `status` tabular output uses existing `colored` crate formatting or a fresh `tabled`/`comfy-table` dep — pick based on what's already in `cargo-pmcp/Cargo.toml`
- How to test DCR without a live pmcp.run — mock HTTP server in integration tests using whichever mocking crate the SDK already depends on (check `Cargo.toml` first)
- Error message copy for the various failure modes (DCR not supported, expired refresh, cache corrupt, etc.)
- Whether DCR lives in `src/client/oauth.rs` directly or gets its own `src/client/dcr.rs` module (research/plan decides based on file size limits)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### SDK-side OAuth code (source of truth for D-01..D-05)
- `src/client/oauth.rs` — existing `OAuthHelper`, `OAuthConfig`, single-server `TokenCache` (line 58), `default_cache_path()` (line 663). DCR lives here (or adjacent module).
- `src/server/auth/oauth2.rs` — **read for pattern reference**: server-side `register_client` trait method (line 320) and DCR types. Client-side must produce requests compatible with this server-side parser.
- `src/server/auth/providers/generic_oidc.rs` line 642 — reference implementation of server-side DCR against an external IdP. Shows the wire format.

### CLI-side auth code (source of truth for D-06..D-23)
- `cargo-pmcp/src/commands/auth.rs` — shared `resolve_auth_middleware` (line 52) and `resolve_auth_header` (line 101). Cache-consulting logic lands here.
- `cargo-pmcp/src/commands/flags.rs` — `AuthFlags` struct (line 108), `AuthMethod` enum (line 79), `resolve()` (line 140). New cache-fallback is added one layer up, not inside `AuthFlags::resolve()`.
- `cargo-pmcp/src/commands/pentest.rs` line 62 — duplicate `--api-key` flag to migrate per D-21.
- `cargo-pmcp/src/main.rs` line 70 — top-level `Commands` enum; add new `Auth { command: AuthCommand }` variant here.
- `cargo-pmcp/src/commands/test/conformance.rs` line 43 — reference example of how a command consumes `AuthFlags` + `resolve_auth_middleware`.

### pmcp.run oauth-proxy wire behavior (authoritative for DCR request shape)
- `/Users/guy/Development/mcp/sdk/pmcp-run/control-plane/oauth-proxy/src/main.rs` line 2209 — `handle_client_registration` — DCR endpoint; parses `client_name` from the request body.
- Same file, line 2853 — `classify_client_type` — substring-matches lowercase `client_name` against `ClientTypeMatcher::name_patterns`. Shows how `--client "claude-desktop"` / `"chatgpt"` / `"cursor"` values get routed to the branded shared Cognito client.
- Same file, line 707 — `ClientRegistrationRequest` struct — authoritative DCR request shape.
- Same file, line 721 — `ClientRegistrationResponse` struct — DCR response shape the SDK must parse.
- Same file, line 614 — `ClientTypeMatcher` struct — shows the `name_patterns`/`display_name`/`branding_style_id` fields.

### Project standards
- `./CLAUDE.md` — Toyota Way, `make quality-gate`, ALWAYS testing requirements (fuzz/property/unit/example), release workflow at bottom.
- `.planning/ROADMAP.md` — Phase 74 goal/dependency chain (swapped ahead of Phase 73 on 2026-04-21).
- `.planning/STATE.md` — frontmatter + Roadmap Evolution note for Phase 74.

### Standards / RFCs
- **RFC 7591** — OAuth 2.0 Dynamic Client Registration Protocol (D-01..D-05 wire format)
- **RFC 8414** — OAuth 2.0 Authorization Server Metadata (`/.well-known/oauth-authorization-server` discovery; D-03 trigger condition)
- **RFC 7636** — PKCE (already implemented; unchanged)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`OAuthHelper` and `OAuthConfig`** (`src/client/oauth.rs`): full PKCE flow including browser redirect, local callback listener, token caching, auto-refresh. Phase 74 adds DCR upstream of PKCE and generalizes the cache layer underneath.
- **`default_cache_path()`** (`src/client/oauth.rs:663`): returns `~/.pmcp/oauth-tokens.json`. Add a sibling `default_cache_v2_path()` returning `~/.pmcp/oauth-cache.json`.
- **`HttpMiddlewareChain` + `BearerToken` + `OAuthClientMiddleware`** (`pmcp::client::http_middleware` / `pmcp::client::oauth_middleware`): already wire up Authorization headers. No change to middleware needed.
- **`AuthFlags::resolve()`** (`cargo-pmcp/src/commands/flags.rs:140`): converts CLI flags into `AuthMethod` enum. The cache fallback is added in `resolve_auth_middleware`/`resolve_auth_header`, not inside `AuthFlags::resolve()`.
- **Server-side `register_client` in `src/server/auth/oauth2.rs`**: existing DCR implementation on the server side. Client-side DCR must produce requests compatible with this.

### Established Patterns
- **Shared `AuthFlags` via `#[command(flatten)]`** — every server-connecting subcommand opts in. New cache fallback is transparent.
- **Mutually-exclusive clap flags via `conflicts_with`** — already used for `--api-key` vs `--oauth-client-id`. Reused for `--client` vs `--oauth-client-id` (D-19).
- **Top-level `Commands` enum additions** — `New`, `Add`, `Test`, `Dev`, `Connect`, `Deploy`, `Landing`, `Schema`, `Validate` are the pattern (`cargo-pmcp/src/main.rs:69`). `Auth { command: AuthCommand }` follows the same shape as `Test { command: TestCommand }`.
- **Subcommand module layout** — `commands/test/{mod.rs, conformance.rs, ...}` is the pattern. New module: `commands/auth_cmd/{mod.rs, login.rs, logout.rs, status.rs, token.rs, refresh.rs}` (name-disambiguated from existing `commands/auth.rs` which holds `resolve_auth_middleware`). Alternative: rename existing `commands/auth.rs` → `commands/auth_resolve.rs` and use `commands/auth/` for the new group.

### Integration Points
- **`src/client/oauth.rs`** — add DCR types, DCR request builder, DCR execution in `OAuthHelper::authorize()` path before PKCE.
- **`cargo-pmcp/src/main.rs`** — new `Commands::Auth { command: AuthCommand }` variant + match arm dispatching to `commands::auth_cmd::*::execute(...)`.
- **`cargo-pmcp/src/commands/mod.rs`** — new `pub mod auth_cmd;` (or rename existing per above).
- **`cargo-pmcp/src/commands/auth.rs::resolve_auth_middleware`** (line 52) — new behavior: on `AuthMethod::None`, check the multi-server cache keyed by normalized URL; return a middleware chain with cached token if present (with auto-refresh per D-15).

</code_context>

<specifics>
## Specific Ideas

- **`status` output format** — tabular, roughly: `URL | ISSUER | SCOPES | EXPIRES | REFRESHABLE`. Use the existing `colored` crate for the header row. No new table crates unless already a dep.
- **DCR example call** — for `cargo pmcp auth login https://mcp.pmcp.run --client claude-desktop`, the DCR POST body:
  ```json
  {
    "client_name": "claude-desktop",
    "redirect_uris": ["http://localhost:8080/callback"],
    "grant_types": ["authorization_code"],
    "token_endpoint_auth_method": "none"
  }
  ```
  Expected response: `{"client_id": "<shared-cognito-id>", "client_id_issued_at": <ts>, ...}` — no `client_secret` for public PKCE clients.
- **Cache file locking** — concurrent `auth login` from two terminals is rare but possible. Use temp-file-plus-atomic-rename (`tempfile::NamedTempFile::persist`) for writes — the dir is user-only, no concurrent readers can see half-written state. Lock-free.
- **SDK-side DCR example** — a new `examples/` entry demonstrating DCR from a library user's perspective (not just CLI): a minimal MCP client that uses `OAuthConfig { client_name: Some("my-app"), dcr_enabled: true, client_id: None, ... }` and connects to a server advertising `registration_endpoint`. Satisfies the CLAUDE.md "EXAMPLE Demonstration (ALWAYS REQUIRED)" gate.

</specifics>

<deferred>
## Deferred Ideas

- **Multiple OAuth apps per server** (composite `(url, client_id)` key) — rare edge case; bump cache `schema_version` to 2 when needed.
- **`auth servers` alias for `auth status` no-args** — cosmetic discoverability improvement; add in a follow-up if users ask.
- **`--verbose` mode on server-connecting commands** to print the precedence decision — useful for debugging but not critical.
- **`--client` on `auth refresh`** — if pmcp.run adds refresh-time branding.
- **Clipboard copy output for `auth token`** — `--copy` flag that pipes to `pbcopy`/`xclip`/`clip`. Nice-to-have.
- **Interactive TUI for `auth status`** — real-time expiry countdown, force-refresh button. Out of scope.
- **Encrypted cache at rest** — the file is `chmod 600` in `~/.pmcp/`, matching `gh`, `aws`, `gcloud` conventions. Keyring integration is a future hardening phase.
- **DCR client credential rotation** — out of RFC 7591 scope.
- **Confidential client support in DCR** (request a `client_secret`) — SDK only requests public PKCE clients for now. Confidential clients are a future SDK extension.
- **Removing `--oauth-client-id`** — kept as enterprise escape hatch (D-20). Could be reconsidered at a future 1.0 milestone with a deprecation window.

</deferred>

---

*Phase: 74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token*
*Context gathered: 2026-04-21 (revised to move DCR into SDK per operator feedback)*
