# Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 13 new/modified
**Analogs found:** 12 / 13 (1 greenfield: `commands/auth_cmd/cache.rs` — no exact analog for multi-key token cache)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/client/oauth.rs` (modify) | SDK core | request-response (HTTP) | SELF (refactor in place) + `src/server/auth/providers/generic_oidc.rs:641-675` for DCR body shape | exact (self) |
| `cargo-pmcp/src/commands/auth_cmd/mod.rs` | subcommand dispatcher | request-response | `cargo-pmcp/src/commands/test/mod.rs` | exact |
| `cargo-pmcp/src/commands/auth_cmd/login.rs` | CLI handler | request-response | `cargo-pmcp/src/commands/test/conformance.rs` | exact |
| `cargo-pmcp/src/commands/auth_cmd/logout.rs` | CLI handler | file-I/O + CRUD | `cargo-pmcp/src/commands/test/conformance.rs` (shell) + `cache.rs` (local) | role-match |
| `cargo-pmcp/src/commands/auth_cmd/status.rs` | CLI handler | file-I/O + transform | `cargo-pmcp/src/commands/test/list.rs` | role-match |
| `cargo-pmcp/src/commands/auth_cmd/token.rs` | CLI handler | file-I/O + stdout | `cargo-pmcp/src/commands/test/download.rs` | role-match |
| `cargo-pmcp/src/commands/auth_cmd/refresh.rs` | CLI handler | request-response | `cargo-pmcp/src/commands/test/conformance.rs` | role-match |
| `cargo-pmcp/src/commands/auth_cmd/cache.rs` | CLI utility | file-I/O (atomic) | `src/client/oauth.rs:56-63,584-617` (single-server TokenCache) — pattern only; not 1:1 | role-match (greenfield-ish) |
| `cargo-pmcp/src/main.rs` (modify) | router | n/a | SELF (lines 69-107, 412-414) — `Test { command: TestCommand }` arm | exact |
| `cargo-pmcp/src/commands/pentest.rs` (modify) | CLI handler | request-response | `cargo-pmcp/src/commands/test/conformance.rs:42-56` (target pattern after migration) | exact |
| `tests/oauth_dcr_integration.rs` (new, SDK) | integration test | HTTP mock | `tests/handler_extensions_properties.rs` (proptest style) + `examples/c07_oidc_discovery.rs:17-52` (MockOidcServer builder) | role-match (first mockito consumer) |
| `cargo-pmcp/tests/auth_integration.rs` (new) | integration test | file-I/O + HTTP mock | `cargo-pmcp/tests/property_tests.rs` (proptest shape) | role-match |
| `examples/c08_oauth_dcr.rs` (new) | example | request-response | `examples/c07_oidc_discovery.rs` | exact |
| `CHANGELOG.md` (modify) | doc | n/a | SELF — `## [2.4.0] - 2026-04-17` entry | exact |
| `Cargo.toml` + `cargo-pmcp/Cargo.toml` (modify) | config | n/a | SELF | exact |

---

## Pattern Assignments

### `src/client/oauth.rs` (SDK, request-response) — refactor in place

**Analog:** `src/client/oauth.rs` itself (lines 38-108) + `src/server/auth/providers/generic_oidc.rs:641-675` for DCR wire execution + `src/client/oauth.rs:546-573` for the HTTP POST + JSON parse idiom.

**OAuthConfig shape today** (`src/client/oauth.rs:38-54`) — the struct to mutate per D-02:
```rust
/// OAuth configuration for CLI authentication flows.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub issuer: Option<String>,
    pub mcp_server_url: Option<String>,
    pub client_id: String,          // CHANGE → Option<String> (breaking-at-struct-literal)
    pub scopes: Vec<String>,
    pub cache_file: Option<PathBuf>,
    pub redirect_port: u16,
    // NEW fields per D-02:
    // pub client_name: Option<String>,
    // pub dcr_enabled: bool,     // default: true
}
```

**DCR types — RE-EXPORT, do not redefine** (`src/server/auth/provider.rs:302-382`):
```rust
// At the top of src/client/oauth.rs, add:
pub use crate::server::auth::provider::{DcrRequest, DcrResponse};
```
The existing `DcrRequest` at lines 302-351 is RFC 7591 complete: `redirect_uris: Vec<String>`, `client_name: Option<String>`, `client_uri`, `logo_uri`, `contacts`, `token_endpoint_auth_method: Option<String>`, `grant_types: Vec<String>`, `response_types: Vec<String>`, `scope`, `software_id`, `software_version`, and `#[serde(flatten)] extra: HashMap<String, Value>` for forward-compat. `DcrResponse` at lines 353-382 has `client_id: String`, optional `client_secret`, `client_secret_expires_at`, `registration_access_token`, `registration_client_uri`, `token_endpoint_auth_method`, and `#[serde(flatten)] extra`.

