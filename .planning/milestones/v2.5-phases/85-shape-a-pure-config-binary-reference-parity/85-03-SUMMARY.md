---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 03
subsystem: infra
tags: [pmcp-sql-server, shape-a, sqlite, chinook, fixtures, parity, code-mode, workspace-crate]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift
    provides: "pmcp-server-toolkit (ServerConfig, synthesizer, ServerBuilderExt) + sqlite feature SqliteConnector"
  - phase: 84-sql-connectors
    provides: "pmcp-toolkit-{postgres,mysql,athena} crates + SqliteConnector (SqlConnector::execute/schema_text)"
  - phase: 85-shape-a-pure-config-binary-reference-parity
    provides: "Plan 85-01 superset config fields + ${VAR} token-secret expansion in pmcp-server-toolkit"
provides:
  - "crates/pmcp-sql-server/ — the Shape A pure-config binary crate skeleton (lib/main split), registered in the workspace"
  - "Feature-gated 4-connector manifest (sqlite/postgres/mysql/athena all default-on, D-07) with chinook.db publish-exclude"
  - "Four self-contained parity fixtures vendored into the SDK repo: data-bearing chinook.db, separate chinook.ddl, generated.yaml (29-scenario contract), reference-config.toml"
  - "tests/schema_fixture.rs — 6 fixture-shape sanity tests proving the DB returns real rows, the DDL is standalone-valid, and the scenarios parse"
affects: [85-04, 85-05, 85-06, 86-shapes-bcd]

# Tech tracking
tech-stack:
  added: [clap, tracing-subscriber, rusqlite-bundled-devdep, serde_yaml-devdep, mcp-tester-devdep]
  patterns: ["lib/main split: testable run() in lib.rs + thin tokio shim in main.rs", "exclude tests/ from publish to keep a ~1MB data blob out of the crates.io tarball", "data-bearing fixture verified through the SAME SqliteConnector path the parity harness uses"]

key-files:
  created:
    - crates/pmcp-sql-server/Cargo.toml
    - crates/pmcp-sql-server/src/lib.rs
    - crates/pmcp-sql-server/src/main.rs
    - crates/pmcp-sql-server/tests/schema_fixture.rs
    - crates/pmcp-sql-server/tests/fixtures/chinook.db
    - crates/pmcp-sql-server/tests/fixtures/chinook.ddl
    - crates/pmcp-sql-server/tests/fixtures/generated.yaml
    - crates/pmcp-sql-server/tests/fixtures/reference-config.toml
  modified:
    - Cargo.toml

key-decisions:
  - "Vendored the DATA-BEARING chinook.db (~984 KB) verbatim, not a schema-only stub — generated.yaml asserts on real values ('Rock', 'AC/DC', 'For Those About To Rock'); an empty DB would fail the Plan 06 replay (REVIEW FIX #1)"
  - "exclude = [tests/, .planning/, .pmat/, fuzz/] in the crate manifest — excludes the whole tests/ tree so chinook.db (and every fixture) stays out of the published crate; cargo package --list ships only Cargo.toml + src/{lib,main}.rs"
  - "Added rusqlite (bundled) as a dev-dependency so the standalone-DDL test can execute_batch the 11-statement DDL — the toolkit's single-statement SqlConnector::execute cannot run a multi-statement schema"
  - "generated.yaml parsed via serde_yaml into mcp_tester::TestScenario (the public re-export); confirmed TestScenario.steps is a public field"

patterns-established:
  - "Shape A binary crate = lib/main split; all assembly logic in lib::run for unit-testability, main.rs is a 3-line tokio shim"
  - "Parity fixtures live under tests/fixtures and are publish-excluded; the data-bearing DB is proven through the real connector, the DDL through a fresh in-memory rusqlite::Connection"

requirements-completed: [REF-02]

# Metrics
duration: 5min
completed: 2026-05-27
---

# Phase 85 Plan 03: Shape A Crate Scaffold + Reference Parity Fixtures Summary

**Scaffolded the `pmcp-sql-server` Shape A binary crate (feature-gated 4-connector, lib/main split) and vendored four self-contained Chinook parity fixtures — a data-bearing chinook.db, a separate standalone DDL, the 29-scenario generated.yaml, and the reference config — with a 6-test fixture-shape sanity suite, all publish-excluded.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-27T00:48:08Z
- **Completed:** 2026-05-27T00:53:00Z
- **Tasks:** 2
- **Files modified:** 9 (8 created, 1 modified)

