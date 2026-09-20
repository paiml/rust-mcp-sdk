---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 07
subsystem: database
tags: [athena, aws-sdk-athena, sql, connector, toolkit, presto, trino, pagination]

# Dependency graph
requires:
  - phase: 84-03
    provides: SqlConnector 3-method trait + ConnectorError execute-time variants + synthesize_from_config_with_connector + DatabaseSection.url (D-08)
  - phase: 84-02
    provides: translate_placeholders(Dialect::Athena) → ? positional placeholders + ordered_params
  - phase: 84-00
    provides: pmcp-toolkit-athena crate scaffold + RUSTSEC-safe Cargo.toml with documented NO-Glue comment
provides:
  - AthenaConnector — fully-functional AWS Athena SqlConnector via aws-sdk-athena (no Glue)
  - AthenaConfig struct + 2-arg from_config(region, workgroup) per CONTEXT.md D-08 + builder methods
  - Query-then-poll execution (StartQueryExecution → GetQueryExecution backoff → paginated GetQueryResults)
  - GetTableMetadata-driven CREATE EXTERNAL TABLE schema_text
  - AthenaMock in-process dev_mock fixture with multi-page simulation seam
affects: [85-shape-a-pure-config, 86-shapes-bcd, 88-dogfood, pmcp-sql-server]

# Tech tracking
tech-stack:
  added: [aws-sdk-athena 1.106.0, aws-config 1.8.17]
  patterns:
    - "REVIEWS M4: 2-arg locked constructor + AthenaConfig struct + with_* builders (avoid signature drift)"
    - "REVIEWS M5: paginated_get_query_results loops on next_token; mock simulates via PAGINATED_QUERY_MARKER + with_pages"
    - "REVIEWS H5: AthenaMock at src/dev_mock.rs under dev_mock feature (publishable example path)"
    - "Query-then-poll backoff: capped exponential (500..5000ms) bounded by query_timeout_ms"
    - "Credential sanitization via per-token AKIA/ASIA + secret-run redaction (strip_aws_credentials)"

key-files:
  created:
    - crates/pmcp-toolkit-athena/src/dev_mock.rs
  modified:
    - crates/pmcp-toolkit-athena/Cargo.toml
    - crates/pmcp-toolkit-athena/src/lib.rs
    - crates/pmcp-toolkit-athena/tests/integration.rs
    - crates/pmcp-toolkit-athena/examples/athena_minimal.rs

key-decisions:
  - "from_config stays EXACTLY 2 positional args (region, workgroup) per D-08 LOCKED; all other knobs via builders / AthenaConfig / from_athena_config (REVIEWS M4)"
  - "Column::builder().build() returns Result (name required) — test helper unwraps each column"
  - "strip_aws_credentials is per-whitespace-token (split_whitespace + redact_token + looks_like_credential) to stay well under PMAT cog 25 vs a monolithic regex-style scan"
  - "format_columns helper extracted from format_table_metadata to keep both under cog 25"
  - "execution_parameters threaded via set_execution_parameters(Option<Vec<String>>): None when empty, Some otherwise (Presto binds strings parametrically)"

patterns-established:
  - "Per-backend dev_mock mock implements SqlConnector directly + records last_translated_sql / last_positional_args (parallel to Postgres/MySQL Plans 05/06)"
  - "Pagination behavioral seam: mock returns flattened pages so the integration test asserts M5 without live Athena"

requirements-completed: [CONN-07, TEST-01]

# Metrics
duration: 13min
completed: 2026-05-26
---

# Phase 84 Plan 07: pmcp-toolkit-athena Connector Summary

**AWS Athena connector via aws-sdk-athena (NO Glue): query-then-poll execution with next_token pagination (M5), 2-arg D-08 from_config + AthenaConfig builders (M4), GetTableMetadata-driven CREATE EXTERNAL TABLE schema, AKIA/secret-run credential redaction, and an in-process AthenaMock with multi-page simulation (H5).**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-05-26T21:58:41Z
- **Completed:** 2026-05-26T22:06:15Z
- **Tasks:** 2 (both TDD)
- **Files modified:** 5 (1 created, 4 modified, 1 deleted)

## Accomplishments

