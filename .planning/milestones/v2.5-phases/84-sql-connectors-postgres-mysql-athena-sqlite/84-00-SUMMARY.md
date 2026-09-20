---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 00
subsystem: database
tags: [postgres, mysql, athena, sqlx, tokio-postgres, aws-sdk-athena, workspace, proptest, fuzz, sql-connector]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit
    provides: "SqlConnector trait (2-method stub), Dialect enum, ConnectorError, pmcp-server-toolkit crate + config parser fuzz target"
provides:
  - "Three new workspace member crates (pmcp-toolkit-postgres, pmcp-toolkit-mysql, pmcp-toolkit-athena) compiling as stubs"
  - "src/sql/translate.rs module with TranslatedSql struct + stub translate_placeholders + 5 RED property tests"
  - "Public surface pmcp_server_toolkit::sql::{translate_placeholders, TranslatedSql} (D-05 free helper)"
  - "Fuzz corpus seed exercising the [database].url key Plan 03 adds"
affects: [84-01, 84-02, 84-05, 84-06, 84-07]

# Tech tracking
tech-stack:
  added:
    - "tokio-postgres 0.7.17 + deadpool-postgres 0.14.1 (Postgres crate)"
    - "sqlx 0.8.6 (mysql + runtime-tokio + tls-rustls-aws-lc-rs)"
    - "aws-sdk-athena 1.105.0 + aws-config 1.8.16 (default-https-client, no aws-sdk-glue)"
  patterns:
    - "Per-backend crate skeleton: Cargo.toml + src/lib.rs stub + tests/mock_*.rs + tests/integration.rs + examples/*_minimal.rs"
    - "RED-first property tests: proptest functions panic with explicit Plan-NN marker until impl lands"
    - "Free-helper SQL surface (translate_placeholders), not a trait method (D-05)"

key-files:
  created:
    - "crates/pmcp-toolkit-postgres/{Cargo.toml,src/lib.rs,tests/mock_postgres.rs,tests/integration.rs,examples/postgres_minimal.rs}"
    - "crates/pmcp-toolkit-mysql/{Cargo.toml,src/lib.rs,tests/mock_mysql.rs,tests/integration.rs,examples/mysql_minimal.rs}"
    - "crates/pmcp-toolkit-athena/{Cargo.toml,src/lib.rs,tests/mock_athena.rs,tests/integration.rs,examples/athena_minimal.rs}"
    - "crates/pmcp-server-toolkit/src/sql/translate.rs"
    - "crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-database-url.toml"
  modified:
    - "Cargo.toml (workspace.members — three new entries)"
    - "crates/pmcp-server-toolkit/src/sql/mod.rs (pub mod translate + re-export)"

key-decisions:
  - "Kept the documented `# NO aws-sdk-glue` comment in athena Cargo.toml per PATTERNS line 470 — substantive criterion (no Glue dependency) is satisfied; the literal-grep criterion was written assuming zero mention."
  - "tests/integration.rs uses #[path = \"mock_<backend>.rs\"] mod to pull the mock as a submodule (Rust 2021 tests/ layout) — avoids the mock file compiling as a standalone test binary with no #[test] functions."
  - "proptest was already a dev-dependency on pmcp-server-toolkit (Phase 83) — no Cargo.toml change needed for Task 2."

patterns-established:
  - "RED property-test scaffold: 5 proptest! functions each panic with 'RED — Plan 02 implements' so the RED gate is explicit and machine-greppable."
  - "Stub connector constructors return ConnectorError::Schema(\"<Backend>: unimplemented — Plan 0N\") to make the deferred wave obvious in error text."

requirements-completed: [CONN-05, CONN-06, CONN-07, TEST-01, TEST-07]

# Metrics
duration: ~25min
completed: 2026-05-26
---

# Phase 84 Plan 00: Wave 0 Connector Scaffolding Summary

**Three per-backend workspace crates (Postgres/MySQL/Athena) stubbed and compiling, plus the `translate.rs` module shell with `TranslatedSql` + 5 RED property tests and a `[database].url` fuzz corpus seed — Wave 0 shape that unblocks Waves 1-2.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 3
- **Files modified:** 18 (17 created + 1 modified root Cargo.toml; mod.rs modified)

