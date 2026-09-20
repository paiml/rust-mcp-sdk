---
phase: 74
plan: 02
subsystem: cargo-pmcp-cli
tags: [cargo-pmcp, cli, oauth, cache, multi-server, clap, mockito, tempfile]
requires:
  - phase: 74-01
    provides: "OAuthConfig::client_id: Option<String>, AuthorizationResult, authorize_with_details(), DCR auto-fire"
provides:
  - "cargo pmcp auth {login,logout,status,token,refresh} subcommand group"
  - "~/.pmcp/oauth-cache.json multi-server TokenCacheV1 (schema_version=1)"
  - "resolve_auth_middleware/header cache fallback on AuthMethod::None (D-13 precedence)"
  - "D-15 transparent refresh within 60s of expiry; D-16 force-refresh handler"
  - "cargo_pmcp::test_support::cache narrow integration-test seam (review MED-5)"
  - "pentest.rs migrated to shared AuthFlags (D-21); legacy --api-key preserved"
affects:
  - "Every server-connecting cargo-pmcp command (test/*, connect, preview, schema, dev, loadtest/run, pentest) now picks up cached OAuth tokens without re-authenticating"
  - "CLI users gain `auth` as a first-class subcommand on `cargo pmcp --help`"
tech-stack:
  added:
    - "tempfile 3 (promoted from dev-deps to regular deps for atomic cache writes)"
    - "url 2 (URL normalization for cache keys)"
    - "mockito 1 (dev-dep, integration-test HTTP mock)"
  patterns:
    - "Atomic file write via tempfile::NamedTempFile::persist + per-file chmod (0o600) + parent-dir chmod (0o700)"
    - "Schema-versioned JSON on-disk cache with explicit `schema_version: u32` guard"
    - "URL normalization (lowercase host, strip path/trailing-slash/default-port) as cache key"
    - "Narrow integration-test seam via #[path] on a top-level module (review MED-5)"
    - "Token-leak grep gate G20: regex over println!/eprintln!/tracing::* for access_token|refresh_token"
key-files:
  created:
    - cargo-pmcp/src/commands/auth_cmd/mod.rs
    - cargo-pmcp/src/commands/auth_cmd/cache.rs
    - cargo-pmcp/src/commands/auth_cmd/login.rs
    - cargo-pmcp/src/commands/auth_cmd/logout.rs
    - cargo-pmcp/src/commands/auth_cmd/status.rs
    - cargo-pmcp/src/commands/auth_cmd/token.rs
    - cargo-pmcp/src/commands/auth_cmd/refresh.rs
    - cargo-pmcp/tests/auth_integration.rs
    - .planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-02-SUMMARY.md
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/lib.rs
    - cargo-pmcp/src/main.rs
    - cargo-pmcp/src/commands/mod.rs
    - cargo-pmcp/src/commands/auth.rs
    - cargo-pmcp/src/commands/pentest.rs
key-decisions:
  - "Cache path `~/.pmcp/oauth-cache.json` (distinct from legacy SDK single-server path `~/.pmcp/oauth-tokens.json`) — keeps multi-server CLI state separate from per-helper SDK cache (D-07)"
  - "Narrow `test_support` seam via `#[path = \"commands/auth_cmd/cache.rs\"]` on a top-level `test_support_cache` module, rather than `pub mod commands;` — integration tests need only `cache`, and the bin-only `commands/` tree has cross-deps on `crate::deployment`/`crate::utils` that would otherwise force the lib target to pull in the entire CLI (review MED-5)"
  - "Token-leak grep gate G20 extended to cover `tracing::{info,debug,warn,error,trace}!` across ALL auth_cmd/* files, not just login.rs (review LOW-8)"
  - "T-74-F (concurrent login race) accepted as last-writer-wins, matching gh/aws conventions; documented in both `cache.rs` rustdoc and `auth_cmd/mod.rs` module rustdoc (review MED-4 Option A)"
  - "`eprintln!` status line in token.rs ('Refreshing cached token for …') satisfies G19 two-output-paths gate while keeping stdout-only discipline for the token value (D-11)"
