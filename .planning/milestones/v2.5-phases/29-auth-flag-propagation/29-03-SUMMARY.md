---
phase: 29-auth-flag-propagation
plan: 03
subsystem: cli
tags: [auth, bearer-token, oauth, preview, schema, connect, reqwest, mcp-proxy]

# Dependency graph
requires:
  - phase: 29-auth-flag-propagation (Plan 01)
    provides: AuthFlags struct, AuthMethod enum, resolve() method, resolve_auth_middleware()
provides:
  - McpProxy auth_header field with new_with_auth() constructor
  - PreviewConfig auth_header field for authenticated preview sessions
  - preview command resolves AuthFlags and passes header to McpProxy
  - schema export adds Authorization header to all MCP JSON-RPC requests
  - connect command generates auth-aware Claude Code and Cursor configs
  - All three commands show --api-key and OAuth flags in --help
affects: [30-tester-integration, 32-help-polish]

# Tech tracking
tech-stack:
  added: []
  patterns: [auth_header passthrough to McpProxy, send_mcp_request auth_header param, Claude Code --header flag for auth, Cursor headers object in JSON config]

key-files:
  created: []
  modified:
    - crates/mcp-preview/src/proxy.rs
    - crates/mcp-preview/src/server.rs
    - cargo-pmcp/src/commands/preview.rs
    - cargo-pmcp/src/commands/schema.rs
    - cargo-pmcp/src/commands/connect.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/test/run.rs
    - cargo-pmcp/src/commands/test/generate.rs

key-decisions:
  - "McpProxy uses auth_header string field (not middleware chain) because it uses raw reqwest internally"
  - "OAuth for preview/schema acquires token once at startup via OAuthHelper::get_access_token() -- refresh during long sessions deferred"
  - "connect_inspector ignores auth flags since Inspector manages its own auth"
  - "schema diff does not support auth yet (passes None) -- lower priority"

patterns-established:
  - "Auth header passthrough: resolve AuthMethod -> construct Bearer string -> pass to McpProxy/send_mcp_request"
  - "Claude Code auth: --header 'Authorization: Bearer KEY' flag on claude mcp add"
  - "Cursor auth: headers object with Authorization key in JSON config"

requirements-completed: [AUTH-04, AUTH-05, AUTH-06]

# Metrics
duration: 8min
completed: 2026-03-13
---

# Phase 29 Plan 03: Preview/Schema/Connect Auth Summary

**Auth propagation to preview (McpProxy auth_header), schema export (reqwest auth), and connect (Claude Code --header, Cursor JSON headers) commands**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-13T01:08:53Z
- **Completed:** 2026-03-13T01:17:21Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- McpProxy applies Authorization header to all outbound requests when configured via new_with_auth()
- Schema export sends auth header on initialize, notification, tools/list, resources/list, prompts/list calls
- Connect generates auth-aware configurations for Claude Code (--header flag) and Cursor (headers JSON)
- All three commands display --api-key and --oauth-* flags in --help output
- 242 tests passing across both crates with zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add auth_header to McpProxy and PreviewConfig, wire preview.rs** - `626d26d` (feat)
2. **Task 1 supplement: auth passthrough warning in test run** - `10f392b` (fix)
3. **Task 2: Wire auth into schema export and connect commands** - `285e42f` (feat)

## Files Created/Modified
- `crates/mcp-preview/src/proxy.rs` - McpProxy auth_header field, new_with_auth(), mcp_post() auth
- `crates/mcp-preview/src/server.rs` - PreviewConfig auth_header field, pass to McpProxy
- `cargo-pmcp/src/commands/preview.rs` - Resolve AuthFlags, OAuth token acquisition, pass auth_header
- `cargo-pmcp/src/commands/schema.rs` - AuthFlags on Export variant, send_mcp_request auth param
- `cargo-pmcp/src/commands/connect.rs` - Auth-aware Claude Code and Cursor config generation
- `cargo-pmcp/src/main.rs` - AuthFlags on Preview and Connect command variants
- `cargo-pmcp/src/commands/test/run.rs` - Accept auth_flags param (blocking fix)
- `cargo-pmcp/src/commands/test/generate.rs` - Accept auth_flags param (blocking fix)

## Decisions Made
- McpProxy uses a plain `Option<String>` auth_header field rather than the HttpMiddlewareChain because it uses raw reqwest internally (not the SDK's middleware abstraction)
- OAuth for preview and schema export acquires the token once at startup via `OAuthHelper::get_access_token()` rather than through the middleware chain; token refresh during long-running preview sessions is a separate concern (deferred)
- `connect_inspector` ignores auth flags since MCP Inspector manages its own authentication; passing auth_flags for signature consistency
- `schema diff` does not support auth yet (passes `None` for auth_header) since diff is lower priority

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed test/run.rs and test/generate.rs compile errors**
- **Found during:** Task 1 (cargo check verification)
- **Issue:** test/mod.rs already had auth_flags fields on Run and Generate variants (from parallel 29-02 work in working tree) but run.rs/generate.rs didn't accept the parameter, causing compile failure
- **Fix:** Added `auth_flags: &AuthFlags` parameter to both functions with `let _ = auth_flags` suppression (actual wiring is 29-02's scope)
- **Files modified:** cargo-pmcp/src/commands/test/run.rs, cargo-pmcp/src/commands/test/generate.rs
- **Verification:** `cargo check -p cargo-pmcp` succeeds
- **Committed in:** 626d26d (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Blocking fix was necessary to compile. No scope creep -- the actual auth wiring for test run/generate remains in 29-02's scope.

## Issues Encountered
- Pre-commit hook auto-committed 29-02 work (671924c, a4d7ce0) interleaved with 29-03 commits during execution. These commits contain test check/apps auth wiring from 29-02's scope that was present in the working tree. The 29-03 work is cleanly contained in commits 626d26d, 10f392b, 285e42f.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All server-connecting commands (test, preview, schema export, connect) now accept auth flags
- Phase 30 (tester integration) can build on this auth infrastructure
- Phase 32 (help polish) can finalize help text for auth flags across all commands

## Self-Check: PASSED

All 6 modified files verified present. All 3 task commits (626d26d, 10f392b, 285e42f) verified in git log.

---
*Phase: 29-auth-flag-propagation*
*Completed: 2026-03-13*
