# Phase 29: Auth Flag Propagation - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Add shared OAuth and API-key flag structs to all server-facing commands. Every command that connects to an MCP server accepts `--api-key` and OAuth authentication flags (`--oauth-client-id`, `--oauth-issuer`, `--oauth-scopes`, `--oauth-no-cache`, `--oauth-redirect-port`). Commands that talk to pmcp.run (deploy, secret, landing, test upload/download/list) are NOT in scope — they use pmcp.run's own auth.

</domain>

<decisions>
## Implementation Decisions

### Command scope
- Auth flags added to 7 server-connecting commands: test check, test run, test generate, test apps, preview, schema export, connect
- pmcp.run commands (test upload/download/list, deploy, secret, landing) stay auth-free — they use pmcp.run's stored token, not MCP server auth
- Validate command stays auth-free (purely local)

### Loadtest migration
- Loadtest `run` migrates its 6 inline auth fields to the shared `AuthFlags` struct via `#[command(flatten)]`
- Loadtest handler signature changes to accept `&AuthFlags` directly (not destructured individual fields)
- Clean migration — no backward compat aliases needed (Phase 28 established "clean break" precedent)

### AuthFlags struct design
- Lives in `cargo-pmcp/src/commands/flags.rs` alongside `ServerFlags` and `FormatValue`
- Separate `#[command(flatten)]` from `ServerFlags` — not embedded inside it
- Fields: `api_key: Option<String>`, `oauth_client_id: Option<String>`, `oauth_issuer: Option<String>`, `oauth_scopes: Option<Vec<String>>`, `oauth_no_cache: bool`, `oauth_redirect_port: u16`
- Same env vars as Phase 26: `MCP_API_KEY`, `MCP_OAUTH_CLIENT_ID`, `MCP_OAUTH_ISSUER`, `MCP_OAUTH_SCOPES`, `MCP_OAUTH_REDIRECT_PORT`
- `--api-key` and `--oauth-client-id` mutually exclusive via clap `conflicts_with` attribute

### Auth resolution
- `AuthFlags::resolve()` method returns an `AuthMethod` enum: `None`, `ApiKey(String)`, `OAuth { client_id, issuer, scopes, no_cache, redirect_port }`
- Handlers match on the enum — no raw field inspection across 7+ handlers
- OAuth is the standard path (browser-based PKCE login + cached token file)
- Auth is explicit only — no auto-discovery from server's `/.well-known` endpoint

### Handler signatures
- All 7 server-connecting handlers use uniform pattern: `execute(..., auth_flags: &AuthFlags, global_flags: &GlobalFlags)`
- Consistent even if some commands don't use all auth capabilities yet

### Conflict handling
- `--api-key` and `--oauth-client-id` are mutually exclusive at clap parse level
- Clap generates the error message automatically — no custom runtime validation needed

### Claude's Discretion
- Exact `AuthMethod` enum field types and naming
- Whether `resolve()` returns `Result<AuthMethod>` or `AuthMethod` (validation already done by clap)
- Internal implementation of how each handler wires auth into `ServerTester::new()` or `OAuthHelper`
- Test strategy for AuthFlags parsing and resolution

</decisions>

<specifics>
## Specific Ideas

- OAuth is the primary auth path — the token comes from a browser-based login flow and is cached to `~/.mcp-tester/tokens.json`, not passed as an argument
- `--api-key` is the simpler alternative for servers using static bearer tokens
- Should feel identical to how loadtest already works — just available on more commands
- Validate command may be redundant with test (noted for future consideration, not this phase)

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `OAuthHelper` (pmcp::client::oauth): Complete OAuth implementation — PKCE, device code, caching, refresh
- `OAuthClientMiddleware` (src/client/oauth_middleware.rs): HTTP middleware for transparent `Authorization: Bearer` injection
- `ServerTester::new()` (crates/mcp-tester): Already accepts `api_key: Option<&str>` and `http_middleware_chain` params — auth-ready
- `GlobalFlags` struct (commands/mod.rs): Established pattern for shared flag propagation
- `ServerFlags` struct (commands/flags.rs): Established `#[command(flatten)]` pattern

### Established Patterns
- `#[command(flatten)]` for composing shared structs into command variants (ServerFlags in test run/generate)
- Flag env vars: `#[arg(long, env = "MCP_OAUTH_CLIENT_ID")]` pattern (loadtest/mod.rs)
- Handlers receive `&GlobalFlags` as last parameter — add `&AuthFlags` before it
- `conflicts_with` available in clap for mutual exclusivity

### Integration Points
- `cargo-pmcp/src/commands/flags.rs` — add `AuthFlags` struct here
- `cargo-pmcp/src/commands/test/mod.rs` — flatten `AuthFlags` into Check, Run, Generate, Apps variants
- `cargo-pmcp/src/commands/preview.rs` — add `AuthFlags` flatten
- `cargo-pmcp/src/commands/schema.rs` — add `AuthFlags` flatten
- `cargo-pmcp/src/commands/connect.rs` — add `AuthFlags` flatten
- `cargo-pmcp/src/commands/loadtest/mod.rs` — replace 6 inline fields with `AuthFlags` flatten
- Each handler: pass `auth_flags` to `ServerTester::new()` or `OAuthHelper` as appropriate
- `cargo-pmcp/src/commands/test/check.rs:47` — currently passes `None` for api_key, wire up AuthFlags
- `cargo-pmcp/src/commands/test/apps.rs:54` — same, currently passes `None`

</code_context>

<deferred>
## Deferred Ideas

- Validate command may be redundant with test — consider removing or merging in a future phase
- Auto-discovery of OAuth from server's `/.well-known` endpoint — potentially useful but adds complexity and surprising behavior

</deferred>

---

*Phase: 29-auth-flag-propagation*
*Context gathered: 2026-03-12*
