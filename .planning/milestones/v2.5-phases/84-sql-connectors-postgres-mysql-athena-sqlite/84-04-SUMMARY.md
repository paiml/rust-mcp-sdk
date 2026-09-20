---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 04
subsystem: database
tags: [sqlite, rusqlite, sql-connector, spawn-blocking, structured-content, code-mode]

# Dependency graph
requires:
  - phase: 84-01
    provides: 3-method SqlConnector trait (dialect/execute/schema_text) + ConnectorError variants
  - phase: 84-02
    provides: translate_placeholders SqlWalker (identity-on-Sqlite, yields ordered_params)
  - phase: 84-03
    provides: synthesize_from_config_with_connector + apply_widget_meta (D-06 flip) + DatabaseSection.url
provides:
  - Public SqliteConnector type behind the `sqlite` feature (CONN-08)
  - ::open(path) file-backed + ::open_in_memory() constructors
  - Full SqlConnector impl backed by rusqlite (bundled), spawn_blocking-wrapped
  - sqlite_minimal Shape C example wired to synthesize_from_config_with_connector
  - tests/synthesizer_structured_content.rs D-06 integration anchor (REVIEWS H1)
affects: [85-shape-a-pure-config-binary, 86-shapes-bcd, 88-dogfood]

# Tech tracking
tech-stack:
  added: [rusqlite-0.39-bundled, tokio-rt-for-spawn-blocking]
  patterns:
    - "spawn_blocking wraps every sync rusqlite call; std::sync::Mutex held ONLY inside the closure"
    - "schema_text fetches fresh from sqlite_master (no cached schema_blob)"
    - "Helper split (json_to_sql/sql_to_json/bind_params/collect_rows) keeps every fn under PMAT cog 25"

key-files:
  created:
    - crates/pmcp-server-toolkit/src/sql/sqlite.rs
    - crates/pmcp-server-toolkit/examples/sqlite_minimal.rs
    - crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs
  modified:
    - crates/pmcp-server-toolkit/src/sql/mod.rs
    - crates/pmcp-server-toolkit/Cargo.toml

key-decisions:
  - "sqlite feature now turns on optional tokio with the rt flag (Rule 3) — spawn_blocking is unavailable otherwise; aws keeps sync, the union lives on one optional dep"
  - "Dropped spike's schema_blob cache — schema_text reads sqlite_master live, avoiding stale-schema bugs after CREATE TABLE via execute()"
  - "MockSqlConnector kept pub(crate) and untouched (Open Question #3) — coexists with the real SqliteConnector"

patterns-established:
  - "Pattern: real-driver connector = Arc<Mutex<Connection>> + spawn_blocking per call; the same shape Plans 05/06/07 follow for Postgres/MySQL/Athena"
  - "Pattern: D-06 binding test exercises with_widget_enrichment directly rather than booting a full ServerCore"

requirements-completed: [CONN-08, CONN-01, CONN-04]

# Metrics
duration: 18min
completed: 2026-05-26
---

# Phase 84 Plan 04: SqliteConnector Promotion Summary

**Public `SqliteConnector` (rusqlite bundled) shipped behind the `sqlite` feature — `::open`/`::open_in_memory` + the full 3-method `SqlConnector` trait, with `spawn_blocking`-wrapped execution and a `sqlite_master`-driven `schema_text`; closes Wave 1 with the D-06 structuredContent integration anchor passing end-to-end.**

## Performance

- **Duration:** ~18 min
- **Tasks:** 3
- **Files created:** 3
- **Files modified:** 2

## Accomplishments

