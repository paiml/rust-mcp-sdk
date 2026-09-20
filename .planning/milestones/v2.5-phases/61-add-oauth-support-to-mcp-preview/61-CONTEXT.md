# Phase 61: Add OAuth support to mcp-preview - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add OAuth authentication support to mcp-preview so developers can test MCP Apps against OAuth-protected servers hosted on pmcp.run. The WASM MCP client embedded in pmcp.run gains OAuth capability, and mcp-preview uses it to negotiate tokens for its proxy requests.

</domain>

<decisions>
## Implementation Decisions

### Browser-based OAuth login flow
- **D-01:** The existing WASM MCP client embedded in pmcp.run is the OAuth negotiation point — it acquires the access token in-browser, then passes it to mcp-preview for proxy use
- **D-02:** Standard browser OAuth redirect flow (Authorization Code with PKCE) — the WASM client opens the authorization endpoint, user logs in, browser receives the code, WASM client exchanges for token
- **D-03:** No server-side OAuth client needed — the WASM client handles the entire flow in the browser
- **D-04:** No refresh token storage — each testing session is short-lived; user re-authenticates if the token expires

### Dynamic endpoint and auth triggering
- **D-05:** The MCP server endpoint URL is injected by the pmcp.run service management UI — mcp-preview does not need its own URL input for this scenario
- **D-06:** When the endpoint is OAuth-protected, a login screen/prompt pops up automatically before the preview session starts
- **D-07:** Auth detection: the WASM client attempts to connect; if the server returns 401/403, trigger the OAuth login flow

### Token handling
- **D-08:** Simple token lifecycle — acquire once per session, use until it expires or the session ends
- **D-09:** No refresh token persistence — no localStorage, no server-side session storage for tokens
- **D-10:** The access token is passed from the WASM client to the mcp-preview proxy as a Bearer header
- **D-11:** On token expiry (401/403 from MCP server during session), show an error in the DevTools panel and offer a "Re-login" button rather than silent refresh

### Claude's Discretion
- How the WASM client communicates the acquired token to the mcp-preview proxy (postMessage, shared state, API call)
- The login prompt UI design (modal, inline, redirect)
- Error messaging for auth failures in the DevTools panel
- Whether to show auth status indicator in the preview header

</decisions>

<specifics>
## Specific Ideas

- "It should be similar to the OAuth support that we have in the mcp-tester" — same PKCE flow, but running in the browser via the WASM client instead of CLI
- The pmcp.run service already has a "Try live" WASM client for calling tools/resources/prompts — OAuth support added there benefits both the "Try live" feature and the hosted mcp-preview
- Short testing sessions mean we can keep token handling simple — no refresh, no persistence

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### OAuth implementation (mcp-tester pattern to replicate)
- `src/client/oauth.rs` — Full PKCE + device code flow, OAuthConfig, OAuthHelper, token cache structure
- `src/client/oauth_middleware.rs` — BearerToken middleware, header injection
- `crates/mcp-tester/src/main.rs` lines 66-89 — CLI OAuth flags pattern
- `cargo-pmcp/src/commands/auth.rs` — `resolve_auth_header()` for simple consumers

### mcp-preview existing auth support
- `crates/mcp-preview/src/server.rs` lines 38-65 — `PreviewConfig.auth_header: Option<String>` already exists
- `crates/mcp-preview/src/proxy.rs` lines 215-227 — `McpProxy::new_with_auth()` already injects Authorization header to all outbound requests
- `crates/mcp-preview/assets/index.html` — Main SPA, needs login UI integration

### WASM client
- `examples/wasm-mcp-server/` — Existing WASM MCP implementation pattern
- `crates/mcp-preview/assets/widget-runtime.mjs` — Bridge runtime for browser-MCP communication

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `McpProxy::new_with_auth(base_url, auth_header)` — already accepts and injects Bearer token to all proxy requests
- `PreviewConfig.auth_header` — already threaded through the preview server startup
- `OAuthConfig` / `OAuthHelper` in `src/client/oauth.rs` — PKCE flow logic (needs WASM-compatible adaptation)
- `AuthFlags` in `cargo-pmcp/src/commands/flags.rs` — CLI flag pattern for OAuth configuration

### Established Patterns
- Proxy pattern: browser → mcp-preview API → MCP server (auth header injected at proxy level)
- WASM client pattern: browser-side MCP client communicating via postMessage bridge
- Token as simple string: `Option<String>` auth header flowing through config → proxy

### Integration Points
- WASM MCP client: add OAuth PKCE flow (browser redirect, code exchange)
- mcp-preview API: endpoint to receive token from browser after OAuth completes
- mcp-preview proxy: already wired — just needs the token updated dynamically
- Preview UI: login prompt trigger on 401/403, re-login button on expiry

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 61-add-oauth-support-to-mcp-preview*
*Context gathered: 2026-03-22*
