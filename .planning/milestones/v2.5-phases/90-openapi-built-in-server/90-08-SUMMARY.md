---
phase: 90-openapi-built-in-server
plan: 08
subsystem: cli
tags: [openapi, scaffold, cargo-pmcp, shape-b, shape-c, deploy, code-mode, template]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 06
    provides: "pmcp-openapi-server lib (dispatch -> (HttpConnector, HttpCodeExecutor) pair; build_server assemble; OpenApiSchema) — the scaffold's <=15-line wiring seam"
  - phase: 86-shapes-bcd
    plan: 03
    provides: "sql-server scaffold blueprint (templates/sql_server.rs generate orchestrator, new.rs --kind arm, validate_crate_name, deploy.toml + .pmcp/deploy.toml emission)"
  - phase: 86-shapes-bcd
    plan: 04
    provides: "tests/support/scaffold_patch.rs ([patch.crates-io] writer + ChildGuard) for cold-compile scaffold tests"
provides:
  - "cargo pmcp new --kind openapi-server — single runnable OpenAPI MCP server crate (OAPI-07 / CF-3)"
  - "templates/openapi_server.rs — the scaffold emitter (Cargo.toml/main.rs/config.toml/api.yaml/deploy.toml + .pmcp/deploy.toml)"
  - "tests/scaffold_openapi_server.rs — file-emission + CF-5 golden-drift (always-on) + env-gated cold cargo check"
