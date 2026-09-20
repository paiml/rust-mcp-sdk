---
phase: 86-shapes-b-c-d-scaffold-library-example-deploy
plan: 04
subsystem: cargo-pmcp
tags: [test, integration, scaffold, sql-server, end-to-end, child-guard, patch-crates-io, code-mode, sqlite, http, shape-b, test-05]

requires:
  - phase: 86-03
    provides: "cargo pmcp new --kind sql-server <name> — the single-crate emitter this test invokes via the REAL binary; the emitted main.rs prints PMCP_SQL_SERVER_ADDR= and binds 127.0.0.1:0; config.toml declares the curated list_books tool"
  - phase: 86-02
    provides: "examples/sql_server_http.rs ChildGuard + spawn-poll + ServerTester pattern reused verbatim; the PMCP_SQL_SERVER_ADDR= stdout contract"
  - phase: 85
    provides: "crates/pmcp-sql-server/tests/parity_chinook.rs — the canonical 20-attempt linear-backoff readiness poll copied verbatim"
provides:
  - "TEST-05: cargo-pmcp/tests/scaffold_sql_server.rs — real-binary scaffold → [patch.crates-io] → cargo run → ChildGuard → address-line poll → tools/list + tools/call end-to-end"
  - "cargo-pmcp/tests/support/scaffold_patch.rs — reusable [patch.crates-io] writer (transitive-dep aware) + ChildGuard + repo_root, shared with Plan 06 (M1)"
affects:
  - "86-06 (deploy integration test): reuses scaffold_patch::{append_crates_io_patch, ChildGuard, repo_root} via #[path = \"support/scaffold_patch.rs\"] — the helper is written once here"

tech-stack:
  added: []
  patterns:
    - "Real-binary scaffold via env!(\"CARGO_BIN_EXE_cargo-pmcp\") subprocess (exercise the actual `new --kind sql-server` command surface, NOT in-process new::execute — M1)"
    - "[patch.crates-io] override in a tempdir Cargo.toml so an unpublished workspace crate (+ its transitive unpublished deps) resolves against in-repo paths (Pitfall §1, Assumption A1)"
    - "ChildGuard(Child) with Drop kill+wait; take() the stdout/stderr pipes BEFORE wrapping so a panic cannot leak a spawned cargo-run server (M1)"
    - "Parse the machine-readable PMCP_SQL_SERVER_ADDR= line from child stdout under a wall-clock budget (never scrape cargo build output — M1)"
    - "parity_chinook.rs 20-attempt linear-backoff readiness poll; panic WITH captured stdout/stderr on timeout (M1)"
    - "Shared test-support module via #[path = \"support/scaffold_patch.rs\"] mod scaffold_patch; (subdir files are NOT auto-discovered as standalone test targets)"

key-files:
  created:
    - "cargo-pmcp/tests/scaffold_sql_server.rs"
    - "cargo-pmcp/tests/support/scaffold_patch.rs"
  modified: []

key-decisions:
  - "Did NOT add [dev-dependencies] to cargo-pmcp/Cargo.toml: mcp-tester (path), tempfile, tokio (full, includes rt-multi-thread+macros), and serde_json are ALREADY in [dependencies], which are visible to integration tests. Adding duplicate [dev-dependencies] entries would be redundant; the clean --no-run compile proves resolution. This is the only deviation from the plan's literal Cargo.toml-wiring instruction (Rule 3 blocking-issue avoidance — there was no blocking issue, so no change needed)."
  - "Compile-checked via --no-run only (orchestrator-sanctioned split): the full test shells out to a cold tempdir `cargo run` that compiles the unpublished toolkit (15+ min, has dropped this agent's streaming connection twice). The ORCHESTRATOR runs the actual `cargo test --test scaffold_sql_server -- --test-threads=1` after this agent returns."
  - "Kept the prior-attempt scaffold_patch.rs verbatim — it already implements the transitive-dep-aware [patch.crates-io] writer (pmcp, pmcp-server-toolkit, pmcp-code-mode, pmcp-widget-utils), ChildGuard with Drop, and repo_root exactly per the plan + Gemini transitive-dep note. No rewrite was needed."
  - "Wall-clock ADDR_READ_TIMEOUT = 600s on the stdout read loop: the cold first build of the scaffolded crate (compiling the unpublished toolkit in a fresh tempdir target/) dominates and must not wedge the line-reader."

