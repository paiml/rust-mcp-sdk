---
phase: 119
slug: documentation-three-shapes-v2-migration
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
# wave_0_complete: TRUE only when every box in `## Wave 0 Requirements` below is ticked.
# Owner of the flip: plan 119-10 (the closing gate), because 119-10 writes the last Wave-0
# item (`tests/windows_disclosure_tripwire.rs`). Plan 119-02 must NOT set it.
wave_0_complete: true
# framework_ready: the NARROWER fact — the harness and tooling Wave 0 exists to unblock are
# present and exercised (mdbook + mdbook-mermaid on PATH; `run_example_to_completion` in
# `tests/common/example_process.rs`; `tests/docs04_examples_run.rs` green on its first leg) —
# independently of whether every Wave-0 checklist item has been written yet. Set by plan 119-02.
framework_ready: true
created: 2026-08-18
---

# Phase 119 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `119-RESEARCH.md` § Validation Architecture. The planner fills the
> Per-Task Verification Map once task IDs exist.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]`, run via `cargo test` and `cargo nextest` |
| **Config file** | none for `cargo test`; nextest available and used repo-wide |
| **Quick run command** | `cargo test --features full --test <name>` |
| **Full suite command** | `make quality-gate` (12 sub-targets; `test-all` = unit + doc + property + examples + integration) |
| **CI command** | `cargo test --all-features --verbose -- --test-threads=1` (`.github/workflows/ci.yml:93`) |
| **Docs gate (separate)** | `cd pmcp-book && mdbook build`; `cd pmcp-course && mdbook build` (`docs.yml`) — **not chained into `make quality-gate`** |
| **Estimated runtime** | ~0.09 s for one example-run leg (measured); `make quality-gate` is minutes |

---

## Sampling Rate

- **After every task commit:** `cargo test --features full --test <tests the task touched>` +
  `cargo fmt --all -- --check`
- **After every commit that touches a `SUMMARY.md`:** `mdbook build` for that book —
  `create-missing = false` makes this the ONLY detector of a SUMMARY↔file break, and it is not
  in `make quality-gate`, so per-commit sampling is the plan's responsibility
- **After every plan wave:** `make quality-gate` (chains the repaired `test-examples` and
  `test-integration`), plus `mdbook build` for any wave that touched a book
- **After ANY `pmcp-course` build (measured, plan 119-02 T2):** restore
  `pmcp-course/src/theme/`. The locally-installed `mdbook-exercises` preprocessor reinstalls its
  own assets on every run, rewriting the two TRACKED files `exercises.css` and `exercises.js` with
  a **+411/-88 line** diff. This fires on EVERY course build, not just a failing one, so the
  restore must follow the LAST build in a task — not the first. Never commit that diff (T-119-10)
- **Before `/gsd-verify-work`:** `make quality-gate` exit 0 **AND** `mdbook build` ×2 exit 0
  **AND** `gsd-tools windows status` parses **AND** `cargo package --list` reviewed
- **Max feedback latency:** < 5 s for the per-task leg

**Nyquist justification.** Three failure modes with three different frequencies get three
sampling rates. Prose defects → review only (no automated frequency). Structural defects
(SUMMARY↔file) change on every chapter commit → sample at every SUMMARY-touching commit.
Behavioural defects (an example stops working) change at the frequency of `src/` edits, which
this phase makes almost none of → per-wave sampling suffices, with the binary-staleness guard
covering the residual risk. The D-03 tripwire samples at CI frequency because the thing it
guards (a *future* `WINDOWS.md` entry) changes on a cadence this phase does not control.

---

## Per-Task Verification Map

One row per automated verification in the phase, keyed by plan and task, sourced from each
plan's `<verify>` block. Plan 119-02's rows are measured; every later row is pending until its
plan executes.

| Plan · Task | Req | Behavior to validate | Test type | Automated command | Status |
|---|---|---|---|---|---|
| 119-01 · T1–T3 | DOCS-05 | The eleven requirements are `[x]` and `113-SPEC-RECHECK.md` `## Verdict` is `PUBLISHED-CONFIRMED` | ledger assertion | `grep -c '^- \[~\] \*\*HTTP-0\|^- \[~\] \*\*CLNT-0' .planning/REQUIREMENTS.md` → 0; `grep -A2 '^## Verdict' .planning/phases/113-*/113-SPEC-RECHECK.md` → not `PENDING` | ✅ green (wave 1) |
| 119-02 · T1 | DOCS-04 | `s50_standalone_vs_sampled` runs to completion and prints its banner; Chapter 12.15 + `ch17-04` reachable; README section exists | run test + structural | `cargo build -p pmcp-agent --example s50_standalone_vs_sampled && cargo test --test docs04_examples_run` | ✅ green — `1 passed`, 0.05 s |
| 119-02 · T1 | DOCS-04 | The book still builds with the new chapter and the re-parented child | structural | `mdbook build` (pmcp-book) | ✅ green — exit 0 |
| 119-02 · T2 | all | `create-missing = false` makes `mdbook build` FAIL on a SUMMARY entry with no file | negative control | append a missing target, `mdbook build`, observe non-zero, revert | ✅ green — observed **exit 101**, `Error: Chapter file not found, zz-negative-control-throwaway.md`; exit 0 after revert |
| 119-02 · T2 | all | The COURSE book does the OPPOSITE — `create-missing = true` writes the missing file and exits 0 | negative control | same experiment against `pmcp-course` | ✅ green — exit **0**; created `pmcp-course/src/part8-advanced/zz-negative-control-throwaway.md` (22 bytes), deleted, untracked check clean |
| 119-02 · T3 | all | Validation state records measured Wave-0 facts | structural | `grep -q '^framework_ready: true' … && grep -q '^wave_0_complete: false' … && grep -q '^status: draft' …` | ✅ green |
| 119-03 · T1 | DOCS-06 | The example-build baseline is recorded before the loop is touched | structural | `D=.planning/phases/119-documentation-three-shapes-v2-migration; test -f "$D/deferred-items.md" && grep -q 'the D-14 baseline, taken BEFORE the D-13 gate change' "$D/deferred-items.md" && git merge-base --is-ancestor 5b90fdd2 9aefc939` — artifact is `deferred-items.md`, not a `119-03-BASELINE.md` (corrected by orchestrator; the original row named a file no plan ever specified). Ordering is proven by ancestry: baseline commit `5b90fdd2` touches only `deferred-items.md` and is an ancestor of gate commit `9aefc939`. | ⬜ pending (wave 2) |
| 119-03 · T2 | DOCS-06 | The counted, exit-1 example build replaces the swallowing loop | structural | `test -x scripts/run-example-builds.sh && make test-examples` | ⬜ pending (wave 2) |
| 119-03 · T3 | DOCS-06 | The repaired `make test-examples` FAILS on a broken example | negative control | `make test-examples && git diff --quiet -- examples/s49_sampling_host.rs` | ⬜ pending (wave 2) |
| 119-04 · T1 | DOCS-04 | `s49_sampling_host` and `doc_review_team` run to completion (the remaining `docs04` legs) | run test | `cargo build --example s49_sampling_host && cargo build -p pmcp-team-servers … && cargo test --test docs04_examples_run` | ⬜ pending (wave 3) |
| 119-04 · T2 | DOCS-06 | `s47_v2_stateless_mrtr` serves a real v2 MRTR round trip, driving `s48`/`s53` as children | run test (socket) | `cargo build --features full --example s47_v2_stateless_mrtr … && cargo test --test docs06_v2_examples_run` | ⬜ pending (wave 3) |
| 119-05 · T1 | DOCS-05 | Consumer-observable disclosures carry the `[CONSUMER-OBSERVABLE]` marker | ledger assertion | `gsd-tools` windows query (see plan) | ⬜ pending (wave 3) |
| 119-05 · T2 | DOCS-05 | The migration chapter exists and the book builds | structural | `test -f pmcp-book/src/ch12-17-migrating-to-mcp-2026-07-28.md && mdbook build` | ⬜ pending (wave 3) |
| 119-05 · T3 | DOCS-05 | The chapter agrees with the settled ledger and the sunset policy | structural | `mdbook build` (pmcp-book) | ⬜ pending (wave 3) |
| 119-06 · T1 | DOCS-04 | The course chapter exists at `ch23-skills` depth | structural | `test -f pmcp-course/src/part8-advanced/ch24-agents-and-teams.md && …` | ⬜ pending (wave 4) |
| 119-06 · T2 | DOCS-04 | The exercises file exists with measured pass predicates | structural | `test -f pmcp-course/src/part8-advanced/ch24-exercises.md && …` | ⬜ pending (wave 4) |
| 119-06 · T3 | DOCS-04 | Part VIII nav is wired and the course chapter is reachable | structural (`test -f`, NOT the build — see Manual-Only) | `test -f pmcp-course/src/part8-advanced/ch24-agents-and-teams.md && …` | ⬜ pending (wave 4) |
| 119-07 · T1 | DOCS-04 | The Agent Teams chapter exists and its cited example builds | structural + build | `test -f pmcp-book/src/ch12-16-agent-teams.md && cargo build -p pmcp-team-servers …` | ⬜ pending (wave 4) |
| 119-07 · T2 | DOCS-04 | All three Phase-111 chapters are reachable in both books | structural | `mdbook build` ×2 | ⬜ pending (wave 4) |
| 119-08 · T1 | DOCS-05 | `ch12-7-tasks.md` carries a provisional v2 era delta | structural | `test "$(git show HEAD:pmcp-book/src/ch12-7-tasks.md \| wc -l)" -lt …` | ⬜ pending (wave 4) |
| 119-08 · T2 | DOCS-05 | Era callouts added to both transport chapters with no code-block churn | structural | `git show HEAD:pmcp-book/src/ch10-transports.md \| awk …` | ⬜ pending (wave 4) |
| 119-09 · T1 | DOCS-04 | README gains `## Protocol Versions` and stale references are refreshed | structural | `test "$(grep -c '^## Protocol Versions' README.md)" -eq 1 && …` | ⬜ pending (wave 4) |
| 119-09 · T2 | DOCS-04 | The `## Examples` block's cited invocations build | build | `cargo build --features full --example s50_v2_tasks_server --example s51_v2_tasks_agent` — **`--features full` is REQUIRED and was missing from this row.** Corrected by the orchestrator after plan 119-08 measured it and the orchestrator re-measured independently: flagless, `s51_v2_tasks_agent` fails with `error[E0433]: cannot find 'testing' in 'pmcp'` (`pub mod testing` is feature-gated at `src/lib.rs:63`, and `default = ["logging", "v1-compat"]`). The original row encoded a false premise — that the absence of an `[[example]]` block implies no required features — so it would have failed against a correct README. | ⬜ pending (wave 4) |
| 119-09 · T3 | DOCS-05 | The two mislabelled CHANGELOG headings are corrected | structural | `test "$(grep -c '^## \[2.19.0\] - Unreleased' CHANGELOG.md)" -eq 1 && …` | ⬜ pending (wave 4) |
| 119-10 · T1 | DOCS-05 | The derived-not-enumerated disclosure tripwire passes | tripwire (D-03) | `cargo test --features full --test windows_disclosure_tripwire` | ✅ green — `1 passed`; derived set = ledger entries 12, 13, 19, 20, 23, computed from the sentinel, never enumerated |
| 119-10 · T2 | DOCS-05 | Removing one citation turns the tripwire RED — **and so does marking a NEW entry** | negative control ×2 | `cargo test --features full --test windows_disclosure_tripwire && git diff --quiet …` | ✅ green — **Control A** (entry 19's citation deleted) → `entries [19] … not cited`; **Control B** (sentinel added to previously-unmarked entry 1) → `entries [1] … not cited`, proving derivation. Both files restored byte-identical; `open_count: 17` / `total_count: 23` unchanged |
| 119-10 · T3 | all | The full phase gate passes | full suite | `make quality-gate` | ✅ green — exit 0 to the `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` banner; `mdbook build` ×2 exit 0; ledger parses; `cargo package --list` reviewed. **Caveat on record:** the gate's FUZZ leg fuzzed nothing (42 targets "completed" over 25 `-Z … nightly compiler` errors on stable — ledger entry 22), so this green is NOT fuzz coverage |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

> **`framework_ready` ≠ `wave_0_complete`.** `framework_ready: true` (set by plan 119-02) is the
> narrower fact that the harness and tooling Wave 0 exists to unblock are present and exercised.
> `wave_0_complete` stays `false` until every box below is ticked; **plan 119-10's closing gate
> owns that flip**. A `wave_0_complete: true` sitting above open boxes would be a false green in a
> field `/gsd-audit-milestone` parses (`gsd-core/workflows/audit-milestone.md:167`).
>
> **What a tick means here — read this before trusting the flag.** For the four mandatory items a
> tick means BUILT AND MEASURED GREEN, and plan 119-10 re-ran each one itself rather than
> inheriting a prior plan's SUMMARY claim (the measured commands and counts are named inline
> below). For the two items prefixed `Optional:` a tick means DISPOSITIONED, **not** built: both
> were explicitly DECLINED for this phase by plan 119-02 on blast-radius grounds, each carries its
> compensating control in its own text, and neither will be found in the tree. Ticking a declined
> optional item is what lets `wave_0_complete` and this checklist agree without either one
> overclaiming; a reader who needs "was it built?" must read the item, not the box.
>
> **Flipped to `true` by plan 119-10 on 2026-08-19**, after walking all six items against fresh
> measurements taken in a clean worktree.

