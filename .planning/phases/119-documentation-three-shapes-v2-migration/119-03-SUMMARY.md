---
phase: 119-documentation-three-shapes-v2-migration
plan: 03
subsystem: testing
tags: [makefile, bash, cargo, ci-gate, examples, false-green, negative-control]

# Dependency graph
requires:
  - phase: 119-02
    provides: the example-run harness (`tests/common/example_process.rs`, `tests/docs04_examples_run.rs`) whose binaries this gate keeps fresh, and `119-VALIDATION.md`'s mandatory "observe the gate red" row
  - phase: 118.1
    provides: the `deferred-items.md` baseline-record format and the measured `examples/26-server-tester` error classes cited here
provides:
  - "`scripts/run-example-builds.sh` — a counted, exit-1-on-failure example build runner covering all three workspace example trees"
  - "`make test-examples` repaired: a build failure is now a red gate, not a yellow `skipped` line"
  - "First-ever gate coverage of `crates/pmcp-agent/examples/` and `crates/pmcp-team-servers/examples/`"
  - "A measured, committed pre-change baseline in `deferred-items.md`, so the gate's first red is attributable"
affects: [119-04, 119-10, any phase whose examples must keep compiling, whoever next owns the standalone example crates]

actuals:
  tokens: 4600
  tasks: 3
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Makefile target delegates its loop to a `scripts/*.sh` runner (the `test-severance` shape)"
    - "Zero-count guard: a runner that discovers nothing fails rather than reporting success"
    - "`2>&1 | tee` under `set -o pipefail` — keeps diagnostics visible AND propagates the real exit status"

key-files:
  created:
    - scripts/run-example-builds.sh
    - .planning/phases/119-documentation-three-shapes-v2-migration/deferred-items.md
  modified:
    - Makefile

key-decisions:
  - "Baseline committed in its own commit ahead of the gate change, touching neither Makefile nor the script — so a future red cannot be misattributed to Phase 119"
  - "One `--examples` build per tree rather than a per-example loop: faster, and cargo already names each failing target"
  - "The `--features full` fallback of the old loop was dropped entirely — it existed only to hide a failure, and `--all-features` is a superset"
  - "Workspace-excluded example sub-crates stay OUT of the gate, with the measured reason recorded in `deferred-items.md` and referenced from the script"
  - "Example counting uses a dependency-free `find` over each tree's `examples/` dir rather than `cargo metadata` + `jq`, so the gate adds no tool prerequisite"
  - "DOCS-06 deliberately NOT booked as complete — see Requirements Ledger below"

patterns-established:
  - "Negative control per tree: a gate that widens its reach must be observed red in EACH newly-covered tree, not just once"
  - "Baseline-before-tighten: measure and commit the pre-existing surface before making a shared gate strict (the 118.1-03 precedent, now applied twice)"

requirements-completed: []

