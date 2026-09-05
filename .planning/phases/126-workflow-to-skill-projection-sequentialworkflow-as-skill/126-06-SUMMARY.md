---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 06
subsystem: api
tags: [sep-2640, skills, workflow, projection, golden, changelog, doctests, supply-chain]

# Dependency graph
requires:
  - phase: 126-02
    provides: "the complete D-11 render universe — `## Context`, `## Inputs` and full per-step detail — so the golden is recorded against a render nothing later in the phase changes"
  - phase: 126-04
    provides: "`SkillProjection` / `ProjectionOutput` / `ProjectionWarning`, GATE A's return shape, and the D-10 narrowing this plan's CHANGELOG names"
  - phase: 126-05
    provides: "GATE B's three public prepend methods and the flag-off byte-identity measurement this plan's CHANGELOG names"
provides:
  - "`tests/golden/workflow_skill_projection.md` — the D-14 byte pin, git-tracked, consumed through `include_str!`"
  - "`golden_render_is_byte_equal` + `golden_break_message`, the named failure message that forbids a silent re-record and states the CHANGELOG/re-pin consequence"
  - "`.gitattributes` forcing LF on `tests/golden/**`, so a CRLF checkout cannot fail the pin for a reason unrelated to the renderer"
  - "the `## [2.20.0] - Unreleased` CHANGELOG section carrying all three inherited obligations plus the digest-significance of constant key order"
  - "the module rustdoc's remaining eight contract points (D-12, D-13, encoding contract, absent length bound, the constant-literal catch-all rule, the D-10 narrowing, GATE B reachability, the `instruction()` observation)"
  - "`ProjectionOutput::into_parts`'s doctest and two builder doctests converted from `no_run`/construction-only to executing assertions"
affects:
  - 126-07

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized diff),
# NOT a harness token count. Measured: `git diff be75ee2e..HEAD | wc -c` == 25,400.
actuals:
  tokens: 6350
  tasks: 3
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A golden's anti-vacuity guards must not shadow the byte comparison they protect. The first mutation run (`policy #7` -> `#8`) tripped the escaped-description guard rather than the equality, proving the guard live but proving nothing about the comparison. A second mutation on a byte no guard covers (`Judgment: Confirm` -> `confirm`) was needed to observe `golden_break_message` actually printed. Two mutations, two different assertions — one would have left half the test unmeasured."
    - "A byte-pinning golden needs a `.gitattributes` line, not just a test. `include_str!` reads the WORKING TREE, so on a checkout with `core.autocrlf=true` every line ending differs and the test fails for a reason that has nothing to do with the renderer. `tests/golden/** text eol=lf` makes the pin mean what it says on every platform."
    - "`rust,no_run` is where a doctest goes to look like coverage. Both GATE B builder doctests compiled and asserted nothing; the `# let _ = server;` line that silences the unused warning is also what hides that no claim is being made. Dropping `no_run` forces the question 'what does this actually prove?' — and for `ServerCoreBuilder`, whose `ServerCore` exposes no synchronous prompt accessor, the honest answer had to name the integration test that carries the real proof."
    - "A CHANGELOG heading in this repo is the bracketed VERSION even when unreleased. `release.yml`'s `create-release` extracts notes with `index($0, \"## [\" ver \"]\") == 1` and exits 1 on no match, so a literal `## [Unreleased]` would fail the release job. `## [2.20.0] - Unreleased` satisfies both, matching the file's own `## [2.16.0] - Unreleased` precedent."

key-files:
  created:
    - tests/golden/workflow_skill_projection.md
    - .gitattributes
  modified:
    - tests/skills_integration.rs
    - CHANGELOG.md
    - src/server/skills/projection.rs
    - src/server/builder.rs
    - src/server/mod.rs

