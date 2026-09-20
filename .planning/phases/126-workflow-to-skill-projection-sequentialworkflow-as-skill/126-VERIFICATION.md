---
phase: 126-workflow-to-skill-projection-sequentialworkflow-as-skill
verified: 2026-09-04T22:26:52Z
status: passed
score: 6/6 must-haves verified (roadmap SC-1..SC-6), plus all 7 plans' must_haves truths
behavior_unverified: 0
overrides_applied: 0
re_verification: no previous VERIFICATION.md existed for this phase
---

# Phase 126: Workflow to skill projection — `SequentialWorkflow::as_skill()` Verification Report

**Phase Goal:** A `SequentialWorkflow` and its projected skill are the SAME content rendered
twice, and cannot drift — because the SDK owns the renderer. `SequentialWorkflow::as_skill()`
derives a SEP-2640-conforming skill from the workflow's already-public introspection surface,
targeting the CURRENT entry shape (digests included) from day one.

**Verified:** 2026-09-04T22:26:52Z
**Status:** passed
**Re-verification:** No — initial verification

## Method note

This report does not accept SUMMARY.md or `126-REVIEW.md` claims at face value. Every truth
below was checked against the actual `src/`, `tests/`, `examples/`, `fuzz/`, `Cargo.toml` and
`CHANGELOG.md` content in this checkout, and every test cited was **run in this session**
(not merely grepped for), using absolute `cargo`/`pmat` binaries (never `rtk`, which this repo's
memory records as corrupting proxied output). The full `RUSTFLAGS="" make quality-gate` was also
re-run in the background for this verification and reached
`✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` — not merely cited from `deferred-items.md`.

Special attention was paid to the review's own warning about this repo's history of
false-greens: I read the body of `yaml_double_quoted`, `sanitize_for_log`, and the widened
proptest generator (`encoder_stressing_text`) rather than trusting their names, and specifically
looked for the exact CR-01 regression tests the review recommended
(`frontmatter_survives_a_line_separator_before_a_document_indicator`,
`frontmatter_survives_a_line_separator_adjacent_to_blanks`) — both exist verbatim and pass.

## Goal Achievement

### Observable Truths (roadmap SC-1..SC-6)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | **SC-1 — Slugification owned by the projection.** `refund_flow` -> `refund-flow`; final URI segment equals frontmatter `name`. | VERIFIED | `src/server/skills/projection.rs` tests `a_boolean_alike_slug_stays_a_json_string`, `slugify_*` (truncation, re-strip, rejects-empty) all pass (`cargo test --lib skills::projection`, 79 passed incl. these). `examples/s56...` run output: `entry uri: skill://refund-flow/SKILL.md` for workflow named `refund_flow`. |
| 2 | **SC-2 — Determinism.** Re-deriving the same workflow yields a byte-equal body; asserted not assumed. | VERIFIED | `prop_sc2_rerender_is_byte_equal`, `sc2_binding_insertion_order_does_not_change_bytes`, `golden_render_is_byte_equal` — all pass. `tests/golden/workflow_skill_projection.md` exists, pinned, includes a `: ` and `#` in its description (non-trivial encoder path), and its byte-equality test passed when I ran it directly. |
| 3 | **SC-3 — Full coverage,** asserted string-by-string. | VERIFIED | `sc3_every_workflow_fact_renders`, `sc3_excluded_accessor_names_are_absent`, `sc3_excluded_execution_mechanics_change_no_byte`, `sc3_frontmatter_carries_exactly_two_keys`, `sc3_no_tool_schema_or_description_reaches_the_body`, `typed_arguments_render_the_lowercase_wire_spellings` — all pass. |
| 4 | **SC-4 — Registry pass-through** on the wire (verbatim frontmatter + `{uri, digest: sha256, size}`, byte-identical `resources/read`). | VERIFIED | `tests/skills_routing.rs` (23/23 pass, incl. `skills_list_returns_a_conforming_entry_on_v2`, `skills_get_returns_the_single_conforming_entry_on_v2`) and `tests/skills_integration.rs::projected_workflow_skill_reads_back_byte_identical`, `entry_manifest_names_every_file_and_sizes_match_the_served_bytes` all pass. Example run confirms a real `sha256:`+64-hex digest and matching served size (1237/1237 bytes). |
| 5 | **SC-5 — Dual-surface invariant** (`as_prompt_text() == body`, tool-name set equality). | VERIFIED | `sc5_prompt_text_equals_body`, `sc5_surface_equivalence_is_set_equality`, `dual_surface_byte_equal_construction_level`, `dual_surface_byte_equal_wire_level_via_get_prompt`, `dual_surface_byte_equal_crlf_and_mixed_line_endings`, `both_handlers_produce_the_same_message_zero` — all pass. |
| 6 | **SC-6 — Projection-time gate warning** on guidance over a side-effecting step. | VERIFIED | `two_side_effecting_guidance_bearing_steps_produce_exactly_two_warnings`, `the_warning_is_a_builder_capability_and_the_bytes_are_identical_either_way`, `without_a_tool_map_nothing_warns_even_for_a_tripping_workflow` all pass. Example run prints exactly one `GuidanceOnSideEffectingStep` warning on `issue_refund`, matching the fixture's single side-effecting+guidance step. |