- [x] **Local `mdbook` + `mdbook-mermaid`** — installed and exercised by plan 119-02.
      Measured **v0.4.52** locally versus CI's pinned v0.4.40 (`docs.yml`), and `mdbook-exercises`
      is also present (the course build needs it). Both books build clean. This CORRECTS
      119-RESEARCH's "missing locally" entry, which is why assumption A6 (the
      `create-missing = false` gate had never actually been observed failing) could finally be
      discharged — see the 119-02 · T2 rows above
- [x] `tests/docs06_v2_examples_run.rs` — DOCS-06's s47/s48/s53 leg (socket shape, port 8161)
      — **owner: plan 119-04**. **VERIFIED by plan 119-10's closing gate**, not inherited from a
      SUMMARY: `cargo test --features full --test docs06_v2_examples_run` → `1 passed` (0.86 s)
- [x] `tests/docs04_examples_run.rs` — the REMAINING DOCS-04 legs (`s49_sampling_host`,
      `doc_review_team`) — **owner: plan 119-04**. The file itself now exists and is green on its
      first leg (`s50_standalone_vs_sampled`, plan 119-02); this box tracks the other two.
      **VERIFIED by plan 119-10's closing gate**: `cargo test --test docs04_examples_run` →
      `3 passed` (0.06 s), i.e. all three legs, not just the two this box tracks
