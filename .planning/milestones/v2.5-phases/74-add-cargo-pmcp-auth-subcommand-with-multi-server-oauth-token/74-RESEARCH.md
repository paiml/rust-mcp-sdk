# Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management - Research

**Researched:** 2026-04-21
**Domain:** OAuth 2.0 client-side Dynamic Client Registration (RFC 7591) in Rust SDK + multi-server CLI token cache
**Confidence:** HIGH

## Summary

Phase 74 is an **additive** feature phase against a codebase that already contains 90% of the needed primitives. All the load-bearing dependencies (`reqwest`, `base64`, `sha2`, `tempfile`, `dirs`, `serde`, `colored`, `mockito`) are already present — the research confirms **no new crate dependencies are required** for either the SDK or CLI deliverable. The DCR request/response types (`DcrRequest`, `DcrResponse`) already exist in `src/server/auth/provider.rs` and **can be reused verbatim on the client side** — they are RFC 7591 compliant and wire-compatible with pmcp.run's oauth-proxy (`ClientRegistrationRequest` / `FullDcrResponse` at `/Users/guy/Development/mcp/sdk/pmcp-run/control-plane/oauth-proxy/src/main.rs:707,720`). The `OidcDiscoveryMetadata` struct already exposes the `registration_endpoint: Option<String>` field needed for the D-03 auto-trigger.

The biggest SDK subtlety: the current `OAuthConfig::client_id: String` is **not optional** — to honor D-02/D-03 (DCR fires when `client_id.is_none()`), the field must change shape. CONTEXT.md locks this as an **additive** change, so the recommended migration is to keep `client_id: String` but treat empty-string as "no client_id" (ergonomically ugly) OR bump to `client_id: Option<String>` (technically breaking at the struct-literal level, but `pmcp` is pre-1.0 at v2.x, so a minor bump that requires `Option::Some` wrapping at existing call sites is acceptable per the "Version Bump Rules" in CLAUDE.md — all existing `OAuthConfig { client_id: "...".to_string(), ... }` call sites in the repo are ours to update). **Option B is cleaner and the planner should adopt it.** See Open Questions §1 for confirmation.

The biggest CLI subtlety: `commands/auth.rs` already exists (holds `resolve_auth_middleware`). Adding a `commands::auth_cmd` subcommand module is the lowest-friction path; `commands/auth.rs` stays put.

**Primary recommendation:** Use all existing dependencies and types. Reuse `DcrRequest` / `DcrResponse` from `src/server/auth/provider.rs` by re-exporting through `pmcp::client::oauth`. Roll a new `TokenCacheV1` struct in `cargo-pmcp/src/commands/auth_cmd/cache.rs` (new module — do NOT modify the SDK-internal single-server `TokenCache` in `src/client/oauth.rs`). Use `tempfile::NamedTempFile::persist()` for atomic cache writes. Use `colored` crate for `status` table output (no new table deps).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| RFC 7591 DCR types (request/response) | SDK library | — | Already live in `src/server/auth/provider.rs`; re-export path keeps single source of truth |
| DCR HTTP execution (POST to registration_endpoint) | SDK library (`pmcp::client::oauth`) | — | Client-side flow, must live with `OAuthHelper` PKCE logic |
| Discovery metadata parsing (extract `registration_endpoint`) | SDK library | — | Already done by `OidcDiscoveryClient::discover()` |
| PKCE browser flow | SDK library | — | Already implemented in `OAuthHelper::authorization_code_flow()` |
| Token cache file I/O (single-server) | SDK library (legacy `TokenCache` @ line 58) | — | Existing — unchanged, still consumed by `OAuthHelper` for library users |
| Token cache file I/O (multi-server keyed) | CLI (`cargo-pmcp`) | — | CLI-specific feature; no SDK user has asked for multi-server. Keep out of SDK surface. |
| `cargo pmcp auth` subcommand plumbing | CLI (`cargo-pmcp`) | — | User-facing CLI ergonomics |
| URL normalization (cache key) | CLI (`cargo-pmcp`) | SDK helper if generalizable | Simple enough to live in CLI; revisit if a second caller appears |
| Auth resolution precedence (flag > env > cache) | CLI (`cargo-pmcp`) | — | Orchestration of existing `AuthFlags` + new cache layer; CLI responsibility |
| On-demand refresh (60s before expiry) | CLI (`cargo-pmcp`) calls SDK `refresh_token` | SDK | CLI decides WHEN to refresh; SDK already has the HTTP-level refresh function |
| pentest `--api-key` flag migration | CLI (`cargo-pmcp`) | — | Flag surface consolidation; CLI-only |

## User Constraints (from CONTEXT.md)

### Locked Decisions

**SDK Dynamic Client Registration (D-01..D-05):**

- **D-01:** DCR is a **general-purpose SDK feature** in `src/client/oauth.rs` (or new `src/client/dcr.rs`). Library users — not just cargo-pmcp — can build auto-registering clients.
- **D-02:** `OAuthConfig` gains two additive fields:
  ```rust
  pub client_name: Option<String>,   // RFC 7591 client_name for DCR
  pub dcr_enabled: bool,             // default: true
  ```
- **D-03:** DCR fires automatically when all three are true: (1) `dcr_enabled == true`, (2) `client_id` is absent, (3) discovery returns a `registration_endpoint`. If the server does NOT advertise a registration_endpoint AND no client_id is provided, SDK returns actionable error: `"server does not support DCR — pass a pre-registered client_id"`.
- **D-04:** `client_name` defaults to `None` at SDK layer. If DCR has to run with no caller-provided name, fall back to literal `"pmcp-sdk"`. cargo-pmcp sets `"cargo-pmcp"` (default) or `<user-value>` (from `--client`).
- **D-05:** DCR request body shape (public PKCE client, no secret):
  ```json
  {
    "client_name": "<name>",
    "redirect_uris": ["http://localhost:<port>/callback"],
    "grant_types": ["authorization_code"],
    "token_endpoint_auth_method": "none"
  }
  ```

**CLI Token Cache (D-06..D-07):**

- **D-06:** Cache key = normalized `mcp_server_url`. Normalization = `scheme://host[:port]` (strip path, strip trailing slash, lowercase host).
- **D-07:** Cache file = `~/.pmcp/oauth-cache.json`. Schema: `schema_version: 1` + `entries: { "<normalized_url>": { access_token, refresh_token, expires_at, scopes, issuer, client_id } }`. Legacy `~/.pmcp/oauth-tokens.json` left untouched — users re-login once.

**CLI Command Surface (D-08..D-10):**

- **D-08:** Ship all 5 subcommands: `login`, `logout`, `status`, `token`, `refresh`.
- **D-09:** `auth logout` with no args errors: `"specify a server URL or --all to log out of everything"`.
- **D-10:** Subcommand behaviors per CONTEXT.md line 82-88.

**CLI Output & DX (D-11..D-12):**

- **D-11:** `auth token <url>` → raw access token to stdout + newline; all status/error to stderr. Matches `gh auth token`.
- **D-12:** `auth login` success: `"Logged in to <url> (issuer: <issuer>, scopes: <scopes>, expires in <duration>)"`. Token is NEVER printed.

**CLI Auth Precedence (D-13..D-14):**

- **D-13:** `explicit flag > env var > cache`. Additive — no CI breakage.
- **D-14:** Silent fallback (no warning) when both a cached token and explicit flag exist for the same URL. `auth status <url>` is the explicit inspection tool.

**CLI Refresh (D-15..D-16):**

- **D-15:** On-demand refresh only, inside `resolve_auth_*` and `auth token <url>`. Expired OR within **60 seconds of expiry** triggers transparent refresh. Refresh failure → actionable error: `cargo pmcp auth login <url>` to re-authenticate.
- **D-16:** `auth refresh <url>` = explicit force-refresh. Errors if no refresh_token cached.

**CLI --client flag (D-17..D-19):**

- **D-17:** `--client <name>` is **on `auth login` ONLY**. Not on refresh/token/status/logout.
- **D-18:** `--client` is **transient** — not persisted to cache entry.
- **D-19:** `--client` and `--oauth-client-id` are **mutually exclusive** (clap `conflicts_with`).

**CLI Escape Hatch (D-20):**

- **D-20:** `--oauth-client-id` / `MCP_OAUTH_CLIENT_ID` stays as enterprise escape hatch. When provided, DCR is **skipped entirely**.

**Scope & Release (D-21..D-23):**

