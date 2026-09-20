---
phase: 49-bump-dependencies-reqwest-0-13-jsonschema-0-45
plan: 01
subsystem: infra
tags: [reqwest, jsonschema, dependency-upgrade, rustls, msrv]

# Dependency graph
requires: []
provides:
  - "reqwest 0.13 across all workspace crates"
  - "jsonschema 0.45 (optional dep, aligned with reqwest 0.13)"
  - "MSRV 1.83.0"
  - "Template strings generating correct reqwest 0.13 dependencies for new projects"
affects: [release, deploy, oauth, mcp-tester, mcp-preview]

# Tech tracking
tech-stack:
  added: [reqwest-0.13, jsonschema-0.45]
  patterns: [oauth2-reqwest-version-bridge]

key-files:
  created: []
  modified:
    - Cargo.toml
    - crates/mcp-tester/Cargo.toml
    - crates/mcp-preview/Cargo.toml
    - cargo-pmcp/Cargo.toml
    - crates/mcp-tester/src/tester.rs
    - cargo-pmcp/src/commands/deploy/init.rs
    - cargo-pmcp/src/templates/oauth/proxy.rs
    - cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs
    - examples/26-server-tester/src/tester.rs

key-decisions:
  - "Use oauth2::reqwest::Client for oauth2 token exchange (oauth2 5.0 re-exports reqwest 0.12)"
  - "MSRV bumped from 1.82.0 to 1.83.0 per jsonschema 0.45 requirement"
  - "Accept dual reqwest versions in lockfile (0.12 via oauth2, 0.13 direct) until oauth2 updates"

patterns-established:
  - "oauth2-reqwest bridge: use oauth2::reqwest::Client for oauth2 API calls to avoid version mismatch"

requirements-completed: [DEP-01]

# Metrics
duration: 18min
completed: 2026-03-13
---

# Phase 49 Plan 01: Bump Dependencies Summary

**Upgraded reqwest 0.12->0.13 and jsonschema 0.38->0.45 across workspace with rustls feature rename, form opt-in, and MSRV bump to 1.83**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-13T02:40:12Z
- **Completed:** 2026-03-13T02:58:57Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- All four workspace Cargo.toml files updated to reqwest 0.13 with correct feature flags (rustls-tls->rustls, form opt-in)
- jsonschema bumped from 0.38 to 0.45, eliminating duplicate reqwest 0.12 transitive dependency from jsonschema
- MSRV updated to 1.83.0 to satisfy jsonschema 0.45 requirement
- Deprecated `danger_accept_invalid_certs` proactively renamed to `tls_danger_accept_invalid_certs` (6 sites)
- Template strings for scaffolded projects now generate correct reqwest 0.13 dependency lines
- Fixed oauth2 token exchange to use oauth2's re-exported reqwest 0.12 Client for version compatibility

## Task Commits

Each task was committed atomically:

1. **Task 1: Update all Cargo.toml files and MSRV** - `f5d0de9` (chore)
2. **Task 2: Update source code (deprecated methods + template strings)** - `df7b9f3` (feat)

## Files Created/Modified
- `Cargo.toml` - Root crate: reqwest 0.13, jsonschema 0.45, MSRV 1.83.0, form feature
- `crates/mcp-tester/Cargo.toml` - reqwest 0.13 with rustls feature
- `crates/mcp-preview/Cargo.toml` - reqwest 0.13
- `cargo-pmcp/Cargo.toml` - reqwest 0.13 with rustls, form features
- `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` - Use oauth2::reqwest::Client for token exchange
- `crates/mcp-tester/src/tester.rs` - Renamed danger_accept_invalid_certs (3 sites)
- `examples/26-server-tester/src/tester.rs` - Renamed danger_accept_invalid_certs (3 sites)
- `cargo-pmcp/src/commands/deploy/init.rs` - Template strings: reqwest 0.13, rustls (2 sites)
- `cargo-pmcp/src/templates/oauth/proxy.rs` - Template string: reqwest 0.13, rustls, form

## Decisions Made
- **oauth2 bridge pattern:** Used `oauth2::reqwest::Client` (the 0.12 client re-exported by oauth2) for oauth2 token exchange calls, since oauth2 5.0 pins reqwest 0.12 and its `AsyncHttpClient` trait requires 0.12's Client type. The rest of cargo-pmcp uses reqwest 0.13 directly.
- **MSRV 1.83.0:** Bumped from 1.82.0 because jsonschema 0.45 requires it. All CI uses stable (1.93+), so this only affects the declared minimum.
- **Dual reqwest in lockfile:** Accepted that oauth2 5.0 keeps reqwest 0.12 in the dependency tree. This adds compile time but is unavoidable until oauth2 releases a reqwest 0.13 compatible version.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed oauth2 reqwest version mismatch in cargo-pmcp auth**
- **Found during:** Task 1 (Cargo.toml updates)
- **Issue:** After upgrading cargo-pmcp to reqwest 0.13, `oauth2::request_async()` rejected the new `reqwest::Client` because oauth2 5.0 implements `AsyncHttpClient` for reqwest 0.12's Client only
- **Fix:** Changed 2 call sites in `auth.rs` to use `oauth2::reqwest::Client::new()` instead of `reqwest::Client::new()` for oauth2 token exchange operations
- **Files modified:** `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs`
- **Verification:** `cargo check --workspace` compiles clean
- **Committed in:** f5d0de9 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential fix for compilation. The plan acknowledged oauth2 would keep reqwest 0.12 but didn't account for the API incompatibility at call sites. No scope creep.

## Issues Encountered
- Pre-existing clippy warnings in mcp-e2e-tests, pmcp-macros, and pmcp lib (unused_async, needless_raw_string_hashes, unnecessary_wraps) cause `make quality-gate` to fail at the full workspace clippy step. These are NOT related to this plan's changes. Our target crates (mcp-tester, mcp-preview, cargo-pmcp) pass clippy clean.
- Pre-existing `atty` crate vulnerability (RUSTSEC-2024-0375) causes audit step failure. Not related to this plan.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Workspace compiles with reqwest 0.13 and jsonschema 0.45
- All tests pass
- Ready for release when version bumps are applied

## Self-Check: PASSED

- All 9 modified files verified present on disk
- Commit f5d0de9 (Task 1) verified in git log
- Commit df7b9f3 (Task 2) verified in git log

---
*Phase: 49-bump-dependencies-reqwest-0-13-jsonschema-0-45*
*Completed: 2026-03-13*
