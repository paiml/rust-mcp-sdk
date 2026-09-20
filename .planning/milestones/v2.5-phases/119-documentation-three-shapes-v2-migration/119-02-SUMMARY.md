---
phase: 119-documentation-three-shapes-v2-migration
plan: 02
subsystem: testing
tags: [mdbook, documentation, pmcp-agent, test-harness, subprocess, timeout, negative-control]

requires:
  - phase: 119-01
    provides: "The settled requirements ledger (HTTP-01..08, CLNT-01/02/05 `[x]`) and 113-SPEC-RECHECK PUBLISHED-CONFIRMED, so this chapter's prose could state v2 behaviours without hedging"
  - phase: 108
    provides: "`pmcp-agent`'s AgentEngine/AgentServer and the `s50_standalone_vs_sampled` example this plan documents and runs"
  - phase: 110
    provides: "The `cargo pmcp agent new|dev` and `team dev` verbs the chapter and README lead with"
provides:
  - "`run_example_to_completion` — a BOUNDED run-to-completion subprocess helper (mandatory timeout, concurrent drains, kill+reap+report on expiry) that plans 119-04 and 119-10 expand"
  - "`tests/docs04_examples_run.rs` — the run-to-completion test binary, green on its first leg"
  - "`pmcp-book/src/ch12-15-agents-as-mcp-clients.md` — the Part III Agents-as-clients chapter"
  - "A SUMMARY.md nav mutation in both directions: Chapter 12.15 inserted, `ch17-04` re-parented out of the Part V examples stub"
  - "README `## Agents & Teams` section, cargo-pmcp-first"
  - "MEASURED: the book/course `create-missing` asymmetry, with its real mechanism (the course WRITES the file)"
  - "MEASURED: the mdbook-exercises theme side effect fires on EVERY course build (+411/-88)"
affects: [119-04, 119-06, 119-07, 119-09, 119-10]

actuals:
  tokens: 8800   # chars/4 over the realized diff (35,205 chars across 6 files)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Bounded subprocess execution: deadline-polled `try_wait` + kill + reap + panic carrying partial output"
    - "Drains publish into shared buffers rather than being joined, so an orphan holding the pipe cannot re-hang the deadline"
    - "Negative controls: observe a gate RED before relying on it"

key-files:
  created:
    - "tests/docs04_examples_run.rs"
    - "pmcp-book/src/ch12-15-agents-as-mcp-clients.md"
  modified:
    - "tests/common/example_process.rs"
    - "pmcp-book/src/SUMMARY.md"
    - "README.md"
    - ".planning/phases/119-documentation-three-shapes-v2-migration/119-VALIDATION.md"

key-decisions:
  - "The run-to-completion drains publish into shared buffers and are never joined — a measured hang proved that kill+reap+join still deadlocks when the child leaves a grandchild holding the pipe"
  - "DOCS-04 was NOT booked complete: this plan delivers one of its several chapters/examples; plans 119-04, 119-06, 119-07 and 119-09 own the rest"
  - "`assert_binary_is_not_stale`'s non-root blind spot stays DOCUMENTED, not fixed (recorded review disposition), with an explicit `-p <crate>` build as the compensating control"
  - "Tracer gate run in its autonomous form (re-verify end-to-end) because `mode: yolo` + `autonomous: true`, despite `auto_advance` being unset"

patterns-established:
  - "Bounded subprocess helper: every run-to-completion leg carries its own budget constant in its own test file, never a shared one"
  - "Course-book plans must assert chapter presence with `test -f` and check `git status --untracked-files=all`, because `create-missing = true` makes the build blind AND dirty"

requirements-completed: []

