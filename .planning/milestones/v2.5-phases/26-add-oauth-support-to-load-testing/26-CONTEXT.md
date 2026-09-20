# Phase 26: Add OAuth Support to Load-Testing - Context

**Gathered:** 2026-02-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Add OAuth and API key authentication support to `cargo pmcp loadtest run` so VUs can target OAuth-protected MCP servers. Mirror the existing `cargo pmcp test` auth pattern for consistency.

</domain>

<decisions>
## Implementation Decisions

### Auth is CLI-flag based, NOT in TOML config
- Mirror `cargo pmcp test` exactly: auth via CLI flags and env vars, NOT in the loadtest TOML config
- Same flags: `--oauth-client-id`, `--oauth-issuer`, `--oauth-scopes`, `--oauth-no-cache`, `--oauth-redirect-port`, `--api-key`
- Same env vars: `MCP_OAUTH_CLIENT_ID`, `MCP_OAUTH_ISSUER`, `MCP_OAUTH_SCOPES`, `MCP_API_KEY`
- Loadtest TOML config files stay auth-free (same config works with any auth provider)

### Reuse existing OAuthHelper from mcp-tester
- `crates/mcp-tester/src/oauth.rs` has the complete OAuth implementation: PKCE auth code flow, device code fallback, token caching, refresh
- Loadtest should reuse `OAuthHelper` and `OAuthClientMiddleware` — not reimplement
- Token cached to `~/.mcp-tester/tokens.json` (shared with test commands)

### Auth flow mirrors test commands
- Token acquired ONCE at startup before VUs spawn (fail fast on misconfigured auth)
- Auto-discovery via `/.well-known/openid-configuration` from the MCP server URL
- Browser-based auth code + PKCE flow (primary), device code flow (fallback)
- Token refresh if expired, re-auth if refresh fails

### Header injection via middleware chain
- `OAuthClientMiddleware` wraps `reqwest::Client` — adds `Authorization: Bearer <token>` transparently
- `McpClient` does NOT need manual auth header logic — middleware handles it
- For `--api-key`: same pattern as test commands (direct header injection)

### Usage should look identical to test commands
```bash
# OAuth
cargo pmcp loadtest run https://api.example.com/mcp \
  --config loadtest.toml \
  --oauth-client-id MY_CLIENT_ID

# With explicit issuer
cargo pmcp loadtest run https://api.example.com/mcp \
  --config loadtest.toml \
  --oauth-client-id MY_CLIENT_ID \
  --oauth-issuer https://auth.example.com

# API key
cargo pmcp loadtest run https://api.example.com/mcp \
  --config loadtest.toml \
  --api-key my-secret-token

# Environment variables
export MCP_OAUTH_CLIENT_ID="my-client-id"
cargo pmcp loadtest run https://api.example.com/mcp --config loadtest.toml
```

### Claude's Discretion
- How to thread the middleware chain through the loadtest engine to McpClient instances
- Whether McpClient needs refactoring to accept a middleware-wrapped client or if reqwest middleware suffices
- Auth type display in test summary (nice to have, not critical)

</decisions>

<specifics>
## Specific Ideas

- Keep it simple and consistent with `cargo pmcp test` — same flags, same OAuthHelper, same flows
- If opportunities to improve on the test auth support are discovered during implementation, those can be explored
- The loadtest TOML config should NOT grow an `[auth]` section — auth is always CLI/env based

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `OAuthHelper` (crates/mcp-tester/src/oauth.rs): Complete OAuth implementation — discovery, PKCE, device code, caching, refresh
- `OAuthClientMiddleware` (src/client/oauth_middleware.rs): HTTP middleware that injects `Authorization: Bearer <token>` into requests
- `HttpMiddlewareChain`: Middleware chain pattern used by ServerTester — can be reused by loadtest engine
- `BearerToken` struct: Token with expiry tracking (`is_expired()`, `expires_soon()`)

### Established Patterns
- CLI OAuth flags: `--oauth-client-id`, `--oauth-issuer`, `--oauth-scopes`, `--oauth-no-cache`, `--oauth-redirect-port` (in mcp-tester main.rs and test/mod.rs)
- Token lifecycle: cache → check expiry → refresh → re-auth (OAuthHelper::get_access_token)
- Middleware wraps reqwest::Client — auth is transparent to the request-making code
- Token caching: `~/.mcp-tester/tokens.json`

### Integration Points
- `cargo-pmcp/src/commands/loadtest/mod.rs` — add OAuth CLI flags to `Run` variant (mirror test/mod.rs)
- `cargo-pmcp/src/commands/loadtest/run.rs` — create OAuthHelper, get token, build middleware chain before engine start
- `cargo-pmcp/src/loadtest/engine.rs` — accept middleware chain, pass to VU creation
- `cargo-pmcp/src/loadtest/client.rs` — McpClient may need to accept middleware-wrapped client or auth header
- `crates/mcp-tester` — may need to expose OAuthHelper as a public API if not already

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 26-add-oauth-support-to-load-testing*
*Context gathered: 2026-02-28*