## Accomplishments
- New `crates/pmcp-sql-server/` registered in the workspace; builds under default (all 4 connectors) AND `--no-default-features --features sqlite` (lean single-backend)
- Closed RESEARCH Open Question #1: the Chinook DB + scenarios + reference config now live in the SDK repo, so the Plan 06 parity harness is CI-runnable with no cross-repo dependency
- Vendored the **data-bearing** chinook.db (~984 KB) so curated tools return the real rows generated.yaml asserts on (REVIEW FIX #1 — a schema-only DB would fail the replay)
- 6-test fixture-shape suite proves: the populated DB returns "Rock"/"AC/DC" through the real `SqliteConnector` path, the DDL builds a valid standalone 11-table schema, and generated.yaml parses as a `TestScenario`
- chinook.db (and the whole `tests/` tree) excluded from the published crate — `cargo package --list` ships only `Cargo.toml` + `src/{lib,main}.rs`

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold the pmcp-sql-server crate (manifest + lib/main stub) and register in the workspace** - `e0643e67` (feat)
2. **Task 2: Vendor the data-bearing chinook.db + DDL + scenarios + reference config and add a fixture-shape sanity test** - `69159ffe` (test)

_Task 2 is TDD: the test was written first (RED — failed to compile with unresolved `rusqlite`/`serde_yaml`/`serde_json`), then the dev-deps + fixtures landed (GREEN — 6 passing). Fixtures are data files, so RED and GREEN share one commit._

## Files Created/Modified
- `crates/pmcp-sql-server/Cargo.toml` - Feature-gated 4-connector manifest; `exclude = [tests/, .planning/, .pmat/, fuzz/]`; clap/tokio/tracing deps + rusqlite/serde_yaml/serde_json/mcp-tester dev-deps
- `crates/pmcp-sql-server/src/lib.rs` - Crate-doc + `RunConfig` scaffold + `pub async fn run() -> Ok(())` placeholder (Wave 2 replaces the body) + 2 unit tests
- `crates/pmcp-sql-server/src/main.rs` - 3-line `#[tokio::main]` shim delegating to `lib::run`
- `crates/pmcp-sql-server/tests/schema_fixture.rs` - 6 fixture-shape sanity tests (REF-02 foundation)
- `crates/pmcp-sql-server/tests/fixtures/chinook.db` - Data-bearing SQLite DB (~984 KB, connector's data source)
- `crates/pmcp-sql-server/tests/fixtures/chinook.ddl` - 11 CREATE TABLE statements (the separate `--schema` text input, D-06)
- `crates/pmcp-sql-server/tests/fixtures/generated.yaml` - 29-scenario parity contract (vendored verbatim)
- `crates/pmcp-sql-server/tests/fixtures/reference-config.toml` - Chinook reference config / Shape A target (vendored verbatim)
- `Cargo.toml` - Inserted `"crates/pmcp-sql-server"` into `[workspace.members]`

## Decisions Made
- **Data-bearing DB over schema-only stub (REVIEW FIX #1):** vendored the real chinook.db verbatim; verified `SELECT Name FROM Genre WHERE Name='Rock'` returns 1 row and an AC/DC track join returns "For Those About To Rock (We Salute You)" through the connector.
- **Whole-`tests/`-tree publish exclude:** simpler and more robust than a single-file glob; keeps the ~1 MB blob and all fixtures out of the crates.io tarball while leaving them on disk for CI.
- **rusqlite bundled dev-dep for the standalone-DDL test:** the toolkit's `SqlConnector::execute` runs one prepared statement; the 11-statement DDL needs `execute_batch`, so the test opens a fresh `rusqlite::Connection::open_in_memory()` directly (pure-Rust, no Docker — matches the toolkit's sqlite feature).
- **Verbatim vendor for config + yaml:** confirmed byte-identical to the pmcp-run source via `cmp`; the reference config's Lambda `file_path = "/var/task/assets/chinook.db"` is left untouched (Plan 06's harness re-points the connector at the vendored DB).

## Deviations from Plan

None - plan executed exactly as written. (The two added dev-deps `rusqlite` and `serde_yaml`/`serde_json` were explicitly anticipated by the plan's behaviors — `execute_batch` of the DDL and `mcp_tester::TestScenario` parse — and are dev-only, not part of the published surface.)

## Issues Encountered
- **`cargo clippy -p pmcp-sql-server --all-features -- -D warnings` surfaces lints in the `pmcp-server-toolkit` dependency, not in this crate.** These are the same pre-existing rust-1.95.0 lints (`builder_ext.rs` needless_return, `code_mode.rs` field_reassign_with_default) already logged in Phase 84's `deferred-items.md` — the local toolchain (1.95.0) is newer than CI's pinned stable. `crates/pmcp-sql-server/src/*` and `tests/schema_fixture.rs` are clippy-clean at `-D warnings`. Logged in this phase's `deferred-items.md`; NOT fixed (out of scope, owned by Phase 83 code).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- **Plan 85-04 (Wave 2)** has a place to build: it replaces `lib::run`'s placeholder body with the real config-load → connector-select → `pmcp::Server` assembly → transport-serve pipeline.
- **Plan 85-06 (parity replay)** is now self-contained: the data-bearing chinook.db + generated.yaml + reference-config.toml are all in-repo and CI-runnable; the harness points the connector at `tests/fixtures/chinook.db` (or a temp copy) and replays the 29 scenarios.
- No blockers. The deferred toolkit-dependency clippy lints persist (CI toolchain mismatch) but do not affect this crate.

## Threat Surface
No new threat surface beyond the plan's `<threat_model>`. T-85-03-01 (committed chinook.db is public Chinook demo data, excluded from publish) and T-85-03-02 (vendored generated.yaml staleness) are both accepted dispositions, realized exactly as planned.

## Self-Check: PASSED

All 8 created files exist on disk; both task commits (`e0643e67`, `69159ffe`) present in git history.

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*
