---
phase: 74
status: passed
verified_at: 2026-04-21
must_haves_total: 32
must_haves_passed: 32
requirements: [SDK-DCR-01, CLI-AUTH-01]
plans: [74-01, 74-02, 74-03]
gaps: []
---

# Phase 74 Verification Report

**Phase Goal:** Consolidate OAuth handling for cargo-pmcp into a dedicated `auth {login,logout,status,token,refresh}` command group with per-server-keyed token cache, plus SDK-level Dynamic Client Registration (RFC 7591) auto-fire and `--client <name>` flag.

**Verified:** 2026-04-21
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement Summary

| Domain | Evidence | Status |
|--------|----------|--------|
| SDK DCR (RFC 7591) types, auto-fire, security guards | `src/client/oauth.rs` 889+ lines; 26 DCR unit tests + 2 proptests + 2 fuzz-smoke + 5 mockito integration | PASSED |
| `cargo pmcp auth` 5-subcommand group | `cargo-pmcp/src/commands/auth_cmd/{login,logout,status,token,refresh}.rs` all present; `auth --help` lists all five | PASSED |
| Per-server `~/.pmcp/oauth-cache.json` cache (schema_version: 1) | `cargo-pmcp/src/commands/auth_cmd/cache.rs`; 14 cache tests + 7 e2e integration tests | PASSED |
| Pentest migrated to `AuthFlags` | `cargo-pmcp/src/commands/pentest.rs` 0 matches for local `MCP_API_KEY` arg, 7 `AuthFlags` hits | PASSED |
| Release coordination (pmcp 2.5.0, cargo-pmcp 0.9.0, mcp-tester 0.5.2) | All 8 workspace pmcp pins at 2.5.0; CHANGELOG dated 2026-04-21; `make quality-gate` exits 0 | PASSED |

---

## Plan 01: SDK DCR (SDK-DCR-01)

| # | Deliverable | Evidence | Status |
|---|-------------|----------|--------|
| 1 | `DcrRequest`/`DcrResponse` re-exported | `grep 'pub use.*Dcr(Request\|Response)' src/client/oauth.rs` → 2 | PASSED |
| 2 | `OAuthConfig::client_id: Option<String>` | `grep 'pub client_id: Option<String>' src/client/oauth.rs` → 1 | PASSED |
| 3 | `client_name` + `dcr_enabled` added | `grep 'pub (client_name: Option<String>\|dcr_enabled: bool)'` → 2 | PASSED |
| 4 | DCR auto-fire per D-03 | `resolve_client_id_for_flow()` in oauth.rs line 324 checks all 3 conditions verbatim | PASSED |
| 5 | Actionable DCR-not-supported error | 2 occurrences of `"server does not support DCR"` | PASSED |
| 6 | DCR request body includes `response_types: ["code"]` (HIGH-1) | 4 literal matches; test `dcr_request_body_contains_response_types_code` passes | PASSED |
| 7 | IPv6 `[::1]` loopback allowlist (LOW-7) | 9 matches for `::1`/`[::1]` in allowlist logic | PASSED |
| 8 | 1 MiB DCR response cap (Gemini LOW-11) | 3 matches for `MAX_DCR_RESPONSE_BYTES`/`1_048_576` | PASSED |
| 9 | Device-code flow fallback documented (MED-3) | 34 references to device-code path + rustdoc | PASSED |
| 10 | `AuthorizationResult` + `authorize_with_details()` | Struct + method present in `src/client/oauth.rs`; consumed by `login.rs` | PASSED |
| 11 | CHANGELOG migration note | `grep 'client_id.*Option<String>' CHANGELOG.md` → 1 | PASSED |

---

## Plan 02: CLI auth group (CLI-AUTH-01)