key-decisions:
  - "The golden is consumed through `include_str!`, NOT `insta`. `insta` is a dev-dependency at `Cargo.toml:253` used nowhere in this tree, and introducing a `cargo insta review` workflow into a quality gate that has no story for it is scope this plan explicitly refuses. `include_str!` additionally registers a cargo rebuild dependency on the golden, so editing the file re-runs the comparison — which IS the D-14 tripwire behaviour, obtained for free."
  - "`.gitattributes` was added (it did not exist) with a single `tests/golden/** text eol=lf` rule. The golden's contract is its exact bytes; a CRLF checkout would break it on Windows for a reason unrelated to the renderer, turning a supply-chain tripwire into a platform-flaky test. Deviation Rule 2 — a correctness requirement the plan did not enumerate."
  - "The CHANGELOG section is `## [2.20.0] - Unreleased` — the bracketed version, never the literal `[Unreleased]`. `Cargo.toml` was deliberately NOT bumped: version bumps belong to CLAUDE.md's release procedure, and `Cargo.toml` is outside this plan's `files_modified`."
  - "Both `with_workflow_skill_prepend` doctests were `rust,no_run` and construction-only. They now execute; `ServerBuilder`'s asserts `get_prompt(\"refund_flow\").is_some()` (the prompt keeps the workflow's own name while the projected skill is `refund-flow`), and `ServerCoreBuilder`'s asserts the projected bytes with prose naming the integration test that carries the end-to-end proof, because `ServerCore` exposes no synchronous prompt accessor. Deviation Rule 2."
  - "`ProjectionOutput::into_parts` gained a doctest it did not have. It is the only way a DOWNSTREAM crate moves both fields out at once — `#[non_exhaustive]` forbids struct-pattern destructuring outside the defining crate — which is worth demonstrating, and it is what honestly moves the `--doc skills` count off the 15 plan 126-04 left it at rather than recording an unchanged count against a `fails_when` that expected growth."
  - "The golden fixture's two template bindings are inserted `zeta_total` first and must render `alpha_reason` first. Without a fixture whose insertion order differs from its sort order, the golden could not catch a regression from `render_step`'s `BTreeMap` back to raw `HashMap` iteration, and half of D-14's value would be gone."

patterns-established:
  - "Prove a golden's sensitivity by mutating the GOLDEN, not the renderer, when the plan is test-only. Editing the recorded bytes is the exact operator action the failure message governs, so mutating there also exercises the message a maintainer will read."
  - "Name the test that carries a proof a doctest cannot. A doctest constrained by a missing accessor should say which integration test closes the gap, rather than asserting something weaker and letting a reader assume it is the whole claim."

requirements-completed: [SC-2, SC-3]