patterns-established:
  - "Schema-versioned JSON cache: `{schema_version: 1, entries: { <normalized_url>: Entry }}` — reader rejects wrong version with actionable upgrade message"
  - "BearerToken chain helper (`bearer_chain`) shared between AuthMethod::None cache-hit path and AuthMethod::ApiKey path — single code path for all bearer auth"
  - "authorize_with_details() consumer pattern: pull full AuthorizationResult into a domain-specific persistence struct, not just the access_token"
requirements-completed: [CLI-AUTH-01]
duration: "36 min"
completed: 2026-04-21
---

# Phase 74 Plan 02: cargo pmcp auth Command Group + Multi-Server Token Cache Summary

**`cargo pmcp auth {login,logout,status,token,refresh}` with per-server OAuth cache at `~/.pmcp/oauth-cache.json`, transparent bearer-token reuse across every server-connecting CLI command, and pentest migrated to shared AuthFlags.**

## Performance

- **Duration:** ~36 min (Task 2.1 → Task 2.4 atomic commits)
- **Tasks:** 4 of 4 complete
- **Commits:** 4 task commits + 1 metadata commit
- **Files created:** 8 (7 source + 1 summary)
- **Files modified:** 6

## Accomplishments

- **`cargo pmcp auth` subcommand group** with 5 subcommands, all wired through clap `#[command(subcommand)]` on `Commands::Auth`. `auth --help` lists login/logout/status/token/refresh with D-09/D-11/D-12/D-19 semantics.
- **TokenCacheV1 schema** (`schema_version: 1`) stored at `~/.pmcp/oauth-cache.json`. Atomic write via `tempfile::NamedTempFile::persist` with `0o600` file perms and `0o700` parent dir perms (T-74-G). `normalize_cache_key` collapses trailing-slash, lowercases host, strips default ports and path (T-74-D).
- **Transparent cache fallback:** `resolve_auth_middleware` and `resolve_auth_header` now consult the cache on `AuthMethod::None`, auto-refresh within 60s of expiry (D-15), and short-circuit explicit flags (D-13 precedence).
- **D-16 force-refresh handler** using the cached `refresh_token` + OIDC discovery to locate the token endpoint; errors actionably when `refresh_token` is absent (Pitfall 5).
- **Blocker #6 fix verified:** login.rs calls `OAuthHelper::authorize_with_details()` (Plan 01 Task 1.2b) and persists the full `AuthorizationResult` — access_token, refresh_token, expires_at, scopes, effective issuer, effective client_id. No `refresh_token: None` / `expires_at: None` hard-coded literals remain.
- **pentest.rs migration (D-21):** removed the local `#[arg(long, env = "MCP_API_KEY")]` field; pentest now consumes `#[command(flatten)] auth_flags: AuthFlags` and routes through `resolve_auth_middleware`. `--api-key` backward-compat preserved by matching on `AuthMethod::ApiKey` and passing the key to `ServerTester::new`.
- **Review MED-5 narrow integration-test seam:** `cargo_pmcp::test_support::cache` re-exports `cache.rs` via `#[path]` on a top-level module, keeping the bin-only `commands/` tree out of the lib target. Acceptance: `grep -c '^pub mod commands' lib.rs == 0`, `grep -c '^pub mod test_support' lib.rs == 2`.
- **24 new test cases** green: 14 cache unit tests (normalize x2, read-missing, schema-reject, roundtrip, perms, is_near_expiry x2, refresh-error, default-path, 2 proptests, 2 login clap gates, 1 logout clap-mutex + 1 logout no-args) + 2 token/refresh placeholders + 7 integration tests (T-74-D, roundtrip, near-expiry window, cache-path suffix, CLI D-09, D-11 stdout discipline, LOW-10 api_key-over-cache precedence).

## Task Commits