**Score:** 6/6 roadmap Success Criteria verified (0 present-but-behavior-unverified).

### Amendment truths (D-04a / D-15a / D-16a) and CR-01 fix

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 7 | D-04a: flag-ON transcript is `[0] skill body` · `[1] create_user_intent` (kept) · assistant-plan message suppressed but its validation call (and `Err` propagation) preserved. | VERIFIED | `flag_on_message_zero_is_the_skill_body`, `flag_on_keeps_user_intent_at_index_one`, `flag_on_suppresses_assistant_plan`, `flag_on_still_rejects_an_unregistered_tool`, `flag_off_transcript_is_unchanged` all pass (part of the 26/26 `skills_integration` run). |
| 8 | D-15a: slug normalization truncates to 64 chars and re-strips a truncation-created trailing hyphen; nothing panics. | VERIFIED | `slugify_truncates_to_sixty_four`, `slugify_restrips_a_hyphen_created_by_truncation`, `slugify_rejects_a_name_with_nothing_legal`, `slugify_rejects_the_empty_name` all pass. |
| 9 | D-16a: the example is `s56_workflow_skill_projection` (not `s45`, which is taken), `required-features = ["skills","full"]`. | VERIFIED | `Cargo.toml:716-718` confirms name/path/required-features; `s45_tool_as_task_lifecycle` confirmed still separately registered. `cargo run --example s56_workflow_skill_projection --features skills,full` exits 0, prints `All assertions passed.` |
| 10 | **CR-01 fix (post-plan, `e3013129`):** `yaml_double_quoted` now escapes `U+2028`/`U+2029` as the `\u2028`/`\u2029` escape sequences (spelled out in text here rather than embedded literally, deliberately, since these are the exact two codepoints CR-01 is about); `sanitize_for_log` (WR-06) and three proptest generators (WR-02) share the fix. | VERIFIED | Read `yaml_double_quoted` (projection.rs:318-365) and `sanitize_for_log` (:289-299) directly — both now branch on `is_unicode_line_separator`. The exact regression tests the review's fix recommended exist verbatim: `frontmatter_survives_a_line_separator_before_a_document_indicator`, `frontmatter_survives_a_line_separator_adjacent_to_blanks`, `sanitize_for_log_replaces_the_unicode_line_separators`, `the_encoder_and_the_log_sanitizer_agree_on_the_line_separators`. All pass. `encoder_stressing_text()` (WR-02 fix) replaces the old blind `".*"` generator with one that samples `U+2028`/`U+2029`, adjacent blanks, and document-indicator sequences 3-in-5 draws; `the_stressing_generator_reaches_every_escape_class` asserts non-vacuity. `CHANGELOG.md:123-149` carries the required D-14 CHANGELOG entry ("Fixed — `U+2028`/`U+2029` are now escaped..."). Golden bytes unchanged (fixture contains neither codepoint), confirmed by a passing `golden_render_is_byte_equal`. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/skills/mod.rs` | D-03 split, former `skills.rs` contents | VERIFIED | Exists (3760 lines); old `src/server/skills.rs` confirmed gone; `git log --follow` on `mod.rs` shows history preserved back through Phase 125/80. |
| `src/server/skills/projection.rs` | New renderer module | VERIFIED | Exists (3099 lines), compiles, 79 tests pass, contains all rendering functions cited by every plan. |
| `src/server/workflow/sequential.rs` | `as_skill()` entry point | VERIFIED | `SequentialWorkflow::as_skill` doctest passes (`--doc sequential`, 7/7). |
| `src/server/workflow/prompt_handler.rs`, `task_prompt_handler.rs` | D-04a opt-in | VERIFIED | Both branches tested (`both_handlers_produce_the_same_message_zero`); doctest for `with_projected_skill_prepend` passes. |
| `src/server/builder.rs`, `src/server/mod.rs` | GATE B builder setters | VERIFIED | `with_workflow_skill_prepend` on both `ServerCoreBuilder` and `ServerBuilder`; reachability tests + doctests pass. |
| `tests/skills_integration.rs`, `tests/skills_routing.rs` | Integration + wire coverage | VERIFIED | 26/23 tests respectively, all pass. |
| `tests/golden/workflow_skill_projection.md` | D-14 golden | VERIFIED | Exists, non-trivial encoder coverage, byte-equality test passes. |
| `fuzz/fuzz_targets/fuzz_workflow_projection.rs`, `fuzz/Cargo.toml`, `.github/workflows/fuzz.yml` | ALWAYS/FUZZ | VERIFIED | `[[bin]]` stanza present, CI matrix row present, registration tripwire test passes. |
| `examples/s56_workflow_skill_projection.rs`, `Cargo.toml` `[[example]]` | ALWAYS/EXAMPLE | VERIFIED | Runs, asserts, exits 0. |
| `CHANGELOG.md` `## [2.20.0] - Unreleased` | D-14 docs | VERIFIED | Present, documents the projection feature, the builder, the D-02 return-shape deviation, and the CR-01 fix. |
| `.planning/phases/126.../deferred-items.md` | Required by plan 07 | VERIFIED | Exists, restates Phase-125 WR-03, all 6 CONTEXT deferred items, the doctest-leg gap (with measured counts I independently reproduced: 7/16/3), GATE B closure, the release-time `Cargo.toml` bump obligation, and — updated post-review — the `pmcp-package-gate` resolution and the 6 deliberately-deferred `126-REVIEW.md` warnings. |

