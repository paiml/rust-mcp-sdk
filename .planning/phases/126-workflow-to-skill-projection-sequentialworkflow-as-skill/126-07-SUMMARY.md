---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 07
subsystem: testing
tags: [skills, workflow, projection, example, fuzz, doctest, pmat, quality-gate, proptest, canonical-json, nfc]

requires:
  - phase: 126-01
    provides: "`SequentialWorkflow::as_skill()`, `SkillProjection`, the projection module split"
  - phase: 126-02
    provides: "the deterministic renderer and the dual-surface `as_prompt_text() == body()` invariant"
  - phase: 126-03
    provides: "`fuzz/fuzz_targets/fuzz_workflow_projection.rs` and the SC-4 wire tests"
  - phase: 126-04
    provides: "`SkillProjection::build()`'s structured `ProjectionWarning` return (SC-6's one delivery channel)"
  - phase: 126-05
    provides: "the D-04a opt-in prepend and GATE B's `with_workflow_skill_prepend` builder setters"
  - phase: 126-06
    provides: "the D-14 golden, the CHANGELOG contract section and the executing doctests"
provides:
  - "`examples/s56_workflow_skill_projection.rs` — the CLAUDE.md ALWAYS/EXAMPLE artefact, 485 lines, 21 assertions, asserting rather than printing"
  - "`deferred-items.md` — seven deferrals written down rather than assumed, closing REVIEWS fable (e)"
  - "The three structurally ungateable verifications performed with output recorded: the `workflow::sequential` doctest leg, the example run, the nightly fuzz campaign"
  - "A green `RUSTFLAGS=\"\" make quality-gate` across all 18 legs, after the leg-10 defect was fixed rather than deferred"
  - "`lint-plans` restored to green — 9 D-19 verify-command violations repaired in this phase's own plan files"
affects: [127, v2.7-release, pmcp-package, makefile-test-selectors]

actuals:
  tokens: 11000
  tasks: 3
  commits: 6

tech-stack:
  added: ["unicode-normalization (crates/pmcp-package [dev-dependencies] only — already in the resolved graph via olpc-cjson)"]
  patterns:
    - "An example ASSERTS its invariants before printing them (c10's habit), so `cargo run --example` is a verification and not a demo"
    - "A deferral is an artefact with a file:line and an owner, not a sentence in a SUMMARY"
    - "A checkpoint may not ask a reviewer to confirm an artefact no task wrote"

key-files:
  created:
    - examples/s56_workflow_skill_projection.rs
    - .planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/deferred-items.md
  modified:
    - Cargo.toml
    - crates/pmcp-package/tests/attestation_opacity.rs
    - crates/pmcp-package/Cargo.toml
    - crates/pmcp-package/tests/attestation_opacity.proptest-regressions
    - .planning/phases/126-.../126-01-PLAN.md, 126-05-PLAN.md, 126-06-PLAN.md, 126-07-PLAN.md

key-decisions:
  - "The example demonstrates D-04a through `ServerBuilder::with_workflow_skill_prepend` — the builder path a real server copies — not through a hand-constructed `WorkflowPromptHandler` (REVIEWS finding 2)"
  - "The example registers with `.try_skills(...)`, never `.skills(...)`, because the latter panics inside a `Result`-returning `build()` (phase 125 WR-03, still open)"
  - "The `pmcp-package` gate failure was FIXED, not deferred — the human declined the deferral at the Task 3 checkpoint"
  - "The cause of that failure is Canonical JSON NFC normalization in `olpc-cjson`, NOT macOS path normalization; the wrong cause was corrected in the project record so it does not outlive the fix"
  - "The fix asserts the round-trip modulo NFC; refusing non-NFC annotations and restricting the generator to NFC input were both considered and rejected"
  - "The 9 D-19 plan-lint violations were repaired rather than waived — `lint-plans` is `make quality-gate`'s first leg and 7 of the 9 were pipelines that would have reported PASS on a FAILING build"

patterns-established:
  - "Pattern: a `<verify>` that pipes to `tail` must be wrapped in `bash -o pipefail -c '...'`, or the pipeline's exit status is the pager's and a red build reads green"
  - "Pattern: when a property fails on a value the serializer transforms, fix the property's CLAIM — refusing the input or narrowing the generator both destroy the coverage the property existed to give"
  - "Pattern: a persisted proptest regression seed is committed, never deleted to green a gate"

