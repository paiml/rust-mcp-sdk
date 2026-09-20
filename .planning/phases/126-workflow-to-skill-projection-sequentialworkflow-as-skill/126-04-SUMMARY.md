---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 04
subsystem: api
tags: [sep-2640, skills, workflow, projection, builder, sc-6, tracing, tdd]

# Dependency graph
requires:
  - phase: 126-01
    provides: "`src/server/skills/projection.rs`, the crate-private `project_with_notices(&SequentialWorkflow) -> (Skill, Vec<ProjectionNotice>)` seam, `slugify` / `fallback_slug` / `sanitize_for_log` / `resolve_description`, and `SequentialWorkflow::as_skill()`"
  - phase: 126-02
    provides: "the full D-11 render (`## Context`, `## Inputs`, per-step detail) whose bytes this plan must not move"
provides:
  - "`SkillProjection<'w>` with `new` / `with_tools` / `build` — the fallible, checking counterpart to `as_skill()` (D-01/D-02)"
  - "`ProjectionOutput { skill, warnings }`, `#[non_exhaustive]`, with `into_parts()` — the GATE A shape"
  - "`ProjectionWarning` + `#[non_exhaustive] ProjectionWarningKind`, both public (D-10)"
  - "the SC-6 gate check: guidance on an annotated side-effecting step, with a DISTINCT `GateCheckUnverifiable` note when annotations are missing (D-08/D-09)"
  - "D-10's narrowed delivery contract, written into three rustdocs: SC-6 warnings have exactly ONE channel, `build()`'s structured return"
  - "`pub use projection::{ProjectionOutput, ProjectionWarning, ProjectionWarningKind, SkillProjection};` in `src/server/skills/mod.rs`"
affects:
  - 126-05
  - 126-06
  - 126-07

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized src/ diff),
# NOT a harness token count. Measured: `git diff 7c2d1f97..HEAD -- src/ | wc -c`
# == 45,068.
actuals:
  tokens: 11267
  tasks: 3
  commits: 5

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Wrap the seam, never a second renderer: `build()` performs its two strict checks and then calls the SAME crate-private `project_with_notices` that `as_skill()` calls, so the two entry points cannot render different bytes. A `debug_assert!` states (and, in debug builds, proves) that both substitution conditions were rejected before the seam ran, so the seam returns no notices."
    - "Two-function split for a four-way lookup: `gate_check` owns the iteration and the `Option`-map short-circuit; `gate_check_step` owns the four-arm match. Keeping the loop and the match in separate bodies is what holds PMAT cognitive complexity at the measured `src/` zero-violation baseline."
    - "`Option<HashMap<..>>`, never `HashMap<..>`: `None` (no tool map supplied) and an EMPTY map (supplied, tool absent) are semantically different — silent versus unverifiable — and collapsing them would erase D-07/D-08's whole distinction."
    - "Sanitize at the log edge, not at construction: the structured `ProjectionWarning` keeps the raw author text so callers and tests see exactly what the workflow said; only the `tracing` record passes through `sanitize_for_log` (T-126-01)."

key-files:
  created: []
  modified:
    - src/server/skills/projection.rs
    - src/server/skills/mod.rs
    - src/server/workflow/sequential.rs

key-decisions:
  - "GATE A implemented as `pub fn build(self) -> Result<ProjectionOutput>` with `#[non_exhaustive] pub struct ProjectionOutput { pub skill: Skill, pub warnings: Vec<ProjectionWarning> }` plus a consuming `into_parts()`. `#[non_exhaustive]` — not a pair of accessors — is what makes the type additive for downstream crates, which read these fields but never construct them."
  - "D-10 NARROWED and written down in three places: SC-6 gate warnings have exactly ONE delivery channel, `build()`'s structured return. `as_skill()` receives only a `&SequentialWorkflow` and annotations live only on `crate::types::ToolInfo::annotations`, so it cannot COMPUTE the warning — it is not that it declines to log it."
  - "`build()` emits `tracing::warn!` on the module's existing `mcp.skills` target for every warning it returns, so the BUILDER path genuinely carries both of D-10's channels, including for SC-6."
  - "The SC-6 trigger is structural and never textual (D-09): guidance present AND `read_only_hint == Some(false)` OR `destructive_hint == Some(true)`. No phrase list, no prose analysis — a heuristic over the wording could be paraphrased around; this cannot."
  - "MCP's literal annotation defaults are NOT followed (D-08). An absent `read_only_hint` produces `GateCheckUnverifiable`, not a gate finding — under the literal defaults the gate would fire on essentially every existing workflow, and a warning that fires everywhere is a warning that gets muted."

