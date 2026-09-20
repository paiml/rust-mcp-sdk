---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 02
subsystem: pmcp-server-toolkit
tags: [sqlite, code-mode, streamable-http, example, shape-c, asset-resolver, mcp-tester]

requires:
  - phase: 86-01
    provides: "SqliteConnector::execute_batch (concrete), demo_db_path() H1 resolver, toolkit `http` feature forwarding pmcp/streamable-http, dev-dep tokio rt-multi-thread"
  - phase: 84
    provides: "SqliteConnector (CONN-08), SqlConnector 3-method trait"
  - phase: 83
    provides: "ServerConfig::from_toml_strict_validated, try_*_from_config_with_connector builder ext, StaticResourceHandler, code-mode wiring"
provides:
  - "examples/sql_server_http.rs — THE canonical ≤15-line Shape C wiring (12 statement-line main body) that Plan 03 emits byte-identically and Plan 05 deploys unchanged"
  - "examples/fixtures/config.toml — strict-parse-valid config with [code_mode] inline DEV token_secret"
  - "examples/fixtures/schema.sql — idempotent demo DDL+seed (CREATE TABLE IF NOT EXISTS / INSERT OR IGNORE)"
  - "tests/sql_server_http_example.rs — ChildGuard subprocess integration test + the shared M4 ≤15-line body assertion"
  - "private `serve(server)` helper shape (with_config + start) the scaffold reuses (CONVERGENCE NOTE)"
affects:
  - "86-03 (scaffold emitter): emits a byte-identical main.rs minus the PMCP_ASSETS_DIR harness line; reuses the serve() helper shape and the idempotent schema.sql"
  - "86-05 (deploy): deploys this exact wiring unchanged; config.toml at /var/task, schema.sql at /var/task/assets, DB at /tmp/demo.db"

tech-stack:
  added: ["mcp-tester (dev-dep, dev-only — tests/ excluded from published artifact)"]
  patterns:
    - "H1 asset/DB resolver in library code: pmcp::assets::load_string for config/schema + demo_db_path() for the mutable DB — same wiring local and on Lambda"
    - "H2 connector ordering: execute_batch on the CONCRETE SqliteConnector BEFORE the Arc<dyn SqlConnector> wrap (shadow rebind)"
    - "Line-budget discipline: hoist harness setup (FIXTURES_DIR const) + HTTP boilerplate (serve() helper) OUT of main so the body stays ≤15 statement lines under rustfmt"
    - "M4 statement-line counting: count physical non-blank/non-comment lines but skip rustfmt method-chain continuations (trimmed start `.`) and lone closing-delimiter continuations — a wrapped statement counts once"
    - "ChildGuard(Drop-kill) + machine-readable PMCP_SQL_SERVER_ADDR= line parse + captured-output-on-timeout subprocess integration-test pattern for examples (no CARGO_BIN_EXE for examples → spawn `cargo run --example`)"

key-files:
  created:
    - "crates/pmcp-server-toolkit/examples/sql_server_http.rs"
    - "crates/pmcp-server-toolkit/examples/fixtures/config.toml"
    - "crates/pmcp-server-toolkit/examples/fixtures/schema.sql"
    - "crates/pmcp-server-toolkit/tests/sql_server_http_example.rs"
  modified:
    - "crates/pmcp-server-toolkit/Cargo.toml ([[example]] sql_server_http slot + mcp-tester dev-dep)"

key-decisions:
  - "Added a private `serve(server)` helper in the example (CONVERGENCE NOTE) to collapse with_config+start into one main-body line — required to stay ≤15 statement lines once rustfmt wraps the builder chain and the with_config args. It inlines the StreamableHttpServer body (does NOT import pmcp_sql_server::serve, Pitfall §2). Plan 03 emits a call to the same shape."
  - "Defined the M4 ≤15-line assertion to count STATEMENT lines, not raw physical lines: rustfmt forcibly wraps method chains (.name/.version, .resources_arc/.build) and multi-arg calls across physical lines, so continuation lines (trimmed start `.`, or lone closing delimiters) are excluded. main body = 12 statement lines."
  - "PMCP_ASSETS_DIR is set inside main from a module-level FIXTURES_DIR const so `cargo run --example` resolves fixtures from any cwd; documented that Plan 03's scaffold DROPS this line because its assets are cwd-local."
  - "Integration test spawns `cargo run --example` (examples get no CARGO_BIN_EXE_<example>) and parses the printed PMCP_SQL_SERVER_ADDR= line for the ephemeral bound port."

patterns-established:
  - "Shape C canonical wiring (≤15 lines, H1 resolver, H2 ordering, inline-secret config) locked for Plan 03 emission + Plan 05 deploy"

requirements-completed: [SHAP-C-01]

duration: ~35min
completed: 2026-05-27
---

# Phase 86 Plan 02: Shape C ≤15-line Serving Example + Integration Test Summary