requirements-completed: [SC-1, SC-2, SC-3, SC-4, SC-5, SC-6]

coverage:
  - id: D1
    description: "`examples/s56_workflow_skill_projection.rs` — one runnable file demonstrating the workflow, its projected skill, the registry pass-through, the SC-6 gate warning and the D-04a opt-in prepend, asserting each"
    requirement: "SC-1"
    verification:
      - kind: e2e
        ref: "RUSTFLAGS=\"\" cargo run --example s56_workflow_skill_projection --features skills,full -> exit 0, `All assertions passed.`"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo build -p pmcp --features \"skills,full\" --example s56_workflow_skill_projection -> exit 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "The `[[example]]` stanza registers `s56` under the correct number with `required-features = [\"skills\", \"full\"]`, touching no `[features]` table"
    requirement: "SC-2"
    verification:
      - kind: unit
        ref: "tests/v1_severability_tripwire.rs (derives default/full/full-v2 from Cargo.toml at test time) -> 18 passed"
        status: pass
      - kind: other
        ref: "grep -c 's45_workflow_skill_projection' Cargo.toml examples/ -r -> 0 (D-16a number correction applied)"
        status: pass
    human_judgment: false
  - id: D3
    description: "The `workflow::sequential` doctest leg — structurally unreachable by any `make` leg — run by hand with its count recorded"
    requirement: "SC-3"
    verification:
      - kind: unit
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features \"skills,full\" --doc sequential -- --test-threads=1 -> `test result: ok. 7 passed; 0 failed`, including `SequentialWorkflow::as_skill (line 284)`"
        status: pass
    human_judgment: false
  - id: D4
    description: "The nightly fuzz campaign on `fuzz_workflow_projection` — CLAUDE.md's ALWAYS/FUZZ requirement, discharged with a real run count and not with `make test-fuzz`"
    requirement: "SC-2"
    verification:
      - kind: e2e
        ref: "cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30 -> `#200000 DONE cov: 706 ft: 1385 corp: 261/17Kb`, `Done 200000 runs in 20 second(s)`, exit 0, 0 crash artifacts"
        status: pass
    human_judgment: false
  - id: D5
    description: "PMAT cognitive-complexity gate at its zero-violation baseline across the whole phase's added surface"
    requirement: "SC-3"
    verification:
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> `Quality Gate: PASSED`, `Total violations: 0`, exit 0"
        status: pass
    human_judgment: false
  - id: D6
    description: "`deferred-items.md` — WR-03 with its file:line, the six CONTEXT `<deferred>` items plus the conditional seventh, the doctest-leg gap scoped to all three dark legs, GATE B recorded CLOSED, the release-time version bump, and the standing no-git-hooks fact"
    requirement: "SC-6"
    verification:
      - kind: other
        ref: "git ls-files -- .../deferred-items.md | grep -c . -> 1 (tracked); grep -c 'builder.rs:1501' -> 1"
        status: pass
    human_judgment: false
  - id: D7
    description: "`make quality-gate` green end to end after the leg-10 `pmcp-package` defect was fixed rather than deferred"
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate -> exit 0, `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`; `✓ pmcp-package tests passed (337 tests)`; `--features full --lib` 2032 passed"
        status: pass
      - kind: unit
        ref: "cargo test --manifest-path crates/pmcp-package/Cargo.toml --test attestation_opacity -> `test result: ok. 6 passed; 0 failed` (re-run post-fix in this plan)"
        status: pass
    human_judgment: false
  - id: D8
    description: "Human sign-off on all six ROADMAP success criteria, the cross-AI review resolutions, and the deferral list"
    verification:
      - kind: manual_procedural
        ref: "126-07 Task 3 checkpoint:human-verify — approved 2026-09-04 with three explicit answers (SC-1..SC-6 approved; deferrals approved; the pmcp-package defect NOT accepted as a deferral)"
        status: pass
    human_judgment: true
    rationale: "Phase closure against ROADMAP success criteria is a judgment about whether recorded evidence is convincing, not a predicate any command can evaluate. It is the checkpoint the plan was built around."

duration: 1h 32m
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 07: Runnable Demonstration and Phase Closure Summary

**`examples/s56_workflow_skill_projection.rs` asserts the whole phase in 485 lines and 21 assertions; the three verifications no gate leg can reach were run by hand with output recorded; and the one red gate leg was fixed — with its cause corrected — rather than deferred.**

## Performance

