---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 02
subsystem: api
tags: [sep-2640, skills, workflow, projection, determinism, proptest, tdd, mutation-testing]

# Dependency graph
requires:
  - phase: 126-01
    provides: "`src/server/skills/projection.rs` (the tracer renderer), `SequentialWorkflow::as_skill()`, the `yaml_double_quoted` frontmatter encoder, the locked body layout, and the crate-private `project_with_notices` seam"
provides:
  - "the complete D-11 render universe — `render_context`, `render_prompt_content`, `render_inputs`, `prompt_argument_type_name`, `render_data_source`, and an extended `render_step`"
  - "template-binding determinism: `render_step` collects `&HashMap` into a `BTreeMap` before iterating, closing the one nondeterministic accessor in the whole input surface"
  - "SC-3 string-by-string coverage (23 per-fact assertions) plus the D-11 exclusion byte-equality pin"
  - "SC-2 determinism proven by a DETERMINISTIC permutation test (primary) plus a 256-case fresh-reconstruction loop (supplemental)"
  - "SC-1's slug legality as a property over arbitrary `String` names, and SC-5's `as_prompt_text() == body` plus tool-name SET equality"
  - "the rendered bytes plan 126-06's golden will pin — the render is now complete, so the golden can be recorded against a stable surface"
affects:
  - 126-03
  - 126-04
  - 126-05
  - 126-06
  - 126-07

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized diff), NOT a harness token count.
actuals:
  tokens: 12160
  tasks: 3
  commits: 5

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mutation verification replaces a degenerate RED gate. Tasks 2 and 3 are test-only, so RED/GREEN collapses — the behaviour they pin already landed in Task 1. Instead of faking a red, each load-bearing pin was proven non-vacuous by INJECTING the defect it claims to catch (an `is_retryable()` line into `render_step`; a `Vec` in place of the `BTreeMap`), confirming the test goes red, then reverting. A test that has never been observed failing is a test whose sensitivity is unmeasured."
    - "Deterministic permutation over probabilistic repetition. The PRIMARY SC-2 proof builds two workflows differing ONLY in template-binding insertion order and compares bytes; it fails 100% of the time against unsorted iteration, on the first run, with no seed dependence. The 100x fresh-reconstruction loop is retained as supplemental, not as the claim."
    - "`#[allow(unreachable_patterns)]` is what makes a `#[non_exhaustive]` D-14 tripwire possible INSIDE the defining crate. Within its own crate a `#[non_exhaustive]` enum is exhaustive, so the mandated `_ =>` constant-literal arm reads as unreachable and `-D warnings` rejects it. The allow buys the future-variant guard."
    - "Every `#[allow]` is measured before it is kept. The `clippy::cognitive_complexity` allow written defensively over the SC-3 suite was removed after measuring that neither clippy nor PMAT fires without it — an allow that suppresses nothing tells a later reader the function is near a cap it is not near."

key-files:
  created: []
  modified:
    - src/server/skills/projection.rs

key-decisions:
  - "Constant key order is NOT sorted, and the rustdoc says so in D-14 terms. `serde_json` is built with `preserve_order`, so the rendered constant matches what the workflow will actually SEND; sorting the render would make the manual procedure disagree with the call it documents. `constant_key_order_is_digest_significant` pins the difference so it stays a decision rather than an accident someone later 'fixes'."
  - "`PromptArgumentType` renders through an explicit four-arm match to `string`/`number`/`integer`/`boolean` — its own `#[serde(rename_all = \"lowercase\")]` wire spellings. The type has NO `Display` impl, so `{:?}` is the reflex reach; a test asserts the capitalised `Debug` spellings appear nowhere in the body."
  - "The D-11 exclusion claim is carried by BYTE EQUALITY across the two settings, not by accessor-name absence. The fixture is built twice — both excluded accessors at defaults vs both non-default — and the bodies are compared byte-for-byte. The name-absence check is demoted to a supplementary readability guard."
  - "`render_step` renders in a fixed order: tool line, argument bindings, template bindings (BTreeMap-sorted), resources, result binding, guidance. The order is arbitrary but now load-bearing — plan 126-06's golden pins it, and changing it is a D-14 re-pin event."
  - "The SC-5 tool-name parse is scoped to the `## Procedure` slice before scanning. A `PromptContent::ToolHandle` instruction renders a backticked tool name into `## Context`, so an unscoped scan passes today and breaks the first time the fixture gains one."

