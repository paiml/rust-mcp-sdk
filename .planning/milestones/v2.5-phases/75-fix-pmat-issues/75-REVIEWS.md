---
phase: 75
reviewers: [gemini, codex]
reviewed_at: 2026-04-23T02:37:00Z
plans_reviewed: [75-00-PLAN.md, 75-01-PLAN.md, 75-02-PLAN.md, 75-03-PLAN.md, 75-04-PLAN.md, 75-05-PLAN.md]
context_provided: [75-CONTEXT.md, 75-RESEARCH.md, 75-VALIDATION.md, 75-PATTERNS.md, ROADMAP §Phase 75, PROJECT.md head]
---

# Cross-AI Plan Review — Phase 75: Fix PMAT issues

## Gemini Review

This is a high-quality, rigorous plan set that prioritizes **Jidoka (Stop the Line)** principles by establishing safety nets before performing surgery on the codebase. It correctly identifies the "Quality Gate" as the primary objective and builds the necessary infrastructure to keep it green.

### 1. Summary
The plan is excellent. It treats code quality remediation as a first-class engineering task rather than a "cleanup" chore. By front-loading Wave 0 with semantic regression baselines and insta snapshots, it mitigates the high risk of breaking procedural macros and AST-walking logic. The decision to pin the PMAT version and the specific `// Why:` justification format shows strong architectural discipline. The wave structure is logical, moving from the critical core (`src/`) to CLI extensions and finally the hardest AST refactors.

### 2. Strengths
*   **Regression-First Approach:** Wave 0's requirement for snapshots in `pmcp-macros` and semantic tests in `pmcp-code-mode` is a pro move. Refactoring a 123-cog AST walker without a semantic baseline is a recipe for silent bugs.
*   **Pragmatic Compliance (D-02/D-03):** The "Hard ceiling at 50, target 25" rule with mandatory justification avoids the "gaming the metric" trap while acknowledging that some protocol dispatch logic is inherently complex.
*   **Infrastructure Pinning:** Pinning PMAT to `3.15.0` in CI (Pitfall 1) is critical to prevent "Red CI" on unrelated PRs due to toolchain drift.
*   **Atomic Traceability:** The requirement for `pmat-complexity: NN (was MM)` in commit messages provides a clear audit trail for the badge restoration.

### 3. Concerns

