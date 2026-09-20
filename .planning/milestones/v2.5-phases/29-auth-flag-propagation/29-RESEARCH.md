# Phase 29: Auth Flag Propagation - Research

**Researched:** 2026-03-12
**Domain:** CLI flag architecture, OAuth/API-key auth propagation in Rust clap derive
**Confidence:** HIGH

## Summary

This phase adds shared `AuthFlags` struct to 7 server-connecting commands (test check, test run, test generate, test apps, preview, schema export, connect) and migrates loadtest's 6 inline auth fields to the same struct. The existing codebase already has a complete OAuth implementation (`OAuthHelper`, `OAuthClientMiddleware`, `BearerToken`) and an established pattern for shared flag structs (`ServerFlags`, `GlobalFlags`). The loadtest `run.rs` handler contains a battle-tested `resolve_auth_middleware()` function that can be extracted to work with the new `AuthFlags` struct.

The core challenge is mechanical: define `AuthFlags`, flatten it into 8 command variants (7 new + loadtest), and wire each handler to pass auth through to `ServerTester::new()` or `OAuthHelper`. The `schema export` and `preview` commands use raw `reqwest::Client` rather than `ServerTester`, so they need different auth wiring (adding `Authorization` headers directly or extending `McpProxy`/`PreviewConfig`).

**Primary recommendation:** Define `AuthFlags` in `flags.rs` with `resolve()` returning `AuthMethod` enum, extract `resolve_auth_middleware()` from loadtest `run.rs` into a shared location, then flatten `AuthFlags` into all 8 command variants and wire handlers uniformly.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Auth flags added to 7 server-connecting commands: test check, test run, test generate, test apps, preview, schema export, connect
- pmcp.run commands (test upload/download/list, deploy, secret, landing) stay auth-free
- Validate command stays auth-free (purely local)
- Loadtest `run` migrates its 6 inline auth fields to shared `AuthFlags` struct via `#[command(flatten)]`
- Loadtest handler signature changes to accept `&AuthFlags` directly (not destructured individual fields)
- Clean migration -- no backward compat aliases needed
- `AuthFlags` lives in `cargo-pmcp/src/commands/flags.rs` alongside `ServerFlags` and `FormatValue`
- Separate `#[command(flatten)]` from `ServerFlags` -- not embedded inside it
- Fields: `api_key: Option<String>`, `oauth_client_id: Option<String>`, `oauth_issuer: Option<String>`, `oauth_scopes: Option<Vec<String>>`, `oauth_no_cache: bool`, `oauth_redirect_port: u16`
- Same env vars as Phase 26: `MCP_API_KEY`, `MCP_OAUTH_CLIENT_ID`, `MCP_OAUTH_ISSUER`, `MCP_OAUTH_SCOPES`, `MCP_OAUTH_REDIRECT_PORT`
- `--api-key` and `--oauth-client-id` mutually exclusive via clap `conflicts_with`
- `AuthFlags::resolve()` returns `AuthMethod` enum: `None`, `ApiKey(String)`, `OAuth { ... }`
- All 7 server-connecting handlers use uniform pattern: `execute(..., auth_flags: &AuthFlags, global_flags: &GlobalFlags)`
- Conflict handling at clap parse level -- no custom runtime validation

### Claude's Discretion
- Exact `AuthMethod` enum field types and naming
- Whether `resolve()` returns `Result<AuthMethod>` or `AuthMethod`
- Internal implementation of how each handler wires auth into `ServerTester::new()` or `OAuthHelper`
- Test strategy for AuthFlags parsing and resolution

