---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 06
subsystem: database
tags: [mysql, sqlx, sql-connector, dev-mock, information-schema, pure-rust, tls-rustls-aws-lc-rs, lambda]

# Dependency graph
requires:
  - phase: 84-00
    provides: pmcp-toolkit-mysql scaffold (Cargo.toml with sqlx mysql + pure-Rust TLS deps, stub connector, test/example shells)
  - phase: 84-01
    provides: 3-method SqlConnector trait (dialect/execute/schema_text) + ConnectorError variants
  - phase: 84-02
    provides: translate_placeholders SqlWalker (:name -> ? for Dialect::MySql, yields ordered_params)
provides:
  - Public MysqlConnector type backed by sqlx 0.8.6 (mysql + runtime-tokio + tls-rustls-aws-lc-rs, no OpenSSL)
  - ::connect(url) constructor via MySqlPool::connect_lazy (offline-safe; URL parsed synchronously, TCP deferred — REVIEWS M3)
  - Full 3-method SqlConnector impl (dialect/execute/schema_text) over a sqlx MySqlPool
  - bind_one Value->sqlx bind dispatch (Null/Bool/i64/f64/String + JSON-text fallback for array/object)
  - information_schema.columns-driven schema_text filtered by the MySQL database name -> backtick CREATE TABLE / ENGINE=InnoDB blocks
  - sanitize_url password redaction on the connect() error path (T-84-06-02)
  - parse_database_from_url helper (extracts schema name from URL, strips port + query string)
  - dev_mock feature exposing pmcp_toolkit_mysql::dev_mock::MysqlMock (REVIEWS H5)
  - mysql_minimal Shape C example (publishable, depends only on src/ via dev_mock)
  - tests/integration.rs D-13 4-point contract anchor
affects: [84-07-athena, 85-shape-a-pure-config-binary, 86-shapes-bcd, 88-dogfood]

# Tech tracking
tech-stack:
  added: [sqlx-0.8.6-mysql, tls-rustls-aws-lc-rs]
  patterns:
    - "MySqlPool::connect_lazy(url)? parses the URL synchronously and defers TCP I/O to first use — offline-safe constructor, real failures surface on first execute/schema_text (REVIEWS M3)"
    - "bind_one consumes-and-returns Query<MySql, MySqlArguments> per Value variant — serde_json::Value does not impl Encode<MySql> directly, so each scalar binds through a concrete Rust type; Null binds None::<&str>, array/object stringify"
    - "schema_text binds the MySQL database name into information_schema.columns WHERE table_schema = ? (NOT the Postgres 'public' literal) — MySQL has no fixed public schema"
    - "dev_mock cargo feature keeps the in-process MysqlMock under src/dev_mock.rs so publishable examples opt into it without referencing tests/ (REVIEWS H5)"
    - "Helper split (sanitize_url/parse_database_from_url/bind_one/column_to_value/row_to_value/schema_col/format_information_schema_as_ddl) keeps every fn under PMAT cog 25"

key-files:
  created:
    - crates/pmcp-toolkit-mysql/src/dev_mock.rs
  modified:
    - crates/pmcp-toolkit-mysql/Cargo.toml
    - crates/pmcp-toolkit-mysql/src/lib.rs
    - crates/pmcp-toolkit-mysql/tests/integration.rs
    - crates/pmcp-toolkit-mysql/examples/mysql_minimal.rs
  deleted:
    - crates/pmcp-toolkit-mysql/tests/mock_mysql.rs

key-decisions:
  - "connect_lazy over connect(url).await — constructor must be offline-safe and must not block against an unreachable MySQL (REVIEWS M3, T-84-06-05)"
  - "Mock lives at src/dev_mock.rs (not tests/mock_mysql.rs) so it is reachable from the publishable example via the public pmcp_toolkit_mysql::dev_mock path; the old tests/ mock file was deleted (REVIEWS H5)"
  - "array/object params bind as JSON text fallback (vs the Postgres connector which rejects them) — sqlx accepts a String bind for any column, so a permissive fallback is safe here"
  - "schema_text filters information_schema.columns by the parsed database name, defaulting to empty string when the URL omits it"

patterns-established:
  - "Pure-Rust sqlx connector shape: connect_lazy + translate_placeholders(Dialect::MySql) + bind_one dispatch + information_schema DDL — reusable template for any sqlx-backed dialect"
  - "Password redaction (sanitize_url) on every Connection error path keeps secrets off the MCP transport"

requirements-completed: [CONN-06, TEST-01]

# Metrics
duration: 18min
completed: 2026-05-26
---

# Phase 84 Plan 06: pmcp-toolkit-mysql MySQL Connector Summary

**Landed `pmcp-toolkit-mysql` as a fully-functional pure-Rust MySQL connector over sqlx 0.8.6 (mysql + runtime-tokio + tls-rustls-aws-lc-rs, no OpenSSL): the 3-method `SqlConnector` trait with `?` placeholders, `information_schema`-driven backtick DDL, `connect_lazy` offline-safe construction, an in-process `MysqlMock` under the `dev_mock` feature, 4 D-13 integration tests, and a publishable Shape C example.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-05-26T21:46Z (approx, post-84-05)
- **Completed:** 2026-05-26
- **Tasks:** 1 of 1 (TDD)
- **Files modified:** 6 (4 modified, 1 created, 1 deleted)

## Accomplishments