**DCR POST pattern to copy** (from server-side `generic_oidc.rs:641-675`, adapt for client-side):
```rust
// Source: src/server/auth/providers/generic_oidc.rs:642-675
async fn register_client(&self, request: DcrRequest) -> Result<DcrResponse> {
    let discovery = self.fetch_discovery().await?;
    let registration_endpoint = discovery.registration_endpoint.ok_or_else(|| {
        Error::protocol(ErrorCode::INVALID_REQUEST,
            format!("Provider '{}' does not support Dynamic Client Registration", self.display_name))
    })?;
    let response = self.http_client
        .post(&registration_endpoint)
        .json(&request)
        .send()
        .await
        .map_err(|e| Error::internal(format!("DCR request failed: {}", e)))?;
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error::protocol(ErrorCode::INVALID_REQUEST,
            format!("DCR failed: {}", error_text)));
    }
    response.json().await
        .map_err(|e| Error::internal(format!("Failed to parse DCR response: {}", e)))
}
```
Client-side adaptation: use `self.client` (the `reqwest::Client` on `OAuthHelper`) instead of `self.http_client`, use `Error::internal` (no `Error::protocol` available in client module) with the Phase 74 error-copy from RESEARCH §"Code Examples / DCR HTTP call (SDK)", wire fallback `client_name` = `"pmcp-sdk"` per D-04.

**Discovery integration point** (`src/client/oauth.rs:131-156`): `discover_metadata` already returns `OidcDiscoveryMetadata` whose `registration_endpoint: Option<String>` is exactly the D-03 trigger field. No change to the discovery helper — just inspect `metadata.registration_endpoint` after line 146 and branch into `do_dynamic_client_registration` when `dcr_enabled && client_id.is_none() && metadata.registration_endpoint.is_some()`.

**PKCE call site to rewire** (`src/client/oauth.rs:378-393`): the `TokenExchangeClient::exchange_code` call currently passes `&self.config.client_id` as a `&str`. After D-02 flip, this becomes the DCR-resolved `client_id` (either provided up front or obtained from `DcrResponse::client_id`). Keep the resolved id on a local variable in `authorization_code_flow` before the call. Same change in `refresh_token` (line 554) and `device_code_flow_internal` (lines 445, 493).

**default_cache_path pattern** (`src/client/oauth.rs:663-668`) — the anchor to mirror for the CLI's multi-server cache:
```rust
pub fn default_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".pmcp");
    path.push("oauth-tokens.json");
    path
}
```
Plan adds a sibling `default_cache_v2_path()` in cargo-pmcp-side `auth_cmd/cache.rs` (NOT in the SDK, per D-07 scope boundary — this is a CLI feature).

**Existing cache_token idiom (tokio::fs — DO NOT copy for new cache)** (`src/client/oauth.rs:584-617`): shows how the SDK writes the single-server `TokenCache` today via `tokio::fs::write` with no `chmod 600`, no atomic rename. This is the pattern to **replace** for the new multi-server cache — use `tempfile::NamedTempFile::persist` + Unix `0o600` instead (see Shared Patterns below).

---

### `cargo-pmcp/src/commands/auth_cmd/mod.rs` (subcommand dispatcher)

**Analog:** `cargo-pmcp/src/commands/test/mod.rs` (359 lines — the blueprint for subcommand modules).

**Imports + module declarations** (from test/mod.rs lines 1-25):
```rust
//! Manage OAuth credentials for MCP servers (login, logout, status, token, refresh).
//!
//! Per-server token cache at `~/.pmcp/oauth-cache.json` (schema_version: 1).
//! See CONTEXT.md Phase 74 D-06..D-16 for command semantics.

mod cache;     // NEW: TokenCacheV1, atomic read/write, URL normalization
mod login;
mod logout;
mod refresh;
mod status;
mod token;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;
```