patterns-established:
  - "A `debug_assert!` rather than a `panic!` for an internal invariant inside a `Result`-returning function: it is compiled out in release, so no user build can panic there (D-15 / T-126-11), while a test build still catches a future regression."
  - "Anti-vacuity inside the negative test: `without_a_tool_map_nothing_warns_even_for_a_tripping_workflow` first proves the SAME fixture produces exactly one warning WITH a map, then asserts zero without one. A bare zero-assertion would pass against a `gate_check` that had been deleted."

requirements-completed: [SC-6, SC-1]

coverage:
  - id: D1
    description: "D-01/D-02 — `SkillProjection::new(&wf).with_tools(..).build()` exists with the GATE A return shape, `as_skill()` stays infallible, and both entry points share ONE renderer (byte-equal bodies)."
    requirement: "SC-1"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::build_and_as_skill_share_one_renderer"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::with_tools_records_a_map_and_still_builds_the_same_bytes"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::projection_output_into_parts_returns_both_halves"
        status: pass
      - kind: other
        ref: "cargo test -p pmcp --features skills,... --doc skills -- 5 new doctests on ProjectionWarning / SkillProjection / new / with_tools (x2) / build, 15 passed"
        status: pass
    human_judgment: false
  - id: D2
    description: "D-15 + REVIEWS finding 6 — `build()` returns `Err(Error::Validation)` on BOTH an illegal name and an empty (or whitespace-only) description, while `as_skill()` substitutes for the same inputs. Neither panics."
    requirement: "SC-1"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{build_rejects_a_name_that_normalizes_to_nothing,as_skill_falls_back_where_build_rejects_the_name}"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{build_rejects_an_empty_description,build_rejects_a_whitespace_only_description,as_skill_substitutes_where_build_rejects_the_description}"
        status: pass
      - kind: other
        ref: "grep -n 'panic!' over the production half of src/server/skills/projection.rs (lines before `#[cfg(test)]`) == 0 — see Deviation 4"
        status: pass
    human_judgment: false
  - id: D3
    description: "SC-6 / D-09 — a guidance-bearing step whose tool is annotated `destructive_hint == Some(true)` or `read_only_hint == Some(false)` produces exactly ONE gate warning naming the step and the tool, whose message states that server-side execution runs the step regardless of the guidance."
    requirement: "SC-6"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::gate_fires_for_guidance_on_a_destructive_tool"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::gate_fires_for_guidance_on_an_explicitly_not_read_only_tool"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::two_side_effecting_guidance_bearing_steps_produce_exactly_two_warnings (count == 2, not merely non-empty)"
        status: pass
    human_judgment: false
  - id: D4
    description: "D-08 — a guidance-bearing step whose tool carries no annotations, or is absent from the supplied map, produces a DISTINCT `GateCheckUnverifiable` note rather than the gate warning; a read-only tool produces nothing; a side effect without guidance produces nothing; a resource-only step produces nothing."
    requirement: "SC-6"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::an_unannotated_tool_is_unverifiable_not_a_gate_finding"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::a_tool_absent_from_the_map_is_unverifiable"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{gate_stays_silent_for_guidance_on_a_read_only_tool,a_destructive_tool_without_guidance_produces_nothing,a_resource_only_step_with_guidance_produces_nothing}"
        status: pass
    human_judgment: false
  - id: D5
    description: "D-07 — `with_tools` never called means zero diagnostics, proven against the SAME fixture that produces one warning when a map IS supplied (anti-vacuous)."
    requirement: "SC-6"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::without_a_tool_map_nothing_warns_even_for_a_tripping_workflow"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::build_without_a_tool_map_emits_no_warnings"
        status: pass
    human_judgment: false
  - id: D6
    description: "D-10 as narrowed — `build()` returns the warnings AND logs them on `mcp.skills`; `as_skill()` never calls `gate_check`; the single-channel limitation is stated at both entry points; the false 'two delivery channels' claim survives nowhere in `src/server/skills/`."
    requirement: "SC-6"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::build_returns_warning_kinds_and_counts_as_data (asserts kinds + count with NO tracing subscriber installed)"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::the_warning_is_a_builder_capability_and_the_bytes_are_identical_either_way"
        status: pass
      - kind: other
        ref: "grep -c 'target: \"mcp.skills\"' src/server/skills/projection.rs == 3; the only distinct `target:` string in the file is `mcp.skills`"
        status: pass
      - kind: other
        ref: "grep -rn 'two delivery channels' src/server/skills/ | grep -c . == 0"
        status: pass
      - kind: other
        ref: "grep -n 'gate_check' — the only call site outside the function pair is `build()`; `project()` / `as_skill()` never reach it"
        status: pass
    human_judgment: false
  - id: D7
    description: "T-126-01 — every author-supplied string entering a `tracing` field, and the message body itself, passes through `sanitize_for_log`; a newline and an ESC are both neutralized."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::an_author_supplied_name_is_neutralized_before_it_reaches_a_log_field"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sanitize_for_log_replaces_control_characters (pre-existing, plan 126-01)"
        status: pass
    human_judgment: false
  - id: D8
    description: "Project quality gates unchanged — PMAT cognitive complexity at the zero-violation baseline (`gate_check`'s four-way branch was the named risk), skills lint clean, rustdoc zero-warning, zero SATD, and the plan-126-02 rendered bytes unmoved."
    verification:
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> Total violations: 0, Quality gate PASSED"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make lint-skills -> exit 0, zero warnings (features full,skills)"
        status: pass
      - kind: other
        ref: "RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps --features <doc-check set> -> exit 0, zero warnings"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make check-todos -> No technical debt comments"
        status: pass
      - kind: integration
        ref: "cargo test --test skills_integration -- 15 passed (unchanged; the builder moved no rendered byte)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> exit 0 (all four guarded selectors non-zero)"
        status: pass
    human_judgment: false