1. **Task 2.1: auth_cmd scaffold + TokenCacheV1 cache** — `aa934380` (feat)
2. **Task 2.2: wire Auth CLI subcommand + cache fallback** — `fc96182f` (feat)
3. **Task 2.3: implement 5 auth subcommand bodies** — `7ace4bfb` (feat)
4. **Task 2.4: pentest AuthFlags migration + integration tests** — `7121324d` (feat)

## Files Created/Modified

### Created
- `cargo-pmcp/src/commands/auth_cmd/mod.rs` — `AuthCommand` enum + dispatcher; MED-4 concurrency rustdoc.
- `cargo-pmcp/src/commands/auth_cmd/cache.rs` — `TokenCacheV1`/`TokenCacheEntry`, `normalize_cache_key`, `write_atomic`, `is_near_expiry`, `refresh_and_persist`, `default_multi_cache_path`, `REFRESH_WINDOW_SECS`; 12 unit tests + 2 proptests.
- `cargo-pmcp/src/commands/auth_cmd/login.rs` — PKCE+DCR via `authorize_with_details`; persists full `AuthorizationResult`; D-19 clap mutex.
- `cargo-pmcp/src/commands/auth_cmd/logout.rs` — D-09 no-args error; `--all` clears; `<url>` removes one.
- `cargo-pmcp/src/commands/auth_cmd/status.rs` — 5-column tabular view; honors `--no-color`.
- `cargo-pmcp/src/commands/auth_cmd/token.rs` — raw token → stdout (D-11); status → stderr.
- `cargo-pmcp/src/commands/auth_cmd/refresh.rs` — D-16 force-refresh.
- `cargo-pmcp/tests/auth_integration.rs` — 7 end-to-end tests including mockito LOW-10 precedence proof.

### Modified
- `cargo-pmcp/Cargo.toml` — `tempfile = "3"` promoted to `[dependencies]`; `url = "2"` added; `mockito = "1"` added to dev-deps.
- `cargo-pmcp/src/lib.rs` — top-level `#[path]` module `test_support_cache` + re-export `test_support::cache` (narrow seam per MED-5).
- `cargo-pmcp/src/main.rs` — new `Commands::Auth { command: AuthCommand }` variant + dispatch arm.
- `cargo-pmcp/src/commands/mod.rs` — `pub mod auth_cmd;` registration.
- `cargo-pmcp/src/commands/auth.rs` — `try_cache_token` helper + `bearer_chain` helper; `AuthMethod::None` arms now consult the cache.
- `cargo-pmcp/src/commands/pentest.rs` — `#[command(flatten)] auth_flags: AuthFlags` replaces `pub api_key: Option<String>`; calls `resolve_auth_middleware` upstream; legacy --api-key preserved via `AuthMethod::ApiKey` arm.

## Decisions Made

- **Cache-path separation.** `~/.pmcp/oauth-cache.json` (this plan) is intentionally distinct from the legacy SDK single-server cache at `~/.pmcp/oauth-tokens.json` so that pre-existing library users aren't disturbed and so that the CLI can evolve the multi-server schema independently.
- **Narrow `test_support` seam via `#[path]`.** Instead of exposing `pub mod commands;` (broadens public API per MED-5) or `pub(crate) mod commands;` (forces entire CLI bin tree to compile in the lib target — which fails because `commands/loadtest/*.rs` self-references `cargo_pmcp::loadtest` as an external crate), a top-level `#[path = "commands/auth_cmd/cache.rs"] pub mod test_support_cache;` pulls in exactly one file. `test_support::cache` re-exports that module under its canonical name.
- **`eprintln!` status line in token.rs.** D-11 mandates stdout-only for the token value. G19 requires two output paths. Compromise: `eprintln!("Refreshing cached token for …")` on the near-expiry path satisfies both — status lands on stderr (safe), token on stdout (mandated).
- **T-74-F accepted.** Concurrent `auth login` races are last-writer-wins via atomic rename; documented in both cache.rs and auth_cmd/mod.rs rustdoc. Genuine simultaneous browser logins are rare during initial setup; matches gh/aws conventions.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] proptest `?` operator on `anyhow::Error`**
- **Found during:** Task 2.1 (running cache proptests).
- **Issue:** The plan's proptest body used `normalize_cache_key(&raw)?` but proptest's `prop_assert_eq!` macro expects `TestCaseError`, which does not implement `From<anyhow::Error>` (anyhow::Error does not impl std::error::Error).
- **Fix:** Replaced `?` with `.map_err(|e| TestCaseError::fail(format!(...)))?` and added `use proptest::test_runner::TestCaseError;`.
- **Files modified:** cargo-pmcp/src/commands/auth_cmd/cache.rs
- **Verification:** `cargo test commands::auth_cmd::cache::proptests` now passes 2/2.
- **Committed in:** `aa934380` (Task 2.1 commit)

