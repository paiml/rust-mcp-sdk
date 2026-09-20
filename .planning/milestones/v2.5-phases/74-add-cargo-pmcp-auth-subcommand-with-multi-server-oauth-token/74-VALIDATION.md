---
phase: 74
slug: add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-21
---

# Phase 74 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Sourced from 74-RESEARCH.md §"Validation Architecture" (27-row matrix) and 74-CONTEXT.md (23 locked decisions).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (workspace-wide) + `cargo fuzz` (proptest for fuzz coverage of DCR response parser) |
| **Config file** | `Cargo.toml` (workspace root + per-crate) |
| **Quick run command** | `cargo test -p pmcp --lib oauth` (SDK unit tests, fast) + `cargo test -p cargo-pmcp --lib auth_cmd` (CLI unit tests) |
| **Full suite command** | `make quality-gate` (matches CI exactly — fmt, clippy pedantic + nursery, build, test, audit) |
| **Estimated runtime** | Quick: ~30s. Full: ~6-8min. |

---

## Sampling Rate

- **After every task commit:** Run the quick command relevant to the edited crate.
- **After every plan wave:** Run `make quality-gate` to match CI.
- **Before `/gsd-verify-work`:** Full suite must be green.
- **Max feedback latency:** 30 seconds for the quick run.

---

## Per-Task Verification Map

Task IDs are placeholders (`74-{plan}-{task}`) — populated during planning. Each row below maps to a guardrail/grep that PLAN.md's verification section must check.

