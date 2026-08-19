---
phase: 119-documentation-three-shapes-v2-migration
plan: 04
subsystem: testing
tags: [rust, cargo-test, example-harness, subprocess, streamable-http, mrtr, v2-2026-07-28]

# Dependency graph
requires:
  - phase: 119-02
    provides: "tests/common/example_process.rs (spawn_example, run_example_to_completion with a mandatory timeout, wait_until_listening, wait_until_released, assert_binary_is_not_stale) and the first green leg in tests/docs04_examples_run.rs"
  - phase: 118.1
    provides: "tests/embedded_resource_example_run.rs — the record-both-legs-before-asserting-either house rule and the documented-port-choice convention this plan copies"
provides:
  - "tests/docs04_examples_run.rs at three green run-to-completion legs (s50_standalone_vs_sampled, s49_sampling_host, doc_review_team)"
  - "tests/docs06_v2_examples_run.rs — a socket leg that spawns s47_v2_stateless_mrtr and drives s48_v2_mrtr_client and s53_v2_agent_client against it as real peer processes"
  - "target/119-04-v2-example-run.json — a recorded artifact carrying both v2 client legs' exit statuses and both streams"
  - "A measured, in-code record that clippy's nursery duration_suboptimal_units forbids whole-minute Durations written in seconds — the reason every budget constant in this phase is from_mins"
affects: [119-06, 119-07, 119-09, 119-10]

actuals:
  tokens: 7605
  tasks: 2
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Run-to-completion example legs: assert exit status first, then non-empty stdout, then a positive banner"
    - "Socket example legs: spawn server, run BOTH client example binaries to completion, record both outcomes to a JSON artifact, only then assert, then prove the port released"
    - "Per-leg named budget constants, declared in the owning test file, never shared or imported"

key-files:
  created:
    - tests/docs06_v2_examples_run.rs
  modified:
    - tests/docs04_examples_run.rs

key-decisions:
  - "Booked NO requirements. DOCS-04 needs chapters, README and course (119-06/07/09 own those); DOCS-06's other half is 119-03's example-build gate. Neither is satisfied in full by this plan."
  - "Budget constants are Duration::from_mins, not from_secs: the repo's own clippy nursery gate (duration_suboptimal_units) rejects a whole-minute duration written in seconds. This contradicts a literal acceptance criterion in the plan; the gate wins."
  - "Port 8161 for the DOCS-06 socket leg — the next free slot after 8147/8149/8150/8151/8153/8155/8157/8159, and passed explicitly as argv[1] to all three binaries because s47's own default 8147 is claimed."
  - "The doc_review_team invocation discrepancy is RECORDED, not fixed: the tests use `--features runtime`, the example header says `--all-features`. D-12 forbids editing example source."
  - "PMCP_REQUEST_STATE_KEY is deliberately left unset so the leg exercises the default per-process-key path; the resulting startup WARN is documented in the header but not asserted on, because spawn_example discards the child's streams."

patterns-established:
  - "Two harness idioms may coexist in one file when they answer different questions (spawn+poll for a server, run-to-completion for a client that is itself an example binary) — say so in the header so the next reader does not unify them"
  - "A budget constant's doc comment states the MEASURED cost it was set against and that it exists to convert a hang into a red, not to police performance"

requirements-completed: []

coverage:
  - id: D1
    description: "s49_sampling_host runs to completion and prints its inverse-direction round-trip banner"
    requirement: "DOCS-04"
    verification:
      - kind: integration
        ref: "tests/docs04_examples_run.rs#s49_sampling_host_runs_to_completion"
        status: pass
    human_judgment: false
  - id: D2
    description: "doc_review_team runs all four reference servers to completion in one process and prints its flow-complete banner"
    requirement: "DOCS-04"
    verification:
      - kind: integration
        ref: "tests/docs04_examples_run.rs#doc_review_team_runs_to_completion"
        status: pass
    human_judgment: false
  - id: D3
    description: "The stateless v2 server example s47_v2_stateless_mrtr serves BOTH documented v2 client examples (s48_v2_mrtr_client, s53_v2_agent_client) over a real socket, with both outcomes recorded before either is asserted and the port proven released after teardown"
    requirement: "DOCS-06"
    verification:
      - kind: e2e
        ref: "tests/docs06_v2_examples_run.rs#s47_serves_both_v2_client_examples_end_to_end"
        status: pass
      - kind: other
        ref: "target/119-04-v2-example-run.json — both client legs recorded, exit_success true, banner present"
        status: pass
    human_judgment: false
  - id: D4
    description: "Every run-to-completion leg is bounded by its own named budget constant, so a non-terminating example turns the suite red rather than hanging it"
    verification:
      - kind: unit
        ref: "grep -c S49_TIMEOUT/DOC_REVIEW_TEAM_TIMEOUT/S48_TIMEOUT/S53_TIMEOUT — each >= 2 (declaration + call site)"
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-08-18
status: complete
---