coverage:
  - id: D1
    description: "D-14 / SC-2 (golden half) — `tests/golden/workflow_skill_projection.md` holds the exact rendered bytes of a fixed fixture, is git-tracked, is consumed via `include_str!`, and a test asserts byte equality. The golden includes the `## Context` and `## Inputs` sections and full per-step detail (the complete D-11 universe), so a render change anywhere in it is loud."
    requirement: "SC-2"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#golden_render_is_byte_equal"
        status: pass
      - kind: other
        ref: "MUTATION 1: `sed 's/policy #7\"/policy #8\"/'` on the golden -> red on the escaped-description anti-vacuity guard; reverted, `cmp` clean"
        status: pass
      - kind: other
        ref: "MUTATION 2: `sed 's/^Judgment: Confirm/Judgment: confirm/'` on the golden (a byte no guard covers) -> red through the byte comparison, printing golden_break_message; reverted, `cmp` clean"
        status: pass
      - kind: other
        ref: "git ls-files -- tests/golden/workflow_skill_projection.md | grep -c . -> 1"
        status: pass
    human_judgment: false
  - id: D2
    description: "The golden pins a NON-TRIVIAL `yaml_double_quoted` output (REVIEWS finding 1): its description carries a `: ` mapping indicator and a `#` comment indicator, so the pinned `description:` line is the escaped form. A golden whose description were a plain word would stay green through a regression that dropped the escaping."
    requirement: "SC-2"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#golden_render_is_byte_equal (asserts GOLDEN contains `description: \"Process a customer refund: policy #7\"\\n`)"
        status: pass
      - kind: other
        ref: "recorded bytes, line 3: `description: \"Process a customer refund: policy #7\"`"
        status: pass
    human_judgment: false
  - id: D3
    description: "D-14 (operator contract) — the golden-mismatch failure message is produced by a NAMED `fn` with a rustdoc, names the file to update, states that a CHANGELOG entry is required because a digest-pinning consumer must re-pin, and forbids re-recording the golden to turn a red test green."
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#golden_break_message (observed printed under MUTATION 2)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Template-binding sort order is pinned: the fixture inserts `zeta_total` before `alpha_reason` and the golden renders `alpha_reason` first, so the golden can catch a regression from `render_step`'s BTreeMap back to raw HashMap iteration."
    requirement: "SC-2"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#golden_render_is_byte_equal (asserts the alpha index precedes the zeta index)"
        status: pass
    human_judgment: false
  - id: D5
    description: "No new integration-test file and no `insta`: the golden test lives in `tests/skills_integration.rs`, reached by `make test-skills` selector 3, and `tests/golden/` is not picked up by cargo's `tests/*.rs` autodiscovery."
    verification:
      - kind: other
        ref: "grep -c 'insta::' tests/skills_integration.rs -> 0"
        status: pass
      - kind: other
        ref: "cargo test --features 'skills,streamable-http,http-client,testing' --no-run 2>&1 | grep -c 'golden/workflow_skill_projection' -> 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> exit 0, all four selectors, no zero-count guard tripped"
        status: pass
    human_judgment: false
  - id: D6
    description: "The CHANGELOG entry names `as_skill`, `SkillProjection`, the non-semver-stable render policy, GATE A's shipped `Result<ProjectionOutput>` return shape, the D-10 narrowing (SC-6 warnings have exactly one delivery channel), GATE B's THREE public methods with their registration-order semantics, and the digest-significance of constant key order."
    verification:
      - kind: other
        ref: "grep over CHANGELOG.md:8-140 -> as_skill 9, SkillProjection 2, with_projected_skill_prepend 1, ServerCoreBuilder::with_workflow_skill_prepend 1, ServerBuilder::with_workflow_skill_prepend 1, 'NOT semver-stable' 1, 'Result<ProjectionOutput>' 1, 'exactly one delivery channel' 1, 'digest-significant' 1, 're-pin' 1"
        status: pass
    human_judgment: false
  - id: D7
    description: "The module rustdoc covers all ten required contract points and names `tests/golden/workflow_skill_projection.md` as the tripwire; `ProjectionWarning`'s rustdoc names `SkillDiagnostic`; `as_skill()`'s names the D-15 divergence from `Skill::with_reference`'s panic arm; zero SATD."
    verification:
      - kind: other
        ref: "RUSTFLAGS=\"\" make doc-check (RUSTDOCFLAGS=\"-D warnings\", skills on) -> exit 0, zero rustdoc warnings"
        status: pass
      - kind: other
        ref: "grep -c 'yaml_double_quoted' src/server/skills/projection.rs -> 15 (>= 2)"
        status: pass
      - kind: other
        ref: "grep -rn 'TODO|FIXME|HACK|XXX' src/server/skills/ src/server/workflow/ -> 0; RUSTFLAGS=\"\" make check-todos -> exit 0"
        status: pass
    human_judgment: false
  - id: D8
    description: "ALWAYS/DOCTEST — every required new public item carries a self-gated, asserting doctest, and all three doctest legs were RUN with nonzero counts, including the two no gate leg reaches."
    verification:
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features 'skills,full' --doc sequential -- --test-threads=1 -> 7 passed (includes SequentialWorkflow::as_skill, line 284)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features 'skills,streamable-http,http-client,testing' --doc skills -- --test-threads=1 -> 16 passed (was 15 at plan 126-04)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features 'skills,full' --doc skill_prepend -- --test-threads=1 -> 3 passed (both builder setters + the handler setter)"
        status: pass
    human_judgment: false
  - id: D9
    description: "Project quality gates unchanged — fmt clean, skills-module lint clean, PMAT cognitive complexity at the zero-violation baseline, and all four `make test-skills` selectors green with selector 2 moving 15 -> 16 and selector 3 moving 25 -> 26."
    verification:
      - kind: other
        ref: "cargo fmt --all -- --check -> exit 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make lint-skills -> exit 0, 0 warnings"
        status: pass
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> Total violations: 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> 173 / 16 / 26 / 23"
        status: pass
    human_judgment: false