patterns-established:
  - "Anti-vacuity BEFORE every negative or equality assertion, never after: `body.len() > 400` and the `---\\nname: \"refund-flow\"\\n` frontmatter prefix (quotes included — 126-01 Task 4 encodes the value) run first, because two empty bodies are also byte-equal and an empty body satisfies every `!contains`."
  - "One fact, one assertion, one failure message naming that fact. SC-3 forbids a length or omnibus-substring heuristic, so the coverage test is a flat sequence of 23 independent `assert!`s rather than a loop over a needle list — a loop would report 'a fact was missing' instead of WHICH."

requirements-completed: [SC-2, SC-3, SC-5]

coverage:
  - id: D1
    description: "SC-3 / D-11 — every workflow fact renders: name, description, each argument's name + description + required flag, each step's tool name, each argument binding and its data source, each `with_guidance` line, each attached resource, each template binding, and the workflow-level instruction — asserted string-by-string with 23 independent per-fact assertions."
    requirement: "SC-3"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc3_every_workflow_fact_renders"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{context_renders_instruction_text_verbatim,inputs_render_name_description_and_requiredness,an_optional_argument_renders_optional_not_required,step_arguments_render_every_name_and_source,a_resource_only_step_renders_its_heading_and_every_resource,guidance_renders_a_judgment_line}"
        status: pass
    human_judgment: false
  - id: D2
    description: "D-11 exclusions (STRENGTHENED per REVIEWS fable (f)) — a workflow with BOTH `with_task_support` and `retryable` set non-default renders a body BYTE-EQUAL to the same workflow with both at their defaults. Anti-vacuity (`len > 400` + frontmatter prefix) runs before the equality."
    requirement: "SC-3"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc3_excluded_execution_mechanics_change_no_byte"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc3_excluded_accessor_names_are_absent (supplementary)"
        status: pass
      - kind: other
        ref: "MUTATION: injecting `if step.is_retryable() { out.push_str(\"MUTANT retryable\\n\"); }` into render_step turns BOTH exclusion tests red (2 failed), then reverts clean"
        status: pass
    human_judgment: false
  - id: D3
    description: "SC-2 PRIMARY (deterministic, REVIEWS codex) — two workflows differing ONLY in template-binding insertion order over three keys render byte-equal bodies. Single-run, no seed dependence."
    requirement: "SC-2"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc2_binding_insertion_order_does_not_change_bytes"
        status: pass
      - kind: other
        ref: "MUTATION: replacing render_step's `BTreeMap` with a `Vec` collected straight from the `HashMap` turns it red on the FIRST run (measured: `alpha, zeta, middle` vs `alpha, middle, zeta`), then reverts clean"
        status: pass
    human_judgment: false
  - id: D4
    description: "SC-2 SUPPLEMENTAL — 256 proptest cases each CONSTRUCT A FRESH `kitchen_sink_workflow()` (which carries template bindings) and compare the rendered body against the first render in the process."
    requirement: "SC-2"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::prop_sc2_rerender_is_byte_equal"
        status: pass
    human_judgment: false
  - id: D5
    description: "SC-1 (property half) — for arbitrary `String` workflow names, the projected skill name is 1..=64 chars drawn only from `[a-z0-9-]`, with no leading/trailing hyphen and no `--`; the same name always yields the same slug; and `as_skill()` never panics on arbitrary name / description / guidance text (D-15)."
    requirement: "SC-1"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{prop_sc1_slug_is_agentskills_legal,prop_sc1_slug_is_deterministic,prop_no_panic_on_arbitrary_text}"
        status: pass
    human_judgment: false
  - id: D6
    description: "SC-5 — `skill.as_prompt_text() == skill.body()` on the kitchen-sink projection, guarded by `references().count() == 0`, `ends_with('\\n')`, `!ends_with(\"\\n\\n\")` and `len() > 200` asserted BEFORE the equality."
    requirement: "SC-5"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc5_prompt_text_equals_body"
        status: pass
    human_judgment: false
  - id: D7
    description: "SC-5 (surface equivalence) — the SET of tool names in the rendered `## Procedure` slice EQUALS `wf.steps().filter_map(WorkflowStep::tool)`, asserted in both directions. The first set is derived by parsing the RENDERED text, scoped to the Procedure section."
    requirement: "SC-5"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::sc5_surface_equivalence_is_set_equality"
        status: pass
    human_judgment: false
  - id: D8
    description: "REVIEWS gemini finding 4 — two semantically identical JSON constants with different key insertion order render DIFFERENTLY, by design; the behaviour is pinned by a test and documented as digest-significant / CHANGELOG-worthy in `render_data_source`'s rustdoc."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{a_constant_renders_in_construction_order,constant_key_order_is_digest_significant}"
        status: pass
      - kind: other
        ref: "grep -c 'digest-significant' && grep -c 'CHANGELOG' src/server/skills/projection.rs -> both nonzero"
        status: pass
    human_judgment: false
  - id: D9
    description: "REVIEWS fable (d) / T-126-09 — `PromptArgumentType` renders through an explicit four-arm literal mapping, never `Debug`; both `#[non_exhaustive]` catch-all arms emit a CONSTANT literal so a future upstream variant cannot silently move the golden."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::typed_arguments_render_the_lowercase_wire_spellings (asserts `Integer`/`Boolean`/`Number`/`String` are ABSENT)"
        status: pass
      - kind: other
        ref: "grep -n '_ => \"' src/server/skills/projection.rs -> 2 hits, both single string literals; no `{:?}` in the render path"
        status: pass
    human_judgment: false
  - id: D10
    description: "T-126-03 — `PromptContent::Image`'s base64 `data` is never rendered; only the MIME type reaches the body."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::an_image_instruction_renders_only_its_mime_type"
        status: pass
    human_judgment: false
  - id: D11
    description: "D-12 / D-13 — no tool description or input schema reaches the body, and the frontmatter parses to exactly the two keys `name` and `description`."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::{sc3_no_tool_schema_or_description_reaches_the_body,sc3_frontmatter_carries_exactly_two_keys}"
        status: pass
    human_judgment: false
  - id: D12
    description: "Project quality gates unchanged — PMAT cognitive complexity at the zero-violation baseline, skills-module lint clean, rustdoc zero-warning, zero SATD, `skills` still absent from `full`, and all four `make test-skills` selectors green with the `--lib skills` count moving 125 -> 152."
    verification:
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> Total violations: 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make lint-skills -> exit 0, 0 warnings"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> exit 0 (all four guarded selectors, no zero-count guard tripped)"
        status: pass
      - kind: other
        ref: "make doc-check -> exit 0, 0 rustdoc warnings; make check-todos -> exit 0"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features \"full\" --test v1_severability_tripwire -- --test-threads=1 (18 passed)"
        status: pass
    human_judgment: false

