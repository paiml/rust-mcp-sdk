---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
plan: 01
subsystem: api
tags: [sep-2640, skills, workflow, yaml, frontmatter, projection, proptest, tdd]

# Dependency graph
requires:
  - phase: 125-sep-2640-conformance-skills-list-skills-get
    provides: "`Skill` / `Skills` / `SkillReference`, the `into_handler()` build-time choke point, `parse_frontmatter_value` (the serde_yaml seam), `validate_names` name-identity enforcement, `sha256_digest_hex`, and the `mcp.skills` tracing target"
provides:
  - "`src/server/skills/` module directory — `skills.rs` moved to `skills/mod.rs` with git blame intact (D-03)"
  - "`src/server/skills/projection.rs` — the deterministic SequentialWorkflow -> SKILL.md renderer"
  - "`SequentialWorkflow::as_skill(&self) -> Skill`, `#[cfg(feature = \"skills\")]`, infallible (D-01/D-02)"
  - "`yaml_double_quoted` frontmatter encoding — closes T-126-21 / REVIEWS finding 1 (HIGH)"
  - "the crate-private `projection::project_with_notices` seam plan 126-04 wraps"
  - "SC-1 name identity and SC-4's in-process byte-identical read-back, proven"
affects:
  - 126-02
  - 126-03
  - 126-04
  - 126-05
  - 126-06
  - 126-07