# Metrics
duration: 35 min
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 04: `SkillProjection` builder and the SC-6 gate warning Summary

**`SkillProjection::new(&wf).with_tools(tools).build()` ships as the fallible, checking counterpart to `as_skill()` — it rejects an illegal name or an empty description where `as_skill()` substitutes, and it is the only path that can see the SC-6 trap: guidance attached to a step whose tool is annotated side-effecting, which server-side execution will run regardless.**

## Performance

- **Duration:** 35 min
- **Started:** 2026-09-04T17:02:00Z (approx.)
- **Completed:** 2026-09-04T17:37:00Z
- **Tasks:** 3 executed (Tasks 1 and 2 carried `tdd="true"` and produced RED + GREEN commits)
- **Files modified:** 3 (0 created)

## Accomplishments

- **GATE A is implemented, and the departure from D-02's literal wording is recorded in the API itself, not just in a plan document.** `build()` returns `Result<ProjectionOutput>`; both `ProjectionOutput`'s rustdoc and `build()`'s carry a `# Return shape — a recorded deviation from D-02's literal wording` section naming the incompatibility (a `Skill` cannot carry a warning vector) and why a named struct was chosen. See **Deviations** below for the side-by-side quotation the plan required.
- **`build()` wraps the 126-01 seam rather than re-rendering.** It performs its two strict checks, then calls the same crate-private `project_with_notices` that `as_skill()` calls. This is why `build_and_as_skill_share_one_renderer` is a byte-equality assertion over the kitchen-sink fixture and not a hopeful comparison of two implementations: there is only one implementation.
- **SC-6 holds, pinned by nine count-exact tests.** Every case in the plan's `<behavior>` list has its own test asserting an exact warning COUNT and KIND — including the four cases that must produce ZERO. The two-tripping-steps case asserts `len() == 2` and the step names in order.
- **D-08's honesty rule is implemented as a distinct kind, not a softer message.** An unannotated tool, or one absent from the supplied map, yields `GateCheckUnverifiable` — a report that the check could not be performed. MCP's literal annotation defaults (absent `read_only_hint` meaning not-read-only) are deliberately not followed, and that rejection is written into `is_annotated_side_effecting`'s rustdoc so a future reader cannot "fix" it back.
- **codex's HIGH finding is closed in source, not only in prose.** The claim that D-07 and D-10 cannot both hold for SC-6 is correct and now has three enforcement points: `build()` emits both channels for everything it can see; `as_skill()`'s, `SkillProjection::new`'s and `SkillProjection`'s rustdocs each state that a bare `as_skill()` holds no tool map and therefore *cannot compute* the warning; and `grep -rn 'two delivery channels' src/server/skills/` returns nothing.
- **T-126-01 is honored at the log edge only.** The structured `ProjectionWarning` keeps the raw author text — that is what callers and tests need — while the `tracing` record neutralizes the workflow name, the step name, the tool name *and the message body* (which embeds the step and tool names) through the existing `sanitize_for_log`. No second neutralizer was written.
- **Test counts moved with nothing lost:** `--lib skills::projection` 53 → **74**, `--lib skills` 152 → **173**, `--doc skills` 10 → **15**. `skills_integration` (15) and `skills_routing` (23) are unchanged, which is the point — the builder moved no rendered byte. PMAT `Total violations: 0`; `make lint-skills`, `make test-skills`, `make check-todos` and the rustdoc zero-warning run all exit 0.

## Task Commits

1. **Task 1 (RED): failing tests for the fallible builder** — `98608e26` (test). RED verified: 7 × `E0433: cannot find type SkillProjection`.
2. **Task 1 (GREEN): `SkillProjection` + `ProjectionWarning` + `ProjectionOutput`** — `e9b3d247` (feat). 53 → 62 projection tests.
3. **Task 2 (RED): failing tests for the SC-6 gate** — `135c39d6` (test). RED verified: 6 failed / 65 passed.
4. **Task 2 (GREEN): `gate_check` + the D-07/SC-6 rustdoc on `as_skill()`** — `dac99b20` (feat). 62 → 71.
5. **Task 3: D-10's narrowed tracing channel** — `16361dc6` (feat). 71 → 74.

No REFACTOR commit was needed on either TDD task; both GREEN implementations were lint-clean and PMAT-clean as written.

## Files Created/Modified

- `src/server/skills/projection.rs` — **modified.** Gained `ProjectionWarningKind` (3 variants, `#[non_exhaustive]`), `ProjectionWarning` (private fields + 4 accessors + a module-private `new`), `ProjectionOutput` (`#[non_exhaustive]`, 2 public fields, `into_parts`), `SkillProjection<'w>` (`new` / `with_tools` / `build`), `is_annotated_side_effecting`, `gate_check_step`, `gate_check`, and 21 new unit tests.
- `src/server/skills/mod.rs` — **modified.** One added re-export: `pub use projection::{ProjectionOutput, ProjectionWarning, ProjectionWarningKind, SkillProjection};`. This is the `pub use` line plan 126-01 Task 3 deliberately withheld.
- `src/server/workflow/sequential.rs` — **modified.** `as_skill()`'s rustdoc gained a `# This path holds no tool map, so it cannot warn about a destructive step` section. See Deviation 5 — this file is not in the plan's `files_modified`, but two acceptance criteria name this rustdoc.