# Metrics
duration: 1h 27m
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 02: Full render coverage and the SC-2/SC-3/SC-5 lockdown Summary

**The tracer renderer is now the complete D-11 universe — Context, Inputs and full per-step detail — with the one nondeterministic accessor in the whole input surface (`template_bindings()`'s `&HashMap`) sorted through a `BTreeMap`, and with every claim in the plan's `must_haves` pinned by a test that was proven able to fail by injecting the defect it catches.**

## Performance

- **Duration:** 1h 27m active (10h 8m wall clock — the session was suspended for ~8h45m between the Task 1 GREEN commit and the start of Task 2; no work occurred in that window)
- **Started:** 2026-09-04T04:59:26Z
- **Completed:** 2026-09-04T15:08:07Z
- **Tasks:** 3 executed
- **Files modified:** 1 (`src/server/skills/projection.rs`, +1059/-10)

## Accomplishments

- **The render is complete and the golden surface is stable.** `render_body` now composes six leaves instead of three: `render_context`, `render_inputs`, `render_procedure` (with a much richer `render_step`), plus the pre-existing frontmatter, title and closing. Every accessor in `126-RESEARCH.md` Q2's inventory except the two D-11 exclusions now reaches the body. Plan 126-06 can record its golden against a render that will not move again inside this phase.
- **The determinism landmine is closed, and the closure is proven.** `render_step` collects `template_bindings()` into a `BTreeMap<&str, &DataSource>` before iterating. The proof is a **deterministic permutation test**, not a probabilistic loop: two workflows whose bindings were inserted in opposite orders over three keys must render byte-equal bodies. Replacing the `BTreeMap` with a `Vec` collected straight from the `HashMap` turns it red on the first run — measured, then reverted.
- **The D-11 exclusion claim now measures something.** REVIEWS fable (f) was right that asserting an accessor's NAME is absent proves little, since the render was never going to print an identifier. The pin is byte equality across the two settings: the kitchen-sink fixture is built twice, once with `with_task_support(false)` + `retryable(false)` and once with both `true`, and the bodies are compared byte-for-byte behind an anti-vacuity guard. Injecting an `is_retryable()` line into `render_step` turns both exclusion tests red.
- **SC-3 is asserted string-by-string, as its wording demands.** `sc3_every_workflow_fact_renders` is a flat sequence of 23 independent `assert!`s, each with a failure message naming the ONE fact it covers. A loop over a needle list would have been shorter and would have reported "a fact was missing" instead of which one — which is exactly the heuristic SC-3 forbids.
- **The `PromptArgumentType` trap (REVIEWS fable (d)) is closed at both ends.** `prompt_argument_type_name` is an explicit four-arm match to the type's own `#[serde(rename_all = "lowercase")]` wire spellings, and `typed_arguments_render_the_lowercase_wire_spellings` asserts the capitalised `Debug` spellings — `Integer`, `Boolean`, `Number`, `String` — appear NOWHERE in the body. The type has no `Display` impl, so `{:?}` is the reflex reach and the assertion is what stops a later reader taking it.
- **Gemini's finding 4 became a documented decision instead of an accident.** Constant key order is deliberately not sorted; `render_data_source`'s rustdoc states in D-14 terms that constant key order is **digest-significant** and **CHANGELOG-worthy**, with the reasoning (`preserve_order` means the render matches what will actually be SENT, so sorting would make the procedure disagree with the call). `constant_key_order_is_digest_significant` pins the difference.
- **Projection test count moved 26 -> 53, and `--lib skills` moved 125 -> 152**, with all four `make test-skills` selectors green and no zero-count guard tripped. Every project gate stayed at baseline: PMAT `Total violations: 0`, `make lint-skills` zero warnings, rustdoc zero warnings, zero SATD, `v1_severability_tripwire` 18 passed (so `skills` still has not leaked into `full`).

## Task Commits

1. **Task 1 (RED): failing tests for the full D-11 render universe** — `6725a0db` (test) — measured RED: 28 passed, 13 failed
2. **Task 1 (GREEN): render the full D-11 workflow-fact universe** — `6e21b2d9` (feat) — 41 passed
3. **Task 2: SC-3 string-by-string coverage and the D-11 exclusion pin** — `cbfa652d` (test) — 46 passed
4. **Task 3: SC-2 determinism, SC-1 slug legality, SC-5 dual surface** — `b4735009` (test) — 53 passed
5. **Cleanup: drop a redundant clippy allow** — `f94596c0` (refactor)

_Task 1 carried `tdd="true"` and ran a genuine RED/GREEN cycle. No REFACTOR commit was needed — the one clippy finding (`doc_markdown`) was folded into GREEN before commit rather than deferred. Tasks 2 and 3 are test-only; see **Deviations** for how their degenerate RED gate was handled._

## Files Created/Modified

- `src/server/skills/projection.rs` — **modified** (+1059/-10). New module-private helpers: `render_prompt_content`, `render_context`, `prompt_argument_type_name`, `render_inputs`, `render_data_source`. `render_step` extended with per-argument bindings, `BTreeMap`-sorted template bindings, attached resources and a `Judgment:` guidance line. `render_body` gained two `push_str` calls. 27 new tests: 15 breadth units (Task 1), 5 SC-3 / D-11 / D-12 / D-13 tests (Task 2), and 7 property/dual-surface tests (Task 3). `render_frontmatter` and `yaml_double_quoted` are byte-for-byte untouched — plan 126-01 Task 4 owns those, and touching them would move plan 126-06's golden.

## Decisions Made

### The rendered per-step order is now load-bearing

`render_step` emits, in order: the tool line, argument bindings, template bindings, resources, the result binding, the guidance line. The ordering itself is arbitrary — but from this commit forward it is pinned by 53 tests and, from plan 126-06, by a golden and a published digest. Changing it is a D-14 re-pin event, not a cosmetic edit.

### Two `#[allow]`s, one kept and one removed, both measured

The plan mandates a `_ =>` constant-literal arm on every `PromptContent` and `DataSource` match (T-126-09). Within the defining crate a `#[non_exhaustive]` enum is exhaustive, so that arm reads as unreachable and the Makefile's own `RUSTFLAGS = -D warnings` rejects it — measured: removing the two `#[allow(unreachable_patterns)]` fails `make lint-skills` with two `error: unreachable pattern`. The allow is what makes the D-14 tripwire possible at all, and both carry a `// Why:` comment saying so.

A third allow, `#[allow(clippy::cognitive_complexity)]` written defensively over the 23-assertion SC-3 test, was measured and **removed**: neither clippy nor PMAT fires without it. Its explanatory comment stays, because the design note (a flat assertion sequence is the right shape for per-fact failure messages) is real independent of any lint.

### Mutation verification stands in for a degenerate RED gate

See **Deviations**.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `clippy::doc_markdown` on a quoted identifier in a doc comment**

- **Found during:** Task 1 (`make lint-skills` gate run)
- **Issue:** `render_inputs`' rustdoc quoted the plan's own phrase `"typed_argument schema information"`. `make lint-skills` runs with `-W clippy::pedantic` under `RUSTFLAGS = -D warnings`, so `doc_markdown` promoted it to `error: item in documentation is missing backticks` and the lint leg exited 101. The baseline was warning-free, so this was attributable to the new code, and CLAUDE.md is zero-tolerance.
- **Fix:** Backticked the identifier — `` "`typed_argument` schema information" ``.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `RUSTFLAGS="" make lint-skills` exits 0 with zero warnings.
- **Committed in:** `6e21b2d9`

**2. [Rule 2 - Missing Critical] Added `prompt_argument_type_name`, not in the plan's artifact inventory**

- **Found during:** Task 1
- **Issue:** `<artifacts_this_phase_produces>` enumerates the module-private functions this phase creates, and the four-arm `PromptArgumentType` mapping the plan mandates in prose had no named home. Inlining the match inside `render_inputs` would have buried the D-14 rationale (why not `{:?}`) in the middle of a formatting loop, where the next reader most needs it and is least likely to look.
- **Fix:** One small pure helper returning `&'static str`, carrying the full rationale in its doc comment: no `Display` impl exists, the literals are the type's own serde wire spellings, and the enum is not `#[non_exhaustive]` so the exhaustive match is the desired compile-time tripwire.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `typed_arguments_render_the_lowercase_wire_spellings` passes; `grep -c 'PromptArgumentType::'` returns 8 (4 in the mapping, 4 in the test), over the required 4.
- **Committed in:** `6e21b2d9`

**3. [Rule 2 - Missing Critical] `#[allow(unreachable_patterns)]` on both `#[non_exhaustive]` catch-all arms**

- **Found during:** Task 1
- **Issue:** The plan mandates a `_ =>` constant-literal arm on the `PromptContent` and `DataSource` matches (T-126-09: a future upstream variant must not reach a `{:?}` fallback and silently move every pinned digest). But `PromptContent` and `DataSource` are defined in THIS crate, and `#[non_exhaustive]` only constrains downstream crates — so within `pmcp` both matches are exhaustive and the mandated arm is an `unreachable_pattern`. The Makefile assigns `RUSTFLAGS = -D warnings` internally, and a recursive Make variable wins over the environment, so `RUSTFLAGS="" make lint-skills` does NOT disarm it.
- **Fix:** `#[allow(unreachable_patterns)]` on each arm with a `// Why:` comment naming the D-14 tripwire it preserves.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** Measured both ways — with the allows, `make lint-skills` exits 0; without them it fails with two `error: unreachable pattern`. The allow is load-bearing, not decorative.
- **Committed in:** `6e21b2d9`

**4. [Rule 1 - Bug] A defensively-written `#[allow]` that suppressed nothing**

- **Found during:** Post-Task-3 gate review
- **Issue:** `#[allow(clippy::cognitive_complexity)]` was placed on `sc3_every_workflow_fact_renders` on the assumption that 23 sequential assertions would trip a complexity cap. It does not. An allow that suppresses nothing is actively misleading — it tells the next reader the function is near a limit it is nowhere near, and invites a needless decomposition that would destroy SC-3's per-fact failure messages.
- **Fix:** Removed the attribute; kept the explanatory comment, which documents a real design choice independent of any lint.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** With it removed, `RUSTFLAGS="" make lint-skills` exits 0 with zero warnings and `pmat quality-gate --checks complexity` reports `Total violations: 0`.
- **Committed in:** `f94596c0`

---

**Total deviations:** 4 auto-fixed (2 missing-critical, 1 blocking, 1 bug)
**Impact on plan:** None on scope. All four are local to the single file the plan assigned. No architectural decision was taken; no gate was weakened.

### Recorded, not a deviation: the degenerate RED gate on Tasks 2 and 3

Tasks 2 and 3 both carry `tdd="true"`, but both are **test-only** — every behaviour they pin was implemented by Task 1's GREEN. Writing them and running them therefore produced an immediate green, and there was no honest way to manufacture a RED.

Rather than record an unmeasured pass, each load-bearing pin was verified by **injecting the exact defect it claims to catch** and confirming it goes red:

| Pin | Injected defect | Result |
|---|---|---|
| `sc3_excluded_execution_mechanics_change_no_byte` + `sc3_excluded_accessor_names_are_absent` | `if step.is_retryable() { out.push_str("MUTANT retryable\n"); }` in `render_step` | **2 failed**, with the byte diff shown; reverted clean |
| `sc2_binding_insertion_order_does_not_change_bytes` | `BTreeMap` -> `Vec` collected straight from the `HashMap` | **1 failed** on the first run (`alpha, zeta, middle` vs `alpha, middle, zeta`); reverted clean |

This is the honest substitute for RED at a task where the implementation already exists: a test never observed failing is a test whose sensitivity is unmeasured, and this phase's whole value is that its assertions bite.

## Known Stubs

None. No placeholder, no hardcoded empty value, and no unwired surface was introduced. The plan's scope was the render plus its tests, and both are complete.

## Known Gaps (planned, owned by later plans)

Unchanged from plan 126-01's list except that the two gaps this plan owned are now closed:

- ~~`render_step` renders tool and binding only~~ — **closed here.**
- ~~`render_context` and `render_inputs` do not exist~~ — **closed here.**
- `SkillProjection`, `ProjectionWarning`, `ProjectionWarningKind`, `ProjectionOutput` and the `pub use projection::{...}` re-export remain absent — plan **126-04** (GATE A).
- The SC-4 **wire** half (loopback `StreamableHttpServer`, `skills/list` entry manifest) — plan **126-03**.
- The D-14 golden, the fuzz target, the `s56` example and the CHANGELOG — plans **126-03/06/07**.
- The D-04a prompt prepend and its builder path — plan **126-05** (GATE B).

## Threat Flags

None. This plan added no network endpoint, no auth path, no file access and no schema change. The renderer remains a pure function over server-author-supplied data with no I/O. The two `mitigate` dispositions in the plan's register that this plan owned are both implemented and tested: T-126-03 (`Image` base64 never rendered — `an_image_instruction_renders_only_its_mime_type`) and T-126-09 (constant-literal catch-all arms — `grep -n '_ => "'` returns exactly two hits, both single string literals).

## Issues Encountered

- **`rtk` truncates piped `git diff` and `make` output**, which is the recorded project gotcha and it bit twice here. `git diff … | wc -c` reported **15,975** bytes where the absolute binary `/usr/bin/git` reported **48,639** — a 3x undercount that would have put a badly wrong `actuals.tokens` in this frontmatter. Separately, `make test-skills`'s redirected log arrives truncated mid-test-list, so the per-selector count lines are unreadable; the Makefile's own zero-count guards make the **exit status** the trustworthy signal, and exact counts were taken from a direct `cargo test` invocation instead. Use `/usr/bin/git` and absolute toolchain paths whenever a number is going to be recorded.
- **One transient `error: the -Z unstable-options flag must also be passed to enable the flag check-cfg` while compiling `dashmap`**, on a toolchain that had just compiled the same dependency successfully. Not reproducible on retry with the same feature set and the same toolchain (`rustc`/`cargo` 1.98.0 for both the rustup shim and the direct `stable-aarch64-apple-darwin` binary). Recorded because it looks like a code failure and is not one; retry before investigating.
- **No git hooks are installed** in this checkout (`.git/hooks/` holds only samples), so CLAUDE.md's pre-commit quality gate does not fire. Every gate was run manually before each commit: `cargo fmt --all`, `make lint-skills`, `make doc-check`, `make check-todos`, `make test-skills`, `v1_severability_tripwire`, and `pmat quality-gate --checks complexity`. This is the second plan in a row to report it — worth the phase owner's attention, since a contributor relying on the hook gets no enforcement here.

## Observation (not a defect)

The Task 1 acceptance criterion reads "no `format!` with `{:?}` anywhere in `projection.rs`". The **render path** satisfies this strictly — `grep` finds `{:?}` only in doc comments explaining why it is forbidden, and in test-diagnostic strings (`panic!("frontmatter did not parse cleanly: {other:?}")`, `assert!(name.is_string(), "name parsed as {name:?}")`). All of the latter predate this plan; they were written by plan 126-01 and are failure messages, never rendered bytes. They were left alone deliberately rather than churned, since rewriting another plan's test diagnostics to satisfy a literal reading of a grep would remove real debugging information for no gain.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Ready for plan 126-03** (the SC-4 wire half). The render is complete, so a `skills/list` entry manifest recorded against it will not need re-recording within this phase.
- **Plan 126-04** inherits an unchanged `project_with_notices(&SequentialWorkflow) -> (Skill, Vec<ProjectionNotice>)` seam — this plan touched only the render leaves beneath it, never the seam's shape. GATE A's `ProjectionOutput` still slots straight in.
- **Plan 126-06 can record the golden against stable bytes.** Both prerequisites are now met: 126-01's frontmatter encoding (finding 1) and this plan's complete render. The golden must include the `## Context` and `## Inputs` sections and the full per-step detail; a golden recorded against the tracer's narrow render would be wrong.
- **One constraint 126-06 must carry into the CHANGELOG:** `render_data_source`'s rustdoc declares that constant key order is digest-significant. That is a user-visible property of the projection and belongs in the entry alongside the GATE A deviation 126-01 already booked.
- **No blockers.**

## Self-Check: PASSED

- `src/server/skills/projection.rs` present on disk; `git status --short src/` clean (no uncommitted or leftover mutation edits — both mutation experiments were reverted and verified by `grep -c MUTANT` returning 0 and `grep -c 'BTreeMap<&str, &DataSource>'` returning 1).
- All five commit hashes resolve in `git log --oneline --all`: `6725a0db`, `6e21b2d9`, `cbfa652d`, `b4735009`, `f94596c0`.
- All four plan-level `<verification>` commands re-run green AFTER the final commit: `--lib skills::projection` **53 passed** (> plan 126-01's 26); `make test-skills` **exit 0** (all four selectors, no zero-count guard tripped); `make lint-skills` **exit 0**; `pmat quality-gate --fail-on-violation --checks complexity` **Total violations: 0**.
- Supplementary gates green: `--lib skills` **152 passed** (> 126-01's 125, so the new tests are inside selector 1's reach), `v1_severability_tripwire` **18 passed**, `make doc-check` **exit 0**, `make check-todos` **exit 0**, `cargo fmt --all -- --check` **exit 0**.
- No proptest regression artifact was saved by any run.