requirements-completed: [TEST-05]

duration: ~3min
completed: 2026-05-27
---

# Phase 86 Plan 04: TEST-05 — Scaffold-to-Serve End-to-End Integration Test Summary

**`cargo-pmcp/tests/scaffold_sql_server.rs` proves the Shape B path end-to-end through the REAL command surface: it scaffolds with `env!("CARGO_BIN_EXE_cargo-pmcp") new --kind sql-server`, writes a transitive-dep-aware `[patch.crates-io]` so the unpublished `pmcp-server-toolkit 0.1.0` resolves in a tempdir, spawns a `cargo run` server under a Drop-kill `ChildGuard`, parses the machine-readable `PMCP_SQL_SERVER_ADDR=` line, polls readiness with the verbatim `parity_chinook.rs` 20-attempt backoff, and asserts both `tools/list` (advertises `list_books`) and one `tools/call` (`list_books`) succeed — CI-runnable with zero creds.**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-05-27
- **Completed:** 2026-05-27
- **Tasks:** 2
- **Files modified:** 2 (2 created, 0 modified)

## Accomplishments

- **Reusable test-support module (Task 1, M1).** `cargo-pmcp/tests/support/scaffold_patch.rs` exposes the three pieces both TEST-05 and Plan 06's TEST-06 need:
  - `append_crates_io_patch(&Path)` — appends a `[patch.crates-io]` block pointing `pmcp`, `pmcp-server-toolkit`, AND the transitive unpublished crates `pmcp-code-mode` + `pmcp-widget-utils` at their in-repo paths (Gemini transitive-dep note — without the transitive entries `cargo` would try to fetch `pmcp-server-toolkit 0.1.0` from crates.io where it does not yet exist).
  - `ChildGuard(pub Child)` with `impl Drop { kill + wait }` so a panic anywhere after spawn cannot leak the `cargo run` server subprocess (M1, threat T-86-04-02).
  - `repo_root()` — derives the patch paths from `CARGO_MANIFEST_DIR` (trusted, threat T-86-04-01), the parent of `cargo-pmcp/`.
  - The module carries `#![allow(dead_code)]` so each consumer can use only the subset it needs without per-target dead-code warnings.
- **TEST-05 end-to-end integration test (Task 2, M1).** `cargo-pmcp/tests/scaffold_sql_server.rs` is a `#[tokio::test(flavor = "multi_thread")]` that:
  1. `tempfile::tempdir()` → isolated scratch dir;
  2. scaffolds via the REAL built binary `env!("CARGO_BIN_EXE_cargo-pmcp") new --kind sql-server scaffold_sql_demo` and asserts success + that `Cargo.toml` was emitted (M1 — the actual command surface, NOT `new::execute` in-process);
  3. `append_crates_io_patch(&crate_dir)` to make the unpublished toolkit resolve (Pitfall §1);
  4. spawns `cargo run --quiet` in the crate dir with piped stdout/stderr, takes the pipes, then IMMEDIATELY wraps the child in `ChildGuard` (M1);
  5. reads stdout line-by-line under a 600s wall-clock budget until the `PMCP_SQL_SERVER_ADDR=` line appears, parsing the URL from THAT line (M1 — not from cargo build output);
  6. runs the `parity_chinook.rs` 20-attempt linear-backoff readiness poll verbatim; on timeout panics WITH the captured stdout/stderr (M1);
  7. asserts `test_initialize` succeeded;
  8. asserts `test_tools_list()` passes and the cached tool list advertises `list_books`;
  9. asserts `test_tool("list_books", {"limit": 5})` succeeds;
  10. `ChildGuard` Drop kills+waits the subprocess; the tempdir auto-cleans.
- **`--test-threads=1` documented** in a top-of-file comment (ephemeral port + per-process env + heavy tempdir build, Pitfall §5 / Gemini CI-perf LOW).