# Metrics
duration: 25 min
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 06: The D-14 golden, the documentation contract, and both doctest legs Summary

**The projected `SKILL.md` bytes are now pinned by a git-tracked golden whose mismatch message forbids a silent re-record and names the CHANGELOG entry a digest-pinning consumer's re-pin requires — and the three obligations waves 1–3 accumulated (GATE A's return-shape deviation, the D-10 narrowing, GATE B's three public methods) are recorded where a reader of the shipped crate will find them, not only in a planning file.**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-09-04T18:56Z
- **Tasks:** 3 executed
- **Files changed:** 7 (2 created, 5 modified) — +491/-14

## Accomplishments

### 1. The D-14 golden (`fc4e1f27`)

`tests/golden/workflow_skill_projection.md` (1,237 bytes, LF, single trailing newline) records the exact render of a fixture built to exercise the whole D-11 universe in one value. Every element is load-bearing for what the golden can catch:

| Fixture element | What it pins |
|---|---|
| name `refund_flow` | `slugify`'s output, not an already-legal passthrough |
| description `Process a customer refund: policy #7` | a NON-TRIVIAL `yaml_double_quoted` output — a `: ` mapping indicator and a `#` comment indicator (REVIEWS finding 1) |
| one workflow instruction | `## Context`, the only place `SequentialWorkflow::instruction()` becomes observable on any surface |
| one typed required + one untyped optional argument | both `## Inputs` bullet shapes and the lowercase wire spelling of the type hint |
| step 1 | two argument bindings, a `Constant` source (construction-order key emission), a result binding |
| step 2 | guidance, a `StepOutput` source, and two template bindings inserted `zeta_total` first so sorted order is observable |
| step 3 | the resource-only step shape and an attached resource |

The comparison is `include_str!`-based, deliberately not `insta`. Anti-vacuity runs before the equality: `len > 400`, the encoded frontmatter prefix, the escaped `description:` line, exactly one trailing newline, and `alpha_reason` before `zeta_total`.

`golden_break_message` follows `wire_break_message`'s three properties (`tests/embedded_resource_golden.rs:360-374`): a named `fn` with a rustdoc saying why it is factored out, naming the file to edit, and stating what the operator must NOT do. Its third clause states the two causes of a red golden and that they need opposite fixes, so re-recording resolves the symptom without deciding which.

### 2. The CHANGELOG and the documentation contract (`54e0298a`)

The new `## [2.20.0] - Unreleased` section records the projection surface and — this plan's inherited obligation — all three cross-AI review resolutions, in the shipped crate's own changelog where a consumer will read them:

1. **GATE A.** `build()` returns `Result<ProjectionOutput>`, not `Result<Skill>`: D-02's literal wording and D-10's structured warnings are incompatible as written, and a `#[non_exhaustive]` named struct is additive under a future field where a tuple is positional.
2. **The D-10 narrowing.** SC-6 gate warnings have exactly ONE delivery channel — `build()`'s structured return with `with_tools` annotations. `as_skill()` cannot *compute* the warning, so it is phrased as a capability boundary rather than a limitation.
3. **GATE B's three public methods.** `WorkflowPromptHandler::with_projected_skill_prepend`, `ServerCoreBuilder::with_workflow_skill_prepend`, `ServerBuilder::with_workflow_skill_prepend` — all default-off, with the builder setters' registration-order semantics spelled out and a worked example.

Plus the digest-significance point 126-02 flagged: constant key order is preserved deliberately (a sorted render would disagree with the call it documents) and is therefore a consumer-visible change, while template-binding order is sorted for cross-process reproducibility.

The module rustdoc gained the eight points it was missing — D-12, D-13, the two independent reasons frontmatter encoding is unconditional (plus `prop_frontmatter_roundtrips` and its `parse_frontmatter_value` oracle), the deliberate absence of a description length bound, the constant-literal rule for every `#[non_exhaustive]` catch-all arm, the D-10 narrowing, GATE B's builder reachability, and the `instruction()` observation — and now names the golden as the D-14 tripwire and `sc3_excluded_execution_mechanics_change_no_byte` as the exclusion pin.