### Data-Flow / Wiring spot checks

- `SequentialWorkflow::as_skill()` -> `SkillProjection::build()`'s renderer -> `render_body()` — same function, confirmed by reading `as_skill()`'s delegation and `build()`'s call site; both produce byte-identical output for inputs that pass `build()`'s strict checks (asserted by `the_warning_is_a_builder_capability_and_the_bytes_are_identical_either_way`).
- Registry pass-through: `Skills::into_handler()` → `SkillsHandler::read()` → wire test — confirmed live over a real loopback `StreamableHttpServer` in `skills_routing.rs`, not merely in-process.
- Prompt-prepend wiring: `ServerCoreBuilder::with_workflow_skill_prepend` → `prompt_workflow` registration-time call → `WorkflowPromptHandler::projected_prepend` → message[0] — confirmed end-to-end in the example (`with .with_workflow_skill_prepend(true) -> message[0] is 1237 bytes, byte-equal to the served skill`) and in `server_core_builder_prompt_workflow_reaches_the_prepend`.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Projection renders, registers, and gate-warns end to end | `cargo run --example s56_workflow_skill_projection --features skills,full` | Exit 0, prints SC-1..SC-6 confirmation lines + `All assertions passed.` | PASS |
| Full skills test surface | `make test-skills` | selector 1: 178 passed · selector 2: 16 passed · selector 3: 26 passed · selector 4: 23 passed | PASS |
| `v1_severability_tripwire` (prohibition: `skills` stays out of `full`) | `cargo test -p pmcp --features full --test v1_severability_tripwire` | 18/18 passed | PASS |
| PMAT cognitive complexity gate | `pmat quality-gate --checks complexity` | `Total violations: 0` | PASS |
| Full project quality gate | `RUSTFLAGS="" make quality-gate` (re-run live for this verification) | `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` | PASS |
| CR-01 fix doctest legs (the 3 dark doctest legs `deferred-items.md` claims counts for) | `cargo test -p pmcp --features skills,full --doc sequential` / `--doc skill_prepend` | 7 passed / 3 passed | PASS (matches claimed counts exactly) |

### Probe Execution

No `scripts/*/tests/probe-*.sh` probes are declared by this phase's plans or referenced by its
success criteria — SKIPPED (no probes applicable to this phase).

### Anti-Patterns Found

None. Scanned every file touched by this phase (`src/server/skills/{mod,projection}.rs`,
`src/server/workflow/{sequential,prompt_handler,task_prompt_handler}.rs`, `src/server/{builder,mod}.rs`,
`examples/s56_workflow_skill_projection.rs`, `fuzz/fuzz_targets/fuzz_workflow_projection.rs`) for
`TODO|HACK|PLACEHOLDER|TBD|FIXME|XXX` — zero matches in any of them.

### Requirements Coverage

No formal REQUIREMENTS.md exists for the v2.7 milestone (confirmed — `.planning/ROADMAP.md:118-119`
states this explicitly and this is the documented, sanctioned shape for this milestone). The
tracked requirement set is `SC-1`..`SC-6` (verified above, all 6/6) plus `126-CONTEXT.md`'s
`D-01`..`D-16` decisions with amendments `D-04a`/`D-15a`/`D-16a` (spot-checked above; all consistent
with the codebase). `ROADMAP.md`'s own decision-coverage gate independently reports
`passed: true, total: 17, covered: 17` for this phase (cited, not re-run, since it requires the
`gsd_run` tool rather than cargo/pmat).

### Deferred Items — judged for adequate provenance, not re-litigated as gaps