coverage:
  - id: D1
    description: "`run_example_to_completion` runs a built example under a mandatory deadline and returns its status plus both captured streams"
    requirement: DOCS-04
    verification:
      - kind: integration
        ref: "tests/docs04_examples_run.rs#s50_standalone_vs_sampled_runs_to_completion"
        status: pass
    human_judgment: false
  - id: D2
    description: "The helper's expiry path kills, reaps and panics with partial output instead of hanging"
    requirement: DOCS-04
    verification:
      - kind: other
        ref: "throwaway control: run_example_to_completion(sh -c 'echo …; sleep 300', 2s) → red at 2.55s carrying partial stdout"
        status: pass
    human_judgment: false
  - id: D3
    description: "Chapter 12.15 exists, is reachable from Part III, and `ch17-04` is re-parented under it"
    requirement: DOCS-04
    verification:
      - kind: integration
        ref: "mdbook build (pmcp-book) → exit 0; grep: 12.15 entry ×1, ch17-04 entry ×1 at line 44 < Part IV at line 46"
        status: pass
    human_judgment: false
  - id: D4
    description: "README `## Agents & Teams` section leading with `cargo pmcp agent new`, naming only real verbs"
    requirement: DOCS-04
    verification:
      - kind: other
        ref: "grep -ohE 'cargo pmcp [a-z-]+ [a-z-]+' over the new section → exactly {agent new, agent dev, team dev}"
        status: pass
    human_judgment: false
  - id: D5
    description: "The `create-missing = false` gate is proven to fail on a missing SUMMARY target, and the course book's opposite behaviour is measured"
    verification:
      - kind: other
        ref: "book control: mdbook build → exit 101 'Chapter file not found'; course control: exit 0 + file written"
        status: pass
    human_judgment: false
  - id: D6
    description: "The new chapter's prose is accurate, cargo-pmcp-first, and does not invent flags"
    requirement: DOCS-04
    verification: []
    human_judgment: true
    rationale: "Ordering, emphasis and technical accuracy of prose are judgements, not string matches — 119-VALIDATION's Manual-Only table already classifies this row as manual"

duration: 29min
completed: 2026-08-18
status: complete
---

# Phase 119 Plan 02: Agents as MCP Clients (the tracer) Summary

**One agent feature wired end-to-end through every Phase-119 layer — a Part III chapter, a README section, a SUMMARY nav mutation in both directions, and a bounded run-to-completion test harness that actually executes the example the chapter cites — with both the mdbook gate and the harness's own timeout observed RED before being trusted.**

## Performance

- **Duration:** ~29 min
- **Started:** 2026-08-18T21:40:39Z (fork point)
- **Completed:** 2026-08-18T22:09:23Z
- **Tasks:** 3
- **Files modified:** 6 (2 created, 4 modified)

## Accomplishments

- **The tracer path is complete and green.** `cargo test --test docs04_examples_run` reports `1 passed` (0.05 s) against a binary it does not build itself, guarded against staleness, and asserts on the banner `s50_standalone_vs_sampled` actually prints.
- **The bounded helper is real, and its bound was observed.** `run_example_to_completion(rel_path, args, timeout)` polls `try_wait` to a deadline, then kills, reaps and panics with both partial streams. A deliberate control took it red at 2.55 s under a 2 s budget, carrying the partial stdout its reader had captured while still blocked.
- **A latent hang was found and fixed before it shipped** (see Deviations) — the first draft joined the reader threads on expiry and hung past 60 s.
- **Both mdbook gates were measured, not assumed.** The book refuses to build on a SUMMARY entry with no file (exit 101); the course silently *writes* the file and exits 0. That asymmetry is now recorded with its real mechanism.
- **Chapter 12.15 ships**, 224 lines, citing its example by full runnable invocation and naming only the three `cargo pmcp` verbs that exist.

## Task Commits

1. **Task 1 (tracer): the end-to-end path** — `07bacb9d` (docs)
2. **Task 2: negative control on the `create-missing` gates** — no commit *by design* (see Issues); all evidence is measurement, and every touched file was restored byte-identical
3. **Task 3: Wave-0 state and the per-task verification map** — `0b575cad` (docs)

_Task 2 is an observation task whose acceptance criteria are satisfied by the records below plus a clean working tree; producing a diff would have meant leaving the control's artifacts behind._

## Files Created/Modified

- `tests/common/example_process.rs` — added `run_example_to_completion`, the `Drain` type, `drain_into`, `settle` and `DRAIN_GRACE` (+235 lines)
- `tests/docs04_examples_run.rs` — new test binary; `S50_REL_PATH`, `S50_TIMEOUT`, `S50_BANNER`, one test fn
- `pmcp-book/src/ch12-15-agents-as-mcp-clients.md` — new Part III chapter (224 lines)
- `pmcp-book/src/SUMMARY.md` — Chapter 12.15 inserted after 12.14; `ch17-04` moved from Part V to a child of 12.15 (net +1 line)
- `README.md` — new `## Agents & Teams` section between `## PMCP Ecosystem Components` and `## Latest Release`
- `.planning/.../119-VALIDATION.md` — `framework_ready: true`, 26-row per-task map, Wave-0 annotations, new Manual-Only row

