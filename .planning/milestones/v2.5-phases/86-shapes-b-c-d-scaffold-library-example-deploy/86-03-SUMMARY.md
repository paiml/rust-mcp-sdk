---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 03
subsystem: cargo-pmcp
tags: [scaffold, sql-server, single-crate, code-mode, sqlite, streamable-http, golden-test, shape-b]

requires:
  - phase: 86-02
    provides: "examples/sql_server_http.rs — THE canonical Shape C wiring this scaffold emits byte-identically (modulo the PMCP_ASSETS_DIR harness line); idempotent fixtures config.toml/schema.sql"
  - phase: 86-01
    provides: "SqliteConnector::execute_batch (concrete), demo_db_path() H1 resolver, toolkit `http` feature forwarding pmcp/streamable-http"
provides:
  - "cargo pmcp new --kind sql-server <name> — single-crate emitter (Cargo.toml + src/main.rs + config.toml + schema.sql) distinct from the multi-crate workspace path"
  - "templates::sql_server::generate — one generate_<file> fn per output, raw fs::write of string literals (no template engine), mirroring workspace.rs"
  - "validate_crate_name — rejects empty/leading-digit/non-[A-Za-z0-9_-]/path-traversal names BEFORE any fs::write (T-86-03-02)"
  - "golden test emitted_main_matches_example_modulo_setup — the emitted main.rs cannot drift from the Plan 02 example"
affects:
  - "86-05 (deploy): the emitted single-crate layout (config.toml + schema.sql + pmcp-server-toolkit dep) is the detection target for config-only deploy; the emitted main.rs deploys unchanged (H1)"
  - "86-04 (scaffold integration test): can `new --kind sql-server` into a tempdir then cargo run the emitted crate end-to-end"

tech-stack:
  added: []
  patterns:
    - "Single-crate template emitter: mirror workspace.rs (one generate_<file> fn per output, each a raw fs::write of an r#\"...\"# / format! literal — no template engine; format! escapes literal braces as {{ }})"
    - "CLI --kind allowlist branch: match kind.as_deref() { Some(\"sql-server\") => single-crate, Some(k) => bail unknown, None => existing workspace path } — additive, leaves the None path untouched"
    - "Crate-name validation BEFORE any fs::write (defense beyond the dir-exists guard): reject empty/leading-digit/illegal-char/path-traversal"
    - "Golden source-assertion test: include_str! the upstream example + a wiring_lines() normalizer (drop blanks, //! //  comments, harness-only FIXTURES_DIR const + PMCP_ASSETS_DIR line) to prove the emitted main.rs == the example modulo the harness seam"

key-files:
  created:
    - "cargo-pmcp/src/templates/sql_server.rs"
  modified:
    - "cargo-pmcp/src/main.rs (New {} variant gains #[arg(long)] kind, threaded through dispatch)"
    - "cargo-pmcp/src/commands/new.rs (kind param + sql-server branch + validate_crate_name + execute_sql_server + print_sql_server_next_steps)"
    - "cargo-pmcp/src/templates/mod.rs (pub mod sql_server;)"
    - "cargo-pmcp/README.md (scoped Config-Driven SQL Server + config-only deploy sections)"

key-decisions:
  - "Committed the emitter module (sql_server.rs) in Task 1 rather than Task 2 because new::execute routes into it — a Task-1-only commit would not compile. Task 1 = the buildable CLI surface + validation + routing + emitter; Task 2 = the scoped README docs. The golden test ships with the emitter (Task 1)."
  - "Golden test normalizes by wiring lines (strip trailing line comments + filter blanks/comments/harness seams) rather than a brittle byte diff, because rustfmt wraps both the example and the emitted literal differently and the example carries a longer //! header + serve() doc. The wiring (statements) is what must not drift; the test asserts line-for-line equality of the normalized wiring."
  - "emitted_main_has_h1_h2_wiring_tokens asserts token PRESENCE (not strict source order) because serve() sits above main, so StreamableHttpServer::with_config legitimately precedes try_code_mode_*. H2 ordering (execute_batch before the Arc<dyn> wrap) is asserted by index comparison; H1 (no PMCP_ASSETS_DIR) by absence."
  - "Config-only deploy is documented (folded scoped README todo) but NOT implemented here — it is Plan 05's deliverable. The README describes the intended detection-based behavior."