### Deferred Ideas (OUT OF SCOPE)
- Validate command may be redundant with test -- consider removing or merging in a future phase
- Auto-discovery of OAuth from server's `.well-known` endpoint
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AUTH-01 | `cargo pmcp test check` accepts `--api-key` and OAuth flags | AuthFlags flatten into Check variant; handler wires to ServerTester::new() api_key and middleware params |
| AUTH-02 | `cargo pmcp test run` accepts `--api-key` and OAuth flags | AuthFlags flatten into Run variant; handler needs auth wiring into run_scenario_with_transport or direct ServerTester usage |
| AUTH-03 | `cargo pmcp test generate` accepts `--api-key` and OAuth flags | AuthFlags flatten into Generate variant; handler needs auth wiring into generate_scenarios_with_transport or direct ServerTester usage |
| AUTH-04 | `cargo pmcp preview` accepts `--api-key` and OAuth flags | AuthFlags flatten into Preview variant; McpProxy needs auth header support or PreviewConfig extension |
| AUTH-05 | `cargo pmcp schema export` accepts `--api-key` and OAuth flags | AuthFlags flatten into SchemaCommand::Export; raw reqwest calls need Authorization header |
| AUTH-06 | `cargo pmcp connect` accepts `--api-key` and OAuth flags | AuthFlags flatten into Connect variant; connect is config-generation so auth flags stored in client config |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x (derive) | CLI arg parsing with `#[command(flatten)]`, `conflicts_with` | Already used throughout; Args derive + flatten is the idiomatic pattern |
| pmcp::client::oauth | workspace | OAuthHelper, OAuthConfig, default_cache_path | Complete OAuth PKCE + device code + caching implementation already in crate |
| pmcp::client::oauth_middleware | workspace | BearerToken, OAuthClientMiddleware | HTTP middleware for transparent Authorization header injection |
| pmcp::client::http_middleware | workspace | HttpMiddlewareChain | Composable middleware chain accepted by ServerTester and LoadTestEngine |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| mcp_tester::ServerTester | workspace | MCP server testing client | Already accepts `api_key: Option<&str>` and `http_middleware_chain` params |
| reqwest | 0.12.x | HTTP client for schema export / preview proxy | Schema export and McpProxy use raw reqwest; auth headers added to request builder |

## Architecture Patterns

### Recommended Project Structure
```
cargo-pmcp/src/commands/
  flags.rs           # AuthFlags struct (NEW), ServerFlags, FormatValue
  mod.rs             # GlobalFlags (unchanged)
  auth.rs            # resolve_auth_middleware() extracted from loadtest/run.rs (NEW)
  test/
    mod.rs           # AuthFlags flattened into Check, Run, Generate, Apps variants
    check.rs         # Handler receives &AuthFlags, wires to ServerTester
    run.rs           # Handler receives &AuthFlags, wires to ServerTester
    generate.rs      # Handler receives &AuthFlags, wires to ServerTester
    apps.rs          # Handler receives &AuthFlags, wires to ServerTester
  loadtest/
    mod.rs           # Inline auth fields replaced with #[command(flatten)] AuthFlags
    run.rs           # Uses shared resolve_auth_middleware() instead of local copy
  preview.rs         # Handler receives &AuthFlags, wires to McpProxy or reqwest
  schema.rs          # SchemaCommand::Export receives AuthFlags, wires to reqwest
  connect.rs         # Handler receives &AuthFlags, stores in client config
```

### Pattern 1: AuthFlags Struct with Resolve
**What:** A clap `Args` derive struct holding all auth-related CLI flags, with a `resolve()` method that returns a typed enum.
**When to use:** Every command that connects to an MCP server.
**Example:**
```rust
// In flags.rs
#[derive(Debug, Args)]
pub struct AuthFlags {
    /// API key for authentication (sent as Bearer token)
    #[arg(long, env = "MCP_API_KEY", conflicts_with = "oauth_client_id")]
    pub api_key: Option<String>,

    /// OAuth client ID (triggers OAuth flow)
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,

    /// OAuth issuer URL
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    pub oauth_issuer: Option<String>,

    /// OAuth scopes (comma-separated)
    #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    pub oauth_scopes: Option<Vec<String>>,

    /// Disable OAuth token caching
    #[arg(long)]
    pub oauth_no_cache: bool,

    /// OAuth redirect port for localhost callback
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}

pub enum AuthMethod {
    None,
    ApiKey(String),
    OAuth {
        client_id: String,
        issuer: Option<String>,
        scopes: Vec<String>,
        no_cache: bool,
        redirect_port: u16,
    },
}

impl AuthFlags {
    pub fn resolve(&self) -> AuthMethod {
        if let Some(ref key) = self.api_key {
            return AuthMethod::ApiKey(key.clone());
        }
        if let Some(ref client_id) = self.oauth_client_id {
            return AuthMethod::OAuth {
                client_id: client_id.clone(),
                issuer: self.oauth_issuer.clone(),
                scopes: self.oauth_scopes.clone().unwrap_or_else(|| vec!["openid".to_string()]),
                no_cache: self.oauth_no_cache,
                redirect_port: self.oauth_redirect_port,
            };
        }
        AuthMethod::None
    }
}
```