# Phase 119 Plan 04: Example-Run Harness Completion Summary

**Three DOCS-04 examples proven to RUN (not build), and the stateless v2 server proven to serve both documented v2 client examples as real peer processes over a real socket with no `initialize` handshake and no `Mcp-Session-Id`.**

## Performance

- **Duration:** ~25 min (execution), plus two `make quality-gate` runs
- **Started:** 2026-08-18T22:05Z (approx, worktree spawn)
- **Completed:** 2026-08-18T22:30Z (last code commit)
- **Tasks:** 2
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- `tests/docs04_examples_run.rs` now runs **all three** DOCS-04 examples to completion — `s50_standalone_vs_sampled` (inherited from the tracer), `s49_sampling_host`, and `doc_review_team` — each asserting exit status, then non-empty stdout, then a positive banner. `cargo test --test docs04_examples_run` reports **3 passed**.
- `tests/docs06_v2_examples_run.rs` (new) spawns `s47_v2_stateless_mrtr` on `127.0.0.1:8161` and drives **both** documented v2 clients against it as real peer processes: `s48_v2_mrtr_client` and `s53_v2_agent_client`. Both complete, both print `All three demonstrations behaved as documented.`, both are recorded to `target/119-04-v2-example-run.json` **before** either is asserted, and the port is proven connect-refusing after teardown. `cargo test --features full --test docs06_v2_examples_run` reports **1 passed**.
- Every run-to-completion leg carries its own named budget constant (`S49_TIMEOUT`, `DOC_REVIEW_TEAM_TIMEOUT`, `S48_TIMEOUT`, `S53_TIMEOUT`), closing T-119-21: `wait_until_listening` bounds only the bind, so without these a server that accepted a connection and then stopped answering would hang the integration suite rather than fail it.
- No skip path anywhere: `#[ignore]` count is 0 in both files, `Stdio::null` appears on no non-comment line of `docs04_examples_run.rs`, and both files' missing-binary paths were **driven red** (see Evidence) rather than assumed.
- `make quality-gate` exits **0** with both new binaries running inside it.

## Task Commits

1. **Task 1: Add the two remaining DOCS-04 run-to-completion legs** — `5c1945fc` (test)
2. **Task 1 follow-up: budget-unit fix demanded by the clippy gate** — `6791cad3` (fix)
3. **Task 2: DOCS-06 socket leg — s47 served, s48 and s53 driven as real peers** — `3a078d03` (test)

Both tasks were `tdd="true"` with test-only deliverables. There is no production code to write for either — the six examples already exist and D-12 forbids editing them — so the RED/GREEN cycle collapses into one `test(...)` commit per task, with RED driven explicitly before each commit (below) instead of being represented by a separate empty implementation commit.

## Files Created/Modified

- `tests/docs06_v2_examples_run.rs` (created, 258 lines) — the DOCS-06 socket leg. Five-section module header: why it exists, why two harness idioms coexist in one file, "Port 8161, deliberately", why both legs run before either is asserted, why the client legs need a budget, and the `PMCP_REQUEST_STATE_KEY` startup-warning record.
- `tests/docs04_examples_run.rs` (modified, +213/-3) — two new legs plus their path/budget/banner constants; module header extended with the three-leg table, the per-leg-budget rationale, and the positive-marker rule.

## Evidence

**RED was driven, not assumed.**