| Guardrail | Plan | Wave | Requirement | Threat Ref | Behavior | Test Type | Automated Command | Expected | Status |
|-----------|------|------|-------------|------------|----------|-----------|-------------------|----------|--------|
| G1-sdk-dcr-types | 01 | 1 | SDK-DCR-01 | T-74-A | DCR request/response types re-exported in `pmcp::client::oauth` | grep | `grep -c 'pub use.*Dcr\(Request\|Response\)' src/client/oauth.rs` | `>= 2` | ⬜ pending |
| G2-sdk-oauthconfig-option | 01 | 1 | SDK-DCR-01 | — | `OAuthConfig::client_id` is `Option<String>` | grep | `grep -c 'pub client_id: Option<String>' src/client/oauth.rs` | `== 1` | ⬜ pending |
| G3-sdk-oauthconfig-fields | 01 | 1 | SDK-DCR-01 | — | `client_name` + `dcr_enabled` added to OAuthConfig | grep | `grep -cE 'pub (client_name: Option<String>\|dcr_enabled: bool)' src/client/oauth.rs` | `== 2` | ⬜ pending |
| G4-sdk-dcr-autofire | 01 | 1 | SDK-DCR-01 | T-74-B | DCR fires when dcr_enabled && client_id.is_none() && registration_endpoint is Some | grep | `grep -cE 'registration_endpoint.*is_some\(\).*dcr_enabled\|dcr_enabled.*registration_endpoint' src/client/oauth.rs` | `>= 1` | ⬜ pending |
| G5-sdk-dcr-no-support-error | 01 | 1 | SDK-DCR-01 | — | Actionable error when DCR needed but server doesn't support it | grep | `grep -c 'server does not support DCR' src/client/oauth.rs` | `>= 1` | ⬜ pending |
| G6-sdk-unit-tests | 01 | 2 | SDK-DCR-01 | — | Unit tests for DCR config defaults, request builder, error paths | test | `cargo test -p pmcp --lib dcr` | `ok.*0 failed` | ⬜ pending |
| G7-sdk-property-tests | 01 | 2 | SDK-DCR-01 | — | Proptest for DCR request serde round-trip + OAuthConfig builder | test | `cargo test -p pmcp --lib oauth::dcr_proptest` | `ok.*0 failed` | ⬜ pending |
| G8-sdk-fuzz-response-parser | 01 | 2 | SDK-DCR-01 | T-74-C | Fuzz DCR response parser (malformed JSON, missing fields, extra fields) | fuzz | `cargo test -p pmcp --lib dcr::parser_fuzz` | `ok.*0 failed` | ⬜ pending |
| G9-sdk-integration-mockito | 01 | 2 | SDK-DCR-01 | — | End-to-end DCR integration test using mockito mock server | test | `cargo test -p pmcp --test dcr_integration` | `ok.*0 failed` | ⬜ pending |
| G10-sdk-example | 01 | 2 | SDK-DCR-01 | — | Working example demonstrating library-user DCR flow | build | `cargo build --example oauth_dcr` | `success` | ⬜ pending |
| G11-sdk-changelog-migration | 01 | 3 | SDK-DCR-01 | — | CHANGELOG.md has migration note for `client_id: String → Option<String>` | grep | `grep -c 'client_id.*Option<String>' CHANGELOG.md` | `>= 1` | ⬜ pending |
| G12-cli-auth-variant | 02 | 3 | CLI-AUTH-01 | — | Top-level `Auth` variant in cargo-pmcp Commands enum | grep | `grep -c 'Auth.*AuthCommand' cargo-pmcp/src/main.rs` | `>= 1` | ⬜ pending |
| G13-cli-subcommands-5 | 02 | 3 | CLI-AUTH-01 | — | All 5 subcommands present (login, logout, status, token, refresh) | grep | `ls cargo-pmcp/src/commands/auth_cmd/{login,logout,status,token,refresh}.rs \| wc -l` | `== 5` | ⬜ pending |
| G14-cli-cache-v2-path | 02 | 3 | CLI-AUTH-01 | — | Cache file path = ~/.pmcp/oauth-cache.json (not overwriting legacy oauth-tokens.json) | grep | `grep -c 'oauth-cache\.json' src/client/oauth.rs cargo-pmcp/src/` | `>= 1` | ⬜ pending |
| G15-cli-cache-schema-version | 02 | 3 | CLI-AUTH-01 | — | Cache file has schema_version field | grep | `grep -c 'schema_version' src/client/oauth.rs` | `>= 1` | ⬜ pending |
| G16-cli-url-normalize | 02 | 3 | CLI-AUTH-01 | T-74-D | URL normalization: lowercase host, strip path, strip trailing slash | grep | `grep -cE 'fn normalize_cache_key\|to_lowercase.*host' src/client/oauth.rs` | `>= 1` | ⬜ pending |
| G17-cli-logout-no-args-errors | 02 | 4 | CLI-AUTH-01 | — | `auth logout` with no args errors out | grep | `grep -c 'specify a server URL or --all' cargo-pmcp/src/commands/auth_cmd/logout.rs` | `>= 1` | ⬜ pending |
| G18-cli-client-flag-mutex | 02 | 4 | CLI-AUTH-01 | — | `--client` and `--oauth-client-id` are mutually exclusive in clap | grep | `grep -c 'conflicts_with.*oauth_client_id\|conflicts_with.*client' cargo-pmcp/src/commands/auth_cmd/login.rs` | `>= 1` | ⬜ pending |
| G19-cli-token-stdout-only | 02 | 4 | CLI-AUTH-01 | T-74-E | `auth token` prints ONLY raw token to stdout; status/errors to stderr | grep | `grep -cE 'eprintln\!\|println\!\("\{\}"' cargo-pmcp/src/commands/auth_cmd/token.rs` | `>= 2` | ⬜ pending |
| G20-cli-login-no-token-output | 02 | 4 | CLI-AUTH-01 | T-74-E | `auth login` success output does NOT contain the token | grep | `grep -c 'access_token\|token_value' cargo-pmcp/src/commands/auth_cmd/login.rs \| awk '$1==0'` | expected `0` occurrences of token leak | ⬜ pending |
| G21-cli-precedence-flag-env-cache | 02 | 4 | CLI-AUTH-01 | — | `resolve_auth_middleware` checks flag, then env (already done), then cache in this order | grep | `grep -cE 'AuthMethod::None.*cache\|cache_lookup.*normalize' cargo-pmcp/src/commands/auth.rs` | `>= 1` | ⬜ pending |
| G22-cli-refresh-60s-window | 02 | 4 | CLI-AUTH-01 | — | On-demand refresh triggers when within 60s of expiry | grep | `grep -cE '60[[:space:]]*\+\|const REFRESH_WINDOW.*60\|expires_at.*60' src/client/oauth.rs cargo-pmcp/src/` | `>= 1` | ⬜ pending |
| G23-cli-pentest-flagmigrate | 02 | 5 | CLI-AUTH-01 | — | pentest.rs migrated from private --api-key to shared AuthFlags | grep | `grep -c 'AuthFlags' cargo-pmcp/src/commands/pentest.rs` | `>= 1` | ⬜ pending |
| G24-cli-pentest-no-duplicate-apikey | 02 | 5 | CLI-AUTH-01 | — | pentest.rs no longer has its own --api-key declaration | grep | `grep -cE '#\[arg.*env = \"MCP_API_KEY\"' cargo-pmcp/src/commands/pentest.rs` | `== 0` | ⬜ pending |
| G25-cli-unit-tests | 02 | 5 | CLI-AUTH-01 | — | Unit tests for each subcommand (login/logout/status/token/refresh) pass | test | `cargo test -p cargo-pmcp --lib auth_cmd` | `ok.*0 failed` | ⬜ pending |
| G26-cli-integration | 02 | 5 | CLI-AUTH-01 | — | Integration test: login → status → token → logout against mockito | test | `cargo test -p cargo-pmcp --test auth_e2e` | `ok.*0 failed` | ⬜ pending |
| G27-release-semver | 03 | 6 | SDK-DCR-01, CLI-AUTH-01 | — | pmcp minor bump + cargo-pmcp 0.8.1 → 0.9.0 in Cargo.toml; cargo-pmcp pmcp dep updated | grep | `grep -cE '^version = \"0\.9\.0\"' cargo-pmcp/Cargo.toml` | `== 1` | ⬜ pending |
| G28-quality-gate | 03 | 6 | SDK-DCR-01, CLI-AUTH-01 | — | `make quality-gate` passes (fmt + clippy + build + test + audit) | build | `make quality-gate` | `exit 0` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp/tests/dcr_integration.rs` — integration test file stubs (uses existing `mockito` dev-dep)
- [ ] `cargo-pmcp/tests/auth_e2e.rs` — integration test file stubs
- [ ] `examples/oauth_dcr/` (or single-file `examples/oauth_dcr.rs`) — ALWAYS example stub
- [ ] No framework install needed — `cargo test`, `proptest`, `mockito` already in workspace deps

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end login against live pmcp.run with `--client claude-desktop` | CLI-AUTH-01 | Requires browser interaction + live pmcp.run staging endpoint; cannot be fully automated | (1) Run `cargo run -p cargo-pmcp -- auth login https://<pmcp.run-staging> --client claude-desktop`; (2) Confirm browser opens to Cognito-hosted login page; (3) Confirm the login page visually matches the claude-desktop brand (logo, colors per pmcp.run's `ClientTypeMatcher::display_name`); (4) Complete login; (5) Verify `cargo run -p cargo-pmcp -- auth status https://<pmcp.run-staging>` shows the cached entry with issuer + scopes + a valid expires_in. |
| Cross-platform cache file permissions (0600 on Unix) | CLI-AUTH-01 | `chmod 600` semantics differ by platform; manual verify on macOS, Linux, Windows | On macOS/Linux: `stat -c '%a' ~/.pmcp/oauth-cache.json` → expect `600`. On Windows: file is user-only via ACL — inspect via PowerShell `Get-Acl`. |

