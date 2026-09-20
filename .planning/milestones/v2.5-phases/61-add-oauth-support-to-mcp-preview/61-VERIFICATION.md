---
phase: 61-add-oauth-support-to-mcp-preview
verified: 2026-03-27T00:00:00Z
status: passed
score: 7/7 must-haves verified
re_verification:
  previous_status: gaps_found
  previous_score: 6/7
  gaps_closed:
    - "forward_mcp returns upstream 401/403 status codes instead of wrapping them as 502"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Full OAuth popup flow end-to-end"
    expected: "Login modal appears on 401/403, popup opens, code exchanges for token, session proceeds"
    why_human: "Requires live OAuth provider and browser interaction"
  - test: "Auth status indicator in header"
    expected: "Green Authenticated label appears after login, red Auth expired + Re-login button appears on token expiry"
    why_human: "Visual state requires browser rendering"
  - test: "Re-login button in events panel on mid-session 401/403"
    expected: "Events panel shows authExpired entry with inline Re-login button that reopens the login modal"
    why_human: "Requires triggering session-level 401/403 from a live server"
---

# Phase 61: Add OAuth Support to mcp-preview — Verification Report

**Phase Goal:** Add browser-based OAuth PKCE authentication to mcp-preview so developers can test MCP Apps against OAuth-protected servers on pmcp.run, with dynamic auth header updates, login modal, and CLI flag wiring.
**Verified:** 2026-03-27
**Status:** passed — 7/7 must-haves verified
**Re-verification:** Yes — after gap closure (plan 61-03)

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                           | Status      | Evidence                                                                                 |
|----|------------------------------------------------------------------------------------------------|-------------|------------------------------------------------------------------------------------------|
| 1  | McpProxy auth_header is dynamically updatable at runtime without restarting the server         | VERIFIED  | `SyncRwLock<Option<String>>` at proxy.rs:224; `set_auth_header` at :248, `has_auth_header` at :253 |
| 2  | POST /api/auth/token-exchange accepts code + code_verifier and exchanges with token endpoint   | VERIFIED  | handlers/auth.rs:38 `token_exchange`; POSTs to `oauth.token_endpoint` with PKCE params; calls `set_auth_header` at :94 |
| 3  | GET /api/auth/callback serves a self-closing HTML page that posts authorization code to opener | VERIFIED  | handlers/auth.rs:106 `callback`; includes auth-callback.html via `include_str!`; postMessage at auth-callback.html:51 |
| 4  | GET /api/auth/status returns JSON with authenticated boolean                                   | VERIFIED  | handlers/auth.rs:114 `status`; returns `{"authenticated": has_auth_header()}` |
| 5  | GET /api/config includes oauth_config when configured                                          | VERIFIED  | handlers/api.rs:70-74 maps `OAuthPreviewConfig` to `OAuthConfigResponse`; added to `ConfigResponse` at :27 |
| 6  | forward_mcp returns upstream 401/403 status codes instead of wrapping them as 502             | VERIFIED  | `forward_raw` signature is `Result<RawForwardResult, McpRequestError>` (proxy.rs:551); detects 401/403 at :566-569 and returns `AuthRequired`; `forward_mcp` matches `AuthRequired` at api.rs:452-455 and returns upstream status; `BAD_GATEWAY` only in `Other` arm at :456 |
| 7  | cargo pmcp preview --oauth-client-id triggers OAuthPreviewConfig creation                     | VERIFIED  | preview.rs:57-94 matches `AuthMethod::OAuth`, calls `discover_oauth_endpoints`, constructs `OAuthPreviewConfig` |

**Score:** 7/7 truths verified

---

## Required Artifacts

| Artifact                                              | Expected                                                              | Status    | Details                                                                    |
|-------------------------------------------------------|-----------------------------------------------------------------------|-----------|----------------------------------------------------------------------------|
| `crates/mcp-preview/src/proxy.rs`                    | SyncRwLock auth_header + set/has methods + McpRequestError enum       | VERIFIED  | Lines 8, 203-217, 224, 248, 253. McpRequestError has AuthRequired(u16, String) and Other(anyhow::Error). forward_raw at :546 returns Result<RawForwardResult, McpRequestError> with 401/403 detection at :566-569. |
| `crates/mcp-preview/src/handlers/auth.rs`            | token_exchange, callback, status HTTP handlers                        | VERIFIED  | All three handlers present and substantive. token_exchange calls set_auth_header after token response. |
| `crates/mcp-preview/assets/auth-callback.html`       | OAuth popup callback page                                             | VERIFIED  | Contains `type: 'oauth-callback'`, `window.opener.postMessage`, closes popup after 1.5s |
| `crates/mcp-preview/src/server.rs`                   | OAuthPreviewConfig struct, oauth_config field on PreviewConfig        | VERIFIED  | OAuthPreviewConfig at :25-34; oauth_config: Option<OAuthPreviewConfig> at :83; Default sets None at :97 |
| `crates/mcp-preview/assets/index.html`               | OAuthManager class with PKCE, popup, login modal, auth status         | VERIFIED  | OAuthManager at :1644; _generateCodeVerifier/:1665, _generateCodeChallenge/:1673, crypto.subtle.digest/'SHA-256'/:1675, code_challenge_method: 'S256'/:1694, window.open/:1703, oauth-login-modal/:1766, oauth-sign-in-btn/:1776, auth-status-indicator/:1804, relogin-btn/:1819 |
| `cargo-pmcp/src/commands/preview.rs`                 | OAuth config wiring from AuthFlags to OAuthPreviewConfig              | VERIFIED  | discover_oauth_endpoints/:152, AuthMethod::OAuth match/:58, oauth_config constructed before resolve_auth_header/:98, graceful fallback at :101-115 |