#### **HIGH SEVERITY: Proc-Macro Testing Constraints (Plan 00, Task 1)**
The plan suggests a `pub mod __test_internal` re-export in `pmcp-macros/src/lib.rs`.
*   **The Issue:** If `pmcp-macros` is defined with `proc-macro = true` in `Cargo.toml` (which it is), the Rust compiler **prohibits** exporting anything other than procedural macros from that crate. You cannot have a `pub mod` or `pub use` that exports regular functions/structs for an integration test to call.
*   **Risk:** Task 1 will fail during implementation when the executor realizes it can't call the expansion functions from an external `tests/` file.
*   **Reference:** [Rust Book - Procedural Macros](https://doc.rust-lang.org/reference/procedural-macros.html).

#### **MEDIUM SEVERITY: "Gaming" the D-03 Ceiling (Wave 3)**
The refactor of `evaluate_with_scope` (cog 123) and `evaluate_array_method_with_scope` (cog 117).
*   **The Issue:** P6 (decomposition) will definitely drop the complexity of the *main* function, but if the logic is just moved into 15 helpers that each have cog 20-25, the *overall* cognitive load of the module hasn't necessarily decreased—it's just been partitioned.
*   **Risk:** The plan satisfies the tool (PMAT) but might result in "helper soup" where the execution flow is harder to follow than the original monolithic match.

#### **MEDIUM SEVERITY: SATD Triage Latency (Plan 04, Task 4-C)**
*   **The Issue:** 33 SATDs is a high volume for a single task. If even 10 of these are "real follow-up work" (Disposition B), the overhead of using `gh issue create` 10 times in a single sub-agent run might hit rate limits or timeout the executor.
*   **Risk:** Task 4-C is a "fat" task that might need more than one sub-agent turn to complete the audit + issue filing + code edits.

#### **LOW SEVERITY: The D-09 Spike Timing**
*   **The Issue:** The entire implementation path for examples/ (21 violations) depends on whether `--include` works.
*   **Risk:** If the spike is performed in Wave 0 but the Wave 4 plan is already "locked" with the assumption of path (b), the executor will have to deviate significantly.

### 4. Suggestions
*   **Fix Macro Testing:** Instead of trying to export internal fns from the proc-macro crate, use the **`trybuild` + `insta` combination on the *expanded* output**. Create a test crate (or use existing `tests/`) that applies the macro to a struct, then use a specialized test to snapshot the result of `cargo expand`. Alternatively, move the logic to a non-proc-macro "support" crate (like `pmcp-macros-support` which you already created for Phase 71) and test it there.
*   **Group SATD Issues:** To mitigate the "fat task" risk, change Task 4-C's policy to file **one GitHub issue per hotspot directory** (e.g., "Remediate SATD in pmcp-code-mode") and list the specific TODOs as checkboxes in the issue body. This reduces 33 `gh` calls to ~5.
*   **Explicitly check helper complexity:** Add a requirement to Wave 3 that the *newly created* `evaluate_<variant>` helpers must themselves be ≤ 25 cog. This prevents the "helper soup" problem and ensures the refactor is genuine.
*   **PMAT config check:** Ensure the `.pmat/project.toml` is actually checked into the repo. If it's in `.gitignore`, the CI gate will behave differently than local.

### 5. Risk Assessment: MEDIUM
The overall risk is **Medium**. While the plans are thorough, the technical challenge of refactoring the AST evaluator without breaking the language semantics is high.
*   **Mitigation:** The "Wave 0" semantic baseline is the most important part of the entire phase. If that baseline is weak, the refactor risk becomes **High**.
*   **Success Probability:** High. Following these plans will almost certainly flip the badge to green, as they methodically target every violation counted by the gate.

**Blunt Verdict:** The plan is solid, but the `pmcp-macros` testing strategy is physically impossible in standard Rust. Fix the testing path for macros (move logic to support crate or use `cargo expand` snapshots) before starting Wave 1. Otherwise, you are clear to proceed.

---

## Codex Review

## Summary

This is a strong research packet wrapped around a shaky execution plan. The hotspot inventory is much better than average, and the refactor patterns are mostly sane, but as written I do **not** trust this plan to flip the README badge reliably. The biggest problem is semantic drift between "the CI gate we want" and "the badge command that actually drives README state." The second biggest problem is that the whole plan depends on PMAT honoring `#[allow(clippy::cognitive_complexity)]`, and that is treated as assumed truth instead of a Wave 0 kill-shot check.

## Strengths

- The research is real, not hand-wavy. The plan names concrete files, functions, cog scores, and verification commands.
- Wave decomposition by hotspot family is mostly correct. `src/` + `pmcp-macros/`, then `cargo-pmcp/`, then `pmcp-code-mode/` is a reasonable blast-radius order.
- D-02 and D-03 are good constraints. Requiring `// Why:` and banning "just slap an allow on 80+ cog monsters" is the right discipline.
- Wave 0 is pointed at the right kinds of baselines: macro regression detection, evaluator semantic regression, PMAT version pinning.
- The `pmcp-code-mode` P6 strategy is directionally right. Dispatch decomposition is the only realistic way to cut 123/117 without rewriting the whole crate.
- Wave 5's "fail-closed" proof is the right instinct. Most plans stop at "add CI step" and never verify it actually blocks.

## Concerns

- **HIGH — 75-04 Task 4-D / 75-05 Task 5-01:** The plan still does not align the **badge** with the **new gate semantics**. `quality-badges.yml` currently derives README state from `pmat quality-gate --fail-on-violation` on the full gate. Wave 5 only adds complexity-only enforcement to `ci.yml`. If duplicates/SATD/sections still fail, CI can go green while the badge stays red. That misses the stated goal.
- **HIGH — 75-00 / all later waves:** The plan assumes PMAT suppresses violations when `#[allow(clippy::cognitive_complexity)]` is present. That is load-bearing across Waves 1-4. I see no explicit Wave 0 task proving that assumption with a tiny fixture. If PMAT ignores the clippy allow, half the plan is dead on arrival.
- **HIGH — 75-RESEARCH baseline math / 75-04 Task 4-B:** Your counts are inconsistent. You cite `94 total`, `73 in-scope`, `21 examples`, and later `21 examples + 3 fuzz + 73 in-scope`. That is `97`, not `94`. Until the inventory is reconciled, every wave delta and exit criterion is suspect.
- **HIGH — 75-05 Task 5-02:** The deliberate regression PR puts the bad function in `tests/regression-pr-75-05.rs`. If Wave 0 selects `--include 'src/**,...'` or config-based exclusions, the gate will not even look at that file. Your fail-closed proof can false-pass.
- **HIGH — 75-04 Task 4-B:** If the D-09 spike says include filtering does **not** work, Wave 4 explodes. Two examples are already above 50, and fuzz has a 122-cog function. That is not "bulk add allows." That is another real refactor wave. The plan treats this as a branch inside one task; it is too small.
- **MEDIUM — 75-00 Task 1:** The proc-macro snapshot strategy is brittle. Test-only re-exports inside a proc-macro crate are often awkward, and the fallback to `cargo expand` quietly adds a toolchain dependency you did not budget for. This is likely to waste time before any PMAT debt is burned down.
- **MEDIUM — 75-00 Task 2 / 75-03:** The evaluator regression baseline is necessary but probably not sufficient. Exact `JsonValue` asserts over hand-built ASTs will catch obvious drift, but they will miss parser-to-evaluator and scope/lifetime interactions unless you include real expression corpus tests.
- **MEDIUM — 75-01 / 75-02 / 75-03:** The verification cadence is too expensive. Full workspace tests and clippy after effectively every function-level commit across ~70+ hotspots is schedule poison. This will drag execution far more than the refactors themselves.
- **MEDIUM — 75-04 Task 4-C:** SATD triage is scope creep on the critical path. Filing GitHub issues, replacing comments, and writing an audit doc does not help flip the badge if you move the badge to complexity-only, and it introduces external auth/network failure modes.
- **LOW — 75-01 / 75-02 summaries and success criteria:** Some projected PMAT count drops are inflated. Retro-justifying legacy `#[allow]` sites improves hygiene, but it does not reduce the complexity count unless PMAT was already honoring those allows and the functions were still being counted.

## Suggestions

- Make the badge command and the PR gate command identical. If Phase 75 says complexity is the gating dimension, then `quality-badges.yml` must also use the complexity-only command or the same proven path filter.
- Add a **Wave 0 proof task**: create one tiny fixture function over 25, confirm PMAT reports it, add `#[allow(clippy::cognitive_complexity)]`, confirm PMAT stops reporting it. If that fails, remove P5 as a strategy immediately.
- Reconcile the authoritative 94-count before execution. Commit one machine-generated inventory file and derive every wave target from that, not from prose.
- Split 75-04 Task 4-B into two explicit branches after the spike: `examples/` plan and `fuzz/` plan. If include filtering fails, that deserves its own wave, not a footnote.
- Change 75-05-02 so the deliberate regression lands in a path the gate actually checks. Put it in a temporary `src/` file on the throwaway branch, not in `tests/`.
- Simplify Wave 0 macro regression coverage. Prefer consumer-facing trybuild or compile snapshots over brittle internal tokenstream plumbing unless you already know the proc-macro internals are testable.
- For `evaluate_with_scope` and `evaluate_array_method_with_scope`, explicitly allow a small structural assist like `EvalContext` / helper context structs. Pure helper extraction may otherwise turn into unreadable 6-arg function spam.
- Move SATD triage behind the badge flip, or mark it explicitly non-blocking and drop it the moment it threatens schedule.
- Reduce verification frequency: per-function targeted tests, per-file/sub-wave workspace tests, full workspace only at wave boundaries.

## Risk Assessment

**HIGH.** The refactor tactics are mostly fine; the plan risk is in the tooling semantics and scope control. Right now the phase can easily end with a green CI gate and a still-red README badge, or with Wave 4 blowing up because the include-path spike fails. Fix the badge/gate mismatch, prove PMAT suppression behavior in Wave 0, and reconcile the inventory counts; after that, the execution risk drops to **MEDIUM**. Realistic effort is closer to **1.5-3 engineer-weeks**, not a quick sequential cleanup phase.

---

## Consensus Summary

Both reviewers approve the overall direction (regression-first Wave 0, hotspot-by-hotspot wave order, D-02/D-03 discipline, fail-closed CI test) but flag substantive issues that should land in CONTEXT.md / PLAN.md before execution starts. Codex is more pessimistic on overall risk (HIGH vs Gemini's MEDIUM) primarily because of the badge/gate semantic mismatch — which Gemini did not catch.

### Agreed Strengths

- **Regression-first Wave 0** with snapshot + semantic baselines before refactor (Gemini: "pro move"; Codex: "right kinds of baselines").
- **D-02 + D-03 discipline** — `// Why:` comments mandatory; hard cap 50 prevents allow-abuse (both reviewers explicit).
- **PMAT version pinning in CI** (Gemini: "critical"; Codex: implicit approval).
- **Wave 5 fail-closed proof** — both like the instinct of actually verifying the gate blocks regressions.
- **Concrete inventory** — both note the per-function/per-file specificity is unusually good.

### Agreed Concerns (HIGH priority — fix before execution)

1. **Proc-macro testing approach in 75-00 Task 1 is broken/brittle.**
   - Gemini (HIGH): physically impossible in standard Rust if `proc-macro = true` (the Rust compiler prohibits non-macro `pub` exports from a proc-macro crate).
   - Codex (MEDIUM): even if technically possible via the fallback, "brittle" and "wastes time before any PMAT debt is burned down."
   - **Both recommend:** drop the `__test_internal` re-export path entirely; use `trybuild` + `cargo expand`-style snapshot of expanded output, OR move logic to a sibling support crate (`pmcp-macros-support` already exists from Phase 71).
   - **Required revision:** Plan 75-00 Task 1 — remove the `__test_internal` branch, commit to the `cargo expand` / consumer-facing snapshot path. Update Step 0 accordingly.

2. **D-09 spike fan-out is undersized** (both flag, different angles).
   - Gemini (LOW): timing risk — Wave 4 may be "locked" before spike result is known.
   - Codex (HIGH): scope risk — if include filtering fails, fuzz has a cog-122 function and 2 examples are over 50; that's another whole refactor wave, not a Wave 4 sub-task.
   - **Both recommend:** Plan 75-04 Task 4-B should be explicitly conditional on Wave 0 spike result with 2-3 named branches, NOT a single task with a footnote. Possibly split into 75-04 + 75-04a sub-plans.

3. **SATD triage (75-04 Task 4-C) is scope creep / too fat.**
   - Gemini (MEDIUM): 33 `gh issue create` calls in a single sub-agent run risks rate limits + timeouts.
   - Codex (MEDIUM): SATD doesn't help the badge flip; introduces external auth/network failure modes; should be moved off the critical path.
   - **Both recommend:** Either (a) group SATD into ~5 issues by hotspot directory (Gemini), or (b) defer SATD triage entirely to Phase 76+ and remove from 75-04 (Codex).

### Codex-only HIGH concerns (Gemini missed — must address)

4. **Badge/gate semantic mismatch — `quality-badges.yml` runs the FULL gate, not complexity-only.**
   - The README badge is set by `quality-badges.yml`'s `pmat quality-gate --fail-on-violation` (no `--checks` filter). Wave 5 only adds `--checks complexity` to `ci.yml`. CI can flip green while the badge stays red because duplicates/SATD/sections still fail in `quality-badges.yml`.
   - **This is a stated-goal failure mode.** The phase goal is "restore the badge", and Wave 5 as planned can satisfy CI without satisfying the badge.
   - **Required revision:** Either (a) Wave 5 also updates `quality-badges.yml` to run the same `--checks complexity` command (so badge and CI gate are identical), OR (b) the planner verifies that duplicates/SATD/sections do NOT fail the default gate today (i.e. they're informational only), in which case the current plan works as-is.

5. **PMAT `#[allow(clippy::cognitive_complexity)]` suppression is load-bearing but UNVERIFIED.**
   - The whole D-02/D-03 escape hatch assumes PMAT honors the clippy allow attribute. If it doesn't, every "P5: justified allow" task fails to reduce the gate count.
   - **Required revision:** Add a Wave 0 task — create a fixture function with cog ≥30, run `pmat quality-gate --fail-on-violation --checks complexity`, confirm violation reported. Add `#[allow(clippy::cognitive_complexity)]` with `// Why:` comment, re-run, confirm violation gone. If suppression doesn't work, P5 must be removed from the planner's toolkit and most refactors become "must reduce", not "may justify".

6. **Inventory math inconsistency — 94 vs 97.**
   - Codex flagged: `94 total` cited but `73 in-scope + 21 examples + 3 fuzz = 97` derived elsewhere.
   - **Required revision:** Reconcile in 75-RESEARCH.md and 75-04-PLAN.md before execution. Commit one machine-generated `pmat-inventory-2026-04-22.json` and reference it from every wave's success criteria.

7. **75-05-02 fail-closed test fixture in `tests/` may false-pass.**
   - The deliberately-bad function for the regression PR lives in `tests/regression-pr-75-05.rs`. If Wave 0 spike resolves to "use `--include 'src/**'`", the gate won't even look at that file. The fail-closed test then proves nothing.
   - **Required revision:** Plan 75-05 Task 5-02 should put the fixture in a temporary `src/` file on the throwaway branch, not in `tests/`. Or condition the path on the Wave 0 spike result.

### Codex MEDIUM concerns (worth addressing)

8. **Verification cadence too expensive** — full workspace tests + clippy after every per-function commit across 70+ hotspots is schedule poison. Consider per-file or per-sub-wave full runs instead of per-task.
9. **Evaluator semantic baseline coverage gap** — hand-built AST asserts may miss parser↔evaluator and scope/lifetime interactions; add real expression corpus tests.
10. **Retro-justified `#[allow]` sites probably don't reduce the gate count** unless PMAT was already honoring those allows AND the functions were still counted. Concern #5 above subsumes this.

### Gemini MEDIUM concerns (Codex didn't address)

11. **"Helper soup" risk in Wave 3** — P6 dispatch decomposition can satisfy PMAT while making the module harder to read if 15 helpers each sit at cog 20-25.
   - **Recommendation:** add an explicit Wave 3 acceptance criterion that newly-created helpers must themselves be ≤25 cog AND total module cognitive complexity (sum across helpers) should be tracked and not balloon disproportionately.

### Divergent Views

- **Overall risk:** Gemini says MEDIUM ("clear to proceed" after fixing macro testing); Codex says HIGH ("do not trust this plan to flip the README badge reliably"). The disagreement is largely driven by Codex catching the badge/gate mismatch. If that's resolved, both probably converge on MEDIUM.
- **`pmcp-code-mode` refactor approach:** Gemini worries about helper soup partitioning the complexity without reducing it; Codex worries the opposite — pure helper extraction will become "6-arg function spam" and recommends introducing an `EvalContext` struct. Both are valid; the planner should explore both before locking in Wave 3 detail.
- **SATD treatment:** Gemini says reduce to ~5 issues and keep in scope. Codex says drop entirely from Phase 75. Either solves the "fat task" problem; the choice depends on whether the team values SATD cleanup as part of "restoring the gate."

### Recommended Next Action

The reviewers' combined feedback identifies 4 issues that meaningfully threaten the phase goal:

1. Plan 75-00 Task 1 will not work as written (proc-macro export prohibition).
2. Wave 5 may flip CI green without flipping the badge (badge command vs gate command mismatch).
3. PMAT `#[allow]` suppression is unproven — half the plan fails if it doesn't work.
4. Inventory math inconsistency — every wave delta is suspect until reconciled.

**Run `/gsd-plan-phase 75 --reviews`** to fold these into a revised plan set before executing. The remaining reviewer feedback (verification cadence, helper soup acceptance criteria, SATD scoping, fixture location) are sharpening edits the planner should bundle into the same revision pass.
