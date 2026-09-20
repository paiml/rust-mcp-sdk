---
phase: 125-sep-2640-conformance-skills-list-skills-get
plan: 03
subsystem: api
tags: [sep-2640, skills, frontmatter, serde_yaml, sha256, diagnostics, validation, tracing, mcp-protocol]

# Dependency graph
requires:
  - phase: 125-01
    provides: "`SkillEntry` / `SkillResourceRef` / `Skills::entries()` with its `Result` signature fixed ahead of its first `Err`, `sha256_digest_hex`, and the crate-private three-way `FrontmatterParse` this plan splits into four."
  - phase: 80-skills
    provides: "`Skill` / `SkillReference` / `Skills`, `SkillsHandler::read` (which defines the exact bytes every manifest row must describe), and the `IndexMap` registration-order contract."
provides:
  - "COMPLETE `resources` manifests — SKILL.md first, then every registered reference — whose digests and sizes are provably the bytes `resources/read` serves for the same URIs."
  - "Verbatim frontmatter including nested mappings and list-valued fields, on LF and CRLF alike, emitted only from the single YAML parse and never from `resolved_description()`."
  - "D-02 warn-and-exclude with THREE distinct diagnostics — absent / invalid / not-a-mapping — each emitting exactly one `tracing::warn!` naming the excluded skill's URI."
  - "The gap-4c frontmatter name-identity REJECT, shared by `Skills::entries()` and `Skills::into_handler()` through one `validate_names` function."
  - "Gap 4a (constructor-name mismatch) and the SEP-2640 Limits bounds as WARNINGS, with the DoS-mitigation claim explicitly withdrawn in source (R-22)."
  - "`SkillBuildArtifact` + `build_artifacts`: one YAML parse per skill per build, consumed by all three build-time consumers (R-21)."
affects: [125-04 index.json retirement, 125-05 make test-skills and deferrals]

actuals:
  tokens: 16892   # chars/4 over the realized diff (67,567 chars, 3324268b~1..HEAD)
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Parse-once build artifact: one private `build_artifact(&Skill)` pass produces the parse outcome, the manifest and every diagnostic; validation, entry synthesis and handler construction all consume it rather than re-deriving from `Skill` bodies."
    - "Diagnostic-returning inner function + thin logging wrapper (`entries_with_diagnostics` / `entries`), so 'a warning is emitted' is directly assertable without installing a subscriber — and the subscriber test then measures the wrapper itself."
    - "Bounds check as a PURE predicate over already-computed totals, so the exact-bound cases are testable with zero allocation, plus one wiring test through the real pipeline so the predicate is proven connected rather than merely correct."
    - "Honest threat-register wording: a guard's rustdoc states what it is NOT and names the control that actually provides the property."

key-files:
  created: []
  modified:
    - src/server/skills.rs
    - tests/skills_integration.rs

key-decisions:
  - "`FrontmatterParse` gained a FOURTH outcome, `NotAMapping`, split out of `Invalid`. The plan's Task-2 `<behavior>` requires three distinct non-`Parsed` diagnostics, which two non-`Parsed` parse outcomes cannot produce. One pre-existing test's destructuring pattern moved from `Invalid` to `NotAMapping`; its assertion text is unchanged."
  - "`exceeds_skill_limits(count, bytes)` returns `Option<SkillLimitBreach>`, not `Option<SkillDiagnostic>`. A `SkillDiagnostic` cannot be constructed without the URI the same task's `<behavior>` requires it to name, and returning one would allocate — contradicting the plan's own 'allocates nothing'. The caller wraps the breach with the URI."
  - "`validate_names` is a free function over `&[SkillBuildArtifact]`, not `Skills::validate_names(&self)`. The `&self` form would have to re-run `build_artifacts`, which is the exact re-parsing R-21 exists to remove, and the method would then be dead code."
  - "Gap 4a ships as `SkillDiagnostic::NameMismatch` (a warning), gap 4c as a hard `Err` from BOTH `entries()` and `into_handler()`. Measured at plan time and re-confirmed here: three in-repo constructions and `pmcp-book`'s taught `.with_path(\"team/topic\")` exercise deliberately give a skill a path whose final segment differs from its constructor name."
  - "The Task-1 property test annotates `skills_strategy_with_refs`'s output with conforming frontmatter before asserting. Without it every generated skill takes the D-02 exclusion path and the property holds over an EMPTY entry set — a vacuous pass. An anti-vacuity assertion (`entries.len() == annotated.len()`) now refuses that outcome."
  - "The capturing subscriber is hand-written against `tracing` itself rather than pulled from `tracing-subscriber`, matching the shipped in-repo idiom at `src/testing/mod.rs:526-597`. The test then carries no feature gate beyond the module's own, so it runs under `--features skills` as well as `--all-features`."