patterns-established:
  - "cargo pmcp new --kind <X> single-crate scaffolding path (allowlisted, validated, golden-guarded) — extensible to future kinds without touching the workspace path"

requirements-completed: [SHAP-B-01]

duration: ~7min
completed: 2026-05-27
---

# Phase 86 Plan 03: Shape B — `cargo pmcp new --kind sql-server` Single-Crate Scaffold Summary

**`cargo pmcp new --kind sql-server <name>` now emits a single runnable crate (Cargo.toml + src/main.rs + config.toml + schema.sql) whose asset-aware main.rs IS the Plan 02 Shape C wiring (H1 `assets::load_string`+`demo_db_path`, H2 `execute_batch`-before-`Arc<dyn>`), guarded by a golden drift test, with crate-name validation before any fs::write and an inline-DEV code-mode config — so the same crate runs locally AND deploys unchanged.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-05-27
- **Completed:** 2026-05-27
- **Tasks:** 2
- **Files modified:** 5 (1 created, 4 modified)

## Accomplishments

- **`--kind sql-server` command surface (D-01, SHAP-B-01).** The `New {}` CLI variant gained `#[arg(long)] kind: Option<String>` (mirrors `--path`), threaded through the single dispatch site in `main.rs`. `new::execute` branches: `Some("sql-server")` routes to the single-crate emitter, `Some(other)` bails with `unknown --kind '<x>'; supported: sql-server`, `None` runs the existing multi-crate workspace path **unchanged**.
- **Single-crate emitter (`templates::sql_server`).** Mirrors `workspace.rs`: one `generate` orchestrator + four `generate_<file>` fns, each a raw `fs::write` of a string literal (no template engine; `format!` escapes literal braces). Emits `Cargo.toml`, `src/main.rs`, `config.toml`, `schema.sql`.
- **Asset-aware + Lambda-compatible emitted `main.rs` (H1).** Reads `config.toml`/`schema.sql` via `pmcp::assets::load_string` and opens SQLite at `demo_db_path()`; the ONLY deviation from the Plan 02 example is the dropped `PMCP_ASSETS_DIR` harness line — so the same `main.rs` runs locally (cwd-local assets) AND on Lambda (`/var/task/assets/`).
- **H2 connector ordering preserved.** Emitted `main.rs` calls `execute_batch` on the concrete `SqliteConnector` (line 35) before the `Arc<dyn SqlConnector>` wrap (line 37).
- **Idempotent emitted `schema.sql` (H2).** `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE` with semicolon-free seed values — a second `cargo run` against a persisted `demo.db` succeeds.
- **Inline-DEV code-mode `config.toml` (D-06).** `[code_mode] enabled = true`, `allow_writes = false` (default-deny, T-86-03-04), `token_secret` ≥16 bytes + `allow_inline_token_secret_for_dev = true`, with a loud DEV-ONLY comment naming the `env:CODE_MODE_SECRET` production path. Only `deny_unknown_fields`-known keys are emitted (T-86-03-03).
- **Crate-name validation before any fs::write (T-86-03-02 / Codex MEDIUM).** `validate_crate_name` rejects empty, leading-digit, non-`[A-Za-z0-9_-]`, and path-separator/`..` names.
- **Golden drift guard.** `emitted_main_matches_example_modulo_setup` includes the Plan 02 example via `include_str!` and asserts the emitted `main.rs` wiring is line-for-line identical modulo the `//!` header, the harness-only `FIXTURES_DIR` const, and the `PMCP_ASSETS_DIR` line. A companion `emitted_main_has_h1_h2_wiring_tokens` asserts the H1/H2/serve tokens are present and the H2 ordering holds.
- **Scoped README (folded todo).** Documents ONLY the two new command surfaces — `new --kind sql-server` (4 emitted files + `cargo run` quick-start + the A6 unpublished-toolkit `[patch.crates-io]` caveat + the DEV-ONLY secret posture) and config-only `deploy` (detection-based asset bundling). The broad rewrite stays Phase 89.