**2. [Rule 3 - Blocker] `pub(crate) mod commands;` in lib.rs breaks lib compile**
- **Found during:** Task 2.4 (wiring the `test_support::cache` seam).
- **Issue:** The plan's Step 0 suggested `pub(crate) mod commands;` in lib.rs as an alternative to bare `pub mod commands;`. This seemed fine, but the `commands/` tree cross-references bin-only modules: `commands/loadtest/*.rs` imports via `use cargo_pmcp::loadtest::…` (i.e. references itself as an external crate, which works in the bin target because `main.rs` is a separate compilation unit, but fails in the lib target because you can't reference the crate you're compiling). `pub(crate) mod commands;` plus `pub(crate) mod deployment;`/`landing;`/etc. produced 67 unresolved-import errors in the lib.
- **Fix:** Abandoned the "whole commands tree in lib" approach. Used `#[path = "commands/auth_cmd/cache.rs"] pub mod test_support_cache;` at the lib root to compile ONLY cache.rs in the lib target. `test_support::cache` re-exports it. Bin target still uses `commands::auth_cmd::cache` as before — same file, two compilation units, no divergence.
- **Files modified:** cargo-pmcp/src/lib.rs
- **Verification:** `cargo build -p cargo-pmcp --tests` exits 0; integration tests import `cargo_pmcp::test_support::cache::…` successfully. G grep gates pass (`pub mod test_support` ≥ 1, `pub mod commands` == 0).
- **Committed in:** `7121324d` (Task 2.4 commit)

**3. [Rule 2 - Missing critical] Integration test platform gating**
- **Found during:** Task 2.4 (writing auth_integration.rs).
- **Issue:** The plan warned integration tests that set `HOME` may be flaky on Windows but left gating as "TBD". On Unix `HOME` is canonical; on Windows `dirs::home_dir()` reads `USERPROFILE`. Without gating, the CLI-driven tests (`logout_no_args_errors_via_cli`, `auth_token_prints_only_token_to_stdout`, `api_key_flag_overrides_cached_oauth_token`) would run with the user's real `~/.pmcp/` on Windows and potentially clobber real credentials.
- **Fix:** Added `#[cfg(unix)]` to the 3 CLI-driven tests. The pure-Rust tests (`normalize_covers_t74d_edge_cases`, `cache_roundtrip_via_write_atomic`, `is_near_expiry_window_is_60s`, `default_multi_cache_path_ends_in_oauth_cache_json`) remain cross-platform.
- **Files modified:** cargo-pmcp/tests/auth_integration.rs
- **Verification:** `cargo test --test auth_integration` → 7/7 pass on macOS.
- **Committed in:** `7121324d` (Task 2.4 commit)

---

**Total deviations:** 3 auto-fixed (1 bug from stale plan snippet, 1 blocker from lib-target compilation shape, 1 missing-critical platform gate).
**Impact on plan:** All deviations local, zero scope creep. The MED-5 narrow seam was delivered with a cleaner implementation than the plan proposed.

## Issues Encountered