## Task Commits

1. **Task 1: reusable scaffold test-support module ([patch.crates-io] writer + ChildGuard + repo_root)** — `db4d7d85` (test)
2. **Task 2: TEST-05 end-to-end scaffold-to-serve integration test** — `3350a7b6` (test)

## Files Created/Modified

- `cargo-pmcp/tests/support/scaffold_patch.rs` (created) — shared `[patch.crates-io]` writer (transitive-dep aware), `ChildGuard` (Drop kill+wait), `repo_root`; `#![allow(dead_code)]` for two consumers.
- `cargo-pmcp/tests/scaffold_sql_server.rs` (created) — the TEST-05 integration test: real-binary scaffold → patch → ChildGuard-protected `cargo run` → address-line poll → readiness backoff → `tools/list` (`list_books`) + `tools/call` (`list_books`).

## Decisions Made

- **No `[dev-dependencies]` additions to `cargo-pmcp/Cargo.toml`.** The plan's Task-2 instruction says "add any missing" dev-deps (`mcp-tester` path, `tempfile`, `tokio` rt-multi-thread+macros, `serde_json`). All four are ALREADY present in `[dependencies]` (`mcp-tester` path dep, `tempfile = "3"`, `tokio` with `features = ["full", "rt-multi-thread"]`, `serde_json = "1"`), and Cargo makes `[dependencies]` visible to integration tests. Adding duplicate `[dev-dependencies]` entries is redundant and would be a no-op (or a version-conflict risk). The clean `--no-run` compile of the test binary proves all four resolve. This is the only deviation from the plan's literal wording and changes no behavior.
- **`--no-run` compile-only, full execution deferred to the orchestrator.** Per the orchestrator-sanctioned `<CRITICAL_BUILD_CONSTRAINT>`, the slow cold-tempdir `cargo run` (compiling the unpublished toolkit, 15+ min, twice dropped this agent's connection) is NOT run inline. The test compiles clean via `cargo test -p cargo-pmcp --test scaffold_sql_server --no-run`; the orchestrator owns the actual `cargo test --test scaffold_sql_server -- --test-threads=1` run.
- **Kept the prior-attempt `scaffold_patch.rs` verbatim.** The interrupted prior attempt left a complete, correct support module (106 lines): transitive-dep-aware patch writer, `ChildGuard` with `Drop`, `repo_root`, plus the threat-model and Gemini-note documentation. Review confirmed it satisfies every Task-1 acceptance criterion, so it was committed as-is rather than rewritten.

## Deviations from Plan

### Plan-instruction simplification (no behavior change)

**1. [Rule 3-adjacent] No `[dev-dependencies]` block added to `cargo-pmcp/Cargo.toml`**
- **Found during:** Task 2 (Cargo.toml wiring step)
- **Issue:** The plan lists `cargo-pmcp/Cargo.toml` in `files_modified` and instructs adding `mcp-tester`/`tempfile`/`tokio`/`serde_json` to `[dev-dependencies]` "if missing".
- **Resolution:** They are NOT missing — all four are in `[dependencies]`, which integration tests inherit. No edit was needed; a duplicate `[dev-dependencies]` block would be redundant. Verified by the clean `--no-run` compile.
- **Files modified:** none (Cargo.toml unchanged)
- **Commit:** n/a (no edit)

No auto-fixes (Rules 1–3) to shipped code were required; the support module from the prior attempt was correct and the test compiled clean on first `--no-run`.

## Issues Encountered

- **`cargo clippy --test scaffold_sql_server -- -D warnings` reports 9 errors — ALL in pre-existing `cargo-pmcp/src/` bin/lib files, NONE in this plan's test files.** Clippy compiles the whole crate (bin + lib) to lint an integration test, so it surfaces pre-existing lint debt: `src/lib.rs:21-24` (`doc_lazy_continuation`), `src/loadtest/summary.rs:58` (`vec_init_then_push`), `src/deployment/config.rs:509` (`collapsible_match`), `src/pentest/attacks/prompt_injection.rs:660,694` (`type_complexity`), `src/pentest/attacks/protocol_abuse.rs:563` (`unnecessary_cast`). `git status --short cargo-pmcp/src/` is clean — this plan modifies ZERO `src/` files. The two files THIS plan adds produce NO clippy errors (`--> …scaffold_*` matched nothing) and are `cargo fmt --check`-clean. Logged to `deferred-items.md` under "Plan 86-04" per the SCOPE BOUNDARY rule; left for the phase-end lint sweep. These are consistent with the 14 pre-existing pentest dead-code warnings already noted in the 86-03 summary.

## Pre-existing Failures (NOT introduced by this plan — out of scope)

Left untouched and NOT staged, per the orchestrator instruction and SCOPE BOUNDARY:

1. **Two benign pre-existing test-file rustfmt reflows** — `crates/pmcp-server-toolkit/tests/synthesizer_structured_content.rs`, `crates/pmcp-sql-server/tests/schema_fixture.rs`.
2. **Pre-existing `.planning/config.json` edit** — present in the starting working tree; not authored by this plan.
3. **Pre-existing clippy `-D warnings` debt in `cargo-pmcp/src/`** — see Issues Encountered + `deferred-items.md`.
4. **Pre-existing untracked files** — `cargo-pmcp/examples/render_ours.rs`, `crates/pmcp-server-toolkit/demo.db` (a runtime artifact from a local scaffold run), fuzz corpus entries, `.claude/*`, `.pmat/*` — none created or staged by this plan.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cargo-pmcp --test scaffold_sql_server --no-run` | **compiles clean** — `Executable tests/scaffold_sql_server.rs` produced; no warnings/errors in the touched files (14 warnings are pre-existing `src/pentest/` dead-code, per 86-03) |
| `grep CARGO_BIN_EXE_cargo-pmcp scaffold_sql_server.rs` | present — scaffold via the REAL binary (M1) |
| test does NOT call `new::execute` or `run_serving` in-process | confirmed — only the subprocess command surfaces are used (M1) |
| `ChildGuard` wraps the spawned `cargo run` child | present (M1, T-86-04-02) |
| address parsed from `PMCP_SQL_SERVER_ADDR=` line | present (M1) |
| `scaffold_patch::append_crates_io_patch` writes `[patch.crates-io]` | present (Pitfall §1) |
| patch covers transitive `pmcp-code-mode` + `pmcp-widget-utils` | present (Gemini note) |
| asserts BOTH `test_tools_list` (`list_books`) AND `test_tool("list_books", …)` | present (TEST-05) |
| readiness poll = parity_chinook.rs 20-attempt linear backoff (no fixed sleep) | present |
| panic WITH captured stdout/stderr on readiness/address timeout | present (M1) |
| top-of-file `--test-threads=1` note | present (Pitfall §5) |
| `cargo fmt -p cargo-pmcp --check` for the two touched files | clean |
| `cargo clippy` errors in the two touched files | none (all 9 are pre-existing `src/`) |

**Full execution status:** Test compiles via `--no-run`; full execution deferred to the orchestrator (slow cold tempdir build of the unpublished toolkit, per the orchestrator-sanctioned `<CRITICAL_BUILD_CONSTRAINT>`). The orchestrator will run `cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1` after this agent returns. Task 2's acceptance is honestly marked: **compiled (clean `--no-run`), not yet executed.**

## Next Phase Readiness

- **Plan 86-06 (deploy config-only integration test)** can pull in the same shared support module via `#[path = "support/scaffold_patch.rs"] mod scaffold_patch;` and reuse `append_crates_io_patch` + `ChildGuard` + `repo_root` — the helper is written once here (M1).
- **Orchestrator action:** run the deferred full execution of TEST-05.
- No blockers introduced. The pre-existing `cargo-pmcp/src/` clippy debt (deferred-items.md) remains for the phase-end lint sweep.

## Self-Check: PASSED

- `cargo-pmcp/tests/scaffold_sql_server.rs` — FOUND
- `cargo-pmcp/tests/support/scaffold_patch.rs` — FOUND
- commit `db4d7d85` (Task 1) — FOUND
- commit `3350a7b6` (Task 2) — FOUND
