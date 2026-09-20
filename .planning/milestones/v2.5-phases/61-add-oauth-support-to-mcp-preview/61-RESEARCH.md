# Phase 61: Add OAuth Support to mcp-preview - Research

**Researched:** 2026-03-22
**Domain:** Browser-based OAuth PKCE for mcp-preview proxy authentication
**Confidence:** HIGH

## Summary

This phase adds OAuth authentication to mcp-preview so it can test MCP Apps against OAuth-protected servers hosted on pmcp.run. The implementation is well-scoped because: (1) the proxy layer already supports `auth_header: Option<String>` end-to-end, (2) the WASM client already connects through the `/api/mcp` proxy which auto-injects auth headers, and (3) the OAuth PKCE flow is well-understood browser technology with zero external library dependencies (Web Crypto API provides everything needed).

The architecture is a **browser-only OAuth popup flow**: when the proxy receives a 401/403 from the MCP server, the browser opens a popup to the authorization endpoint, handles the redirect callback, exchanges the authorization code for a token using the proxy as a relay (to avoid CORS on the token endpoint), and then passes the token to the mcp-preview server which updates the proxy's `auth_header` at runtime. No WASM-side OAuth logic is needed -- the WASM client talks to `/api/mcp` which always has the token injected server-side.

**Primary recommendation:** Implement OAuth as a purely browser-side JS flow in index.html with a thin server-side `/api/auth/token-exchange` endpoint for PKCE code-to-token exchange, and an `/api/auth/update-token` endpoint to dynamically update the proxy's auth header. Keep the WASM client untouched -- it already routes through the auth-injecting proxy.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** The existing WASM MCP client embedded in pmcp.run is the OAuth negotiation point -- it acquires the access token in-browser, then passes it to mcp-preview for proxy use
- **D-02:** Standard browser OAuth redirect flow (Authorization Code with PKCE) -- the WASM client opens the authorization endpoint, user logs in, browser receives the code, WASM client exchanges for token
- **D-03:** No server-side OAuth client needed -- the WASM client handles the entire flow in the browser
- **D-04:** No refresh token storage -- each testing session is short-lived; user re-authenticates if the token expires
- **D-05:** The MCP server endpoint URL is injected by the pmcp.run service management UI -- mcp-preview does not need its own URL input for this scenario
- **D-06:** When the endpoint is OAuth-protected, a login screen/prompt pops up automatically before the preview session starts
- **D-07:** Auth detection: the WASM client attempts to connect; if the server returns 401/403, trigger the OAuth login flow
- **D-08:** Simple token lifecycle -- acquire once per session, use until it expires or the session ends
- **D-09:** No refresh token persistence -- no localStorage, no server-side session storage for tokens
- **D-10:** The access token is passed from the WASM client to the mcp-preview proxy as a Bearer header
- **D-11:** On token expiry (401/403 from MCP server during session), show an error in the DevTools panel and offer a "Re-login" button rather than silent refresh

### Claude's Discretion
- How the WASM client communicates the acquired token to the mcp-preview proxy (postMessage, shared state, API call)
- The login prompt UI design (modal, inline, redirect)
- Error messaging for auth failures in the DevTools panel
- Whether to show auth status indicator in the preview header

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

## Architecture Patterns

### Architecture Overview

The existing mcp-preview architecture has two bridge modes:

1. **Proxy mode**: browser JS -> `/api/tools/call` -> `McpProxy` -> MCP server
2. **WASM mode**: browser JS -> WASM client -> `/api/mcp` (forward_raw) -> `McpProxy` -> MCP server

Both modes funnel through `McpProxy`, which already injects `auth_header` via `mcp_post()`. The key insight: **OAuth token management belongs in the browser JS layer, not the WASM client**. The WASM client doesn't need to know about OAuth at all -- it connects through the proxy which handles auth injection.

### Recommended Flow

```
1. Browser loads mcp-preview
2. Browser attempts session init (tools/list via proxy)
3. Proxy forwards to MCP server
4. If 401/403 returned:
   a. Proxy returns 401/403 to browser
   b. Browser shows "Login Required" modal
   c. Browser generates PKCE code_verifier + code_challenge (Web Crypto API)
   d. Browser opens popup to authorization_endpoint
   e. User authenticates in popup
   f. Popup redirects to mcp-preview callback page with ?code=...
   g. Callback page extracts code, posts to parent via window.opener.postMessage
   h. Parent sends code + code_verifier to POST /api/auth/token-exchange
   i. Server-side exchanges code for token (avoids CORS on token endpoint)
   j. Server stores token in McpProxy.auth_header (runtime update)
   k. Browser retries session init -- now succeeds
5. Session proceeds normally
```