- Task 1: `target/debug/examples/s49_sampling_host` moved aside → `cargo test --test docs04_examples_run` reported `1 passed; 2 failed`, both failures being the missing-binary panic naming the exact `cargo build` command. Binary restored → `3 passed`.
- Task 2: run before building the three v2 examples → `0 passed; 1 failed`, panicking with `... s47_v2_stateless_mrtr is missing. This leg FAILS rather than skipping, by design`. After building → `1 passed` in 0.87 s.

**Measured example costs** (this worktree, `time ./target/debug/examples/<name>`):

| Example | Measured | Budget | Ratio |
|---|---|---|---|
| `s49_sampling_host` | 0.014 s | `from_mins(1)` | ~4000x |
| `doc_review_team` | 0.041 s | `from_mins(2)` | ~2900x |
| `s48` + `s53` against a live `s47` | 0.87 s (whole leg) | `from_mins(1)` each | ~70x |

**Recorded artifact** `target/119-04-v2-example-run.json`: `s48_v2_mrtr_client` → `exit status: 0`, banner present; `s53_v2_agent_client` → `exit status: 0`, banner present; `bind_addr` `127.0.0.1:8161`.

**Gate results:** `make lint` exit 0 · `cargo fmt --all -- --check` exit 0 · `cargo test --test docs04_examples_run` 3 passed · `cargo test --features full --test docs06_v2_examples_run` 1 passed · `make quality-gate` exit 0 (both new binaries observed running inside `test-all`).

## Decisions Made

1. **`requirements-completed: []`.** Neither DOCS-04 nor DOCS-06 is satisfied in full here. DOCS-04 requires chapters, runnable examples, README **and** course — 119-06, 119-07 and 119-09 own three of those four. DOCS-06's other half (the repaired `make test-examples` example-build gate) belongs to 119-03, which is outside this plan's file scope. Booking either would credit work this plan did not do.
2. **Budget constants are `Duration::from_mins`.** See Deviation 1.
3. **The `doc_review_team` invocation discrepancy is recorded, not resolved.** See "For plans 119-06 and 119-07" below.
4. **Port 8161**, chosen against the eight already-claimed 81xx ports and passed explicitly as `argv[1]` to server and both clients rather than relying on three defaults agreeing. It ran concurrently with `embedded_resource_example_run.rs` (8157) inside `make quality-gate` with no collision.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The plan's `Duration::from_secs` criterion is unsatisfiable under the repo's own clippy gate**

- **Found during:** Task 1 verification (`make lint`), then again in Task 2's new file.
- **Issue:** The plan specifies `const S49_TIMEOUT: Duration = Duration::from_secs(60);` and `from_secs(120)`, and an acceptance criterion requires `grep -c 'Duration::from_secs'` ≥ 3. But `make lint` runs clippy's **nursery** group, and `clippy::duration_suboptimal_units` errors on a whole-minute `Duration` written in seconds ("constructing a `Duration` using a smaller unit when a larger unit would be more readable"). This is exactly why the tracer wrote `S50_TIMEOUT` as `Duration::from_mins(1)`. I first "normalised" `S50_TIMEOUT` to `from_secs(60)` for cross-leg comparability — which turned `make lint` from green to **exit 101 with three errors**, two of them mine.
- **Fix:** Reverted `S50_TIMEOUT` to `from_mins(1)`; declared `S49_TIMEOUT = from_mins(1)`, `DOC_REVIEW_TEAM_TIMEOUT = from_mins(2)`, `S48_TIMEOUT = from_mins(1)`, `S53_TIMEOUT = from_mins(1)`. Each constant's doc comment now records the lint reason so the next reader does not repeat the same failed edit. `READY_TIMEOUT` (30 s) and `RELEASE_TIMEOUT` (10 s) are sub-minute and stay `from_secs`.
- **Files modified:** `tests/docs04_examples_run.rs`, `tests/docs06_v2_examples_run.rs`
- **Verification:** `make lint` exit 0 (was 101); both suites still green.
- **Committed in:** `6791cad3` (docs04) and `3a078d03` (docs06, never committed in the broken form)
- **Criterion status:** The literal criterion `grep -c 'Duration::from_secs' >= 3` is **NOT met** (count is 1, in a doc comment explaining why). Its stated INTENT — "the budgets are distinct rather than copied" — **is** met: `from_mins(1)` / `from_mins(1)` / `from_mins(2)`, each with its own measured rationale, and the double for `doc_review_team` is now visible directly in the literal. The plan's own success criterion `make quality-gate exits 0` takes precedence over a grep that contradicts it.

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** No scope creep. Both artifacts and all four test fns are exactly as specified; only the numeric literal's unit changed, and it changed to satisfy a gate the plan itself lists as a success criterion.

