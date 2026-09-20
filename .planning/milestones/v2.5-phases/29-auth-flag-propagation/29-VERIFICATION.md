---
phase: 29-auth-flag-propagation
verified: 2026-03-12T00:00:00Z
status: passed
score: 10/10 must-haves verified
gaps: []
human_verification:
  - test: "Pass --api-key to cargo pmcp test check and verify Authorization header reaches MCP server"
    expected: "Server receives Authorization: Bearer <key> header on every request"
    why_human: "Requires a running MCP server to observe actual HTTP traffic"
  - test: "Pass --oauth-client-id to cargo pmcp loadtest run and verify OAuth flow executes"
    expected: "Browser opens for PKCE flow; token cached; load test proceeds with auth header"
    why_human: "Requires a running OAuth provider; cannot verify PKCE flow programmatically"
  - test: "Pass --api-key to cargo pmcp preview and verify McpProxy sends Authorization header"
    expected: "Every JSON-RPC request to the MCP server includes Authorization: Bearer <key>"
    why_human: "Requires a running MCP server and network observation"
  - test: "Pass --api-key to cargo pmcp connect --client claude-code and verify generated command"
    expected: "claude mcp add -t http --header 'Authorization: Bearer KEY' server url is printed/executed"
    why_human: "Side-effect behavior (process execution / printed output) not verifiable by grep alone"
---

# Phase 29: Auth Flag Propagation Verification Report

