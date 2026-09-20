---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 01
subsystem: pmcp-server-toolkit
tags: [sqlite, execute_batch, lambda, asset-resolver, http-feature, deploy-spike]
requires:
  - "SqliteConnector (CONN-08, Phase 84 Plan 04)"
  - "pmcp::assets::load_string (src/assets/loader.rs)"
  - "SqlConnector 3-method trait (locked, CONN-01)"
provides:
  - "SqliteConnector::execute_batch — multi-statement schema.sql bootstrap (inherent, concrete connector)"
  - "demo_db_path() — writable SQLite path resolver (/tmp on Lambda, relative locally)"
  - "toolkit `http` feature forwarding pmcp/streamable-http (opt-in, not default)"
  - "dev-dep tokio rt-multi-thread for the Plan 02 serving #[tokio::main] example"
  - "CONCRETE deploy single-crate-resolution spike answer for Plan 05 scope"
affects:
  - "86-02 (canonical wiring/example): calls execute_batch on concrete connector before Arc<dyn> wrap; reads via demo_db_path + pmcp::assets"
  - "86-03 (scaffold emitter): emits idempotent schema.sql + deploy.toml target_type=pmcp-run"
  - "86-05 (deploy): MUST add cargo-pmcp/src/deployment/builder.rs to files_modified (single-crate layout NOT resolved today)"
tech-stack:
  added: []
  patterns:
    - "Inherent helper on concrete connector (NOT the locked trait) to keep CONN-01 surface frozen while adding batch bootstrap"
    - "spawn_blocking + Arc<Mutex<Connection>> mirrored from execute() for execute_batch"
    - "Single-source path resolution (Review H1): config/schema via pmcp::assets, mutable DB via demo_db_path"
key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/sql/sqlite.rs (execute_batch + 3 unit tests + doctest)"
    - "crates/pmcp-server-toolkit/src/lib.rs (demo_db_path + 2 unit tests + doctest)"
    - "crates/pmcp-server-toolkit/Cargo.toml (http feature forward, dev-dep tokio rt-multi-thread)"
decisions:
  - "execute_batch is an inherent method on the concrete SqliteConnector, NOT on the SqlConnector trait — preserves the locked 3-method CONN-01 surface and forces callers to invoke it before the Arc<dyn> wrap (Review H2 / Gemini §1)"
  - "demo_db_path detects Lambda via LAMBDA_TASK_ROOT and returns /tmp/demo.db (read-only /var/task); config/schema use pmcp::assets::load_string (resolver decided ONCE, Review H1)"
  - "http feature is opt-in (NOT in default) to keep the no-default-features build lean; required because [[example]] required-features can only name toolkit features"
  - "DEPLOY SPIKE (RESEARCH Open Q#2): NO — find_lambda_package_dir does NOT resolve a single-crate layout; builder.rs MUST be added to Plan 05 scope"
metrics:
  duration: ~15min
  completed: 2026-05-27
  tasks: 2
  files: 3
---

# Phase 86 Plan 01: Toolkit Load-Bearing Pieces (execute_batch + asset/db resolver + http feature + deploy spike) Summary

Added the four toolkit pieces Shapes B/C/D depend on BEFORE the canonical wiring (Plan 02) is written: a multi-statement `SqliteConnector::execute_batch` bootstrap helper, a once-decided asset/DB path resolver (`demo_db_path()` + `pmcp::assets` for config/schema), an opt-in `http` feature forwarding `pmcp/streamable-http`, and a CONCRETE recorded answer to the deploy single-crate-resolution spike that de-risks Shape D.

## What Was Built

### Task 1 — `execute_batch` + asset/db-path resolver (commit `d75b0ee5`)