- Promoted spike 005's `sqlite_backend` into a first-class `pub struct SqliteConnector` (CONN-08) — public type, file-backed + in-memory constructors, full `SqlConnector` impl (dialect/execute/schema_text).
- `execute()` wraps every sync `rusqlite` call in `tokio::task::spawn_blocking` with a `std::sync::Mutex` held only inside the closure — the async runtime never blocks. Helpers (`json_to_sql`/`sql_to_json`/`bind_params`/`collect_rows`) keep every function under PMAT cog 25 with zero `#[allow(cognitive_complexity)]`.
- `schema_text()` reads DDL live from `sqlite_master` (dropped the spike's cached `schema_blob`), so a `CREATE TABLE` issued through `execute()` is immediately visible.
- 7 unit tests + module doctest (31 doctests green) + the moved D-06 integration test (4 tokio tests) all pass against a real in-memory rusqlite DB.
- `sqlite_minimal` Shape C example builds, runs, and prints `synthesized 0 tools`, calling the Plan-03 `synthesize_from_config_with_connector` variant (REVIEWS H3).
- Wave 1 complete: 108 lib tests green (was 101), workspace check green across the toolkit + 3 backend crates, no-default-features build still clean.

## Task Commits

1. **Task 1: SqliteConnector + full trait impl + unit tests** - `d4a03394` (feat)
2. **Task 2: sqlite_minimal Shape C example** - `98662833` (feat)
3. **Task 3: synthesizer_structured_content integration anchor** - `2392fd62` (test)

_Note: Task 1 was TDD-natured (fresh module) — tests and impl landed in one atomic commit since the module did not previously exist._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/sql/sqlite.rs` (created) - `SqliteConnector` + 4 conversion/bind helpers + 3-method trait impl + 7 unit tests + module doctest.
- `crates/pmcp-server-toolkit/src/sql/mod.rs` (modified) - `#[cfg(feature = "sqlite")] pub mod sqlite;` + `pub use sqlite::SqliteConnector;`. `MockSqlConnector` untouched.
- `crates/pmcp-server-toolkit/examples/sqlite_minimal.rs` (created) - ≤15-line Shape C `main.rs` using `synthesize_from_config_with_connector`.
- `crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs` (created) - D-06 anchor: widget_meta flip + handler `Value::Array` + `with_widget_enrichment` structured_content contract; `:name`-only seed (REVIEWS H4).
- `crates/pmcp-server-toolkit/Cargo.toml` (modified) - `sqlite` feature pulls `dep:tokio`; the optional tokio dep gains the `rt` feature alongside `sync`; new `[[example]]` entry.

## Decisions Made

- **`sqlite` feature now pulls `tokio` with `rt`.** The plan said "no Cargo changes needed for the dep itself" — true for `rusqlite`, but `tokio::task::spawn_blocking` is not reachable without `tokio` as a real (non-dev) dependency under the `sqlite` feature. Added `dep:tokio` to the feature and `rt` to the optional tokio dep's feature list (it previously carried only `sync` for the `aws` providers). Both features now share one optional dep with the union of flags. The no-default-features build still compiles, confirming the gate is correct.
- **Dropped the spike's `schema_blob` cache.** `schema_text()` queries `sqlite_master` on each call (the plan's stated intent at objective lines 102/155). Negligible overhead, no stale-schema risk.
- **`MockSqlConnector` left in place** (`pub(crate)`, Open Question #3) — the new real connector coexists with the test fixture.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `sqlite` feature did not enable `tokio` runtime needed for `spawn_blocking`**
- **Found during:** Task 1 (first build of `sqlite.rs`)
- **Issue:** `error[E0433]: cannot find module or crate tokio` — the optional `tokio` dep was gated behind `aws` only and carried just the `sync` feature; the `sqlite` feature never enabled it, and `rt` (required for `spawn_blocking`) was absent.
- **Fix:** Added `"dep:tokio"` to the `sqlite` feature and `"rt"` to the optional tokio dependency's feature list (now `["sync", "rt"]`).
- **Files modified:** `crates/pmcp-server-toolkit/Cargo.toml`
- **Verification:** `cargo build -p pmcp-server-toolkit --features sqlite` green; `--no-default-features` build still green (gate correct).
- **Committed in:** `d4a03394` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to compile the connector; no scope creep — the change is minimal and feature-gated.

## Issues Encountered

None beyond the Task-1 blocking dependency fix above. All verification steps passed first time after that fix.

## Threat Flags

None — the implementation stays within the plan's `<threat_model>`. T-84-04-01 (parametric binding via `raw_bind_parameter`, never concatenation) is satisfied by `bind_params`; T-84-04-03 (mutex poisoning → recoverable `ConnectorError::Driver`) is implemented.

## Known Stubs

None — `SqliteConnector` is a real driver with live `sqlite_master` schema reads and parametric binding; no placeholder/empty-value flows.

## Deferred Issues

Pre-existing local-toolchain (rust-1.95.0) clippy errors in `builder_ext.rs` (`clippy::needless_return`) and `code_mode.rs` (`clippy::field_reassign_with_default`) surface when the lib is compiled at `-D warnings`. These are unrelated to this plan's files, already logged in `deferred-items.md`, and out of scope per the executor SCOPE BOUNDARY. The new `sql/sqlite.rs`, `examples/sqlite_minimal.rs`, and `tests/synthesizer_structured_content.rs` are clippy-clean at `-D warnings`.

## User Setup Required

None - no external service configuration required (rusqlite `bundled` is pure-Rust, no system SQLite).

## Next Phase Readiness

- Wave 1 of Phase 84 is complete: 84-01 (trait/errors) → 84-02 (translate) → 84-03 (synthesizer + structuredContent) → 84-04 (SqliteConnector).
- `SqliteConnector` is the reference real-driver shape (Arc<Mutex<Connection>> + spawn_blocking) that Plans 05/06/07 follow for Postgres/MySQL/Athena.
- Phase 85 (Shape A pure-config binary) can now wire a real connector via `synthesize_from_config_with_connector`; `DatabaseSection.url` (84-03) feeds the per-backend URL constructors (D-08).

## Self-Check: PASSED

All 4 created files present; all 3 task commits (`d4a03394`, `98662833`, `2392fd62`) found in git history.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