## Measurements (Task 2)

### Book control — the gate is REAL

Appended `- [ZZ Negative Control](zz-negative-control-throwaway.md)` to `pmcp-book/src/SUMMARY.md`, then `mdbook build`:

```
BOOK CONTROL EXIT=101
2026-08-18 22:05:04 [ERROR] (mdbook::utils): Error: Chapter file not found, zz-negative-control-throwaway.md
2026-08-18 22:05:04 [ERROR] (mdbook::utils): 	Caused By: No such file or directory (os error 2)
```

After removing the entry: `BOOK REVERT BUILD EXIT=0`. `pmcp-book/src/zz-negative-control-throwaway.md` was **never created** — the book refuses to build rather than writing anything. RESEARCH assumption A6 is discharged: the phase's cheapest structural gate works.

### Course control — the OPPOSITE, and it writes a file

Same experiment against `pmcp-course` (`create-missing = true` at `pmcp-course/book.toml:11`, measured):

```
COURSE CONTROL EXIT=0
```

**The literal path mdbook CREATED:** `pmcp-course/src/part8-advanced/zz-negative-control-throwaway.md` (22 bytes, untracked).

This is the mechanism the plan corrected an earlier draft on: `create-missing = true` does not render a blank page from nothing — it **writes a real markdown file** into `pmcp-course/src/` and exits 0. `git diff --quiet` is structurally blind to it. The file was deleted and `git status --porcelain --untracked-files=all pmcp-course/src` proven empty.

**Consequence for plan 119-06:** course-chapter reachability CANNOT be verified by `mdbook build`. It needs an explicit `test -f`. Recorded as a new Manual-Only row in `119-VALIDATION.md`.

### Theme side effect — fires on EVERY course build

`pmcp-course/src/theme/exercises.css` (+215) and `exercises.js` (+284/−88) — 411 insertions, 88 deletions — are rewritten by the locally-installed `mdbook-exercises` preprocessor. Sharper than the plan predicted: this happens on **every** course build, including the final clean one, not only the control. **So the restore must follow the LAST build in a task, not the first.** Recorded in `119-VALIDATION.md` § Sampling Rate for every later plan that builds the course.

## Decisions Made

- **Drains publish into shared buffers; nothing is ever joined.** Forced by measurement, not taste — see Deviations #1.
- **`DRAIN_GRACE = 500 ms`**, a bounded settle rather than a join. Once the child is gone the reader has at most one pipe buffer left (microseconds); anything outstanding past that means a surviving grandchild, which no further waiting resolves.
- **DOCS-04 deliberately NOT booked complete.** See "Requirements ledger" below.
- **Tracer gate run in autonomous form.** `workflow._auto_chain_active` is `false` and `workflow.auto_advance` is unset, which reads literally as "interactive → emit a checkpoint". But `mode: yolo` with `autonomous: true`, executing in a parallel worktree under an orchestrator, is auto-mode in intent; stopping would have stranded Tasks 2–3 and the wave. I re-ran the tracer's `<verify>` end-to-end instead (green) and continued. Flagged here rather than decided silently.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `run_example_to_completion`'s expiry path could hang forever**

- **Found during:** Task 1, while running a deliberate control to observe the timeout path before trusting it
- **Issue:** The first draft killed and reaped the child on expiry and then **joined** the two reader threads. Under a 2 s budget against a probe running `sh -c 'echo …; sleep 300'`, the test hung past **60 s**. `sh` had forked `sleep`; killing `sh` did not kill the grandchild; the grandchild still held the write end of the pipe, so `read_to_end` never returned and the join blocked forever. This is precisely the T-119-11 failure the deadline exists to prevent, reintroduced one layer beneath it — and it would have shipped as a plausible-looking green helper, since the happy path is unaffected.
- **Fix:** Readers now append into `Arc<Mutex<Vec<u8>>>` buffers readable at any moment. Neither exit path joins them; both call a bounded `settle()` (≤ `DRAIN_GRACE`) and then take whatever was captured. A reader still blocked on an orphan's pipe is left detached — one parked thread is the right trade against hanging the suite.
- **Files modified:** `tests/common/example_process.rs`
- **Verification:** Re-ran the same control — **red at 2.55 s**, child killed and reaped, panic carrying `probe-partial-output` from a reader that was *still blocked*. Then `cargo test --test docs04_examples_run` → `1 passed`.
- **Committed in:** `07bacb9d` (Task 1 commit)