- **D-21:** Migrate `cargo-pmcp/src/commands/pentest.rs:62` from its duplicate `--api-key` flag to shared `AuthFlags`.
- **D-22:** Semver: `pmcp` minor (e.g., `2.4.0 → 2.5.0`); `cargo-pmcp 0.8.1 → 0.9.0` minor.
- **D-23:** Release order: `pmcp` first, then `cargo-pmcp` (update its `pmcp = { version = "..." }` pin). `mcp-tester` / `mcp-preview` / `pmcp-macros` are **not** bumped.

### Claude's Discretion

- Concrete struct/enum shapes for `TokenCacheV1`, DCR types, per-entry record
- File-locking strategy for concurrent `auth login` (tempfile-plus-atomic-rename recommended in Specific Ideas)
- `status` tabular output: existing `colored` vs fresh table crate (pick based on what's in Cargo.toml)
- How to test DCR without live pmcp.run — mock HTTP server using SDK's existing mock crate
- Error message copy for failure modes
- Whether DCR lives in `src/client/oauth.rs` directly or gets its own `src/client/dcr.rs` module

### Deferred Ideas (OUT OF SCOPE)

- Multiple OAuth apps per server (composite `(url, client_id)` key — bump cache schema_version to 2 when needed)
- `auth servers` alias for `auth status` no-args
- `--verbose` mode to print precedence decision
- `--client` on `auth refresh`
- Clipboard copy output for `auth token` (`--copy` flag)
- Interactive TUI for `auth status`
- Encrypted cache at rest (keyring integration) — future hardening phase
- DCR client credential rotation
- Confidential client support in DCR (request a `client_secret`)
- Removing `--oauth-client-id`

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SDK-DCR-01 | Public RFC 7591 DCR support in `src/client/oauth.rs`. `OAuthConfig` gains `client_name: Option<String>` + `dcr_enabled: bool`. `OAuthHelper` auto-performs DCR when `dcr_enabled && client_id.is_none() && discovery.registration_endpoint.is_some()`. Fuzz + property + unit + example per CLAUDE.md ALWAYS. | Existing `DcrRequest`/`DcrResponse` in `src/server/auth/provider.rs:304,355` are RFC 7591 compliant and reusable. Existing `OidcDiscoveryMetadata::registration_endpoint` at `src/server/auth/oauth2.rs:192`. Existing `reqwest` usage in `OAuthHelper` makes DCR `POST + deserialize` a ~30 LOC addition. `mockito` already in dev-deps for integration tests. |
| CLI-AUTH-01 | New `cargo pmcp auth` command group (login/logout/status/token/refresh). Per-server cache at `~/.pmcp/oauth-cache.json` with `schema_version: 1`. `login --client <name>` sets `OAuthConfig::client_name`. `logout` errors with no args. `token <url>` → raw stdout. Transparent refresh at 60s-before-expiry. Migrate `pentest.rs` to shared `AuthFlags`. | Existing `AuthFlags` + `resolve_auth_middleware` + `resolve_auth_header` are the wrapping seams. `tempfile` already in cargo-pmcp dev-deps (promote to regular deps). `colored` already a dep. `dirs` already a dep for home directory. `reqwest` already a dep for refresh HTTP call. Existing `Commands::Test { command: TestCommand }` in `main.rs:104` is the shape to copy for `Auth { command: AuthCommand }`. |

## Standard Stack

### Core (already in Cargo.toml — NO new deps needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | 0.13 | HTTP client for DCR POST + token refresh | Already used by `OAuthHelper`; SDK's standard HTTP client `[VERIFIED: Cargo.toml:117]` |
| `serde` | 1.0 | JSON (de)serialization for DCR wire + cache file | Already used for every wire type `[VERIFIED: Cargo.toml:50]` |
| `serde_json` | 1.0 | JSON string I/O | Already used `[VERIFIED: Cargo.toml:51]` |
| `tempfile` | 3.19 | Atomic cache writes via `NamedTempFile::persist` | Already in pmcp dev-deps (cargo-pmcp: also dev-dep, promote) `[VERIFIED: Cargo.toml:130, cargo-pmcp/Cargo.toml:76]` |
| `dirs` | 6 | `home_dir()` for `~/.pmcp/oauth-cache.json` | Already used by `default_cache_path()` `[VERIFIED: Cargo.toml:87, cargo-pmcp/Cargo.toml:31]` |
| `colored` | 3 | `status` table formatting (header bold, expiry colored) | Already in cargo-pmcp `[VERIFIED: cargo-pmcp/Cargo.toml:29]` |
| `clap` | 4 (features=derive, env) | Subcommand + flag parsing | Standard already; `conflicts_with` for D-19 `[VERIFIED: cargo-pmcp/Cargo.toml:22]` |
| `url` | 2.5 | Normalization (scheme/host/port extraction) | Already used by `OAuthHelper` `[VERIFIED: Cargo.toml:62]` |
| `chrono` | 0.4 | Expiry timestamp + duration formatting | Already a dep of both crates `[VERIFIED: Cargo.toml:65, cargo-pmcp/Cargo.toml:51]` |

### Dev-dependencies (already present)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `mockito` | 1.5.0 | Mock HTTP server for DCR integration tests | Already in pmcp dev-deps (unused so far — first user is Phase 74) `[VERIFIED: Cargo.toml:140]` |
| `proptest` | 1.7 (pmcp) / 1 (cargo-pmcp) | Property tests (URL normalization round-trips, cache serde round-trips) | Already in both crates `[VERIFIED: Cargo.toml:131, cargo-pmcp/Cargo.toml:77]` |
| `tempfile` | 3.19 / 3 | Scratch dirs in integration tests | Already `[VERIFIED]` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Re-export `DcrRequest`/`DcrResponse` from server module | Define NEW client-side types | Rejected — duplicating types risks divergence; `src/server/auth/provider.rs` DCR types are already RFC 7591-complete and match pmcp.run's wire. `[CITED: src/server/auth/provider.rs:304-382]` |
| `mockito` for DCR tests | `wiremock` | Rejected — `mockito` already in dev-deps. Adding `wiremock` violates "no new OAuth deps" principle. |
| `colored` + println! for `status` table | `comfy-table` / `tabled` | Rejected per CONTEXT.md Claude's Discretion — add no new table crates if `colored` suffices. 5-column table is trivial with `format!` + column width calculation. |
| `tempfile::NamedTempFile::persist` for atomic write | `fs2`-based advisory locking | Rejected per CONTEXT.md Specific Ideas — temp-file-plus-rename is lock-free, cross-platform atomic on modern Linux + Windows. |
| `oauth2` crate for DCR | Handrolled POST via `reqwest` | Rejected — the `oauth2 = "5.0"` crate in `cargo-pmcp/Cargo.toml:47` does NOT support DCR (it targets auth-code flow only; DCR support is a separate `oauth2-dcr` crate not in our tree). Existing `OAuthHelper` is handrolled reqwest; DCR follows the same pattern. `[VERIFIED: cargo-pmcp/Cargo.toml:47; crates.io/oauth2 - DCR not in public API]` |

**Installation:** None required. All dependencies already present.

**Version verification (2026-04-21):**

```bash
$ cargo search pmcp --limit 3
pmcp = "2.4.0"           # [VERIFIED: crates.io 2026-04-21]
cargo-pmcp = "0.8.0"     # Note: local source is 0.8.1 (unpublished patch)
pmcp-macros = "0.6.0"
```

Target bumps per D-22: `pmcp 2.4.0 → 2.5.0`, `cargo-pmcp 0.8.1 → 0.9.0`.

## Architecture Patterns

### System Architecture Diagram

```
┌────────────────────┐        ┌──────────────────────────┐
│  User: cargo pmcp  │        │  User: any SDK-built     │
│  auth login <url>  │        │  OAuth client            │
│  --client claude   │        │  (OAuthConfig{..})       │
└──────────┬─────────┘        └──────────┬───────────────┘
           │                             │
           ▼                             ▼
┌────────────────────────────────────────────────────────┐
│  cargo-pmcp: commands/auth_cmd/{login,logout,...}.rs   │
│    • parses --client --oauth-client-id etc             │
│    • normalizes server URL for cache key               │
└──────────┬─────────────────────────────────────────────┘
           │  constructs OAuthConfig { client_id: None,
           │                            client_name: Some("claude-desktop"),
           │                            dcr_enabled: true, ... }
           ▼
┌────────────────────────────────────────────────────────┐
│  pmcp::client::oauth::OAuthHelper                      │
│   1. discover_metadata(server_url)                     │  ◀── GET /.well-known/openid-configuration
│      → OidcDiscoveryMetadata (reg_endpoint?)           │      OR /.well-known/oauth-authorization-server
│   2. IF client_id.is_none() && dcr_enabled             │
│      && metadata.registration_endpoint.is_some():      │
│      ──▶ do_dcr(registration_endpoint, client_name)    │  ◀── POST registration_endpoint
│            → DcrResponse { client_id, ... }            │      body: RFC 7591 shape
│      ELSE IF client_id.is_none():                      │
│      ──▶ error "server does not support DCR …"        │
│   3. authorization_code_flow(client_id, metadata)      │  ◀── browser → localhost callback
│      → access_token                                    │      → POST token_endpoint
└──────────┬─────────────────────────────────────────────┘
           │  returns access_token
           ▼
┌────────────────────────────────────────────────────────┐
│  cargo-pmcp: commands/auth_cmd/cache.rs                │
│    • read_cache() → TokenCacheV1                        │  ◀── ~/.pmcp/oauth-cache.json
│    • entry = Entry { access_token, refresh_token,      │
│                       expires_at, scopes, issuer,      │
│                       client_id }                       │
│    • write_cache_atomic() via NamedTempFile::persist   │  ──▶ tmp → rename → cache.json
└────────────────────────────────────────────────────────┘

Consumer flow (test/conformance, loadtest, preview, etc.):
┌────────────────────────────────────────────────────────┐
│  AuthFlags::resolve() → AuthMethod enum                │
└──────────┬─────────────────────────────────────────────┘
           ▼
┌────────────────────────────────────────────────────────┐
│  commands/auth.rs::resolve_auth_middleware (MODIFIED)  │
│    match method:                                       │
│      ApiKey(k) → existing                              │
│      OAuth(…)  → existing                              │
│      None      → NEW: look up cache[normalized_url]    │
│                     if hit: return cached bearer       │
│                     if near-expiry: refresh silently   │
│                     else: return None (unchanged)      │
└────────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
src/client/
├── oauth.rs                 # existing — add DCR inline (recommended, < 1000 LOC added)
│                            # OR split: add src/client/dcr.rs if oauth.rs exceeds 1500 LOC
│                            # Current oauth.rs is 693 LOC → inline is fine
├── auth.rs                  # existing — OidcDiscoveryClient (unchanged)
└── oauth_middleware.rs      # existing — BearerToken / OAuthClientMiddleware (unchanged)

examples/
└── c08_oauth_dcr.rs         # NEW — SDK-side DCR demo (satisfies ALWAYS requirement)

cargo-pmcp/src/commands/
├── auth.rs                  # existing — resolve_auth_middleware / resolve_auth_header
│                            # MODIFIED: add cache-fallback on AuthMethod::None branch
├── auth_cmd/                # NEW subcommand module (name-disambiguated from auth.rs)
│   ├── mod.rs               # AuthCommand enum + AuthCommand::execute() dispatcher
│   ├── login.rs
│   ├── logout.rs
│   ├── status.rs
│   ├── token.rs
│   ├── refresh.rs
│   ├── cache.rs             # TokenCacheV1, Entry, atomic read/write, URL normalization
│   └── errors.rs            # shared error messages (D-09, D-15, D-16)
├── flags.rs                 # existing — AuthFlags unchanged
├── pentest.rs               # MODIFIED per D-21 — drop local --api-key, #[command(flatten)] AuthFlags
└── mod.rs                   # add: pub mod auth_cmd;

cargo-pmcp/tests/
└── auth_integration.rs      # NEW — end-to-end against mockito DCR + token mock
```

### Pattern 1: DCR Type Reuse

**What:** Re-export existing `DcrRequest` / `DcrResponse` from `src/server/auth/provider.rs` via `pmcp::client::oauth`.
**When to use:** Always — single source of truth, no type drift risk.
**Example:**
```rust
// src/client/oauth.rs
// Re-export at the client module for ergonomic `pmcp::client::oauth::DcrRequest` access.
pub use crate::server::auth::provider::{DcrRequest, DcrResponse};
```
Source: reuse of `[VERIFIED: src/server/auth/provider.rs:302-382]` which already has the full RFC 7591 shape including optional fields (client_uri, logo_uri, contacts, software_id, extra via `#[serde(flatten)]`).

### Pattern 2: Atomic Cache Write via tempfile

**What:** Write token cache through a sibling tempfile, then atomic rename.
**When to use:** Every cache mutation (`login` success, `refresh` success, `logout`).
**Example:**
```rust
use tempfile::NamedTempFile;
use std::io::Write;

// Source: tempfile 3.x docs — NamedTempFile::persist atomically replaces on
// Windows + modern Linux filesystems. Cannot cross filesystem boundaries; we
// place the tempfile in the same directory (~/.pmcp/) to guarantee same-fs.
// [CITED: https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist]
fn write_cache_atomic(path: &Path, cache: &TokenCacheV1) -> Result<()> {
    let parent = path.parent().ok_or(…)?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = NamedTempFile::new_in(parent)?;  // same-fs guarantee
    let json = serde_json::to_vec_pretty(cache)?;
    tmp.write_all(&json)?;
    tmp.flush()?;
    // chmod 600 on Unix before persisting (covers one-write window)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file().set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    tmp.persist(path)?;  // atomic replace
    Ok(())
}
```
**Why lock-free is safe:** The `~/.pmcp/` directory is user-private (typical `0o700` on Unix). No concurrent reader can observe a half-written file because `rename()` is atomic at the VFS level. Two concurrent `auth login` invocations will race — the loser's write is simply overwritten, but neither writes corrupt JSON (last-writer-wins is acceptable for this rare scenario).

### Pattern 3: URL Normalization (D-06)

**What:** Cache key = `scheme://host[:port]` (lowercase host, strip path, strip trailing slash, strip default ports).
**Example:**
```rust
// Source: reuse pattern from src/client/oauth.rs:113-128 (OAuthHelper::extract_base_url)
fn normalize_cache_key(mcp_server_url: &str) -> Result<String> {
    let parsed = Url::parse(mcp_server_url)?;
    let host = parsed.host_str().ok_or(…)?.to_ascii_lowercase();
    let mut base = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        let is_default = (parsed.scheme() == "https" && port == 443)
            || (parsed.scheme() == "http"  && port == 80);
        if !is_default {
            base.push_str(&format!(":{}", port));
        }
    }
    Ok(base)  // e.g., "https://mcp.pmcp.run"
}
```
This is a direct mirror of the existing `OAuthHelper::extract_base_url` — consider promoting to a `pub fn` in `pmcp::client::oauth` so both CLI and SDK share one implementation. `[CITED: src/client/oauth.rs:113-128]`

### Pattern 4: Subcommand Dispatcher

**What:** `Commands::Auth { command: AuthCommand }` mirrors existing `Commands::Test { command: TestCommand }` at `cargo-pmcp/src/main.rs:104`.
**Example:**
```rust
// main.rs
#[derive(Subcommand)]
enum Commands {
    // … existing variants …
    /// Manage OAuth credentials for MCP servers
    Auth {
        #[command(subcommand)]
        command: commands::auth_cmd::AuthCommand,
    },
}

// commands/auth_cmd/mod.rs
#[derive(Subcommand)]
pub enum AuthCommand {
    /// Log in to an OAuth-protected MCP server
    Login(login::LoginArgs),
    /// Remove cached credentials for a server (or all servers)
    Logout(logout::LogoutArgs),
    /// Show cached credential status
    Status(status::StatusArgs),
    /// Print the cached access token to stdout
    Token(token::TokenArgs),
    /// Force-refresh the cached access token
    Refresh(refresh::RefreshArgs),
}
```
`[CITED: cargo-pmcp/src/main.rs:104]` for the pattern; `[CITED: cargo-pmcp/src/commands/test/mod.rs]` for the module layout.

### Anti-Patterns to Avoid

- **Do NOT add a new `oauth2-dcr` / `oauth2_dcr` dependency.** The `oauth2 = "5.0"` crate already in `cargo-pmcp/Cargo.toml:47` does not expose DCR; DCR is a separate ecosystem crate. Handrolled `reqwest` POST is ~30 LOC and matches the existing `OAuthHelper` style.
- **Do NOT define new client-side DCR types.** Reuse `crate::server::auth::provider::{DcrRequest, DcrResponse}`. Defining a parallel set in `src/client/*` guarantees drift the first time RFC 7591 gains an optional field.
- **Do NOT migrate the legacy `~/.pmcp/oauth-tokens.json` cache.** CONTEXT.md D-07 says leave it alone; users re-login once. Migration code is dead weight and a source of schema-confusion bugs.
- **Do NOT add `--client` to `auth refresh` / `auth token` / `auth status`.** D-17 explicitly locks this to `login` only. Re-identifying on refresh is meaningless — the issuer already knows the client_id.
- **Do NOT hand-roll a file lock.** `flock(2)` / Windows file locking is an ecosystem of sharp edges; `tempfile::NamedTempFile::persist` is the idiomatic lock-free alternative for user-private config files (mirroring how `gh`, `aws`, `gcloud` all handle their credential stores).
- **Do NOT print the access token in `auth login`.** D-12. Shared terminals and shell history are threat surfaces.
- **Do NOT log the access token at any verbosity level.** Existing `OAuthHelper::create_middleware_chain` logs `&access_token[..20]` at `src/client/oauth.rs:642-645` — this is already borderline; **do not extend this pattern** to the new cache code path.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DCR request/response types | New `ClientRegistrationRequest` struct | `pmcp::server::auth::provider::{DcrRequest, DcrResponse}` | Already RFC 7591 complete with `#[serde(flatten)] extra: HashMap<...>` forward-compat escape hatch. `[VERIFIED: src/server/auth/provider.rs:302-382]` |
| OIDC discovery | `reqwest::get("/.well-known/...")` directly | `pmcp::client::auth::OidcDiscoveryClient::discover()` | Handles retries, CORS, openid-configuration path construction. `[VERIFIED: src/client/auth.rs:136]` |
| PKCE S256 code verifier/challenge | SHA-256 dance | `OAuthHelper::generate_code_verifier` / `generate_code_challenge` | Already in `src/client/oauth.rs:247-258`; battle-tested |
| Token refresh HTTP flow | `reqwest::post(token_endpoint).form(...)` | `OAuthHelper::refresh_token` (private — consider exposing) | Already in `src/client/oauth.rs:546-573` |
| Atomic file write | `rename` + `open(O_EXCL)` + error handling | `tempfile::NamedTempFile::persist` | Cross-platform, already a dep |
| Home dir resolution | `std::env::var("HOME")` | `dirs::home_dir()` | Handles Windows `%USERPROFILE%`, already a dep |
| Duration formatting ("expires in 2h 15m") | `format!("{} seconds", …)` | `chrono::Duration::to_std() + custom formatter` or `humantime` (NOT a dep — don't add) | Existing `chrono` dep sufficient; write a tiny helper |

**Key insight:** This phase is 90% plumbing, 10% new logic. Nearly every "hard" primitive (DCR types, discovery, PKCE, refresh, atomic write) already exists in the tree. Planning tasks should be scoped around **gluing existing primitives together**, not reimplementing them.

## Runtime State Inventory

*This phase is additive greenfield, not a rename/refactor. No runtime state inventory required.*

For completeness:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — new cache file `~/.pmcp/oauth-cache.json` is greenfield | None |
| Live service config | Legacy `~/.pmcp/oauth-tokens.json` exists for some users (single-server SDK cache) | **No migration** per D-07 — leave untouched; users re-login once to populate new multi-server cache |
| OS-registered state | None | None |
| Secrets/env vars | Existing `MCP_API_KEY`, `MCP_OAUTH_*` env vars (unchanged) | None |
| Build artifacts | None | None |

## Common Pitfalls

### Pitfall 1: Discovery path mismatch (OIDC vs OAuth 2.0 metadata)

**What goes wrong:** `OidcDiscoveryClient::discover()` hits `/.well-known/openid-configuration` at `src/client/auth.rs:137`. RFC 8414 servers (pure OAuth 2.0, no OIDC) publish at `/.well-known/oauth-authorization-server`.
**Why it happens:** Our existing discovery only implements the OIDC well-known. pmcp.run happens to serve both at `main.rs:3412-3413`, so existing tests pass — but third-party OAuth-only servers may 404 on openid-configuration.
**How to avoid:** For this phase, assume the existing discovery behavior is good enough (pmcp.run is the target). Document as a follow-up hardening: `OidcDiscoveryClient` could try openid-configuration first and fall back to oauth-authorization-server.
**Warning signs:** DCR integration test against a pure OAuth 2.0 mock returns 404 on discovery.

### Pitfall 2: `client_id: String` vs `client_id: Option<String>` breaking change

**What goes wrong:** `OAuthConfig::client_id: String` is not `Option<String>`. D-03 requires checking `client_id.is_none()` to trigger DCR. Flipping to `Option<String>` technically breaks every struct-literal construction at the call site (requires wrapping existing `"foo".to_string()` → `Some("foo".to_string())`).
**Why it happens:** CONTEXT.md D-02 says "additive, backward-compatible" but the specified check (`client_id.is_none()`) demands `Option`.
**How to avoid:** Treat this as a **minor-version breaking change** at the struct-literal level — pmcp is pre-1.0 at v2.x (breaking-change window per `MEMORY.md` v2.0 cleanup philosophy). Update the two existing call sites (`cargo-pmcp/src/commands/auth.rs:36` and any example) to wrap in `Some(...)`. Alternative: sentinel empty-string meaning "no client_id" — **REJECTED** as error-prone.
**Warning signs:** Cannot compile `cargo build --features oauth` without call-site updates.

### Pitfall 3: Cache directory permissions on first-run

**What goes wrong:** `~/.pmcp/` may be created with default umask (e.g., `0o755`), making the cache file world-readable on multi-user systems before `chmod 600` is applied to the file itself.
**Why it happens:** Current `default_cache_path()` at `src/client/oauth.rs:663-668` creates `~/.pmcp/` via `create_dir_all` with default permissions, and the existing `cache_token` at `src/client/oauth.rs:610` writes via `tokio::fs::write` with no chmod.
**How to avoid:**
1. On the directory: set `0o700` after `create_dir_all`.
2. On the file: write tempfile → `set_permissions(0o600)` on the tempfile → persist (atomic rename preserves mode on Unix).
3. On read: log-warn if mode is looser than `0o600` (do NOT reject — users sometimes have legitimate shared-machine setups).
**Warning signs:** `umask 022` on the dev machine → cache is `0o644` instead of `0o600`.

### Pitfall 4: Concurrent `auth login` race (losing refresh_token)

**What goes wrong:** User opens two terminals, runs `cargo pmcp auth login url1` in each. Both complete the PKCE flow in separate browser windows. Writer B's `persist()` overwrites Writer A's entry for `url1`, losing A's token.
**Why it happens:** `NamedTempFile::persist` is last-writer-wins atomic replace; it replaces the whole cache file, not just the entry for `url1`.
**How to avoid:** Read-modify-write the entire cache file **in one critical section**: (1) read current cache from disk, (2) mutate `entries` map, (3) write via atomic rename. The window is ms-small; true simultaneous-browser logins are exceedingly rare. Accept last-writer-wins as the documented semantic.
**Warning signs:** Docs / inline comment on `write_cache_atomic`: "last-writer-wins; concurrent login not isolated. Retrying logout+login on the loser side is the escape hatch."

### Pitfall 5: Refresh token is null for some IdPs

**What goes wrong:** Some OAuth providers (notably, public-PKCE clients against certain Cognito configurations) do NOT return a `refresh_token` even on initial auth-code exchange. D-15 silent-refresh path panics if the cache entry has `refresh_token: None` at expiry time.
**Why it happens:** RFC 6749 does not require issuing a refresh_token; OIDC profile does. Public PKCE clients often skip it to reduce attack surface.
**How to avoid:** Treat `refresh_token: None` as a cached entry that cannot self-refresh. At expiry: return an error with actionable message: `"cached token for <url> expired and no refresh_token is available — run 'cargo pmcp auth login <url>' to re-authenticate"`. `auth refresh <url>` errors per D-16.
**Warning signs:** Integration test against a mock that omits `refresh_token` — ensure the absence path is tested.

### Pitfall 6: URL normalization collisions

**What goes wrong:** `https://mcp.example.com/v1/sql` and `https://mcp.example.com/v2/billing` both normalize to `https://mcp.example.com` (path stripped). If they use different OAuth issuers, the cache entry is ambiguous.
**Why it happens:** D-06 strips path per design; multi-app-per-host is deferred.
**How to avoid:** Document in the `auth login` help: "cached per origin (scheme://host:port). If two apps at the same origin use different OAuth issuers, the most recent `login` wins." Refer to deferred composite key.
**Warning signs:** User reports "login to A broke login to B at the same host."

### Pitfall 7: `token_endpoint_auth_method: "none"` rejected by some IdPs

**What goes wrong:** D-05 hardcodes `"none"` (public PKCE client, no secret). Some enterprise IdPs require `"client_secret_basic"` or `"client_secret_post"` even for DCR, rejecting `"none"` with a 400.
**Why it happens:** RFC 7591 §2 lists `"none"` as valid but allows IdPs to reject methods not in their `token_endpoint_auth_methods_supported` discovery metadata.
**How to avoid:** (a) Check `metadata.token_endpoint_auth_methods_supported` contains `"none"` before sending; if not, return actionable error `"server requires confidential-client DCR which is not supported — pass a pre-registered --oauth-client-id"`. (b) pmcp.run itself accepts `"none"` per its permissive DCR handler `[CITED: pmcp-run/control-plane/oauth-proxy/src/main.rs:2209]`.
**Warning signs:** DCR test against a non-pmcp.run IdP returns `{ "error": "invalid_client_metadata" }`.

### Pitfall 8: pentest.rs flag migration CI breakage

**What goes wrong:** Moving pentest from its local `--api-key` to `#[command(flatten)] auth: AuthFlags` changes the flag surface — `--oauth-*` flags appear where they didn't exist. If `pentest.rs` `execute()` only uses `--api-key`, `--oauth-client-id` silently does nothing (confusing UX).
**Why it happens:** D-21 migration is an additive flag surface expansion.
**How to avoid:** Route all `AuthFlags` values through `resolve_auth_middleware` in `execute_pentest`. Since pentest already uses `reqwest` HTTP, adding the middleware chain to its client builder is small.
**Warning signs:** `pentest --oauth-client-id foo <url>` runs but sends no auth header.

## Code Examples

### DCR HTTP call (SDK)

```rust
// src/client/oauth.rs — new private method on OAuthHelper
// Source: pattern mirrors src/server/auth/providers/generic_oidc.rs:641-675
//         request/response types are reused from src/server/auth/provider.rs:302-382
async fn do_dynamic_client_registration(
    &self,
    registration_endpoint: &str,
) -> Result<crate::server::auth::provider::DcrResponse> {
    use crate::server::auth::provider::DcrRequest;

    let client_name = self
        .config
        .client_name
        .clone()
        .unwrap_or_else(|| "pmcp-sdk".to_string());   // D-04 fallback
    let redirect_uri = format!("http://localhost:{}/callback", self.config.redirect_port);

    let request = DcrRequest {
        redirect_uris: vec![redirect_uri],
        client_name: Some(client_name),
        client_uri: None,
        logo_uri: None,
        contacts: vec![],
        token_endpoint_auth_method: Some("none".to_string()),  // D-05 public PKCE
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec![],  // not required; IdP may default
        scope: None,
        software_id: None,
        software_version: None,
        extra: Default::default(),
    };

    let response = self
        .client
        .post(registration_endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::internal(format!("DCR request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::internal(format!(
            "DCR failed ({}): {}\n\
             \n\
             The server rejected dynamic client registration. Possible causes:\n\
             - The server does not allow public PKCE clients (token_endpoint_auth_method=none)\n\
             - The server requires software_statement JWT authentication\n\
             - Rate limiting on the registration endpoint\n\
             \n\
             Pass a pre-registered client_id with --oauth-client-id as a workaround.",
            status, body
        )));
    }

    response
        .json::<crate::server::auth::provider::DcrResponse>()
        .await
        .map_err(|e| Error::internal(format!("Failed to parse DCR response: {e}")))
}
```

### TokenCacheV1 schema (CLI)

```rust
// cargo-pmcp/src/commands/auth_cmd/cache.rs
// Source: schema per CONTEXT.md D-07
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheV1 {
    /// Schema version for forward compatibility.
    /// Readers reject any value != 1 and suggest the user upgrade cargo-pmcp.
    pub schema_version: u32,
    /// Map from normalized server URL (scheme://host[:port]) to credential entry.
    pub entries: std::collections::BTreeMap<String, TokenCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheEntry {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,       // Unix seconds
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub client_id: String,             // DCR-issued or user-provided; needed for refresh
}

impl TokenCacheV1 {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn empty() -> Self {
        Self { schema_version: Self::CURRENT_VERSION, entries: Default::default() }
    }

    pub fn read(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let v: Self = serde_json::from_str(&s)
                    .map_err(|e| anyhow::anyhow!("cache file corrupt: {e} (delete {:?} to reset)", path))?;
                if v.schema_version != Self::CURRENT_VERSION {
                    anyhow::bail!(
                        "cache schema_version {} unsupported (expected {}); upgrade cargo-pmcp",
                        v.schema_version, Self::CURRENT_VERSION
                    );
                }
                Ok(v)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
            Err(e) => Err(anyhow::anyhow!("failed to read cache: {e}")),
        }
    }
}
```

`BTreeMap` is chosen over `HashMap` for deterministic JSON output (easier diff-ing / debugging).

### Precedence resolution (CLI — modified `resolve_auth_middleware`)

```rust
// cargo-pmcp/src/commands/auth.rs — MODIFIED
// Source: CONTEXT.md D-13 precedence; D-15 silent refresh
pub async fn resolve_auth_middleware(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    match auth_method {
        AuthMethod::ApiKey(key) => {
            /* existing — unchanged */
        },
        AuthMethod::OAuth { .. } => {
            /* existing — unchanged (explicit flag wins per D-13/D-14) */
        },
        AuthMethod::None => {
            // D-13: cache is the lowest-precedence fallback
            let cache_path = default_multi_cache_path();   // NEW: ~/.pmcp/oauth-cache.json
            let cache = TokenCacheV1::read(&cache_path)?;
            let key = normalize_cache_key(mcp_server_url)?;
            let Some(entry) = cache.entries.get(&key) else {
                return Ok(None);   // no cached auth — pass through (existing behavior)
            };

            // D-15: transparent refresh when near expiry
            let access_token = if is_near_expiry(entry, /*grace_secs*/ 60) {
                refresh_and_persist(&cache_path, &key, entry).await
                    .map_err(|e| anyhow::anyhow!(
                        "cached token for {} expired and refresh failed: {e}\n\
                         Run `cargo pmcp auth login {}` to re-authenticate.",
                        key, key
                    ))?
            } else {
                entry.access_token.clone()
            };

            Ok(Some(bearer_chain(access_token)))
        },
    }
}
```

### `auth token` (stdout-only behavior per D-11)

```rust
// cargo-pmcp/src/commands/auth_cmd/token.rs
pub async fn execute(args: TokenArgs) -> Result<()> {
    let cache = TokenCacheV1::read(&default_multi_cache_path())?;
    let key = normalize_cache_key(&args.url)?;
    let entry = cache.entries.get(&key).ok_or_else(|| {
        anyhow::anyhow!(
            "no cached token for {}. Run `cargo pmcp auth login {}` first.",
            key, key
        )
    })?;

    let token = if is_near_expiry(entry, 60) {
        refresh_and_persist(&default_multi_cache_path(), &key, entry).await?
    } else {
        entry.access_token.clone()
    };

    // D-11: raw token to stdout + newline; no stderr on success
    println!("{}", token);
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded OAuth client_ids in CLI configs | RFC 7591 Dynamic Client Registration | RFC finalized 2015; rapid adoption 2018-2020 | Enterprise IdPs now commonly support DCR; pmcp.run mandates it for pmcp-run-hosted servers |
| Single-profile credential files | Per-host/per-profile keyed stores | `gh` CLI 2019+, `gcloud` 2015+, `aws` 2013+ | Standard user expectation; cargo-pmcp catches up |
| Plaintext tokens in `~/.config/...` | Same plaintext + `chmod 600`, keyring as opt-in | Current industry standard (all three of gh/aws/gcloud use plaintext by default) | Keyring is out-of-scope per CONTEXT.md deferred |
| OpenID Connect `/.well-known/openid-configuration` only | RFC 8414 `/.well-known/oauth-authorization-server` also | 2018 RFC 8414 finalized | Our discovery still hits OIDC-only; see Pitfall 1 for follow-up |

**Deprecated/outdated:**
- Legacy pmcp single-blob `~/.pmcp/oauth-tokens.json` — kept, not migrated, not deleted per D-07.
- Device code flow (RFC 8628) fallback in `OAuthHelper::device_code_flow_with_metadata` — unchanged by this phase; remains the fallback when PKCE fails.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Making `OAuthConfig::client_id` optional (`String → Option<String>`) is acceptable despite D-02 saying "backward-compatible" because pmcp is pre-1.0 in its v2.x breaking-change window per MEMORY.md v2.0 cleanup philosophy | Pitfall 2, Open Questions §1 | If operator insists on zero call-site churn, must use sentinel empty-string (ugly) or rename field (worse) |
| A2 | pmcp.run's DCR handler at main.rs:2209 accepts `token_endpoint_auth_method: "none"` without issue | D-05 / Pitfall 7 | If rejected, we need a `"client_secret_basic"` fallback path — adds one task |
| A3 | All existing `OAuthConfig` struct-literal constructions in the workspace are in the repo (no external crates depend on `pmcp::client::oauth::OAuthConfig`) | Pitfall 2 | If external downstream crates exist, `client_id: String → Option<String>` is a public breaking change requiring a major bump |
| A4 | `mockito 1.5.0` supports mocking the DCR endpoint POST with JSON body matching | Validation Architecture | `mockito` JSON matching is known-good; worst case is regex match on body |

**Assumption mitigation:** All assumptions above are validated by the planner in PLAN-CHECK phase OR by discuss-phase before implementation starts. A1 is the only one that could force a plan rewrite.

## Open Questions (RESOLVED)

All open questions have been resolved during PLAN-CHECK revision (iteration 1). Each question below carries an inline **RESOLVED:** annotation with the disposition applied in Plan 01 / Plan 02.

1. **Should `OAuthConfig::client_id` become `Option<String>`?**
   - What we know: D-03 requires `client_id.is_none()` check. Current field is `String`.
   - What's unclear: Whether CONTEXT.md's "additive, backward-compatible" (D-02) means "no public-API source-level breakage" OR "no semantic breakage for existing users who provide a client_id".
   - Recommendation: Adopt `Option<String>`. Pre-1.0 v2.x is in the "breaking-change window" per `MEMORY.md v2.0 cleanup philosophy`. Update the two in-tree call sites (`cargo-pmcp/src/commands/auth.rs:36` + example) to wrap in `Some(...)`. Planner should confirm with user in any pre-execution review.
   - **RESOLVED:** `Option<String>` adopted per CONTEXT.md D-02 (operator decision confirmed in discuss-phase). Plan 01 Task 1.1 performs the field-type flip AND updates every in-tree caller discovered during revision: `cargo-pmcp/src/commands/auth.rs`, `cargo-pmcp/src/commands/flags.rs`, `examples/c07_oidc_discovery.rs`, `examples/m01_basic_middleware.rs`, `benches/comprehensive_benchmarks.rs`, AND `crates/mcp-tester/src/main.rs:598` (added during Blocker #1 fix). `cargo-pmcp/src/commands/preview.rs:75` explicitly excluded — different struct (see Q2 resolution notes).

2. **Should DCR live in `src/client/oauth.rs` or `src/client/dcr.rs`?**
   - What we know: `src/client/oauth.rs` is 693 LOC today. Adding ~150 LOC for DCR (types re-export + one async method + one example code path) puts it at ~850 LOC.
   - What's unclear: Project style preference for max file length.
   - Recommendation: **Inline in `src/client/oauth.rs`** since the DCR flow is tightly coupled to `OAuthHelper`'s PKCE path. Split only if the file crosses 1500 LOC after full implementation. (Planner decides at code-write time.)
   - **RESOLVED:** Keep DCR inline in `src/client/oauth.rs`. 693 LOC base + ~250 LOC added (DCR method + AuthorizationResult + resolver + tests) ~= 940 LOC, comfortably under the 1500 LOC split threshold. Tight coupling to the PKCE flow (DCR returns the client_id the PKCE step consumes) makes a separate file harmful. Revision note: Plan 01 Task 1.1 Step 5 explicitly lists `cargo-pmcp/src/commands/preview.rs` as DO-NOT-MODIFY — that file constructs `mcp_preview::OAuthPreviewConfig` (different crate, different struct at `crates/mcp-preview/src/handlers/api.rs:33`), NOT the SDK's `pmcp::client::oauth::OAuthConfig`.

3. **Should the cache file include an `updated_at: u64` top-level timestamp?**
   - What we know: D-07 schema specifies `schema_version` + `entries`. No `updated_at`.
   - What's unclear: Whether users would want this for diagnostics.
   - Recommendation: **Defer.** Cache freshness can be read from `mtime` via `fs::metadata`. One less surface to bikeshed.
   - **RESOLVED:** Deferred from v1 schema. `fs::metadata().modified()` provides equivalent information without schema surface. Re-evaluate if a user asks for in-file diagnostics; would be a `schema_version: 2` additive field.

4. **Do we promote `normalize_cache_key` into the SDK public API?**
   - What we know: `OAuthHelper::extract_base_url` at `src/client/oauth.rs:113` is identical logic but private.
   - What's unclear: Whether library users have asked for cache-key normalization.
   - Recommendation: **Defer.** Keep it CLI-local in `cargo-pmcp/src/commands/auth_cmd/cache.rs`. Promote only if a second caller emerges.
   - **RESOLVED:** Deferred. `normalize_cache_key` stays internal to `cargo-pmcp` (not re-exported). If a second consumer appears (e.g., a future PMCP CLI plugin), promote then — YAGNI for now.

5. **Does `pentest.rs` migration to `AuthFlags` include re-running the existing pentest test suite for flag-parsing regressions?**
   - What we know: D-21 is a straight flag-surface consolidation.
   - What's unclear: Whether pentest has integration tests that assert on `--api-key` alone.
   - Recommendation: Planner adds one task "run full pentest test suite post-migration" to the G-matrix.
   - **RESOLVED:** Plan 02 Task 2.4 covers this (T-74-F/pentest scope). Task 2.4 Step 1 migrates `pentest.rs` to `AuthFlags`; Task 2.4 `verify` block runs `cargo test -p cargo-pmcp` which exercises existing pentest tests. Regression coverage is passive (any pre-existing `--api-key`-asserting tests will fail if backward compat breaks). If no such tests exist, the `cargo run -p cargo-pmcp -- pentest --api-key foo https://x` smoke command in the plan-level verification confirms the backward-compat path.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain | All builds | ✓ | 1.83.0+ (project min) | — |
| cargo | All builds | ✓ | bundled with toolchain | — |
| `make` | `make quality-gate` | ✓ | standard on macOS/Linux dev machines | — |
| Internet access for `cargo search` / `cargo publish` | release workflow | ✓ | — | — |
| `mockito` test server | DCR integration tests | ✓ (dev-dep) | 1.5.0 | — |
| `cargo-fuzz` | fuzz tests (ALWAYS req) | ✗ (separate install) | — | `cargo install cargo-fuzz` — plan Task 0 or Wave 0 to confirm |
| pmcp.run dev instance | manual E2E validation | unknown (external) | — | Rely on mockito for automated tests; manual validation optional |

**Missing dependencies with fallback:** `cargo-fuzz` installation can be a one-line prereq in Wave 0.

**Missing dependencies, blocking:** None.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (standard Rust test harness) + `proptest` + `mockito` + `cargo-fuzz` |
| Config file | `Cargo.toml` `[dev-dependencies]` (no custom config) |
| Quick run command | `cargo test -p pmcp --features oauth oauth::dcr` + `cargo test -p cargo-pmcp auth_cmd` |
| Full suite command | `make quality-gate` (matches CI) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SDK-DCR-01 | DCR fires when all D-03 conditions are true | unit | `cargo test -p pmcp --features oauth oauth::dcr::fires_when_eligible -x` | Wave 0 |
| SDK-DCR-01 | DCR skipped when `dcr_enabled == false` | unit | `cargo test -p pmcp --features oauth oauth::dcr::disabled_skips_dcr -x` | Wave 0 |
| SDK-DCR-01 | DCR skipped when `client_id.is_some()` (D-20 escape hatch) | unit | `cargo test -p pmcp --features oauth oauth::dcr::client_id_skips_dcr -x` | Wave 0 |
| SDK-DCR-01 | Actionable error when no registration_endpoint advertised (D-03 error surface) | unit | `cargo test -p pmcp --features oauth oauth::dcr::no_registration_endpoint_errors -x` | Wave 0 |
| SDK-DCR-01 | DCR request body matches RFC 7591 public PKCE shape (D-05) | integration | `cargo test -p pmcp --features oauth --test oauth_dcr_integration request_body_shape -x` | Wave 0 |
| SDK-DCR-01 | DCR response parsing extracts `client_id`; ignores unknown fields via `#[serde(flatten)]` | property | `cargo test -p pmcp --features oauth oauth::dcr::proptests -- --ignored property_` | Wave 0 |
| SDK-DCR-01 | DCR response parser robust to malformed JSON (does not panic) | fuzz | `cd fuzz && cargo fuzz run dcr_response_parser -- -max_total_time=60` | Wave 0 (new fuzz target) |
| SDK-DCR-01 | D-04 fallback: `client_name = None` → body contains `"pmcp-sdk"` | unit | `cargo test -p pmcp --features oauth oauth::dcr::default_client_name_fallback -x` | Wave 0 |
| SDK-DCR-01 | Working example demonstrating DCR from library user's POV | example | `cargo build --example c08_oauth_dcr --features oauth` | Wave 0 (examples/c08_oauth_dcr.rs) |
| CLI-AUTH-01 | `auth login` writes entry to cache; success msg omits token (D-12) | integration | `cargo test -p cargo-pmcp --test auth_integration login_happy_path -x` | Wave 0 |
| CLI-AUTH-01 | `auth login --client claude-desktop` sets `OAuthConfig::client_name` (D-17) | unit | `cargo test -p cargo-pmcp auth_cmd::login::client_flag_sets_client_name -x` | Wave 0 |
| CLI-AUTH-01 | `--client` and `--oauth-client-id` conflict at parse time (D-19) | unit | `cargo test -p cargo-pmcp auth_cmd::login::client_conflicts_with_oauth_client_id -x` | Wave 0 |
| CLI-AUTH-01 | `auth logout` no-args errors (D-09) | unit | `cargo test -p cargo-pmcp auth_cmd::logout::no_args_errors -x` | Wave 0 |
| CLI-AUTH-01 | `auth logout <url>` removes single entry | unit | `cargo test -p cargo-pmcp auth_cmd::logout::removes_single -x` | Wave 0 |
| CLI-AUTH-01 | `auth logout --all` empties entries | unit | `cargo test -p cargo-pmcp auth_cmd::logout::all_removes_all -x` | Wave 0 |
| CLI-AUTH-01 | `auth status` prints tabular output; no args = all servers (D-10) | unit | `cargo test -p cargo-pmcp auth_cmd::status::tabular_output -x` | Wave 0 |
| CLI-AUTH-01 | `auth token <url>` raw stdout, newline-terminated (D-11) | unit | `cargo test -p cargo-pmcp auth_cmd::token::raw_stdout -x` | Wave 0 |
| CLI-AUTH-01 | `auth token <url>` no-cache errors actionably | unit | `cargo test -p cargo-pmcp auth_cmd::token::missing_entry_errors -x` | Wave 0 |
| CLI-AUTH-01 | `auth refresh <url>` no-refresh-token errors (D-16) | unit | `cargo test -p cargo-pmcp auth_cmd::refresh::no_refresh_token_errors -x` | Wave 0 |
| CLI-AUTH-01 | URL normalization: strip path, lowercase host, strip default port, strip trailing slash (D-06) | property | `cargo test -p cargo-pmcp auth_cmd::cache::normalize_key_properties -- --ignored property_` | Wave 0 |
| CLI-AUTH-01 | TokenCacheV1 JSON round-trip stable | property | `cargo test -p cargo-pmcp auth_cmd::cache::serde_roundtrip -- --ignored property_` | Wave 0 |
| CLI-AUTH-01 | Precedence: explicit flag > env > cache (D-13) | integration | `cargo test -p cargo-pmcp --test auth_integration precedence_ordering -x` | Wave 0 |
| CLI-AUTH-01 | Silent fallback (no warning) when flag and cache both present (D-14) | integration | `cargo test -p cargo-pmcp --test auth_integration silent_fallback -x` | Wave 0 |
| CLI-AUTH-01 | Transparent refresh at 60s-before-expiry (D-15) | integration | `cargo test -p cargo-pmcp --test auth_integration near_expiry_refreshes -x` | Wave 0 |
| CLI-AUTH-01 | Atomic cache write survives write interrupt (no half-written file) | integration | `cargo test -p cargo-pmcp --test auth_integration atomic_write -x` | Wave 0 |
| CLI-AUTH-01 | Cache file written with `0o600` on Unix | unit | `cargo test -p cargo-pmcp auth_cmd::cache::unix_mode_600 -x` (gated `#[cfg(unix)]`) | Wave 0 |
| CLI-AUTH-01 | Legacy `~/.pmcp/oauth-tokens.json` left untouched (D-07) | integration | `cargo test -p cargo-pmcp --test auth_integration legacy_cache_untouched -x` | Wave 0 |
| CLI-AUTH-01 | pentest with `--oauth-client-id` now uses OAuth middleware (D-21) | integration | `cargo test -p cargo-pmcp --test pentest_integration oauth_via_authflags -x` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p <crate> <module>` (< 10 sec per module)
- **Per wave merge:** `cargo test -p pmcp --features oauth && cargo test -p cargo-pmcp` (< 60 sec full)
- **Phase gate:** `make quality-gate` (full, CI-matching) + `cargo fuzz run dcr_response_parser -- -max_total_time=60` before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `src/client/oauth.rs` — add DCR unit tests module (`mod dcr_tests { … }`)
- [ ] `tests/oauth_dcr_integration.rs` (NEW, pmcp) — mockito-driven DCR integration tests
- [ ] `fuzz/fuzz_targets/dcr_response_parser.rs` (NEW, pmcp) — fuzz target for DcrResponse parser
- [ ] `examples/c08_oauth_dcr.rs` (NEW, pmcp) — satisfies CLAUDE.md ALWAYS "example" requirement
- [ ] `cargo-pmcp/tests/auth_integration.rs` (NEW) — end-to-end auth subcommand tests
- [ ] `cargo-pmcp/src/commands/auth_cmd/*.rs` (NEW modules) — each with inline `#[cfg(test)] mod tests`
- [ ] `cargo-pmcp` Cargo.toml: promote `tempfile` from dev-deps to regular deps
- [ ] `cargo install cargo-fuzz` — Wave 0 prereq confirmation (or via `rustup component` if available)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | RFC 7636 PKCE (already implemented) + RFC 7591 DCR (new). Both are industry-standard OAuth 2.0 client authentication. |
| V3 Session Management | yes | Token expiry check (60s grace per D-15); refresh_token rotation (on-demand per D-16) |
| V4 Access Control | partial | Not introducing new access decisions — we're a client consuming server-side ACL. Pass-through for bearer tokens. |
| V5 Input Validation | yes | URL normalization (validate `http(s)` scheme only), JSON schema version check, `clap` already validates enum flags |
| V6 Cryptography | yes | SHA-256 via existing `sha2` crate for PKCE (already implemented); NO new crypto — do NOT hand-roll |
| V7 Error Handling | yes | All DCR failure modes return actionable errors with next-step guidance (see Common Pitfalls) |
| V8 Data Protection | yes | File permissions (`0o600` on cache, `0o700` on `~/.pmcp/` dir); no token logging at any verbosity |
| V9 Communication | yes | TLS via rustls through reqwest; no plaintext OAuth endpoints |
| V10 Malicious Code | — | N/A (we're shipping a binary; covered by Cargo supply chain) |

### Known Threat Patterns for OAuth CLI + DCR

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| DCR request to attacker-controlled `registration_endpoint` (discovery spoofing) | Tampering + Elevation | Require HTTPS on `registration_endpoint` (reject `http://` except `localhost`); rely on TLS cert validation via rustls. Document: "DCR trusts the discovery document — use HTTPS-only issuers." |
| Token cache file tampering | Tampering | `chmod 600` on write; warn-log (do not reject) on looser modes at read time. Document: assumes single-user trust boundary on `~/.pmcp/`. |
| Token leak via stderr in verbose/debug logs | Information Disclosure | Grep new code for `tracing::*!("…{}…", token)` patterns; audit existing `src/client/oauth.rs:642-645` on the cache path (existing behavior leaks first 20 chars — do NOT extend). |
| `auth token <url>` in command line leaked to process listings | Information Disclosure | D-11 prints to stdout only (no arg-encoded token); usage doc: `TOKEN=$(cargo pmcp auth token URL)` idiom. The URL is the ONLY argument — URLs are not secrets. |
| Concurrent auth login race leaks one user's token to another | Information Disclosure | Single-user threat model — `~/.pmcp/` is user-private. Not relevant for multi-user shared systems (explicit out-of-scope). |
| Cache file enumeration by cross-user attacker on shared machine | Information Disclosure | `~/.pmcp/` mode `0o700`; cache file mode `0o600`. Matches gh/aws/gcloud conventions. |
| Malicious DCR response containing oversized fields (memory DoS) | DoS | `reqwest` response body size limit (default ~unlimited — consider adding 1MB cap for DCR endpoint); `serde_json` is resilient to oversized strings. Low risk for user-initiated flow. |
| Refresh token rotation attack (theft + replay) | Spoofing | On refresh failure with `invalid_grant`, invalidate cache entry immediately (do NOT retry). Force `auth login` re-auth. |
| `--client` name injection routing to attacker-controlled Cognito branding | Tampering + Spoofing | Server-side matter (pmcp.run's `classify_client_type` at `main.rs:2853`). Our client just passes the literal string. No SSRF risk since we don't fetch anything based on `--client` content. |
| Open redirect via DCR `redirect_uris` | Tampering | We control the redirect_uri (always `http://localhost:<port>/callback`); no user input flows into it. |

### Threat Model Seeds for PLAN.md `<threat_model>` section

1. **T1 — Discovery spoofing:** user logs in to `https://evil.example.com/mcp` with DNS hijack; DCR posts `client_name="cargo-pmcp"` to attacker-controlled registration_endpoint. Mitigation: TLS + cert validation (existing); document the trust boundary.
2. **T2 — Cache file tampering:** another process on the same machine writes a bogus cached entry. Mitigation: `0o600` on file, `0o700` on dir; single-user trust assumption; log-warn on looser modes at read.
3. **T3 — Token leak via logs:** tracing at debug/trace level in new DCR path. Mitigation: explicit `tracing::debug!("Token cached for {}", normalized_url)` — never the token bytes.
4. **T4 — Near-expiry refresh failure cascades to unauthenticated request:** if refresh fails silently, the consumer command retries without auth → "unauthorized" error. Mitigation: D-15 actionable error message `run auth login <url> to re-authenticate`; propagate up, don't retry unauthed.
5. **T5 — Concurrent login race:** last-writer-wins on cache file. Mitigation: document; rare; no security impact (both writers are the same user).

## Project Constraints (from CLAUDE.md)

- **Zero-tolerance quality gates:** `make quality-gate` must pass before any commit. Includes `cargo fmt --all`, `cargo clippy` with pedantic+nursery lints (via `--features full`), doctests, build.
- **ALWAYS requirements for new features** (mandatory, not optional):
  - Fuzz testing (`cargo fuzz run <target>`)
  - Property testing (`cargo test -- --ignored property_`)
  - Unit testing (80%+ coverage target)
  - Example demonstration (`cargo run --example <name>`)
- **Pre-commit hook:** cannot commit without passing quality gates. Emergency override `--no-verify` requires immediate follow-up.
- **Cognitive complexity ≤ 25** per function.
- **Zero SATD** (self-admitted technical debt) comments.
- **Contract-first:** per CLAUDE.md "Contract-First Development", new features write contract YAML in `../provable-contracts/contracts/<crate>/` first, run `pmat comply check`, then implement. Planner should verify existing contracts in that directory and add new ones for DCR + auth subcommands.
- **Release workflow:** bump `pmcp` first; update `cargo-pmcp`'s `pmcp = { version = "..." }` pin; release via tag `vX.Y.Z` → CI publishes both.
- **Only bump crates that have changed:** mcp-tester / mcp-preview / pmcp-macros are NOT bumped in this phase.
- **Pre-flight before release:** `rustup update stable` + `cargo search pmcp cargo-pmcp` + `git diff --stat vLAST..HEAD -- src/ crates/ cargo-pmcp/`.
- **Local `cargo clippy` is weaker than CI** — always run `make quality-gate` (matches CI: `--features full` with pedantic+nursery lint groups).
- **Justfile preferred over Makefile** per user global CLAUDE.md — however, the project ALREADY uses Makefile (`make quality-gate`). Do not introduce a justfile in this phase; defer to project convention.

## Sources

### Primary (HIGH confidence)

- `src/client/oauth.rs` (existing OAuthHelper, OAuthConfig, TokenCache, default_cache_path) — `[VERIFIED: file read 2026-04-21]`
- `src/client/auth.rs` (OidcDiscoveryClient) — `[VERIFIED: file read]`
- `src/server/auth/provider.rs:302-382` (DcrRequest, DcrResponse — reusable) — `[VERIFIED: file read]`
- `src/server/auth/oauth2.rs:172-220` (OidcDiscoveryMetadata with registration_endpoint field) — `[VERIFIED: file read]`
- `src/server/auth/providers/generic_oidc.rs:641-675` (server-side DCR reference impl) — `[VERIFIED: file read]`
- `cargo-pmcp/src/commands/auth.rs` (resolve_auth_middleware, resolve_auth_header) — `[VERIFIED: file read]`
- `cargo-pmcp/src/commands/flags.rs:108-158` (AuthFlags, AuthMethod, resolve) — `[VERIFIED: file read]`
- `cargo-pmcp/src/commands/pentest.rs:60-64` (duplicate --api-key) — `[VERIFIED: file read]`
- `cargo-pmcp/src/main.rs:69-108` (Commands enum, Test pattern) — `[VERIFIED: file read]`
- `cargo-pmcp/src/commands/mod.rs` (existing `pub mod auth;` collision) — `[VERIFIED: file read]`
- `Cargo.toml` (pmcp deps incl. reqwest, tempfile, mockito, proptest) — `[VERIFIED: file read]`
- `cargo-pmcp/Cargo.toml` (dep versions, oauth2, colored, tempfile-in-dev) — `[VERIFIED: file read]`
- `/Users/guy/Development/mcp/sdk/pmcp-run/control-plane/oauth-proxy/src/main.rs:707-755, 2209, 2853` (wire behavior) — `[VERIFIED: file read]`
- `.planning/REQUIREMENTS.md:74-79` (SDK-DCR-01, CLI-AUTH-01) — `[VERIFIED: file read]`
- `.planning/STATE.md` (milestone context, Phase 74 entry) — `[VERIFIED: file read]`

### Secondary (MEDIUM confidence — cited)

- RFC 7591 §2 (all client metadata optional; `token_endpoint_auth_method: "none"` for public clients) — `[CITED: https://datatracker.ietf.org/doc/html/rfc7591]`
- RFC 7591 §3.2.2 (error response: `error` required ASCII, `error_description` optional) — `[CITED: https://datatracker.ietf.org/doc/html/rfc7591#section-3.2.2]`
- RFC 8414 (OAuth 2.0 Authorization Server Metadata, `/.well-known/oauth-authorization-server`) — `[CITED: https://datatracker.ietf.org/doc/html/rfc8414]`
- RFC 7636 (PKCE) — `[CITED: existing implementation in src/client/oauth.rs:247-258]`
- `tempfile::NamedTempFile::persist` atomic-replace semantics on Windows + Linux — `[CITED: https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist]`
- `gh` CLI per-host credential file (`~/.config/gh/hosts.yml` YAML) — `[ASSUMED: widely-known industry convention]`
- `aws` CLI per-profile credential file (`~/.aws/credentials` INI) — `[ASSUMED: widely-known industry convention]`

### Tertiary (LOW confidence — flag for validation)

- `oauth2 = "5.0"` crate lacks DCR support — `[ASSUMED from general crate knowledge; cargo-pmcp does not import oauth2 for OAuth flows, just for pmcp-run integration]`. If planner wants to verify: `cargo doc -p oauth2 --open` and grep for "registration".

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — every dep verified in Cargo.toml, no new crates needed
- Architecture: **HIGH** — every integration point located and read in source
- Pitfalls: **HIGH** — pitfalls 1-3 directly reproducible from source; 4-8 are known RFC/ecosystem gotchas with clear mitigations
- RFC 7591 wire format: **HIGH** — confirmed both by RFC text and pmcp.run's reference parser
- Validation architecture: **HIGH** — all commands are runnable against existing infrastructure

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — stable domain, no fast-moving deps)