### Pattern 2: Shared Auth Middleware Resolution
**What:** Extract `resolve_auth_middleware()` from `loadtest/run.rs` into a shared module so all handlers can convert `AuthMethod` into `Option<Arc<HttpMiddlewareChain>>`.
**When to use:** Any handler that needs to pass auth to `ServerTester::new()` or `LoadTestEngine`.
**Example:**
```rust
// In commands/auth.rs (new file)
use std::sync::Arc;
use anyhow::Result;
use pmcp::client::http_middleware::HttpMiddlewareChain;

use super::flags::AuthMethod;

/// Convert AuthMethod into HTTP middleware chain for ServerTester/LoadTestEngine.
pub async fn resolve_auth_middleware(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    match auth_method {
        AuthMethod::None => Ok(None),
        AuthMethod::ApiKey(key) => {
            use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
            let bearer_token = BearerToken::new(key.clone());
            let middleware = OAuthClientMiddleware::new(bearer_token);
            let mut chain = HttpMiddlewareChain::new();
            chain.add(Arc::new(middleware));
            Ok(Some(Arc::new(chain)))
        }
        AuthMethod::OAuth { client_id, issuer, scopes, no_cache, redirect_port } => {
            use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};
            let cache_file = if *no_cache { None } else { Some(default_cache_path()) };
            let config = OAuthConfig {
                issuer: issuer.clone(),
                mcp_server_url: Some(mcp_server_url.to_string()),
                client_id: client_id.clone(),
                scopes: scopes.clone(),
                cache_file,
                redirect_port: *redirect_port,
            };
            let helper = OAuthHelper::new(config)
                .map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))?;
            let chain = helper.create_middleware_chain().await
                .map_err(|e| anyhow::anyhow!("OAuth authentication failed: {e}"))?;
            Ok(Some(chain))
        }
    }
}
```

### Pattern 3: Flatten into Command Enum Variants
**What:** Add `#[command(flatten)] auth_flags: AuthFlags` to each command variant that connects to an MCP server.
**When to use:** test check/run/generate/apps, preview, schema export, connect, loadtest run.
**Example:**
```rust
// In test/mod.rs
Check {
    url: String,
    #[arg(long)]
    transport: Option<String>,
    #[arg(long, default_value = "30")]
    timeout: u64,
    #[command(flatten)]
    auth_flags: AuthFlags,  // NEW
},
```

### Pattern 4: Handler Auth Wiring for ServerTester
**What:** Convert `AuthFlags` to middleware chain, pass to `ServerTester::new()`.
**When to use:** test check, test apps (which create ServerTester directly).
**Example:**
```rust
// In check.rs
pub async fn execute(
    url: String,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let auth_method = auth_flags.resolve();
    let middleware = crate::commands::auth::resolve_auth_middleware(&url, &auth_method).await?;

    let api_key_ref = match &auth_method {
        AuthMethod::ApiKey(key) => Some(key.as_str()),
        _ => None,
    };

    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false,
        api_key_ref,
        transport.as_deref(),
        middleware,
    )?;
    // ... rest of handler
}
```

