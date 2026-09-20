---
phase: 61-add-oauth-support-to-mcp-preview
plan: 01
subsystem: auth
tags: [oauth, pkce, mcp-preview, proxy, parking_lot, axum]

# Dependency graph
requires:
  - phase: 44-improving-mcp-preview-to-support-chatgpt-version
    provides: mcp-preview server with proxy architecture and config response
provides:
  - McpProxy dynamic auth_header with parking_lot::SyncRwLock and set/has methods
  - McpRequestError enum propagating upstream 401/403 status codes
  - OAuthPreviewConfig struct for browser-based PKCE flow configuration
  - Three /api/auth/* endpoints (token-exchange, callback, status)
  - OAuth callback HTML page with postMessage to parent window
  - OAuthConfigResponse in /api/config for browser-side OAuth awareness
affects: [61-add-oauth-support-to-mcp-preview, cargo-pmcp-preview]

# Tech tracking
tech-stack:
  added: [parking_lot 0.12 (mcp-preview)]
  patterns: [SyncRwLock for runtime-updatable proxy state, McpRequestError for auth-aware error propagation]

key-files:
  created:
    - crates/mcp-preview/src/handlers/auth.rs
    - crates/mcp-preview/assets/auth-callback.html
  modified:
    - crates/mcp-preview/Cargo.toml
    - crates/mcp-preview/src/proxy.rs
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/lib.rs
    - crates/mcp-preview/src/handlers/api.rs
    - crates/mcp-preview/src/handlers/mod.rs
    - cargo-pmcp/src/commands/preview.rs

key-decisions:
  - "Used McpRequestError enum (adapted from plan's ForwardError) since forward_raw did not exist in the actual codebase"
  - "Added auth_header and oauth_config to PreviewConfig (neither existed before, contrary to plan's interface section)"
  - "Used parking_lot = 0.12 directly (not workspace = true) because worktree Cargo.toml lacks workspace.dependencies section"
  - "Updated all public McpProxy methods to return McpRequestError for consistent auth error propagation"

patterns-established:
  - "SyncRwLock pattern: use parking_lot::RwLock for synchronous runtime state updates on McpProxy"
  - "McpRequestError: structured error type distinguishing auth failures from other errors for handler-level propagation"

requirements-completed: []

# Metrics
duration: 8min
completed: 2026-03-28
---

# Phase 61 Plan 01: OAuth Server Infrastructure Summary

**Dynamic proxy auth via parking_lot::SyncRwLock with three /api/auth/* endpoints, McpRequestError for 401/403 propagation, and OAuth callback HTML for browser popup flow**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-28T03:28:37Z
- **Completed:** 2026-03-28T03:36:45Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- McpProxy auth_header is dynamically updatable at runtime via parking_lot::SyncRwLock without server restart
- Three /api/auth/* endpoints (token-exchange, callback, status) enable browser-side OAuth PKCE flow
- McpRequestError propagates upstream 401/403 to browser instead of wrapping as 502
- OAuthPreviewConfig and OAuthConfigResponse expose OAuth config to browser via /api/config
- OAuth callback HTML page posts authorization code to parent window via postMessage

## Task Commits

Each task was committed atomically:

1. **Task 1: Make McpProxy auth_header dynamically updatable and add OAuthPreviewConfig** - `095222c` (feat)
2. **Task 2: Implement auth handlers, callback HTML, update config endpoint and forward_mcp** - `cc06b1e` (feat)

## Files Created/Modified
- `crates/mcp-preview/Cargo.toml` - Added parking_lot dependency
- `crates/mcp-preview/src/proxy.rs` - SyncRwLock auth_header, McpRequestError, base_post helper, new_with_auth
- `crates/mcp-preview/src/server.rs` - OAuthPreviewConfig struct, auth_header/oauth_config on PreviewConfig, auth routes
- `crates/mcp-preview/src/lib.rs` - Export OAuthPreviewConfig
- `crates/mcp-preview/src/handlers/auth.rs` - token_exchange, callback, status handlers
- `crates/mcp-preview/src/handlers/api.rs` - OAuthConfigResponse, oauth_config in ConfigResponse, McpRequestError handling
- `crates/mcp-preview/src/handlers/mod.rs` - pub mod auth
- `crates/mcp-preview/assets/auth-callback.html` - OAuth popup callback page
- `cargo-pmcp/src/commands/preview.rs` - Added auth_header and oauth_config fields to PreviewConfig construction

## Decisions Made
- Adapted plan's ForwardError to McpRequestError since forward_raw did not exist in the actual proxy codebase
- Updated all public McpProxy methods (list_tools, call_tool, list_resources, read_resource) to return McpRequestError for consistent auth propagation
- Used direct parking_lot = "0.12" dependency instead of workspace = true because the worktree root Cargo.toml lacks a workspace.dependencies section
- Added base_post() helper method to centralize auth header injection across all outgoing requests

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adapted ForwardError/forward_raw to McpRequestError/send_request**
- **Found during:** Task 1 (proxy.rs changes)
- **Issue:** Plan assumed forward_raw method and ForwardError existed; the actual proxy uses send_request internally
- **Fix:** Created McpRequestError enum on send_request, propagated through all public methods
- **Files modified:** crates/mcp-preview/src/proxy.rs, crates/mcp-preview/src/handlers/api.rs
- **Verification:** cargo check -p mcp-preview passes, cargo check --workspace passes
- **Committed in:** 095222c, cc06b1e

**2. [Rule 3 - Blocking] Added missing auth_header/oauth_config to PreviewConfig**
- **Found during:** Task 1 (server.rs changes)
- **Issue:** Plan interface section claimed PreviewConfig already had auth_header; it did not
- **Fix:** Added both auth_header and oauth_config fields, updated Default impl and cargo-pmcp construction
- **Files modified:** crates/mcp-preview/src/server.rs, cargo-pmcp/src/commands/preview.rs
- **Verification:** cargo check --workspace passes
- **Committed in:** 095222c

**3. [Rule 3 - Blocking] Changed parking_lot dependency from workspace to direct**
- **Found during:** Task 1 (Cargo.toml changes)
- **Issue:** Plan specified `parking_lot = { workspace = true }` but worktree root Cargo.toml has no workspace.dependencies
- **Fix:** Used `parking_lot = "0.12"` directly
- **Files modified:** crates/mcp-preview/Cargo.toml
- **Verification:** cargo check -p mcp-preview passes
- **Committed in:** 095222c

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary adaptations to the actual codebase state. The plan's interface section was stale. No scope creep.

## Issues Encountered
- One dead_code warning for McpProxy::new() (public API method that is no longer called internally since start() uses new_with_auth). Kept as-is since it's a public convenience constructor.

## Known Stubs
None - all endpoints are fully implemented and wired.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Server-side OAuth infrastructure complete, ready for Plan 02 (browser-side OAuth UI)
- OAuthPreviewConfig needs to be populated by cargo-pmcp preview command (Plan 02 or future work)
- McpProxy auth_header dynamically updatable for browser OAuth flow to inject tokens

---
*Phase: 61-add-oauth-support-to-mcp-preview*
*Completed: 2026-03-28*