Per the task framing, these are recorded as deferred-with-provenance, not as verification gaps.
I independently confirmed each deferred item's underlying code state still matches what
`deferred-items.md` and `126-REVIEW.md` describe (i.e., the deferral record is accurate, not stale):

| Item | Deferred by | Verified still accurately described |
|------|-------------|--------------------------------------|
| Phase 125 WR-03 (`finalize_skills_resources` panics in `build()`) | Phase 125, declined again here with reasoning (unreachable through this phase's surface) | Not re-checked line-for-line (out of this phase's diff by the deferral's own argument); the argument that a projected skill's name/URI are both derived from one `slugify()` call and thus cannot trip `validate_names` holds by construction, confirmed by reading `as_skill()`'s single call to `slugify`. |
| 6 CONTEXT `<deferred>` items (tri-surface docs, AgentPackage pins, default-on prepend, `.strict(true)`, provenance frontmatter key, frozen semver render) | Discussion-time decision, `126-CONTEXT.md` | None appear in the diff; confirmed no `x-pmcp-source`-style key in `render_frontmatter`, no `.strict(` on `SkillProjection`, default is `false` for `with_workflow_skill_prepend` in both builder setters. |
| Doctest-leg Makefile gap (3 dark legs) | Tooling-phase scope, not this phase's | Counts (7/16/3) independently reproduced exactly. |
| WR-01 (emptiness-predicate asymmetry) | Human, in the scoped gap-closure | Confirmed still present: `resolve_description` uses `.is_empty()` (line 630), `build()`'s check uses `.trim().is_empty()` (line 1257). |
| WR-03 (raw body interpolation of author text) | Human | Confirmed still present: `render_body` interpolates `description` raw after YAML-encoding it separately for frontmatter (line 645-652). |
| WR-04 (example not reached by any gate) | Human | Confirmed: `make check` uses `--features "full"` and has no `s56`-reaching leg; no `tests/docs*_examples_run.rs` reference to `s56`. The example DOES run and pass when invoked directly (see Behavioral Spot-Checks), so the phase's own ALWAYS requirement is satisfied by manual execution — the gap is specifically that no automated **gate** reaches it, which is what's deferred, not the requirement itself. |
| WR-05 (unconditional `## Procedure` heading) | Human | Confirmed still present: `render_procedure` has no empty-guard, unlike `render_context`/`render_inputs`. |
| WR-07 (no role-sequence assertion for the two-adjacent-User-messages transcript shape) | Human | Confirmed: no `roles[..2]`-style assertion exists in `tests/skills_integration.rs`. |
| WR-08 (dead `.min()` guard in fuzz chunker) | Human | Confirmed still present verbatim in `fuzz_workflow_projection.rs:125-127`. |
| `pmcp-package-gate` NFC assertion (`dc0eb3e7`) | Outside phase 126's own code (workspace-excluded crate); fixed by the orchestrator at human direction | Confirmed: `crates/pmcp-package` has its own empty `[workspace]` table and no `pmcp` dependency; this fix is legitimately out of phase 126's scope, and `make quality-gate`'s green run (re-verified live) confirms the fix holds. |
| Release-time `Cargo.toml` version bump to 2.20.0 | Releaser's responsibility, not this phase's | `Cargo.toml:3` still reads a pre-2.20.0 version; `CHANGELOG.md` carries `## [2.20.0] - Unreleased`. This is correctly a releaser task, not a phase-126 gap. |

None of the above block the phase goal: the SAME-content-rendered-twice claim (the phase goal
itself) is proven by SC-2 and SC-5's passing tests plus the example's live demonstration, and the
one defect that threatened that claim's actual soundness (CR-01 — silent exclusion from
`skills/list`, and silent value corruption of the very bytes the digest and D-05's "one string, one
digest" claim depend on) is fixed, tested with the reviewer's own recommended regressions, and
documented in the CHANGELOG per D-14.

### Human Verification Required

None. This phase's deliverable is a pure-Rust library surface (a renderer, a builder, two prompt-handler
opt-ins) with no UI, no external service integration, and no behavior that a passing automated test
cannot observe. Every truth above was verified either by a test I ran in this session or by direct
code inspection matched against the plans' own must_haves wording. The deferred items are recorded
with named owners in `deferred-items.md` and were independently confirmed to still match the current
code state (not stale).

### Gaps Summary

No gaps. All 6 roadmap Success Criteria are verified against passing tests I ran directly, the
CR-01 blocker from `126-REVIEW.md` is fixed and covered by exactly the regression the reviewer
prescribed, the full `make quality-gate` passes, PMAT complexity is 0 violations, and the six
review warnings the human chose to defer are recorded with accurate, currently-true provenance
in `deferred-items.md` rather than silently dropped.

---

_Verified: 2026-09-04T22:26:52Z_
_Verifier: Claude (gsd-verifier)_
