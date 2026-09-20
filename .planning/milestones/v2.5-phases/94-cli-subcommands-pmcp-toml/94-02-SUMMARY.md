---
phase: 94-cli-subcommands-pmcp-toml
plan: 02
subsystem: cli
tags: [clap, cargo-pmcp, workbook, lint, exit-codes, pmcp-workbook-compiler]

# Dependency graph
requires:
  - phase: 94-01
    provides: "workbook/config.rs (PmcpToml parser) + pmcp-workbook-compiler path dep + minimal workbook/mod.rs"
  - phase: 93
    provides: "pmcp-workbook-compiler: ingest::ingest, dialect::linter::lint, DialectRules, LintReport, WorkbookCellSource"
provides:
  - "`cargo pmcp workbook` clap subcommand GROUP (compile|lint|emit) wired into main.rs (D-04)"
  - "`workbook lint <wb.xlsx>` standalone dialect linter with text/json output (WBCL-02, D-09)"
  - "Phase-wide exit-code transport: EXIT_OK/EXIT_ERROR/EXIT_GATE_BLOCK + WorkbookExit typed error + main.rs downcast→std::process::exit (D-10)"
  - "Reusable pure helpers format_lint_report(text/json) + lint_exit_code for compile.rs (Plan 94-03)"
  - "Neutral (zero-SATD) compile.rs/emit.rs handler skeletons owned by Plans 94-03/94-04"