patterns-established:
  - "A negative grep over a `sed` range must be paired with a non-emptiness assertion on that range (R-26). Both halves were run for Task 1: range = 36 lines, `resolved_description` code-line count = 0."
  - "When a plan's acceptance criterion and its own instructions collide, satisfy the INTENT with a precise measurement and record the collision as an explicit deviation — never weaken the check silently and never contort the code to fit."
  - "`Skills::entries()` and `Skills::into_handler()` run the SAME validation function over artifacts of the same shape, so a registry can never produce entries it would refuse to serve, nor the reverse. Both directions are asserted."

requirements-completed: [D-02, D-05]

coverage:
  - id: D1
    description: "A skill's entry `resources` manifest lists its own SKILL.md URI first followed by every registered reference URI, in registration order, and each row's digest and size are the bytes `resources/read` returns for that URI."
    requirement: "D-05"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_manifest_lists_skill_md_first_then_every_reference"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#entry_manifest_names_every_file_and_sizes_match_the_served_bytes"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#prop_manifest_rows_are_the_bytes_the_handler_serves"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every emitted digest matches `^sha256:[0-9a-f]{64}$` and every `size` equals the byte length of the served content, for arbitrary generated registries — a property, not an example, asserted against bytes read back THROUGH the `ResourceHandler`."
    requirement: "D-05"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#prop_manifest_rows_are_the_bytes_the_handler_serves"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#sha256_digest_hex_is_prefixed_lowercase_64_hex"
        status: pass
    human_judgment: false
  - id: D3
    description: "Emitted `frontmatter` is verbatim: a nested `metadata:` object, a list-valued field and a non-required scalar all survive the YAML-to-JSON round trip, and the object is never reconstructed from `resolved_description()` (which `with_description` can legitimately override)."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_frontmatter_is_verbatim_including_nested_and_list_fields"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entries_frontmatter_ignores_the_with_description_override"
        status: pass
      - kind: other
        ref: "sed -n '/pub fn entries/,/^    }/p' src/server/skills.rs | wc -l == 36 (> 5) AND | grep -v '^\\s*//' | grep -c 'resolved_description' == 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "LF-authored and CRLF-authored frontmatter blocks produce identical `frontmatter` JSON — the existing CRLF lock preserved at the entry level."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_frontmatter_is_identical_for_lf_and_crlf"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#lf_and_crlf_frontmatter_are_identical"
        status: pass
    human_judgment: false
  - id: D5
    description: "A skill whose body carries no frontmatter is EXCLUDED from `skills/list` entries, is still enumerated by `resources/list`, is still readable byte-identically via `resources/read`, and produces a build-time warning naming it — never a synthesized `{name, description}` (D-02)."
    requirement: "D-02"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_with_diagnostics_excludes_one_and_names_it"
        status: pass
      - kind: integration
        ref: "tests/skills_integration.rs#frontmatter_less_skill_is_excluded_from_entries_but_still_served"
        status: pass
    human_judgment: false
  - id: D6
    description: "A frontmatter block that is PRESENT but unterminated, unparseable, or parses to a scalar or sequence produces a DIFFERENT diagnostic from an absent block, and each warning names its own case (R-20)."
    requirement: "D-02"
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_diagnose_an_absent_frontmatter_block"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entries_diagnose_an_invalid_frontmatter_block"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#entries_diagnose_a_non_mapping_frontmatter_block"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#the_three_frontmatter_diagnostics_are_distinct_variants"
        status: pass
    human_judgment: false
  - id: D7
    description: "Each skill's frontmatter is parsed exactly ONCE per build: validation, entry synthesis and handler construction all read one shared per-skill build artifact (R-21)."
    verification:
      - kind: other
        ref: "grep -n 'parse_frontmatter_value' src/server/skills.rs — exactly ONE non-test invocation (line 997), inside `build_artifact`, which is the entire body of `Skills::build_artifacts` (`self.skills.iter().map(build_artifact).collect()`)"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#frontmatter_name_identity_is_rejected_by_entries_and_into_handler (both consumers agree because both run the same validate_names over artifacts)"
        status: pass
    human_judgment: false
  - id: D8
    description: "`entries()` emits exactly one `tracing::warn!` per diagnostic, asserted with a capturing subscriber that counts real WARN events rather than inferring from the diagnostic count (R-24)."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#entries_emits_exactly_one_warn_event_per_diagnostic"
        status: pass
    human_judgment: false
  - id: D9
    description: "Build-time validation REJECTS a skill whose final URI segment does not equal its FRONTMATTER `name`, from both `entries()` and `into_handler()`, aggregating every offender — and only when a frontmatter `name` is present (ROADMAP SC#3, gap 4c)."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#frontmatter_name_identity_is_rejected_by_entries_and_into_handler"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#the_name_rule_never_touches_a_skill_without_a_frontmatter_name"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#two_name_mismatches_produce_one_error_naming_both"
        status: pass
    human_judgment: false
  - id: D10
    description: "A skill exceeding 512 resource entries or 16,777,216 total bytes produces a build-time WARNING and is still listed; both bounds are inclusive (ROADMAP SC#3, gap 5)."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#exceeds_skill_limits_bounds_are_inclusive"
        status: pass
      - kind: unit
        ref: "src/server/skills.rs#an_over_count_registry_produces_a_wired_limit_diagnostic"
        status: pass
    human_judgment: false
  - id: D11
    description: "Gap 4a (constructor-name mismatch) warns rather than rejects, and every pre-existing frontmatter-less `Skill::new(...)` call site — including the duplicate-URI tests and both proptest strategies — still compiles and passes (RESEARCH Pitfall 3, corrected by the plan-time measurement)."
    verification:
      - kind: unit
        ref: "src/server/skills.rs#constructor_name_mismatch_warns_rather_than_rejects"
        status: pass
      - kind: other
        ref: "cargo test -p pmcp --all-features --lib skills -- --test-threads=1 → 96 passed, 0 failed, including test_1_8, test_1_8a and prop_1_17_no_reference_ever_listed with unedited assertions"
        status: pass
    human_judgment: false