- `MysqlConnector` implements the 3-method `SqlConnector` trait via a `sqlx` `MySqlPool`: `dialect()` -> `Dialect::MySql`, `execute()` translating `:name` -> `?` via `translate_placeholders(_, Dialect::MySql)` and binding per-`Value` through `bind_one`, and `schema_text()` querying `information_schema.columns` filtered by the parsed MySQL database name into backtick `CREATE TABLE ... ENGINE=InnoDB` blocks.
- REVIEWS M3: `connect()` uses `MySqlPool::connect_lazy(url)?` — the constructor parses the URL synchronously and is offline-safe; a malformed URL returns `ConnectorError::Connection(_)` immediately while real connection failures surface on first use.
- REVIEWS H5: the in-process `MysqlMock` lives at `src/dev_mock.rs` gated by `#[cfg(any(test, feature = "dev_mock"))]`; the old `tests/mock_mysql.rs` was deleted; the example and integration test import it via the public `pmcp_toolkit_mysql::dev_mock::MysqlMock` path.
- `sanitize_url` redacts the password segment before any `ConnectorError::Connection` is constructed (T-84-06-02) — verified by unit tests.
- 4 D-13 integration tests, a ≤15-line Shape C `mysql_minimal` example, unit tests, and a doctest all green; pure-Rust TLS (no OpenSSL); no Docker/testcontainers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement `MysqlConnector` (connect_lazy) + bind dispatch + dev_mock + integration tests + example** - `422b9460` (feat)

_Note: This is a TDD-style plan but a single cohesive crate-build task — the tests cannot compile without the connector types, so implementation + tests landed together in one `feat` commit. Lib unit tests (incl. the REVIEWS M3 `connect_lazy` invariant test) and the 4 integration tests verify the behavior._

## Files Created/Modified

- `crates/pmcp-toolkit-mysql/src/lib.rs` - Full `MysqlConnector` impl: `connect_lazy` constructor, `sanitize_url`, `parse_database_from_url`, `bind_one` Value dispatch, `column_to_value`/`row_to_value` row decoding, `format_information_schema_as_ddl` (backtick + InnoDB), the 3-method `SqlConnector` impl, and 7 unit tests.
- `crates/pmcp-toolkit-mysql/src/dev_mock.rs` - (created) `MysqlMock` authentic in-process mock under the `dev_mock` feature: `?` placeholders, backtick/InnoDB `schema_text`, `pub` `last_translated_sql` + `last_positional_args` recording fields, cheap fixture query engine.
- `crates/pmcp-toolkit-mysql/Cargo.toml` - Added `[features] default = [] / dev_mock = []` and the `[[example]] name = "mysql_minimal" required-features = ["dev_mock"]` block (sqlx pure-Rust TLS deps were already present from Plan 00).
- `crates/pmcp-toolkit-mysql/tests/integration.rs` - 4 D-13 `#[tokio::test]`s importing the mock via the published `dev_mock` path: `dialect_is_mysql`, `execute_translates_named_to_question_mark`, `schema_text_contains_expected_ddl_with_backticks`, `repeated_named_params_translate_to_question_marks_in_order`.
- `crates/pmcp-toolkit-mysql/examples/mysql_minimal.rs` - ≤15-line Shape C example using `MysqlMock::employee_directory()` via the public dev_mock path; prints `mysql_minimal: 2 rows`.
- `crates/pmcp-toolkit-mysql/tests/mock_mysql.rs` - (deleted) old `tests/`-side mock removed per REVIEWS H5; the mock now lives under `src/`.

## Verification

| Gate | Result |
|------|--------|
| `cargo build -p pmcp-toolkit-mysql` (default) | green; example auto-skipped (required-features unmet) |
| `cargo build -p pmcp-toolkit-mysql --features dev_mock` | green |
| `cargo test -p pmcp-toolkit-mysql --lib` | 7 passed (incl. `test_connect_lazy_returns_ok_without_network`) |
| `cargo test -p pmcp-toolkit-mysql --features dev_mock --test integration` | 4 passed |
| `cargo run -p pmcp-toolkit-mysql --features dev_mock --example mysql_minimal` | prints `mysql_minimal: 2 rows` |
| `cargo test --doc -p pmcp-toolkit-mysql` | 1 passed |
| clippy on toolkit-mysql files (make-lint level + pedantic/nursery) | clean — zero lints reference any `crates/pmcp-toolkit-mysql/` file |
| PMAT `--max-cognitive 25` | 0 violations on the mysql crate |
| dep-metadata gate (sqlx `mysql` feature) | present |
| `cargo check --workspace --all-features` | green |
| REVIEWS guards (connect_lazy / no eager connect / src/dev_mock.rs / no tests/mock_mysql.rs / dev_mock=[] / no openssl / no testcontainers) | all OK |

## Deviations from Plan

None for the connector itself — the plan executed as written.

**Out-of-scope (NOT fixed, already tracked):** the project's full `make quality-gate` / broad `cargo clippy --all-targets -W clippy::pedantic` fails to *build* at `crates/pmcp-widget-utils/src/lib.rs:46,50` (`clippy::uninlined_format_args`) under the local rust-1.95.0 toolchain, which is ahead of CI's pinned stable. This is a pre-existing lint already logged in `.planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/deferred-items.md` (Plan 84-01 entry) and is unrelated to this plan's files. The mysql crate's own sources are clippy-clean — confirmed by filtering the pedantic+nursery sweep for `crates/pmcp-toolkit-mysql/` paths (zero matches).

## Known Stubs

None. The connector is fully wired against a live sqlx `MySqlPool`; the `MysqlMock` is an intentional, feature-gated in-process test/example fixture (not a stub on the production path).

## Self-Check: PASSED

- All created/modified files exist on disk (lib.rs, dev_mock.rs, integration.rs, mysql_minimal.rs, Cargo.toml, 84-06-SUMMARY.md).
- `tests/mock_mysql.rs` confirmed deleted (REVIEWS H5).
- Task commit `422b9460` present in git history.