None. All four tasks built, tested, and committed atomically. `make quality-gate` exits 0 (fmt clean, clippy lint-free on modified files).

## User Setup Required

None — `cargo pmcp auth login` is interactive and self-contained (opens a browser, writes to `~/.pmcp/oauth-cache.json`). No config files to author, no env vars to preset.

## Next Phase Readiness for Plan 03

Plan 03 (release + migration docs) can now:

1. **Document `cargo pmcp auth` in README.md / docs** with the full 5-subcommand reference.
2. **Update CHANGELOG.md** for the next cargo-pmcp release with the new `auth` command group, the pentest.rs `--api-key` migration notice (backward-compatible), and the new `tempfile` + `url` regular deps.
3. **Stamp the cargo-pmcp version bump** (0.8.1 → 0.9.0 per semver — new feature), the pmcp 2.5.0 release date, and add `mockito` to the dev-dep section of the published Cargo.toml.
4. **G27/G28 coverage** (release workflow) is Plan 03's scope.

## Threat Flags

None. No new security-relevant surface introduced outside the plan's `<threat_model>` block. T-74-D/E/F/G mitigations all landed as specified.

## Self-Check: PASSED

**File existence:**
- `cargo-pmcp/src/commands/auth_cmd/mod.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/cache.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/login.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/logout.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/status.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/token.rs` — FOUND
- `cargo-pmcp/src/commands/auth_cmd/refresh.rs` — FOUND
- `cargo-pmcp/tests/auth_integration.rs` — FOUND

**Commit existence:**
- `aa934380` Task 2.1: FOUND
- `fc96182f` Task 2.2: FOUND
- `7ace4bfb` Task 2.3: FOUND
- `7121324d` Task 2.4: FOUND

**Acceptance criteria spot-check:**
- G12 `grep -c 'Auth[[:space:]]*{' main.rs`: 2 PASS
- G13 (5 subcommand files): 5 PASS
- G14 `grep -c 'oauth-cache\.json' cache.rs`: 4 PASS
- G15 `grep -c 'schema_version' cache.rs`: 10 PASS
- G16 `grep -cE 'fn normalize_cache_key|to_ascii_lowercase' cache.rs`: 2 PASS
- G17 `grep -c 'specify a server URL or --all' logout.rs`: 3 PASS
- G18 `grep -cE 'conflicts_with.*oauth_client_id' login.rs`: 1 PASS
- G19 `grep -cE 'eprintln!|println!\("\{\}"' token.rs`: 2 PASS
- G20 token-leak grep across all auth_cmd/*: 0 PASS (no leaks)
- G21 `grep -cE 'AuthMethod::None.*try_cache_token|try_cache_token.*await' auth.rs`: 2 PASS
- G22 `grep -cE 'REFRESH_WINDOW_SECS|is_near_expiry|refresh_and_persist' auth.rs`: 5 PASS
- G23 AuthFlags in pentest.rs: PASS
- G24 no `#[arg.*env = "MCP_API_KEY"]` in pentest.rs: 0 PASS
- G25 auth_cmd unit tests: 17 passed
- G26 auth_integration: 7 passed
- G31 MED-4 concurrency doc in auth_cmd/mod.rs: 1 PASS
- G32 LOW-10 precedence test present: 1 PASS
- `cargo build -p cargo-pmcp`: exits 0
- `cargo test -p cargo-pmcp --bin cargo-pmcp`: 317 passed
- `cargo test -p cargo-pmcp --test auth_integration`: 7 passed
- `cargo run -p cargo-pmcp -- auth --help`: lists 5 subcommands
- `cargo run -p cargo-pmcp -- auth logout`: exits 1 with D-09 copy
- `make quality-gate`: exit 0 (fmt OK, clippy OK on modified files)

All 24 must_haves truths + 8 must_haves artifacts satisfied. CLI-AUTH-01 requirement is addressable end-to-end.

---
*Phase: 74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token*
*Completed: 2026-04-21*