**Subcommand enum shape** (lines 27-229 in test/mod.rs — follow exactly):
```rust
#[derive(Debug, Subcommand)]
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
Two style options exist in the tree: inline-fields variant (as `TestCommand` does) and `#[derive(Args)] struct + tuple-variant` (Phase 74 should use the **struct variant** — fewer match-arm fields to destructure, cleaner for the 5 subcommands; see `pentest.rs:19-64` for a single-command `#[derive(clap::Args)]` example).

**execute() dispatcher pattern** (from test/mod.rs lines 231-359):
```rust
impl AuthCommand {
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        match self {
            AuthCommand::Login(args) => runtime.block_on(login::execute(args, global_flags)),
            AuthCommand::Logout(args) => runtime.block_on(logout::execute(args, global_flags)),
            AuthCommand::Status(args) => runtime.block_on(status::execute(args, global_flags)),
            AuthCommand::Token(args) => runtime.block_on(token::execute(args, global_flags)),
            AuthCommand::Refresh(args) => runtime.block_on(refresh::execute(args, global_flags)),
        }
    }
}
```
`[CITED: cargo-pmcp/src/commands/test/mod.rs:231-359]`

---

### `cargo-pmcp/src/commands/auth_cmd/login.rs` (CLI handler, request-response)

**Analog:** `cargo-pmcp/src/commands/test/conformance.rs` (130 lines — the closest AuthFlags-consuming handler).

**Imports pattern** (from conformance.rs lines 1-13):
```rust
//! `cargo pmcp auth login` — PKCE + optional DCR, cache result.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};

use crate::commands::auth_cmd::cache::{normalize_cache_key, TokenCacheEntry, TokenCacheV1};
use crate::commands::GlobalFlags;
```