- [x] `tests/windows_disclosure_tripwire.rs` — D-03, with the `v2_conformance_pin` excluded-tree
      guard applied twice (research § F-3: the packaging fork excludes **two** trees)
      — **owner: plan 119-10**. **DONE**: `1 passed`, and OBSERVED RED twice (a removed citation,
      and a newly-marked ledger entry the test had never seen) before being trusted
- [x] A **run-to-completion helper** in `tests/common/example_process.rs`:
      `run_example_to_completion(rel_path: &str, args: &[&str], timeout: Duration) -> Output` —
      three of the six examples need it; `spawn_example`'s `Stdio::null()` + `wait_until_listening`
      are wrong for them. The `timeout` argument is MANDATORY (cross-AI review, HIGH): the helper
      must drain both streams concurrently, poll to a deadline, and kill + reap + panic with the
      captured partial output on expiry, so a non-terminating example fails the suite rather than
      hanging it. Each leg declares its own budget constant in its own test file
      — **DONE (plan 119-02)**: `run_example_to_completion(rel_path, args, timeout) -> Output`.
      The bound is real and was OBSERVED, not assumed: a probe left running under a 2 s budget went
      red at 2.55 s, killed, reaped, carrying its partial stdout. That control also found a defect
      in the first draft — it joined the reader threads on expiry and hung past 60 s when the
      child's forked grandchild still held the pipe, so the drains now publish into shared buffers
      that are readable without a join. `S50_TIMEOUT` lives in `tests/docs04_examples_run.rs`, per
      the "timeouts are arguments" house rule