**Phase Goal:** Every command that connects to an MCP server accepts OAuth and API-key authentication flags
**Verified:** 2026-03-12
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1 | AuthFlags struct is importable from commands::flags and usable via #[command(flatten)] | VERIFIED | `pub struct AuthFlags` in flags.rs L101; `#[command(flatten)] auth_flags: AuthFlags` in test/mod.rs, loadtest/mod.rs, main.rs |
| 2 | AuthFlags::resolve() returns correct AuthMethod variant for each input combination | VERIFIED | 7 unit tests in flags.rs L153-289 all pass (233 total tests across crate pass) |
| 3 | resolve_auth_middleware() converts AuthMethod into Option<Arc<HttpMiddlewareChain>> | VERIFIED | Fully implemented in auth.rs L23-73; all three AuthMethod arms handled |
| 4 | --api-key and --oauth-client-id are mutually exclusive at parse level | VERIFIED | `conflicts_with = "oauth_client_id"` on api_key arg (flags.rs L103); clap_rejects_api_key_with_oauth_client_id test passes |
| 5 | cargo pmcp test check accepts --api-key and OAuth flags and authenticates via middleware | VERIFIED | check.rs L46-47 resolves auth; L50-57 passes middleware to ServerTester(None, middleware) |
| 6 | cargo pmcp test run and test generate accept auth flags with degraded-support warning | VERIFIED | run.rs L21-28 and generate.rs L24-31 resolve auth and eprintln! warning when auth != None |
| 7 | cargo pmcp test apps accepts auth flags and authenticates via middleware | VERIFIED | apps.rs L53-64 resolves auth and passes middleware to ServerTester |
| 8 | Loadtest uses shared AuthFlags instead of 6 inline fields; local resolve_auth_middleware deleted | VERIFIED | loadtest/mod.rs L49-51 `#[command(flatten)] auth_flags: AuthFlags`; loadtest/run.rs uses crate::commands::auth::resolve_auth_middleware; no local copy present |
| 9 | cargo pmcp preview accepts auth flags; McpProxy sends Authorization header | VERIFIED | preview.rs L52-86 resolves auth and builds auth_header string; PreviewConfig L64 has auth_header field; proxy.rs L205 stores auth_header; L243-244 applies it in mcp_post() |
| 10 | cargo pmcp schema export and connect accept auth flags and use them | VERIFIED | schema.rs L29-31 AuthFlags on Export variant; L278-311 resolves and passes to send_mcp_request (L685-699); connect.rs L45-48 injects --header for ApiKey; L142-156 adds headers JSON for Cursor |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `cargo-pmcp/src/commands/flags.rs` | AuthFlags struct with resolve() -> AuthMethod | VERIFIED | L71-151: AuthMethod enum, AuthFlags struct, resolve() impl; 7 tests |
| `cargo-pmcp/src/commands/auth.rs` | Shared resolve_auth_middleware() function | VERIFIED | L23-73: async fn resolves all 3 AuthMethod arms; L81-86: resolve_api_key() helper |
| `cargo-pmcp/src/commands/mod.rs` | pub mod auth declaration | VERIFIED | L3: `pub mod auth;` |
| `cargo-pmcp/src/commands/test/mod.rs` | AuthFlags flattened into Check, Run, Generate, Apps variants | VERIFIED | L57, L81, L107, L145: #[command(flatten)] auth_flags: AuthFlags in all 4 variants; L211-276: all dispatch arms pass &auth_flags |
| `cargo-pmcp/src/commands/test/check.rs` | Handler wired with resolve_auth_middleware -> ServerTester | VERIFIED | L46-47 resolve; L50-57 ServerTester::new with (None, middleware) |
| `cargo-pmcp/src/commands/test/run.rs` | Handler accepts auth_flags, warns about unsupported passthrough | VERIFIED | L17 signature; L21-28 resolve + warn |
| `cargo-pmcp/src/commands/test/generate.rs` | Handler accepts auth_flags, warns about unsupported passthrough | VERIFIED | L20 signature; L24-31 resolve + warn |
| `cargo-pmcp/src/commands/test/apps.rs` | Handler wired with resolve_auth_middleware -> ServerTester | VERIFIED | L53-64 resolve + ServerTester with middleware |
| `cargo-pmcp/src/commands/loadtest/mod.rs` | 6 inline auth fields replaced with #[command(flatten)] AuthFlags | VERIFIED | L49-51; LoadtestCommand::Run passes &auth_flags to run::execute_run L112 |
| `cargo-pmcp/src/commands/loadtest/run.rs` | Handler uses shared resolve_auth_middleware via AuthFlags | VERIFIED | L64-66: resolve() + crate::commands::auth::resolve_auth_middleware; no local copy |
| `cargo-pmcp/src/commands/preview.rs` | Handler receives AuthFlags, acquires token, passes header to McpProxy | VERIFIED | L52-99: full auth resolution + PreviewConfig{auth_header} |
| `cargo-pmcp/src/commands/schema.rs` | SchemaCommand::Export receives AuthFlags, adds Authorization header | VERIFIED | L29-31 flatten; L276-311 resolve; L685-699 send_mcp_request auth param applied |
| `cargo-pmcp/src/commands/connect.rs` | Handler receives AuthFlags, includes auth in generated client config | VERIFIED | L14 signature; L45-48 Claude Code --header; L142-156 Cursor headers JSON |
| `cargo-pmcp/src/main.rs` | Preview and Connect command variants include auth_flags field | VERIFIED | L126: Connect auth_flags; L224: Preview auth_flags; L362 and L408: both dispatch arms pass &auth_flags |
| `crates/mcp-preview/src/proxy.rs` | McpProxy accepts optional auth_header and applies to all requests | VERIFIED | L200-227: struct field + new_with_auth(); L237-247: mcp_post() applies header |
| `crates/mcp-preview/src/server.rs` | PreviewConfig includes optional auth_header field | VERIFIED | L64: `pub auth_header: Option<String>`; L95: McpProxy::new_with_auth(&config.mcp_url, config.auth_header.clone()) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| cargo-pmcp/src/commands/flags.rs | cargo-pmcp/src/commands/auth.rs | AuthMethod import | WIRED | auth.rs L13: `use super::flags::AuthMethod` |
| cargo-pmcp/src/commands/auth.rs | pmcp::client::oauth | OAuthHelper for OAuth flow | WIRED | auth.rs L47: `use pmcp::client::oauth::{..., OAuthHelper}` |
| cargo-pmcp/src/commands/auth.rs | pmcp::client::oauth_middleware | BearerToken for API key auth | WIRED | auth.rs L31: `use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware}` |
| cargo-pmcp/src/commands/test/mod.rs | cargo-pmcp/src/commands/flags.rs | AuthFlags import | WIRED | test/mod.rs L23: `use super::flags::{AuthFlags, ...}` |
| cargo-pmcp/src/commands/test/check.rs | cargo-pmcp/src/commands/auth.rs | resolve_auth_middleware call | WIRED | check.rs L15-16; L47: `auth::resolve_auth_middleware(&url, &auth_method).await?` |
| cargo-pmcp/src/commands/loadtest/run.rs | cargo-pmcp/src/commands/auth.rs | shared auth module replaces local function | WIRED | loadtest/run.rs L11-12; L66: `auth::resolve_auth_middleware(&url, &auth_method).await?` |
| cargo-pmcp/src/main.rs | cargo-pmcp/src/commands/preview.rs | auth_flags passed through execute_command dispatch | WIRED | main.rs L396+L408: auth_flags destructured, &auth_flags passed |
| cargo-pmcp/src/commands/preview.rs | crates/mcp-preview/src/server.rs | auth_header field in PreviewConfig | WIRED | preview.rs L98: `auth_header` field set in PreviewConfig literal |
| crates/mcp-preview/src/server.rs | crates/mcp-preview/src/proxy.rs | McpProxy::new_with_auth | WIRED | server.rs L95: `McpProxy::new_with_auth(&config.mcp_url, config.auth_header.clone())` |
| cargo-pmcp/src/commands/schema.rs | reqwest Authorization header | send_mcp_request receives optional auth header | WIRED | schema.rs L680-699: `auth_header: Option<&str>` param; L698-699 applies header |
| cargo-pmcp/src/commands/connect.rs | Claude Code CLI output | connect_claude_code adds --header Authorization flag | WIRED | connect.rs L45-48: `args.push("--header"); args.push(&header_value)` |
| cargo-pmcp/src/commands/connect.rs | Cursor JSON config output | connect_cursor adds headers object with Authorization | WIRED | connect.rs L142-156: prints `"headers"` JSON block when ApiKey |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AUTH-01 | 29-02 | cargo pmcp test check accepts --api-key and OAuth flags | SATISFIED | check.rs fully wires AuthFlags -> middleware -> ServerTester |
| AUTH-02 | 29-02 | cargo pmcp test run accepts --api-key and OAuth flags | SATISFIED | run.rs accepts auth_flags; resolves and warns about partial passthrough |
| AUTH-03 | 29-02 | cargo pmcp test generate accepts --api-key and OAuth flags | SATISFIED | generate.rs accepts auth_flags; resolves and warns about partial passthrough |
| AUTH-04 | 29-03 | cargo pmcp preview accepts --api-key and OAuth flags | SATISFIED | preview.rs resolves auth; McpProxy sends Authorization header on all requests |
| AUTH-05 | 29-03 | cargo pmcp schema export accepts --api-key and OAuth flags | SATISFIED | schema.rs Export variant flattens AuthFlags; send_mcp_request applies auth header |
| AUTH-06 | 29-03 | cargo pmcp connect accepts --api-key and OAuth flags | SATISFIED | connect.rs resolves AuthFlags; Claude Code --header and Cursor headers JSON |