# Metrics
duration: 35 min
completed: 2026-09-02
status: complete
---

# Phase 125 Plan 03: Complete manifests, verbatim frontmatter, and build-time validation Summary

**`Skills::entries()` now produces entries a conforming SEP-2640 host will accept — complete per-file manifests whose digests are provably the bytes `resources/read` serves, verbatim frontmatter including nested and list-valued fields — and refuses to produce ones a host would reject, with three distinct exclusion diagnostics, a name-identity hard reject shared by both build-time entry points, and a limits warning whose rustdoc withdraws the DoS claim it cannot support.**

## Performance

- **Duration:** 35 min
- **Started:** 2026-09-02T06:30:00Z
- **Completed:** 2026-09-02T07:05:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- **The manifest is complete, and it cannot lie.** Every entry now names its own `SKILL.md` first and then every registered reference in registration order. `skill_resource_manifest` computes each row's digest and size from the exact `&str` that `SkillsHandler::read` returns for the exact same URI, so a divergence between a listing and a later read is unconstructible rather than merely untested (T-125-14). The proptest reads the bytes back **through the `ResourceHandler`** rather than re-deriving them, so the property cannot pass by both sides making the same mistake.

- **The property test would have been vacuous, and that was caught rather than shipped.** `skills_strategy_with_refs` generates bodies from `[a-zA-Z]{0,20}` — bodies that essentially never contain frontmatter, so every generated skill takes the D-02 exclusion path and `entries()` returns an empty vector. Asserting "for every entry, …" over an empty vector passes loudest exactly when nothing is being measured. The generated skills are now annotated with conforming frontmatter (`name` equal to the `p{i}` path the strategy already assigns), and `prop_assert_eq!(entries.len(), annotated.len())` refuses the empty case.