### Component Responsibilities

```
Browser JS (index.html):
  - PKCE challenge generation (Web Crypto API)
  - Popup management (window.open -> authorization endpoint)
  - Callback handling (window.postMessage from popup)
  - Token relay to server (POST /api/auth/token-exchange)
  - Login UI (modal overlay)
  - Auth status display
  - Re-login button on 401/403 during session

Server (Rust):
  - /api/auth/token-exchange: POST code + verifier -> exchange at token endpoint -> update proxy
  - /api/auth/callback: GET callback page for popup (extracts code, posts to opener)
  - /api/auth/status: GET current auth state
  - McpProxy: runtime-updatable auth_header (needs interior mutability)
  - PreviewConfig: oauth_config fields (issuer, client_id, scopes, redirect_uri)

WASM client:
  - NO CHANGES (routes through /api/mcp which has auth injected)
```

### Recommended Project Structure Changes

```
crates/mcp-preview/
  src/
    handlers/
      api.rs          # existing - unchanged
      auth.rs         # NEW - /api/auth/* handlers
      mod.rs          # add auth module
      ...
    proxy.rs          # MODIFIED - auth_header becomes RwLock<Option<String>>
    server.rs         # MODIFIED - add oauth config, auth routes
  assets/
    index.html        # MODIFIED - add OAuth popup flow, login modal, auth status
    auth-callback.html # NEW - popup callback page (extracts code, posts to opener)
```

### Pattern: Dynamic Auth Header Update

The critical implementation detail is making `McpProxy.auth_header` updatable at runtime. Currently it is `auth_header: Option<String>` set once at construction. It needs to become `auth_header: RwLock<Option<String>>` (or `tokio::sync::RwLock`) so the auth handler can update it after OAuth completes.

```rust
// proxy.rs - current
pub struct McpProxy {
    auth_header: Option<String>,  // set once at construction
}

// proxy.rs - after
pub struct McpProxy {
    auth_header: tokio::sync::RwLock<Option<String>>,  // updatable
}

impl McpProxy {
    /// Update the auth header at runtime (after OAuth flow completes).
    pub async fn set_auth_header(&self, header: Option<String>) {
        let mut guard = self.auth_header.write().await;
        *guard = header;
    }

    fn mcp_post(&self) -> reqwest::RequestBuilder {
        // This becomes async or needs to read from RwLock
        // Option: use parking_lot::RwLock for sync read access
        // since the read is tiny (clone an Option<String>)
    }
}
```

**Recommendation:** Use `parking_lot::RwLock` (already in project deps) instead of `tokio::sync::RwLock` for `auth_header`. The auth_header is a small string that is read on every request -- `parking_lot::RwLock` provides sync reads which avoids making `mcp_post()` async and minimizes disruption to the existing code.

### Pattern: PKCE in Browser JavaScript (No External Libraries)

The browser Web Crypto API provides everything needed for PKCE:

```javascript
// Generate code_verifier: 43-128 character random string
function generateCodeVerifier() {
  const array = new Uint8Array(32);
  crypto.getRandomValues(array);
  return btoa(String.fromCharCode(...array))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

// Generate code_challenge: SHA-256 hash of verifier, base64url-encoded
async function generateCodeChallenge(verifier) {
  const encoder = new TextEncoder();
  const data = encoder.encode(verifier);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return btoa(String.fromCharCode(...new Uint8Array(hash)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}
```

**NOTE:** `crypto.subtle` requires a secure context (HTTPS or localhost). This is fine because mcp-preview runs on localhost for local dev, and on HTTPS for pmcp.run.

### Pattern: Popup OAuth Flow

```javascript
// Open authorization popup
function openAuthPopup(authorizationUrl) {
  const width = 600, height = 700;
  const left = (screen.width - width) / 2;
  const top = (screen.height - height) / 2;
  const popup = window.open(
    authorizationUrl,
    'oauth-popup',
    `width=${width},height=${height},left=${left},top=${top}`
  );
  return popup;
}

// Listen for callback from popup
window.addEventListener('message', async (event) => {
  if (event.origin !== window.location.origin) return;
  if (event.data?.type !== 'oauth-callback') return;
  const { code } = event.data;
  // Exchange code for token via server
  await exchangeCodeForToken(code, codeVerifier);
});
```