---

## Key Link Verification

| From                                        | To                                        | Via                                         | Status    | Details                                                                  |
|---------------------------------------------|-------------------------------------------|---------------------------------------------|-----------|--------------------------------------------------------------------------|
| `handlers/auth.rs`                          | `proxy.rs`                                | `state.proxy.set_auth_header()`             | WIRED     | auth.rs:94 calls `set_auth_header(Some(format!("Bearer {}", access_token)))` |
| `server.rs`                                 | `handlers/auth.rs`                        | axum route registration `/api/auth/*`       | WIRED     | server.rs:160-162 registers token-exchange, callback, status routes |
| `index.html` OAuthManager                   | `/api/auth/token-exchange`                | `fetch` POST in `_handleCallback`           | WIRED     | index.html:1736 `fetch('/api/auth/token-exchange', { method: 'POST' })` |
| `index.html`                                | `/api/config`                             | fetch reads `oauth_config`                  | WIRED     | index.html:1949 `if (config.oauth_config) { this.oauth.setConfig(...) }` |
| `cargo-pmcp/src/commands/preview.rs`        | `mcp_preview::OAuthPreviewConfig`         | config struct construction                  | WIRED     | preview.rs:74-79 constructs OAuthPreviewConfig; passed to PreviewConfig at :129 |
| `handlers/api.rs` forward_mcp              | `proxy.rs` forward_raw                    | 401/403 propagation via McpRequestError     | WIRED     | forward_raw returns `Result<RawForwardResult, McpRequestError>` (proxy.rs:551); 401/403 detection at :566-569; forward_mcp matches `AuthRequired` at api.rs:452-455; BAD_GATEWAY only in `Other` arm at :456 |

---

## Requirements Coverage

No requirement IDs were declared in plan frontmatter. Phase has no formal REQUIREMENTS.md entries.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | All prior blockers resolved in plan 61-03 |

The two blocker anti-patterns from initial verification are resolved:
- `check_response` is no longer called for upstream responses in `forward_raw`; 401/403 are detected first and returned as `McpRequestError::AuthRequired`
- `forward_mcp` no longer returns `BAD_GATEWAY` for all errors; only `McpRequestError::Other` triggers `BAD_GATEWAY`
- `McpProxy::new()` carries `#[allow(dead_code)]` with comment at proxy.rs:229-230

`cargo check -p mcp-preview` reports zero errors and zero warnings.

---

## Scope Note: WASM Path vs Non-WASM Path

Both paths now handle 401/403 correctly:

- `list_tools` / `call_tool` / `list_resources` (via `send_request`) — `McpRequestError::AuthRequired` → HTTP 401/403 (VERIFIED, unchanged)
- `forward_mcp` (via `forward_raw`) — `McpRequestError::AuthRequired` → HTTP 401/403 (NOW VERIFIED, gap closed in plan 61-03)

The `executeTool()` auth-detection condition at index.html:2576 (checking `response.status === 401 || response.status === 403`) will now correctly trigger on the WASM path.

---

## Human Verification Required

### 1. Full OAuth Popup Flow

**Test:** Start `cargo pmcp preview <oauth-protected-server> --oauth-client-id <id> --oauth-issuer <url>`. Observe that login modal appears when session fails with 401/403.
**Expected:** Modal shows "Authentication Required", clicking Sign In opens popup to authorization endpoint, after login popup closes and session proceeds normally.
**Why human:** Requires live OAuth provider, browser interaction, and popup behavior.

### 2. Auth Status Indicator

**Test:** After successful OAuth login in the browser, observe the header area.
**Expected:** A green "Authenticated" label appears. If token expires mid-session, it changes to red "Auth expired" with a Re-login button.
**Why human:** Visual state requires browser rendering and live token expiry.

### 3. Re-login Events Panel Entry

**Test:** Trigger a 401/403 during an active tool call (simulate by revoking token on server side).
**Expected:** Events panel shows an authExpired entry with inline Re-login button that opens the login modal when clicked.
**Why human:** Requires triggering mid-session auth failure from a live server.

---

## Re-verification Summary

**Gap closed (plan 61-03):** The WASM bridge path (browser -> `/api/mcp` -> `forward_mcp` -> `forward_raw` -> MCP server) now correctly propagates 401/403 upstream status codes to the browser.

Changes verified in codebase:
1. `forward_raw` signature changed to `Result<RawForwardResult, McpRequestError>` (proxy.rs:551)
2. 401/403 detection before `check_response` at proxy.rs:566-569 — returns `McpRequestError::AuthRequired(status.as_u16(), text)`
3. Non-auth HTTP errors return `McpRequestError::Other` at proxy.rs:571-574
4. `forward_mcp` matches `McpRequestError::AuthRequired` at api.rs:452-455 and returns upstream status code
5. `BAD_GATEWAY` used only for `McpRequestError::Other` at api.rs:456 — not as a catch-all
6. `McpProxy::new()` has `#[allow(dead_code)]` annotation at proxy.rs:230
7. `cargo check -p mcp-preview` — zero errors, zero warnings

All 7 must-haves are now verified. Phase goal is achieved.

---

_Verified: 2026-03-27 (re-verification after plan 61-03 gap closure)_
_Verifier: Claude (gsd-verifier)_
