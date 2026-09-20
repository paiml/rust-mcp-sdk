---
phase: 61-add-oauth-support-to-mcp-preview
plan: 02
subsystem: auth
tags: [oauth, pkce, mcp-preview, browser, popup, web-crypto, oidc-discovery]

# Dependency graph
requires:
  - phase: 61-add-oauth-support-to-mcp-preview
    plan: 01
    provides: McpProxy dynamic auth_header, OAuthPreviewConfig, /api/auth/* endpoints, McpRequestError with AuthRequired
provides:
  - OAuthManager class in index.html with full PKCE popup flow (code_verifier, code_challenge, S256)
  - Login modal shown on 401/403 when OAuth is configured
  - Auth status indicator in header (authenticated/expired states)
  - Re-login button in events panel for expired sessions
  - discover_oauth_endpoints() for OIDC .well-known/openid-configuration discovery
  - CLI OAuth flags wired to OAuthPreviewConfig via preview command
  - Graceful CLI token failure fallback when browser OAuth is available
affects: [mcp-preview, cargo-pmcp-preview]

# Tech tracking
tech-stack:
  added: []
  patterns: [browser PKCE via Web Crypto API, OIDC discovery for endpoint resolution, best-effort CLI auth with browser fallback]

key-files:
  created: []
  modified:
    - crates/mcp-preview/assets/index.html
    - cargo-pmcp/src/commands/preview.rs

key-decisions:
  - "OAuthManager instantiated unconditionally in PreviewRuntime constructor; activates only when oauth_config is set"
  - "Auth status indicator appended to existing .status-indicator div rather than creating a new header element"
  - "discover_oauth_endpoints uses issuer URL when provided, falls back to MCP server URL for OIDC discovery"
  - "oauth_config constructed BEFORE resolve_auth_header so browser can handle auth even when CLI fails"
  - "logEvent enhanced with special rendering for authError/authSuccess/authExpired event types"

patterns-established:
  - "Browser PKCE pattern: OAuthManager generates code_verifier/code_challenge in browser, opens popup, exchanges code server-side"
  - "Graceful auth fallback: construct browser OAuth config first, then attempt CLI token; CLI failure non-fatal when browser can handle"

requirements-completed: []

# Metrics
duration: 7min
completed: 2026-03-28
---

# Phase 61 Plan 02: Browser OAuth PKCE Flow and CLI Wiring Summary

**OAuthManager class with browser PKCE popup flow, login modal, auth status indicator, and CLI OAuth flag wiring with OIDC discovery**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-28T03:46:32Z
- **Completed:** 2026-03-28T03:53:45Z
- **Tasks:** 3 (2 auto + 1 checkpoint auto-approved)
- **Files modified:** 2

## Accomplishments
- OAuthManager class in index.html implements full PKCE flow: code_verifier generation (crypto.getRandomValues), code_challenge via SHA-256 (crypto.subtle.digest), S256 method
- Login modal overlay shown automatically when 401/403 detected on session init with OAuth configured
- Re-login button rendered in events panel when auth expires mid-session (401/403 on tool call)
- Auth status indicator in header shows "Authenticated" (green) or "Auth expired + Re-login" (red)
- CLI preview command discovers OAuth endpoints via OIDC .well-known/openid-configuration
- CLI token acquisition failure is non-fatal when browser OAuth is available (graceful fallback)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add OAuthManager to index.html with PKCE popup flow, login modal, and auth status** - `d015c38` (feat)
2. **Task 2: Wire CLI OAuth flags to OAuthPreviewConfig in cargo-pmcp preview command** - `8ec6e1a` (feat)
3. **Task 3: Verify complete OAuth flow end-to-end** - auto-approved (checkpoint)

## Files Created/Modified
- `crates/mcp-preview/assets/index.html` - OAuthManager class, OAuth CSS styles, login modal, auth status indicator, 401/403 detection in initSession/executeTool, auth event rendering
- `cargo-pmcp/src/commands/preview.rs` - discover_oauth_endpoints(), OAuth config construction before CLI auth, graceful fallback, AuthMethod::OAuth matching

## Decisions Made
- OAuthManager is always instantiated but only activates when oauth_config is received from /api/config (zero overhead when OAuth not configured)
- Auth status indicator is appended to the existing .status-indicator div to maintain header layout consistency
- OIDC discovery falls back to MCP server URL when no issuer is provided (matches mcp-tester pattern)
- oauth_config is constructed BEFORE calling resolve_auth_header, ensuring browser OAuth is available even if CLI token acquisition fails
- Auth events (authError, authSuccess, authExpired) get special rendering with colored type labels and inline re-login buttons

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- cargo fmt required a second pass after initial write (trailing comma placement in match arms) - fixed by running cargo fmt before committing
- Pre-existing dead_code warning for McpProxy::new() (from Plan 01) - not addressed as it is a public API convenience constructor

## Known Stubs
None - all OAuth flow components are fully implemented and wired.

## User Setup Required
None - no external service configuration required. OAuth flows activate only when --oauth-client-id is provided.

## Next Phase Readiness
- Phase 61 (OAuth support for mcp-preview) is complete across both plans
- Server-side: dynamic auth header, token exchange, callback, status endpoints (Plan 01)
- Browser-side: OAuthManager PKCE flow, login modal, auth status, re-login (Plan 02)
- CLI: OAuth flags wired to OAuthPreviewConfig with OIDC discovery (Plan 02)
- Ready for testing against real OAuth-protected MCP servers

---
*Phase: 61-add-oauth-support-to-mcp-preview*
*Completed: 2026-03-28*