affects: [94-03, 94-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "subcommand-group enum + execute(&GlobalFlags) dispatch (mirrors configure/mod.rs)"
    - "typed-error exit-code transport: WorkbookExit downcast in main() → std::process::exit for a DISTINCT gate-block code"
    - "pure String renderer (format_lint_report) + thin print wrapper so JSON is testable without stdout capture"

key-files:
  created:
    - cargo-pmcp/src/commands/workbook/lint.rs
    - cargo-pmcp/src/commands/workbook/compile.rs
    - cargo-pmcp/src/commands/workbook/emit.rs
  modified:
    - cargo-pmcp/src/commands/workbook/mod.rs
    - cargo-pmcp/src/main.rs

key-decisions:
  - "WorkbookExit used ONLY for the gate-block code 2; ordinary lint/compile errors keep plain anyhow::bail! (→ default exit 1 == EXIT_ERROR)"
  - "lint takes an explicit positional workbook_path (no compile-all for lint this phase)"
  - "Workbook left OUT of is_target_consuming() — no Phase-77 env injection"

patterns-established:
  - "Pure format_lint_report(&LintReport, fmt)->String + lint_exit_code(&LintReport)->i32 reused by compile.rs"
  - "Neutral forward-reference bail! handlers (no SATD tokens) keep a wave compilable while logic lands in a later plan"

requirements-completed: [WBCL-02]

# Metrics
duration: ~35min
completed: 2026-06-13
---

# Phase 94 Plan 02: Workbook subcommand group + lint verb Summary

**`cargo pmcp workbook` clap group (compile|lint|emit) with a standalone dialect `lint` verb (text/json, errors-fail/warnings-pass) and a typed `WorkbookExit` exit-code transport that surfaces a gate block as a distinct shell code 2.**

## Performance

- **Duration:** ~35 min
- **Tasks:** 2
- **Files modified:** 5 (3 created, 2 modified)

## Accomplishments
- Extended `workbook/mod.rs` with the `WorkbookCommand{Compile,Lint,Emit}` subcommand group + `execute(&GlobalFlags)` dispatch, mirroring `configure/mod.rs`.
- Established the phase-wide exit-code transport in ONE place: shared `EXIT_OK`/`EXIT_ERROR`/`EXIT_GATE_BLOCK` constants + the `WorkbookExit` typed error (`gate_block` → code 2) + the `main.rs` `downcast_ref::<WorkbookExit>()` → `std::process::exit(code)` arm (D-10). Without this, anyhow would collapse a Plan-03 gate block to exit 1.
- Implemented `workbook lint`: ingest → `dialect::linter::lint` → a PURE `format_lint_report(&LintReport, fmt) -> String` (json round-trips the library serde type, text renders located findings) + a thin `print_lint_report` wrapper + a pure `lint_exit_code` (errors → `EXIT_ERROR`, warnings/info → `EXIT_OK`).
- Wired `main.rs`: `Workbook` variant in `enum Commands` (with an `after_long_help` examples block), the dispatch arm, the exit-code downcast, and deliberately LEFT it out of `is_target_consuming()`.
- Created neutral `compile.rs`/`emit.rs` skeletons (Args struct + forward-reference `anyhow::bail!`) carrying zero SATD tokens — owned/replaced by Plans 94-03/94-04.

## Task Commits

1. **Task 1 (group skeleton + exit transport + main wiring) and Task 2 (lint handler)** — `bbd8bc0f` (feat)
   - Committed together: `mod.rs` references `lint::LintArgs`, so the tree is only buildable with `lint.rs` present. Both tasks ship in one atomic, building commit.

**Plan metadata:** committed separately with this SUMMARY.

## Files Created/Modified
- `cargo-pmcp/src/commands/workbook/mod.rs` — `WorkbookCommand` group enum + dispatch; `EXIT_*` constants; `WorkbookExit` typed error + `Display`/`Error`/`gate_block`; unit tests.
- `cargo-pmcp/src/commands/workbook/lint.rs` — `LintArgs`, `execute`, pure `format_lint_report` (text/json) + `render_text`, thin `print_lint_report`, pure `lint_exit_code`, 8 unit tests.
- `cargo-pmcp/src/commands/workbook/compile.rs` — neutral `CompileArgs` + bail!-ing handler (Plan 94-03).
- `cargo-pmcp/src/commands/workbook/emit.rs` — neutral `EmitArgs` + bail!-ing handler (Plan 94-04).
- `cargo-pmcp/src/main.rs` — `Workbook` enum variant, dispatch arm, `WorkbookExit` downcast → `std::process::exit`; NOT added to `is_target_consuming()`.

## Decisions Made
- `WorkbookExit` is reserved for the gate-block code (2); lint/compile errors stay on plain `anyhow::bail!` → exit 1 (`EXIT_ERROR`). Keeps the common error path simple and the distinct code intentional.
- `lint` takes an explicit positional `workbook_path` (no compile-all variant this phase).
- Quiet/no-color decorative header gated on `gf.should_output() && PMCP_QUIET` unset; the findings render itself always goes to stdout (data channel, D-11).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test/run target is `--bin cargo-pmcp`, not `--lib`**
- **Found during:** Task 2 verification.
- **Issue:** The PLAN's `<verify>`/`<acceptance_criteria>` use `cargo test -p cargo-pmcp --lib workbook::lint` and `cargo run -p cargo-pmcp`. `cargo-pmcp/src/lib.rs` deliberately excludes `commands::*`, so `--lib` reports `0 passed, 429 filtered out` (false pass / no coverage), and bare `cargo run -p cargo-pmcp` errors with "could not determine which binary to run" (two binaries exist).
- **Fix:** Ran tests via `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::` (27 passed) and the binary via `cargo run -q -p cargo-pmcp --bin cargo-pmcp -- workbook ...`. Confirmed the `--lib` path reports the documented false `0 passed`.
- **Files modified:** none (verification-command substitution only).
- **Verification:** `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::` → 27 passed; `workbook --help` lists compile/lint/emit; `workbook lint /tmp/missing.xlsx` exits 1.
- **Committed in:** n/a (no code change; carried forward from the Wave-0 critical-learning note).

---

**Total deviations:** 1 auto-fixed (1 blocking — verification-command target).
**Impact on plan:** No code-scope change. The substitution is the only way to actually exercise the new tests; documented per the Wave-0 gotcha.

## Issues Encountered
- `make quality-gate` returned exit 0 (fmt OK, clippy "No lint issues", build/test/audit green). The log also surfaced pre-existing `cargo-fuzz` ASAN build failures ("failed to run rustc to learn about target-specific information") on this host — these are an unrelated nightly-sanitizer environment issue, NOT caused by this plan's files, and did not fail the gate's exit code. Logged as out-of-scope; not fixed.

## Threat Surface
- T-94-02-EXIT (mitigate): `lint_exit_code` is a pure function unit-tested against the shared `EXIT_*` constants (Error⇒EXIT_ERROR, warnings-only⇒EXIT_OK, empty⇒EXIT_OK); `WorkbookExit` downcast in `main()` guarantees a gate-block code reaches the shell distinct from a compile error. ✅
- T-94-02-FMT (mitigate): unknown `--format` returns an `Err` naming `text`/`json` (tested). ✅
- No new package installs (T-94-02-SC N/A).

## Known Stubs
- `compile.rs` / `emit.rs` are intentional neutral skeletons: their handlers `anyhow::bail!` with a forward-reference message ("delivered by plan 94-03" / "94-04") and carry NO SATD tokens (verified by grep). They are replaced wholesale by Plans 94-03 (compile) and 94-04 (emit), which each list their file in `files_modified`. This is by design per the plan's stub-ownership policy (concern D) and does not block the plan's goal (the `lint` verb is fully functional).

## Next Phase Readiness
- Plan 94-03 (compile) can reuse `format_lint_report` + `lint_exit_code` for the lint phase inside compile, and signal a gate block via `WorkbookExit::gate_block(rendered)` → shell code 2.
- Plan 94-04 (emit) replaces `emit.rs` with the ungated emit path.

## Self-Check: PASSED

- All created files exist on disk (lint.rs, compile.rs, emit.rs, mod.rs, main.rs, SUMMARY).
- Task commit `bbd8bc0f` present in git history.

---
*Phase: 94-cli-subcommands-pmcp-toml*
*Completed: 2026-06-13*