## Task Commits

1. **Task 1: `--kind sql-server` CLI surface + crate-name validation + single-crate emitter + golden tests** — `a7a56220` (feat)
2. **Task 2: scoped README docs for `new --kind sql-server` + config-only deploy** — `7bfd15ee` (docs)

_Note: the emitter module (`sql_server.rs`) and its golden test were committed in Task 1 (not Task 2) because `new::execute` routes into it — a Task-1-only commit would not compile. Task 1 is therefore the complete buildable CLI+routing+emitter slice; Task 2 is the documentation-only addition. The plan's intent (emitter behavior proven by a golden test, scoped README) is fully satisfied across the two commits._

## Files Created/Modified

- `cargo-pmcp/src/templates/sql_server.rs` (created) — the single-crate emitter: `generate` + `generate_{cargo_toml,main_rs,config_toml,schema_sql}`, the `emitted_main_rs()` static blueprint, and the `#[cfg(test)] mod tests` golden + token assertions.
- `cargo-pmcp/src/main.rs` — `New {}` variant gains `#[arg(long)] kind: Option<String>`; dispatch arm threads `kind` into `commands::new::execute(name, path, None, kind, global_flags)`.
- `cargo-pmcp/src/commands/new.rs` — `kind` param on `execute`; the allowlisted `--kind` branch after the dir-exists guard; `validate_crate_name`; `execute_sql_server`; `print_sql_server_next_steps`. The `None`-kind workspace path is untouched.
- `cargo-pmcp/src/templates/mod.rs` — `pub mod sql_server;` (alphabetical, after `server_common`).
- `cargo-pmcp/README.md` — `Config-Driven SQL Server` + `Config-only deploy` sections; `--kind sql-server` noted on the Commands table `new` row.

## Decisions Made

- **Emitter committed in Task 1.** See Task Commits note — the routing target must exist for the CLI to compile, so the buildable slice (CLI + validation + routing + emitter + golden test) is one commit; the README is the second.
- **Wiring-line golden comparison, not byte diff.** The example carries a longer `//!` header and a multi-line `serve()` doc; both files are rustfmt-formatted independently. The test normalizes to load-bearing wiring lines (strip trailing `//` comments, drop blanks/comments/`FIXTURES_DIR`/`PMCP_ASSETS_DIR`) and asserts equality — this is what "cannot drift" means in practice and is robust to comment/doc differences.
- **Token-presence (not strict order) for the H1/H2 token test.** `serve()` precedes `main`, so `StreamableHttpServer::with_config` legitimately appears before `try_code_mode_*`. H2 ordering is enforced separately by an index comparison (`execute_batch` before the `Arc<dyn>` wrap); H1 by asserting `PMCP_ASSETS_DIR` is absent.

## Deviations from Plan

None affecting behavior. The only structural choice is the commit boundary (emitter + golden test in Task 1 rather than Task 2) to keep every commit compilable — documented above. No auto-fixes (Rules 1–3) were needed; the emitted artifacts and CLI behaved as designed on first verification (after one golden-test token-order adjustment that was part of writing the test, not a fix to shipped code).

## Issues Encountered