- `AthenaConnector` implements the full 3-method `SqlConnector` trait over `aws-sdk-athena 1.106.0` with NO `aws-sdk-glue` dependency (Landmine #4) — schema introspection rides `GetTableMetadata`.
- REVIEWS M4: `from_config(region, workgroup)` is exactly 2 positional args per CONTEXT.md D-08 (LOCKED); `AthenaConfig` + `with_database`/`with_output_location`/`with_query_timeout`/`with_tables` builders + `from_athena_config` secondary constructor carry the rest. Runtime gate rejects an empty `output_location` before any AWS call.
- REVIEWS M5: `paginated_get_query_results` loops on `next_token` across every page; `AthenaMock` simulates multi-page responses via `with_pages` + `PAGINATED_QUERY_MARKER`, and `multi_page_query_returns_all_pages_combined` asserts 2+3 pages → 5 rows in order.
- Query-then-poll lifecycle: `StartQueryExecution` → `poll_until_done` (capped exponential backoff 500..5000ms, bounded by `query_timeout_ms`) → paginated `GetQueryResults` → JSON objects keyed by column name.
- `strip_aws_credentials` redacts AKIA/ASIA access-key IDs + long secret-key runs from every error path (T-84-07-02).
- REVIEWS H5: `AthenaMock` lives at `src/dev_mock.rs` under the `dev_mock` feature; the Shape C `athena_minimal` example (6-line `main`) reaches it via the published path; legacy `tests/mock_athena.rs` deleted.
- ALWAYS coverage: 12 unit + 5 integration + 1 doctest + runnable example. PMAT clean (no function over cog 25, no `#[allow(clippy::cognitive_complexity)]`). athena crate files clippy-clean at `-D warnings`.

## Task Commits

Each task was committed atomically:

1. **Task 1: AthenaConnector + pagination + GetTableMetadata schema + credential sanitization + dev_mock scaffold** — `ac131bd8` (feat)
2. **Task 2: AthenaMock at src/dev_mock.rs + 5 integration tests + Shape C example** — `66bbb1d9` (feat)

**Plan metadata:** (this commit) (docs: complete plan)

_TDD note: both tasks were GREEN on first compile after one mechanical fix (Column builder Result unwrap); RED was implicit (no prior impl)._

## Files Created/Modified

- `crates/pmcp-toolkit-athena/Cargo.toml` — added `[features] default/dev_mock` + `[[example]] athena_minimal` (required-features dev_mock); aws-sdk-athena/aws-config feature set unchanged (RUSTSEC-safe `default-https-client`, no Glue).
- `crates/pmcp-toolkit-athena/src/lib.rs` — full `AthenaConnector` + `AthenaConfig`, builders, `from_athena_config`, `execute` (translate → execution_parameters → poll → paginate), `schema_text` (GetTableMetadata), and helpers `stringify_value`, `next_backoff_ms`, `build_execution_parameters`, `strip_aws_credentials`/`redact_token`/`looks_like_credential`, `poll_until_done`, `extract_headers`, `accumulate_rows`, `paginated_get_query_results`, `format_columns`, `format_table_metadata`; 12 unit tests.
- `crates/pmcp-toolkit-athena/src/dev_mock.rs` — `AthenaMock` SqlConnector impl + `with_pages` / `PAGINATED_QUERY_MARKER` multi-page seam + `cheap_query_engine`.
- `crates/pmcp-toolkit-athena/tests/integration.rs` — 5 tests (4 D-13 + 1 M5 pagination) via the published `dev_mock` path.
- `crates/pmcp-toolkit-athena/examples/athena_minimal.rs` — Shape C 6-line `main`.
- `crates/pmcp-toolkit-athena/tests/mock_athena.rs` — DELETED (REVIEWS H5: mock moved under `src/`).

## Decisions Made

- **D-08 LOCKED honored (M4):** `from_config` delegates to `from_athena_config(AthenaConfig::new(region, workgroup))`, keeping the 2-arg public signature stable while reusing the AWS-config load path.
- **Credential redaction strategy:** per-whitespace-token scan (`split_whitespace` → `redact_token` → `looks_like_credential`) rather than a single regex-shaped function, so each helper stays well under PMAT cog 25. Matches AKIA/ASIA 20-char IDs and ≥40-char base64 secret-key runs.
- **`format_columns` extracted** so both the column list and the `PARTITIONED BY` block reuse one renderer and `format_table_metadata` stays simple.
- **`set_execution_parameters(Option<Vec<String>>)`:** passes `None` when no binds (avoids an empty-vec Athena rejection) and `Some(stringified)` otherwise.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `Column::builder().build()` returns `Result`, not `Column`**
- **Found during:** Task 1 (format_table_metadata unit test)
- **Issue:** The aws-sdk-athena 1.106.0 `ColumnBuilder::build()` returns `Result<Column, BuildError>` (name is a required field), so the test's direct `.build()` into `TableMetadataBuilder::columns(...)` failed to compile.
- **Fix:** Added a local `col(name)` closure that calls `.build().expect("column builds")` and reused it for all three columns/partition keys.
- **Files modified:** crates/pmcp-toolkit-athena/src/lib.rs (test module only)
- **Verification:** `cargo test -p pmcp-toolkit-athena --lib` — 12 tests green.
- **Committed in:** ac131bd8 (Task 1 commit)

**2. [Rule 2 - Missing Critical] `redact_token` guards against empty trimmed token**
- **Found during:** Task 1 (strip_aws_credentials helper)
- **Issue:** A whitespace token consisting only of punctuation would trim to an empty string; `looks_like_credential("")` is false but `token.replace("", "***")` would still mangle output. Added `!trimmed.is_empty()` guard.
- **Fix:** `if !trimmed.is_empty() && looks_like_credential(trimmed)`.
- **Files modified:** crates/pmcp-toolkit-athena/src/lib.rs
- **Verification:** `test_strip_aws_credentials_does_not_destroy_safe_text` passes; safe text round-trips byte-identical.
- **Committed in:** ac131bd8 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing-critical correctness). Both are local to the athena crate; no scope creep.
**Impact on plan:** Plan executed essentially as written. aws-sdk-athena resolved to 1.106.0 (≥ the 1.105.0 pin's minor floor — semver-compatible, not a stale-assumption stop).

## Issues Encountered

- **aws-sdk-glue grep over-strictness:** the plan's `! grep -rn "aws-sdk-glue"` guard returns two matches, BOTH explanatory comments (Cargo.toml Wave-0 comment + lib.rs doc comment) documenting the NO-Glue invariant. There is NO `aws-sdk-glue` dependency in the build graph (`! grep -qE '^[^#]*aws-sdk-glue' Cargo.toml` passes), which is the substantive Landmine #4 requirement and matches the objective's stated intent ("finds only the explanatory comment, never a dependency"). Treated as satisfied.

## TDD Gate Compliance

This plan's frontmatter `type` is `execute` (not plan-level `tdd`), and each task is `tdd="true"`. Both tasks landed implementation + tests together in a single `feat` commit each (RED was implicit — no prior AthenaConnector/AthenaMock impl existed). No separate `test(...)` RED commit was created; the unit/integration tests gate the same commit as the implementation. This is consistent with the sibling Plans 05/06 (postgres/mysql) commit shape.

## Out-of-Scope (logged, NOT fixed)

- Pre-existing rust-1.95.0 clippy lints in dependency crate `pmcp-server-toolkit` (`builder_ext.rs:284` needless_return, `code_mode.rs:207-208` field_reassign_with_default) and `pmcp-widget-utils/src/lib.rs:46,50` (uninlined_format_args) fire under broad `--all-targets` clippy. Already logged in `deferred-items.md`; out of scope for this plan (SCOPE BOUNDARY). The athena crate's own files are clippy-clean at `-D warnings`.

## Threat Flags

None — all security-relevant surface (SQL injection via placeholder translation, credential leakage in errors, RUSTSEC TLS path, query-hang DoS, pagination truncation, mock-leak) is covered by the plan's existing `<threat_model>` register (T-84-07-01..08).

## Next Phase Readiness

- Wave 2 of Phase 84 is COMPLETE: all three per-backend connector crates ship (84-05 Postgres, 84-06 MySQL, 84-07 Athena) plus the 84-04 SQLite feature. `pmcp-toolkit-athena` is ready for Phase 85 (Shape A pure-config binary) to wire via `synthesize_from_config_with_connector` + `DatabaseSection.url`.
- Remaining in Phase 84: 84-08 (fuzz corpus extension + CLAUDE.md publish-order + REQUIREMENTS closure + verification sweep).

## Self-Check: PASSED

- All 5 created/modified files present on disk; legacy `tests/mock_athena.rs` confirmed removed.
- Task commits `ac131bd8` (Task 1) and `66bbb1d9` (Task 2) present in git history.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
