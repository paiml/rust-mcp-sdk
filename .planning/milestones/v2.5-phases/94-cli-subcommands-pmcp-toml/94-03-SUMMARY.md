---
phase: 94-cli-subcommands-pmcp-toml
plan: 03
subsystem: api
tags: [cargo-pmcp, cli, clap, workbook-compiler, governance-gate, pmcp-toml, excel-bundle]

# Dependency graph
requires:
  - phase: 94-00
    provides: "pmcp-workbook-compiler public seams — read_workbook_version, prepare_candidate, Candidate (1:1 with PromoteInputs)"
  - phase: 94-01
    provides: "workbook/config.rs — PmcpToml::load/resolve/all_entries + WorkbookEntry + project-root containment"
  - phase: 94-02
    provides: "workbook/mod.rs WorkbookExit + EXIT_* constants + main.rs downcast arm; workbook/lint.rs print_lint_report/lint_exit_code"
provides:
  - "CompileArgs + the `cargo pmcp workbook compile` handler (the BA's primary verb, WBCL-01)"
  - "Seed lane (first version, no gate, D-12) via compile_workbook"
  - "Gated-update lane (prior baseline) via prepare_candidate -> derive_corpus -> gate -> block-or-promote, gate-before-write"
  - "--accept --approver --effective-date approval flow (D-07)"
  - "bundle-id resolution + bare compile-all through pmcp.toml, continue-on-error + worst-status-wins (WBCL-04/D-05)"
affects: [94-04-emit, 94-05-main-wiring, v23-workbook-cli]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Thin-shell CLI: the handler CALLS library verbs (compile_workbook/prepare_candidate/derive_corpus/gate/accept/promote) and reimplements zero gate/corpus/promotion logic"
    - "Prior-baseline gate inputs (IR/DAG/prev_bundle_hash/manifest/version) read from the on-disk bundle members via plain serde_json + build_dag — no served bundle loader needed at build time"
    - "Worst-status reduction (EXIT_GATE_BLOCK > EXIT_ERROR > EXIT_OK) surfaced via the typed WorkbookExit transport for the distinct gate-block exit code"

key-files:
  created: []
  modified:
    - "cargo-pmcp/src/commands/workbook/compile.rs — replaced the Plan-02 neutral stub with the full handler (+809 lines)"

key-decisions:
  - "Lane selection probes the out-root for the highest `{workflow}@{version}/` dir; presence => gated lane, absence => seed lane (D-12)"
  - "Prior IR/DAG/prev_bundle_hash recovered by reading the prior bundle's executable.ir.json + manifest.json + BUNDLE.lock as plain JSON (the compiler does not re-export a bundle loader, and cargo-pmcp deps only the compiler — so no Cargo.toml change was needed)"
  - "Change classes derived via change_class::classify(prior, candidate) stripped to Vec<ChangeClass> for the gate; region detail stays in the block render"
  - "--accept enforced to require --effective-date in the handler (clap pairing); --approver clap-required so accept can never self-approve without an explicit approver (D-06)"
  - "Default out-root for a bare-path compile with no --out is the workbook's parent dir"

patterns-established:
  - "Per-target compile_one returning an exit code; the execute loop reduces worst-status-wins and is continue-on-error (one workbook's failure never aborts the rest)"
  - "Gate block path returns Err(WorkbookExit::gate_block(render)) AFTER printing render() verbatim and BEFORE any write (gate-before-write, T-94-03-WRITE)"

requirements-completed: [WBCL-01, WBCL-04]

# Metrics
duration: 38min
completed: 2026-06-13
---

# Phase 94 Plan 03: Workbook Compile Handler Summary

**`cargo pmcp workbook compile` orchestrating the Phase-93 compiler through a seed lane (first version, no gate) and a gated-update lane (prepare_candidate -> derive_corpus -> gate -> block-or-promote, gate-before-write), with mandatory --approver, the --accept approval flow, and pmcp.toml bundle-id / compile-all resolution — all as a thin shell over library verbs.**

## Performance

- **Duration:** ~38 min
- **Started:** 2026-06-13 (Wave 3 of phase 94)
- **Completed:** 2026-06-13
- **Tasks:** 1 (TDD)
- **Files modified:** 1

## Accomplishments
- Replaced the Plan-02 neutral `compile.rs` stub with the complete `CompileArgs` + `execute` handler (~809 net new lines, 791-line file).
- **Seed lane (D-12):** a first version with no prior accepted baseline runs `compile_workbook` (version read from the workbook via `read_workbook_version`, never a flag/toml) and writes the seven-member bundle.
- **Gated-update lane:** a re-compile with a prior baseline builds the candidate WITHOUT writing via `prepare_candidate`, derives the prior-baseline corpus golden via `derive_corpus`, grades each case via `gate`; on a block it prints `GateBlock::render()` verbatim and returns `WorkbookExit::gate_block(...)` (distinct exit 2) writing NOTHING (gate-before-write); on a pass it `promote`s the new version.
- **--accept flow (D-07):** records a fingerprint-bound `ApprovalRecord` via `accept` then promotes; `--accept` requires `--effective-date`; `--approver` is clap-mandatory (D-06).
- **pmcp.toml resolution (WBCL-04/D-05):** a bundle-id resolves path/out_dir/workflow; a bare `compile` compiles ALL declared workbooks, continue-on-error, worst-status-wins.
- 15 unit tests cover exit-code reduction, accept/effective-date pairing, gate-block render substrings + the WorkbookExit code, target resolution (bare-path-requires-workflow, compile-all visits every entry, unknown bundle-id), and prior-baseline probing.