- **Duration:** 1h 32m
- **Started:** 2026-09-04T18:58:09Z (immediately after 126-06's metadata commit `2f49cbcd`)
- **Completed:** 2026-09-04T20:30:00Z
- **Tasks:** 3 (1 auto, 1 auto with a Rule-3 deviation and a scope-boundary log, 1 `checkpoint:human-verify`)
- **Files modified:** 10

## Accomplishments

- **The ALWAYS/EXAMPLE requirement is discharged with an artefact that can go RED.** `examples/s56_workflow_skill_projection.rs` (485 lines, 21 `assert*` calls) follows `c10_client_skills`'s assert-then-print habit rather than `s44_server_skills`'s print-only one. `cargo run --example s56_workflow_skill_projection --features skills,full` exits 0 and prints `All assertions passed.` after proving SC-1, SC-2, SC-4, SC-5, SC-6 and D-04a in one process.
- **The D-04a demonstration goes through the path a real server copies.** REVIEWS finding 2 (fable HIGH / gemini Finding 1) objected that demonstrating the opt-in only through a hand-constructed `WorkflowPromptHandler` teaches a path nobody uses. GATE B answered `add-builder-path`, so the example builds the ON case through `ServerBuilder::with_workflow_skill_prepend(true)` and asserts message [0] is byte-equal to the served 1237-byte skill body, against a default run whose message [0] is the ordinary user-intent string.
- **Seven deferrals are written down rather than assumed.** `deferred-items.md` (312 lines) closes REVIEWS fable (e): Task 3's checkpoint asked a reviewer to confirm WR-03 "is restated in `deferred-items.md`" while, in the original plan set, no task in any of the seven plans wrote that file — a confirmation that could only be waved through.
- **The three structurally ungateable verifications were performed, not claimed.** `make quality-gate` builds examples but never runs them, does not run PMAT, and cannot reach the `workflow::sequential` doctest. Each was run by hand and its actual output is recorded below (T-126-10).
- **The one red gate leg was FIXED and its cause CORRECTED.** The human declined the deferral. `dc0eb3e7` repaired the `pmcp-package` property, and the project record now names the real cause — Canonical JSON NFC normalization — instead of the plausible-but-wrong "macOS path normalization" first written down.

## Task Commits

1. **Task 1: the `s56` example and its `[[example]]` stanza** — `cf7bea62` (docs)
2. **Task 2a: `deferred-items.md`, written before the checkpoint reads it** — `5b7597a8` (docs)
3. **Task 2b: [Rule 3 deviation] repair 9 D-19 violations in this phase's plan verify blocks** — `46a8458e` (fix)
4. **Task 2c: [scope-boundary log] record the `pmcp-package-gate` failure** — `043bd5eb` (docs)
5. **Post-checkpoint: fix the `pmcp-package` property (by the orchestrator)** — `dc0eb3e7` (fix)
6. **Post-checkpoint: re-record that entry as RESOLVED with the corrected cause** — `2918a249` (docs)

## Human Verification (Task 3) — the three answers

The `checkpoint:human-verify` gate was reached after Task 2 and answered on 2026-09-04.

**1. The six success criteria SC-1 … SC-6: APPROVED**, on the evidence recorded across `126-01`…`126-06` and the verifications below.

| Criterion | Evidence cited at approval |
|---|---|
| **SC-1** slugification owned by the projection | The example prints `refund_flow` → `refund-flow`; `tests/skills_routing.rs` shows the final URI segment equals the frontmatter `name` on the wire |
| **SC-2** determinism | The example asserts a second independent derivation is byte-equal; the property test does 100 fresh re-derivations including template bindings; `tests/golden/workflow_skill_projection.md` pins the bytes; the fuzz target asserts byte-equal determinism across two renders per input |
| **SC-3** full coverage | The `skills::projection` unit suite grew 26 → 53 → 74 with each workflow fact carrying its own named assertion; the two D-11 exclusions are pinned absent over a workflow that sets both to non-default |
| **SC-4** registry pass-through on the wire | `tests/skills_routing.rs` (23 passed) shows a `skills/list` entry with two-key verbatim frontmatter and a `sha256:` + 64-lowercase-hex manifest whose `size` matches the served body, and `resources/read` returning those exact bytes over a loopback server. The example reproduces it in-process: digest `sha256:81206836…04fa`, `entry size: 1237 (served 1237 bytes)` |
| **SC-5** dual-surface invariant | `as_prompt_text() == body()` with anti-vacuity guards; the rendered Procedure's tool-name set equals `wf.steps().filter_map(WorkflowStep::tool)` in both directions; the example asserts it a third time |
| **SC-6** projection-time gate warning | The example prints exactly one `GuidanceOnSideEffectingStep` naming `issue_refund` / `payments_refund`; a distinct unverifiable note appears when annotations are missing. `build()`'s structured return is SC-6's single delivery channel and both rustdocs say so |

**2. The deferrals in `deferred-items.md`: APPROVED** — WR-03 with its `builder.rs:1501` reference and the reasoning that phase 126 neither fixes nor worsens it; all six CONTEXT `<deferred>` items plus the conditional seventh; the doctest-leg gap scoped to all three dark legs rather than the one gemini Finding 5 named; GATE B recorded as CLOSED with both setters named; the release-time `Cargo.toml` 2.19.3 → 2.20.0 bump; and the standing "no git hooks in this checkout" fact.

**3. The red `pmcp-package-gate`: NOT accepted as a deferral.** The human directed that the defect be fixed before the phase closes. See *The `pmcp-package` fix* below.

## Verification Results — the four manual verifications

Each was re-run in this plan, after `dc0eb3e7`, through absolute binary paths (`/Users/guy/.cargo/bin/...`) because `rtk` proxying is a recorded source of truncated and corrupted cargo output.

### 1. PMAT cognitive complexity — CI-only by Phase 75 D-07, so `make quality-gate` never runs it

```
$ pmat quality-gate --fail-on-violation --checks complexity
🔍 Running quality gate checks...
📋 Checks to run:
  ✓ Complexity analysis
Warning: AST analysis failed for ./deploy/cloudflare/src/lib.rs, using heuristic fallback
  💾 Persisted 0 violations to .pmat/context.db
Quality Gate: PASSED
Total violations: 0
✅ Quality gate PASSED
```

Exit 0. Zero `src/` violations, matching the measured baseline — so nothing this phase added (`render_step` and `gate_check` were the two named risks) crossed cog 25 or needed an `#[allow]`. The `deploy/cloudflare` warning is pre-existing and outside `src/`.

### 2. The `workflow::sequential` doctest leg — reachable by no `make` leg

```
$ RUSTFLAGS="" cargo test -p pmcp --features "skills,full" --doc sequential -- --test-threads=1
running 7 tests
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::argument (line 82) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::as_skill (line 284) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::instruction (line 160) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::new (line 56) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::step (line 145) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::typed_argument (line 114) ... ok
test src/server/workflow/sequential.rs - ...::SequentialWorkflow::with_task_support (line 183) ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 552 filtered out; finished in 2.18s
```

Exit 0, **7 passed**, `as_skill (line 284)` among them. `make test-skills` selector 2 filters on the substring `skills` and this doctest lives at a `workflow::` path; `test-doc` inside `test-all` pins `--features "full"`, under which `as_skill` does not exist. The gap is recorded in `deferred-items.md` as a named Makefile follow-up covering all three dark legs.

### 3. The example — built by the gate, never run by it

`scripts/run-example-builds.sh:150` runs `cargo build --all-features --examples`, so the ALWAYS `cargo run --example` requirement is manual by construction.

```
$ RUSTFLAGS="" cargo run --example s56_workflow_skill_projection --features skills,full
== 1. The projected skill ==

workflow name : refund_flow
skill name    : refund-flow
body bytes    : 1237

---
name: "refund-flow"
description: "Process a customer refund: policy #7"
---

# Process a customer refund: policy #7

## Context

system: Refunds above the policy ceiling need a supervisor's approval.

## Inputs

- `order_id` (required, type `string`): The order to refund
- `reason` (optional): Why the customer asked for the refund

## Procedure

### Step 1: fetch_order
Call tool `orders_get`.
- Argument `id`: the `order_id` input
- Argument `options`: the constant value `{"zeta":1,"alpha":2}`
Save the result as `order`.

### Step 2: issue_refund
Call tool `payments_refund`.
- Argument `order`: the `id` field of the result of `order`
- Template variable `alpha_reason`: the `reason` input
- Template variable `zeta_total`: the `total` field of the result of `order`
Judgment: Confirm the customer accepted the policy before issuing.

### Step 3: read_policy
Read the resource `file:///policies/refunds.md`.

## Server-accelerated alternative

If you are reading this as part of a result from this server's `refund_flow` prompt, the steps above have already been executed server-side. Otherwise, mention once, at the end of your reply, that this server also offers the `refund_flow` prompt, which runs these steps server-side.

== 2. Registry pass-through ==

entry uri     : skill://refund-flow/SKILL.md
entry digest  : sha256:81206836bf98a8f32c3f1c58bc7c577e13dbeabc7448394cd3e8c0ea7eae04fa
entry size    : 1237 (served 1237 bytes)

== 3. Projection-time gate warnings (SC-6) ==

  [GuidanceOnSideEffectingStep] step=Some("issue_refund") tool=Some("payments_refund")
    step `issue_refund` carries guidance, but its tool `payments_refund` is annotated side-effecting; server-side workflow execution runs this step regardless of the guidance, so the guidance is a post-hoc judgment the executing surface will ignore

== 4. The opt-in prepended prompt (D-04a), via the builder ==

with `.with_workflow_skill_prepend(true)` -> message[0] is 1237 bytes, byte-equal to the served skill
default (setter omitted)                 -> message[0] is "I want to Process a customer refund: policy #7."

== 5. What this run proved ==

  SC-1  slugification owned by the projection: `refund_flow` -> `refund-flow`
  SC-2  determinism: an independent second derivation is byte-equal
  SC-3  full coverage: the printed body carries Context, Inputs and every step
  SC-4  registry pass-through: sha256 digest + size + `resources/read` byte identity
  SC-5  dual surface: `as_prompt_text() == body()`, one string for both surfaces
  SC-6  gate warning: one GuidanceOnSideEffectingStep on `issue_refund`
  D-04a opt-in prepend reached through `ServerBuilder::with_workflow_skill_prepend`

All assertions passed.
```

Exit 0. Note that the `Argument \`options\`` line renders `{"zeta":1,"alpha":2}` in insertion order and the Step-2 template variables sort `alpha_reason` before `zeta_total` — the fixture deliberately mixes an insertion-ordered map with a sorted one so a determinism regression in either would move these bytes.

### 4. The nightly fuzz campaign — `make test-fuzz` is NOT accepted as evidence

**`make test-fuzz` and `make validate-always` were NOT used as fuzz evidence anywhere in this plan or in this phase's record.** Both swallow the stable-toolchain failure (`error: the option 'Z' is only accepted on the nightly compiler`) with `|| echo` and exit 0 having fuzzed nothing.

```
$ cd fuzz && cargo +nightly fuzz run fuzz_workflow_projection -- -runs=200000 -max_total_time=30
#189480	REDUCE cov: 706 ft: 1385 corp: 261/17Kb lim: 2488 exec/s: 9972 rss: 522Mb L: 46/803 MS: 1 EraseBytes-
#195941	REDUCE cov: 706 ft: 1385 corp: 261/17Kb lim: 2543 exec/s: 9797 rss: 523Mb L: 156/803 MS: 1 EraseBytes-
#197767	REDUCE cov: 706 ft: 1385 corp: 261/17Kb lim: 2554 exec/s: 9888 rss: 523Mb L: 73/803 MS: 1 EraseBytes-
#198098	REDUCE cov: 706 ft: 1385 corp: 261/17Kb lim: 2554 exec/s: 9904 rss: 523Mb L: 554/803 MS: 1 EraseBytes-
#200000	DONE   cov: 706 ft: 1385 corp: 261/17Kb lim: 2565 exec/s: 10000 rss: 523Mb
###### Recommended dictionary. ######
"\000\000\000\000\000\000\006\326" # Uses: 14874
###### End of recommended dictionary. ######
Done 200000 runs in 20 second(s)
```

Exit 0. **200,000 executed runs**, coverage 706 features 1385, corpus 261 inputs, and `find fuzz/artifacts/fuzz_workflow_projection/ -type f | wc -l` returns **0** — zero crash artifacts. Coverage rose from plan 126-03's `cov: 697 ft: 1367 corp: 251` on the same target, i.e. the campaign found new edges and still crashed on none.

### 5. The full gate

`RUSTFLAGS="" make quality-gate` → **exit 0**, `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`. The `RUSTFLAGS=""` prefix is not decoration: CI exports `RUSTFLAGS` and local shells do not, and that mismatch is a recorded class of CI failure a bare local gate run passes through.

Inside it: `✓ pmcp-package tests passed (337 tests)`, `✓ pmcp-package fmt/clippy/test/example OK`, `no-crypto-check PASSED: pmcp-package resolved graph is allowlisted`, and `--features full --lib` **2032 passed**. `make test-skills` reports **173 / 16 / 26 / 23** across its four selectors with no zero-count guard tripped.

## The `pmcp-package` fix (`dc0eb3e7`) — and the cause that was wrong

Task 2 found `make quality-gate` red at leg 10 of 18 and, per the executor SCOPE BOUNDARY rule, logged it as an out-of-scope pre-existing failure with attribution evidence. **The human declined that deferral and directed a fix.** It was made by the orchestrator, not by a phase-126 plan.

**The predecessor's scope call was right. Its stated CAUSE was wrong.** It reported "macOS path normalization" — plausible, because the failing pair `U+8C48` / `U+F900` is exactly what APFS normalizes on a path, and because the two render identically so the failure read `left: "豈", right: "豈"` with no visible difference. But **the annotation value never reaches a path**. Nothing filesystem-shaped is involved.

The real cause, confirmed by reading the dependency source:

1. `pmcp-package` writes its OCI manifest as **Canonical JSON**, via `olpc-cjson`.
2. The Canonical JSON specification requires strings to be **NFC-normalized**, and `olpc-cjson`'s `write_string_fragment` applies `str::nfc` to every fragment it writes — **before** the digest is taken over the blob.
3. U+F900 (CJK COMPATIBILITY IDEOGRAPH-F900) has a **singleton** canonical decomposition to U+8C48, so NFC maps one to the other.

Therefore `adversarial_annotation_values_come_back_as_inert_data`'s assertion that annotations come back **verbatim** was **false for any non-NFC input**, and no change to `pmcp-package` could have made it true. The defect was in the property's claim, not in the pack/unpack path.

**The fix asserts the round-trip modulo NFC.** Two alternatives were considered and rejected:

| Alternative | Why rejected |
|---|---|
| Make `pack_server` **REFUSE** non-NFC annotations | Exactly the "quietly widen into refusing legitimate non-ASCII issuers" failure the property's own doc warns against — an issuer legitimately written in NFD would start being rejected |
| Restrict the **generator** to NFC input | Makes the test agree with the implementation by construction and stops covering the transformation at all |

**Normalization does not weaken what the property proves.** The claim is that annotation values are inert DATA reaching no filesystem API. A normalization the serializer applies uniformly before any byte is written touches neither half of that claim.

**Non-vacuity was measured, not assumed:** reverting the `nfc` helper to the identity function reproduces the original failure on the persisted seed exactly.

Two supporting changes rode along: `crates/pmcp-package/tests/attestation_opacity.proptest-regressions` is now **git-tracked**, so the case is deterministic for every checkout rather than only the machine that generated it; and `unicode-normalization` joined `[dev-dependencies]`, adding **no** crate to the resolved graph `make no-crypto-check` allowlists, because `olpc-cjson` — a direct runtime dependency — already pulls it in to do this same normalization.

Re-run in this plan after the fix: `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test attestation_opacity` → `test result: ok. 6 passed; 0 failed`, exit 0.

`deferred-items.md`'s entry was rewritten (`2918a249`) to record the item as RESOLVED, to keep the three attribution measurements (they are what makes the scope call defensible), and to correct the cause in place so the wrong one does not outlive the fix.

## Files Created/Modified

- `examples/s56_workflow_skill_projection.rs` — **created**, 485 lines, 21 assertions. Module docs in the `s44` shape: the run line, an explicit statement that `s44` remains the hand-authored-skills example and this is its projection sibling, and a numbered list of what it demonstrates. Registers via `.try_skills(...)` with a comment saying why. Five demonstrations, each asserting before printing.
- `Cargo.toml` — **+7 lines**: the `[[example]]` stanza at lines 715–718, `name = "s56_workflow_skill_projection"`, `required-features = ["skills", "full"]`, placed immediately after `c10_client_skills` to keep the skills examples contiguous. No `[features]` table touched.
- `.planning/phases/126-.../deferred-items.md` — **created**, 312 lines. Seven headings: WR-03, the six CONTEXT items, the three-legged doctest gap, GATE B (closed), the release-time version bump, the no-git-hooks fact, and the `pmcp-package` entry now recording RESOLVED.
- `crates/pmcp-package/tests/attestation_opacity.rs` — **±43 lines**: the NFC-modulo assertion plus the `nfc` helper and its rationale.
- `crates/pmcp-package/Cargo.toml` — **+9 lines**: `unicode-normalization` under `[dev-dependencies]`.
- `crates/pmcp-package/tests/attestation_opacity.proptest-regressions` — **+7 lines**, now tracked.
- `126-01/05/06/07-PLAN.md` — **±20 lines**: the nine D-19 verify-command repairs.

## Decisions Made

- **`s56`, not `s45`.** D-16a's number correction: `Cargo.toml:713-717` already registers `s45_tool_as_task_lifecycle`. `grep -c 's45_workflow_skill_projection' Cargo.toml examples/ -r` returns 0.
- **Assert, then print.** `c10_client_skills`'s habit, explicitly not `s44_server_skills`'s print-only one. A print-only example is a demo; an asserting one is a verification that can go red.
- **`.try_skills(...)`, never `.skills(...)`** in the example (threat T-126-19). A reader copying the example copies the form that cannot abort their process while WR-03 is open.
- **The builder path for D-04a**, per GATE B and REVIEWS finding 2, with a comment noting the setter must precede `prompt_workflow` because the flag is read at registration time.
- **Fix the `pmcp-package` property rather than defer it**, on the human's direction — and record the corrected cause rather than quietly replacing the entry.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Repaired 9 D-19 violations in this phase's own plan verify blocks**

- **Found during:** Task 2, item 1 (`RUSTFLAGS="" make quality-gate`)
- **Issue:** The gate's **first** leg, `lint-plans`, was RED at HEAD with 9 violations across `126-01/05/06/07-PLAN.md`. All were introduced by plan authoring in `5b26a981` (2026-09-03), none by any code commit. Seven were RULE 1: `cargo build … | tail -5` with no `pipefail`, so the pipeline's exit status was the pager's and **a FAILING build would have reported PASS**. One was the same shape in a 126-06 acceptance criterion. One was RULE 3: `|| true` on the SATD check, which erases its status entirely.
- **Fix:** The seven pipelines wrapped as `bash -o pipefail -c '...'`; the acceptance criterion rewritten capture-then-assert so there is no pipeline at all; the `|| true` replaced with a negated grep that ALSO asserts both directories exist, closing the missing-path vacuity a bare `! grep` would have introduced.
- **Files modified:** `126-01-PLAN.md`, `126-05-PLAN.md`, `126-06-PLAN.md`, `126-07-PLAN.md`
- **Verification:** `./scripts/lint-plan-verify-commands.sh` → exit 0, 12 files, 485 lines.
- **Committed in:** `46a8458e`
- **Note:** this is the same shape phase 125 recorded — the lint runs only inside `make quality-gate`, and no plan of the phase had run it. It is in scope under Rule 3 because it blocked the plan's own required verification at its first leg.

**2. [Scope boundary → escalated to a fix on human direction] The `pmcp-package-gate` leg**

- **Found during:** Task 2, item 1
- **Initially:** logged to `deferred-items.md` per the SCOPE BOUNDARY rule with three attribution measurements, and legs 11–18 run individually so the rest of the gate was still proven (`043bd5eb`).
- **Then:** the human declined the deferral at the Task 3 checkpoint. Fixed in `dc0eb3e7` by the orchestrator; the record corrected in `2918a249`. Full account above.

---

**Total deviations:** 2 (1 Rule-3 auto-fix, 1 scope-boundary log later escalated to a fix on explicit human direction)
**Impact on plan:** The Rule-3 repair was a precondition for running the plan's own gate and touched only planning artefacts. The `pmcp-package` fix is outside phase 126's code by construction (standalone crate, no `pmcp` dependency) and was made on direct human instruction. No scope creep into the projection surface: `git diff 2f49cbcd..HEAD -- src/` is empty.

## Issues Encountered

- **The recorded evidence for the four manual verifications lived only in the previous executor's context.** The checkpoint return carried the gate readings but not the four commands' outputs, and no artefact on disk held them. Rather than paraphrase numbers this plan could not read, all four were **re-run** through absolute binary paths and their real output pasted above. That is strictly better evidence than the originals: it is post-`dc0eb3e7`, so it measures the tree as it actually closes.
- **`rtk` proxying corrupts cargo and git output** (a recorded project fact — swallowed doctest names, a `git diff` truncated 48,639 → 15,975 bytes, `grep` missing `test result:` lines that were present). Every number in this SUMMARY was read through `/Users/guy/.cargo/bin/cargo` or `/Users/guy/.cargo/bin/pmat`.
- **No git hooks are installed in this checkout** — `.git/hooks/` holds only `*.sample` files, despite CLAUDE.md describing a blocking pre-commit gate. "It committed" is therefore evidence of nothing here; every gate claim above cites the gate's own recorded output. Recorded as a standing fact in `deferred-items.md`.

## Known Stubs

None. The example wires real objects end to end — a real `SequentialWorkflow`, a real `Skills` registry read back through its handler, a real `SkillProjection::build()` returning real warnings, and two real `PromptHandler::handle` calls. Nothing in it is placeholder data.

`ProjectionWarningKind::SlugFallback` remains declared-but-unemittable-from-`build()`, unchanged from 126-06 and characterised there as API vocabulary rather than a stub: `build()` rejects the input `as_skill()` substitutes for, the variant's own rustdoc says so, and it exists so a future lenient mode is additive rather than breaking.

## Threat Flags

None. This plan added an example, a planning artefact and a test-property correction. It introduces no network endpoint, no auth path, no file-access pattern and no schema change at a trust boundary. Its manifest edits declare no new runtime dependency: the `[[example]]` stanza declares none at all, and `unicode-normalization` is a `[dev-dependencies]` entry already present in the resolved graph via `olpc-cjson` (`no-crypto-check` re-verified green).

Threats owned by this plan and their outcomes:

| Threat | Outcome |
|---|---|
| T-126-10 — coverage claimed but never executed | Mitigated. All four manual verifications have real pasted output; the fuzz run reports 200,000 executed runs against a `<fails_when>` floor of 1,000 |
| T-126-18 — a green command that measured nothing | Mitigated. No verify command in this plan used `--features "full"` for a skills test or `cargo nextest -E 'test(/.../)'`; the doctest reports 7 passed and the tripwire 18 passed, both nonzero |
| T-126-19 — `s56` teaching an unsafe registration pattern | Mitigated. The example uses `.try_skills(...)` and says why |
| T-126-20 — silent scope creep via the deferred list | Mitigated. All six CONTEXT items plus the conditional seventh are enumerated in `deferred-items.md`; `git diff 2f49cbcd..HEAD -- src/` is empty |
| T-126-26 — a confirmation asking a reviewer to check an artefact nobody wrote | Mitigated. `deferred-items.md` was written in `5b7597a8`, **before** the checkpoint, and two automated verifies asserted it exists, is tracked, and names `builder.rs:1501` |
| T-126-SC — supply chain | The plan installed no package. The one dependency added by the post-checkpoint fix is dev-only and adds no crate to the resolved graph |

## User Setup Required

None — no `user_setup` block in this plan's frontmatter and no external service configuration required.

## Next Phase Readiness

**Phase 126 is complete as executed.** All seven plans have SUMMARYs; all six ROADMAP success criteria are human-approved against recorded evidence; `make quality-gate` is green end to end.

Carried forward, all written down in `deferred-items.md` rather than left to memory:

1. **Phase 125 WR-03 remains OPEN** — the build-time `panic!` inside a `Result`-returning `build()`, canonical reference `src/server/builder.rs:1501`, at HEAD drifted to `:1576` because plan 126-05 inserted the GATE B field above it. Phase 126 neither fixes nor worsens it.
2. **The three dark doctest legs** — `workflow::sequential`, `skills::` (the one leg that IS gated) and `skill_prepend` — need a Makefile selector change carrying the same zero-count guard the existing selectors carry. Scope is all three, not the one gemini Finding 5 named.
3. **A releaser must bump `Cargo.toml` 2.19.3 → 2.20.0 before tagging.** `CHANGELOG.md` already carries `## [2.20.0] - Unreleased`; `release.yml`'s `create-release` job exits 1 on a version mismatch, *after* the tag is pushed.
4. **The six CONTEXT deferrals** plus the conditional seventh, each with an owner.

**No blockers.** Phase completion in ROADMAP.md is deliberately NOT marked here — that is the orchestrator's step after the verifier runs. Only the `126-07-PLAN.md` plan line was ticked.

---
*Phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill*
*Completed: 2026-09-04*

## Self-Check: PASSED

All three artefacts present on disk (`examples/s56_workflow_skill_projection.rs`, `deferred-items.md`, `126-07-SUMMARY.md`) and all seven commits resolve in `git log --all`: `cf7bea62`, `5b7597a8`, `46a8458e`, `043bd5eb`, `dc0eb3e7`, `2918a249`, `34c80048`.