## Decisions Made

### D-02 deviation, quoted side by side (the plan's explicit obligation)

| | |
|---|---|
| **D-02's literal wording** (`126-CONTEXT.md`) | "the builder's `build() -> Result<Skill>` is where strict checks live" |
| **Shipped signature** | `pub fn build(self) -> Result<ProjectionOutput>` where `#[non_exhaustive] pub struct ProjectionOutput { pub skill: Skill, pub warnings: Vec<ProjectionWarning> }` |

D-10 requires `build()` to return structured warnings; a `Skill` cannot carry a warning vector, so D-02 and D-10 are literally incompatible. All three cross-AI review lanes raised this, and it is the only finding all three raised. The human GATE A answer (plan 126-01 Task 1, LOCKED) selected `output-struct`. The deviation is from the success type's **name and shape only** — `build()`'s fallibility, its strict checks and its `# Errors` contract are exactly what D-02 specified.

This deviation is recorded in three places, as required: this SUMMARY, `build()`'s rustdoc, and `ProjectionOutput`'s rustdoc. **Plan 126-06 inherits the fourth: naming it in the CHANGELOG entry.**

### D-10 narrowing (from the ROADMAP line for this plan, and the plan's own `<d07_d10_resolution>`)

SC-6 gate warnings have **exactly one** delivery channel: the structured return from `build()`. The plan's `<objective>` still said "the D-10 dual delivery channel"; the narrowing supersedes it. Verified against source: `ToolAnnotations` are reachable only through `crate::types::ToolInfo::annotations`; `WorkflowStep::tool()` returns a `ToolHandle`, which carries a name. `as_skill()` therefore cannot *compute* an SC-6 warning, so documenting a `tracing` channel for it on that path would tell an operator to watch for a log line that can never be emitted (T-126-23).