- **`SqliteConnector::execute_batch(&self, sql: &str) -> Result<(), ConnectorError>`** added as an **inherent method on the concrete `SqliteConnector`** (NOT on the locked 3-method `SqlConnector` trait). It mirrors `execute()`'s concurrency shape exactly — `Arc::clone(&self.conn)`, `sql.to_string()`, `tokio::task::spawn_blocking`, mutex locked only inside the closure — and calls `rusqlite::Connection::execute_batch`, mapping a failure to `ConnectorError::Query(_)` and a join/poison failure to `ConnectorError::Driver`. The rustdoc documents the IDEMPOTENT-bootstrap convention (`CREATE TABLE IF NOT EXISTS` / `INSERT OR IGNORE`) and the H2 "call-on-concrete-before-Arc<dyn>" constraint.
- **`demo_db_path() -> std::path::PathBuf`** added at the toolkit crate root: returns `/tmp/demo.db` when `LAMBDA_TASK_ROOT` is set (Lambda `/var/task` is read-only) and a relative `demo.db` otherwise. Module-level docs record that `config.toml`/`schema.sql` are read via `pmcp::assets::load_string(...)` (Lambda `$LAMBDA_TASK_ROOT/assets`, local cwd/`PMCP_ASSETS_DIR` — verified in `src/assets/loader.rs:93-118`), so callers never hand-roll path logic.
- **Tests (ALWAYS coverage):** `execute_batch` doctest (multi-statement → COUNT=2); 3 unit tests (multi-table seed; invalid-statement → `Err(Query)` not panic; idempotent second run leaves exactly the seeded rows); `demo_db_path` doctest + 2 unit tests (Lambda `/tmp/demo.db` vs local `demo.db`, env restored, serial under `--test-threads=1`).
- **Trait untouched:** `git diff crates/pmcp-server-toolkit/src/sql/mod.rs` is empty — the 3-method `SqlConnector` surface is preserved.

### Task 2 — `http` feature + serving-example dev-deps + deploy spike (commit `6c8cd5c8`)

- **`http = ["pmcp/streamable-http"]`** added to `[features]`, opt-in (NOT in `default`). `StreamableHttpServer` is gated behind pmcp's `streamable-http`; the toolkit's pmcp dep is `default-features = false`, and an `[[example]]`'s `required-features` can only name toolkit features — so this forward is mandatory for a serving Shape C example to compile.
- **`[dev-dependencies]` tokio widened to `["macros", "rt-multi-thread"]`** for the Plan 02 `#[tokio::main]` serving example. The published `[dependencies]` tokio line (`["sync", "rt"]`) is UNCHANGED — the library's own runtime surface is not widened.
- **No `[[example]]` slot registered** for `sql_server_http` (M2 ordering hazard) — the example file and its block land together in Plan 02.

## Deploy single-crate resolution spike (RESEARCH Open Q#2 — answered CONCRETELY, Review H3)

1. **Does `find_lambda_package_dir` (builder.rs:312) resolve a single-crate config-driven layout (project root IS the Lambda package)? → NO.** It returns a path ONLY when (a) a `<server>-lambda` subdir with a `Cargo.toml` exists, or (b) a workspace member is a `*-lambda` package exposing a `bootstrap` binary; otherwise it `bail!`s (builder.rs:343-347). A single-crate root project matches neither.
   - **Plan 05 MUST add `cargo-pmcp/src/deployment/builder.rs` to its `files_modified`** with "project root IS the Lambda package" support (when no `*-lambda` package is found and the root crate builds a `bootstrap`/deployable binary, return `self.project_root`) plus a unit test. This finding is recorded so Plan 05 already has it in scope.