---

## Threat Model References

Per plan, `<threat_model>` blocks should cover at minimum:

| Ref | Threat | Mitigation |
|-----|--------|------------|
| T-74-A | DCR request to attacker-controlled registration_endpoint (discovery spoofing) | Reject non-HTTPS registration_endpoints except localhost; validate response TLS chain |
| T-74-B | DCR success with attacker-returned client_id causing PKCE-only leak | Tie PKCE challenge to the DCR-returned client_id; fail loudly on mismatch during token exchange |
| T-74-C | DCR response parser panics on malformed input (DoS, info leak) | Fuzz parser; all decode errors return typed errors, never panic |
| T-74-D | Cache key collisions via URL-normalization edge cases (e.g., IDN, mixed case, trailing dot) | Canonicalize via `url::Url` parse, explicit lowercase, strip trailing slash AND trailing dot on host |
| T-74-E | Access token leak via stdout/logs (shell history, screen share, CI logs) | `auth login` never prints token; `auth token` prints ONLY token (to stdout); all logging uses `***redacted***` for token values |

---

## Plans Expected (from research recommendation)

- **Plan 01:** SDK DCR (src/client/oauth.rs, OAuthConfig refactor, DcrRequest/Response re-export, auto-fire logic, tests, example, CHANGELOG migration note)
- **Plan 02:** CLI auth command group + multi-server cache (new `commands/auth_cmd/` module, `resolve_auth_middleware` cache fallback, pentest.rs flag migration)
- **Plan 03:** Release coordination + final quality-gate (version bumps, CHANGELOG entries, workspace dependency pin updates)

---

*Last updated: 2026-04-21*