### Anti-Patterns to Avoid
- **Duplicating auth fields per command:** Never add `api_key`, `oauth_client_id`, etc. as individual fields on each command variant. Always flatten `AuthFlags`.
- **Duplicating `resolve_auth_middleware()`:** The loadtest handler has a working copy. Extract it, don't copy it to 7 more places.
- **Passing `&AuthFlags` through to library functions:** Library functions (`run_scenario_with_transport`, `generate_scenarios_with_transport`) should receive the resolved middleware chain, not raw CLI flags.
- **Runtime mutual exclusion checks:** clap's `conflicts_with` handles this at parse time. Don't add manual `if api_key.is_some() && oauth_client_id.is_some()` checks.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OAuth PKCE flow | Custom OAuth implementation | `pmcp::client::oauth::OAuthHelper` | Full PKCE + device code + token caching + refresh already implemented |
| Bearer token injection | Manual header setting per request | `OAuthClientMiddleware` + `HttpMiddlewareChain` | Transparent injection, token refresh, expiry checks built in |
| CLI argument conflicts | Runtime if/else validation | clap `conflicts_with` attribute | Parse-time error with auto-generated message |
| Token caching | Custom file-based cache | `default_cache_path()` + OAuthConfig `cache_file` | Cache path, format, refresh logic already implemented |

## Common Pitfalls

### Pitfall 1: ServerTester Takes Both api_key AND middleware
**What goes wrong:** `ServerTester::new()` accepts both `api_key: Option<&str>` and `http_middleware_chain: Option<Arc<HttpMiddlewareChain>>`. For API key auth, the loadtest handler creates middleware (BearerToken via OAuthClientMiddleware), but other handlers might try to pass the API key as the `api_key` parameter. This would cause double auth headers or inconsistent behavior.
**Why it happens:** The `api_key` param on `ServerTester::new()` is a simpler path that doesn't use middleware.
**How to avoid:** Decide one approach per auth method. For API keys: either pass as `api_key` param OR as middleware, not both. The middleware approach is more consistent since OAuth always uses middleware. Recommendation: use middleware for both (matching loadtest pattern), pass `None` for the `api_key` param.
**Warning signs:** Tests work with `--api-key` but headers appear twice in verbose output.

### Pitfall 2: test run and test generate Use Library Functions That Don't Accept Auth
**What goes wrong:** `run_scenario_with_transport()` and `generate_scenarios_with_transport()` create their own `ServerTester` internally with `None` for both `api_key` and `http_middleware_chain`. Adding auth flags to the CLI won't actually authenticate.
**Why it happens:** These convenience functions were designed for unauthenticated scenarios.
**How to avoid:** The handlers need to create `ServerTester` directly (like check.rs and apps.rs do) instead of using the convenience functions, OR the convenience functions need to be extended with auth parameters.
**Warning signs:** Auth flags are accepted but the server rejects requests as unauthenticated.

### Pitfall 3: schema export Uses Raw reqwest Without Middleware
**What goes wrong:** `schema.rs::export()` builds its own `reqwest::Client` and calls `send_mcp_request()` directly. There's no middleware chain support.
**Why it happens:** Schema export was built as a standalone tool before the middleware system existed.
**How to avoid:** For API key auth, add `Authorization: Bearer {key}` header to the `send_mcp_request()` function or the reqwest client builder. For OAuth, acquire the token via `OAuthHelper::get_access_token()` first, then add the header. Don't try to retrofit the full middleware chain into raw reqwest -- just extract the token and set the header.
**Warning signs:** Schema export with `--api-key` still gets 401 responses.

### Pitfall 4: preview Command's McpProxy Has No Auth Support
**What goes wrong:** `McpProxy::new()` creates a plain `reqwest::Client` with no auth headers. Adding `--api-key` to the preview command won't actually authenticate proxy requests to the MCP server.
**Why it happens:** McpProxy was designed for local development servers that don't need auth.
**How to avoid:** Either extend `McpProxy::new()` to accept an optional `Authorization` header value, or build the reqwest client with a default header. The simplest approach: add an optional `auth_header: Option<String>` to McpProxy and apply it in `mcp_post()`.
**Warning signs:** Preview with `--api-key` connects but gets 401 when trying to list tools.