- **Three exclusion diagnostics, not one.** An author whose SKILL.md has a mistyped YAML key is told the block is broken; one with no block is told to add one; one whose block is a list is told SEP-2640 needs top-level `key: value` pairs. This needed a fourth `FrontmatterParse` outcome, because two non-`Parsed` parse states cannot produce three distinct diagnostics.

- **The warning loop is measured, not claimed.** A capturing `tracing` subscriber counts real WARN events during a live `entries()` call and asserts the count equals the diagnostic count and that each captured event names its skill's URI. Without it, the loop that turns diagnostics into warnings could be deleted and every other test would stay green.

- **The name rules are split exactly where the ROADMAP splits them.** Gap 4c — frontmatter `name` versus the URI's final segment — is a hard `Err` from **both** `entries()` and `into_handler()`, aggregating every offender into one message. Gap 4a — constructor name versus that segment — is a warning, because `src/server/skills.rs:846-853`, `:864-871`, the `skills_strategy_with_refs` strategy and `pmcp-book/src/ch12-8-skills.md:392` all deliberately violate it for reasons unrelated to naming. All four still pass with unedited assertions.

- **The limits guard says what it is.** Its rustdoc records that it fires strictly *after* the bodies it would need to bound are already retained and parsed, that pmcp therefore makes no DoS-mitigation claim for it, and that the streamable-HTTP collected-body cap is the control that actually bounds allocation from an untrusted peer (R-22). The bounds themselves are exercised through a pure zero-allocation predicate at exactly 512 / 16,777,216 and one past each, plus one over-COUNT registry through the real synthesis path so the predicate is proven *wired* rather than merely correct.

- **One parse per skill per build.** `SkillBuildArtifact` + `build_artifacts` replaced a path where a registry's YAML could be parsed three or more times per server build. `parse_frontmatter_value` now has exactly ONE non-test call site.

## Task Commits

1. **Task 1 (tdd): complete resources manifests and verbatim frontmatter** — `3324268b` (feat)
2. **Task 2 (tdd): warn and exclude frontmatter-less skills (D-02)** — `9fe2a055` (feat)
3. **Task 3 (tdd): frontmatter name-identity reject and the SEP limits warning** — `f6962639` (feat)

_No separate RED commits: each task's tests and implementation are one behavioural change to a module whose existing suite would not compile against a half-landed enum split, and a `test(...)` commit that does not build is a claim rather than a gate. Every task's `<verify>` block ran green before its commit._

## Files Created/Modified

- `src/server/skills.rs` — `skill_resource_manifest` (the complete-manifest invariant + its rustdoc); `FrontmatterParse::NotAMapping` as a fourth outcome; `SkillDiagnostic` (5 variants) with `uri()` and `message()`; `SkillLimitBreach` + `MAX_SKILL_RESOURCES` / `MAX_SKILL_TOTAL_BYTES` + the pure `exceeds_skill_limits`; `SkillBuildArtifact` + `build_artifact` + `Skills::build_artifacts`; `final_path_segment`; `validate_names`; `Skills::entries_with_diagnostics` and the thin warning `entries()` wrapper; `into_handler` now runs `validate_names` first; expanded rustdoc on `entries()` (complete manifest, verbatim-from-file, strict-mode deferral as prose) and on `into_handler` / `SkillEntry::resources`; **17 new tests** including two proptests and a hand-written capturing subscriber.
- `tests/skills_integration.rs` — three new integration tests: the full-manifest-versus-served-bytes check, LF/CRLF frontmatter equality, and the D-02 still-listed-still-readable proof.

## Decisions Made

See `key-decisions` in the frontmatter. The three most consequential:

1. **`FrontmatterParse` had to become four-way.** The plan asked for three distinct non-`Parsed` diagnostics while 125-01 shipped two non-`Parsed` parse outcomes. The split is the honest fix; the alternative (deriving "not a mapping" from the reason *string*) would have made a user-facing diagnostic load-bearing for control flow.

2. **`validate_names` is a free function over artifacts.** The plan's `pub(crate) fn validate_names(&self) -> Result<()>` shape would have to call `build_artifacts()` itself, which is precisely the re-parsing R-21 exists to remove — and, once both real consumers already hold artifacts, the method would be dead code.