What each path does now:

| Path | Slug fallback | Empty description | SC-6 gate |
|------|---------------|-------------------|-----------|
| `as_skill()` | `tracing::warn!` on `mcp.skills`, substitutes | `tracing::warn!`, substitutes | **cannot compute** — nothing emitted |
| `build()` | `Err(Error::Validation)` | `Err(Error::Validation)` | structured return **and** `tracing::warn!` on `mcp.skills` |

### `ProjectionWarningKind::SlugFallback` is declared but not emitted today

The plan's Task 1 `<action>` specifies three variants, one of them for the slug fallback. `build()` **rejects** that input rather than substituting, so the builder path cannot emit this kind — and it is the only path that produces `ProjectionWarning`s at all. The variant ships anyway, with its rustdoc saying so explicitly, because it completes the vocabulary over the conditions the infallible path resolves and makes a future lenient mode additive rather than breaking. It is a deliberate API declaration, not a stub: no functionality is missing behind it and no caller is blocked by it.

### Implementation decisions taken here

- **`Option<HashMap<..>>` over `HashMap<..>`.** `None` and an empty map are semantically different (D-07 silence versus D-08 unverifiable) and the type keeps them apart; a comment at the field says so.
- **`gate_check` / `gate_check_step` split.** The plan named `gate_check`'s four-way branch as the specific cog-25 risk. Splitting the iteration from the match kept PMAT at `Total violations: 0` without an `#[allow]`.
- **`debug_assert!`, never `panic!`, for the seam invariant.** `build()` asserts that `project_with_notices` returned no notices (both substitution conditions were rejected above). `debug_assert!` is compiled out in release, so no user build can panic inside this `Result`-returning function — D-15 / T-126-11's shape, which is Phase 125's still-open WR-03 finding.
- **Sanitize the message body too.** `ProjectionWarning::message()` interpolates the step and tool names, both author-supplied. The log record passes the message through `sanitize_for_log`; the structured return keeps it raw.
- **A second `with_tools` doctest.** The first shows a read-only tool tripping nothing and the bytes staying identical; the second shows the gate firing with `assert_eq!(output.warnings.len(), 1)`. `with_tools` exists for the second case, so a doctest that only demonstrated silence would document the wrong thing.

## Deviations from Plan

### Recorded deviations

**1. [Mandated record] `build()`'s success type is `ProjectionOutput`, not D-02's literal `Result<Skill>`**

- **Found during:** N/A — pre-decided at plan 126-01 Task 1's human gate and carried into this plan as an obligation.
- **Issue:** D-02 and D-10 cannot both hold as written.
- **Fix:** Implemented the LOCKED GATE A answer. Quoted side by side under **Decisions Made**; stated in `build()`'s and `ProjectionOutput`'s rustdocs.
- **Files modified:** `src/server/skills/projection.rs`
- **Committed in:** `e9b3d247`

**2. [Rule 2 - Missing Critical] `ProjectionOutput` ships `#[non_exhaustive]` public fields, not the plan text's `skill()` / `warnings()` accessors**