2. **Detection seam:** `find_lambda_package_dir` is called from **`build_lambda_binary` (builder.rs:114) at line 132** (`let lambda_pkg_dir = self.find_lambda_package_dir(&config.server.name)?;`). That call site is where the single-crate-aware resolution must plug in. The deploy entry point `execute_async` (deploy/mod.rs:549) resolves the target via `get_target_id` then dispatches to the target's `build()` which ultimately reaches `build_lambda_binary`.
3. **`get_target_id` (deploy/mod.rs:1017) does NOT infer the target from project shape** — priority is `--target-type` flag > `config.target.target_type` (deploy.toml) > `"aws-lambda"` default. **The scaffolded `deploy.toml` MUST set `target_type = "pmcp-run"`** (M3); there is no shape-based fallback.
4. **Bundler placement (H1 validation):** `add_config_toml_to_zip` (builder.rs:570) writes `zip.start_file("config.toml", ...)` at the **zip ROOT** (builder.rs:585). `[assets] include` files are written under **`assets/<relative>`** (builder.rs:529-535). On Lambda these extract to `$LAMBDA_TASK_ROOT` so `config.toml` is at `/var/task/config.toml` and assets at `/var/task/assets/<file>` — matching exactly what `pmcp::assets::load_string("schema.sql")` resolves (`/var/task/assets/schema.sql`). The H1 resolver decision is validated against the real bundler.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p pmcp-server-toolkit --features sqlite,code-mode execute_batch -- --test-threads=1` | 3 passed |
| `cargo test -p pmcp-server-toolkit --features sqlite,code-mode demo_db_path -- --test-threads=1` | 2 passed |
| `cargo test --doc -p pmcp-server-toolkit --features sqlite,code-mode -- sqlite` | 2 passed (incl. new execute_batch doctest) |
| `cargo test --doc -p pmcp-server-toolkit --features sqlite,code-mode -- demo_db_path` | 1 passed |
| `cargo build -p pmcp-server-toolkit --features sqlite,code-mode,http` | success |
| `grep -c 'http = \["pmcp/streamable-http"\]' Cargo.toml` | 1 |
| `grep -c 'name = "sql_server_http"' Cargo.toml` | 0 (no slot pre-registered) |
| `git diff crates/pmcp-server-toolkit/src/sql/mod.rs` | empty (3-method trait intact) |
| `cargo fmt -p pmcp-server-toolkit -- --check` | clean (touched files) |

> NOTE: The verify command in the plan (`... execute_batch demo_db_path ...`) passes two positional test filters in one `cargo test` invocation, which cargo rejects (`unexpected argument`). The two filters were run as two separate invocations — both green. This is a plan-text artifact, not a behavior gap.

## Deviations from Plan

### Plan-text adjustment (not a behavior change)

**1. [Rule 3 - Blocking] Split the combined `cargo test ... execute_batch demo_db_path ...` verify command into two invocations.**
- **Found during:** Task 1 verification.
- **Issue:** `cargo test` accepts only ONE positional `TESTNAME` filter; passing both filters errors with `unexpected argument 'demo_db_path' found`.
- **Fix:** Ran `cargo test ... execute_batch ...` and `cargo test ... demo_db_path ...` separately (and the corresponding doctest filters). All green. No code change.
- **Files modified:** none.

## Deferred Issues (out of scope — SCOPE BOUNDARY)

Logged to `.planning/phases/86-shapes-b-c-d-scaffold-library-example-deploy/deferred-items.md`:

1. **Pre-existing clippy `field_reassign_with_default` (rust-1.95.0) in `code_mode.rs:520-521`** — that file was last modified in Plan 85-10 (commit `d962051e`), not touched by this plan. Consistent with prior STATE.md notes on pre-existing rust-1.95.0 lints in the toolkit surface.
2. **Pre-existing workspace `cargo fmt --all --check` failures in Phase 84 connector crates** (`pmcp-toolkit-athena`, `-mysql`, `-postgres`) — committed-clean code reflowed by a newer rustfmt version. This causes `make quality-gate`'s `fmt-check` step to fail on the untouched workspace, independent of this plan. All THREE files this plan touched are fmt-clean. Recommend a dedicated `cargo fmt --all` chore commit rather than folding it into 86-01.

   Because of (2), the full `make quality-gate` does not pass on the current workspace for reasons unrelated to this plan. There is no active pre-commit hook installed (`.git/hooks` holds only `.sample` files), so the two task commits were created with normal `git commit` (no `--no-verify`).

## Self-Check: PASSED

- `crates/pmcp-server-toolkit/src/sql/sqlite.rs` — FOUND (execute_batch present)
- `crates/pmcp-server-toolkit/src/lib.rs` — FOUND (demo_db_path present)
- `crates/pmcp-server-toolkit/Cargo.toml` — FOUND (http feature present)
- commit `d75b0ee5` — FOUND
- commit `6c8cd5c8` — FOUND