3. **The limits predicate returns a breach, not a diagnostic.** The plan's stated `exceeds_skill_limits(count, bytes) -> Option<SkillDiagnostic>` cannot produce a diagnostic that names its URI (the same task's `<behavior>` requires it to), and constructing one allocates, contradicting the same sentence's "allocates nothing". Returning `Option<SkillLimitBreach>` satisfies both halves; the caller adds the URI.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `FrontmatterParse` needed a fourth variant, moving one pre-existing test's destructuring pattern**

- **Found during:** Task 2 (design of the three-way diagnostic mapping)
- **Issue:** Task 2's `<behavior>` requires an absent block, a broken block and a non-mapping block to produce three DIFFERENT `SkillDiagnostic` variants. 125-01 shipped `FrontmatterParse { Absent, Parsed, Invalid }` — two non-`Parsed` outcomes, which cannot discriminate three cases without re-reading the diagnostic *string*. The same task's acceptance criteria also say every pre-existing test must pass "without edits to its assertions", and `parse_frontmatter_value_invalid_is_not_absent` case 3 binds `FrontmatterParse::Invalid(reason)` for the sequence case.
- **Fix:** Added `FrontmatterParse::NotAMapping(String)` and moved the not-a-mapping producer onto it. In the pre-existing test, only the `let … else` **destructuring pattern** changed (`Invalid` → `NotAMapping`); the `assert!(reason.contains("must be a YAML mapping"))` assertion is byte-identical, and a comment at the site records the move and why. Reading the criterion precisely — it constrains *assertions* — it is satisfied.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `parse_frontmatter_value_invalid_is_not_absent` passes unchanged in substance; `the_three_frontmatter_diagnostics_are_distinct_variants` proves the three are pairwise distinct in both variant and message.
- **Committed in:** `9fe2a055`

**2. [Rule 3 - Blocking] `exceeds_skill_limits` returns `Option<SkillLimitBreach>`, not `Option<SkillDiagnostic>`**

- **Found during:** Task 3 (step 3)
- **Issue:** The plan specifies `exceeds_skill_limits(count: usize, bytes: u64) -> Option<SkillDiagnostic>` that "allocates nothing", while the same task's `<behavior>` requires the limit diagnostic to name the skill's URI. Those cannot both hold: a URI-naming diagnostic needs the URI (absent from the signature) and a `String` (an allocation).
- **Fix:** The predicate stays two-argument and genuinely pure/zero-allocation, returning a `Copy` `SkillLimitBreach { TooManyResources(usize) | TooManyBytes(u64) }`. `build_artifact` wraps it as `SkillDiagnostic::LimitExceeded { uri, breach }`. The R-23 intent — exact-bound testing with no skill bodies allocated — is met exactly.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `exceeds_skill_limits_bounds_are_inclusive` (no allocation at all) and `an_over_count_registry_produces_a_wired_limit_diagnostic` (the real pipeline).
- **Committed in:** `f6962639`

**3. [Rule 3 - Blocking] `validate_names` is a free function over `&[SkillBuildArtifact]`**

- **Found during:** Task 3 (step 4)
- **Issue:** The plan lists `Skills::validate_names` as `pub(crate) fn (&self) -> Result<()>` *and* requires (R-21) that it consume `build_artifacts` output. A `&self` method must build its own artifacts, which reintroduces the second parse pass the same step exists to remove; and once `entries_with_diagnostics` and `into_handler` hold artifacts already, the method has no caller and becomes dead code under `-D warnings`.
- **Fix:** `fn validate_names(artifacts: &[SkillBuildArtifact]) -> Result<()>`, called from `entries_with_diagnostics` and `into_handler` on artifacts each already holds. The plan's key link ("both must share one validation function") holds literally, and both directions are asserted.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `frontmatter_name_identity_is_rejected_by_entries_and_into_handler` asserts BOTH entry points `Err`; `the_name_rule_never_touches_a_skill_without_a_frontmatter_name` asserts BOTH accept.
- **Committed in:** `f6962639`

**4. [Rule 2 - Missing Critical] The Task 1 proptest would have passed vacuously**