### Pattern: Server-Side Token Exchange

The token exchange must happen server-side because:
1. The token endpoint may be on a different origin (CORS blocks browser-to-token-endpoint)
2. Even for public clients (no client_secret), CORS policies on authorization servers often block browser-direct requests to the token endpoint

```rust
// handlers/auth.rs
pub async fn token_exchange(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TokenExchangeRequest>,
) -> Result<Json<TokenExchangeResponse>, (StatusCode, String)> {
    let client = reqwest::Client::new();
    let response = client.post(&req.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &req.code),
            ("redirect_uri", &req.redirect_uri),
            ("client_id", &req.client_id),
            ("code_verifier", &req.code_verifier),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let token_response: TokenResponse = response.json().await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    // Update proxy auth header
    let auth_header = format!("Bearer {}", token_response.access_token);
    state.proxy.set_auth_header(Some(auth_header)).await;

    Ok(Json(TokenExchangeResponse { success: true }))
}
```

### Anti-Patterns to Avoid

- **Don't put OAuth logic in the WASM client:** The WASM client talks through `/api/mcp` which already has auth injected. Adding OAuth to WASM increases compile time, WASM binary size, and complexity for zero benefit.
- **Don't use localStorage for tokens:** Per D-09, no persistence. Tokens live only in server memory (McpProxy.auth_header) and are lost on server restart.
- **Don't redirect the main page:** Use a popup window for the OAuth flow so the preview session state (selected tool, widget, etc.) is preserved.
- **Don't try to make browser-to-token-endpoint requests directly:** CORS will block them. Route through the preview server.
- **Don't add OAuth dependencies to mcp-preview crate:** The server-side token exchange is a simple HTTP POST with form data -- `reqwest` (already a dependency) handles it.

## Standard Stack