**A runnable `cargo run --example sql_server_http` streamable-HTTP MCP server in a 12-statement-line `main` body — loads config/schema via `pmcp::assets::load_string`, opens SQLite at `demo_db_path()`, bootstraps on the concrete connector (H2), wires tools + code_mode (inline DEV secret) + resources, and is proven by a ChildGuard subprocess test (initialize → tools/list → tools/call) plus a shared M4 ≤15-line body assertion.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-05-27
- **Completed:** 2026-05-27
- **Tasks:** 2
- **Files modified:** 5 (4 created, 1 modified)

## Accomplishments

- **The canonical Shape C wiring (SHAP-C-01).** `examples/sql_server_http.rs` composes the verified toolkit API into a serving HTTP MCP server; its `main` body is 12 statement lines (≤15, M4). This is THE shape Plan 03's scaffold emits byte-identically and Plan 05 deploys unchanged.
- **Asset-aware + Lambda-compatible by construction.** Config/schema are read via `pmcp::assets::load_string` (cwd/`PMCP_ASSETS_DIR` locally, `/var/task/assets/` on Lambda) and SQLite opens at `demo_db_path()` (`/tmp/demo.db` on Lambda) — the Plan 01 H1 resolver, decided once.
- **H2 connector ordering enforced.** `execute_batch` runs on the CONCRETE `SqliteConnector` (line 57) before the `Arc<dyn SqlConnector>` shadow-wrap (line 59); calling it on the trait object would not compile.
- **Idempotent demo bootstrap.** `schema.sql` uses `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE`; verified a second run against a persisted `/tmp/demo.db` succeeds and leaves exactly 3 seeded rows.
- **ChildGuard subprocess integration test** proves initialize → tools/list (`list_books` present) → tools/call (`list_books` with `{limit:5}`) over real HTTP, parsing the machine-readable `PMCP_SQL_SERVER_ADDR=` line (M1), with a Drop-kill guard and captured-output-on-timeout panic.
- **Shared M4 ≤15-line body assertion** with a documented statement-line counter that tolerates rustfmt's chain wrapping.

## Task Commits

1. **Task 1: ≤15-line serving Shape C example + idempotent fixtures + Cargo slot** — `ba886e43` (feat)
2. **Task 2: ChildGuard subprocess integration test + ≤15-line body assertion** — `0ca43a03` (test)

