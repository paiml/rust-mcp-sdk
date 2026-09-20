---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 01
subsystem: database
tags: [sql-connector, trait, connector-error, thiserror, async-trait, semver, security]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit
    provides: "SqlConnector trait (2-method MVP), Dialect enum, ConnectorError (Io/Schema/DialectMismatch), pub(crate) MockSqlConnector"
  - phase: 84-sql-connectors-postgres-mysql-athena-sqlite
    plan: 00
    provides: "Three per-backend stub crates + translate.rs RED proptest shell"
provides:
  - "3-method SqlConnector trait: dialect() + execute() + schema_text() (CONN-01)"
  - "Four additive ConnectorError variants: Driver, Query, ParameterBind { name, reason }, Connection"
  - "Stable execute() contract: async fn execute(&self, sql: &str, params: &[(String, serde_json::Value)]) -> Result<Vec<serde_json::Value>, ConnectorError>"
  - "Credential-leak guardrail unit test + Rustdoc redaction mandate on Connection variant (T-84-01-01)"
affects: [84-04, 84-05, 84-06, 84-07]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Breaking trait-method addition (no default body) acceptable pre-publish (0.1.0, 0 crates.io hits) per P83 D-05"
    - "Additive #[non_exhaustive] enum-variant growth — semver-clean for downstream consumers"
    - "Local Dummy struct in trait doctest avoids circular doctest dependency on downstream crates (REVIEWS H6)"
    - "Compile-only Send + Sync + 'static assertion via assert_send_sync::<Box<dyn Trait>>()"

key-files:
  created:
    - ".planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/deferred-items.md"
  modified:
    - "crates/pmcp-server-toolkit/src/sql/mod.rs"

key-decisions:
  - "MockSqlConnector::execute returns ConnectorError::Driver(\"...fixture-only...\") — the only in-tree trait impl; the 3 backend stubs do NOT yet impl SqlConnector (Wave 0 left them as connect()/from_config() stubs), so no other impl needed updating."
  - "execute() placed between dialect() and schema_text() per plan's exact method ordering."
  - "Trait rustdoc pivoted from 'Phase 84 will land execute()' to 'Phase 84 ships full 3-method surface'; deferred-evolution section now documents future execute_stream() (default-impl additive) + SqlTransactional extension (D-02)."

metrics:
  duration: ~4min
  tasks: 2
  files-modified: 2
  completed: 2026-05-26

requirements-completed: [CONN-01, CONN-02]
---

# Phase 84 Plan 01: SqlConnector 3-Method Trait + Execute-Time ConnectorError Variants Summary

**Extended the Phase 83 `SqlConnector` trait from its 2-method MVP to the 3-method CONN-01 shape by adding `execute(sql, &[(String, Value)]) -> Result<Vec<Value>, ConnectorError>` (no default body), and grew the `#[non_exhaustive]` `ConnectorError` enum with the four execute-time variants (`Driver`, `Query`, `ParameterBind { name, reason }`, `Connection`) that Wave 2's per-backend crates will emit — locking the connector contract before the three parallel backend impls land.**

## Performance

- **Duration:** ~4 min
- **Tasks:** 2 (both `feat`, committed atomically)
- **Files modified:** 2 (`src/sql/mod.rs` modified; `deferred-items.md` created)

## Accomplishments

- **`SqlConnector` now declares exactly 3 methods** in the required order: `dialect()`, `execute()`, `schema_text()`. The `execute` signature matches CONN-01 / D-01 / D-03 verbatim: `async fn execute(&self, sql: &str, params: &[(String, serde_json::Value)]) -> Result<Vec<serde_json::Value>, ConnectorError>`, with no default body so every backend must implement it.
- **Module + trait rustdoc updated** to reflect that Phase 84 ships the full surface; the semver-evolution section now documents the deferred `execute_stream()` (default-impl-backed, semver-additive) and the separate `SqlTransactional` extension (D-02), rather than promising `execute()` as future work.
- **Trait doctest uses a LOCAL `Dummy` struct** (`no_run`) implementing all three methods — deliberately referencing no downstream per-backend crate, avoiding the circular doctest dependency flagged by REVIEWS H6. The H6 guard (`! grep -q 'pmcp_toolkit_' …`) passes.
- **`ConnectorError` gained 4 additive variants** (`Driver(String)`, `Query(String)`, `ParameterBind { name, reason }`, `Connection(String)`), each with a `thiserror` `#[error(...)]` annotation matching the existing variant style. The enum was already `#[non_exhaustive]`, so this is semver-clean for downstream consumers.
- **Security guardrail (T-84-01-01):** the `Connection` variant carries a Rustdoc `# Security` section mandating implementor-side credential redaction, and `test_connection_display_does_not_echo_password` asserts the `Display` output never synthesizes `password` / `AWS_SECRET_ACCESS_KEY` / `DATABASE_URL` tokens.
- **`MockSqlConnector` keeps compiling** under the larger trait surface via a fixture-only `execute()` returning `ConnectorError::Driver(...)`. All Phase 83 tests that reference it (in `code_mode.rs`) still pass.
- **Compile-time object-safety guard:** `execute_signature_tests` asserts `Box<dyn SqlConnector>: Send + Sync + 'static` so the `Arc<dyn SqlConnector>` plumbing and per-backend crates stay sound.

## Task Commits

1. **Task 1: Add `execute()` to `SqlConnector` trait** — `bce4161c` (feat)
2. **Task 2: Extend `ConnectorError` with 4 execute-time variants + credential-leak guardrail** — `c05cc348` (feat)