- **Found during:** Task 1
- **Issue:** The plan's Task 1 `<action>` asks for "public accessors `skill()`, `warnings()` and an `into_parts()` consuming accessor", but the LOCKED GATE A record in `126-01-SUMMARY.md` shows **public fields** and the caller form `projection.build()?.skill`. Shipping both would be redundant surface on a published 2.x crate; shipping accessors only would contradict the locked record.
- **Fix:** Public fields (matching the locked record) plus `into_parts()`, with `#[non_exhaustive]` supplying the additivity the accessors were meant to provide — downstream crates read these fields but can never construct the type, so a later field is additive.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `projection_output_into_parts_returns_both_halves`; all five doctests use `output.skill` / `output.warnings` field access and pass.
- **Committed in:** `e9b3d247`

**3. [Rule 2 - Missing Critical] The plan's `<objective>` line still promised "the D-10 dual delivery channel" for SC-6**

- **Found during:** Task 3
- **Issue:** The plan's own `<d07_d10_resolution>` block and the ROADMAP narrowing both strike that claim, but the `<objective>` sentence survived the replan.
- **Fix:** Implemented the narrowing. SC-6 has one channel; `build()` has two channels for everything it can observe; the false claim is greppably absent from `src/server/skills/`.
- **Files modified:** `src/server/skills/projection.rs`, `src/server/workflow/sequential.rs`
- **Verification:** `grep -rn 'two delivery channels' src/server/skills/ | grep -c .` == 0.
- **Committed in:** `dac99b20`, `16361dc6`

**4. [Rule 1 - Bug] The acceptance criterion `grep -c 'panic!' src/server/skills/projection.rs` returns 0 is unsatisfiable as written**