coverage:
  - id: D1
    description: "The pre-existing example-build surface is measured at this plan's HEAD and committed BEFORE the gate is tightened, with error classes and owners named"
    requirement: "DOCS-06"
    verification:
      - kind: integration
        ref: "cargo build --all-features --examples; cargo build -p pmcp-agent --all-features --examples; cargo build -p pmcp-team-servers --all-features --examples (all exit 0, 0 errors, 0 warnings)"
        status: pass
      - kind: other
        ref: "git log --oneline -1 -- .planning/phases/119-*/deferred-items.md → 5b90fdd2, touching neither Makefile nor scripts/"
        status: pass
    human_judgment: false
  - id: D2
    description: "`make test-examples` fails with a non-zero exit when an example does not compile, preserves the compiler diagnostic, covers all three example trees, and stays chained into quality-gate"
    requirement: "DOCS-06"
    verification:
      - kind: integration
        ref: "make test-examples → exit 0, '87 examples built across 3 trees, 0 failures.'"
        status: pass
      - kind: other
        ref: "make -n test-all → './scripts/run-example-builds.sh' appears between test-property and test-integration"
        status: pass
      - kind: other
        ref: "grep -v '^[[:space:]]*#' scripts/run-example-builds.sh | grep -c '2>/dev/null' → 0; same filter for '|| true' → 0; grep -cF 'requires specific features' Makefile → 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "The repaired gate has been OBSERVED failing on a deliberately broken example in the root tree AND in a crates/*/examples/ tree, then passing after revert"
    requirement: "DOCS-06"
    verification:
      - kind: other
        ref: "negative control ×2 — broken examples/s49_sampling_host.rs and crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs each produced a non-zero make exit with E0425 visible; both reverted byte-identical (git diff --quiet exit 0)"
        status: pass
      - kind: integration
        ref: "cargo test --test docs04_examples_run → 1 passed"
        status: pass
    human_judgment: false

duration: 31min
completed: 2026-08-19
status: complete
---

# Phase 119 Plan 03: Repair the example-build gate Summary

**`make test-examples` now fails the build when an example stops compiling — proven by breaking one example in each newly-covered tree and watching the gate go red — against a committed baseline of 87 example targets building clean.**

## Performance

- **Duration:** 31 min
- **Started:** 2026-08-19T05:16:56Z
- **Completed:** 2026-08-19T05:48:23Z
- **Tasks:** 3
- **Files modified:** 3 (2 created, 1 modified)

## Accomplishments

- **The false green is gone.** `Makefile:253-266` used to run each example through two
  `2>/dev/null` build attempts and, when both failed, print
  `⚠ Example X requires specific features (skipped)` and continue — exiting 0. A build failure and
  a feature gap were indistinguishable, and `make quality-gate` (which chains this target through
  `test-all`) could not see either. The recipe now delegates to a runner that exits 1 on any
  failure.
- **Diagnostics survive.** `2>&1 | tee` under `set -o pipefail` keeps the compiler's error text on
  the terminal while still propagating cargo's exit status; on failure the script additionally
  reprints every `error[...]`/`error:` line and names the tree.
- **Two example trees are gated for the first time.** The old loop iterated `ls examples/*.rs`, so
  `crates/pmcp-agent/examples/` and `crates/pmcp-team-servers/examples/` — two of D-15's six
  documented examples — were outside its reach entirely, independent of the swallow bug. Coverage
  went from 85 example targets to **87 across 3 trees**.
- **A zero-discovery run is now a failure.** A missing tree or an emptied `examples/` directory
  fails with an explicit message instead of building nothing and reporting success — the same
  guard, and the same reasoning, as `scripts/run-severance-proofs.sh`.
- **The gate landed against a MEASURED baseline, committed first, in its own commit.**

## Task Commits

1. **Task 1: Measure and record the example-build baseline (D-14)** — `5b90fdd2` (docs)
2. **Task 2: Counted, exit-1 example build runner (D-13)** — `9aefc939` (fix)
3. **Task 3: Negative control — break, observe RED, revert** — no commit by design; the task's
   deliverable is an OBSERVATION, and both touched files were restored byte-identical
   (`git diff --quiet` exit 0). Its record is this SUMMARY.

**Plan metadata:** this commit (docs: complete plan)

## Files Created/Modified

- `scripts/run-example-builds.sh` (new, executable) — builds all examples in the root `pmcp`,
  `pmcp-agent` and `pmcp-team-servers` trees; counts what each tree discovers and fails on zero;
  exits 1 on any build failure; header records why it exists, why the count is load-bearing, and
  which trees are deliberately excluded
- `Makefile` — `test-examples` body replaced with the three-line delegation, plus a `lint-plans`-style
  comment block recording that the target IS chained into `quality-gate` through `test-all` and must
  stay chained (`test-integration`'s `spawn_example` legs depend on it for non-stale binaries), that
  the previous non-blocking behaviour was changed deliberately, and where the measured baseline lives.
  The stale "Examples are built but not run to avoid blocking on I/O" note was corrected — the run
  tests added in this phase do run several of them.
- `.planning/phases/119-.../deferred-items.md` (new) — the D-14 baseline record

## Measured results

### Baseline (Task 1, at `aa0e6c9a279dde94435567b3ae9c8663de5c71d3`)

| Tree | Example targets | Exit | `^error` | `^warning` |
|---|---|---|---|---|
| root (`pmcp`) | 85 | 0 | 0 | 0 |
| `crates/pmcp-agent/examples/` | 1 | 0 | 0 | 0 |
| `crates/pmcp-team-servers/examples/` | 1 | 0 | 0 | 0 |

Re-measured here rather than copied from research; confirms § F-5 exactly. Target counts come
from `cargo metadata --no-deps` (kind `example`), not from `ls`.

The excluded sub-crates were measured too, and the record classifies each one — including the
finding that **a nested worktree cannot faithfully measure a workspace-EXCLUDED sub-crate at all**
(see Issues Encountered).

### Negative controls (Task 3) — the gate observed RED, twice

| Control | Command | Observed exit | Observed diagnostic |
|---|---|---|---|
| ROOT tree: undeclared identifier appended to `examples/s49_sampling_host.rs` | `make test-examples` | **2** (recipe: `make: *** [test-examples] Error 1`) | `error[E0425]: cannot find value \`__gsd_undeclared_identifier_119_03\` in this scope --> examples/s49_sampling_host.rs:228:13`, then `error: could not compile \`pmcp\` (example "s49_sampling_host")` |
| after `git checkout --` revert | `make test-examples` | **0** | `87 examples built across 3 trees, 0 failures.` |
| `crates/pmcp-agent/examples/` tree: same edit to `s50_standalone_vs_sampled.rs` | `make test-examples` | **2** | script header `FAILURE: tree 'pmcp-agent' failed to build.`, then `error[E0425] ... --> crates/pmcp-agent/examples/s50_standalone_vs_sampled.rs:383:13` and `error: could not compile \`pmcp-agent\` (example "s50_standalone_vs_sampled")` |
| after `git checkout --` revert | `make test-examples` | **0** | `87 examples built across 3 trees, 0 failures.` |

Both observations matter for different claims: the first proves failures now propagate with their
diagnostics intact; the second proves the widened tree coverage is REAL — the pre-repair loop
could not have compiled that file at all, let alone failed on it.

Post-control state: `git status --porcelain` prints nothing for either path,
`git diff --quiet` over both exits 0, and `cargo test --test docs04_examples_run` is `1 passed`.

## Decisions Made

- **Baseline in its own commit, first.** `5b90fdd2` touches only `deferred-items.md`. The T-119-13
  repudiation threat (pre-existing breakage silently reassigned to this phase) is closed by
  ordering, not by assertion.
- **Dropped the `--features full` fallback.** The old loop tried `--all-features`, then
  `--features full`, then declared success-by-skipping. Cargo features are additive, so
  `--all-features` is a superset of `full`; the second attempt could only ever mask something.
- **Counted with `find`, not `cargo metadata | jq`.** `make test-examples` runs inside
  `quality-gate`; adding a `jq` prerequisite would make the gate fail on a machine that merely
  lacks a tool. The script's header states precisely what the count is (source files in each
  tree's `examples/` dir) and what it is not (a `[[example]]` target count), since the guard's job
  is zero-versus-non-zero.
- **Excluded sub-crates stay excluded.** Recorded, referenced from the script, and owned.

## Deviations from Plan

None — the plan executed as written. Three notes that are recordings, not deviations:

1. **Task 3 produced no commit.** Its deliverable is an observation and a byte-identical tree;
   committing would have violated its own acceptance criteria (T-119-15).
2. **`make quality-gate` needed a second run.** The first run died on disk exhaustion, not on
   anything this plan changed — see Issues Encountered. The second run exits 0.
3. **Measurement B could not be taken faithfully for six sub-crates from this worktree.** Rather
   than inherit research's numbers as if re-measured, the record marks each affected row
   UNMEASURABLE / UNCLASSIFIED with the mechanism stated and the exact main-checkout commands to
   re-run. The plan's `read_first` explicitly warned that a baseline cited rather than taken
   proves nothing; that rule applies to the rows this environment cannot measure just as much as
   to the ones it can.

## Issues Encountered

- **`make quality-gate` run 1 failed with `No space left on device` (errno 28), 11 times, all at
  the link step.** `error: linking with 'cc' failed` × 8 plus
  `ld: write() failed, errno=28`, `failed to write query cache ...: No space left on device`, and
  `error: failed to write output stream`. Not a code defect and not caused by this plan: three
  parallel wave-3 worktree agents each carry an independent multi-gigabyte `target/` (this one
  measured **21 GB**, with `target/debug/incremental` alone at **9.1 GB**) against ~13 GB free.
  This is the documented "disk exhaustion fakes code regressions" failure mode.
  **Resolved** by deleting this worktree's `target/debug/incremental` (a gitignored build cache;
  freed ~6 GB) and re-running with `CARGO_INCREMENTAL=0`. **Run 2 exits 0.**
  Worth flagging to the orchestrator: parallel-wave execution on this machine is close to the disk
  ceiling, and the symptom presents as unrelated linker errors.
- **`make` output captured through the local `rtk` proxy is re-ordered and filtered.** In the
  quality-gate log, `lint-plans` output (gate stage 1) appears at line 1042, after clippy output,
  and the example-gate summary line does not appear at all — while the same line IS present when
  `make test-examples` is redirected on its own. Log CONTENT from a long proxied run is therefore
  not a reliable witness; the exit status is (run 1 propagated its failure correctly). The
  test-examples chaining was consequently confirmed structurally with `make -n test-all`, which
  prints `./scripts/run-example-builds.sh` between `test-property` and `test-integration`.

## Requirements Ledger

**`requirements-completed: []` — DOCS-06 is deliberately NOT booked.**

The plan frontmatter lists `requirements: [DOCS-06]`, and DOCS-06 reads: *"Runnable v2 examples: a
stateless (Lambda-style) v2 server and a v2 client/agent example."* This plan wrote no example and
ran none; it repaired the gate that keeps examples compiling. The requirement is satisfied by plan
**119-04**, which owns `tests/docs06_v2_examples_run.rs` and the `s47`/`s48`/`s53` legs. Booking it
here would mark DOCS-06 complete before its actual deliverable exists — exactly the ledger defect
plan 119-01 hit and reverted. No `requirements.mark-complete` call was made, and
`.planning/REQUIREMENTS.md` is unmodified by this plan.

## Verification

| Plan verification step | Result |
|---|---|
| 1. `test -f .planning/phases/119-*/deferred-items.md` | PASS |
| 2. `make test-examples` | PASS — exit 0, `87 examples built across 3 trees, 0 failures.` |
| 3. `cargo test --test docs04_examples_run` | PASS — `1 passed` |
| 4. `git status --porcelain` over the example trees | PASS — empty |
| 5. `make quality-gate` | PASS — exit 0 on run 2 (`CARGO_INCREMENTAL=0`); run 1's red was ENOSPC, not a target |
| `cargo fmt --all -- --check` | PASS — exit 0 |
| `make lint-plans` | PASS — 54 plan files, 2129 verification lines, no masked status |

## Self-Check: PASSED

- `scripts/run-example-builds.sh` — FOUND, executable
- `.planning/phases/119-.../deferred-items.md` — FOUND
- `Makefile` — modified, `requires specific features` count 0, `run-example-builds.sh` count 1
- Commit `5b90fdd2` — FOUND (touches only `deferred-items.md`)
- Commit `9aefc939` — FOUND (`Makefile` + `scripts/run-example-builds.sh`)

## Notes for the orchestrator / next plans

- **`119-VALIDATION.md`'s 119-03 · T1 row points at `119-03-BASELINE.md`**, but the plan's
  `files_modified`, artifacts block and every acceptance criterion say `deferred-items.md`, which
  is what was written. That row will not match on disk. `119-VALIDATION.md` is outside this plan's
  file scope and is being written by sibling plans in the same wave, so it was not edited here;
  the discrepancy is also logged in `deferred-items.md`. Owner: plan 119-10 or the merge.
- **The three wave-3 plans share a disk.** If a sibling reports linker errors, check `df -h /`
  before reading them as code defects.
- **This worktree's `target/debug/incremental` was deleted** to free space. It is a gitignored
  build cache; nothing tracked was touched.

## User Setup Required

None — no external service configuration required. No packages were installed; the script uses
`cargo` and POSIX shell only.

## Next Phase Readiness

- The example gate is strict, counted, three-tree, and observed red. Plan 119-04's new run legs
  (`tests/docs06_v2_examples_run.rs`) inherit a `test-examples` that will actually fail if one of
  their examples stops compiling, and `test-all` still builds those binaries immediately before
  `test-integration` spawns them.
- Residual, owned, and out of scope: `examples/26-server-tester` (8 pre-existing errors),
  `examples/wasm-mcp-server` (4 pre-existing wasm-target errors), `examples/wasm` (manifest
  artifact), `examples/wasm-client` (unclassified), and 32 example targets in eleven other
  workspace members that no gate reaches. All recorded in `deferred-items.md` with owners.

---
*Phase: 119-documentation-three-shapes-v2-migration*
*Completed: 2026-08-19*