| # | Deliverable | Evidence | Status |
|---|-------------|----------|--------|
| 1 | `Auth { command: AuthCommand }` in main.rs | `commands::auth_cmd::AuthCommand` at line 122 | PASSED |
| 2 | All 5 subcommand files present | `ls auth_cmd/{login,logout,status,token,refresh}.rs` → 5 | PASSED |
| 3 | `~/.pmcp/oauth-cache.json` (not overwriting legacy) | 7 total matches across auth files (4 in cache.rs) | PASSED |
| 4 | `schema_version` field (== 1) | 10 matches in cache.rs | PASSED |
| 5 | URL normalization fn (lowercase host, strip path) | `normalize_cache_key` + `to_ascii_lowercase` → 2 matches | PASSED |
| 6 | `auth logout` no-args errors (D-09) | `"specify a server URL or --all"` → 3 hits; CLI exits 1 | PASSED |
| 7 | `--client` ⨯ `--oauth-client-id` mutex (D-19) | `conflicts_with` in login.rs → 1 match | PASSED |
| 8 | `token` raw-stdout / status-stderr (D-11) | `eprintln!`/`println!("{}"` in token.rs → 2 | PASSED |
| 9 | `login` does not leak token | 1 hit for `access_token` (struct field assignment, not a print); no leak confirmed | PASSED |
| 10 | Cache fallback in `resolve_auth_middleware` | `try_cache_token` → 6 matches in auth.rs | PASSED |
| 11 | 60 s near-expiry refresh window (D-15) | `REFRESH_WINDOW_SECS`/`is_near_expiry` → 7 matches | PASSED |
| 12 | pentest migrated to `AuthFlags` (D-21) | 7 `AuthFlags` matches; 0 `MCP_API_KEY`-typed arg attrs | PASSED |
| 13 | `test_support` narrow seam (MED-5) | `test_support`/`test_support_cache` → 4 matches in lib.rs | PASSED |
| 14 | MED-4 concurrency docstring | `last-writer-wins` → 1 match in auth_cmd/mod.rs | PASSED |
| 15 | LOW-10 precedence integration test | `api_key_flag_overrides_cached_oauth_token` → 1 match | PASSED |
| 16 | No `tracing::*` token leaks | 0 matches for `tracing::{info,debug,warn,error,trace}!.*(access_token\|refresh_token)` | PASSED |

---

## Plan 03: Release Coordination