- **Two binaries in the crate** (`cargo-pmcp` + `mock_test_binary`) mean bare `cargo run -p cargo-pmcp` and `cargo test -p cargo-pmcp --lib` are ambiguous/empty. Used `--bin cargo-pmcp` for run/test. The `templates` module lives in the **bin** target (not the lib surface in `lib.rs`), so the golden tests run under `cargo test -p cargo-pmcp --bin cargo-pmcp emitted_main` (the plan's `--lib` invocation finds 0 tests because the module is bin-only). This is a verification-command nuance, not a defect.
- **`PMCP_QUIET` does not suppress the new next-steps print** — `execute_sql_server` gates its prints behind `global_flags.should_output()` (the CLI quiet flag), matching the existing workspace-path convention; the emitter's own `✓` line is `PMCP_QUIET`-gated. Both are consistent with the surrounding code.

## Pre-existing Failures (NOT introduced by this plan — out of scope)

Per the orchestrator note, these pre-date this plan and are handled separately at phase-end. NOT touched:

1. **`field_reassign_with_default` clippy lint in `crates/pmcp-server-toolkit/src/code_mode.rs:520`** (from Plan 85-10). Surfaces under the full `make quality-gate`; unrelated to cargo-pmcp.
2. **Workspace `cargo fmt --all --check` drift in the Phase 84 connector crates** (`pmcp-toolkit-{athena,mysql,postgres}`) — committed-clean code reflowed by a newer rustfmt.
3. **Two benign pre-existing test-file rustfmt reflows** (`crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs`, `crates/pmcp-sql-server/tests/schema_fixture.rs`) — left untouched and NOT staged, per the orchestrator instruction.
4. **`cargo-pmcp/examples/render_ours.rs`** is a pre-existing untracked file (present in the starting git status) — NOT created or staged by this plan.

All files THIS plan touched are `cargo fmt -p cargo-pmcp --check`-clean and clippy `-D warnings`-clean. The full `make quality-gate` was therefore NOT run to green (it fails on items 1–2, unrelated to this plan); scoped `cargo build`/`clippy`/`test`/`fmt` for cargo-pmcp all pass.

## Verification

| Check | Result |
|-------|--------|
| `cargo build -p cargo-pmcp` | success (14 pre-existing pentest warnings, none in plan files) |
| `cargo run -p cargo-pmcp --bin cargo-pmcp -- new --help \| grep -i kind` | shows `--kind <KIND>` |
| `new --kind sql-server '../evil'` | rejected → `NAME_GUARD_OK` (path-traversal guard, T-86-03-02) |
| `new --kind bogus foo` | exits non-zero: `unknown --kind 'bogus'; supported: sql-server` |
| `new --kind sql-server scaf_test` emits 4 files | `EMIT_OK` — Cargo.toml + src/main.rs + config.toml + schema.sql |
| emitted config.toml has `allow_inline_token_secret_for_dev` | present (D-06) |
| emitted schema.sql idempotent (`IF NOT EXISTS` + `INSERT OR IGNORE`) | present (H2) |
| emitted main.rs uses `assets::load_string` + `demo_db_path` | present (H1) |
| emitted main.rs has NO `PMCP_ASSETS_DIR` | absent (H1 — the one allowed deviation) |
| emitted main.rs `execute_batch`@35 before `Arc<dyn SqlConnector>`@37 | ordering correct (H2) |
| emitted Cargo.toml pins toolkit `["code-mode","sqlite","http"]` | present |
| golden test `emitted_main_matches_example_modulo_setup` | passed |
| token test `emitted_main_has_h1_h2_wiring_tokens` | passed |
| `cargo-pmcp/README.md` mentions `sql-server` + config-only `deploy` | present (scoped) |
| `cargo-pmcp/src/templates/sqlite_explorer.rs` unchanged (D-03) | git diff empty |
| None-kind workspace path unchanged | yes (additive branch only) |
| `cargo fmt -p cargo-pmcp --check` (plan files) | clean |
| `cargo clippy -p cargo-pmcp --bin cargo-pmcp --tests -- -D warnings` (plan files) | clean |

## Next Phase Readiness

- **Plan 04 (scaffold integration test)** can `new --kind sql-server` into a tempdir and `cargo run` the emitted crate end-to-end (the emitted crate is runnable; needs a `[patch.crates-io]` for the unpublished toolkit per A6).
- **Plan 05 (deploy)** inherits the emitted single-crate layout as its detection target (`config.toml` + `schema.sql` + `pmcp-server-toolkit` dep) and the unchanged-main.rs guarantee (H1). The README's config-only-deploy paragraph describes the behavior Plan 05 implements.
- No blockers introduced. The pre-existing workspace fmt/clippy drift (items 1–2) remains for the phase-end fix.

## Self-Check: PASSED

- `cargo-pmcp/src/templates/sql_server.rs` — FOUND
- `cargo-pmcp/src/commands/new.rs` (kind param + sql-server branch) — FOUND
- `cargo-pmcp/src/main.rs` (kind arg threaded) — FOUND
- `cargo-pmcp/README.md` (sql-server section) — FOUND
- commit `a7a56220` — FOUND
- commit `7bfd15ee` — FOUND