- **Found during:** Task 1 (acceptance-criteria verification loop)
- **Issue:** The count is **1**, and the single occurrence is `parsed_frontmatter`, a `#[cfg(test)]` helper written by plan 126-01 that panics with a diagnostic when the encoder's output fails to parse. The criterion's intent is D-15/T-126-11: no `panic!` inside a `Result`-returning production function. A file-wide grep cannot distinguish a test assertion from a production panic, and rewording the helper to dodge the grep would either weaken a real assertion or game the gate — the opposite of 126-01's Deviation 2, where rewording a *comment* was the honest fix.
- **Fix:** Verified the criterion's actual intent instead: the production half of the file (everything before `#[cfg(test)]`) contains **zero** `panic!`, and `build()` returns `Err` on both failure conditions. The one internal invariant is a `debug_assert!`, compiled out in release.
- **Files modified:** none (the pre-existing helper is left alone; it is out of this plan's scope).
- **Verification:** `sed -n "1,$(grep -n '^#\[cfg(test)\]' src/server/skills/projection.rs | cut -d: -f1)p" src/server/skills/projection.rs | grep -c 'panic!'` == 0.

**5. [Rule 3 - Blocking] Edited `src/server/workflow/sequential.rs`, which is not in the plan's `files_modified`**

- **Found during:** Task 2
- **Issue:** Task 2's and Task 3's acceptance criteria both require `as_skill()`'s rustdoc to state the D-07 limitation and SC-6's single channel. `as_skill()` lives in `src/server/workflow/sequential.rs`, but the plan's frontmatter lists only the two `skills/` files.
- **Fix:** Added the required section to `as_skill()`'s rustdoc. The change is documentation only — no code in that file was touched.
- **Files modified:** `src/server/workflow/sequential.rs` (+23 lines, all `///`)
- **Verification:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` exits 0; `--doc sequential`'s `as_skill` doctest still runs.
- **Committed in:** `dac99b20`

**6. [Rule 3 - Blocking] `ProjectionWarning::new` was withheld from Task 1's commit**

- **Found during:** Task 1 (`make lint-skills`)
- **Issue:** Task 1 has no warning producer, so a constructor added there is `dead_code`, and `make lint-skills` runs under `RUSTFLAGS=-D warnings`. CLAUDE.md is zero-tolerance, so Task 1's own gate would have failed.
- **Fix:** `ProjectionWarning::new` was added in Task 2, at its first use site.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `make lint-skills` exits 0 at both `e9b3d247` and `dac99b20`.

---

**Total deviations:** 1 mandated record + 5 auto-fixed (1 bug, 2 missing-critical, 2 blocking)
**Impact on plan:** None on scope or architecture. No new decision was taken outside the two human gates; Deviation 2 resolves a conflict between the plan text and the locked gate record in favour of the gate record.

## Threat Flags

None. This plan added no network endpoint, no auth path, no file access and no schema change. The one logging side effect is covered by the existing T-126-01 mitigation (`sanitize_for_log`), and every warning stays on the server's own `mcp.skills` target — no `skills/list`, `skills/get` or `resources/read` response carries one (T-126-12, disposition `accept`).

## Known Stubs

None. `ProjectionWarningKind::SlugFallback` is documented above as a declared-but-not-yet-emitted API variant with an explicit rustdoc explanation; no functionality is missing behind it and nothing is blocked by it.

## Issues Encountered

- **`rtk` corrupted output repeatedly, as the environment notes warned.** `make lint-skills`' tee log came back containing only the echoed clippy invocation, and a `cat` of a 51-line `make test-skills` log was truncated with "(200 lines truncated)". Every number and test name in this SUMMARY was read through the absolute toolchain binary (`~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo`) or from a `$?` capture, never from a filtered pipe.
- **A wrong clippy feature set produced two false findings.** Running the skills lint policy against `skills,streamable-http,http-client,testing` (the *test* feature set) rather than `LINT_SKILLS_FEATURES = full,skills` surfaced two `clippy::zero_sized_map_values` errors in `src/server/auth/jwt.rs`. The Makefile already documents this exact trap in a comment above the variable. Out of scope, untouched, and green under the correct set.
- **No git hooks are installed** in this checkout, so CLAUDE.md's pre-commit quality gate does not fire. Every gate was run manually before each of the five commits: `cargo fmt --all`, `make lint-skills`, the `doc-check` rustdoc invocation, `make check-todos`, `make test-skills`, `v1_severability_tripwire`, and `pmat quality-gate --checks complexity`.
- **`.pmat/` tracked files show as modified** after every PMAT run (`context.db`, `deps-cache.json`, `metrics/dependencies.json`, `project.toml`). This is tool churn predating this plan; none of it was staged.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Ready for plan 126-05** (wave 4: `WorkflowPromptHandler::with_projected_skill_prepend` and GATE B's `add-builder-path` on `ServerCoreBuilder` / `ServerBuilder`). GATE B is settled input, not an open question.
- **Plan 126-06 inherits the CHANGELOG obligation** for the GATE A deviation, and now also for the D-10 narrowing. Its module-rustdoc task (Task 2) must state SC-6's single delivery channel — the two entry-point rustdocs already do.
- **The D-14 golden is safe.** `skills_integration` is unchanged at 15 passing, and `with_tools_records_a_map_and_still_builds_the_same_bytes` plus `the_warning_is_a_builder_capability_and_the_bytes_are_identical_either_way` both assert byte-equality against `as_skill()`, so plan 126-06's golden derived from the 126-02 render still matches.
- **No blockers.** No `.strict(true)` escalation was added (explicitly deferred in CONTEXT.md), and `grep -cE 'fn strict' src/server/skills/projection.rs` returns 0.

## Self-Check: PASSED

- All modified files present on disk: `src/server/skills/projection.rs`, `src/server/skills/mod.rs`, `src/server/workflow/sequential.rs`.
- All five commit hashes resolve in `git log --oneline --all`: `98608e26`, `e9b3d247`, `135c39d6`, `dac99b20`, `16361dc6`.
- All plan-level `<verification>` commands re-run green after the final commit: `--lib skills::projection` **74 passed** (> plan 126-02's 53), `--doc skills` **15 passed** (nonzero), `make test-skills` exit 0, `make lint-skills` exit 0, `pmat quality-gate --fail-on-violation --checks complexity` **Total violations: 0 / Quality gate PASSED**.
- Supplementary: `--lib skills` 173 passed, `skills_integration` 15 passed, `skills_routing` 23 passed, `v1_severability_tripwire` 18 passed, rustdoc zero-warning run exit 0, `make check-todos` clean.