**LoginArgs struct** (mirror `pentest.rs:19-64`'s `#[derive(clap::Args)]` style):
```rust
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// URL of the MCP server to authenticate against
    pub url: String,

    /// Client name for Dynamic Client Registration (DCR, RFC 7591).
    /// Mutually exclusive with --oauth-client-id (D-19).
    #[arg(long, conflicts_with = "oauth_client_id")]
    pub client: Option<String>,

    /// Pre-registered OAuth client ID (skips DCR entirely — D-20 escape hatch)
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,

    /// OAuth issuer URL for OIDC discovery
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    pub oauth_issuer: Option<String>,

    /// OAuth scopes (comma-separated)
    #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    pub oauth_scopes: Option<Vec<String>>,

    /// Localhost port for the OAuth redirect callback
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}
```
`conflicts_with` pattern is lifted from `cargo-pmcp/src/commands/flags.rs:110` (`--api-key` vs `--oauth-client-id`) and enforces D-19.

**execute() body pattern** (from conformance.rs lines 15-90):
```rust
pub async fn execute(args: LoginArgs, global_flags: &GlobalFlags) -> Result<()> {
    // 1. Banner (copy style from conformance.rs:25-40)
    if global_flags.should_output() {
        println!();
        println!("{}", "OAuth Login".bright_cyan().bold());
        println!("  URL: {}", args.url.bright_white());
        // ...
    }

    // 2. Build OAuthConfig — D-17/D-19 branch on --client vs --oauth-client-id
    let client_name = args.client.clone()
        .or_else(|| if args.oauth_client_id.is_none() {
            Some("cargo-pmcp".to_string())   // D-04 default
        } else { None });

    let config = OAuthConfig {
        issuer: args.oauth_issuer,
        mcp_server_url: Some(args.url.clone()),
        client_id: args.oauth_client_id,   // None → SDK does DCR per D-03
        client_name,
        dcr_enabled: true,
        scopes: args.oauth_scopes.unwrap_or_else(|| vec!["openid".to_string()]),
        cache_file: None,                   // multi-server cache is CLI-managed, not SDK
        redirect_port: args.oauth_redirect_port,
    };

    // 3. Run PKCE (+ auto-DCR) via SDK
    let helper = OAuthHelper::new(config).context("OAuth setup failed")?;
    let access_token = helper.get_access_token().await.context("OAuth flow failed")?;

    // 4. Persist to multi-server cache — read-modify-write (Pitfall 4 mitigation)
    let cache_path = cache::default_multi_cache_path();
    let mut cache = TokenCacheV1::read(&cache_path)?;
    let key = normalize_cache_key(&args.url)?;
    cache.entries.insert(key.clone(), TokenCacheEntry { /* ... */ });
    cache.write_atomic(&cache_path)?;

    // 5. Success output — D-12: NEVER print the token
    if global_flags.should_output() {
        println!("Logged in to {} (issuer: {}, scopes: {}, expires in {})",
            key.bright_white(), /* ... */);
    }
    Ok(())
}
```
`[CITED: cargo-pmcp/src/commands/test/conformance.rs:16-90]` for signature shape and banner style.

---

### `cargo-pmcp/src/commands/auth_cmd/logout.rs` (CLI handler, file-I/O CRUD)

**Analog:** `cargo-pmcp/src/commands/test/conformance.rs` for the handler signature; cache mutation semantics are novel.

**Args struct + no-args-errors (D-09)**:
```rust
#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// URL of the MCP server to log out from (mutually exclusive with --all)
    #[arg(conflicts_with = "all")]
    pub url: Option<String>,

    /// Log out from every cached server
    #[arg(long)]
    pub all: bool,
}

pub async fn execute(args: LogoutArgs, global_flags: &GlobalFlags) -> Result<()> {
    if args.url.is_none() && !args.all {
        anyhow::bail!("specify a server URL or --all to log out of everything");  // D-09 exact copy
    }
    // ... read-modify-write cache via cache::write_atomic
}
```

---

### `cargo-pmcp/src/commands/auth_cmd/status.rs` (CLI handler, file-I/O transform → table)

**Analog:** `cargo-pmcp/src/commands/test/list.rs` (160 lines) for tabular output style; uses the existing `colored` crate (no new table crate per RESEARCH §"Alternatives Considered").

**Tabular output idiom** — use `format!` with column-width calculation + `colored::Colorize` for the header. No `tabled`/`comfy-table` dep. Header row columns per D-10: `URL | ISSUER | SCOPES | EXPIRES | REFRESHABLE`.

---

### `cargo-pmcp/src/commands/auth_cmd/token.rs` (CLI handler, file-I/O → stdout)

**Analog:** `cargo-pmcp/src/commands/test/download.rs` (96 lines) for the read-then-stdout flow; D-11 forces `println!("{}", token)` with ALL status/error via `eprintln!` / `anyhow::bail`.

**Behavior per D-11** (raw token to stdout, `gh auth token` ergonomics):
```rust
pub async fn execute(args: TokenArgs, _global_flags: &GlobalFlags) -> Result<()> {
    let cache_path = cache::default_multi_cache_path();
    let cache = TokenCacheV1::read(&cache_path)?;
    let key = cache::normalize_cache_key(&args.url)?;
    let entry = cache.entries.get(&key).ok_or_else(|| {
        anyhow::anyhow!("no cached token for {}. Run `cargo pmcp auth login {}` first.", key, key)
    })?;

    // D-15 transparent refresh at 60s-before-expiry
    let token = if cache::is_near_expiry(entry, 60) {
        cache::refresh_and_persist(&cache_path, &key, entry).await?
    } else {
        entry.access_token.clone()
    };

    // D-11: raw token, newline, nothing else to stdout
    println!("{}", token);
    Ok(())
}
```
`[CITED: 74-RESEARCH.md:638-658]` (research's reference implementation).

---

### `cargo-pmcp/src/commands/auth_cmd/refresh.rs` (CLI handler, request-response)

**Analog:** `cargo-pmcp/src/commands/test/conformance.rs` for signature. D-16 force-refresh errors when `entry.refresh_token.is_none()` — actionable copy: `"no refresh_token cached for {url} — run \`cargo pmcp auth login {url}\` to re-authenticate"` (aligns with Pitfall 5 mitigation in RESEARCH.md).

---

### `cargo-pmcp/src/commands/auth_cmd/cache.rs` (CLI utility, file-I/O atomic) — GREENFIELD

**Analog:** partial match — `src/client/oauth.rs:56-63, 584-617` shows the legacy single-server `TokenCache` shape; this file generalizes it to multi-server with atomic writes and `0o600` permissions.

**TokenCacheV1 schema** (per D-07, RESEARCH §"TokenCacheV1 schema"):
```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheV1 {
    pub schema_version: u32,
    pub entries: BTreeMap<String, TokenCacheEntry>,  // BTreeMap for deterministic JSON
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCacheEntry {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub client_id: String,
}

impl TokenCacheV1 {
    pub const CURRENT_VERSION: u32 = 1;
    pub fn empty() -> Self {
        Self { schema_version: Self::CURRENT_VERSION, entries: BTreeMap::new() }
    }
}
```
`[CITED: 74-RESEARCH.md:540-588]`

**default_multi_cache_path** — mirror `src/client/oauth.rs:663-668`:
```rust
pub fn default_multi_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".pmcp");
    path.push("oauth-cache.json");   // NEW filename per D-07
    path
}
```

**normalize_cache_key** — mirror `src/client/oauth.rs:113-128` (`OAuthHelper::extract_base_url`):
```rust
pub fn normalize_cache_key(mcp_server_url: &str) -> Result<String> {
    use url::Url;
    let parsed = Url::parse(mcp_server_url)
        .map_err(|e| anyhow::anyhow!("Invalid MCP server URL: {e}"))?;
    let host = parsed.host_str()
        .ok_or_else(|| anyhow::anyhow!("URL missing host"))?
        .to_ascii_lowercase();
    let mut base = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        let is_default = (parsed.scheme() == "https" && port == 443)
            || (parsed.scheme() == "http"  && port == 80);
        if !is_default {
            base.push_str(&format!(":{}", port));
        }
    }
    Ok(base)
}
```
`[CITED: src/client/oauth.rs:113-128]` — DO NOT copy verbatim (returns `Error` from pmcp); adapt to `anyhow::Result`.

**Atomic write idiom** — see Shared Patterns §"Atomic cache write" below.

---

### `cargo-pmcp/src/main.rs` (router, modify)

**Analog:** SELF — `Commands::Test { command: TestCommand }` at lines 104-107, dispatch at lines 412-414.

**Enum variant to add** (mirror lines 96-107):
```rust
/// Manage OAuth credentials for MCP servers
///
/// Log in, log out, inspect status, and fetch tokens for OAuth-protected servers.
#[command(after_long_help = "Examples:
  cargo pmcp auth login https://mcp.pmcp.run
  cargo pmcp auth login https://mcp.pmcp.run --client claude-desktop
  cargo pmcp auth status
  cargo pmcp auth token https://mcp.pmcp.run")]
Auth {
    #[command(subcommand)]
    command: commands::auth_cmd::AuthCommand,
},
```
`[CITED: cargo-pmcp/src/main.rs:96-107]`

**Match arm to add** (mirror line 412-414):
```rust
Commands::Auth { command } => {
    command.execute(global_flags)?;
},
```
`[CITED: cargo-pmcp/src/main.rs:412-414]`

**Module registration** — add `pub mod auth_cmd;` to `cargo-pmcp/src/commands/mod.rs` (alphabetically, after the existing `pub mod auth;` at line 3, named `auth_cmd` to disambiguate from the existing `auth.rs` per CONTEXT.md "Established Patterns").

---

### `cargo-pmcp/src/commands/pentest.rs` (modify, D-21 flag migration)

**Analog:** `cargo-pmcp/src/commands/test/conformance.rs:42-56` — the target pattern after migration.

**Current shape to remove** (`pentest.rs:61-64`):
```rust
/// API key for authentication (sent as Bearer token)
#[arg(long, env = "MCP_API_KEY")]
pub api_key: Option<String>,
```
And the middleware construction at lines 157-165 using `cmd.api_key.as_deref()`.

**Replace with** (mirror conformance.rs:11-12, 42-56):
```rust
// Add to imports:
use crate::commands::auth;
use crate::commands::flags::AuthFlags;

// In PentestCommand struct (replace lines 61-64):
#[command(flatten)]
pub auth_flags: AuthFlags,

// In execute_pentest() (replace lines 157-165):
let auth_method = cmd.auth_flags.resolve();
let middleware = auth::resolve_auth_middleware(&cmd.url, &auth_method).await?;

let mut tester = mcp_tester::ServerTester::new(
    &cmd.url,
    Duration::from_secs(cmd.timeout),
    false,
    None,              // api_key — auth handled via middleware for consistency
    cmd.transport.as_deref(),
    middleware,        // CHANGED: was None
)
.context("Failed to create server tester")?;
```
`[CITED: cargo-pmcp/src/commands/test/conformance.rs:42-56]` (the exact pattern to mirror). Note this also requires changing `execute_pentest` from sync-wrapping to an `async` flow since `resolve_auth_middleware` is async — follow the test/conformance.rs approach where `execute` is `async` and the outer sync wrapper does `Runtime::new()?.block_on(...)` (already the case in `PentestCommand::execute` at lines 66-72).

---

### `tests/oauth_dcr_integration.rs` (NEW, pmcp crate, integration)

**Analog:** partial — `tests/handler_extensions_properties.rs` for the plain-Rust test-file shape; `examples/c07_oidc_discovery.rs:17-52` for the `OidcDiscoveryMetadata` builder. This is the FIRST `mockito` consumer in the tree per RESEARCH.md.

**Mockito is already in pmcp dev-deps** (`Cargo.toml:140` per research — verified). Shape:
```rust
//! Integration tests for Dynamic Client Registration (RFC 7591) in OAuthHelper.

use mockito::{Matcher, Server};
use pmcp::client::oauth::{DcrResponse, OAuthConfig, OAuthHelper};

#[tokio::test]
async fn dcr_fires_when_eligible() {
    let mut server = Server::new_async().await;

    // Mock discovery
    let _m1 = server.mock("GET", "/.well-known/openid-configuration")
        .with_body(serde_json::json!({
            "issuer": server.url(),
            "authorization_endpoint": format!("{}/authorize", server.url()),
            "token_endpoint": format!("{}/token", server.url()),
            "registration_endpoint": format!("{}/register", server.url()),
            /* ... */
        }).to_string())
        .create_async().await;

    // Mock DCR endpoint
    let _m2 = server.mock("POST", "/register")
        .match_header("content-type", Matcher::Regex("application/json.*".into()))
        .with_body(serde_json::json!({"client_id": "dcr-issued-id"}).to_string())
        .create_async().await;

    // ... run OAuthHelper flow, assert DCR body matches D-05 shape
}
```

---

### `cargo-pmcp/tests/auth_integration.rs` (NEW, integration)

**Analog:** `cargo-pmcp/tests/property_tests.rs` (6KB — existing test file pattern in cargo-pmcp) for the file-header/imports; test bodies are novel.

**Imports style** (mirror property_tests.rs lines 1-12):
```rust
//! End-to-end tests for the `cargo pmcp auth` subcommand group.
//!
//! Covers: login happy path, logout no-args error, token stdout, precedence
//! ordering (flag > env > cache), transparent refresh at 60s-before-expiry.

use cargo_pmcp::commands::auth_cmd::cache::{TokenCacheV1, TokenCacheEntry};
use tempfile::TempDir;
```
Note: to import `cargo_pmcp::commands::auth_cmd::*` from an integration test, the subcommand module must be `pub mod auth_cmd;` in `src/commands/mod.rs` AND `cache` / types must be `pub`. The existing `cargo-pmcp/src/lib.rs` re-exports `commands` per the `[lib] name = "cargo_pmcp"` entry in Cargo.toml (lines 13-15).

**27-row Validation Matrix → Test Map** in `74-RESEARCH.md` lines 742-771 enumerates the exact test names to generate here — planner should reference that matrix 1:1.

---

### `examples/c08_oauth_dcr.rs` (NEW, SDK example)

**Analog:** `examples/c07_oidc_discovery.rs` (331 lines — the immediately preceding OAuth-related example, same `c0X_` numbering scheme).

**File-header + imports style** (mirror c07 lines 1-16):
```rust
//! Example: Dynamic Client Registration (RFC 7591) with OAuthHelper.
//!
//! Demonstrates how a library user can build an OAuth client that
//! auto-registers itself with any server advertising a `registration_endpoint`
//! via OIDC discovery, without hardcoding a client_id.
//!
//! Run with:
//!   cargo run --example c08_oauth_dcr --features oauth

use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
```

**Numbering convention**: examples are numbered `c01_..c07_` (client), `s01_..s08_` (server), `m01_..m08_` (middleware). New example uses `c08_` (next free client slot). NOTE: `examples/25-oauth-basic/` is the legacy multi-file style being phased out — use the single-file `c0X_` style per the pattern.

**Cargo.toml exclude review**: the root `Cargo.toml:16-40` lists excluded examples (mcp-apps-*, wasm-*, 25-oauth-basic, etc.). Single-file `cXX_` examples are NOT in the exclude list — no Cargo.toml change needed for the new example to ship with the crate.

---

### `CHANGELOG.md` (modify)

**Analog:** SELF — the `## [2.4.0] - 2026-04-17` entry near the top (lines 8-30).

**Entry shape to follow**:
```markdown
## [2.5.0] - 2026-04-??

### Added
- **pmcp 2.5.0 — Dynamic Client Registration (RFC 7591) support in OAuthHelper** (Phase 74).
  `OAuthConfig` gains `client_name: Option<String>` and `dcr_enabled: bool` (default `true`).
  When `dcr_enabled && client_id.is_none() && discovery.registration_endpoint.is_some()`,
  `OAuthHelper` auto-registers with the server's DCR endpoint before PKCE, eliminating
  the need to pre-provision client IDs against OAuth servers that support RFC 7591.
  Public `DcrRequest` / `DcrResponse` types re-exported from `pmcp::client::oauth` for
  library consumers building custom flows.
- **cargo-pmcp 0.9.0 — `cargo pmcp auth` command group** (Phase 74). Five subcommands
  (`login`, `logout`, `status`, `token`, `refresh`) manage per-server OAuth tokens in a
  new `~/.pmcp/oauth-cache.json` (schema_version: 1). `--client <name>` flag on
  `auth login` drives the SDK's new DCR path. `auth token <url>` prints the raw access
  token to stdout (gh-style). All server-connecting commands (`test/*`, `connect`,
  `preview`, `schema`, `dev`, `loadtest/run`, `pentest`) now consult the cache as the
  lowest-precedence auth source after explicit flags and env vars.

### Changed
- **BREAKING (minor-within-v2.x):** `OAuthConfig::client_id` type changed
  `String → Option<String>`. Existing callers must wrap `"x".to_string()` as
  `Some("x".to_string())`. Rationale: enable DCR auto-trigger when `client_id.is_none()`.
  Per the v2.x breaking-change window policy in MEMORY.md, this ships as a minor bump.
- **cargo-pmcp `pentest`**: migrated from local `--api-key` flag to shared `AuthFlags`.
  `--api-key` continues to work identically; `--oauth-client-id` and friends are now
  also accepted for OAuth-protected targets.
```
`[CITED: CHANGELOG.md:8-30]` for the entry shape.

---

### `Cargo.toml` + `cargo-pmcp/Cargo.toml` (modify)

**Analog:** SELF.

- Root `Cargo.toml:3` — bump `version = "2.4.0"` → `"2.5.0"`.
- `cargo-pmcp/Cargo.toml:3` — bump `version = "0.8.1"` → `"0.9.0"`.
- `cargo-pmcp/Cargo.toml:41` — bump `pmcp = { version = "2.2.0", ... }` → `"2.5.0"` (the version pin is already stale vs `2.4.0` — Phase 74 catches it up).
- `cargo-pmcp/Cargo.toml:76` — promote `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]` (per RESEARCH §"Wave 0 Gaps"). SDK-side `Cargo.toml:130` already has `tempfile = "3.19"` as a dev-dep only; no change needed there since the atomic-write code lives on the CLI side.

---

## Shared Patterns

### Pattern: AuthFlags → middleware via `resolve_auth_middleware`
**Source:** `cargo-pmcp/src/commands/auth.rs:52-91`
**Apply to:** `pentest.rs` migration (D-21); login/refresh in `auth_cmd/*` consume the SDK's `OAuthHelper` directly, not this middleware path.

```rust
// Exact call pattern all server-connecting commands use today:
let auth_method = auth_flags.resolve();
let middleware = auth::resolve_auth_middleware(&url, &auth_method).await?;
// then pass `middleware` to ServerTester::new, LoadTestEngine, HttpClient, etc.
```

`auth.rs` itself grows a new responsibility in Phase 74: on the `AuthMethod::None` arm (line 57), before returning `Ok(None)`, consult the new cache for a matching entry and return a `BearerToken` middleware chain when found (D-13/D-15 precedence). The existing `AuthFlags::resolve()` in `flags.rs:140-157` is UNCHANGED — the cache fallback is added one layer up in `resolve_auth_middleware` / `resolve_auth_header`.

---

### Pattern: clap `conflicts_with` mutual exclusion
**Source:** `cargo-pmcp/src/commands/flags.rs:110` (`--api-key` vs `--oauth-client-id`)
**Apply to:** `auth_cmd/login.rs` for `--client` vs `--oauth-client-id` (D-19); `auth_cmd/logout.rs` for `<url>` vs `--all` (D-09).

```rust
#[arg(long, env = "MCP_API_KEY", conflicts_with = "oauth_client_id")]
pub api_key: Option<String>,
```
Clap attribute name-matches the *field name* (snake_case), not the flag name — so `conflicts_with = "oauth_client_id"` targets the `pub oauth_client_id: Option<String>` field at line 115.

**Test pattern for the conflict** (`flags.rs:282-295`):
```rust
#[test]
fn clap_rejects_api_key_with_oauth_client_id() {
    let result = TestCli::try_parse_from([
        "test-cli", "--api-key", "my-key", "--oauth-client-id", "my-client",
    ]);
    assert!(result.is_err(), "Expected clap parse error for conflicting flags");
}
```
Copy this test verbatim for login's `--client` vs `--oauth-client-id` conflict and logout's `<url>` vs `--all` conflict.

---

### Pattern: Atomic cache write (`tempfile::NamedTempFile::persist`)
**Source:** new pattern — `tempfile` usage today in the tree is only `NamedTempFile::new()` (`cargo-pmcp/src/loadtest/config.rs:563`); no existing consumer uses `.persist(path)`. Design per RESEARCH §"Pattern 2: Atomic Cache Write via tempfile" lines 287-312.
**Apply to:** `auth_cmd/cache.rs::write_atomic` (every mutation — login, refresh, logout).

```rust
use tempfile::NamedTempFile;
use std::io::Write;
use std::path::Path;

pub fn write_atomic(path: &Path, cache: &TokenCacheV1) -> anyhow::Result<()> {
    let parent = path.parent()
        .ok_or_else(|| anyhow::anyhow!("cache path has no parent: {:?}", path))?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut tmp = NamedTempFile::new_in(parent)?;  // same-fs → rename is atomic
    let json = serde_json::to_vec_pretty(cache)?;
    tmp.write_all(&json)?;
    tmp.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file().set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    tmp.persist(path)?;
    Ok(())
}
```
`[CITED: 74-RESEARCH.md:295-311]` — DO NOT hand-roll `flock` or `O_EXCL` per anti-pattern list.

---

### Pattern: Mockito-driven integration tests
**Source:** First consumer — pattern derives from the `mockito 1.5.0` docs (dev-dep at `Cargo.toml:140`). Existing analog for mock-server-BUILDER style is `examples/c07_oidc_discovery.rs:17-52` (but that example builds a pure in-memory struct, not an HTTP mock).
**Apply to:** `tests/oauth_dcr_integration.rs`; `cargo-pmcp/tests/auth_integration.rs`.

Key wire: mockito spins up an HTTP server on a random port; `server.url()` gives the base URL to pass as the "MCP server URL" in `OAuthConfig::mcp_server_url`. Each endpoint (discovery, DCR, token, refresh) gets a `server.mock(...)` handler with body assertions (`match_body(Matcher::JsonString(expected_json))`).

---

### Pattern: PKCE `code_verifier` / `code_challenge` — DO NOT hand-roll
**Source:** `src/client/oauth.rs:247-258` (existing `generate_code_verifier` / `generate_code_challenge` on `OAuthHelper`).
**Apply to:** DCR path — no new PKCE needed; DCR happens BEFORE PKCE in the flow. Just keep the existing PKCE call site untouched.

---

### Pattern: Error message copy conventions
**Source:** `src/client/oauth.rs:148-154` (discovery failure) + `src/client/oauth.rs:417-425` (device flow unsupported). Both use the structured `"{problem}.\n\nPlease {action}, or {alternative}"` shape.
**Apply to:** All new error sites in Phase 74 — match this multi-line actionable-advice style. Exact copy for DCR-not-supported per D-03: `"server does not support DCR — pass a pre-registered client_id"`.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `cargo-pmcp/src/commands/auth_cmd/cache.rs` | utility | file-I/O (atomic multi-key) | No existing multi-key token cache exists. The SDK's `TokenCache` (`src/client/oauth.rs:56-63`) is single-server only. Design per RESEARCH §"TokenCacheV1 schema" and §"Pattern 2: Atomic Cache Write via tempfile". |
| `tests/oauth_dcr_integration.rs` | integration test | HTTP mock | First `mockito` consumer in the tree (per RESEARCH `[VERIFIED: Cargo.toml:140]` — already a dev-dep, never used). No existing analog for mockito-based tests; pattern derives from mockito's own docs. |

## Metadata

**Analog search scope:**
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/` (all subcommand modules)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/` (oauth, auth, http_middleware, oauth_middleware)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/auth/provider.rs` (DCR types)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/auth/providers/generic_oidc.rs` (DCR POST idiom)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/` (c07_oidc_discovery, examples numbering)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/` (integration test style)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/` (property_tests shape)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/fuzz/fuzz_targets/` (fuzz target shape for future `dcr_response_parser.rs`)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md` (entry style)

**Files scanned:** ~25 (targeted; no re-reads).
**Pattern extraction date:** 2026-04-21.