## Accomplishments
- Three new workspace member crates created and recognized by `cargo metadata` (3/3); all `cargo check -p` green.
- Driver versions pinned exactly per RESEARCH §1: tokio-postgres 0.7.17, deadpool-postgres 0.14.1, sqlx 0.8.6 (tls-rustls-aws-lc-rs), aws-sdk-athena 1.105.0 + aws-config 1.8.16. No `aws-sdk-glue` dependency (Landmine #4).
- `src/sql/translate.rs` module shell: `TranslatedSql { sql, ordered_params }` (Debug/Clone/PartialEq/Eq), stub `translate_placeholders` returning input verbatim, and 5 proptest functions RED with explicit "Plan 02 implements" panic. Public surface re-exported from `pmcp_server_toolkit::sql` (D-05). Doctest green.
- Fuzz corpus seeded with `seed-database-url.toml` exercising `[database] url = "env:DATABASE_URL"`; no new fuzz target (D-14, extend-don't-duplicate).
- No `[[example]]` blocks added to the new crates (WARNING #7 — Plans 05/06/07 add them with correct `required-features`).

## Task Commits

Each task was committed atomically:

1. **Task 1: Create three per-backend workspace crate skeletons** - `aaf3e468` (feat)
2. **Task 2: translate.rs module shell + 5 RED property tests** - `c11f0962` (test — RED gate)
3. **Task 3: Seed fuzz corpus with [database].url snippet** - `9e708527` (test)

**Plan metadata:** (this commit — docs: complete plan)

_Note: Task 2 is a TDD task in RED state. Per the plan, only the RED gate lands in this plan; the GREEN feat commit (real SqlWalker impl) belongs to Plan 02._

## Files Created/Modified
- `crates/pmcp-toolkit-postgres/Cargo.toml` - Postgres crate manifest (tokio-postgres 0.7.17 + deadpool-postgres 0.14.1)
- `crates/pmcp-toolkit-postgres/src/lib.rs` - `PostgresConnector::connect` stub
- `crates/pmcp-toolkit-postgres/tests/{mock_postgres,integration}.rs` - mock + integration anchor shells
- `crates/pmcp-toolkit-postgres/examples/postgres_minimal.rs` - buildable Shape C shell
- `crates/pmcp-toolkit-mysql/*` - same shape, sqlx 0.8.6 (mysql + tls-rustls-aws-lc-rs)
- `crates/pmcp-toolkit-athena/*` - same shape, aws-sdk-athena 1.105.0 + aws-config 1.8.16, `from_config(region, workgroup)` stub
- `crates/pmcp-server-toolkit/src/sql/translate.rs` - `TranslatedSql` + stub helper + 5 RED proptests
- `crates/pmcp-server-toolkit/src/sql/mod.rs` - `pub mod translate;` + `pub use translate::{translate_placeholders, TranslatedSql};`
- `crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-database-url.toml` - corpus seed
- `Cargo.toml` (root) - three new workspace members after `crates/pmcp-server-toolkit`

## Decisions Made
- **`# NO aws-sdk-glue` comment retained** in athena Cargo.toml. The PATTERNS map (line 470) prescribes this exact comment as the documented-absence marker. See Deviations.
- **`#[path = "mock_<backend>.rs"] mod`** in each `tests/integration.rs` rather than a bare `mod mock_<backend>;`. Both forms resolve the sibling file, but the explicit `#[path]` makes the layout unambiguous and keeps the mock from being treated as a standalone test binary.
- **proptest dev-dependency already present** on pmcp-server-toolkit from Phase 83, so Task 2 added no Cargo.toml change (plan said "if not already present").

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Acceptance-criterion literal-grep on `aws-sdk-glue` vs. documented-absence comment**
- **Found during:** Task 1 (Athena Cargo.toml)
- **Issue:** Task 1's acceptance criterion says `git grep -n "aws-sdk-glue" crates/pmcp-toolkit-athena/Cargo.toml` returns empty. The PATTERNS map (line 470) prescribes an explicit `# NO aws-sdk-glue — GetTableMetadata covers schema introspection` comment as the Landmine-#4 marker. These two directives conflict on the literal string, not on the substance.
- **Fix:** Kept the documented-absence comment (PATTERNS is the authoritative pattern source). Verified there is NO actual `aws-sdk-glue = ...` dependency line via `grep -E "^\s*aws-sdk-glue\s*="` (returns empty). The substantive requirement — no Glue dependency in the build graph — is fully satisfied; `cargo check` resolves zero glue crates.
- **Files modified:** crates/pmcp-toolkit-athena/Cargo.toml
- **Verification:** `grep -E "^\s*aws-sdk-glue\s*=" crates/pmcp-toolkit-athena/Cargo.toml` empty; full `cargo check -p pmcp-toolkit-athena` green with no glue crate compiled.
- **Committed in:** aaf3e468 (Task 1 commit)

---

**Total deviations:** 1 (criterion-vs-pattern reconciliation; no production-logic change)
**Impact on plan:** None on deliverables. The no-Glue invariant (Landmine #4) holds; only the literal-grep wording of one acceptance criterion was superseded by the PATTERNS-prescribed comment. No scope creep.

## Issues Encountered
- The toolkit `fuzz/corpus/` directory was entirely untracked at plan start (pre-existing libfuzzer hash-named artifacts never committed). Staged ONLY the new `seed-database-url.toml` (1 file) explicitly to avoid pulling thousands of build-output corpus files into the commit and to honor the git-safety instruction (preserve untracked artifacts).
- `aws-sdk-athena` resolved to 1.106.0 at compile time from the `"1.105.0"` minimum pin (semver-compatible). The manifest carries the exact `"1.105.0"` string the criteria require; cargo's resolution to 1.106 is expected and harmless.

## User Setup Required
None - no external service configuration required. (Real driver connections land in Waves 1-2; Wave 0 is compile-only scaffolding.)

## Next Phase Readiness
- Workspace shape is ready for Wave 1 (toolkit core extension: `execute()`, `ConnectorError` variants, `SqliteConnector`) and Wave 2 (per-backend crate bodies in Plans 05/06/07).
- The 5 RED property tests in `src/sql/translate.rs` turn GREEN in Plan 02 once `translate_placeholders` ships the `SqlWalker` state machine.
- Plans 05/06/07 must add their `[[example]]` blocks with correct `required-features` when the example bodies land (WARNING #7).
- Note for Plan 03: the `[database].url` field is not yet on `DatabaseSection` in `config.rs` — the fuzz seed anticipates it. The seed will only fully exercise the new key once Plan 03 adds the field (until then, strict-parse rejects `url` as unknown, which is still valid no-panic fuzz input).

## Self-Check: PASSED

All 9 spot-checked created files exist on disk; all 3 task commit hashes (aaf3e468, c11f0962, 9e708527) present in git history.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