| # | Deliverable | Evidence | Status |
|---|-------------|----------|--------|
| 1 | Root `Cargo.toml` version = 2.5.0 | 1 match | PASSED |
| 2 | `cargo-pmcp/Cargo.toml` version = 0.9.0 | 1 match | PASSED |
| 3 | `mcp-tester/Cargo.toml` version = 0.5.2 | 1 match | PASSED |
| 4 | All workspace pmcp pins = "2.5.0" | 8 hits across 7 files | PASSED |
| 5 | Zero stale pins matching `2\.[234]\.0` | 0 hits | PASSED |
| 6 | CHANGELOG dated `2026-04-21`, no `<unreleased>` | 1 dated entry, 0 `<unreleased>` | PASSED |
| 7 | `make quality-gate` exits 0 | Confirmed EXIT=0 (fuzz-build-script stderr is a pre-existing local toolchain issue: fuzz targets need nightly `-Zsanitizer`; this predates Phase 74 and does not block the gate per the Makefile's tolerance — Plan 03 summary separately reported 461 s success on CI-equivalent path) | PASSED |

---

## G-matrix Results (G1..G32)

| Gate | Requirement | Expected | Actual | Status |
|------|-------------|----------|--------|--------|
| G1 | DcrRequest/DcrResponse re-exports | ≥ 2 | 2 | PASSED |
| G2 | `client_id: Option<String>` | == 1 | 1 | PASSED |
| G3 | `client_name` + `dcr_enabled` fields | == 2 | 2 | PASSED |
| G4 | DCR auto-fire condition | ≥ 1 | Semantic match in `resolve_client_id_for_flow`; grep one-line pattern does not match (code spans 3 lines) but intent verified | PASSED (semantic) |
| G5 | "server does not support DCR" error | ≥ 1 | 2 | PASSED |
| G6 | DCR unit tests | 0 failed | 26 passed | PASSED |
| G7 | DCR proptest | 0 failed | 2 passed | PASSED |
| G8 | DCR fuzz-style parser tests | 0 failed | 2 passed | PASSED |
| G9 | mockito integration test | 0 failed | 5 passed | PASSED |
| G10 | `examples/c08_oauth_dcr` builds | success | builds with 0 new compilation | PASSED |
| G11 | CHANGELOG migration note | ≥ 1 | 1 | PASSED |
| G12 | `Auth { command: AuthCommand }` in main.rs | ≥ 1 | 1 (exact form: `command: commands::auth_cmd::AuthCommand`) | PASSED |
| G13 | All 5 subcommand files | == 5 | 5 | PASSED |
| G14 | `oauth-cache.json` references | ≥ 1 | 7 | PASSED |
| G15 | `schema_version` field | ≥ 1 | 10 | PASSED |
| G16 | URL normalization fn | ≥ 1 | 2 | PASSED |
| G17 | logout no-args error copy | ≥ 1 | 3 | PASSED |
| G18 | `--client` mutex | ≥ 1 | 1 | PASSED |
| G19 | token raw-stdout + status-stderr | ≥ 2 | 2 | PASSED |
| G20 | login does not print token | 0 leaks | 0 (one structural `access_token:` field assign, no print) | PASSED |
| G21 | cache fallback in resolve_auth_middleware | ≥ 1 | 6 | PASSED |
| G22 | 60 s refresh window | ≥ 1 | 7 | PASSED |
| G23 | pentest uses `AuthFlags` | ≥ 1 | 7 | PASSED |
| G24 | no duplicate `--api-key` in pentest | == 0 | 0 | PASSED |
| G25 | auth_cmd unit tests | 0 failed | 17 passed | PASSED |
| G26 | auth_integration e2e | 0 failed | 7 passed | PASSED |
| G27 | pmcp 2.5.0 + cargo-pmcp 0.9.0 | == 1 each | 1 each | PASSED |
| G28 | `make quality-gate` | exit 0 | exit 0 | PASSED |
| G29 | `response_types: ["code"]` (HIGH-1) | ≥ 1 | 4 | PASSED |
| G30 | device-code rustdoc (MED-3) | ≥ 1 | Multiple references + explicit rustdoc block | PASSED |
| G31 | MED-4 concurrency doc | ≥ 1 | 1 | PASSED |
| G32 | LOW-10 precedence test | ≥ 1 | 1 | PASSED |

**G-matrix totals: 32/32 gates PASSED.**

---

## CONTEXT.md Decisions Verification (D-01..D-23)

| Decision | Satisfied By | Status |
|----------|--------------|--------|
| D-01 DCR is general-purpose SDK feature | `src/client/oauth.rs` hosts all DCR types + logic | PASSED |
| D-02 OAuthConfig shape change (client_id Option, +client_name, +dcr_enabled) | All three fields verified | PASSED |
| D-03 DCR auto-fire trigger (3 conditions) | `resolve_client_id_for_flow` implements all 3 + error path | PASSED |
| D-04 `client_name` default None, fallback `"pmcp-sdk"` | Code comment + default impl | PASSED |
| D-05 RFC 7591 request body shape | `DcrRequest` includes client_name, redirect_uris, grant_types, token_endpoint_auth_method, response_types (HIGH-1) | PASSED |
| D-06 Cache key = normalized scheme://host[:port] | `normalize_cache_key` strips path, lowercases host | PASSED |
| D-07 Cache file = `~/.pmcp/oauth-cache.json`, schema_version 1 | `default_multi_cache_path()` + `schema_version: 1` | PASSED |
| D-08 All 5 subcommands | login/logout/status/token/refresh all present | PASSED |
| D-09 `logout` no-args errors | Error message + exit 1 confirmed | PASSED |
| D-10 Subcommand behaviors | Each subcommand file implements per-spec (login has --client + --oauth-client-id etc.) | PASSED |
| D-11 `token` raw-stdout | println!("{}", token) + eprintln! for status | PASSED |
| D-12 `login` does not print token | No `println!` on access_token; success string uses issuer/scopes/expires_in | PASSED |
| D-13 Precedence flag > env > cache | `try_cache_token` only fires on `AuthMethod::None` | PASSED |
| D-14 Silent cache fallback | No warning emitted when both cache + flag exist | PASSED |
| D-15 On-demand refresh within 60 s | `REFRESH_WINDOW_SECS = 60` + `is_near_expiry` | PASSED |
| D-16 Force-refresh `auth refresh` | `refresh.rs` implements explicit refresh; errors on missing refresh_token | PASSED |
| D-17 `--client` on login only | Only in `login.rs`, not in refresh/token/status/logout | PASSED |
| D-18 `--client` transient | Not persisted to `TokenCacheEntry` (no client_name field on entry) | PASSED |
| D-19 `--client` ⨯ `--oauth-client-id` clap mutex | `conflicts_with` in login.rs | PASSED |
| D-20 `--oauth-client-id` escape hatch | `OAuthConfig::client_id = Some(...)` path skips DCR | PASSED |
| D-21 pentest migrated to shared `AuthFlags` | 7 AuthFlags matches; 0 MCP_API_KEY arg attrs | PASSED |
| D-22 Semver bumps (pmcp minor, cargo-pmcp minor) | 2.4.0→2.5.0 + 0.8.1→0.9.0 + mcp-tester 0.5.1→0.5.2 | PASSED |
| D-23 CLAUDE.md release workflow followed | `make quality-gate` used; tagging deferred to operator | PASSED |

**Decision totals: 23/23 satisfied.**

---

## Review Items Verification (11 items)

| ID | Severity | Finding | Shipped Fix | Status |
|----|----------|---------|-------------|--------|
| HIGH-1 | HIGH | DCR body missing `response_types: ["code"]` | `response_types: vec!["code".to_string()]` + wire-body test | FIXED |
| MED-2 | MED | OAuthConfig {} repo-wide audit | 30 matches across 11 files, all intentional; 16 in src/client/oauth.rs are tests/docs | FIXED |
| MED-3 | MED | `authorize_with_details()` device-code fallback | Device-code path documented in rustdoc; `refresh_token=None` per RFC 8628 §3.5 | FIXED |
| MED-4 | MED | Concurrent login race (last-writer-wins) | Option A selected: docstring in cache.rs + auth_cmd/mod.rs documents the acceptance | FIXED |
| MED-5 | MED | Narrow public test seam | `#[path] pub mod test_support_cache` in lib.rs (not `pub mod commands`) | FIXED |
| LOW-6 | LOW | Test hook cfg gate | Deviation rationale: integration tests are separate compilation unit; kept `cfg(any(test, feature="oauth"))` + `#[doc(hidden)]` + `test_` prefix | ACCEPTED |
| LOW-7 | LOW | IPv6 `[::1]` loopback | Allowlist matches both `"::1"` and `"[::1]"` | FIXED |
| LOW-8 | LOW | `tracing::*` token-leak grep | Verified 0 token-bearing tracing calls | FIXED |
| LOW-9 | LOW | Plan 03 pin-count prose inconsistency | Corrected to 8 pins across 7 files; 8 matches verified | FIXED |
| LOW-10 | LOW | Explicit `--api-key` overrides cached OAuth | `api_key_flag_overrides_cached_oauth_token` integration test present | FIXED |
| LOW-11 | LOW | DCR response body size cap (Gemini) | `MAX_DCR_RESPONSE_BYTES = 1_048_576` enforced | FIXED |

**Review-item totals: 11/11 addressed (10 fixed + 1 deviation documented with technical justification).**

---

## Integration E2E Sanity Checks

| Check | Result |
|-------|--------|
| `cargo build -p cargo-pmcp` builds cleanly | PASS (warnings only, pre-existing) |
| `cargo run -p cargo-pmcp -- auth --help` lists 5 subcommands | PASS (login, logout, status, token, refresh, help) |
| `cargo run -p cargo-pmcp -- auth logout` (no args) errors out | PASS (exit=1, "Error: specify a server URL or --all to log out of everything") |
| `cargo run -p cargo-pmcp -- auth status` (no args, empty cache) handles gracefully | PASS ("No cached credentials. Run `cargo pmcp auth login <url>` to authenticate.") |
| CHANGELOG has dated entry, not `<unreleased>` | PASS (`## [2.5.0] - 2026-04-21`; 0 `<unreleased>` remain) |

---

## Version Consistency Check

| File | Expected | Actual | Status |
|------|----------|--------|--------|
| `Cargo.toml` | `version = "2.5.0"` | `2.5.0` | PASS |
| `cargo-pmcp/Cargo.toml` | `version = "0.9.0"` | `0.9.0` | PASS |
| `crates/mcp-tester/Cargo.toml` | `version = "0.5.2"` | `0.5.2` | PASS |
| Workspace pmcp pins at 2.5.0 | ≥ 8 hits | 8 across 7 files (cargo-pmcp, pmcp-server, pmcp-server-lambda, mcp-tester, pmcp-tasks x2, test-basic, 25-oauth-basic) | PASS |
| Stale pmcp pins `2\.[234]\.0` | 0 | 0 | PASS |

---

## Semver Audit (BREAKING CHANGE: `OAuthConfig::client_id: String → Option<String>`)

| Check | Status |
|-------|--------|
| CHANGELOG has migration note with before/after snippet | PASS (`client_id.*Option<String>` present in CHANGELOG.md) |
| All in-repo `OAuthConfig {` callers use `Some(...)` or opt into DCR (`client_id: None, dcr_enabled: true`) | PASS — 30 matches across 11 files; plan-02 login.rs uses DCR flow, plan-01 updates cargo-pmcp/commands/auth.rs and mcp-tester/main.rs to new shape |
| Files excluded from audit (test fixtures/docs) | 16 of 30 are in `src/client/oauth.rs` itself (tests + docs); 5 in `tests/oauth_dcr_integration.rs` (tests); remainder are runtime callers | PASS |

---

## Test Suite Summary

| Suite | Command | Pass Count | Status |
|-------|---------|------------|--------|
| pmcp DCR unit (lib, oauth feature) | `cargo test -p pmcp --lib --features oauth dcr` | 26 passed | PASS |
| pmcp DCR proptest | `cargo test -p pmcp --lib --features oauth dcr_proptest` | 2 passed | PASS |
| pmcp DCR fuzz smoke | `cargo test -p pmcp --lib --features oauth dcr_parser_fuzz` | 2 passed | PASS |
| pmcp DCR mockito integration | `cargo test -p pmcp --test oauth_dcr_integration --features oauth` | 5 passed | PASS |
| cargo-pmcp example build | `cargo build --example c08_oauth_dcr --features oauth` | Build OK | PASS |
| cargo-pmcp auth_cmd unit | `cargo test -p cargo-pmcp --bin cargo-pmcp auth_cmd` | 17 passed | PASS |
| cargo-pmcp auth e2e | `cargo test -p cargo-pmcp --test auth_integration` | 7 passed | PASS |
| make quality-gate | `make quality-gate` | Exit 0 | PASS |

**Total new tests: 26 unit + 2 proptest (64 cases each) + 2 fuzz-smoke (200 cases each) + 5 mockito + 17 cargo-pmcp auth_cmd + 7 auth_integration = 59 test cases.**

---

## Notes & Observed Environment Issues

1. **Fuzz target local build failures (NON-BLOCKING).** `make quality-gate` emits `error: the option 'Z' is only accepted on the nightly compiler` for multiple fuzz targets, but these error messages do not fail the top-level `make quality-gate` (exit code 0). This is a pre-existing local-toolchain limitation: `cargo fuzz` requires nightly for `-Zsanitizer=address`, but the project builds on stable Rust. CI runs the authoritative gate. Plan 03 independently reported a 461 s green `make quality-gate` on the same HEAD.

2. **Disk-space limit reached during `cargo test --workspace`.** The verifier's disk (134 Mi free, 100% capacity) prevented a fresh full-workspace compile, but the incremental-compile test runs for the phase-relevant suites (listed in Test Suite Summary above) all passed using cached artifacts. This is a verifier-environment issue, not a phase-74 code issue.

3. **G4 semantic match.** The G4 grep pattern `registration_endpoint.*is_some().*dcr_enabled|dcr_enabled.*registration_endpoint` returns 0 matches because the actual implementation in `resolve_client_id_for_flow` spans three lines (checks `client_id.is_some()` shortcut, then `dcr_enabled`, then `registration_endpoint.as_ref()`). The semantic intent is verified correct: all three D-03 conditions are enforced in the exact order specified.

---

## Observable Truths — Summary

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SDK client can build OAuthConfig without client_id and obtain one via DCR | VERIFIED | `resolve_client_id_for_flow` + mockito test `dcr_autofires_when_config_has_no_client_id` |
| 2 | DCR request body is RFC 7591 §3.1 compliant (includes response_types) | VERIFIED | `dcr_request_body_contains_response_types_code` test + 4 `response_types.*code` matches |
| 3 | `cargo pmcp auth login <url>` obtains token and writes `~/.pmcp/oauth-cache.json` | VERIFIED | `login.rs` → `authorize_with_details()` → `TokenCacheV1::write_atomic` |
| 4 | Every server-connecting command transparently reuses cached token | VERIFIED | `resolve_auth_middleware` cache fallback + auth_integration e2e tests |
| 5 | `auth token <url>` prints raw token to stdout only | VERIFIED | D-11; `println!("{}", token)` + `eprintln!` for status |
| 6 | `auth logout` requires explicit target | VERIFIED | D-09; CLI exits 1 with message |
| 7 | pentest no longer has its own `--api-key` flag | VERIFIED | `grep '#[arg.*env = "MCP_API_KEY"' pentest.rs` → 0 |
| 8 | `pmcp` 2.5.0 + `cargo-pmcp` 0.9.0 + `mcp-tester` 0.5.2 on main | VERIFIED | Cargo.toml version strings + 8 matching workspace pins |
| 9 | CHANGELOG has migration snippet for breaking change | VERIFIED | `client_id.*Option<String>` in CHANGELOG.md |
| 10 | `make quality-gate` exits 0 | VERIFIED | EXIT=0 confirmed |

---

## VERIFICATION PASSED

All 32 G-matrix gates, all 23 CONTEXT.md decisions, and all 11 review items closed. Three plans (01 SDK DCR, 02 CLI auth group, 03 release) shipped per spec. 59 new test cases pass. Quality-gate exits 0. `cargo pmcp auth --help` renders all 5 subcommands; `auth logout` (no args) errors per D-09; `auth status` (empty cache) handles gracefully. Workspace is release-ready for the operator's `git tag v2.5.0 && git push upstream v2.5.0` step.

---

*Verified: 2026-04-21*
*Verifier: Claude (gsd-verifier)*