### 3. Both doctest legs, and a third that was also dark (`1994785b`)

Every required item carries a self-gated asserting doctest. All three legs were run explicitly and recorded:

| Leg | Command | Result |
|---|---|---|
| `workflow::sequential` | `--features "skills,full" --doc sequential` | **7 passed** (includes `SequentialWorkflow::as_skill`, line 284) |
| `skills::` (selector 2) | `--features "skills,..." --doc skills` | **16 passed** (was 15) |
| `skill_prepend` | `--features "skills,full" --doc skill_prepend` | **3 passed** |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical] `.gitattributes` did not exist; the golden had no LF guarantee**

- **Found during:** Task 1
- **Issue:** `include_str!` reads the working tree. On a checkout with `core.autocrlf=true` every line ending in `tests/golden/workflow_skill_projection.md` would differ from the render, failing `golden_render_is_byte_equal` for a reason with nothing to do with the renderer — turning a supply-chain tripwire into a platform-flaky test and inviting exactly the re-record the failure message forbids.
- **Fix:** Created `.gitattributes` with a commented `tests/golden/** text eol=lf` rule.
- **Files modified:** `.gitattributes` (new)
- **Commit:** `fc4e1f27`

**2. [Rule 2 — Missing critical] Both GATE B builder doctests were `rust,no_run` and construction-only**

- **Found during:** Task 3
- **Issue:** Task 3's acceptance criterion requires every new doctest to carry at least one assertion and none to be construction-only. Both `with_workflow_skill_prepend` doctests (inherited from 126-05) were `rust,no_run` with a `# let _ = server;` and no assertion — they compiled and claimed nothing.
- **Fix:** Dropped `no_run` so both execute. `ServerBuilder`'s asserts `server.get_prompt("refund_flow").is_some()`, with a comment noting the prompt keeps the workflow's own name while the projected skill is `refund-flow`. `ServerCoreBuilder`'s asserts the projected bytes and adds prose stating that `ServerCore` exposes no synchronous prompt accessor and naming `server_core_builder_prompt_workflow_reaches_the_prepend` as the end-to-end proof — a weaker assertion presented without that pointer would read as the whole claim.
- **Files modified:** `src/server/builder.rs`, `src/server/mod.rs`
- **Verification:** `--features "skills,full" --doc skill_prepend` → 3 passed
- **Commit:** `1994785b`

**3. [Rule 2 — Missing critical] `ProjectionOutput::into_parts` had no doctest, and the `--doc skills` count would otherwise not have moved**

- **Found during:** Task 3
- **Issue:** The `--doc skills` verify block's `fails_when` requires the count to increase over 126-04's recorded 15. Every item in Task 3's *required* doctest set already had one from 126-04, so the count would have stayed at 15 — a `fails_when` trip caused by upstream diligence rather than by a gap. Recording an unchanged count against it would have been dishonest; padding with a vacuous doctest would have been worse.
- **Fix:** Added a doctest to `ProjectionOutput::into_parts`, a public method this phase introduced that genuinely lacked one. It demonstrates the thing a downstream crate cannot otherwise do: `#[non_exhaustive]` forbids struct-pattern destructuring outside the defining crate, so `into_parts()` is the only way to move both halves out at once.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `--doc skills` 15 → 16
- **Commit:** `1994785b`

**Total deviations:** 3 auto-fixed (3 × Rule 2). **Impact:** all three strengthen the artefacts the plan asked for rather than working around them; none changed a rendered byte, and the golden recorded in Task 1 is unaffected by Tasks 2 and 3.

## Sensitivity, measured rather than assumed

Task 1 is test-only, so its `tdd="true"` RED gate cannot honestly fail. Following 126-02's pattern, the pin was proven able to fail by injecting the defect it claims to catch — twice, because the first mutation revealed that the anti-vacuity guards can shadow the comparison they protect:

| Mutation | Result |
|---|---|
| `policy #7"` → `policy #8"` in the golden | RED — but on the escaped-description anti-vacuity guard, so the byte comparison itself stayed unmeasured |
| `Judgment: Confirm` → `Judgment: confirm` (a byte no guard covers) | RED through the byte comparison, printing `golden_break_message` verbatim |

Both reverted; `cmp` against the pre-mutation copy clean each time.

## Verification Results

| Check | Result |
|---|---|
| `cargo test --test skills_integration` | **26 passed** (was 25) |
| `--features "skills,full" --doc sequential` | **7 passed** |
| `--features "skills,..." --doc skills` | **16 passed** (was 15) |
| `--features "skills,full" --doc skill_prepend` | **3 passed** |
| `make doc-check` (`RUSTDOCFLAGS="-D warnings"`) | exit 0, zero rustdoc warnings |
| `make check-todos` | exit 0, no SATD |
| `make test-skills` | **173 / 16 / 26 / 23**, all four selectors, no zero-count guard tripped |
| `make lint-skills` | exit 0, zero warnings |
| `cargo fmt --all -- --check` | exit 0 |
| `pmat quality-gate --checks complexity` | Total violations: 0 |
| `git ls-files -- tests/golden/...md \| grep -c .` | 1 (tracked) |
| autodiscovery check (`--no-run \| grep -c golden/...`) | 0 (not compiled as a test target) |

## Known Stubs

None. `ProjectionWarningKind::SlugFallback` remains declared-but-unemittable-from-`build()`, which is API vocabulary rather than a stub: its own rustdoc states that `build()` rejects the input `as_skill()` substitutes for, and that the variant exists so the warning vocabulary covers the conditions the infallible path resolves and so a future lenient mode is additive rather than breaking. This SUMMARY does not describe it as reachable.

## Threat Flags

None. This plan added a test fixture, a CHANGELOG entry and documentation; it introduces no network endpoint, no auth path, no file access pattern and no schema change. Its one security-relevant surface — the rendered bytes as a supply-chain pin (T-126-06, T-126-16) — is the threat this plan mitigates rather than one it introduces.

## Issues Encountered

None blocking. Two notes for plan 126-07:

1. **A third dark doctest leg.** Plan 126-06 was scoped to two legs; there are three. Neither `--doc skills` (the `skill_prepend` paths contain `skill`, not `skills`) nor `test-doc`'s `--features "full"` (under which the items do not exist) reaches `ServerCoreBuilder::with_workflow_skill_prepend`, `ServerBuilder::with_workflow_skill_prepend` or `WorkflowPromptHandler::with_projected_skill_prepend`. It was run explicitly here (3 passed). **126-07's `deferred-items.md` should record the Makefile-selector extension as covering all three legs, not just the `workflow::sequential` one gemini Finding 5 named.**
2. **`Cargo.toml` still reads `2.19.3`** while the CHANGELOG now carries a `## [2.20.0] - Unreleased` section. That is intentional — version bumps belong to CLAUDE.md's release procedure and `Cargo.toml` is outside this plan's `files_modified` — but a releaser must bump it before tagging `v2.20.0`, or `release.yml`'s note extraction will not find a matching section.

## Next Phase Readiness

Plan 126-07 (the `s56_workflow_skill_projection` example, `deferred-items.md`, the full gate and the manual verification pass) is unblocked. It inherits:

- The golden as a live tripwire: if the example's own render diverges, `golden_render_is_byte_equal` is where it surfaces.
- 126-05's standing instruction that the example must drive the opt-in through the **builder** (`.with_workflow_skill_prepend(true).prompt_workflow(wf)?`), not by hand-constructing a `WorkflowPromptHandler` — the builder path is what a real server uses, and both builder doctests now demonstrate exactly that call, executing.
- The three-leg doctest note above.

## Self-Check: PASSED

- `tests/golden/workflow_skill_projection.md` — FOUND
- `.gitattributes` — FOUND
- Commit `fc4e1f27` — FOUND
- Commit `54e0298a` — FOUND
- Commit `1994785b` — FOUND
- All plan-level `<verification>` commands re-run at the final tree state: all pass (table above)