_Note: Task 1 carried `tdd="true"`. The behavioral test for the example IS the Task 2 integration test (per the plan's explicit task split), so Task 1 was committed as the `feat` artifact and Task 2 as the `test` proving its behavior — a single RED→GREEN arc across the two task commits rather than an intra-task test/feat pair._

## Files Created/Modified

- `crates/pmcp-server-toolkit/examples/sql_server_http.rs` — canonical Shape C wiring; module-level `FIXTURES_DIR` const + `serve(server)` helper keep the `main` body at 12 statement lines.
- `crates/pmcp-server-toolkit/examples/fixtures/config.toml` — strict-parse-valid; `[code_mode] enabled` + inline DEV `token_secret` (≥16 bytes) + `allow_inline_token_secret_for_dev = true` + loud DEV-ONLY comment; `[[tools]] list_books` with a `limit` param.
- `crates/pmcp-server-toolkit/examples/fixtures/schema.sql` — idempotent `books` DDL + 3 `INSERT OR IGNORE` seeds.
- `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs` — `ChildGuard`, address-line parse, readiness poll, tools/list + tools/call assertions, and `example_body_is_at_most_15_lines`.
- `crates/pmcp-server-toolkit/Cargo.toml` — `[[example]] sql_server_http` (required-features `sqlite,code-mode,http`) + `mcp-tester` dev-dependency.

## Decisions Made

- **Added a private `serve(server)` helper (CONVERGENCE NOTE, plan-sanctioned).** Once rustfmt wraps the builder chain and the `with_config` call across physical lines, inlining steps 6–8 in `main` pushed past 15 lines. Collapsing `with_config` + `start` into one helper call (still inlining the `StreamableHttpServer` body, NOT importing `pmcp_sql_server::serve`) keeps the body at 12 statement lines. The scaffold reuses this exact shape.
- **M4 counts statements, not raw physical lines.** The plan's M4 wording is "physical non-empty body STATEMENT lines." rustfmt forcibly splits method chains and multi-arg calls, so the assertion skips continuation lines (trimmed start `.`, or lone closing delimiters like `)`, `);`, `)?;`). A statement rustfmt wraps counts once. This matches the plan's intent and is documented verbatim in both the example and the test.
- **`cargo run --example` subprocess (not CARGO_BIN_EXE).** Examples do not get a `CARGO_BIN_EXE_<example>` env, so the test spawns `cargo run --example` and parses the printed address — the plan's documented fallback.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed needless `mut` on the ChildGuard binding**
- **Found during:** Task 2 (integration test)
- **Issue:** `let mut guard = ChildGuard(child);` triggered `unused_mut` (a `-D warnings` failure under the project's clippy gate); the binding is only read, then `drop`ped.
- **Fix:** Changed to `let guard = ChildGuard(child);`.
- **Files modified:** crates/pmcp-server-toolkit/tests/sql_server_http_example.rs
- **Verification:** `cargo clippy --test sql_server_http_example -- -D warnings` shows zero warnings attributable to this file; test re-run green (2 passed).
- **Committed in:** `0ca43a03` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug).
**Impact on plan:** Trivial lint fix; no scope creep. The plan's anti-pattern guidance (do NOT re-implement build_server) was honored — the `serve()` helper is a 3-line transport collapse, not a server-assembly reimplementation.

## Issues Encountered

- **rustfmt vs. the ≤15-line budget.** The verified RESEARCH §1 body assumed compact statements, but rustfmt wraps the builder chain (`.name`/`.version`, `.resources_arc`/`.build`), the `execute_batch().await`, and the `with_config(...)` call across multiple physical lines. Resolved by (a) hoisting harness/boilerplate out of `main` (`FIXTURES_DIR` const, `serve()` helper) and (b) defining the M4 assertion to count statements rather than raw physical lines. Final body: 12 statement lines.

## Pre-existing Failures (NOT introduced by this plan — out of scope)

Per the orchestrator note and Plan 86-01's `deferred-items.md`, these pre-date this plan and are being handled separately at phase-end. They were NOT touched:

1. **`field_reassign_with_default` clippy lint (rust-1.95.0) in `crates/pmcp-server-toolkit/src/code_mode.rs:520-521`** — last modified in Plan 85-10, not by this plan. Surfaces under `clippy -- -D warnings` on the toolkit lib target. The files THIS plan created are clippy-clean.
2. **Workspace `cargo fmt --all --check` drift in Phase 84 connector crates** (`pmcp-toolkit-{athena,mysql,postgres}`) — committed-clean code reflowed by a newer rustfmt. Makes the full `make quality-gate` fmt step fail on the untouched workspace. All files THIS plan touched are `cargo fmt -p pmcp-server-toolkit --check`-clean.
3. **Pre-existing benign rustfmt reflow in two test files** (`crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs`, `crates/pmcp-sql-server/tests/schema_fixture.rs`) — left untouched and NOT staged per the orchestrator instruction.

Because of (1) and (2), the full `make quality-gate` does not pass on the current workspace for reasons unrelated to this plan. No active pre-commit hook is installed, so both task commits were created with normal `git commit` (no `--no-verify`).

## Verification

| Check | Result |
|-------|--------|
| `cargo build -p pmcp-server-toolkit --example sql_server_http --features sqlite,code-mode,http` | success |
| `cargo run --example sql_server_http ...` (1st run) | serves; prints `PMCP_SQL_SERVER_ADDR=http://127.0.0.1:<port>` |
| `cargo run --example sql_server_http ...` (2nd run, persisted /tmp/demo.db) | idempotent — succeeds, COUNT(books)=3 |
| `cargo test -p pmcp-server-toolkit --features sqlite,code-mode,http --test sql_server_http_example -- --test-threads=1` | 2 passed (integration + ≤15-line assertion) |
| H1 source assertion (`assets::load_string` + `demo_db_path` present) | both present in code |
| H2 source assertion (execute_batch@57 < Arc<dyn> wrap@59) | ordering correct |
| Pitfall §2 (no `pmcp_sql_server::serve` import) | only match is the doc-comment saying NOT to import it |
| code_mode wiring + `PMCP_SQL_SERVER_ADDR=` print | both present |
| fixture config (inline secret ≥16B + dev flag + DEV-ONLY comment) | present |
| schema.sql idempotent (`IF NOT EXISTS` + `INSERT OR IGNORE`) | present |
| Cargo `[[example]]` slot (required-features sqlite,code-mode,http) | registered |
| test ChildGuard `impl Drop` + address-line parse | present |
| `cargo fmt -p pmcp-server-toolkit -- --check` | clean (this plan's files) |

## Next Phase Readiness

- **Plan 03 (scaffold emitter)** can now emit a byte-identical `main.rs` (dropping the `PMCP_ASSETS_DIR` harness line) and reuse the `serve()` helper shape + idempotent `schema.sql`.
- **Plan 05 (deploy)** inherits this exact wiring; the H1 resolver maps `config.toml` → `/var/task`, `schema.sql` → `/var/task/assets`, DB → `/tmp/demo.db` (validated against the bundler in 86-01).
- No blockers introduced. Pre-existing workspace fmt/clippy drift (items 1–2 above) remains for the phase-end fix.

## Self-Check: PASSED

- `crates/pmcp-server-toolkit/examples/sql_server_http.rs` — FOUND
- `crates/pmcp-server-toolkit/examples/fixtures/config.toml` — FOUND
- `crates/pmcp-server-toolkit/examples/fixtures/schema.sql` — FOUND
- `crates/pmcp-server-toolkit/tests/sql_server_http_example.rs` — FOUND
- commit `ba886e43` — FOUND
- commit `0ca43a03` — FOUND