# Actuals (#2632) — estimateTokens scale (chars/4 over the realized diff), NOT a harness token count.
actuals:
  tokens: 9525
  tasks: 2
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Hand-written YAML double-quoted encoder, library-verified decoder: `serde_yaml` is the test ORACLE (via the crate's own `parse_frontmatter_value`) rather than the emitter, because D-14 makes the emitted bytes a published digest and an emitter's quoting heuristics are not a contract."
    - "Notice-returning seam + logging wrapper: `project_with_notices() -> (Skill, Vec<ProjectionNotice>)` is the pure crate-private core; `project()` is the thin `tracing::warn!` wrapper. Lets the later fallible builder surface the same conditions as structured data without a second render."
    - "Composer-over-leaves render geometry (`crates/pmcp-cfn-renderer` discipline): `render_body` is a straight push_str sequence over single-purpose `render_*` leaves — keeps PMAT cognitive complexity at the measured `src/` zero-violation baseline."

key-files:
  created:
    - src/server/skills/projection.rs
  modified:
    - src/server/skills/mod.rs
    - src/server/workflow/sequential.rs
    - tests/skills_integration.rs
    - Cargo.toml
    - Makefile

key-decisions:
  - "GATE A (human, LOCKED): `SkillProjection::build()` returns `Result<ProjectionOutput>` with `pub struct ProjectionOutput { skill, warnings }` — additive under future extension, satisfies D-10 literally, deviates from D-02 only in the success type's name."
  - "GATE B (human, LOCKED): `add-builder-path` — plan 126-05 Task 4 adds a `#[cfg(feature = \"skills\")]` field + `#[must_use]` setter to BOTH `ServerCoreBuilder` and `ServerBuilder`, so the D-04a prepend is reachable from `prompt_workflow` and the anti-drift claim holds per SERVER, not merely per workflow value."
  - "Frontmatter values are ALWAYS emitted as escaped YAML double-quoted scalars, never conditionally — `slugify(\"True\")`/`slugify(\"123\")` are legal slugs that parse as a YAML bool/int and defeat `validate_names`' `as_str()` guard, and conditional quoting would make published digest bytes depend on a predicate over author text."
  - "Empty description substitutes the deterministic `Projected from the {slug} workflow.` (derived from the slug, which is already `[a-z0-9-]`, so it needs no escaping of its own) and warns; no description length bound is invented because `sep-2640-conformance.md` states none."
  - "`Skill::with_description` is set explicitly, because `Skill::new` would otherwise derive the description via `strip_prefix(\"description: \") + trim()` with no YAML decoding and surface the encoded scalar — quotes and backslashes intact — in `prompts/list`."

patterns-established:
  - "Panic-free truncation by ordering: `String::truncate(64)` runs only AFTER the ASCII-reducing map, so byte index equals char index. The invariant is stated in a comment at the call site because reordering the steps silently reintroduces a panic."
  - "Fallback hashes the ORIGINAL un-normalized name, never the empty normalized string — the latter collides every failing workflow onto one slug, converting a warn path into a hard duplicate-URI build failure."
  - "Encoder tests assert against the DECODER the production code actually uses (`super::super::parse_frontmatter_value`), not against a hand-written expectation of YAML."

requirements-completed: [SC-1, SC-4]

coverage:
  - id: D1
    description: "D-03 module split — `src/server/skills.rs` is now `src/server/skills/mod.rs` with git blame continuity, `skills/projection.rs` compiles as its child, and no public path moved."
    requirement: "D-03"
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo build -p pmcp --features \"skills,streamable-http,http-client,testing\""
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo build -p pmcp --no-default-features"
        status: pass
      - kind: unit
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features \"skills,...\" --lib skills (99 -> 125 passed; zero lost)"
        status: pass
      - kind: other
        ref: "git log --follow --oneline -- src/server/skills/mod.rs | wc -l == 14 (> 1, blame survived)"
        status: pass
    human_judgment: false
  - id: D2
    description: "SC-1 — a `SequentialWorkflow` named `refund_flow` projects to a `Skill` named `refund-flow`, and the frontmatter `name` equals the final URI segment by construction (no path override is ever set)."
    requirement: "SC-1"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::as_skill_name_is_the_slugified_workflow_name"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::slugify_* (6 cases: underscore, hyphen collapse, all-punctuation, empty, 64-truncation, truncation re-strip)"
        status: pass
      - kind: other
        ref: "grep -c 'with_path' src/server/skills/projection.rs == 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "T-126-21 / REVIEWS finding 1 (HIGH) — frontmatter round-trips to exactly two JSON-string keys for arbitrary descriptions AND for YAML-type-alike slugs, so the silent diagnostic-downgrade path that skips SC-1 is unreachable."
    requirement: "SC-1"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#server::skills::projection::tests::prop_frontmatter_roundtrips (proptest over description in \".*\")"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#frontmatter_survives_{a_mapping_indicator,a_comment_indicator,a_flow_sequence,embedded_quotes,a_leading_dash,a_document_delimiter,a_newline_injection_attempt,a_yaml_boolean_alike_description} (8 adversarial descriptions)"
        status: pass
      - kind: unit
        ref: "src/server/skills/projection.rs#{a_boolean_alike_slug_stays_a_json_string,an_integer_alike_slug_stays_a_json_string} (2 adversarial slugs, both assert is_string())"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#projected_skill_with_an_awkward_description_still_registers_and_reads"
        status: pass
    human_judgment: false
  - id: D4
    description: "SC-4 (in-process half) — the projected skill registers through the real `Skills::into_handler()` and `handler.read(\"skill://refund-flow/SKILL.md\", ...)` returns bytes byte-identical to `skill.body()`."
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "tests/skills_integration.rs#projected_workflow_skill_reads_back_byte_identical"
        status: pass
    human_judgment: false
  - id: D5
    description: "SC-5 preconditions — the rendered body ends in exactly one newline and the projected skill carries zero references, so `as_prompt_text() == body()`."
    requirement: "SC-5"
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#projected_body_shape_satisfies_the_sc5_preconditions"
        status: pass
    human_judgment: false
  - id: D6
    description: "REVIEWS finding 6 — the empty-description fallback and the `prompts/list` half of the encoding fix (`resolved_description()` returns the RAW workflow description, not the encoded scalar)."
    verification:
      - kind: unit
        ref: "src/server/skills/projection.rs#{empty_description_gets_the_deterministic_fallback,resolved_description_is_the_raw_workflow_description}"
        status: pass
    human_judgment: false
  - id: D7
    description: "D-01/D-02 — `SequentialWorkflow::as_skill()` exists behind `#[cfg(feature = \"skills\")]`, is `#[must_use]`, is infallible, and its self-gating doctest actually executes (no gate leg reaches a `workflow::`-path doctest)."
    requirement: "D-01"
    verification:
      - kind: unit
        ref: "cargo test -p pmcp --features \"skills,full\" --doc sequential -- SequentialWorkflow::as_skill (line 261) ... ok"
        status: pass
    human_judgment: false
  - id: D8
    description: "Project quality gates unchanged — PMAT cognitive complexity at the zero-violation baseline, skills-module lint clean, rustdoc zero-warning, zero SATD, and `skills` still absent from `default`/`full`."
    verification:
      - kind: other
        ref: "pmat quality-gate --fail-on-violation --checks complexity -> Total violations: 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make lint-skills -> exit 0, 0 warnings"
        status: pass
      - kind: other
        ref: "RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps --features <doc-check set> -> exit 0"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make check-todos -> No technical debt comments"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp --features \"full\" --test v1_severability_tripwire -- --test-threads=1 (18 passed)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-skills -> exit 0 (all four guarded selectors)"
        status: pass
    human_judgment: false

# Metrics
duration: 41 min
completed: 2026-09-04
status: complete
---

# Phase 126 Plan 01: Workflow-to-skill projection tracer Summary

**`SequentialWorkflow::as_skill()` renders a `refund_flow` workflow into an agentskills-legal `refund-flow` SKILL.md whose frontmatter is YAML-encoded rather than concatenated — closing the HIGH-severity T-126-21 injection that would otherwise have made SC-1's identity check silently unenforced — and that skill registers and reads back byte-identical through the real `ResourceHandler`.**

## Performance

- **Duration:** 41 min
- **Started:** 2026-09-04T04:12:00Z
- **Completed:** 2026-09-04T04:53:12Z
- **Tasks:** 2 executed (Tasks 3 and 4); Tasks 1 and 2 were decision gates discharged by the human before this executor ran
- **Files modified:** 6 (1 created, 1 renamed, 4 modified)

## Accomplishments

- **D-03 module split landed alone and first.** `git mv src/server/skills.rs src/server/skills/mod.rs` with `git log --follow` returning 14 commits, so blame continuity survived the 157 KB move. `src/server/mod.rs` has zero diff — `pub mod skills;` resolves to the directory identically, so no public path moved.
- **The whole architecture is proven on one commit, not ten.** Module split, slugification, renderer composer, `as_skill()` entry point and registry pass-through all execute end to end for a trivial workflow.
- **T-126-21 (HIGH) is closed in wave 1, three waves before the golden is recorded.** `yaml_double_quoted` emits both frontmatter values as escaped YAML double-quoted scalars, unconditionally. The failure it prevents is silent, not loud: `build_artifact_inner` downgrades a frontmatter parse failure to a diagnostic and sets `frontmatter = None`, after which `validate_names` `continue`s — so an ordinary description like `Refund an order: fast path` would have made SC-1's identity check *skipped* rather than *enforced*, with no test going red.
- **The encoder is verified by the decoder production actually uses.** `prop_frontmatter_roundtrips` over `description in ".*"` plus ten named adversarial cases parse the emitted block back through the crate's own `parse_frontmatter_value` (which is `serde_yaml`), asserting exactly two keys, both JSON strings, both equal to their inputs. The two slug cases (`True` -> `true`, `123` -> `123`) assert `is_string()` explicitly — that is the shape `validate_names`' `.and_then(Value::as_str)` guard would otherwise skip.
- **SC-4's in-process half holds.** Two integration tests register a projected skill through the real `Skills::into_handler()` choke point and assert the served bytes equal `skill.body()` exactly; the second uses a `: `/`#`-bearing description and additionally asserts `entries()` builds, which is only reachable if the frontmatter parsed.
- **Skills unit-test count moved 99 -> 125 with none lost**, and every project gate stayed green: PMAT `Total violations: 0`, `make lint-skills` clean, rustdoc zero-warning, zero SATD, `v1_severability_tripwire` still passing (so `skills` did not leak into `full`/`full-v2`).

## Task Commits

1. **Task 3: D-03 module split** — `e851758e` (refactor)
2. **Task 4 (RED): failing tests for the tracer projection** — `10f9ce9e` (test)
3. **Task 4 (GREEN): `as_skill()` implementation** — `785268c0` (feat)

_Task 4 carried `tdd="true"`; no REFACTOR commit was needed — the GREEN implementation required no cleanup pass, and the one clippy `single_match_else` finding was folded into GREEN before commit rather than deferred._

## Files Created/Modified

- `src/server/skills/projection.rs` — **created.** The renderer: `slugify`, `fallback_slug`, `sanitize_for_log`, `yaml_double_quoted`, `fallback_description`, `render_frontmatter`, `render_step`, `render_procedure`, `render_closing`, `resolve_description`, `render_body`, plus the crate-private `project_with_notices` seam, its `project` logging wrapper, the `ProjectionNotice` enum, and 26 unit/property tests.
- `src/server/skills/mod.rs` — **renamed** from `src/server/skills.rs`; gained exactly one item, `pub mod projection;` with a doc line. No `pub use projection::` — that is plan 126-04's, deliberately.
- `src/server/workflow/sequential.rs` — `as_skill(&self) -> Skill` behind `#[cfg(feature = "skills")]` + `#[must_use]`, a thin delegate, with a self-gating doctest and a rustdoc contract stating why it warns-and-falls-back rather than panicking.
- `tests/skills_integration.rs` — two SC-4 in-process tests appended (placed here, not in a new file, because a new integration target is invisible to all four `make test-skills` selectors).
- `Cargo.toml`, `Makefile` — comment-only path corrections at the six sites naming the old `src/server/skills.rs`.

## Decisions Made

### The two human gates (LOCKED — downstream plans must treat these as settled input)

**GATE A — `SkillProjection::build()` returns an output struct.**

```rust
pub struct ProjectionOutput {
    pub skill: Skill,
    pub warnings: Vec<ProjectionWarning>,
}

impl<'w> SkillProjection<'w> {
    pub fn build(self) -> Result<ProjectionOutput>;
}
```

Named and self-documenting; additive, so a later phase can add a field (a manifest, a `strict` verdict) without a breaking signature change; satisfies D-10 exactly and deviates from D-02 only in the success type's *name*, not its fallibility. One-way on a published 2.x crate.

**GATE B — `add-builder-path`.** Plan 126-05 Task 4 adds, to BOTH `ServerCoreBuilder` (`src/server/builder.rs`) and `ServerBuilder` (`src/server/mod.rs`): one `#[cfg(feature = "skills")]` field, one `#[must_use]` chainable setter, and one chained call at `WorkflowPromptHandler` construction inside `prompt_workflow`. Default-off and feature-gated, so flag-off transcripts stay byte-identical. This makes the anti-drift claim hold per SERVER, not merely per workflow value. Also one-way.

### Obligations these decisions create for downstream plans

| Plan | Obligation |
|------|-----------|
| **126-04** | Implement `ProjectionOutput` per GATE A, **and record the departure from D-02's literal `Result<Skill>` wording as an explicit deviation** in its `<decision_deviation>` block and in `126-04-SUMMARY.md`. Wrap — do not rewrite — the `projection::project_with_notices` seam this plan left crate-private; it already returns `(Skill, Vec<ProjectionNotice>)`, which is the shape a warning vector slots into. |
| **126-05** | Implement GATE B's builder path in Task 4. The gate is **answered**, not open — do not re-litigate it. |
| **126-06** | **Name the GATE A deviation in the CHANGELOG entry.** |

### Implementation decisions taken here

- **Always-quote, never conditionally** (see key-decisions). Both reasons are written into `yaml_double_quoted`'s doc comment so a future reader cannot "optimize" the quoting away.
- **`serde_yaml` is the ORACLE, not the emitter.** `serde_yaml::to_string`'s quoting style and line wrapping are emitter heuristics whose exact output is unverifiable at planning time, and D-14 makes these bytes a supply-chain pin.
- **The seam shape.** The plan left the seam "of your choosing"; it is `project_with_notices(&SequentialWorkflow) -> (Skill, Vec<ProjectionNotice>)` (pure, returns conditions as data) plus `project(&SequentialWorkflow) -> Skill` (emits each notice on `mcp.skills`). Both crate-private.
- **`fallback_slug` uses iterator slicing, not byte-range indexing.** `digest.chars().skip(7).take(8)` cannot panic even if the digest format changes underneath it; `&digest[7..][..8]` could.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added `fallback_description` and `resolve_description` helpers not enumerated in the plan's artifact inventory**

- **Found during:** Task 4
- **Issue:** The plan's `<artifacts_this_phase_produces>` lists the module-private functions this phase creates, and the empty-description fallback (step 3d) had no named home. Inlining it at both use sites — `render_body` and `project_with_notices` — would have let the two drift, and the substituted string is asserted byte-for-byte by a test.
- **Fix:** Two small named helpers. `fallback_description(slug)` owns the literal; `resolve_description(wf, slug)` owns the empty-check. Both are pure and called from exactly the two sites that need them.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `empty_description_gets_the_deterministic_fallback` and `prop_frontmatter_roundtrips`' empty-string branch both pass.
- **Committed in:** `785268c0`

**2. [Rule 1 - Bug] Comment wording defeated its own acceptance gate**

- **Found during:** Task 4 (acceptance-criteria verification loop)
- **Issue:** A code comment reading ``NEVER `.with_path(...)`: ...`` explained the SC-1 invariant correctly but contained the literal string `with_path`, which made the acceptance criterion `grep -c 'with_path' src/server/skills/projection.rs` return 1 instead of the required 0. The gate exists to prove the projection never sets a URI path override; a comment about not doing so is not a violation, but the gate cannot tell the difference and a future reader running it would have read a false failure.
- **Fix:** Reworded to "NEVER set a URI path override on a projected skill", preserving the explanation and clearing the gate honestly (the projection genuinely never calls it).
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `grep -c 'with_path' src/server/skills/projection.rs` returns 0.
- **Committed in:** `785268c0`

**3. [Rule 3 - Blocking] Two rustdoc intra-doc links to module-private items broke `make doc-check`**

- **Found during:** Task 4 (gate run)
- **Issue:** The module-level `//!` docs linked `[`yaml_double_quoted`]`, and `ProjectionNotice`'s docs linked `[`project`]`. `make doc-check` runs with `RUSTDOCFLAGS="-D warnings"`, and the module-level link failed to resolve — `error: could not document 'pmcp'`, blocking the gate.
- **Fix:** Replaced the module-level link with a plain code span naming the function as module-private; reworded the `ProjectionNotice` sentence to describe the logging wrapper rather than link it.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <doc-check set>` exits 0 with no output.
- **Committed in:** `785268c0`

**4. [Rule 1 - Bug] `clippy::single_match_else` warning introduced by the slug-fallback match**

- **Found during:** Task 4 (`make lint-skills`)
- **Issue:** The `match slugify(...) { Some => .., None => {...} }` form tripped `clippy::single_match_else` (pedantic). The baseline `make lint-skills` was warning-free, so this was attributable to the new code — and CLAUDE.md is zero-tolerance.
- **Fix:** Rewrote as `if let Some(slug) = slugify(...) { slug } else { ... }`.
- **Files modified:** `src/server/skills/projection.rs`
- **Verification:** `RUSTFLAGS="" make lint-skills` exits 0 with zero warnings.
- **Committed in:** `785268c0`

---

**Total deviations:** 4 auto-fixed (2 bugs, 1 missing critical, 1 blocking)
**Impact on plan:** None on scope. All four are small, local, and inside the files the plan already assigned to this task. No architectural decision was taken outside the two human gates.

### Recorded, not a deviation

The plan's Tasks 1 and 2 were `checkpoint:decision` gates with `gate="blocking-human"`. Both were answered by the human before this continuation executor ran; they carried no non-decision work, so no commit was created for either. Their content is recorded above under **Decisions Made**.

## Known Gaps (planned, owned by later plans)

These are functionality gaps the plan explicitly authorized — "breadth is a functionality gap plan 126-02 fills; it is not an architectural gap" — not stubs, and not defects. They are listed so no reader mistakes the tracer's narrowness for completeness:

- `render_step` renders a step's **tool** and **binding** only. Arguments (`step.arguments()`), guidance, `resources`, and `template_bindings` are absent — plan **126-02** (SC-2/SC-3 breadth). The `template_bindings` `HashMap` determinism landmine therefore has not yet been hit, and 126-02 must sort it (`BTreeMap`) before rendering.
- `render_context` and `render_inputs` do not exist yet — plan **126-02**. They were deliberately NOT stubbed with placeholder comments, because `make check-todos` greps `src/` for SATD markers and zero SATD is a gate.
- `SkillProjection`, `ProjectionWarning`, `ProjectionWarningKind`, `ProjectionOutput` and the `pub use projection::{...}` re-export in `skills/mod.rs` do not exist — plan **126-04**. An acceptance criterion of this task asserts the re-export is absent from *this* commit.
- The SC-4 **wire** half (loopback `StreamableHttpServer`, `skills/list` entry manifest) is plan **126-03**'s; only the in-process half is proven here.
- The D-14 golden, the fuzz target, the `s56` example and the CHANGELOG are plans **126-03/06/07**.

## Issues Encountered

- **`rtk` filtered the doctest output**, so `--doc sequential`'s per-test names were invisible and the run could not be distinguished from one that filtered to zero. Resolved per the project's recorded workaround: re-ran through the absolute toolchain binary (`~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo`), which showed `SequentialWorkflow::as_skill (line 261) ... ok` explicitly. This matters because the plan's `<fails_when>` for that leg specifically warns that no gate leg reaches a `workflow::`-path doctest — a `0 passed` there would have been a silent miss.
- **No git hooks are installed** in this checkout (`.git/hooks/` has only samples), so the CLAUDE.md pre-commit quality gate does not fire automatically. Every gate was therefore run manually before each commit: `cargo fmt --all`, `make lint-skills`, `make doc-check`'s rustdoc invocation, `make check-todos`, `make test-skills`, `v1_severability_tripwire`, and `pmat quality-gate --checks complexity`. Worth flagging for the phase owner — a contributor relying on the hook would get no enforcement here.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Ready for plan 126-02** (wave 2 breadth: Context, Inputs, per-step detail, SC-2 determinism, SC-3 coverage, SC-5 tool-name set equality). The body layout is locked and every later plan extends it without rearranging.
- **Plan 126-04 inherits a settled GATE A** and a crate-private seam already shaped `(Skill, Vec<ProjectionNotice>)` to wrap. It owes the D-02 deviation record.
- **Plan 126-05 inherits a settled GATE B.**
- **Plan 126-06 owes the CHANGELOG mention of the GATE A deviation**, and records the D-14 golden against bytes that already carry the finding-1 encoding — the ordering constraint the reviews called out is satisfied, with the encoding landing three waves ahead of the golden.
- **No blockers.** The one HIGH threat in the register (T-126-21) is closed and pinned by tests that cannot pass while the round-trip suite is absent or red.

## Self-Check: PASSED

- All created/modified files present on disk (`src/server/skills/projection.rs`, `src/server/skills/mod.rs`, `src/server/workflow/sequential.rs`, `tests/skills_integration.rs`); `src/server/skills.rs` confirmed gone.
- All three commit hashes resolve in `git log --oneline --all`: `e851758e`, `10f9ce9e`, `785268c0`.
- All plan-level `<verification>` commands re-run green after the final commit: skills lib 125 passed (> pre-split 99), `skills_integration` 15 passed, `--doc sequential` 7 passed including `as_skill (line 261)`, `v1_severability_tripwire` 18 passed, `make test-skills` exit 0, `pmat quality-gate --checks complexity` Total violations: 0, `--lib skills::projection` 26 passed (>= the required 20) with `prop_frontmatter_roundtrips` present and no proptest failure artifact saved.