## Task Commits

Each task was committed atomically:

1. **Task 1: CompileArgs + seed lane + gated-update lane (gate-before-write)** — `37cd3e71` (feat)

_Note: this TDD task's tests and implementation were authored and committed together (the file fully replaces a stub, so the new tests cannot exist against the old stub — RED would not compile independently); RED was observed as the pre-existing stub `bail!` plus the new test module, GREEN as the passing 15-test run._

## Files Created/Modified
- `cargo-pmcp/src/commands/workbook/compile.rs` — `CompileArgs` clap struct; `execute` (accept/effective-date guard, target resolution, continue-on-error loop, worst-status reduction); `resolve_targets` (bare-path / bundle-id / compile-all); `compile_one` (lint phase reuse, version read, lane selection); `run_seed_lane`; `find_prior_baseline` + `read_prior_bundle` (on-disk gate inputs); `run_gated_lane` (corpus derive, per-case gate, block-or-promote); `accept_and_promote`; `promote_candidate`; `emit_block` (text verbatim / json); 15 unit tests.

## Decisions Made
- **Lane selection by out-root probe:** the highest `{workflow}@{version}/` dir under the out-root determines gated vs seed. Sufficient for the single-prior-baseline transition the gate grades against; lexical max picks the prior version deterministically.
- **Prior gate inputs from on-disk bundle members:** the compiler does not re-export `pmcp_workbook_runtime::bundle_loader::load`, and cargo-pmcp depends only on the compiler crate. Rather than add a runtime dep or new re-exports (out of this plan's scope — sibling plans / Cargo.toml ownership), the prior IR/DAG/prev_bundle_hash/manifest are recovered by reading `executable.ir.json` + `manifest.json` + `BUNDLE.lock` as plain JSON (`Cell`/`Manifest`/`BundleLock` are all compiler re-exports) and rebuilding the DAG via `build_dag`. This keeps the thin-shell invariant (no gate/corpus/promotion logic) and touches only `compile.rs`.
- **Change classes via `classify`:** prior-vs-candidate manifest + IR classification is delegated to `change_class::classify`, stripped to `Vec<ChangeClass>` for the gate; the per-region detail is surfaced by the library's block render.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Test target uses `--bin cargo-pmcp`, not `--lib`**
- **Found during:** Task 1 (verification)
- **Issue:** The PLAN's `<verify>`/`<acceptance>` commands use `cargo test -p cargo-pmcp --lib workbook::compile`, but `cargo-pmcp`'s `commands::*` is BIN-only (lib.rs excludes it), so `--lib` reports a false `0 passed` (verified in Waves 0+1, carried in the prompt's critical-learning note).
- **Fix:** Ran the tests via `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::compile` (15 passed). No source change — only the verify command substitution.
- **Files modified:** none (verification command only)
- **Verification:** `cargo test -p cargo-pmcp --bin cargo-pmcp workbook::compile` → 15 passed; `... workbook` → 42 passed.
- **Committed in:** n/a (no code change)

---

**Total deviations:** 1 (Rule 3 - blocking, verify-command substitution only)
**Impact on plan:** No scope creep, no source deviation. The substitution is the documented project gotcha for bin-only command modules.

## Issues Encountered
- **`cargo fmt` reflow:** the initial write needed `cargo fmt` reformatting (multi-line `eprintln!`/builder chains). Applied `cargo fmt -p cargo-pmcp`; the gate's `cargo fmt --all -- --check` then passed clean for this file.
- **`make quality-gate` fuzz step environment failure:** the gate's fuzz-build step fails with `failed to run rustc to learn about target-specific information` for the nightly ASAN/sancov `aarch64-apple-darwin` fuzz targets — a pre-existing sandbox/toolchain limitation unrelated to this change. The relevant gates passed: `cargo fmt --all -- --check` (clean), `cargo clippy` with the exact pedantic+nursery gate allow-list ("✓ No lint issues" and zero violations on compile.rs under the gate flags), `cargo check --examples`, and the workbook test suite.

## Known Stubs
None — the handler is fully wired (both lanes ship this phase; no placeholder data paths).

## Threat Flags
None — no new network endpoints, auth paths, or schema changes beyond the plan's `<threat_model>` register. The `--out` write target resolves through the same Plan-01 `PmcpToml` containment; the gate-before-write invariant (T-94-03-WRITE) is upheld (block returns before any `promote`/emit).

## Self-Check: PASSED
- `cargo-pmcp/src/commands/workbook/compile.rs` — FOUND (791 lines, contains `pub struct CompileArgs`).
- Commit `37cd3e71` — FOUND in `git log`.
- Grep acceptance: library-verb count = 7 (≥3); reimplemented gate/promote/candidate logic = 0; version-from-toml/`--version` = 0; `.render()` present; ≥180 lines (791).

## Next Phase Readiness
- The compile verb is complete and wired through `WorkbookCommand::Compile`. Plan 94-04 (emit, the ungated sibling) and 94-05 (main wiring) can proceed.
- The `emit.rs` sibling (owned by another plan) was NOT touched.

---
*Phase: 94-cli-subcommands-pmcp-toml*
*Completed: 2026-06-13*