affects: [90-09-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "OpenAPI scaffold main.rs depends on the pmcp-openapi-server LIB for dispatch+build_server (the http path has NO ServerBuilderExt method, Plan 06 decision) — unlike the SQL scaffold which uses toolkit ServerBuilderExt directly (Rule 3 reconciliation)"
    - "Scaffold deploy parity = a scaffolded deploy.toml + .pmcp/deploy.toml with [target] type = pmcp-run (the DeployConfig schema key; CF-6) — the Phase 77 target enum is UNCHANGED, detection-based"
    - "Scaffold compile tests are two-tier (mirror Phase 86 TEST-05): always-on file-emission + golden-drift line-budget checks (fast, no build) + an env-gated (PMCP_SCAFFOLD_COMPILE_TEST=1) cold cargo check that builds the unpublished toolkit"
    - "CF-5 <=15-line budget asserted by counting STATEMENT lines of main (skip blanks/comments/rustfmt method-chain + closing-delimiter continuations); the StreamableHttpServer boilerplate is hoisted into a private serve() helper outside main (Plan 86-02 Pitfall §2)"

key-files:
  created:
    - cargo-pmcp/src/templates/openapi_server.rs
    - cargo-pmcp/tests/scaffold_openapi_server.rs
  modified:
    - cargo-pmcp/src/templates/mod.rs
    - cargo-pmcp/src/commands/new.rs
    - cargo-pmcp/tests/support/scaffold_patch.rs
    - cargo-pmcp/README.md

key-decisions:
  - "The scaffold's main.rs depends on pmcp-openapi-server (the Plan 06 lib) for dispatch + build_server, NOT on the toolkit alone: the OpenAPI http path has no ServerBuilderExt::*_with_http_connector method (Plan 06 SUMMARY decision), so the (HttpConnector, HttpCodeExecutor) pair seam + assemble step live in that lib — exactly as its own examples/openapi_server_min.rs imports them. This is a Rule 3 blocking reconciliation against the real toolkit surface."
  - "deploy.toml uses the DeployConfig schema's `[target] type = \"pmcp-run\"` key (byte-identical shape to sql_server.rs generate_deploy_toml), which IS the `target_type` the plan's must_have/grep references abstractly — get_target_id reads target.type and has no shape inference (CF-6); the Phase 77 enum is unchanged."
  - "scaffold_patch.rs [patch.crates-io] extended to also cover pmcp-openapi-server (REQUIRED by the openapi scaffold; harmless unused-patch warning in the SQL scaffold path) so the cold cargo check resolves the unpublished lib against its in-repo path."
  - "api.yaml is loaded with read_to_string(...).ok().map(parse).transpose() so a missing spec boots curated-only (D-03) — the scaffold ships a minimal spec for the discovery story, but the runtime never requires it."

requirements-completed: [OAPI-07]

# Metrics
duration: 13min
completed: 2026-05-29
---

# Phase 90 Plan 08: `cargo pmcp new --kind openapi-server` Scaffold Summary

**Added the `cargo pmcp new --kind openapi-server` scaffold (OAPI-07 / CF-3) mirroring `--kind sql-server`: a `templates/openapi_server.rs` emitter produces a SINGLE runnable crate — `Cargo.toml` (toolkit `openapi-code-mode` umbrella + the `pmcp-openapi-server` lib for the `dispatch`/`build_server` seam, since the OpenAPI http path has no `ServerBuilderExt` method), a `src/main.rs` that IS the ≤15-statement-line Shape C wiring (load config[+optional `api.yaml`] → `dispatch` → `build_server` → serve streamable HTTP, with the `StreamableHttpServer` boilerplate hoisted into a private `serve()` helper, CF-5), a `config.toml` with `[backend]` + a single-call AND a script tool + `[code_mode] enabled=true` carrying an inline DEV `token_secret` guarded by `allow_inline_token_secret_for_dev=true` plus a LOUD replace-for-production note (CF-4), a minimal `api.yaml` (D-03 scaffold-discovery, optional at runtime), and a `deploy.toml` + `.pmcp/deploy.toml` declaring `[target] type="pmcp-run"` with the Phase 77 target enum UNCHANGED (CF-6). The `new.rs` `--kind` match gained an `openapi-server` arm + `execute_openapi_server`/`print_openapi_server_next_steps` and a widened error message. A two-tier `tests/scaffold_openapi_server.rs` asserts the emitted files + a CF-5 ≤15-line golden-drift always-on, and proves the scaffold COMPILES via an env-gated cold `cargo check` (verified green). The cargo-pmcp README documents the new `--kind` in a scoped fold.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-05-29T21:32:58Z
- **Completed:** 2026-05-29T21:46Z
- **Tasks:** 2
- **Files modified:** 6 (2 created, 4 modified)

## Accomplishments

- **Task 1 — template emitter:** `templates/openapi_server.rs` mirrors `sql_server.rs`: a `generate` orchestrator calling one `generate_<file>` per output. Cargo.toml pins `pmcp-server-toolkit` on the `openapi-code-mode` umbrella (composes `http` + `code-mode` + `pmcp-code-mode/js-runtime` so the script-tool/Code-Mode engine compiles — RESEARCH Pitfall 4) AND `pmcp-openapi-server = "0.1.0"` for `dispatch`/`build_server`. `main.rs` is the ≤15-line wiring with a hoisted `serve()` helper. `config.toml` carries `[backend]` + a `list_widgets` single-call tool + a `widget_with_detail` script tool (engine-accurate JS subset: `api.get` template-literal paths, `const` bind before `return`) + `[code_mode] enabled=true` with the DEV inline secret + dev flag + replace-for-production note. `api.yaml` is a minimal OpenAPI 3 spec. `deploy.toml` + `.pmcp/deploy.toml` declare `[target] type="pmcp-run"`. Declared `pub mod openapi_server;` in `templates/mod.rs`. 5 unit tests (≤15-line budget, CF-5 wiring tokens, CF-4 config, pmcp-run deploy, openapi-code-mode umbrella) green.
- **Task 2 — command arm + scaffold test:** Added the `Some("openapi-server") => execute_openapi_server(...)` arm to `new.rs` and widened the error to `"supported: sql-server, openapi-server"`. `execute_openapi_server` validates the crate name (path-traversal guard, T-90-08-01), creates `src/`, and calls `templates::openapi_server::generate`; `print_openapi_server_next_steps` gives the run/deploy hints. Extended `scaffold_patch.rs`'s `[patch.crates-io]` writer to also cover `pmcp-openapi-server`. `tests/scaffold_openapi_server.rs` has 3 tests: an always-on file-emission check (all 6 files + CF-3/4/6 markers), an always-on CF-5 ≤15-line golden-drift check, and an env-gated (`PMCP_SCAFFOLD_COMPILE_TEST=1`) cold `cargo check` proving the scaffold compiles. The cold compile was RUN and passed (27s). README gained a scoped "Config-Driven OpenAPI Server" section + a Commands-table fold.
- **Verification:** `cargo test -p cargo-pmcp --test scaffold_openapi_server` → 3 passed; `cargo test -p cargo-pmcp --bin cargo-pmcp openapi_server` → 5 passed; `PMCP_SCAFFOLD_COMPILE_TEST=1` cold `cargo check` → green; `cargo build -p cargo-pmcp` green; `cargo fmt -p cargo-pmcp --check` clean; clippy no errors.

## Task Commits

1. **Task 1: openapi-server scaffold template emitter (CF-3/4/5/6)** — `4caab84b` (feat)
2. **Task 2: --kind openapi-server arm + scaffold compile test (OAPI-07)** — `6b992761` (feat)

## Files Created/Modified

- `cargo-pmcp/src/templates/openapi_server.rs` (created) — the scaffold emitter + 5 unit tests.
- `cargo-pmcp/tests/scaffold_openapi_server.rs` (created) — file-emission + CF-5 golden-drift (always-on) + env-gated cold `cargo check`.
- `cargo-pmcp/src/templates/mod.rs` — `pub mod openapi_server;`.
- `cargo-pmcp/src/commands/new.rs` — `openapi-server` arm + widened error + `execute_openapi_server` + `print_openapi_server_next_steps`.
- `cargo-pmcp/tests/support/scaffold_patch.rs` — `[patch.crates-io]` now covers `pmcp-openapi-server`.
- `cargo-pmcp/README.md` — scoped "Config-Driven OpenAPI Server (`new --kind openapi-server`)" section + Commands-table fold.

## Decisions Made

- **Scaffold depends on `pmcp-openapi-server` (the lib), not the toolkit alone (Rule 3 blocking).** The SQL scaffold wires the server via toolkit `ServerBuilderExt` methods, but Plan 06 established there is NO `ServerBuilderExt::*_with_http_connector` for the OpenAPI http path — the `dispatch` (→ `(HttpConnector, HttpCodeExecutor)` pair) and `build_server` orchestrators live ONLY in `pmcp-openapi-server`. Emitting a toolkit-only ≤15-line `main.rs` was impossible without reimplementing dispatch+assemble inline, so the scaffold takes a lib dependency on `pmcp-openapi-server`, exactly as that crate's own `examples/openapi_server_min.rs` imports `build_server`/`dispatch`. Verified at compile time by the cold `cargo check`.
- **deploy.toml schema key is `[target] type = "pmcp-run"`, not a top-level `target_type`.** The plan's `must_haves`/grep reference `target_type = "pmcp-run"` abstractly; the actual `DeployConfig` schema (and `sql_server.rs::generate_deploy_toml`, copied byte-for-byte in shape) uses `[target] type`. `get_target_id` reads `target.type` (no shape inference) — this IS the `target_type` selection (CF-6); the Phase 77 target enum is unchanged.
- **`scaffold_patch.rs` extended (not duplicated).** TOML disallows two `[patch.crates-io]` tables, so the single shared writer gained a `pmcp-openapi-server` entry. It is unused by the SQL scaffold's dep graph (cargo emits a harmless unused-patch warning) but required by the OpenAPI scaffold.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The scaffold's `main.rs` could not be wired from the toolkit alone — `dispatch`/`build_server` live in the `pmcp-openapi-server` lib**
- **Found during:** Task 1 (after drafting `main.rs` against a presumed `pmcp_server_toolkit::openapi::{dispatch, build_server}` path).
- **Issue:** The plan's `read_first` aspirationally referenced a `ServerBuilderExt` http wiring shape, but Plan 06's SUMMARY recorded there is NO `ServerBuilderExt::*_with_http_connector` method; the OpenAPI dispatch/assemble seam lives only in the `pmcp-openapi-server` binary crate's library (`pmcp_openapi_server::{dispatch, build_server}`). A toolkit-only `pmcp_server_toolkit::openapi` path does not exist (grep returned nothing).
- **Fix:** The emitted `Cargo.toml` takes a `pmcp-openapi-server = "0.1.0"` lib dependency and the `main.rs` imports `pmcp_openapi_server::{build_server, dispatch}` — mirroring that crate's own `examples/openapi_server_min.rs`. Extended `scaffold_patch.rs` so the cold-compile test resolves the unpublished lib against its in-repo path.
- **Files modified:** cargo-pmcp/src/templates/openapi_server.rs, cargo-pmcp/tests/support/scaffold_patch.rs
- **Verification:** env-gated cold `cargo check` of the scaffolded crate passed (the dep + wiring compile).
- **Committed in:** `4caab84b` (Task 1) / `6b992761` (Task 2 — the patch entry).

---

**Total deviations:** 1 auto-fixed (Rule 3 blocking). It is the same class of reconciliation Plan 06 hit (the OpenAPI http path uses free functions / a lib seam, not a `ServerBuilderExt` method) — resolved against the real surface and proven by a cold compile. No scope creep; public surface matches the plan's `artifacts` + `must_haves` + `key_links`.

## Issues Encountered

- The `cargo-pmcp` `templates` module is declared in `main.rs` (the bin), NOT `lib.rs`, so the template unit tests run under `--bin cargo-pmcp` (a `--lib` filter finds 0). Documented for any future template-test runner.
- `cargo fmt` reformatted some `assert!`/`println!` calls in the new files after the Task 1 commit; the reformat was folded into the Task 2 commit (same files), and the final tree is `fmt --check` clean.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (no per-task `tdd="true"`). Tests and implementation were committed together per task because every assertion targets net-new template output / a net-new command arm with no prior passing behavior to protect. All acceptance criteria have passing tests; the OAPI-07 "single runnable crate" promise is proven by an actually-executed cold `cargo check`.

## Known Stubs

None. The scaffold emits a fully runnable crate: the `config.toml` declares a real single-call tool, a real script tool, and code-mode; the `main.rs` wires the real `dispatch`/`build_server` from `pmcp-openapi-server`; the deploy descriptor is a complete parseable `DeployConfig`. The env-gated cold `cargo check` confirms the emitted crate compiles end to end (not a placeholder).

## Threat Flags

None — no new network endpoint / auth path / file-access pattern beyond the plan's `<threat_model>`. T-90-08-01 (path traversal via `--name`) is mitigated by `validate_crate_name` (called before any `fs::write` in `execute_openapi_server`); T-90-08-02 (inline DEV secret to production) is mitigated by the LOUD replace-for-production note + `allow_inline_token_secret_for_dev` flag in the emitted config + the deploy-path secret-rewrite posture; T-90-08-03 (deploy shape inference picking the wrong target) is mitigated by the explicit `[target] type="pmcp-run"` in both deploy.toml copies with the Phase 77 enum unchanged.

## User Setup Required

None for development. To scaffold: `cargo pmcp new my-openapi-server --kind openapi-server`. Until `pmcp-server-toolkit 0.1.0` + `pmcp-openapi-server 0.1.0` are published, add a local `[patch.crates-io]` (or path deps) so `cargo run` resolves the unpublished crates (documented in the README and the scaffold test helper).

## Next Phase Readiness

- **Plan 09 (docs)** can document the `cargo pmcp new --kind openapi-server` scaffold (Shape B/C/D) alongside the `pmcp-openapi-server --config c.toml [--spec s.yaml]` binary (Shape A); the scoped README section + next-steps hints are in place. The broad cargo-pmcp README/help rewrite remains Phase 89 scope.
- No blockers.

## Self-Check: PASSED

- Both created files present on disk: `cargo-pmcp/src/templates/openapi_server.rs`, `cargo-pmcp/tests/scaffold_openapi_server.rs`.
- Both task commits present in git history: `4caab84b`, `6b992761`.
- Acceptance: `openapi-server` arm + widened error in `new.rs` (grep); `cargo test -p cargo-pmcp --test scaffold_openapi_server` → 3 passed; template unit tests → 5 passed; env-gated cold `cargo check` → green; README lists `openapi-server` (8 matches); `cargo build -p cargo-pmcp` green; `cargo fmt --check` clean.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
