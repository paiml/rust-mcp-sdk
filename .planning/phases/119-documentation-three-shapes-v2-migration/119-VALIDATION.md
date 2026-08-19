---
phase: 119
slug: documentation-three-shapes-v2-migration
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
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

*To be filled by the planner once task IDs exist. Requirement→behaviour map from research:*

| Req | Behavior to validate | Test type | Automated command | File exists |
|-----|----------------------|-----------|-------------------|-------------|
| DOCS-04 | The three named chapters are reachable in the book (2 new + `ch17-04` re-parented) and the course chapter is reachable | structural | `cd pmcp-book && mdbook build` (fails on missing file: `create-missing = false`) | ❌ Wave 0 — needs local mdbook |
| DOCS-04 | `s49_sampling_host` runs to completion | run test | `cargo test --features full --test docs04_examples_run` → `status.success()` + stdout marker | ❌ Wave 1 |
| DOCS-04 | `s50_standalone_vs_sampled` runs to completion | run test | same binary, second leg (`cargo build -p pmcp-agent --example …` prerequisite) | ❌ Wave 1 |
| DOCS-04 | `doc_review_team` runs to completion | run test | same binary, third leg (`-p pmcp-team-servers --features runtime`) | ❌ Wave 1 |
| DOCS-04 | Every `cargo pmcp` command named in the docs exists | structural | assert doc command strings against `cargo pmcp agent --help` / `team --help` | ❌ Wave 2 — recommended; research § C-3 shows the risk is real (2 verbs, not 4) |
| DOCS-05 | Every consumer-observable `WINDOWS.md` entry is cited in the migration chapter | tripwire (D-03) | `cargo test --features full --test windows_disclosure_tripwire` — derive ids from the sentinel, never hard-code | ❌ Wave 3 |
| DOCS-05 | The eleven requirements are `[x]` and `113-SPEC-RECHECK.md` `## Verdict` is `PUBLISHED-CONFIRMED` | ledger assertion | `grep -c '^- \[~\] \*\*HTTP-0\|^- \[~\] \*\*CLNT-0' .planning/REQUIREMENTS.md` → must be 0; `grep -A2 '^## Verdict' .planning/phases/113-*/113-SPEC-RECHECK.md` → must not say `PENDING` | ✅ shell, in the plan's verify block |
| DOCS-05 | The chapter does not restate `docs/v1-sunset-policy.md` | review | manual — a mechanical check would be brittle | n/a |
| DOCS-06 | `s47_v2_stateless_mrtr` serves a real v2 MRTR round trip | run test (socket) | `cargo test --features full --test docs06_v2_examples_run` — `spawn_example` on port 8161, drive `s48` and `s53` as children, assert both exit 0 | ❌ Wave 1 |
| DOCS-06 | The repaired `make test-examples` FAILS on a broken example | negative control | deliberately break an example, observe RED, revert. **Mandatory** — a gate never observed failing is not known to work | ❌ Wave 1 |
| all | Both books build | structural | `mdbook build` ×2 | ❌ Wave 0 tooling |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] **Local `mdbook` v0.4.40 + `mdbook-mermaid`** — without them the SUMMARY↔file invariant has
      no local detector. `cargo install mdbook --version 0.4.40 && cargo install mdbook-mermaid`
- [ ] `tests/docs06_v2_examples_run.rs` — DOCS-06's s47/s48/s53 leg (socket shape, port 8161)
- [ ] `tests/docs04_examples_run.rs` — DOCS-04's s49/s50/`doc_review_team` legs
      (run-to-completion shape). **This shape does not exist yet** — both existing tests are
      socket-shaped
- [ ] `tests/windows_disclosure_tripwire.rs` — D-03, with the `v2_conformance_pin` excluded-tree
      guard applied twice (research § F-3: the packaging fork excludes **two** trees)
- [ ] A **run-to-completion helper** in `tests/common/example_process.rs` (e.g.
      `run_example_to_completion(rel_path, args) -> Output`) — three of the six examples need it;
      `spawn_example`'s `Stdio::null()` + `wait_until_listening` are wrong for them
- [ ] Optional: generalize `assert_binary_is_not_stale`'s source roots to reach
      `crates/*/examples/` and `crates/*/src/` (research § F-7 gap)
- [ ] Optional: a `make book` / `make course` target so the docs gate is reachable from the dev loop

*No framework install needed — `cargo test` is already the harness.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Migration chapter links to, and does not restate or contradict, `docs/v1-sunset-policy.md` | DOCS-05 | A mechanical "does not restate" check is brittle; the policy is normative prose | Read `docs/v1-sunset-policy.md` (esp. its table of the seven items deliberately NOT severed), then read the new chapter's sunset section; confirm it links and adds no competing normative claim |
| Each new chapter/section leads with the `cargo pmcp` workflow before dropping to crate-level API | DOCS-04, DOCS-05 | Ordering/emphasis is a judgement, not a string match | Read each new chapter and each new README section top-down; the first runnable command in each must be a `cargo pmcp` invocation |
| The Tasks era-delta carries a provisionality callout (research § F-2: TASK-01..06 remain `[~]`; only "the wire is final" is unquotable) | DOCS-05 | Provisionality is a prose property | Read the amended `ch12-7-tasks.md` era section; confirm behaviours T-1..T-7 are stated as shipped with source cites, and the wire-finality claim is explicitly marked provisional |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5 s for the per-task leg
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