- [x] Optional: generalize `assert_binary_is_not_stale`'s source roots to reach
      `crates/*/examples/` and `crates/*/src/` (research § F-7 gap)
      — **DECLINED for this phase (plan 119-02).** This is the recorded disposition of the cross-AI
      review's staleness-guard finding (119-02 `<review_dispositions>`, downgraded by consensus to
      LOW). Generalizing the root set changes a guard four existing test legs already depend on,
      which is outside a documentation phase's blast radius. The limitation is instead DOCUMENTED
      in `run_example_to_completion`'s rustdoc and restated in each non-root leg's constant doc
      comment, rather than silently inherited. **Compensating control:** every path that runs these
      legs builds the binary first with an explicit `-p <crate>` invocation, and that same command
      is named in the panic messages the guard would otherwise raise
- [x] Optional: a `make book` / `make course` target so the docs gate is reachable from the dev loop
      — **DECLINED for this phase (plan 119-02)**, same blast-radius reasoning. Plan 119-10's
      closing gate runs both `mdbook build` invocations explicitly instead, so the docs gate is
      still executed before the phase closes; it just is not reachable from a `make` target

*No framework install needed — `cargo test` is already the harness.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Migration chapter links to, and does not restate or contradict, `docs/v1-sunset-policy.md` | DOCS-05 | A mechanical "does not restate" check is brittle; the policy is normative prose | Read `docs/v1-sunset-policy.md` (esp. its table of the seven items deliberately NOT severed), then read the new chapter's sunset section; confirm it links and adds no competing normative claim |
| Each new chapter/section leads with the `cargo pmcp` workflow before dropping to crate-level API | DOCS-04, DOCS-05 | Ordering/emphasis is a judgement, not a string match | Read each new chapter and each new README section top-down; the first runnable command in each must be a `cargo pmcp` invocation |
| The Tasks era-delta carries a provisionality callout (research § F-2: TASK-01..06 remain `[~]`; only "the wire is final" is unquotable) | DOCS-05 | Provisionality is a prose property | Read the amended `ch12-7-tasks.md` era section; confirm behaviours T-1..T-7 are stated as shipped with source cites, and the wire-finality claim is explicitly marked provisional |
| A pmcp-course chapter file is actually PRESENT for every course `SUMMARY.md` entry | DOCS-04 | **No build catches this.** `pmcp-course/book.toml:11` sets `create-missing = true` (measured, plan 119-02 T2), so the mechanism is that `mdbook build` **WRITES the missing file** into `pmcp-course/src/` and exits 0 — it does not render a blank page, and it does not skip. The build is therefore structurally incapable of detecting the break. The book is the opposite: `pmcp-book/book.toml:14` sets `create-missing = false` and the build exits 101 naming the missing file | Plan 119-06 must assert course-chapter reachability with an explicit `test -f` on each new chapter path — **never** by relying on `mdbook build` exiting 0. Corollary for any plan that builds the course: check `git status --porcelain --untracked-files=all pmcp-course/src`, because a file mdbook created is UNTRACKED and `git diff --quiet` cannot see it |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5 s for the per-task leg
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