No orphaned requirements: all 6 AUTH-0[1-6] requirements are claimed by plans 29-02 and 29-03 and fully implemented.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `cargo-pmcp/src/commands/auth.rs` | 80 | `#[allow(dead_code)]` with stale comment "Used by Plan 03 when preview/schema/connect wire up auth" | Info | `resolve_api_key()` is never called by any Plan 03 consumer; plans used direct Bearer string construction instead; comment is factually stale but code is compile-clean. Function itself is not harmful. |

No placeholder stubs, empty handlers, or wiring gaps found.

### Human Verification Required

#### 1. API key authentication on test check

**Test:** Run `cargo pmcp test check https://some-authenticated-server.example.com --api-key sk-test-key` against a real server that requires auth
**Expected:** Server receives `Authorization: Bearer sk-test-key`; check passes. Without the key, check fails with 401.
**Why human:** Requires a live MCP server with authentication enforced.

#### 2. OAuth PKCE flow on loadtest run

**Test:** Run `cargo pmcp loadtest run https://server.example.com --oauth-client-id my-client --oauth-issuer https://auth.example.com` with a config file present
**Expected:** Browser opens for PKCE authorization; token cached in default path; load test executes with OAuth bearer token.
**Why human:** Requires a live OAuth provider; cannot simulate PKCE redirect locally.

#### 3. Preview auth header forwarding

**Test:** Run `cargo pmcp preview https://authenticated-server.example.com --api-key sk-key` and inspect outgoing HTTP requests with a network proxy
**Expected:** Every MCP JSON-RPC request to the server contains `Authorization: Bearer sk-key`.
**Why human:** Requires an actual MCP server and network observation (Wireshark/mitmproxy).

#### 4. Connect Claude Code --header injection

**Test:** Run `cargo pmcp connect my-server claude-code https://server.example.com --api-key sk-key` on a machine with Claude CLI installed
**Expected:** `claude mcp add -t http --header "Authorization: Bearer sk-key" my-server https://server.example.com` executes successfully.
**Why human:** Side-effect behavior (subprocess execution) and requires Claude CLI to be installed.

### Gaps Summary

No gaps found. All 10 observable truths are verified, all 16 artifacts pass all three levels (exists, substantive, wired), all 12 key links are confirmed wired, and all 6 requirements are satisfied.

One info-level anti-pattern: `resolve_api_key()` in auth.rs has a stale `#[allow(dead_code)]` comment claiming it will be consumed by Plan 03, but Plan 03 chose direct Bearer string construction instead. The annotation is harmless (clippy is clean) and the function may be useful in future phases, but the comment no longer reflects reality.

---

_Verified: 2026-03-12_
_Verifier: Claude (gsd-verifier)_