### Pitfall 5: connect Command Is Config-Generation, Not Connection
**What goes wrong:** The connect command doesn't actually connect to the MCP server -- it generates config for Claude Code, Cursor, or Inspector. Auth flags need to be stored in the generated config, not used at runtime.
**Why it happens:** Assumption that "connect" means making a network request.
**How to avoid:** For connect, auth flags should be included in the generated client config (e.g., `claude mcp add --header "Authorization: Bearer {key}"`). Check what each client supports for auth configuration.
**Warning signs:** Auth flags are parsed but silently ignored because the handler doesn't use them.

### Pitfall 6: clap `default_value` on oauth_redirect_port Is Always Present
**What goes wrong:** `oauth_redirect_port` has `default_value = "8080"`, meaning it's always `8080` even when no OAuth is configured. This is fine because `resolve()` only includes it in the `OAuth` variant, but it could confuse users in `--help` output.
**Why it happens:** clap default values are always shown in help text.
**How to avoid:** This is acceptable -- the field is only meaningful when `--oauth-client-id` is provided, and the help text context makes this clear.

## Code Examples

### Example 1: Current loadtest auth wiring (source of truth)
```rust
// Source: cargo-pmcp/src/commands/loadtest/run.rs:181-241
// This is the resolve_auth_middleware() function to extract and generalize
async fn resolve_auth_middleware(
    mcp_server_url: &str,
    api_key: Option<String>,
    oauth_client_id: Option<String>,
    oauth_issuer: Option<String>,
    oauth_scopes: Option<Vec<String>>,
    oauth_no_cache: bool,
    oauth_redirect_port: u16,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    // API key path
    if let Some(key) = api_key {
        let bearer_token = BearerToken::new(key);
        let middleware = OAuthClientMiddleware::new(bearer_token);
        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(middleware));
        return Ok(Some(Arc::new(chain)));
    }
    // OAuth path
    if let Some(client_id) = oauth_client_id {
        let config = OAuthConfig { ... };
        let helper = OAuthHelper::new(config)?;
        let chain = helper.create_middleware_chain().await?;
        return Ok(Some(chain));
    }
    Ok(None)
}
```

### Example 2: ServerTester::new() signature
```rust
// Source: crates/mcp-tester/src/tester.rs:82-90
pub fn new(
    url: &str,
    timeout: Duration,
    insecure: bool,
    api_key: Option<&str>,           // Simple auth path
    force_transport: Option<&str>,
    http_middleware_chain: Option<Arc<HttpMiddlewareChain>>,  // Middleware auth path
) -> Result<Self>
```

### Example 3: Flatten pattern (from ServerFlags)
```rust
// Source: cargo-pmcp/src/commands/test/mod.rs:83-86
Run {
    #[command(flatten)]
    server_flags: ServerFlags,
    // ... other fields
},
```