## Issues Encountered

**`make quality-gate` first run failed with `No space left on device` — environmental, not a code defect.**

The first `make quality-gate` run failed in `test-doc` with `error: failed to write to .../full.rmeta: No space left on device (os error 28)`, `make: *** [quality-gate] Error 2`. `df -h /` showed **193 MiB free** on the root volume. This is the known shape recorded in project memory ("Disk exhaustion fakes code regressions") and had nothing to do with these changes — `make lint`, `cargo fmt --check` and both new suites were already green when it fired.

Resolved by deleting **only** regenerable build cache inside this worktree: `rm -rf target/debug/incremental` (5.9 GB), taking free space to 4.7 GiB. Nothing tracked, no example binary and no sibling worktree was touched. The re-run exited **0**.

Residual risk for the orchestrator: the machine's root volume reports `926Gi size / 12Gi used / 4.7Gi avail`, which means most of the volume is held by APFS snapshots or purgeable space that a sub-agent cannot reclaim. Sibling worktrees running `make quality-gate` in this wave may hit the same wall. That is a machine-level condition needing operator action, not a phase defect.

## For plans 119-06 and 119-07 — action required

`crates/pmcp-team-servers/examples/doc_review_team.rs:24` documents the invocation as:

```bash
cargo run -p pmcp-team-servers --example doc_review_team --all-features
```

The tests, and the canonical form adopted by this plan, use the minimal invocation that was actually measured:

```bash
cargo run -p pmcp-team-servers --example doc_review_team --features runtime
```

**Both work** — `Cargo.toml:161-163` declares `required-features = ["runtime"]`, and `--all-features` is a superset. The defect is that the docs and the tests would otherwise disagree. D-12 forbids editing example source, so the header was left alone. **Plans 119-06 and 119-07, which cite this example in prose, must use the `--features runtime` form.** If a later plan is permitted to touch example source, the header at line 24 should be brought into line.

## Known Stubs

None. No skip path, no `#[ignore]`, no `Stdio::null()` on a run-to-completion leg, no unrun `<verify>`, and every acceptance criterion was executed rather than reasoned about. The one criterion not met (`Duration::from_secs` count) is documented above with its measured cause.

## Threat Flags

None. Both files use only `std`, `serde_json` and the existing `tests/common/` modules — zero packages installed (T-119-SC holds). No key material appears in either file.

## Notes for the orchestrator

- **STATE.md, ROADMAP.md and REQUIREMENTS.md were NOT modified** (worktree mode; `requirements-completed` is empty by decision, so there is nothing to book).
- `Makefile` was NOT touched — plan 119-03 owns it this wave.
- The phase `deferred-items.md` was NOT touched (119-03 owns it); the disk-exhaustion note and the `doc_review_team` header discrepancy are recorded here instead.

## Next Phase Readiness

- D-15's six examples are now all covered by an automated RUN test: three in `tests/docs04_examples_run.rs`, three (one server + two clients) in `tests/docs06_v2_examples_run.rs`. None is covered by a build-only check.
- 119-10's closing `make quality-gate` gate has one fewer unknown: both new binaries already run green inside it.
- Wave-4 prose plans (119-06, 119-07, 119-09) can now cite these examples knowing the citations are enforced by a running test, provided they use the `--features runtime` form for `doc_review_team`.

## Self-Check: PASSED

- Files: `tests/docs04_examples_run.rs`, `tests/docs06_v2_examples_run.rs`, `119-04-SUMMARY.md`, `target/119-04-v2-example-run.json` — all present.
- Commits: `5c1945fc`, `6791cad3`, `3a078d03`, `19145d36` — all resolve to commit objects.
- Working tree clean (`.pmat/*` cache churn from the gate restored with `git checkout -- .pmat/`).

---
*Phase: 119-documentation-three-shapes-v2-migration*
*Completed: 2026-08-18*