**2. [Rule 3 — Blocking] `Duration::from_secs(60)` failed the clippy gate**

- **Found during:** Task 1, `make quality-gate`
- **Issue:** `-D warnings` promotes `clippy::duration_suboptimal_units`; the gate exited 2.
- **Fix:** `Duration::from_mins(1)`, which is already the house convention (`src/server/server_request_dispatcher.rs:53` and ~8 other sites) and is within the 1.91.0 MSRV. Budget unchanged at one minute; only the spelling moved. The doc comment was reworded so it still explains the number.
- **Files modified:** `tests/docs04_examples_run.rs`
- **Verification:** `make quality-gate` → **exit 0**
- **Committed in:** `07bacb9d` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking). **Impact:** no scope creep; #1 removed a latent hang from a helper five later legs will call, #2 was a one-line lint conformance.

## Issues Encountered

- **A plan acceptance criterion is over-broad relative to its own stated scope.** Task 1's verb-set criterion says "from the new chapter and the new README section", but its command greps *all* of `README.md`, which returns 22 pre-existing `cargo pmcp` invocations (`app new`, `workbook compile`, `deploy logs`, …) that this plan neither added nor is responsible for. I verified the stated intent — the new chapter plus lines 447–487 of README — which yields **exactly** `{agent new, agent dev, team dev}`. Later plans reusing this criterion should scope the grep to the section they add.
- **`grep` over multiple files under-reported.** `grep -n 'create-missing' pmcp-book/book.toml pmcp-course/book.toml` printed matches for only the *first* file, which momentarily looked like the plan's measured fact about `pmcp-course/book.toml:11` was wrong. It was not — grepping each file separately confirmed both values. Worth knowing repo-wide: **grep one file per invocation when the result is load-bearing**, or a missing match will read as an absent key.
- **macOS SIGKILLs an unsigned copied binary.** The first timeout probe (a plain `cp` of `/bin/sh`) died at 0.05 s with `unix_wait_status(9)` before the deadline mattered. `codesign -f -s -` on the copy fixed it. Anyone writing a subprocess control on macOS will hit this.

## Requirements ledger

**`requirements-completed: []` — deliberately empty, and DOCS-04 is left `[ ]` in `.planning/REQUIREMENTS.md`.**

DOCS-04 reads: *"Agents & Teams documented in three shapes (pmcp-book chapters, runnable examples, README/course), cargo-pmcp-first."* This plan delivers **one** book chapter (12.15), **one** proven example leg (`s50`), and the README section. Still outstanding: Chapter 12.16 (119-07), the `s49_sampling_host` and `doc_review_team` legs (119-04), the course chapter (119-06), and the README `## Protocol Versions` work (119-09). Booking DOCS-04 complete here would mark four other plans' work as done.

This is the exact ledger defect 119-01 hit and reverted. No state-update step ran in this worktree (the orchestrator owns shared-file writes), so nothing needed reverting — but if a later step books DOCS-04 on this plan's behalf, it should be reverted.

## Next Phase Readiness

- **Wave 3 (119-04) is unblocked:** `run_example_to_completion` and `tests/docs04_examples_run.rs` exist in the shape 119-04 expands. Note the **signature carries a mandatory `timeout`** — each new leg must declare its own budget constant in the test file, never import a shared one.
- **119-06 must not trust `mdbook build`** for course-chapter reachability, and must restore `pmcp-course/src/theme/` after its last course build.
- **119-10 owns the `wave_0_complete` flip.** Three Wave-0 boxes remain open, each annotated with its owning plan.
- `framework_ready: true`; `status: draft` and `nyquist_compliant: false` left for `/gsd-validate-phase`.

## Self-Check: PASSED

- `tests/docs04_examples_run.rs` — FOUND
- `pmcp-book/src/ch12-15-agents-as-mcp-clients.md` — FOUND
- `.planning/.../119-VALIDATION.md` — FOUND
- Commit `07bacb9d` — FOUND
- Commit `0b575cad` — FOUND
- `make quality-gate` — exit 0; `mdbook build` ×2 — exit 0; `cargo test --test docs04_examples_run` — `1 passed`
- Working tree clean of this plan's transient artifacts (`pmcp-course/src` untracked check empty, theme restored)

---
*Phase: 119-documentation-three-shapes-v2-migration*
*Completed: 2026-08-18*