### Example 4: conflicts_with pattern (from secret/mod.rs)
```rust
// Source: cargo-pmcp/src/commands/secret/mod.rs:76
#[arg(long, conflicts_with_all = ["stdin", "file", "env", "value", "generate"])]
prompt: bool,
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Inline auth fields per command | Shared AuthFlags struct via flatten | This phase | Eliminates 6-field duplication across 8 commands |
| resolve_auth_middleware in loadtest only | Shared auth resolver module | This phase | Enables auth for all server-connecting commands |
| No auth on test/preview/schema/connect | Full auth propagation | This phase | Users can test authenticated servers with any command |

## Open Questions

1. **How to wire auth into `run_scenario_with_transport()` and `generate_scenarios_with_transport()`?**
   - What we know: These library functions create `ServerTester` internally with `None` for auth params. They don't accept auth parameters.
   - What's unclear: Should we extend these functions with auth params, or have the CLI handlers create `ServerTester` directly?
   - Recommendation: Extend the library functions to accept `api_key: Option<&str>` and `http_middleware_chain: Option<Arc<HttpMiddlewareChain>>` parameters. This preserves the convenience API while adding auth support. Alternative: create new `_with_auth` variants.

2. **How to wire auth into preview's McpProxy?**
   - What we know: McpProxy creates its own reqwest::Client without auth headers. Preview is long-running (server stays up).
   - What's unclear: Should we pass an `Authorization` header to McpProxy, or create a pre-configured reqwest::Client?
   - Recommendation: Add optional `auth_header: Option<HeaderValue>` to `McpProxy::new()` and apply in `mcp_post()`. For OAuth, acquire the token once at startup via `OAuthHelper::get_access_token()` and construct the header. Token refresh during long preview sessions is a separate concern (defer).

3. **What should connect do with auth flags?**
   - What we know: Connect generates client config (Claude Code `claude mcp add`, Cursor JSON, Inspector URL). It doesn't make server requests itself.
   - What's unclear: Do these clients support auth configuration? Claude Code `claude mcp add` supports `--header` flag. Cursor JSON supports `headers` field.
   - Recommendation: Pass auth flags through to the generated config where possible. For `--api-key`, add `--header "Authorization: Bearer {key}"` to Claude Code command and `"headers": { "Authorization": "Bearer {key}" }` to Cursor config. For OAuth, note the complexity and potentially just document that the user should configure OAuth at the client level.

## Validation Architecture

> nyquist_validation not set in config.json -- including validation section.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace test config |
| Quick run command | `cargo test -p cargo-pmcp --lib -- auth` |
| Full suite command | `cargo test -p cargo-pmcp` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | test check accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |
| AUTH-02 | test run accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |
| AUTH-03 | test generate accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |
| AUTH-04 | preview accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |
| AUTH-05 | schema export accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |
| AUTH-06 | connect accepts auth flags | unit | `cargo test -p cargo-pmcp -- test_auth_flags` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p cargo-pmcp --lib`
- **Per wave merge:** `cargo test -p cargo-pmcp && make quality-gate`
- **Phase gate:** Full `make quality-gate` green before verify

### Wave 0 Gaps
- [ ] `cargo-pmcp/src/commands/auth.rs` -- shared resolve_auth_middleware function + tests
- [ ] Unit tests for `AuthFlags::resolve()` covering None, ApiKey, OAuth variants
- [ ] Unit tests for clap parse-level mutual exclusion (`--api-key` conflicts with `--oauth-client-id`)
- [ ] Integration pattern: cannot test real OAuth flow in unit tests; verify struct wiring only

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection of all 8 affected files (check.rs, run.rs, generate.rs, apps.rs, preview.rs, schema.rs, connect.rs, loadtest/mod.rs)
- Direct inspection of `ServerTester::new()` signature in `crates/mcp-tester/src/tester.rs`
- Direct inspection of `OAuthHelper`, `OAuthConfig`, `BearerToken` in `src/client/oauth.rs` and `src/client/oauth_middleware.rs`
- Direct inspection of `McpProxy` in `crates/mcp-preview/src/proxy.rs`
- Existing `resolve_auth_middleware()` in `cargo-pmcp/src/commands/loadtest/run.rs`
- Existing `conflicts_with_all` usage in `cargo-pmcp/src/commands/secret/mod.rs`

### Secondary (MEDIUM confidence)
- clap `conflicts_with` attribute behavior on `#[command(flatten)]` structs -- verified via existing codebase usage in secret module

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, no new dependencies
- Architecture: HIGH -- patterns directly observed in existing codebase (ServerFlags flatten, loadtest auth wiring)
- Pitfalls: HIGH -- identified by tracing data flow through actual handler code
- Open questions: MEDIUM -- connect command auth propagation depends on external client capabilities (Claude Code CLI, Cursor config format)

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- internal codebase patterns, no external API changes expected)