- **Found during:** Task 1 (step 4)
- **Issue:** The plan says to reuse `skills_strategy_with_refs` and assert "for every generated registry that yields entries". That generator's bodies are `[a-zA-Z]{0,20}` — no frontmatter, ever — so `entries()` returns an EMPTY vector for essentially every case and the whole property quantifies over nothing. This is the same defect class R-26 names in this plan's own review table.
- **Fix:** A helper re-bodies each generated skill with conforming frontmatter whose `name` equals the `p{i}` path the strategy already assigns, and the property opens with `prop_assert_eq!(entries.len(), annotated.len())` so an empty entry set fails rather than passes. The generator itself is untouched and still shared with `prop_1_17` / `prop_1_19a`.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `prop_manifest_rows_are_the_bytes_the_handler_serves` passes with the anti-vacuity assertion live.
- **Committed in:** `3324268b`

**5. [Rule 1 - Bug in the plan's own acceptance criterion] The LF/CRLF equality criterion is vacuous against the shipped fixtures**

- **Found during:** Task 1 (writing the integration assertion)
- **Issue:** The criterion asks for "equal `frontmatter` JSON for every key other than the deliberately-differing name". Measured: `build_widget_skill_crlf` differs from `build_widget_skill_lf` in BOTH of its frontmatter fields — `name` *and* `description` — and those are the only two keys. Excluding the differing name leaves a one-key comparison whose other side also differs; excluding both leaves nothing to compare.
- **Fix:** The CRLF twin is derived from the LF fixture's own body inside the test, so EVERY key must match — a strictly stronger check — with an explicit non-vacuity assertion that the compared object has real keys. The shipped CRLF fixture is still exercised, for key-set equality, so the separately-authored CRLF file remains covered. The test's rustdoc records why.
- **Files modified:** `tests/skills_integration.rs`
- **Verification:** `lf_and_crlf_frontmatter_are_identical`; plus the in-module `entries_frontmatter_is_identical_for_lf_and_crlf`.
- **Committed in:** `3324268b`

**6. [Rule 3 - Blocking] A temporary `#[allow(clippy::enum_variant_names)]` in Task 2, removed in Task 3**

- **Found during:** Task 2 (`cargo clippy --all-targets --all-features -- -D warnings`)
- **Issue:** With only its three frontmatter variants, `SkillDiagnostic`'s variants all share the `Frontmatter` prefix and `clippy::enum_variant_names` (deny, via `-D warnings`) fails the build. Renaming to dodge it would leave `Absent` / `Invalid` / `NotAMapping`, which say nothing about *what* is absent.
- **Fix:** A `// Why:`-annotated `#[allow]` for the one commit, removed in Task 3 when `NameMismatch` and `LimitExceeded` land and the shared prefix no longer exists. No `allow` remains at HEAD.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` clean at both commits; `grep -c 'enum_variant_names' src/server/skills.rs` → 0 at HEAD.
- **Committed in:** `9fe2a055` (added), `f6962639` (removed)

**7. [Rule 3 - Blocking] `SkillBuildArtifact` does not store a `total_bytes` field**

- **Found during:** Task 3 (step 4)
- **Issue:** The plan lists "its byte total" among the artifact's fields. Because the limits check runs inside the same build pass that computes the manifest, nothing downstream reads that total, and an unread private field is a `dead_code` failure under `-D warnings`.
- **Fix:** The total is computed locally in `build_artifact`, checked, and dropped. Everything the plan wanted it *for* — the limit diagnostic naming the byte total — is carried by `SkillLimitBreach::TooManyBytes(u64)`.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` clean.
- **Committed in:** `f6962639`

**8. [Rule 2 - Missing Critical] The capturing subscriber is hand-written against `tracing`, not `tracing-subscriber`**

- **Found during:** Task 2 (step 4a)
- **Issue:** The plan routes the R-24 test through `tracing-subscriber` 0.3.20, which is optional and enabled only by the `logging` feature. That couples a `skills` test to an unrelated feature: it would fail to compile under a plain `cargo test --features skills`, which is exactly the leg 125-05 is adding.
- **Fix:** A ~40-line `tracing::Subscriber` impl installed with the plan's own `tracing::subscriber::with_default`, mirroring the shipped in-repo idiom at `src/testing/mod.rs:526-597`, whose rustdoc gives this same reason. No new dependency, no feature coupling.
- **Files modified:** `src/server/skills.rs`
- **Verification:** `entries_emits_exactly_one_warn_event_per_diagnostic` passes and asserts on CAPTURED events.
- **Committed in:** `9fe2a055`

---

**Total deviations:** 8 auto-fixed (5 × Rule 3 blocking, 2 × Rule 2 missing-critical, 1 × Rule 1 bug in a plan criterion).
**Impact on plan:** No scope creep and no file touched outside `files_modified`. Five of the eight are the plan's own text being internally inconsistent (a three-way diagnostic on a two-way parse; a pure zero-alloc predicate that must allocate a URI-naming diagnostic; a parse-once method that must re-parse; an artifact field with no reader; a vacuous fixture comparison) — each resolved by satisfying the INTENT with a measurement and recording the collision, per this plan's own instruction. The two remaining substantive departures (deviations 4 and 8) both make the result stronger than the literal text: one converts a vacuous property into a real one, the other removes a feature coupling that would have broken 125-05's `make test-skills` leg.

## Issues Encountered

**Two measurement hazards, both known and both handled; no code defect.**

1. **The full `cargo test --all-features` run aborted at `tests/docs04_examples_run.rs`** with the stale-binary guard naming `doc_review_team` and `s50_standalone_vs_sampled`. This is the hazard 125-01 recorded, with one extra wrinkle worth writing down: the guard's suggested command (`cargo build --features full --example <name>`) **does not work from the workspace root** — both examples live in *other* packages (`pmcp-team-servers`, `pmcp-agent`) and `cargo` answers `no example target named ... in default-run packages`. The working form is `cargo build -p <owning-package> --all-features --example <name>`. After that the full suite completed: **139 targets `ok`, 0 `failures:` lines, 0 FAILED result lines**, doctests (479 passed) reached, so the run finished rather than aborting early.

2. **Every grep, `sed` and `cargo` invocation was run through an absolute binary path** (`/usr/bin/grep`, `/usr/bin/sed`, `/Users/guy/.cargo/bin/cargo`), per the hazard 125-01 and 125-02 both recorded. No corruption was observed this session, which is the expected outcome of avoiding the proxy rather than evidence the proxy is fixed.

## Known Stubs

**None introduced. All three of 125-01's stubs are now RESOLVED by this plan:**

| 125-01 stub | Status |
|---|---|
| The `resources` manifest holds only the skill's own SKILL.md | **Resolved** — `skill_resource_manifest` emits SKILL.md plus every reference, asserted at unit, property and integration level. |
| `Skills::entries()` returns `Result` but has no `Err` path | **Resolved** — `validate_names` is that path; both `entries()` and `into_handler()` reach it. |
| Skills yielding `Absent`/`Invalid` are excluded with only a `debug!` breadcrumb | **Resolved** — three distinct diagnostics, one `tracing::warn!` each, counted by a capturing subscriber. |

One boundary worth naming as a boundary rather than a stub: the **16 MiB byte bound is not exercised through the real synthesis path**, only through the pure predicate. That is the plan's own instruction (R-23) and its reasoning is recorded at the test: allocating 16 MiB of `String` bodies to prove an inclusive comparison would cost every future run of the suite for no additional signal, and the over-COUNT test already proves the predicate is wired into the pipeline.

`skill://index.json` is still served and still enumerated. Its retirement is **125-04**'s, unchanged by this plan.

## Threat Flags

None. Every trust boundary this plan crosses is in the plan's own `<threat_model>`, and each disposition landed as written:

- **T-125-11** (arbitrary bytes → `parse_frontmatter_value`) — *mitigate*, landed. The parse never panics or unwraps; unterminated, unparseable and non-mapping inputs all take the exclusion path. Covered by the anchor/alias and twenty-level-nesting fixtures added here, by `skill_strategy`'s arbitrary bodies, and (pending) by 125-05's fuzz target.
- **T-125-12** (unbounded registry) — *accept*, and the withdrawal is now **in source**: `exceeds_skill_limits`'s rustdoc states that the guard fires after the allocation it would need to prevent, that pmcp makes no DoS claim for it, and that the transport's collected-body cap is the real control.
- **T-125-13** (verbatim frontmatter carrying author secrets) — *accept*, reinforced. The existing `# Security` disclosure warning on `entries()` is unchanged and now sits beside the nested-field path, where a `metadata:` block is the likeliest place a secret hides.
- **T-125-14** (manifest disagreeing with served bytes) — *mitigate*, landed and asserted from the handler side, so the property cannot pass by both sides being wrong together.
- **T-125-15** (digest read as an integrity guarantee) — *accept*, documented; `SkillResourceRef`'s `# Security` section is unchanged.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready for 125-04 (wave 3).**

- **The fixture rule now BITES, and it is already satisfied repo-wide.** `validate_names` is a hard reject, so any skill fixture whose frontmatter `name` disagrees with its URI's final segment fails the build. Every in-repo frontmatter-bearing skill was scanned and conforms: `examples/skills/{hello-world,refunds,code-mode}/SKILL.md`, `tests/skills_routing.rs`'s `REFUNDS_BODY`, `tests/skills_integration.rs`'s three fixtures, and every doctest. **Any fixture authored from here on must obey it.**
- **125-02's re-run instruction is discharged:** `cargo test --all-features --test skills_routing -- --test-threads=1` → 18 passed with `validate_names` live.
- **For 125-04:** `SkillsHandler`'s `skill://index.json` path is untouched by this plan, and `Skills::entries()` no longer has any dependency on it. Note that `resources_list_returns_skill_md_and_index_only` and the new `frontmatter_less_skill_is_excluded_from_entries_but_still_served` both assert on `resources/list` contents, so retiring the index touches the second one too (its `uris.contains(&"skill://bare/SKILL.md")` assertion survives; a length assertion is deliberately not used there for exactly this reason).
- **For 125-05:** the `logging` feature is deliberately NOT required by any test added here — the capturing subscriber is hand-written — so the `make test-skills` leg can pin `--features skills` without pulling `tracing-subscriber`. `make quality-gate` still does not reach this module (`--features "full"` excludes `skills`); this plan was verified with `--all-features` throughout.
- **`requirements mark-complete D-02 D-05` was a no-op** and is expected to be: D-02/D-05 are 125-CONTEXT decision IDs, not `REQUIREMENTS.md` requirement IDs (the v2.6 requirement set is PKG-01..PKGR-01). They are recorded in this SUMMARY's `requirements-completed` exactly as 125-01 and 125-02 recorded theirs.

## Self-Check: PASSED

- `src/server/skills.rs` and `tests/skills_integration.rs` both exist on disk and carry the changes (1,361 insertions / 53 deletions across the three commits).
- `git log --oneline --all | grep 3324268b` → found (Task 1, feat).
- `git log --oneline --all | grep 9fe2a055` → found (Task 2, feat).
- `git log --oneline --all | grep f6962639` → found (Task 3, feat).
- Plan `<verification>` re-run at close: `cargo test -p pmcp --all-features --lib skills -- --test-threads=1` → **96 passed, 0 failed**, one `test result: ok` line (the second verify command's `grep -c` returns 1, not 0); `cargo test --all-features -- --test-threads=1` → **139 `test result: ok` lines, 0 FAILED, 0 `failures:` lines**, doctests reached (479 passed); `cargo clippy --all-targets --all-features -- -D warnings` → clean; `make check-todos` → exit 0, "No technical debt comments"; `cargo fmt --all -- --check` → clean.
- Task 1's R-26 pair: `sed -n '/pub fn entries/,/^    }/p' src/server/skills.rs | wc -l` → **36** (> 5, non-empty) AND the same range with comment lines stripped → **0** `resolved_description` hits.
- Task 3's parse-once measurement: `grep -n 'parse_frontmatter_value' src/server/skills.rs` → exactly **one non-test invocation** (line 997), inside `build_artifact`, which is the entire body of `Skills::build_artifacts`.
- Task 3's regression check: `test_1_8`, `test_1_8a` and `prop_1_17_no_reference_ever_listed` all appear in the passing set with unedited assertions.
- Plan `<success_criteria>`: all five met — complete manifests whose digests/sizes are the served bytes ✅; verbatim frontmatter incl. nested and list-valued fields on LF and CRLF ✅; frontmatter-less skills excluded with a named warning, never synthesized ✅; frontmatter name-identity rejected while constructor-name mismatch and limit overruns warn ✅; every pre-existing skills test passing with unedited assertions ✅.

---
*Phase: 125-sep-2640-conformance-skills-list-skills-get*
*Completed: 2026-09-02*