**Plan metadata:** (final docs commit — this SUMMARY + STATE + ROADMAP + REQUIREMENTS)

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/sql/mod.rs` — module/trait rustdoc rewrite, `execute()` trait method, trait doctest with local `Dummy`, `MockSqlConnector::execute` fixture impl, 4 new `ConnectorError` variants, `execute_signature_tests`, `connector_error_tests`.
- `.planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/deferred-items.md` — created to log out-of-scope clippy lints surfaced by the newer local toolchain (see Issues Encountered).

## Decisions Made

- **Only `MockSqlConnector` needed an `execute()` impl.** The plan + executor context anticipated that the 3 backend stub crates would need stub `execute()` methods, but Wave 0 (84-00) left them as bare `connect()` / `from_config()` constructors that do NOT yet `impl SqlConnector` (verified via `grep -rn "impl SqlConnector"` — only `mod.rs` matches in the source tree). Adding `execute()` to the trait therefore did not break those crates; no stub `execute()` was needed there. The real impls land in Plans 84-05/06/07.
- **Task 1 used `ConnectorError::Schema` transiently, then Task 2 switched to `Driver`.** To keep Task 1 atomically green (its acceptance criterion is `cargo build` exits 0) before `Driver` existed, the Task-1 `MockSqlConnector::execute` body used the pre-existing `Schema` variant, then Task 2 flipped it to the semantically-correct `Driver` variant once that variant was introduced. Net result in the final tree: `Driver`.
- **`# Security` Rustdoc wording chosen to satisfy the literal acceptance grep** (`Implementors MUST redact credentials`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Reworded doctest comment to satisfy the REVIEWS H6 literal-grep guard**
- **Found during:** Task 1 verification
- **Issue:** An explanatory comment in the trait doctest rustdoc originally read "does NOT reference any downstream `pmcp_toolkit_*` crate", which tripped the H6 acceptance guard `! grep -q 'pmcp_toolkit_' crates/pmcp-server-toolkit/src/sql/mod.rs` (a blanket literal grep for `pmcp_toolkit_`, which does not distinguish a prose mention from an actual import).
- **Fix:** Reworded the comment to "does NOT reference any downstream per-backend crate" — preserves the H6 intent (no circular doctest dependency; the `Dummy` struct is local) while keeping the guard at zero matches.
- **Files modified:** `crates/pmcp-server-toolkit/src/sql/mod.rs`
- **Commit:** `bce4161c` (folded into Task 1)

**Total deviations:** 1 (doc-comment wording vs. literal guard; no production-logic change).

## Issues Encountered

- **Local clippy toolchain (rust-1.95.0) is newer than CI's pinned stable**, surfacing pedantic/nursery and some default lints in files this plan did NOT touch: `builder_ext.rs:178` (`needless_return`), `code_mode.rs:207-208` (`field_reassign_with_default`) — both Phase 83 code; `sql/translate.rs:76-119` — Wave 0 RED proptest scaffold (Plan 84-02 territory); `pmcp-widget-utils/src/lib.rs:46,50` (`uninlined_format_args`, pedantic) — unrelated dependency. Per the SCOPE BOUNDARY rule these are out of scope and were NOT fixed; they are logged in `deferred-items.md`. **The file this plan modified (`src/sql/mod.rs`) is clippy-clean at `-D warnings`** (verified: zero error locations point at `sql/mod.rs`).
- **`make quality-gate` was not run end-to-end** because its workspace test phase fails on the 5 intentionally-RED `translate::proptests` (committed RED in Wave 0, GREEN in Plan 84-02) and on the unrelated pre-existing clippy lints above — neither caused by this plan. Instead, the gate's meaningful-for-this-diff parts were run and pass: `cargo fmt --all -- --check` (clean workspace-wide), `cargo clippy -p pmcp-server-toolkit ... -D warnings` (clean for `sql/mod.rs`), `cargo build -p pmcp-server-toolkit --features sqlite --features code-mode`, `cargo test -p pmcp-server-toolkit --features sqlite --lib sql::` (7/7 non-RED pass), `cargo test --doc -p pmcp-server-toolkit --features sqlite` (26 pass). No pre-commit hook is installed in this working tree, so commits were not auto-gated.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build -p pmcp-server-toolkit --features sqlite --features code-mode` | PASS |
| `cargo test -p pmcp-server-toolkit --features sqlite --lib sql::tests / execute_signature_tests / connector_error_tests` | PASS (7/7 non-RED) |
| `cargo test --doc -p pmcp-server-toolkit --features sqlite` | PASS (26 doctests, incl. new `Dummy` trait doctest) |
| `cargo fmt --all -- --check` | PASS (clean) |
| `cargo clippy -p pmcp-server-toolkit ... -D warnings` on `sql/mod.rs` | PASS (zero issues in modified file) |
| REVIEWS H6 guard `! grep -q 'pmcp_toolkit_' …mod.rs` | PASS |
| No `aws-sdk-glue` referenced in `mod.rs` | PASS |
| `sql::translate::proptests::*` (5) | RED (intentional — Plan 84-02; out of scope) |

## User Setup Required

None — pure trait/enum surface change, no external service configuration.

## Next Phase Readiness

- The 3-method `SqlConnector` contract and the execute-time `ConnectorError` variants are now stable. Wave 2's three parallel per-backend crates (Plans 84-05 Postgres / 84-06 MySQL / 84-07 Athena) and the SQLite connector (Plan 84-04 promoting `MockSqlConnector` → `SqliteConnector`) implement against this locked surface.
- Plan 84-02 turns the 5 RED `translate::proptests` GREEN when `translate_placeholders` ships the `SqlWalker` state machine; per-backend `execute()` impls will call it then bind from `params`.
- The pre-existing clippy lints logged in `deferred-items.md` should be swept when the CI toolchain advances or in a Phase 83 follow-up; they do not block Wave 1/2 functionality.

## Self-Check: PASSED

All claimed artifacts verified — see appended self-check section.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
