# Deferred Items — Phase 90.1

Out-of-scope pre-existing issues discovered during execution (NOT caused by this
plan's changes — logged per the SCOPE BOUNDARY rule, not fixed).

## Pre-existing fmt diff in unmodified source

- **File:** `crates/pmcp-openapi-server/src/dispatch.rs:107`
- **Discovered during:** Plan 90.1-01 (running `cargo fmt -p pmcp-openapi-server -- --check`)
- **Status:** `dispatch.rs` is byte-identical to base commit `4592bb44`; this plan
  did not touch it. `cargo fmt --check` reports a diff there (an `auth` let-binding
  wrapping). Out of scope for 90.1-01.

## Pre-existing clippy warning in unmodified source

- **File:** `crates/pmcp-server-toolkit/src/config.rs:526`
- **Lint:** `clippy::redundant_guards` (`Some(name) if name.is_empty()` →
  suggest `Some("")`)
- **Discovered during:** Plan 90.1-01 (`cargo clippy -p pmcp-openapi-server --tests --examples`)
- **Status:** In `pmcp-server-toolkit`, not touched by this plan. Per repo MEMORY,
  the toolkit crates are not clippy-gated in CI (`make lint`/ci.yml lints only root
  `pmcp` with a generous allow-list), so this does not block CI. Out of scope.