### Core (Already in dependencies -- no new crates needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.13 | Server-side token exchange HTTP POST | Already in mcp-preview Cargo.toml |
| axum | 0.8 | New /api/auth/* route handlers | Already in mcp-preview Cargo.toml |
| serde/serde_json | 1 | Request/response serialization | Already in mcp-preview Cargo.toml |
| parking_lot | (workspace) | Sync RwLock for auth_header | Already in workspace deps |
| Web Crypto API | browser built-in | PKCE challenge generation (SHA-256) | Zero-dependency browser standard |

### No New Dependencies Needed

The existing mcp-preview dependencies cover everything:
- `reqwest` for server-side token exchange
- `axum` for route handlers
- `serde`/`serde_json` for JSON
- `parking_lot` from workspace for sync RwLock
- Browser Web Crypto API for PKCE (no JS libraries needed)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PKCE code_verifier generation | Custom random string | `crypto.getRandomValues()` | Cryptographically secure randomness |
| SHA-256 hashing | JS crypto library | `crypto.subtle.digest()` | Browser-native, zero dependencies |
| Base64URL encoding | npm base64 library | Manual btoa() + char replace | 3 lines of code, no deps |
| OAuth discovery | Custom endpoint probing | `/.well-known/openid-configuration` fetch | Standard OIDC discovery protocol |
| Token exchange HTTP | Raw XMLHttpRequest | `fetch()` to own server endpoint | Server relays to avoid CORS |

## Common Pitfalls

### Pitfall 1: CORS on Token Endpoint
**What goes wrong:** Browser tries to POST directly to the OAuth provider's token endpoint and gets blocked by CORS.
**Why it happens:** Authorization servers typically don't allow cross-origin requests to their token endpoint from arbitrary SPAs.
**How to avoid:** Route the token exchange through the mcp-preview server (`POST /api/auth/token-exchange`). The server is a "backend" and not subject to CORS restrictions.
**Warning signs:** `Access-Control-Allow-Origin` errors in browser console.

### Pitfall 2: Popup Blockers
**What goes wrong:** The OAuth popup gets blocked by the browser's popup blocker.
**Why it happens:** Popup blockers trigger when `window.open()` is called outside of a direct user interaction (click handler).
**How to avoid:** Always call `window.open()` from within a click event handler (the "Login" button click), never from an async callback or setTimeout.
**Warning signs:** Popup returns null from `window.open()`.

### Pitfall 3: McpProxy Borrow Issues After Making auth_header Mutable
**What goes wrong:** Making `auth_header` an `RwLock` breaks `mcp_post()` which currently reads `self.auth_header` synchronously.
**Why it happens:** `mcp_post()` is sync, but `tokio::sync::RwLock` requires `.await` for reads.
**How to avoid:** Use `parking_lot::RwLock` which provides sync `.read()`. The lock duration is trivially short (cloning an `Option<String>`).
**Warning signs:** Compilation errors about `await` in non-async context.

### Pitfall 4: crypto.subtle Not Available
**What goes wrong:** `crypto.subtle` is undefined, PKCE generation fails.
**Why it happens:** `crypto.subtle` requires a secure context (HTTPS or localhost). Development over plain HTTP on non-localhost addresses will fail.
**How to avoid:** mcp-preview already binds to 127.0.0.1 by default (secure context). For pmcp.run deployment, HTTPS is guaranteed. Add a runtime check with a clear error message.
**Warning signs:** `TypeError: Cannot read property 'digest' of undefined`.

### Pitfall 5: Race Condition on Auth Header Update
**What goes wrong:** Multiple requests in-flight when auth header is updated, some use old (empty) header, some use new header.
**Why it happens:** The token exchange completes and updates the auth_header while other requests are already built.
**How to avoid:** After updating the auth header, explicitly retry the session initialization. The `parking_lot::RwLock` ensures all subsequent `mcp_post()` calls see the new value.
**Warning signs:** Intermittent 401s after successful OAuth.

### Pitfall 6: Callback Page Origin Mismatch
**What goes wrong:** `window.opener.postMessage` from the callback page targets the wrong origin or is rejected.
**Why it happens:** The callback redirect URL must be on the same origin as the mcp-preview server. If the OAuth provider redirects to a different origin, postMessage validation fails.
**How to avoid:** Register the callback URI as `http://localhost:{port}/api/auth/callback` (same origin as mcp-preview). For pmcp.run, use the HTTPS origin.
**Warning signs:** No message received in the parent window after popup redirects.

## Code Examples

### Browser-Side PKCE + Popup Flow (index.html)

```javascript
// OAuth manager integrated into PreviewRuntime
class OAuthManager {
  constructor(runtime) {
    this.runtime = runtime;
    this.codeVerifier = null;
    this.popup = null;
    // Listen for callback from popup
    window.addEventListener('message', (e) => this.handleCallback(e));
  }

  async startLogin(oauthConfig) {
    // Generate PKCE
    this.codeVerifier = this.generateCodeVerifier();
    const codeChallenge = await this.generateCodeChallenge(this.codeVerifier);

    // Build authorization URL
    const params = new URLSearchParams({
      client_id: oauthConfig.client_id,
      response_type: 'code',
      redirect_uri: `${window.location.origin}/api/auth/callback`,
      scope: oauthConfig.scopes.join(' '),
      code_challenge: codeChallenge,
      code_challenge_method: 'S256',
      state: this.generateCodeVerifier(), // CSRF protection
    });
    const authUrl = `${oauthConfig.authorization_endpoint}?${params}`;

    // Open popup (MUST be in click handler)
    this.popup = window.open(authUrl, 'oauth', 'width=600,height=700');
  }

  async handleCallback(event) {
    if (event.origin !== window.location.origin) return;
    if (event.data?.type !== 'oauth-callback') return;

    const { code } = event.data;
    if (this.popup) this.popup.close();

    // Exchange code for token via server
    const resp = await fetch('/api/auth/token-exchange', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        code,
        code_verifier: this.codeVerifier,
      }),
    });
    if (resp.ok) {
      this.runtime.setAuthStatus('authenticated');
      await this.runtime.initSession(); // Retry
    }
  }

  generateCodeVerifier() {
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    return btoa(String.fromCharCode(...array))
      .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
  }

  async generateCodeChallenge(verifier) {
    const data = new TextEncoder().encode(verifier);
    const hash = await crypto.subtle.digest('SHA-256', data);
    return btoa(String.fromCharCode(...new Uint8Array(hash)))
      .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
  }
}
```

### Server-Side Auth Handlers (handlers/auth.rs)

```rust
use axum::{extract::State, http::StatusCode, response::Html, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;

#[derive(Deserialize)]
pub struct TokenExchangeRequest {
    pub code: String,
    pub code_verifier: String,
}

#[derive(Serialize)]
pub struct TokenExchangeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn token_exchange(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TokenExchangeRequest>,
) -> Result<Json<TokenExchangeResponse>, (StatusCode, String)> {
    let oauth = state.config.oauth_config.as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "OAuth not configured".into()))?;

    let client = reqwest::Client::new();
    let redirect_uri = format!("http://localhost:{}/api/auth/callback", state.config.port);

    let resp = client.post(&oauth.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", req.code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", oauth.client_id.as_str()),
            ("code_verifier", req.code_verifier.as_str()),
        ])
        .send().await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Token exchange failed: {e}")))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Ok(Json(TokenExchangeResponse {
            success: false,
            error: Some(format!("Token endpoint returned error: {text}")),
        }));
    }

    #[derive(Deserialize)]
    struct TokenResp { access_token: String }
    let token: TokenResp = resp.json().await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid token response: {e}")))?;

    // Update proxy auth header
    state.proxy.set_auth_header(Some(format!("Bearer {}", token.access_token)));

    Ok(Json(TokenExchangeResponse { success: true, error: None }))
}

/// Serve the OAuth callback page (loaded in popup, posts code to opener)
pub async fn callback() -> Html<&'static str> {
    Html(include_str!("../../assets/auth-callback.html"))
}

/// Return current auth status
pub async fn status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let has_auth = state.proxy.has_auth_header();
    Json(serde_json::json!({ "authenticated": has_auth }))
}
```

### OAuth Callback Page (auth-callback.html)

```html
<!DOCTYPE html>
<html>
<head><title>OAuth Callback</title></head>
<body>
  <p>Authenticating...</p>
  <script>
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    const error = params.get('error');
    if (window.opener) {
      window.opener.postMessage(
        { type: 'oauth-callback', code, error },
        window.location.origin
      );
    }
    // Popup closes itself after posting
    setTimeout(() => window.close(), 1000);
  </script>
</body>
</html>
```

### McpProxy Dynamic Auth Header (proxy.rs changes)

```rust
use parking_lot::RwLock;

pub struct McpProxy {
    base_url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
    session: tokio::sync::RwLock<Option<SessionInfo>>,
    auth_header: RwLock<Option<String>>,  // Changed from Option<String>
}

impl McpProxy {
    pub fn new_with_auth(base_url: &str, auth_header: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
            session: tokio::sync::RwLock::new(None),
            auth_header: RwLock::new(auth_header),
        }
    }

    pub fn set_auth_header(&self, header: Option<String>) {
        *self.auth_header.write() = header;
    }

    pub fn has_auth_header(&self) -> bool {
        self.auth_header.read().is_some()
    }

    fn mcp_post(&self) -> reqwest::RequestBuilder {
        let mut builder = self.client.post(&self.base_url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");
        if let Some(ref auth) = *self.auth_header.read() {
            builder = builder.header("Authorization", auth.clone());
        }
        builder
    }
}
```

### PreviewConfig OAuth Extension (server.rs)

```rust
/// OAuth configuration for browser-based PKCE flow
#[derive(Debug, Clone, Default)]
pub struct OAuthPreviewConfig {
    /// OAuth client ID for the preview app
    pub client_id: String,
    /// Authorization endpoint URL
    pub authorization_endpoint: String,
    /// Token endpoint URL
    pub token_endpoint: String,
    /// Requested scopes
    pub scopes: Vec<String>,
}

pub struct PreviewConfig {
    // ... existing fields ...
    /// OAuth configuration for browser-based auth (None = no OAuth)
    pub oauth_config: Option<OAuthPreviewConfig>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Implicit grant (token in URL fragment) | Authorization Code + PKCE | RFC 9700 / OAuth 2.1 | Must use PKCE, never implicit |
| Full-page redirect for OAuth | Popup window + postMessage | Mainstream ~2020+ | Preserves SPA state |
| Client secret for SPAs | Public client (no secret) | OAuth 2.1 draft | Browser apps must not have secrets |
| Custom crypto for PKCE | Web Crypto API | ~2018+ universal support | Zero-dependency, browser-native |

## Design Decisions (Claude's Discretion)

### Token Communication: API Call (recommended)

Options considered:
1. **postMessage from popup to parent** -> parent sends to server via `POST /api/auth/token-exchange`
2. **Shared state** (global variable) -- fragile, timing issues
3. **Direct API call** from popup -- popup is on same origin, could call API directly

**Recommendation:** Option 1 -- popup posts authorization code to parent via `postMessage`, parent calls `/api/auth/token-exchange`. This is the standard popup OAuth pattern and keeps all logic in one place (the OAuthManager in the parent window).

### Login UI: Modal Overlay (recommended)

Options considered:
1. **Modal overlay** -- shows "Login Required" with a "Sign In" button over the preview
2. **Inline banner** -- shows at top of page
3. **Full redirect** -- navigates away from preview

**Recommendation:** Modal overlay. It is the most visually clear, blocks interaction with the preview until auth completes, and is trivially implementable with existing CSS variables. The modal should show the MCP server URL, OAuth provider info, and a "Sign In" button that opens the popup.

### Auth Status Indicator: Yes, in Header (recommended)

**Recommendation:** Add a small auth status indicator next to the existing connection status dot in the header. When authenticated, show a green lock icon. When not authenticated (no OAuth configured), show nothing. When auth expired, show a red lock with "Re-login" button.

### Error Messaging for Auth Failures

**Recommendation:** Auth failures should appear in the DevTools Events panel (same as other events) with type `authError`. Additionally, 401/403 responses during the session should trigger a dismissible toast notification with a "Re-login" button.

## Open Questions

1. **OAuth Discovery for pmcp.run Servers**
   - What we know: The mcp-tester uses OIDC discovery (`.well-known/openid-configuration`) to find endpoints automatically. pmcp.run servers presumably expose this.
   - What's unclear: Whether pmcp.run will provide the `authorization_endpoint` and `token_endpoint` via the service management UI, or whether mcp-preview should discover them.
   - Recommendation: Support both -- accept explicit `authorization_endpoint`/`token_endpoint` in `OAuthPreviewConfig`, but also support auto-discovery from the MCP server URL (matching the mcp-tester pattern). The config endpoint (`/api/config`) should include OAuth config so the browser JS knows the endpoints.

2. **OAuth Client ID Provisioning**
   - What we know: A `client_id` is needed for the PKCE flow. For local dev, this comes from CLI flags. For pmcp.run, it would come from the service configuration.
   - What's unclear: Whether pmcp.run assigns a shared `client_id` for all mcp-preview instances or per-server.
   - Recommendation: Accept `client_id` as a config field. For Phase 61, focus on making the preview accept it via config. How pmcp.run provisions it is outside this phase's scope.

3. **Forward_raw 401/403 Detection**
   - What we know: `forward_raw` in proxy.rs uses `check_response` which returns errors on non-2xx. However, the error propagates as `BAD_GATEWAY` (502) to the browser.
   - What's unclear: Whether the browser JS currently distinguishes 502 from other errors.
   - Recommendation: When `forward_raw` gets a 401/403 from upstream, return the upstream status code (not 502) to the browser so the JS can detect auth failures and trigger the login flow.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/mcp-preview/src/proxy.rs` -- McpProxy with `auth_header: Option<String>`, `new_with_auth()`, `mcp_post()`, `forward_raw()`
- Codebase analysis: `crates/mcp-preview/src/server.rs` -- PreviewConfig, AppState, route setup
- Codebase analysis: `crates/mcp-preview/assets/index.html` -- WASM client initialization via `/api/mcp` proxy
- Codebase analysis: `src/client/oauth.rs` -- OAuthHelper PKCE implementation (server-side reference)
- Codebase analysis: `cargo-pmcp/src/commands/auth.rs` -- resolve_auth_header pattern
- Codebase analysis: `src/shared/wasm_http.rs` -- WasmHttpConfig.extra_headers, WasmHttpClient

### Secondary (MEDIUM confidence)
- [Auth0: Authorization Code Flow with PKCE](https://auth0.com/docs/get-started/authentication-and-authorization-flow/authorization-code-flow-with-pkce)
- [OAuth 2.0 for Browser-Based Applications (IETF draft)](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-browser-based-apps)
- [Curity: PKCE JavaScript Example](https://github.com/curityio/pkce-javascript-example)
- [OAuth Popup Practical Guide](https://dev.to/didof/oauth-popup-practical-guide-57l9)
- [Spotify: Authorization Code with PKCE Flow](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow)
- [OWASP: OAuth 2.0 Protocol Cheatsheet](https://cheatsheetseries.owasp.org/cheatsheets/OAuth2_Cheat_Sheet.html)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, everything already in workspace
- Architecture: HIGH -- existing proxy pattern is clear, OAuth popup is well-established
- Pitfalls: HIGH -- CORS, popup blockers, and auth header mutability are well-documented

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable browser APIs, stable Rust crate versions)
